use rusqlite::params;
use super::channels::{self, discord, http, slack, telegram, web, whatsapp};
use super::{brief, gateway, guard, schedule, *};
use serde_json::json;
use super::shared_channels::Running;

const MINUTE: i64 = 60_000;
const DAY: i64 = 24 * 60 * MINUTE;

struct Fixture {
    store: Store,
    root: std::path::PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!("pandacode-bots-{}", crate::storage::id()));
    std::fs::create_dir_all(&root).unwrap();
    Fixture { store: Store::open(std::path::Path::new(":memory:")).unwrap(), root }
}

fn input(name: &str) -> BotInput {
    BotInput { name: name.into(), role: "Research assistant".into(), instructions: "Be brief.".into(), provider: "claude".into(), model: "default".into(), mode: "ask".into(), ..Default::default() }
}

fn bot(f: &Fixture) -> Bot {
    create(&f.store, &input("Scout"), &f.root).unwrap()
}

#[test]
fn optional_skill_failures_preserve_distinct_provisioning_statuses() {
    for (reason, expected) in [
        ("reviewer_skill_unavailable: missing skill", "unavailable"),
        ("reviewer_skill_untrusted: safety issue", "untrusted"),
        ("reviewer_skill_truncated: prompt limit", "truncated"),
        ("reviewer_skill_drift: replaced skill", "outdated"),
        ("reviewer_skill_cycle: dependency loop", "incompatible"),
        ("reviewer_skill_incompatible: unsupported harness", "incompatible"),
        ("invalid source", "skipped"),
    ] {
        assert_eq!(shared_channels::skipped_skill_status(reason), expected);
    }
}

#[test]
fn shared_channel_council_is_opt_in_and_never_selects_outsiders_or_executors() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let lead = bot(&f);
    let peer = create(&f.store, &input("Peer"), &f.root).unwrap();
    let outsider = create(&f.store, &input("Outsider"), &f.root).unwrap();
    let channel = shared_channels::create(&f.store, "reviews", &[lead.id.clone(),peer.id.clone()]).unwrap();
    let config = crate::architect::CouncilConfig::default();
    assert_eq!(shared_channels::channel_delivery_thread("first",false), DESKTOP);
    assert_ne!(shared_channels::channel_delivery_thread("first",true), DESKTOP);
    assert_ne!(shared_channels::channel_delivery_thread("first",true),
        shared_channels::channel_delivery_thread("second",true));
    assert!(crate::architect::validate_channel_reviewers(
        &f.store,&config,&channel.member_ids,std::slice::from_ref(&lead.id)).is_ok());
    let mut selected = config.clone();
    selected.reviewer_bot_ids = vec![peer.id.clone(), outsider.id.clone()];
    assert!(crate::architect::validate_channel_reviewers(
        &f.store,&selected,&channel.member_ids,std::slice::from_ref(&lead.id)
    ).unwrap_err().contains("member"));
    selected.reviewer_bot_ids[1] = lead.id.clone();
    assert!(crate::architect::validate_channel_reviewers(
        &f.store,&selected,&channel.member_ids,std::slice::from_ref(&lead.id)
    ).unwrap_err().contains("executor"));
    let message_id = crate::storage::id();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES(?1,?2,'user','Review this','queued',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    assert!(shared_channels::council_request(&f.store,&message_id).unwrap().is_none());
    f.store.db.execute(
        "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at) VALUES(?1,?2,?3,?4)",
        params![message_id,serde_json::to_string(&config).unwrap(),
            serde_json::to_string(&vec![lead.id.clone()]).unwrap(),crate::storage::now()],
    ).unwrap();
    let frozen = shared_channels::council_request(&f.store,&message_id).unwrap().unwrap();
    assert_eq!(frozen.council,config);
    assert_eq!(frozen.executors,vec![lead.id]);
}

#[test]
fn council_channel_deliveries_use_distinct_native_tasks_and_never_touch_desktop_history() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let lead = bot(&f);
    let peer = create(&f.store, &input("Peer"), &f.root).unwrap();
    let channel = shared_channels::create(&f.store, "quality", &[lead.id.clone(),peer.id.clone()]).unwrap();
    let original = thread_session(&f.store,&lead,DESKTOP,DESKTOP,"Scout · You").unwrap().0;
    let message_id = crate::storage::id();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES(?1,?2,'user','Review work','in_progress',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    let council = crate::architect::CouncilConfig::default();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at) VALUES(?1,?2,?3,?4)",
        params![message_id,serde_json::to_string(&council).unwrap(),
            serde_json::to_string(&vec![lead.id.clone(),peer.id.clone()]).unwrap(),crate::storage::now()],
    ).unwrap();
    let first_id = crate::storage::id();
    let (_,_,first_prompt,first_thread) = shared_channels::provision_channel_delivery(
        &f.store,&channel.id,&message_id,&lead.id,"Review work",shared_channels::ChannelDeliveryAdmission { created_at: crate::storage::now(), depth: 0 },&first_id,
    ).unwrap();
    let second_id = crate::storage::id();
    let (_,_,second_prompt,second_thread) = shared_channels::provision_channel_delivery(
        &f.store,&channel.id,&message_id,&peer.id,"Review work",shared_channels::ChannelDeliveryAdmission { created_at: crate::storage::now(), depth: 0 },&second_id,
    ).unwrap();
    assert_ne!(first_thread,second_thread);
    assert_ne!(first_thread,DESKTOP);
    assert_ne!(second_thread,DESKTOP);
    assert!(first_prompt.contains("Review work") && second_prompt.contains("Review work"));
    let owned: Vec<(String,String)> = {
        let mut stmt=f.store.db.prepare(
            "SELECT bot_id,session_id FROM bot_shared_channel_deliveries WHERE message_id=?1 ORDER BY bot_id",
        ).unwrap();
        let rows = stmt.query_map([&message_id],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        rows.collect::<rusqlite::Result<_>>().unwrap()
    };
    assert_eq!(owned.len(),2);
    assert_ne!(owned[0].1,owned[1].1,"one Council cycle per executor delivery");
    for (bot_id, sid) in &owned {
        let team = crate::architect::load(&f.store,sid).unwrap().unwrap();
        assert_eq!(team.config.council,Some(council.clone()));
        assert_eq!(team.session_id,*sid);
        assert!(channel.member_ids.contains(bot_id));
    }
    let existing: String=f.store.db.query_row(
        "SELECT session_id FROM bot_threads WHERE bot_id=?1 AND channel_id=?2 AND external_id=?2",
        params![lead.id,DESKTOP],|row|row.get(0),
    ).unwrap();
    assert_eq!(existing,original,"ordinary bot desktop thread must never be replaced");
    let cycles: i64=f.store.db.query_row(
        "SELECT COUNT(DISTINCT cycle_id) FROM extended_channel_council_deliveries WHERE source_message_id=?1",
        [&message_id],|row|row.get(0),
    ).unwrap();
    assert_eq!(cycles,2,"delivery cycles must be distinct and persisted");
    let detail = shared_channels::detail(&f.store,&channel.id,100).unwrap();
    let first = detail.messages.iter().find(|item| item.id==first_id).unwrap();
    let second = detail.messages.iter().find(|item| item.id==second_id).unwrap();
    assert_ne!(first.council.as_ref().unwrap().cycle_id,second.council.as_ref().unwrap().cycle_id);
    assert_ne!(first.council.as_ref().unwrap().session_id,second.council.as_ref().unwrap().session_id);
    assert!(owned.iter().any(|(_,sid)| sid == &first.council.as_ref().unwrap().session_id));
    assert!(owned.iter().any(|(_,sid)| sid == &second.council.as_ref().unwrap().session_id));
    assert_eq!(first.council.as_ref().unwrap().state,"Reviewing");
    assert_eq!(second.council.as_ref().unwrap().state,"Reviewing");
    // Stopping one delivery cannot mark its sibling stopped or passed.
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='stopped' WHERE id=?1",
        [&first_id],
    ).unwrap();
    let updated = shared_channels::detail(&f.store,&channel.id,100).unwrap();
    assert_eq!(updated.messages.iter().find(|item| item.id==first_id).unwrap().council.as_ref().unwrap().state,"Stopped");
    assert_eq!(updated.messages.iter().find(|item| item.id==second_id).unwrap().council.as_ref().unwrap().state,"Reviewing");
}


#[test]
fn channel_selected_reviewer_scope_is_revalidated_after_membership_and_reassignment() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let a = create(&f.store,&input("Reviewer A"),&f.root).unwrap();
    let b = create(&f.store,&input("Reviewer B"),&f.root).unwrap();
    let channel = shared_channels::create(&f.store,"scope",&[executor.id.clone(),a.id.clone(),b.id.clone()]).unwrap();
    let delivery_id = crate::storage::id();
    let external_id = format!("channel-council:{delivery_id}");
    let session_id = thread_session(&f.store,&executor,DESKTOP,&external_id,"Scope").unwrap().0;
    let message_id = crate::storage::id();
    let config = crate::architect::CouncilConfig {
        reviewer_bot_ids: vec![a.id.clone(), b.id.clone()],
        ..Default::default()
    };
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES(?1,?2,'user','Review','in_progress',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at) VALUES(?1,?2,?3,?4)",
        params![message_id,serde_json::to_string(&config).unwrap(),
            serde_json::to_string(&vec![executor.id.clone()]).unwrap(),crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,depth,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'queued',0,?6,?6)",
        params![delivery_id,channel.id,message_id,executor.id,session_id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_deliveries(delivery_id,channel_id,source_message_id,executor_bot_id,cycle_id,objective,artifact_revision,created_at) VALUES(?1,?2,?3,?4,?5,'Review','0',?6)",
        params![delivery_id,channel.id,message_id,executor.id,crate::storage::id(),crate::storage::now()],
    ).unwrap();
    assert!(shared_channels::validate_delivery_reviewers(&f.store,&session_id,&config.reviewer_bot_ids).is_ok());
    assert!(shared_channels::validate_delivery_reviewers(&f.store,&session_id,&[])
        .unwrap_err().contains("roster changed"));
    f.store.db.execute(
        "DELETE FROM bot_shared_channel_members WHERE channel_id=?1 AND bot_id=?2",
        params![channel.id,b.id],
    ).unwrap();
    assert!(shared_channels::validate_delivery_reviewers(&f.store,&session_id,&config.reviewer_bot_ids)
        .unwrap_err().contains("no longer a channel member"));
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_members(channel_id,bot_id,position) VALUES(?1,?2,2)",
        params![channel.id,b.id],
    ).unwrap();
    let reassigned = crate::storage::id();
    let other_session = thread_session(&f.store,&a,DESKTOP,DESKTOP,"Reassigned").unwrap().0;
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,depth,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'queued',0,?6,?6)",
        params![reassigned,channel.id,message_id,a.id,other_session,crate::storage::now()],
    ).unwrap();
    assert!(shared_channels::validate_delivery_reviewers(&f.store,&session_id,&config.reviewer_bot_ids)
        .unwrap_err().contains("message executor"));
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='stopped' WHERE id=?1",[&delivery_id],
    ).unwrap();
    assert!(shared_channels::validate_delivery_reviewers(&f.store,&session_id,&config.reviewer_bot_ids)
        .unwrap_err().contains("no longer active"));
}

#[test]
fn two_channel_council_messages_to_one_bot_use_separate_native_tasks() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let channel = shared_channels::create(&f.store,"separate",std::slice::from_ref(&executor.id)).unwrap();
    let mut contexts = Vec::new();
    for objective in ["Inspect alpha","Inspect beta"] {
        let message_id = crate::storage::id();
        let delivery_id = crate::storage::id();
        let council = crate::architect::CouncilConfig::default();
        f.store.db.execute(
            "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES(?1,?2,'user',?3,'in_progress',?4)",
            params![message_id,channel.id,objective,crate::storage::now()],
        ).unwrap();
        f.store.db.execute(
            "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at) VALUES(?1,?2,?3,?4)",
            params![message_id,serde_json::to_string(&council).unwrap(),
                serde_json::to_string(&vec![executor.id.clone()]).unwrap(),crate::storage::now()],
        ).unwrap();
        let (_,_,_,thread) = shared_channels::provision_channel_delivery(
            &f.store,&channel.id,&message_id,&executor.id,objective,
            shared_channels::ChannelDeliveryAdmission { created_at: crate::storage::now(), depth: 0 },&delivery_id,
        ).unwrap();
        let sid: String = f.store.db.query_row(
            "SELECT session_id FROM bot_shared_channel_deliveries WHERE id=?1",[&delivery_id],|row|row.get(0),
        ).unwrap();
        assert_eq!(thread,format!("channel-council:{delivery_id}"));
        assert_eq!(crate::architect::load(&f.store,&sid).unwrap().unwrap().config.council,Some(council));
        contexts.push((delivery_id,sid));
    }
    assert_ne!(contexts[0].1,contexts[1].1);
    let detail = shared_channels::detail(&f.store,&channel.id,100).unwrap();
    let first = detail.messages.iter().find(|m| m.id==contexts[0].0).unwrap().council.as_ref().unwrap();
    let second = detail.messages.iter().find(|m| m.id==contexts[1].0).unwrap().council.as_ref().unwrap();
    assert_ne!(first.cycle_id,second.cycle_id);
    assert_ne!(first.session_id,second.session_id);
    // One delivery may contain frozen skill evidence without leaking that
    // reviewer's skill receipts to the other Council for the same bot.
    let mut team = crate::architect::load(&f.store,&contexts[0].1).unwrap().unwrap();
    team.council_since = 1234;
    let created = 1235;
    let mut worker: crate::architect::Worker = serde_json::from_value(json!({
        "id":"frozen-reviewer","sessionId":"private-reviewer-session",
        "name":"Quality reviewer","profileId":"auto","provider":"codex","model":"gpt-5",
        "reasoningEffort":null,"status":"completed","assignment":"Verify source",
        "summary":"Reviewed the source","error":null,"dependsOn":[],"files":[],
        "attempt":1,"createdAt":created,"updatedAt":created,
        "council":{"round":1,"lens":"security","score":9.5,"startedAt":created,"closedBySelection":false}
    })).unwrap();
    let mut binding = crate::architect::reviewer_security::ReviewerBinding::native(
        &team,&worker.id,created,&worker.provider,&worker.model,
        worker.council.as_ref().unwrap(),
    );
    binding.reviewer_bot_id = Some("selected-reviewer".into());
    binding.snapshot.bot_context = Some(crate::architect::reviewer_security::ReviewerBotContext {
        name: "Selected reviewer".into(),role: "QA".into(),
        instructions: String::new(),goals: String::new(),soul: String::new(),
        reasoning_effort: String::new(),skill_ids: vec!["required-provenance".into()],
    });
    binding.snapshot.skills = Some(crate::skills::reviewer::ReviewerSkills {
        pins: vec![crate::skills::reviewer::SkillPin {
            id: "required-provenance".into(),source_kind:"github".into(),
            source_location:"/sensitive/path/must-not-leak".into(),
            source_ref:Some("main".into()),source_commit:Some("c".repeat(40)),
            content_sha256:"a".repeat(64),file_sha256:Default::default(),
            dependencies:vec![],required_for_acceptance:true,check_required:true,
            delivery_mode:"bounded_prompt".into(),staged_path:None,
            imported:true,bot_assigned:true,eligible:true,supplied:true,
            invoked:false,check_executed:false,evidence_verified:false,
        }],
        prompt: "Frozen methodology only".into(),
        skipped: vec![crate::skills::reviewer::SkippedSkill {
            id:"unsafe-optional".into(),reason:"invalid source".into(),
            bot_assigned:true,eligible:false,supplied:false,
        }],
    });
    let revision = binding.artifact_revision.clone();
    worker.reviewer_binding = Some(binding);
    team.workers.push(worker);
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![contexts[0].1,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    let details = shared_channels::detail(&f.store,&channel.id,100).unwrap();
    let first_skills = &details.messages.iter().find(|m| m.id==contexts[0].0)
        .unwrap().council.as_ref().unwrap().reviewers[0].skills;
    assert_eq!(first_skills.len(),2);
    assert_eq!(first_skills[0].status,"not_verified",
        "native retrieval or a reviewer claim is not proof of executed verification");
    assert!(first_skills[0].required);
    assert_eq!(first_skills[0].source_commit.as_deref(),Some("cccccccccccccccccccccccccccccccccccccccc"));
    assert_eq!(first_skills[1].status,"skipped");
    assert!(details.messages.iter().find(|m| m.id==contexts[1].0)
        .unwrap().council.as_ref().unwrap().reviewers.is_empty());
    let serialized = serde_json::to_string(&details).unwrap();
    assert!(!serialized.contains("/sensitive/path") && !serialized.contains("Frozen methodology"),
        "channel details must expose provenance, not private instructions or local paths");
    let observed = serde_json::from_value(json!({
        "contentSha256":"a".repeat(64),"artifactRevision":revision,
        "invoked":true,"checkExecuted":true,"evidenceVerified":true,
        "checkKind":"pinned_input_integrity"
    })).unwrap();
    team.workers[0].reviewer_binding.as_mut().unwrap()
        .skill_evidence.insert("required-provenance".into(),observed);
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![contexts[0].1,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    let verified = shared_channels::detail(&f.store,&channel.id,100).unwrap();
    let skill = &verified.messages.iter().find(|m| m.id==contexts[0].0)
        .unwrap().council.as_ref().unwrap().reviewers[0].skills[0];
    assert!(skill.native_retrieval && skill.native_check_verified);
    assert_eq!(skill.status,"available");
}


#[test]
fn missing_native_cycle_cannot_downgrade_opted_in_council_to_ordinary_delivery() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let channel = shared_channels::create(
        &f.store, "missing-cycle", std::slice::from_ref(&executor.id),
    ).unwrap();
    let message_id = crate::storage::id();
    let delivery_id = crate::storage::id();
    let session_id = thread_session(
        &f.store, &executor, DESKTOP, DESKTOP, "Council with missing cycle",
    ).unwrap().0;
    let council = crate::architect::CouncilConfig::default();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
         VALUES(?1,?2,'user','Review requested','in_progress',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at)
         VALUES(?1,?2,?3,?4)",
        params![message_id,serde_json::to_string(&council).unwrap(),
            serde_json::to_string(&vec![executor.id.clone()]).unwrap(),crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_deliveries
         (id,channel_id,message_id,bot_id,session_id,status,depth,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,'in_progress',0,?6,?6)",
        params![delivery_id,channel.id,message_id,executor.id,session_id,crate::storage::now()],
    ).unwrap();

    assert!(matches!(
        channel_council_outcome(&f.store, &session_id).unwrap(),
        ChannelCouncilOutcome::Denied(_),
    ), "a Council opt-in without native cycle evidence must fail closed");
    assert_eq!(
        shared_channels::settle_channel_delivery(
            &f.store, &session_id,
            (&delivery_id, &channel.id, &message_id, &executor.id),
            true, "Unreviewed work is not approved.",
        ).unwrap(),
        Some(false),
    );
    assert!(!shared_channels::council_callback_allowed(&f.store, &delivery_id).unwrap(),
        "callback recovery cannot treat a broken Council link as Council-off");
    let (delivery_status, message_status): (String, String) = f.store.db.query_row(
        "SELECT d.status,m.status FROM bot_shared_channel_deliveries d
         JOIN bot_shared_channel_messages m ON m.id=d.message_id
         WHERE d.id=?1",
        [&delivery_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).unwrap();
    assert_eq!(delivery_status, "failed");
    assert_eq!(message_status, "failed");
    let replies: i64 = f.store.db.query_row(
        "SELECT COUNT(*) FROM bot_shared_channel_messages WHERE channel_id=?1 AND kind='reply'",
        [&channel.id], |row| row.get(0),
    ).unwrap();
    assert_eq!(replies, 0);
}

#[test]
fn council_channel_callback_waits_for_approval_and_terminal_claim_is_idempotent() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let channel = shared_channels::create(&f.store,"gate",std::slice::from_ref(&executor.id)).unwrap();
    let message_id = crate::storage::id();
    let delivery_id = crate::storage::id();
    let council = crate::architect::CouncilConfig::default();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
         VALUES(?1,?2,'user','Review','in_progress',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at)
         VALUES(?1,?2,?3,?4)",
        params![message_id,serde_json::to_string(&council).unwrap(),
            serde_json::to_string(&vec![executor.id.clone()]).unwrap(),crate::storage::now()],
    ).unwrap();
    shared_channels::provision_channel_delivery(
        &f.store,&channel.id,&message_id,&executor.id,"Review",
        shared_channels::ChannelDeliveryAdmission { created_at: crate::storage::now(), depth: 0 },
        &delivery_id,
    ).unwrap();
    let session_id: String = f.store.db.query_row(
        "SELECT session_id FROM bot_shared_channel_deliveries WHERE id=?1",
        [&delivery_id], |row| row.get(0),
    ).unwrap();
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='in_progress' WHERE id=?1",
        [&delivery_id],
    ).unwrap();
    assert_eq!(channel_council_outcome(&f.store,&session_id).unwrap(), ChannelCouncilOutcome::Waiting);
    assert_eq!(shared_channels::settle_channel_delivery(
        &f.store,&session_id,(&delivery_id,&channel.id,&message_id,&executor.id),true,"Too early"
    ).unwrap(),None);
    let mut team = crate::architect::load(&f.store,&session_id).unwrap().unwrap();
    team.status = "completed".into();
    team.council_since = 1234;
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![session_id,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    assert_eq!(channel_council_outcome(&f.store,&session_id).unwrap(),ChannelCouncilOutcome::Waiting,
        "an unbound original Council cycle cannot permit completion");
    shared_channels::bind_delivery_cycle(&f.store,&session_id,team.council_since).unwrap();
    assert!(matches!(channel_council_outcome(&f.store,&session_id).unwrap(),ChannelCouncilOutcome::Denied(_)),
        "a bound but unreviewed Council is still denied");
    assert_eq!(shared_channels::settle_channel_delivery(
        &f.store,&session_id,(&delivery_id,&channel.id,&message_id,&executor.id),true,"False success"
    ).unwrap(),Some(false));
    assert_eq!(shared_channels::settle_channel_delivery(
        &f.store,&session_id,(&delivery_id,&channel.id,&message_id,&executor.id),true,"Replay"
    ).unwrap(),None);
    let status: String = f.store.db.query_row(
        "SELECT status FROM bot_shared_channel_deliveries WHERE id=?1",[&delivery_id],|r|r.get(0),
    ).unwrap();
    assert_eq!(status,"failed");
    let replies: i64 = f.store.db.query_row(
        "SELECT COUNT(*) FROM bot_shared_channel_messages WHERE channel_id=?1 AND kind='reply'",
        [&channel.id],|r|r.get(0),
    ).unwrap();
    assert_eq!(replies,0,"neither intermediate nor denied Council may publish a bot reply");
    let detail = shared_channels::detail(&f.store, &channel.id, 100).unwrap();
    let rejected = detail.messages.iter()
        .find(|entry| entry.id == delivery_id).unwrap()
        .council.as_ref().unwrap();
    assert_eq!(rejected.state, "Failed");
    assert!(rejected.notice.as_deref().unwrap_or_default().contains("without approval"),
        "one native delivery-level denial is visible without inserting a forbidden message kind");
    let message_count: i64 = f.store.db.query_row(
        "SELECT COUNT(*) FROM bot_shared_channel_messages WHERE channel_id=?1",
        [&channel.id], |r| r.get(0),
    ).unwrap();
    assert_eq!(message_count, 1, "a denied or replayed Council cannot forge a channel message");
}


#[test]
fn failed_ordinary_channel_delivery_preserves_failure_callback_without_success() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let channel = shared_channels::create(
        &f.store, "failed-delivery", std::slice::from_ref(&executor.id),
    ).unwrap();
    let message_id = crate::storage::id();
    let delivery_id = crate::storage::id();
    let session_id = thread_session(
        &f.store, &executor, DESKTOP, DESKTOP, "Failed delivery",
    ).unwrap().0;
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
         VALUES(?1,?2,'user','Unsuccessful work','in_progress',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_deliveries
         (id,channel_id,message_id,bot_id,session_id,status,depth,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,'in_progress',0,?6,?6)",
        params![delivery_id,channel.id,message_id,executor.id,session_id,crate::storage::now()],
    ).unwrap();
    assert_eq!(
        channel_council_outcome(&f.store, &session_id).unwrap(),
        ChannelCouncilOutcome::Ordinary,
    );
    assert_eq!(
        shared_channels::settle_channel_delivery(
            &f.store, &session_id,
            (&delivery_id, &channel.id, &message_id, &executor.id),
            false, "An unfinished response must not be forwarded.",
        ).unwrap(),
        Some(true),
        "ordinary collaboration still reports a failed delivery to its sender",
    );
    let status: String = f.store.db.query_row(
        "SELECT status FROM bot_shared_channel_deliveries WHERE id=?1",
        [&delivery_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(status, "failed");
    assert!(shared_channels::council_callback_allowed(&f.store, &delivery_id).unwrap(),
        "ordinary failed deliveries remain eligible for a failure callback");
    let replies: i64 = f.store.db.query_row(
        "SELECT COUNT(*) FROM bot_shared_channel_messages
         WHERE channel_id=?1 AND kind='reply'",
        [&channel.id], |row| row.get(0),
    ).unwrap();
    assert_eq!(replies, 0);
    assert_eq!(
        shared_channels::settle_channel_delivery(
            &f.store, &session_id,
            (&delivery_id, &channel.id, &message_id, &executor.id),
            true, "Replay",
        ).unwrap(),
        None,
        "a failed terminal delivery must not be converted into success by replay",
    );
}

#[test]
fn a_cached_pass_without_native_scores_cannot_approve_a_channel_delivery() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let channel = shared_channels::create(
        &f.store, "scoring-gate", std::slice::from_ref(&executor.id),
    ).unwrap();
    let message_id = crate::storage::id();
    let delivery_id = crate::storage::id();
    let council = crate::architect::CouncilConfig::default();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
         VALUES(?1,?2,'user','Review evidence','in_progress',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at)
         VALUES(?1,?2,?3,?4)",
        params![message_id,serde_json::to_string(&council).unwrap(),
            serde_json::to_string(&vec![executor.id.clone()]).unwrap(),crate::storage::now()],
    ).unwrap();
    shared_channels::provision_channel_delivery(
        &f.store, &channel.id, &message_id, &executor.id, "Review evidence",
        shared_channels::ChannelDeliveryAdmission { created_at: crate::storage::now(), depth: 0 },
        &delivery_id,
    ).unwrap();
    let session_id: String = f.store.db.query_row(
        "SELECT session_id FROM bot_shared_channel_deliveries WHERE id=?1",
        [&delivery_id], |row| row.get(0),
    ).unwrap();
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='in_progress' WHERE id=?1",
        [&delivery_id],
    ).unwrap();
    let mut team = crate::architect::load(&f.store, &session_id).unwrap().unwrap();
    team.status = "completed".into();
    team.council_since = 1234;
    team.council_status = Some(crate::architect::CouncilStatus {
        state: "passed".into(), round: 1, lowest: Some(9.5), target: 9.0,
        stale: false, notice: None,
    });
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![session_id,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    shared_channels::bind_delivery_cycle(&f.store, &session_id, team.council_since).unwrap();

    assert!(matches!(
        channel_council_outcome(&f.store, &session_id).unwrap(),
        ChannelCouncilOutcome::Denied(_),
    ), "cached pass labels without native reviewer scores cannot bypass Council");

    // An interrupted callback after a stored terminal transition must make
    // the same native evidence check, not trust a cached verdict string.
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='completed' WHERE id=?1",
        [&delivery_id],
    ).unwrap();
    assert!(!shared_channels::council_callback_allowed(&f.store, &delivery_id).unwrap());
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='in_progress' WHERE id=?1",
        [&delivery_id],
    ).unwrap();
    assert_eq!(
        shared_channels::settle_channel_delivery(
            &f.store, &session_id,
            (&delivery_id, &channel.id, &message_id, &executor.id),
            true, "Unverified work must not be published.",
        ).unwrap(),
        Some(false),
    );
    let replies: i64 = f.store.db.query_row(
        "SELECT COUNT(*) FROM bot_shared_channel_messages WHERE channel_id=?1 AND kind='reply'",
        [&channel.id], |row| row.get(0),
    ).unwrap();
    assert_eq!(replies, 0);
}

#[test]
fn an_approved_council_with_no_text_still_posts_one_truthful_final_result() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let channel = shared_channels::create(
        &f.store, "empty-council-reply", std::slice::from_ref(&executor.id),
    ).unwrap();
    let message_id = crate::storage::id();
    let delivery_id = crate::storage::id();
    let council = crate::architect::CouncilConfig::default();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
         VALUES(?1,?2,'user','Please review','in_progress',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at)
         VALUES(?1,?2,?3,?4)",
        params![message_id,serde_json::to_string(&council).unwrap(),
            serde_json::to_string(&vec![executor.id.clone()]).unwrap(),crate::storage::now()],
    ).unwrap();
    shared_channels::provision_channel_delivery(
        &f.store, &channel.id, &message_id, &executor.id, "Please review",
        shared_channels::ChannelDeliveryAdmission { created_at: crate::storage::now(), depth: 0 },
        &delivery_id,
    ).unwrap();
    let session_id: String = f.store.db.query_row(
        "SELECT session_id FROM bot_shared_channel_deliveries WHERE id=?1",
        [&delivery_id], |row| row.get(0),
    ).unwrap();
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='in_progress' WHERE id=?1",
        [&delivery_id],
    ).unwrap();
    let cycle: String = f.store.db.query_row(
        "SELECT cycle_id FROM extended_channel_council_deliveries WHERE delivery_id=?1",
        [&delivery_id], |row| row.get(0),
    ).unwrap();
    let mut team = crate::architect::load(&f.store, &session_id).unwrap().unwrap();
    team.status = "completed".into();
    team.council_since = 1234;
    team.council_accepted = Some(1234);
    team.changed_at = 1280;
    team.dispatch_revision = 7;
    team.council_status = Some(crate::architect::CouncilStatus {
        state: "accepted".into(), round: 1, lowest: Some(7.5),
        target: 9.0, stale: false, notice: None,
    });
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![session_id,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    shared_channels::bind_delivery_cycle(&f.store, &session_id, 1234).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_acceptances
         (delivery_id,cycle_id,council_since,dispatch_revision,changed_at,actor,reason,accepted_at)
         VALUES(?1,?2,1234,7,1280,'local_user','Reviewed and accepted',?3)",
        params![delivery_id,cycle,crate::storage::now()],
    ).unwrap();
    assert_eq!(channel_council_outcome(&f.store,&session_id).unwrap(),ChannelCouncilOutcome::Approved);
    assert_eq!(
        shared_channels::settle_channel_delivery(
            &f.store, &session_id,
            (&delivery_id, &channel.id, &message_id, &executor.id),
            true, "  ",
        ).unwrap(), Some(true)
    );
    assert_eq!(
        shared_channels::settle_channel_delivery(
            &f.store, &session_id,
            (&delivery_id, &channel.id, &message_id, &executor.id),
            true, "Replay",
        ).unwrap(), None
    );
    let replies: Vec<String> = {
        let mut query = f.store.db.prepare(
            "SELECT text FROM bot_shared_channel_messages
             WHERE channel_id=?1 AND kind='reply'",
        ).unwrap();
        query.query_map([&channel.id], |row| row.get::<_,String>(0))
            .unwrap().collect::<rusqlite::Result<_>>().unwrap()
    };
    assert_eq!(replies, vec!["The assigned bot finished without a text reply."]);
    let status: String = f.store.db.query_row(
        "SELECT status FROM bot_shared_channel_deliveries WHERE id=?1",
        [&delivery_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(status,"completed");
}

#[test]
fn interrupted_github_issue_create_is_recovered_as_unknown_without_replay() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let channel = shared_channels::create(
        &f.store, "unknown-issue", std::slice::from_ref(&executor.id),
    ).unwrap();
    let message_id = crate::storage::id();
    let delivery_id = crate::storage::id();
    let session_id = thread_session(
        &f.store, &executor, DESKTOP, DESKTOP, "Unknown issue",
    ).unwrap().0;
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
         VALUES(?1,?2,'user','Review findings','completed',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_deliveries
         (id,channel_id,message_id,bot_id,session_id,status,depth,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,'completed',0,?6,?6)",
        params![delivery_id,channel.id,message_id,executor.id,session_id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_followups
         (delivery_id,authorization_id,repository,revision,title,body,state,updated_at)
         VALUES(?1,'user-grant','owner/repo','revision-1','Approved','Finding','sending',?2)",
        params![delivery_id,crate::storage::now()],
    ).unwrap();

    shared_channels::reconcile_in_progress(&f.store).unwrap();
    shared_channels::reconcile_in_progress(&f.store).unwrap();
    let (state,error): (String,String) = f.store.db.query_row(
        "SELECT state,error FROM extended_channel_council_followups WHERE delivery_id=?1",
        [&delivery_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).unwrap();
    assert_eq!(state,"unknown");
    assert!(error.contains("interrupted"));
    assert!(shared_channels::claim_post_acceptance_issue(&f.store,&delivery_id).unwrap().is_none(),
        "a restarted GitHub issue action must not be executed twice");
    assert!(shared_channels::github_result_uncertain(
        "GitHub CLI operation timed out. Check the repository before retrying."
    ));
    assert!(!shared_channels::github_result_uncertain(
        "GitHub CLI is not installed."
    ));
}

#[test]
fn council_approval_proof_and_github_followup_have_exactly_one_durable_claim() {
    let f = fixture();
    upgrade(&f.store.db).unwrap();
    let executor = bot(&f);
    let channel = shared_channels::create(&f.store,"acceptance",std::slice::from_ref(&executor.id)).unwrap();
    let message_id = crate::storage::id();
    let delivery_id = crate::storage::id();
    let council = crate::architect::CouncilConfig {
        post_acceptance_issue: Some(crate::architect::PostAcceptanceIssue {
            authorization_id: "explicit-user-action".into(), repository: "owner/repo".into(),
        }),
        ..Default::default()
    };
    f.store.db.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
         VALUES(?1,?2,'user','Review findings','in_progress',?3)",
        params![message_id,channel.id,crate::storage::now()],
    ).unwrap();
    f.store.db.execute(
        "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at)
         VALUES(?1,?2,?3,?4)",
        params![message_id,serde_json::to_string(&council).unwrap(),
            serde_json::to_string(&vec![executor.id.clone()]).unwrap(),crate::storage::now()],
    ).unwrap();
    shared_channels::provision_channel_delivery(
        &f.store,&channel.id,&message_id,&executor.id,"Review findings",
        shared_channels::ChannelDeliveryAdmission { created_at: crate::storage::now(), depth: 0 },
        &delivery_id,
    ).unwrap();
    let sid: String = f.store.db.query_row(
        "SELECT session_id FROM bot_shared_channel_deliveries WHERE id=?1",
        [&delivery_id],|r|r.get(0),
    ).unwrap();
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='in_progress' WHERE id=?1",
        [&delivery_id],
    ).unwrap();
    let cycle: String = f.store.db.query_row(
        "SELECT cycle_id FROM extended_channel_council_deliveries WHERE delivery_id=?1",
        [&delivery_id],|r|r.get(0),
    ).unwrap();
    let mut team = crate::architect::load(&f.store,&sid).unwrap().unwrap();
    team.status = "completed".into();
    team.council_since = 1234;
    team.council_accepted = Some(1234);
    team.changed_at = 1280;
    team.dispatch_revision = 7;
    team.council_status = Some(crate::architect::CouncilStatus {
        state: "accepted".into(),round:1,lowest:Some(7.5),target:9.0,
        stale:false,notice:None,
    });
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![sid,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    shared_channels::bind_delivery_cycle(&f.store,&sid,1234).unwrap();
    assert!(shared_channels::bind_delivery_cycle(&f.store,&sid,9999).is_err(),
        "a new cycle must not overwrite the original deliverable's authorization");
    assert_eq!(channel_council_outcome(&f.store,&sid).unwrap(),ChannelCouncilOutcome::Waiting,
        "a native Council acceptance without attributed reason never grants channel approval");
    f.store.db.execute(
        "INSERT INTO extended_channel_council_acceptances
         (delivery_id,cycle_id,council_since,dispatch_revision,changed_at,actor,reason,accepted_at)
         VALUES(?1,?2,1234,7,1280,'local_user','Accepted limitations',?3)",
        params![delivery_id,cycle,crate::storage::now()],
    ).unwrap();
    assert_eq!(channel_council_outcome(&f.store,&sid).unwrap(),ChannelCouncilOutcome::Approved);
    // A new native file-change record can arrive after the team's saved
    // acceptance snapshot. It must revoke both live and replay admission.
    let change_id = crate::storage::id();
    let changed_at = crate::storage::now();
    let change_message: i64 = f.store.db.query_row(
        "INSERT INTO messages(session_id,role,content,created_at)
         VALUES(?1,'changes','',?2) RETURNING id",
        params![sid, changed_at], |row| row.get(0),
    ).unwrap();
    let change_summary = json!({
        "files":[{"path":"src/after-review.rs","status":"modified","additions":1,"deletions":0,
                  "binary":false,"truncated":false}],
        "ownFiles":1,"attributed":true
    });
    f.store.db.execute(
        "INSERT INTO turn_changes(id,session_id,message_id,summary,created_at,diff_bytes)
         VALUES(?1,?2,?3,?4,?5,0)",
        params![change_id,sid,change_message,change_summary.to_string(),changed_at],
    ).unwrap();
    assert!(matches!(channel_council_outcome(&f.store,&sid).unwrap(),ChannelCouncilOutcome::Denied(_)),
        "a newly persisted change must revoke a saved user exception");
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='completed' WHERE id=?1",
        [&delivery_id],
    ).unwrap();
    assert!(!shared_channels::council_callback_allowed(&f.store,&delivery_id).unwrap(),
        "restart callbacks cannot reuse approval after new native file changes");
    f.store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status='in_progress' WHERE id=?1",
        [&delivery_id],
    ).unwrap();
    f.store.db.execute("DELETE FROM turn_changes WHERE id=?1", [&change_id]).unwrap();
    f.store.db.execute("DELETE FROM messages WHERE id=?1", [change_message]).unwrap();
    assert_eq!(channel_council_outcome(&f.store,&sid).unwrap(),ChannelCouncilOutcome::Approved);
    team.config.council.as_mut().unwrap().target = 2.0;
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![sid,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    assert!(matches!(channel_council_outcome(&f.store,&sid).unwrap(),ChannelCouncilOutcome::Denied(_)),
        "changing the task Council target cannot bypass the message's frozen approval");
    team.config.council.as_mut().unwrap().target = 9.0;
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![sid,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    assert!(!shared_channels::council_callback_allowed(&f.store,&delivery_id).unwrap(),
        "an approved but nonterminal delivery may not publish a callback");
    team.changed_at = 1281;
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![sid,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    assert_eq!(channel_council_outcome(&f.store,&sid).unwrap(),ChannelCouncilOutcome::Waiting,
        "changed evidence invalidates an earlier user exception");
    team.changed_at = 1280;
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![sid,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    assert_eq!(shared_channels::settle_channel_delivery(
        &f.store,&sid,(&delivery_id,&channel.id,&message_id,&executor.id),true,
        "Accepted finding: fix the permission boundary."
    ).unwrap(),Some(true));
    assert!(shared_channels::settle_channel_delivery(
        &f.store,&sid,(&delivery_id,&channel.id,&message_id,&executor.id),true,"Replay"
    ).unwrap().is_none(),"a duplicate callback cannot settle or publish again");
    let published: i64 = f.store.db.query_row(
        "SELECT COUNT(*) FROM bot_shared_channel_messages WHERE channel_id=?1 AND kind='reply'",
        [&channel.id],|r|r.get(0),
    ).unwrap();
    assert_eq!(published,1);
    assert!(shared_channels::council_callback_allowed(&f.store,&delivery_id).unwrap(),
        "only a completed native Council and matching proof may send a callback");
    let pending: (String,String,String) = f.store.db.query_row(
        "SELECT repository,state,authorization_id FROM extended_channel_council_followups
         WHERE delivery_id=?1",
        [&delivery_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).unwrap();
    assert_eq!(pending,("owner/repo".into(),"pending".into(),"explicit-user-action".into()));
    // The durable pending flag alone must never authorize an external write.
    f.store.db.execute(
        "UPDATE extended_channel_council_followups SET repository='outside/scope' WHERE delivery_id=?1",
        [&delivery_id],
    ).unwrap();
    assert!(shared_channels::claim_post_acceptance_issue(&f.store,&delivery_id).unwrap().is_none(),
        "the native tool boundary must reject a repository outside the frozen user grant");
    f.store.db.execute(
        "UPDATE extended_channel_council_followups SET repository='owner/repo' WHERE delivery_id=?1",
        [&delivery_id],
    ).unwrap();
    let frozen_revision: String = f.store.db.query_row(
        "SELECT revision FROM extended_channel_council_followups WHERE delivery_id=?1",
        [&delivery_id], |row| row.get(0),
    ).unwrap();
    f.store.db.execute(
        "UPDATE extended_channel_council_followups SET revision='forged-revision' WHERE delivery_id=?1",
        [&delivery_id],
    ).unwrap();
    assert!(shared_channels::claim_post_acceptance_issue(&f.store,&delivery_id).unwrap().is_none(),
        "GitHub issue publishing must reject a forged delivery/revision binding");
    f.store.db.execute(
        "UPDATE extended_channel_council_followups SET revision=?2 WHERE delivery_id=?1",
        params![delivery_id,frozen_revision],
    ).unwrap();
    team.changed_at = 1281;
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![sid,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    assert!(shared_channels::claim_post_acceptance_issue(&f.store,&delivery_id).unwrap().is_none(),
        "a stale Council exception cannot claim an external GitHub write");
    team.changed_at = 1280;
    f.store.db.execute(
        "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
        params![sid,serde_json::to_string(&team).unwrap()],
    ).unwrap();
    let first=shared_channels::claim_post_acceptance_issue(&f.store,&delivery_id).unwrap().unwrap();
    assert_eq!(first.0,"owner/repo");
    assert!(first.2.contains("Accepted finding"));
    assert!(shared_channels::claim_post_acceptance_issue(&f.store,&delivery_id).unwrap().is_none(),
        "the external GH CLI action is claimed once even if callback is replayed");
    let state: String = f.store.db.query_row(
        "SELECT state FROM extended_channel_council_followups WHERE delivery_id=?1",
        [&delivery_id],|r|r.get(0),
    ).unwrap();
    assert_eq!(state,"sending");
    f.store.db.execute(
        "UPDATE extended_channel_council_followups
         SET state='unknown',error='GitHub result is uncertain; check the repository.'
         WHERE delivery_id=?1",
        [&delivery_id],
    ).unwrap();
    let detail = shared_channels::detail(&f.store,&channel.id,100).unwrap();
    let view = detail.messages.iter().find(|entry| entry.id == delivery_id)
        .unwrap().council.as_ref().unwrap();
    assert_eq!(view.followup_status.as_deref(),Some("unknown"));
    assert!(view.followup_error.as_deref().unwrap().contains("uncertain"),
        "the delivery card exposes a trustworthy follow-up error independently of Council verdict");
}

#[test]
fn creates_a_bot_with_its_own_workspace_project() {
    let f = fixture();
    let bot = bot(&f);
    assert_eq!(bot.name, "Scout");
    assert!(!bot.paused);
    assert_eq!(project_ids(&f.store).unwrap(), vec![bot.project_id.clone()]);
    let name: String = f.store.db.query_row("SELECT name FROM projects WHERE id=?1", [&bot.project_id], |r| r.get(0)).unwrap();
    assert_eq!(name, "Scout");
    assert!(std::fs::read_dir(&f.root).unwrap().count() == 1);
    assert_eq!(list(&f.store).unwrap().len(), 1);
    assert_eq!(activity(&f.store, &bot.id, None, 10).unwrap()[0].kind, "created");
}

#[test]
fn rejects_invalid_bot_settings() {
    let f = fixture();
    assert!(create(&f.store, &input("  "), &f.root).is_err());
    let mut bad = input("Scout");
    bad.mode = "yolo".into();
    assert!(create(&f.store, &bad, &f.root).is_err());
}

#[test]
fn memories_dedupe_and_bump_the_brief_version() {
    let f = fixture();
    let bot = bot(&f);
    let first = remember(&f.store, &bot.id, "Prefers metric units", "user").unwrap();
    let again = remember(&f.store, &bot.id, "prefers METRIC units", "bot").unwrap();
    assert_eq!(first.id, again.id);
    assert_eq!(memories(&f.store, &bot.id).unwrap().len(), 1);
    let version = get(&f.store, &bot.id).unwrap().memory_version;
    assert!(version > bot.memory_version);
    forget(&f.store, &bot.id, &first.id).unwrap();
    assert!(get(&f.store, &bot.id).unwrap().memory_version > version);
    assert!(forget(&f.store, &bot.id, &first.id).is_err());
    assert!(remember(&f.store, &bot.id, "   ", "user").is_err());
}

#[test]
fn editing_instructions_rebriefs_threads() {
    let f = fixture();
    let bot = bot(&f);
    let (session, created) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout · You").unwrap();
    assert!(created);
    assert_eq!(brief_version_of(&f.store, &session).unwrap(), -1);
    mark_briefed(&f.store, &session, bot.memory_version).unwrap();
    let mut changed = input("Scout");
    changed.instructions = "Be thorough.".into();
    let updated = update(&f.store, &bot.id, &changed).unwrap();
    assert!(updated.memory_version > bot.memory_version);
    let (same, created) = thread_session(&f.store, &updated, DESKTOP, DESKTOP, "Scout · You").unwrap();
    assert_eq!((same.as_str(), created), (session.as_str(), false));
    assert_eq!(owner_of(&f.store, &session).unwrap().unwrap().0.id, bot.id);
}

#[test]
fn profile_changes_reconfigure_existing_idle_threads() {
    let f = fixture();
    let bot = bot(&f);
    let (session, _) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout · You").unwrap();
    let mut changed = input("Scout");
    changed.provider = "codex".into();
    changed.model = "gpt-5".into();
    changed.mode = "plan".into();
    changed.reasoning_effort = "high".into();
    update(&f.store, &bot.id, &changed).unwrap();
    let current = f.store.session(&session).unwrap();
    assert_eq!((current.provider.as_str(), current.model.as_str(), current.mode.as_str()), ("codex", "gpt-5", "plan"));
    assert_eq!(current.reasoning_effort.as_deref(), Some("high"));
}

#[test]
fn desktop_model_pick_survives_followups_without_changing_channel_defaults() {
    let f = fixture();
    let scout = bot(&f);
    let (desktop, _) = thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
    let (channel, _) = thread_session(&f.store, &scout, "web", "customer", "Scout · Customer").unwrap();

    // The chat's model picker saves the selected harness on the session itself.
    f.store.configure(&desktop, "kimi", "kimi-test-model", None).unwrap();
    f.store.configure(&channel, "codex", "gpt-5", None).unwrap();
    for _ in 0..3 {
        // The gateway resolves the thread again before every incoming message.
        let (same, created) = thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
        assert_eq!((same.as_str(), created), (desktop.as_str(), false));
        let selected = f.store.session(&desktop).unwrap();
        assert_eq!((selected.provider.as_str(), selected.model.as_str()), ("kimi", "kimi-test-model"));
    }
    let (same, _) = thread_session(&f.store, &scout, "web", "customer", "Scout · Customer").unwrap();
    assert_eq!(same, channel);
    let external = f.store.session(&channel).unwrap();
    assert_eq!((external.provider.as_str(), external.model.as_str()), ("claude", "default"));
    assert_eq!((get(&f.store, &scout.id).unwrap().provider.as_str(), get(&f.store, &scout.id).unwrap().model.as_str()), ("claude", "default"), "a desktop pick does not change the bot's channel/schedule default");

    // Changing access from the chat must not unexpectedly restore the first harness.
    let changed = set_mode(&f.store, &scout.id, "full").unwrap();
    assert_eq!(changed.mode, "full");
    assert_eq!((f.store.session(&desktop).unwrap().provider.as_str(), f.store.session(&desktop).unwrap().model.as_str()), ("kimi", "kimi-test-model"));
    assert_eq!(f.store.session(&desktop).unwrap().mode, "full");

    // Editing an unrelated profile field must also preserve the desktop pick.
    let mut settings = input("Scout");
    settings.role = "Updated role".into();
    update(&f.store, &scout.id, &settings).unwrap();
    assert_eq!(f.store.session(&desktop).unwrap().provider, "kimi");

    // But changing the default harness in Settings intentionally updates all threads.
    settings.provider = "codex".into();
    settings.model = "gpt-5".into();
    update(&f.store, &scout.id, &settings).unwrap();
    let desktop_settings = f.store.session(&desktop).unwrap();
    assert_eq!((desktop_settings.provider.as_str(), desktop_settings.model.as_str()), ("codex", "gpt-5"));
    let channel_settings = f.store.session(&channel).unwrap();
    assert_eq!((channel_settings.provider.as_str(), channel_settings.model.as_str()), ("codex", "gpt-5"));
}

#[test]
fn deferred_desktop_model_change_is_not_reset_before_the_next_turn() {
    let f = fixture();
    let scout = bot(&f);
    let (desktop, _) = thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
    f.store.status(&desktop, "running").unwrap();
    thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
    // The model picker defers a mid-turn change until the current run finishes.
    f.store.status(&desktop, "stopped").unwrap();
    f.store.configure(&desktop, "codex", "gpt-5", None).unwrap();
    thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
    assert_eq!((f.store.session(&desktop).unwrap().provider.as_str(), f.store.session(&desktop).unwrap().model.as_str()), ("codex", "gpt-5"));
}

#[test]
fn chat_permission_change_updates_bot_policy_and_all_idle_threads() {
    let f = fixture();
    let bot = bot(&f);
    let other = create(&f.store, &input("Other"), &f.root).unwrap();
    let (desktop, _) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout · You").unwrap();
    let (channel, _) = thread_session(&f.store, &bot, "web", "client", "Scout · Client").unwrap();
    let (other_thread, _) = thread_session(&f.store, &other, DESKTOP, DESKTOP, "Other · You").unwrap();

    f.store.update(&desktop, None, None, None, Some("full".into())).unwrap();
    let (same, created) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout · You").unwrap();
    assert_eq!((same, created), (desktop.clone(), false));
    assert_eq!(f.store.session(&desktop).unwrap().mode, "ask", "upstream resets task-only modes");

    let changed = set_mode(&f.store, &bot.id, "full").unwrap();
    assert_eq!(changed.mode, "full");
    assert_eq!(changed.role, bot.role);
    assert_eq!(f.store.session(&desktop).unwrap().mode, "full");
    assert_eq!(f.store.session(&channel).unwrap().mode, "full");
    assert_eq!(f.store.session(&other_thread).unwrap().mode, "ask");
    let (same, _) = thread_session(&f.store, &changed, DESKTOP, DESKTOP, "Scout · You").unwrap();
    assert_eq!(same, desktop);
    assert_eq!(f.store.session(&desktop).unwrap().mode, "full");

    assert!(set_mode(&f.store, &bot.id, "unrestricted").is_err());
    assert_eq!(get(&f.store, &bot.id).unwrap().mode, "full");
    set_mode(&f.store, &bot.id, "ask").unwrap();
    assert_eq!(f.store.session(&desktop).unwrap().mode, "ask", "revocation also takes effect from chat");
    assert_eq!(f.store.session(&channel).unwrap().mode, "ask");
}

#[test]
fn active_bot_threads_pick_up_chat_permission_on_the_next_turn() {
    let f = fixture();
    let bot = bot(&f);
    let (desktop, _) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout · You").unwrap();
    f.store.status(&desktop, "running").unwrap();
    set_mode(&f.store, &bot.id, "full").unwrap();
    assert_eq!(f.store.session(&desktop).unwrap().mode, "ask", "active turns are not reconfigured");
    f.store.status(&desktop, "stopped").unwrap();
    let changed = get(&f.store, &bot.id).unwrap();
    thread_session(&f.store, &changed, DESKTOP, DESKTOP, "Scout · You").unwrap();
    assert_eq!(f.store.session(&desktop).unwrap().mode, "full");
    set_mode(&f.store, &bot.id, "plan").unwrap();
    assert_eq!(f.store.session(&desktop).unwrap().mode, "plan");
}

#[test]
fn bot_message_without_fresh_context_reuses_the_desktop_conversation() {
    let f = fixture();
    let source = bot(&f);
    let target = create(&f.store, &input("Partner"), &f.root).unwrap();
    let (desktop, _) = thread_session(&f.store, &target, DESKTOP, DESKTOP, "Partner · You").unwrap();
    let (same, thread) = shared_channels::bot_message_target(&f.store, &source, &target, "first", false).unwrap();
    let (also_same, _) = shared_channels::bot_message_target(&f.store, &source, &target, "second", false).unwrap();
    assert_eq!(thread, DESKTOP);
    assert_eq!(same, desktop);
    assert_eq!(also_same, desktop);
    assert_eq!(threads(&f.store, &target.id).unwrap().len(), 1);
}

#[test]
fn fresh_bot_message_keeps_active_desktop_queue_memory_and_schedules() {
    let f = fixture();
    let source = bot(&f);
    let target = create(&f.store, &input("Partner"), &f.root).unwrap();
    let (source_session, _) = thread_session(&f.store, &source, DESKTOP, DESKTOP, "Scout · You").unwrap();
    let (desktop, _) = thread_session(&f.store, &target, DESKTOP, DESKTOP, "Partner · You").unwrap();
    f.store.status(&desktop, "running").unwrap();
    crate::message_queue::add(&f.store, &desktop, "Keep this queued message", &json!([])).unwrap();
    let memory = remember(&f.store, &target.id, "Use the project glossary", "user").unwrap();
    schedule::create(&f.store, &target.id, &schedule::ScheduleInput {
        prompt: "Check status later".into(),
        kind: "interval".into(),
        interval_minutes: Some(60),
        ..Default::default()
    }, "bot", 0).unwrap();
    let target = get(&f.store, &target.id).unwrap();

    let (fresh, thread) = shared_channels::bot_message_target(&f.store, &source, &target, "fresh-request", true).unwrap();
    assert_eq!(thread, "delegation:fresh-request");
    assert_ne!(fresh, desktop);
    let new_task = f.store.session(&fresh).unwrap();
    assert!(new_task.resume_id.is_none());
    assert_eq!(new_task.project_id, target.project_id);
    assert_eq!(new_task.provider, target.provider);
    assert_eq!(new_task.model, target.model);
    assert_eq!(brief_version_of(&f.store, &fresh).unwrap(), -1, "the recipient receives current instructions and memories on its first turn");
    assert_eq!(owner_of(&f.store, &fresh).unwrap().unwrap().0.id, target.id);

    assert_eq!(f.store.session(&desktop).unwrap().status, "running");
    let queued = crate::message_queue::list(&f.store, Some(&desktop)).unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].text, "Keep this queued message");
    assert!(crate::message_queue::list(&f.store, Some(&fresh)).unwrap().is_empty());
    assert!(memories(&f.store, &target.id).unwrap().iter().any(|item| item.id == memory.id));
    assert_eq!(schedule::list(&f.store, &target.id).unwrap().len(), 1);

    let (same, _) = shared_channels::bot_message_target(&f.store, &source, &target, "fresh-request", true).unwrap();
    let (another, _) = shared_channels::bot_message_target(&f.store, &source, &target, "next-request", true).unwrap();
    assert_eq!(same, fresh, "reopening one delegation must not create a duplicate session");
    assert_ne!(another, fresh, "separate delegations must have separate contexts");

    f.store.db.execute("INSERT INTO bot_messages(id,source_bot_id,target_bot_id,source_session_id,target_session_id,source_channel_id,source_external_id,text,status,depth,created_at,updated_at) VALUES('fresh-request',?1,?2,?3,?4,'desktop','desktop','Review this task','queued',1,1,1)", params![source.id,target.id,source_session,fresh]).unwrap();
    let running = shared_channels::promote_taken(&f.store, &fresh, "Message from Scout:\n\nReview this task").unwrap();
    assert_eq!(running, Running::Message("fresh-request".into()));
    let status = |id: &str| f.store.db.query_row("SELECT status FROM bot_messages WHERE id=?1", [id], |r| r.get::<_, String>(0)).unwrap();
    assert_eq!(status("fresh-request"), "in_progress");
    shared_channels::launch_failed(&f.store, &fresh, &running);
    assert_eq!(status("fresh-request"), "queued", "a failed launch must leave the request recoverable");
    assert_eq!(crate::message_queue::list(&f.store, Some(&desktop)).unwrap().len(), 1);
}

#[test]
fn restarted_councils_recover_independent_live_and_approved_deliveries_once() {
    let f = fixture();
    let db_path = f.root.join("council-restart.sqlite");
    let ids = {
        let store = Store::open(&db_path).unwrap();
        upgrade(&store.db).unwrap();
        let reviewer = create(&store, &input("Reviewer"), &f.root).unwrap();
        let channel = shared_channels::create(&store, "restart", std::slice::from_ref(&reviewer.id)).unwrap();
        let council = crate::architect::CouncilConfig::default();
        let mut deliveries = Vec::new();
        for index in 0..2 {
            let message_id = crate::storage::id();
            let delivery_id = crate::storage::id();
            store.db.execute(
                "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
                 VALUES(?1,?2,'user','Review pending work','in_progress',?3)",
                params![message_id,channel.id,crate::storage::now()],
            ).unwrap();
            store.db.execute(
                "INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at)
                 VALUES(?1,?2,?3,?4)",
                params![message_id,serde_json::to_string(&council).unwrap(),
                    serde_json::to_string(&vec![reviewer.id.clone()]).unwrap(),crate::storage::now()],
            ).unwrap();
            shared_channels::provision_channel_delivery(
                &store, &channel.id, &message_id, &reviewer.id, "Review pending work",
                shared_channels::ChannelDeliveryAdmission { created_at: crate::storage::now(), depth: 0 },
                &delivery_id,
            ).unwrap();
            let session_id: String = store.db.query_row(
                "SELECT session_id FROM bot_shared_channel_deliveries WHERE id=?1",
                [&delivery_id], |row| row.get(0),
            ).unwrap();
            store.db.execute(
                "UPDATE bot_shared_channel_deliveries SET status='in_progress' WHERE id=?1",
                [&delivery_id],
            ).unwrap();
            let mut team = crate::architect::load(&store, &session_id).unwrap().unwrap();
            team.council_since = 1234 + index;
            shared_channels::bind_delivery_cycle(&store, &session_id, team.council_since).unwrap();
            if index == 1 {
                team.status = "completed".into();
                team.council_accepted = Some(team.council_since);
                team.changed_at = 1280;
                team.dispatch_revision = 7;
                team.council_status = Some(crate::architect::CouncilStatus {
                    state: "accepted".into(),round:1,lowest:Some(8.0),target:9.0,
                    stale:false,notice:None,
                });
                let cycle: String = store.db.query_row(
                    "SELECT cycle_id FROM extended_channel_council_deliveries WHERE delivery_id=?1",
                    [&delivery_id], |row| row.get(0),
                ).unwrap();
                store.db.execute(
                    "INSERT INTO extended_channel_council_acceptances
                     (delivery_id,cycle_id,council_since,dispatch_revision,changed_at,actor,reason,accepted_at)
                     VALUES(?1,?2,?3,7,1280,'local_user','Accepted after review',?4)",
                    params![delivery_id,cycle,team.council_since,crate::storage::now()],
                ).unwrap();
                store.db.execute(
                    "INSERT INTO messages(session_id,role,content,created_at)
                     VALUES(?1,'assistant','Approved final result',?2)",
                    params![session_id,crate::storage::now()],
                ).unwrap();
            } else {
                team.status = "waiting".into();
            }
            store.db.execute(
                "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
                params![session_id,serde_json::to_string(&team).unwrap()],
            ).unwrap();
            deliveries.push((delivery_id,session_id,message_id));
        }
        (channel.id, deliveries)
    };
    // Both the native Council and delivery records are genuinely reloaded
    // from a new SQLite connection, not cloned in-memory Rust structs.
    let store = Store::open(&db_path).unwrap();
    upgrade(&store.db).unwrap();
    for _ in 0..2 { shared_channels::reconcile_in_progress(&store).unwrap(); }
    let (channel_id, deliveries) = ids;
    let status = |id: &str| store.db.query_row(
        "SELECT status FROM bot_shared_channel_deliveries WHERE id=?1",
        [id],|row|row.get::<_,String>(0),
    ).unwrap();
    assert_eq!(status(&deliveries[0].0),"in_progress",
        "a reviewing Council must retain its frozen native cycle after restart");
    assert_eq!(status(&deliveries[1].0),"completed",
        "the matching accepted terminal Council must complete after a restart");
    assert_ne!(deliveries[0].1,deliveries[1].1,
        "two deliveries to the same bot must never share a native Council session");
    let replies: Vec<String> = {
        let mut query = store.db.prepare(
            "SELECT text FROM bot_shared_channel_messages
             WHERE channel_id=?1 AND kind='reply' ORDER BY id",
        ).unwrap();
        query.query_map([&channel_id], |row| row.get::<_,String>(0))
            .unwrap().collect::<rusqlite::Result<_>>().unwrap()
    };
    assert_eq!(replies,vec!["Approved final result"]);
    let pending_view = shared_channels::detail(&store,&channel_id,100).unwrap();
    let reviewing = pending_view.messages.iter()
        .find(|message| message.id == deliveries[0].0).unwrap();
    assert!(reviewing.council.is_some());
    let resumed = crate::architect::load(&store,&deliveries[0].1).unwrap().unwrap();
    assert_eq!(resumed.council_since,1234);
}

#[test]
fn startup_reconciles_active_bot_deliveries_as_failed() {
    let f = fixture();
    let source = bot(&f);
    let target = create(&f.store, &input("Partner"), &f.root).unwrap();
    let (source_session, _) = thread_session(&f.store, &source, DESKTOP, "source", "Scout · Source").unwrap();
    let (target_session, _) = thread_session(&f.store, &target, DESKTOP, "target", "Partner · Target").unwrap();
    f.store.db.execute("INSERT INTO bot_messages(id,source_bot_id,target_bot_id,source_session_id,target_session_id,source_channel_id,source_external_id,text,status,depth,created_at,updated_at) VALUES('delivery',?1,?2,?3,?4,'web','thread','hello','in_progress',1,1,1)", params![source.id,target.id,source_session,target_session]).unwrap();
    let channel = shared_channels::create(&f.store, "launch", std::slice::from_ref(&source.id)).unwrap();
    let message_id = crate::storage::id();
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES(?1,?2,'user','ship it','in_progress',1)", params![message_id,channel.id]).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,created_at,updated_at) VALUES('channel-delivery',?1,?2,?3,?4,'in_progress',1,1)", params![channel.id,message_id,source.id,target_session]).unwrap();
    shared_channels::reconcile_in_progress(&f.store).unwrap();
    let direct: String = f.store.db.query_row("SELECT status FROM bot_messages WHERE id='delivery'", [], |row| row.get(0)).unwrap();
    let assigned: String = f.store.db.query_row("SELECT status FROM bot_shared_channel_deliveries WHERE id='channel-delivery'", [], |row| row.get(0)).unwrap();
    let parent: String = f.store.db.query_row("SELECT status FROM bot_shared_channel_messages WHERE id=?1", [&message_id], |row| row.get(0)).unwrap();
    assert_eq!((direct.as_str(), assigned.as_str(), parent.as_str()), ("failed", "failed", "failed"));
}

#[test]
fn channel_post_status_follows_retries_and_bot_resets() {
    let f = fixture();
    let scout = bot(&f);
    let writer = create(&f.store, &input("Writer"), &f.root).unwrap();
    let (scout_session, _) = thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
    let (writer_session, _) = thread_session(&f.store, &writer, DESKTOP, DESKTOP, "Writer · You").unwrap();
    let channel = shared_channels::create(&f.store, "launch", &[scout.id.clone(), writer.id.clone()]).unwrap();
    let parent = |id: &str| f.store.db.query_row("SELECT status FROM bot_shared_channel_messages WHERE id=?1", [id], |row| row.get::<_, String>(0)).unwrap();
    let delivery = |id: &str, message: &str, bot: &str, session: &str, status: &str, created: i64, updated: i64| {
        f.store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![id,channel.id,message,bot,session,status,created,updated]).unwrap();
    };
    // Stopped at 5, then continued on Writer at 6, which completed: the retry wins.
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('retried',?1,'user','ship it','stopped',1)", [&channel.id]).unwrap();
    delivery("first", "retried", &scout.id, &scout_session, "stopped", 1, 5);
    delivery("second", "retried", &writer.id, &writer_session, "completed", 6, 8);
    shared_channels::settle_posts(&f.store, &["retried".into()]).unwrap();
    assert_eq!(parent("retried"), "completed");
    // Resetting the bot doing the work leaves the post stopped, so it can be continued.
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('open',?1,'user','draft it','in_progress',9)", [&channel.id]).unwrap();
    delivery("working", "open", &scout.id, &scout_session, "in_progress", 9, 9);
    reset(&f.store, &scout.id).unwrap();
    assert_eq!(parent("open"), "stopped");
}

#[test]
fn failed_channel_posts_stay_failed_until_a_retry_and_queue_takes_promote_their_delivery() {
    let f = fixture();
    let scout = bot(&f);
    let writer = create(&f.store, &input("Writer"), &f.root).unwrap();
    let (scout_session, _) = thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
    let (writer_session, _) = thread_session(&f.store, &writer, DESKTOP, DESKTOP, "Writer · You").unwrap();
    let channel = shared_channels::create(&f.store, "launch", &[scout.id.clone(), writer.id.clone()]).unwrap();
    let parent = |id: &str| f.store.db.query_row("SELECT status FROM bot_shared_channel_messages WHERE id=?1", [id], |row| row.get::<_, String>(0)).unwrap();
    let delivery = |id: &str, bot: &str, session: &str, status: &str, created: i64, updated: i64| {
        f.store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,created_at,updated_at) VALUES(?1,?2,'post',?3,?4,?5,?6,?7)", params![id,channel.id,bot,session,status,created,updated]).unwrap();
    };
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('post',?1,'user','ship it','in_progress',10)", [&channel.id]).unwrap();
    // One post to two bots shares its fan-out time: Scout's early failure is not hidden by Writer.
    delivery("scout", &scout.id, &scout_session, "failed", 10, 11);
    delivery("writer", &writer.id, &writer_session, "completed", 10, 20);
    shared_channels::settle_posts(&f.store, &["post".into()]).unwrap();
    assert_eq!(parent("post"), "failed");
    // A retry made after the failure wins once it completes.
    delivery("retry", &scout.id, &scout_session, "completed", 21, 30);
    shared_channels::settle_posts(&f.store, &["post".into()]).unwrap();
    assert_eq!(parent("post"), "completed");

    // Taking a queued assignment from the queue marks it in progress at once, so Stop sees it.
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('queued',?1,'user','draft it','queued',40)", [&channel.id]).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,created_at,updated_at) VALUES('waiting',?1,'queued',?2,?3,'queued',40,40)", params![channel.id,writer.id,writer_session]).unwrap();
    assert_eq!(shared_channels::promote_taken(&f.store, &writer_session, "Work requested in #other:\n\ndraft it"), None);
    assert_eq!(shared_channels::promote_taken(&f.store, &writer_session, "Work requested in #launch:\n\ndraft it\n\nBrief"), Some(Running::Delivery("waiting".into())));
    let waiting = Running::Delivery("waiting".into());
    assert!(!shared_channels::delivery_stopped(&f.store, &waiting));
    f.store.db.execute("UPDATE bot_shared_channel_deliveries SET status='stopped' WHERE id='waiting'", []).unwrap();
    assert!(shared_channels::delivery_stopped(&f.store, &waiting));
}

#[test]
fn promoted_posts_recover_after_a_restart_and_launches_credit_their_own_request() {
    let f = fixture();
    let scout = bot(&f);
    let writer = create(&f.store, &input("Writer"), &f.root).unwrap();
    let (writer_session, _) = thread_session(&f.store, &writer, DESKTOP, DESKTOP, "Writer · You").unwrap();
    let channel = shared_channels::create(&f.store, "launch", std::slice::from_ref(&writer.id)).unwrap();
    let status = |table: &str, id: &str| f.store.db.query_row(&format!("SELECT status FROM {table} WHERE id=?1"), [id], |row| row.get::<_, String>(0)).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('post',?1,'user','draft it','queued',1)", [&channel.id]).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,created_at,updated_at) VALUES('d',?1,'post',?2,?3,'queued',1,1)", params![channel.id,writer.id,writer_session]).unwrap();
    // Promotion moves the post with its delivery, so a restart can settle it.
    assert_eq!(shared_channels::promote_taken(&f.store, &writer_session, "Work requested in #launch:\n\ndraft it"), Some(Running::Delivery("d".into())));
    assert_eq!(status("bot_shared_channel_messages", "post"), "in_progress");
    shared_channels::reconcile_in_progress(&f.store).unwrap();
    assert_eq!((status("bot_shared_channel_deliveries", "d"), status("bot_shared_channel_messages", "post")), ("failed".into(), "failed".into()));
    // A queued delivery whose queue entry was taken before the restart is settled too.
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('lost',?1,'user','test it','queued',2)", [&channel.id]).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,created_at,updated_at) VALUES('l',?1,'lost',?2,?3,'queued',2,2)", params![channel.id,writer.id,writer_session]).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('kept',?1,'user','ship it','queued',3)", [&channel.id]).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,created_at,updated_at) VALUES('k',?1,'kept',?2,?3,'queued',3,3)", params![channel.id,writer.id,writer_session]).unwrap();
    crate::message_queue::add(&f.store, &writer_session, "Work requested in #launch:\n\nship it", &json!([])).unwrap();
    shared_channels::reconcile_in_progress(&f.store).unwrap();
    assert_eq!((status("bot_shared_channel_messages", "lost"), status("bot_shared_channel_messages", "kept")), ("failed".into(), "queued".into()));
    // Two requests wait for Writer; the turn that launches marks only its own request running.
    let (scout_session, _) = thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
    for (id, text) in [("first", "one"), ("second", "two")] {
        f.store.db.execute("INSERT INTO bot_messages(id,source_bot_id,target_bot_id,source_session_id,target_session_id,source_channel_id,source_external_id,text,status,depth,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'desktop','desktop',?6,'queued',1,1,1)", params![id,scout.id,writer.id,scout_session,writer_session,text]).unwrap();
    }
    assert_eq!(shared_channels::promote_taken(&f.store, &writer_session, "Message from Scout:\n\ntwo"), Some(Running::Message("second".into())));
    assert_eq!((status("bot_messages", "first"), status("bot_messages", "second")), ("queued".into(), "in_progress".into()));
    // A turn that fails to launch puts its request back to queued, with its post.
    shared_channels::launch_failed(&f.store, &writer_session, &Running::Message("second".into()));
    assert_eq!(status("bot_messages", "second"), "queued");
    let promoted = shared_channels::promote_taken(&f.store, &writer_session, "Work requested in #launch:\n\nship it").unwrap();
    assert_eq!((status("bot_shared_channel_deliveries", "k"), status("bot_shared_channel_messages", "kept")), ("in_progress".into(), "in_progress".into()));
    shared_channels::launch_failed(&f.store, &writer_session, &promoted);
    assert_eq!((status("bot_shared_channel_deliveries", "k"), status("bot_shared_channel_messages", "kept")), ("queued".into(), "queued".into()));
    // A deleted channel is preview only until it is restored.
    shared_channels::set_archived(&f.store, &channel.id, true).unwrap();
    assert!(shared_channels::update(&f.store, &channel.id, "renamed", std::slice::from_ref(&writer.id), None).is_err());
    shared_channels::set_archived(&f.store, &channel.id, false).unwrap();
    assert!(shared_channels::update(&f.store, &channel.id, "renamed", std::slice::from_ref(&writer.id), None).is_ok());
}

#[test]
fn assignments_carry_recent_channel_history() {
    let f = fixture();
    let scout = bot(&f);
    let channel = shared_channels::create(&f.store, "launch", std::slice::from_ref(&scout.id)).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('ask',?1,'user','find the bug','completed',1)", [&channel.id]).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,author_bot_id,text,created_at) VALUES('answer',?1,'reply',?2,'It is in\n parse()',2)", params![channel.id,scout.id]).unwrap();
    f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES('now',?1,'user','fix it','in_progress',3)", [&channel.id]).unwrap();
    assert_eq!(shared_channels::recent_history(&f.store, &channel.id, "now").unwrap(), "- You: find the bug\n- Scout: It is in parse()");
    assert_eq!(shared_channels::recent_history(&f.store, &channel.id, "ask").unwrap().lines().count(), 2);
}

#[test]
fn channel_history_returns_the_latest_page() {
    let f = fixture();
    let scout = bot(&f);
    let channel = shared_channels::create(&f.store, "launch", std::slice::from_ref(&scout.id)).unwrap();
    for at in 1..=5 {
        f.store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,created_at) VALUES(?1,?2,'reply',?3,?4)", params![format!("m{at}"),channel.id,format!("reply {at}"),at]).unwrap();
    }
    let page = shared_channels::detail(&f.store, &channel.id, 3).unwrap();
    assert!(page.has_more);
    assert_eq!(page.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m3", "m4", "m5"]);
    let all = shared_channels::detail(&f.store, &channel.id, 5).unwrap();
    assert!(!all.has_more && all.messages.len() == 5);
}

#[test]
fn reset_wipes_conversations_memories_schedules_and_activity() {
    let f = fixture();
    let bot = bot(&f);
    let (session, _) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "t").unwrap();
    remember(&f.store, &bot.id, "Lives in Lisbon", "user").unwrap();
    let schedule_input = schedule::ScheduleInput { prompt: "Check the inbox".into(), kind: "interval".into(), interval_minutes: Some(60), ..Default::default() };
    schedule::create(&f.store, &bot.id, &schedule_input, "user", 0).unwrap();
    reset(&f.store, &bot.id).unwrap();
    assert!(memories(&f.store, &bot.id).unwrap().is_empty());
    assert!(schedule::list(&f.store, &bot.id).unwrap().is_empty());
    assert!(threads(&f.store, &bot.id).unwrap().is_empty());
    assert!(f.store.session(&session).is_err());
    assert!(activity(&f.store, &bot.id, None, 10).unwrap().iter().all(|a| a.kind == "reset"));
}

#[test]
fn delete_removes_the_bot_and_its_channels() {
    let f = fixture();
    let bot = bot(&f);
    let channel = channels::add(&f.store, &bot.id, &channels::ChannelInput { kind: "web".into(), ..Default::default() }).unwrap();
    assert_eq!(channels::web_token(&f.store, &channel.id).map(|token| token.len()), Some(64));
    assert!(channel.secrets.is_empty(), "the web link is not a keychain secret");
    assert!(!serde_json::to_string(&channel).unwrap().contains(&channels::web_token(&f.store, &channel.id).unwrap()), "the link token never reaches the webview through channel data");
    delete(&f.store, &bot.id).unwrap();
    assert!(get(&f.store, &bot.id).is_err());
    assert!(channels::get(&f.store, &channel.id).is_err());
    assert!(channels::web_token(&f.store, &channel.id).is_none());
}

#[test]
fn glob_rules_match_whole_text_and_substrings() {
    assert!(guard::matches("npm test*", "npm test -- --watch"));
    assert!(!guard::matches("npm test*", "run npm test"));
    assert!(guard::matches("*deploy*", "Run the deploy script"));
    assert!(guard::matches("a*b*c", "aXXbYYc"));
    assert!(!guard::matches("a*b*c", "aXXbYY"));
    assert!(guard::matches("git status", "Run: git status --short"));
    assert!(!guard::matches("", "anything"));
}

#[test]
fn deny_beats_allow_and_dangerous_actions_always_ask() {
    let f = fixture();
    let bot = bot(&f);
    assert_eq!(guard::decide(&f.store, &bot.id, "ls -la").unwrap(), guard::Decision::Ask);
    guard::add_rule(&f.store, &bot.id, "*", "allow").unwrap_err();
    guard::add_rule(&f.store, &bot.id, "ls*", "allow").unwrap();
    guard::add_rule(&f.store, &bot.id, "git*", "allow").unwrap();
    assert_eq!(guard::decide(&f.store, &bot.id, "ls -la").unwrap(), guard::Decision::Allow);
    assert_eq!(guard::decide(&f.store, &bot.id, "git push --force origin main").unwrap(), guard::Decision::Ask);
    guard::add_rule(&f.store, &bot.id, "ls -la", "deny").unwrap();
    assert_eq!(guard::decide(&f.store, &bot.id, "ls -la").unwrap(), guard::Decision::Deny);
    assert_eq!(guard::pattern_for("npm run build --prod\nmore"), "npm run build*");
    assert_eq!(guard::pattern_for("ls"), "ls");
}

#[test]
fn approval_replies_only_count_while_something_is_waiting() {
    assert_eq!(gateway::parse_command("yes", true), gateway::Command::Approve(true, false));
    assert_eq!(gateway::parse_command("Always.", true), gateway::Command::Approve(true, true));
    assert_eq!(gateway::parse_command("no", true), gateway::Command::Approve(false, false));
    assert_eq!(gateway::parse_command("yes", false), gateway::Command::Message);
    assert_eq!(gateway::parse_command("/STOP", false), gateway::Command::Stop);
    assert_eq!(gateway::parse_command("/new", false), gateway::Command::NewThread);
    assert_eq!(gateway::parse_command("/help", false), gateway::Command::Help);
}

#[test]
fn schedules_compute_their_next_local_run() {
    // Thursday 1970-01-01 10:00 UTC, no offset.
    let thursday = 10 * 60 * MINUTE;
    assert_eq!(schedule::next_run("interval", Some(30), None, thursday, 0), Some(thursday + 30 * MINUTE));
    assert_eq!(schedule::next_run("interval", Some(1), None, thursday, 0), None);
    assert_eq!(schedule::next_run("daily", None, Some("09:00"), thursday, 0), Some(DAY + 9 * 60 * MINUTE));
    assert_eq!(schedule::next_run("daily", None, Some("11:30"), thursday, 0), Some(11 * 60 * MINUTE + 30 * MINUTE));
    // Friday 10:00 -> weekdays at 09:00 skips the weekend to Monday.
    let friday = DAY + thursday;
    assert_eq!(schedule::next_run("weekdays", None, Some("09:00"), friday, 0), Some(4 * DAY + 9 * 60 * MINUTE));
    // A +2h zone: 09:00 local is 07:00 UTC.
    assert_eq!(schedule::next_run("daily", None, Some("09:00"), thursday, 2 * 60 * MINUTE), Some(DAY + 7 * 60 * MINUTE));
    assert_eq!(schedule::next_run("daily", None, Some("25:00"), thursday, 0), None);
}

#[test]
fn schedules_validate_and_advance() {
    let f = fixture();
    let bot = bot(&f);
    let once = schedule::ScheduleInput { title: "Reminder".into(), prompt: "Remind me to stretch".into(), kind: "once".into(), run_at: Some(10 * MINUTE), ..Default::default() };
    let created = schedule::create(&f.store, &bot.id, &once, "bot", 0).unwrap();
    assert_eq!(created.created_by, "bot");
    let empty = schedule::ScheduleInput { kind: "daily".into(), time_of_day: Some("09:00".into()), ..Default::default() };
    assert!(schedule::create(&f.store, &bot.id, &empty, "user", 0).is_err());
    let (due, _) = schedule::due(&f.store, 11 * MINUTE).unwrap();
    assert_eq!(due.len(), 1);
    schedule::advance(&f.store, &due[0], 11 * MINUTE).unwrap();
    let after = &schedule::list(&f.store, &bot.id).unwrap()[0];
    assert!(!after.enabled, "one-off schedules turn off after running");
    assert!(schedule::due(&f.store, 20 * MINUTE).unwrap().0.is_empty());
}

#[test]
fn paused_bots_skip_heartbeats() {
    let f = fixture();
    let mut with_heartbeat = input("Watcher");
    with_heartbeat.heartbeat_minutes = 60;
    let bot = create(&f.store, &with_heartbeat, &f.root).unwrap();
    let due_at = bot.next_heartbeat_at.unwrap();
    assert_eq!(schedule::due(&f.store, due_at + 1).unwrap().1.len(), 1);
    set_paused(&f.store, &bot.id, true).unwrap();
    assert!(schedule::due(&f.store, due_at + 1).unwrap().1.is_empty());
}

#[test]
fn brief_carries_identity_memories_and_channel_etiquette() {
    let f = fixture();
    let bot = bot(&f);
    remember(&f.store, &bot.id, "Works at Acme\nin sales", "user").unwrap();
    let text = brief::compose(&get(&f.store, &bot.id).unwrap(), &memories(&f.store, &bot.id).unwrap(), "telegram", "Ana");
    assert!(text.starts_with("\n\nBot brief:\nYou are Scout, Research assistant"));
    assert!(text.contains("- Works at Acme in sales"));
    assert!(text.contains("phone chat app"));
    assert!(text.contains("from Ana"));
    assert!(brief::compose(&bot, &[], "schedule", "").contains(gateway::NOTHING));
}

#[test]
fn channels_require_their_secrets_and_valid_kinds() {
    let f = fixture();
    let bot = bot(&f);
    let missing = channels::ChannelInput { kind: "telegram".into(), ..Default::default() };
    assert!(channels::add(&f.store, &bot.id, &missing).unwrap_err().contains("token"));
    let unknown = channels::ChannelInput { kind: "fax".into(), ..Default::default() };
    assert!(channels::add(&f.store, &bot.id, &unknown).is_err());
    let whatsapp_bad = channels::ChannelInput { kind: "whatsapp".into(), config: json!({"phoneNumberId": "abc"}), ..Default::default() };
    assert!(channels::add(&f.store, &bot.id, &whatsapp_bad).is_err());
    let telegram = channels::ChannelInput { kind: "telegram".into(), label: "Phone".into(), secrets: [("token".to_owned(), "123:abc".to_owned())].into(), ..Default::default() };
    let channel = channels::add(&f.store, &bot.id, &telegram).unwrap();
    assert_eq!(channel.secrets, vec!["token".to_owned()]);
    assert_eq!(channels::list(&f.store, &bot.id).unwrap().len(), 1);
    let serialized = serde_json::to_string(&channel).unwrap();
    assert!(!serialized.contains("123:abc"), "secrets never leave native code");
}

#[test]
fn pairing_codes_are_stable_until_decided() {
    let f = fixture();
    let bot = bot(&f);
    let telegram = channels::ChannelInput { kind: "telegram".into(), secrets: [("token".to_owned(), "1:x".to_owned())].into(), ..Default::default() };
    let channel = channels::add(&f.store, &bot.id, &telegram).unwrap();
    let code = channels::request_pairing(&f.store, &channel.id, "42", "Ana").unwrap().unwrap();
    assert_eq!(code.len(), 4);
    assert_eq!(channels::request_pairing(&f.store, &channel.id, "42", "Ana").unwrap().unwrap(), code);
    assert!(!channels::is_approved(&f.store, &channel.id, "42").unwrap());
    channels::decide_pairing(&f.store, &channel.id, "42", "approved").unwrap();
    assert!(channels::is_approved(&f.store, &channel.id, "42").unwrap());
    channels::decide_pairing(&f.store, &channel.id, "42", "blocked").unwrap();
    assert!(channels::request_pairing(&f.store, &channel.id, "42", "Ana").unwrap().is_none());
    assert!(channels::decide_pairing(&f.store, &channel.id, "42", "maybe").is_err());
    assert_eq!(channels::pairings(&f.store, &bot.id).unwrap()[0].status, "blocked");
    assert_eq!(channels::direct_thread("discord", "42"), "dm:42");
}

#[test]
fn long_replies_split_on_natural_boundaries() {
    let text = format!("{}\n\n{}", "a".repeat(30), "b".repeat(30));
    assert_eq!(channels::chunks(&text, 40), vec!["a".repeat(30), "b".repeat(30)]);
    let words = "word ".repeat(30);
    let parts = channels::chunks(&words, 32);
    assert!(parts.iter().all(|part| part.chars().count() <= 32 && !part.ends_with(' ')));
    assert_eq!(parts.join(" ").split_whitespace().count(), 30);
    assert_eq!(channels::chunks("ünïcödé".repeat(10).as_str(), 7).len(), 10);
    assert!(channels::chunks("   ", 10).is_empty());
}

#[test]
fn telegram_answers_private_chats_mentions_and_replies() {
    let private = json!({"text": "hello", "chat": {"type": "private"}, "from": {"id": 1}});
    assert_eq!(telegram::addressed_text(&private, "scout_bot", 99).as_deref(), Some("hello"));
    let group = json!({"text": "hello all", "chat": {"type": "group"}});
    assert_eq!(telegram::addressed_text(&group, "scout_bot", 99), None);
    let mention = json!({"text": "@Scout_Bot what's up", "chat": {"type": "group"}});
    assert_eq!(telegram::addressed_text(&mention, "scout_bot", 99).as_deref(), Some("what's up"));
    let reply = json!({"text": "and then?", "chat": {"type": "supergroup"}, "reply_to_message": {"from": {"id": 99}}});
    assert_eq!(telegram::addressed_text(&reply, "scout_bot", 99).as_deref(), Some("and then?"));
    let command = json!({"text": "/stop@scout_bot", "chat": {"type": "group"}});
    assert_eq!(telegram::addressed_text(&command, "scout_bot", 99).as_deref(), Some("/stop"));
}

#[test]
fn slack_handles_mentions_in_threads_and_direct_messages() {
    let mention = json!({"type": "app_mention", "user": "U1", "channel": "C1", "ts": "1.2", "text": "<@B1> summarize"});
    assert_eq!(slack::inbound(&mention, "B1"), Some(("U1".into(), "C1|1.2".into(), "summarize".into())));
    let threaded = json!({"type": "app_mention", "user": "U1", "channel": "C1", "ts": "1.3", "thread_ts": "1.0", "text": "<@B1> more"});
    assert_eq!(slack::inbound(&threaded, "B1").unwrap().1, "C1|1.0");
    let direct = json!({"type": "message", "channel_type": "im", "user": "U1", "channel": "D1", "ts": "2", "text": "hi"});
    assert_eq!(slack::inbound(&direct, "B1").unwrap().1, "D1");
    let channel_message = json!({"type": "message", "channel_type": "channel", "user": "U1", "channel": "C1", "ts": "3", "text": "hi"});
    assert_eq!(slack::inbound(&channel_message, "B1"), None);
    let own = json!({"type": "message", "channel_type": "im", "bot_id": "B1", "channel": "D1", "ts": "4", "text": "echo"});
    assert_eq!(slack::inbound(&own, "B1"), None);
}

#[test]
fn discord_answers_dms_and_mentions_but_not_bots() {
    let dm = json!({"author": {"id": "7", "username": "ana"}, "channel_id": "55", "content": "hi"});
    assert_eq!(discord::inbound(&dm, "1"), Some(("7".into(), "ana".into(), "55".into(), "hi".into())));
    let guild = json!({"guild_id": "9", "author": {"id": "7", "username": "ana"}, "channel_id": "55", "content": "hi", "mentions": []});
    assert_eq!(discord::inbound(&guild, "1"), None);
    let mentioned = json!({"guild_id": "9", "author": {"id": "7", "global_name": "Ana"}, "channel_id": "55", "content": "<@1> status", "mentions": [{"id": "1"}]});
    assert_eq!(discord::inbound(&mentioned, "1").unwrap().3, "status");
    let bot = json!({"author": {"id": "8", "bot": true}, "channel_id": "55", "content": "hi"});
    assert_eq!(discord::inbound(&bot, "1"), None);
}

#[test]
fn whatsapp_checks_signatures_and_reads_text_messages() {
    use hmac::{Hmac, Mac};
    let body = br#"{"entry":[{"changes":[{"value":{"contacts":[{"wa_id":"351900","profile":{"name":"Ana"}}],"messages":[{"from":"351900","type":"text","text":{"body":"hello"}},{"from":"351900","type":"image"}]}}]}]}"#;
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(b"app-secret").unwrap();
    mac.update(body);
    let signature: String = mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    assert!(whatsapp::signature_valid("app-secret", body, &format!("sha256={signature}")));
    assert!(!whatsapp::signature_valid("other", body, &format!("sha256={signature}")));
    assert!(!whatsapp::signature_valid("app-secret", body, &signature));
    let payload: serde_json::Value = serde_json::from_slice(body).unwrap();
    assert_eq!(whatsapp::messages(&payload), vec![("351900".into(), "Ana".into(), "hello".into())]);
}

#[test]
fn local_server_parses_requests_and_guards_hosts() {
    let request = http::parse_head("GET /api/chat/abc/events?after=12&x=a%20b+c HTTP/1.1\r\nHost: 127.0.0.1:47321\r\nAuthorization: Bearer t0k").unwrap();
    assert_eq!(request.path, "/api/chat/abc/events");
    assert_eq!(request.query["after"], "12");
    assert_eq!(request.query["x"], "a b c");
    assert_eq!(request.bearer(), Some("t0k"));
    assert!(request.loopback_host());
    let rebound = http::parse_head("GET / HTTP/1.1\r\nHost: evil.example:47321").unwrap();
    assert!(!rebound.loopback_host());
    assert!(http::parse_head("garbage").is_none());
    assert!(http::same("abc", "abc") && !http::same("abc", "abd") && !http::same("abc", "ab"));
    assert_eq!(web::visible("hi\n\nBot brief:\nYou are Scout"), "hi");
    assert_eq!(preview("**hi**\n\nBot brief:\nYou are Scout"), "hi");
    assert_eq!(web::new_token().len(), 64);
    assert_eq!(web::page("not a channel!").status, 404);
    let engine = http::Response::markdown_engine();
    assert_eq!((engine.status, engine.content_type), (200, "text/javascript; charset=utf-8"));
    assert!(std::str::from_utf8(&engine.body).unwrap().trim_end().ends_with("export default createMd4cModule;"));
}

#[test]
fn only_registered_bot_tools_skip_approval() {
    assert!(super::tools::self_scoped("pandacode_bot_remember"));
    assert!(super::tools::self_scoped("pandacode_bot_notify"));
    assert!(!super::tools::self_scoped("pandacode_bot_anything_else"));
    assert!(!super::tools::self_scoped("pandacode_open_browser"));
    assert!(super::tools::catalog().iter().all(|tool| tool["name"].as_str().is_some_and(super::tools::self_scoped)));
}

#[test]
fn setup_catches_misplaced_and_truncated_tokens_before_any_request() {
    let input = |kind: &str, secrets: &[(&str, &str)], config: serde_json::Value| channels::ChannelInput { kind: kind.into(), config, secrets: secrets.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(), ..Default::default() };
    assert!(channels::check_format(&input("telegram", &[("token", "123456789:AAE1234567890abcdefghijklmnopqrstu")], json!({}))).is_ok());
    assert!(channels::check_format(&input("telegram", &[("token", "123456789")], json!({}))).unwrap_err().contains("BotFather"));
    assert!(channels::check_format(&input("slack", &[("token", "xapp-1"), ("app_token", "xoxb-1")], json!({}))).unwrap_err().contains("swapped"));
    assert!(channels::check_format(&input("slack", &[("token", "xoxp-1"), ("app_token", "xapp-1")], json!({}))).unwrap_err().contains("xoxb-"));
    assert!(channels::check_format(&input("slack", &[("token", "xoxb-1"), ("app_token", "xapp-1")], json!({}))).is_ok());
    assert!(channels::check_format(&input("discord", &[("token", "client-secret")], json!({}))).unwrap_err().contains("Reset Token"));
    assert!(channels::check_format(&input("discord", &[("token", &format!("{}.{}.{}", "a".repeat(24), "b".repeat(6), "c".repeat(38)))], json!({}))).is_ok());
    assert!(channels::check_format(&input("whatsapp", &[], json!({"phoneNumberId": "+351 912"}))).unwrap_err().contains("not the phone number"));
    assert!(discord::invite_url("123").starts_with("https://discord.com/oauth2/authorize?client_id=123&scope=bot&permissions="));
}

#[test]
fn whatsapp_gets_a_shown_verify_token_and_settings_are_validated() {
    let f = fixture();
    let bot = bot(&f);
    let whatsapp = channels::ChannelInput { kind: "whatsapp".into(), config: json!({"phoneNumberId": "123456"}), secrets: [("access_token".to_owned(), "EAAG".to_owned()), ("signing_secret".to_owned(), "s3cret".to_owned())].into(), ..Default::default() };
    let channel = channels::add(&f.store, &bot.id, &whatsapp).unwrap();
    assert!(channel.config["verifyToken"].as_str().unwrap().starts_with("pandacode-"));
    assert_eq!(channel.secrets, vec!["access_token".to_owned(), "signing_secret".to_owned()]);
    let configured = channels::configure(&f.store, &channel.id, &json!({"publicUrl": "https://quiet-river.trycloudflare.com/"})).unwrap();
    assert_eq!(configured.config["publicUrl"], "https://quiet-river.trycloudflare.com");
    assert!(channels::configure(&f.store, &channel.id, &json!({"publicUrl": "http://insecure.example"})).is_err());
    assert!(channels::configure(&f.store, &channel.id, &json!({"publicUrl": "https://user:pw@example.com"})).is_err());
    assert!(channels::configure(&f.store, &channel.id, &json!({"remote": true})).is_err(), "remote access is a web chat setting");
    let web = channels::add(&f.store, &bot.id, &channels::ChannelInput { kind: "web".into(), ..Default::default() }).unwrap();
    assert_eq!(channels::configure(&f.store, &web.id, &json!({"remote": true})).unwrap().config["remote"], true);
    assert!(channels::configure(&f.store, &web.id, &json!({"secret_names": "[]"})).is_err());
}

#[test]
fn summaries_name_the_desktop_conversation_once_it_exists() {
    let f = fixture();
    let bot = bot(&f);
    assert_eq!(summaries(&f.store).unwrap()[0].desktop_session, None);
    let (session, _) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout · You").unwrap();
    thread_session(&f.store, &bot, "channel", "chat", "Scout · Ana").unwrap();
    let json = serde_json::to_value(&summaries(&f.store).unwrap()[0]).unwrap();
    assert_eq!(json["desktopSession"], session.as_str());
    assert_eq!(json["name"], "Scout", "bot fields stay at the top level");
}

#[test]
fn summaries_carry_a_one_line_preview_of_the_latest_message() {
    let f = fixture();
    let bot = bot(&f);
    assert!(summaries(&f.store).unwrap()[0].last_message.is_none());
    let (session, _) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout · You").unwrap();
    let insert = |role: &str, content: &str, at: i64| {
        f.store.db.execute("INSERT INTO messages(session_id,role,content,created_at) VALUES(?1,?2,?3,?4)", params![session, role, content, at]).unwrap();
    };
    insert("user", "What changed today?", 10);
    insert("assistant", "## Done\n\nInbox at **zero**.   Two `replies` ready.", 20);
    insert("system", "Run finished", 30);
    let json = serde_json::to_value(&summaries(&f.store).unwrap()[0]).unwrap();
    assert_eq!(json["lastMessage"]["role"], "assistant");
    assert_eq!(json["lastMessage"]["text"], "Done Inbox at zero. Two replies ready.");
    assert_eq!(json["lastMessage"]["at"], 20);
    insert("user", &"x".repeat(400), 40);
    let long = summaries(&f.store).unwrap()[0].last_message.clone().unwrap();
    assert_eq!(long.role, "user");
    assert!(long.text.chars().count() <= 161, "previews stay short");
}

#[test]
fn schema_version_includes_bots() {
    let f = fixture();
    let version: i64 = f.store.db.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(version, crate::storage::SCHEMA_VERSION);
}

#[test]
fn profile_settings_default_save_and_validate() {
    let f = fixture();
    let created = bot(&f);
    assert_eq!((created.color.as_str(), created.fallback_mode.as_str(), created.while_working.as_str()), ("", "ask", "queue"));
    assert!(created.notify && !created.show_plan && created.backup_models.is_empty());
    let mut next = input("Scout");
    next.color = "#3B82F6".into();
    next.reasoning_effort = "high".into();
    next.backup_models = vec![BackupModel { provider: "codex".into(), model: "gpt-5".into() }];
    next.fallback_mode = "auto".into();
    next.notify = Some(false);
    next.while_working = "steer".into();
    next.show_plan = true;
    let saved = update(&f.store, &created.id, &next).unwrap();
    assert_eq!(saved.color, "#3b82f6");
    assert_eq!(saved.backup_models, next.backup_models);
    assert_eq!((saved.fallback_mode.as_str(), saved.while_working.as_str(), saved.reasoning_effort.as_str()), ("auto", "steer", "high"));
    assert!(!saved.notify && saved.show_plan);
    assert_eq!(saved.memory_version, created.memory_version + 1, "the plan rule is part of the brief");
    assert!(brief::compose(&saved, &[], "desktop", "").contains("short plan"));
    let (session, _) = thread_session(&f.store, &saved, DESKTOP, DESKTOP, "Scout · You").unwrap();
    assert_eq!(f.store.session(&session).unwrap().reasoning_effort.as_deref(), Some("high"));
    for bad in [
        BotInput { color: "red".into(), ..input("Scout") },
        BotInput { fallback_mode: "never".into(), ..input("Scout") },
        BotInput { while_working: "drop".into(), ..input("Scout") },
        BotInput { reasoning_effort: "max; drop".into(), ..input("Scout") },
        BotInput { backup_models: vec![BackupModel { provider: "nope".into(), model: "x".into() }], ..input("Scout") },
        BotInput { backup_models: (0..5).map(|i| BackupModel { provider: "codex".into(), model: format!("m{i}") }).collect(), ..input("Scout") },
    ] {
        assert!(update(&f.store, &created.id, &bad).is_err());
    }
}

#[test]
fn upgrade_adds_profile_columns_to_older_bot_tables() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE bots(id TEXT PRIMARY KEY, name TEXT NOT NULL); INSERT INTO bots VALUES('b','Old');").unwrap();
    upgrade(&db).unwrap();
    upgrade(&db).unwrap();
    let (notify, plan, backups, working): (bool, bool, String, String) =
        db.query_row("SELECT notify,show_plan,backup_models,while_working FROM bots", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    assert!(notify && !plan);
    assert_eq!((backups.as_str(), working.as_str()), ("[]", "queue"));
    for table in ["bot_shared_channels", "bot_shared_channel_members", "bot_shared_channel_messages", "bot_shared_channel_deliveries", "bot_messages"] {
        let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)", [table], |r| r.get(0)).unwrap();
        assert!(exists, "upgrade creates {table}");
    }
}

#[test]
fn shared_channel_routing_uses_lead_and_limits_mentions_to_two_members() {
    let members = vec!["scout".to_owned(), "writer".to_owned(), "ledger".to_owned()];
    assert_eq!(shared_channels::route_recipients(&members, Some("scout"), &[]).unwrap(), ["scout"]);
    assert_eq!(shared_channels::route_recipients(&members, Some("scout"), &["writer".into(), "ledger".into()]).unwrap(), ["writer", "ledger"]);
    assert!(shared_channels::route_recipients(&members, Some("scout"), &["scout".into(), "writer".into(), "ledger".into()]).is_err());
    assert!(shared_channels::route_recipients(&members, Some("missing"), &[]).is_err());
    assert!(shared_channels::route_recipients(&members, Some("scout"), &["outsider".into()]).is_err());
}

#[test]
fn shared_channel_persists_members_and_archive_state() {
    let f = fixture();
    let scout = bot(&f);
    let mut writer_input = input("Writer"); writer_input.name = "Writer".into();
    let writer = create(&f.store, &writer_input, &f.root).unwrap();
    let channel = shared_channels::create(&f.store, "launch", &[scout.id.clone(), writer.id.clone()]).unwrap();
    assert_eq!(channel.lead_bot_id.as_deref(), Some(scout.id.as_str()));
    let updated = shared_channels::update(&f.store, &channel.id, "launch", std::slice::from_ref(&writer.id), Some(&writer.id)).unwrap();
    assert_eq!(updated.lead_bot_id.as_deref(), Some(writer.id.as_str()));
    shared_channels::set_archived(&f.store, &channel.id, true).unwrap();
    let detail = shared_channels::detail(&f.store, &channel.id, shared_channels::HISTORY_PAGE).unwrap();
    assert!(detail.channel.archived);
    assert_eq!(detail.channel.member_ids, [writer.id]);
}

#[test]
fn a_bot_update_carries_its_hop_depth_into_the_next_turn_only() {
    let f = fixture();
    let scout = bot(&f);
    let writer = create(&f.store, &input("Writer"), &f.root).unwrap();
    let (source_session, _) = thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout · You").unwrap();
    let (target_session, _) = thread_session(&f.store, &writer, DESKTOP, DESKTOP, "Writer · You").unwrap();
    f.store.db.execute(
        "INSERT INTO bot_messages(id,source_bot_id,target_bot_id,source_session_id,target_session_id,source_channel_id,source_external_id,text,status,result,depth,created_at,updated_at) VALUES('collab',?1,?2,?3,?4,'desktop','desktop','Review this','completed','Looks good',2,10,20)",
        rusqlite::params![scout.id,writer.id,source_session,target_session],
    ).unwrap();
    f.store.message(&source_session, "user", "Update from Writer:\n\nLooks good").unwrap();
    assert_eq!(shared_channels::current_hop_depth(&f.store, &source_session).unwrap(), 2);
    f.store.message(&source_session, "user", "A new request from me").unwrap();
    assert_eq!(shared_channels::current_hop_depth(&f.store, &source_session).unwrap(), 0);
}

/// Talks to the real services with made-up credentials: proves the requests are well-formed
/// and that each rejection becomes a message a person can act on. No account needed.
/// cargo test --lib bots::tests::live_services_reject_fake_credentials_clearly -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn live_services_reject_fake_credentials_clearly() {
    let input = |kind: &str, secrets: &[(&str, &str)], config: serde_json::Value| channels::ChannelInput { kind: kind.into(), config, secrets: secrets.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(), ..Default::default() };
    let telegram = channels::verify(&input("telegram", &[("token", "123456789:AAE_fake_token_for_pandacode_tests_00")], json!({}))).await.unwrap_err();
    println!("telegram: {telegram}");
    assert!(telegram.contains("Telegram rejected the bot token"), "{telegram}");
    let slack_bot_token = ["xoxb", "1234567890123", "1234567890123", "abcdefghijklmnopqrstuvwx"].join("-");
    let slack_app_token = ["xapp", "1", "A0000000000", "1234567890123", "abcdef"].join("-");
    let slack = channels::verify(&input("slack", &[("token", slack_bot_token.as_str()), ("app_token", slack_app_token.as_str())], json!({}))).await.unwrap_err();
    println!("slack: {slack}");
    assert!(slack.contains("Slack didn't accept the bot token"), "{slack}");
    let discord = channels::verify(&input("discord", &[("token", &format!("{}.{}.{}", "MTA".repeat(8), "Gabcde", "x".repeat(38)))], json!({}))).await.unwrap_err();
    println!("discord: {discord}");
    assert!(discord.contains("Discord rejected the bot token"), "{discord}");
    let whatsapp = channels::verify(&input("whatsapp", &[("access_token", "EAAG_fake_token")], json!({"phoneNumberId": "123456789012345"}))).await.unwrap_err();
    println!("whatsapp: {whatsapp}");
    assert!(whatsapp.contains("Meta rejected the access token") || whatsapp.starts_with("WhatsApp:"), "{whatsapp}");
}

#[test]
fn soul_persists_apart_from_instructions_and_rebriefs_when_changed() {
    let f = fixture();
    let plain = bot(&f);
    assert_eq!(plain.soul, "");
    let brief_text = brief::compose(&plain, &[], DESKTOP, "");
    assert!(!brief_text.contains("Personality"), "a bot without a soul keeps today's brief");

    let mut with_soul = input("Leo");
    with_soul.role = "Chief of staff".into();
    with_soul.goals = "Make sure I don't miss a ship date.".into();
    with_soul.soul = "# Leo\nDry wit, loyal, blunt under pressure.".into();
    let leo = create(&f.store, &with_soul, &f.root).unwrap();
    let saved = get(&f.store, &leo.id).unwrap();
    assert_eq!(saved.soul, "# Leo\nDry wit, loyal, blunt under pressure.");
    assert_eq!(saved.instructions, "Be brief.");
    let text = brief::compose(&saved, &[], DESKTOP, "");
    let (instructions, personality) = (text.find("Instructions from the user:").unwrap(), text.find("Personality (soul, from the user):").unwrap());
    assert!(instructions < personality);
    assert!(text[personality..].contains("Dry wit, loyal"));
    assert!(!text[instructions..personality].contains("Dry wit"));
    assert!(text.contains("never grants tools, permissions, credentials or authority"));

    let (session, _) = thread_session(&f.store, &saved, DESKTOP, DESKTOP, "Leo · You").unwrap();
    mark_briefed(&f.store, &session, saved.memory_version).unwrap();
    let same = update(&f.store, &leo.id, &with_soul).unwrap();
    assert_eq!(same.memory_version, saved.memory_version, "saving unchanged settings keeps the brief");
    with_soul.soul = String::new();
    let removed = update(&f.store, &leo.id, &with_soul).unwrap();
    assert!(removed.memory_version > saved.memory_version);
    assert_eq!(removed.soul, "");
    assert!(!brief::compose(&removed, &[], DESKTOP, "").contains("Personality"));
}

#[test]
fn oversized_soul_is_refused() {
    let f = fixture();
    let mut big = input("Leo");
    big.soul = "a".repeat(MAX_SOUL + 1);
    assert!(create(&f.store, &big, &f.root).unwrap_err().contains("64 KB"));
    big.soul = "a".repeat(MAX_SOUL);
    assert_eq!(create(&f.store, &big, &f.root).unwrap().soul.len(), MAX_SOUL);
}

#[test]
fn soul_migration_keeps_existing_bots() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE bots(id TEXT PRIMARY KEY, name TEXT NOT NULL); INSERT INTO bots VALUES('b','Scout');").unwrap();
    migrate_soul(&db).unwrap();
    migrate_soul(&db).unwrap();
    let (name, soul): (String, String) = db.query_row("SELECT name, soul FROM bots", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((name.as_str(), soul.as_str()), ("Scout", ""));
}

#[test]
fn capabilities_persist_as_references_and_rebrief() {
    let f = fixture();
    let a = bot(&f);
    let b = create(&f.store, &input("Other"), &f.root).unwrap();
    let saved = set_capabilities(&f.store, &a.id, &["review".into(), "review".into(), "deploy".into()], &["github".into()]).unwrap();
    assert_eq!((saved.skills.clone(), saved.mcp.clone()), (vec!["review".to_string(), "deploy".into()], vec!["github".to_string()]));
    assert!(saved.memory_version > a.memory_version);
    assert_eq!(get(&f.store, &a.id).unwrap().skills, saved.skills, "persisted");
    assert!(get(&f.store, &b.id).unwrap().skills.is_empty(), "same harness, separate selections");
    assert!(set_capabilities(&f.store, &a.id, &["../x".into()], &[]).is_err());
    // Editing the profile keeps the references.
    assert_eq!(update(&f.store, &a.id, &input("Scout")).unwrap().skills, saved.skills);
    assert!(brief::compose(&saved, &[], DESKTOP, "").contains("MCP servers chosen for this bot: github"));
}

#[test]
fn capability_migration_keeps_existing_bots() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE bots(id TEXT PRIMARY KEY, name TEXT NOT NULL); INSERT INTO bots VALUES('b','Scout');").unwrap();
    migrate_capabilities(&db).unwrap();
    migrate_capabilities(&db).unwrap();
    let (skills, mcp): (String, String) = db.query_row("SELECT skills, mcp FROM bots", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((skills.as_str(), mcp.as_str()), ("[]", "[]"));
    db.query_row("SELECT COUNT(*) FROM bot_tasks", [], |r| r.get::<_, i64>(0)).unwrap();
}

#[test]
fn project_tasks_get_their_own_bot_brief_and_skill_union() {
    let f = fixture();
    let library_dir = f.root.join("library");
    let library = crate::skills::Library::new(library_dir, f.root.join("home"));
    let source = || crate::skills::Source { kind: "folder".into(), location: "x".into(), git_ref: None, commit: None };
    for name in ["review", "lint"] {
        library.import(crate::skills::Kind::Skill, vec![("SKILL.md".into(), format!("---\nname: {name}\ndescription: {name} things\n---\nDo {name}.\n").into_bytes())], name, source()).unwrap();
    }
    let checkout = f.root.join("project");
    std::fs::create_dir_all(&checkout).unwrap();
    let project = f.store.add_project(&checkout).unwrap();
    library.activate("lint", "kimi", crate::skills::Scope::Project, Some(&f.store.workspace_root(&project.id, None).unwrap()), false).unwrap();
    let reviewer = set_capabilities(&f.store, &bot(&f).id, &["review".into()], &[]).unwrap();
    let mut other_input = input("Writer");
    other_input.instructions = "Write docs.".into();
    let writer = create(&f.store, &other_input, &f.root).unwrap();

    // Two tasks share one checkout with different bots, and a third has none.
    let one = f.store.create_session(&project.id, "kimi", "default", "ask").unwrap();
    let two = f.store.create_session(&project.id, "kimi", "default", "ask").unwrap();
    let plain = f.store.create_session(&project.id, "kimi", "default", "ask").unwrap();
    assign_task(&f.store, &one.id, Some(&reviewer.id)).unwrap();
    assign_task(&f.store, &two.id, Some(&writer.id)).unwrap();
    assert!(assign_task(&f.store, &one.id, Some("missing")).is_err());

    let first = task_brief(&f.store, Some(&library), &one.id).unwrap().unwrap();
    assert!(first.contains("You are Scout") && first.contains("Do review.") && first.contains("Do lint."), "union of bot and project skills");
    assert_eq!(first.matches("### review").count(), 1);
    assert!(!first.contains("pandacode_bot_remember"), "bot tools are not registered in project tasks");
    assert_eq!(task_brief(&f.store, Some(&library), &one.id).unwrap(), None, "the brief goes with the first message only");
    let second = task_brief(&f.store, Some(&library), &two.id).unwrap().unwrap();
    assert!(second.contains("You are Writer") && !second.contains("Do review."), "tasks stay isolated");
    assert_eq!(task_brief(&f.store, Some(&library), &plain.id).unwrap(), None);
    assert!(!checkout.join(".claude").exists(), "nothing is written into the shared checkout");

    let links = task_links(&f.store).unwrap();
    assert_eq!((links[&one.id].as_str(), links[&two.id].as_str(), links.len()), (reviewer.id.as_str(), writer.id.as_str(), 2));
    assign_task(&f.store, &one.id, None).unwrap();
    assert_eq!(task_bot(&f.store, &one.id).unwrap(), None);
    // A bot's own conversation is never relinked to another bot.
    let (thread, _) = thread_session(&f.store, &writer, DESKTOP, DESKTOP, "x").unwrap();
    assert!(assign_task(&f.store, &thread, Some(&reviewer.id)).is_err());
}
