use super::*;
#[path = "council_flow_tests.rs"]
mod flow;
use crate::architect::council::review;
use council::{finish_refusal, parse_score, CouncilConfig, CouncilSeat};

const SINCE: i64 = 1000;

#[test]
fn selected_bot_config_requires_distinct_bounded_identity_roster() {
    let mut c = on(9.0, 10, 0);
    c.reviewer_bot_ids = vec!["bot-1".into()];
    assert!(c.validate().unwrap_err().contains("2 to 8"));
    c.reviewer_bot_ids.push("bot-1".into());
    assert!(c.validate().unwrap_err().contains("unique"));
    c.reviewer_bot_ids[1] = "bot-2".into();
    assert!(c.validate().is_ok());
    let two = team_with(c.clone(), vec![seat("first", 1, Some(9.0), SINCE + 1), seat("second", 1, Some(9.0), SINCE + 2)]);
    assert!(council::complete(&two, &c, 1));
    c.reviewer_bot_ids.push("bot-3".into());
    assert!(!council::complete(&two, &c, 1));
    c.reviewer_bot_ids.extend((3..=9).map(|i| format!("bot-{i}")));
    assert!(c.validate().unwrap_err().contains("2 to 8"));
    c.reviewer_bot_ids.clear();
    assert!(c.validate().is_ok());
    assert!(council::complete(&two, &c, 1));
}

#[test]
fn selected_bot_quorum_is_frozen_when_later_round_chooses_more_bots() {
    let mut original = on(9.0, 10, 0);
    original.reviewer_bot_ids = vec!["bot-1".into(), "bot-2".into()];
    let mut team = team_with(original.clone(), vec![
        seat("worker-one", 1, Some(9.0), SINCE + 1),
        seat("worker-two", 1, Some(9.0), SINCE + 2),
    ]);
    for index in 0..team.workers.len() {
        let worker = &team.workers[index];
        let mut binding = reviewer_security::ReviewerBinding::native(
            &team, &worker.id, worker.created_at, &worker.provider, &worker.model, worker.council.as_ref().unwrap(),
        );
        binding.reviewer_bot_id = Some(original.reviewer_bot_ids[index].clone());
        binding.roster_bot_ids = original.reviewer_bot_ids.clone();
        binding.snapshot.bot_context = Some(reviewer_security::ReviewerBotContext {
            name: format!("Reviewer {}", index + 1), role: "QA".into(),
            instructions: String::new(), goals: String::new(), soul: String::new(),
            reasoning_effort: String::new(), skill_ids: vec![],
        });
        team.workers[index].reviewer_binding = Some(binding);
    }
    assert!(council::complete(&team, &original, 1));
    let mut changed_while_open = original.clone();
    changed_while_open.reviewer_bot_ids.push("bot-3".into());
    // A completed round can be followed by an explicitly selected new roster,
    // but the previous issued quorum remains two.
    assert!(bot_reviewers::allow_roster_change(&team, &Some(changed_while_open.clone())).is_ok());
    let mut expanded = original.clone();
    expanded.reviewer_bot_ids.push("bot-3".into());
    assert!(council::complete(&team, &expanded, 1));
    team.workers[1].council.as_mut().unwrap().score = None;
    team.config.council = Some(expanded);
    assert!(finish_refusal(&team).unwrap().contains("at least 2 needed"), "the refusal must report the issued quorum too");
    assert!(!bot_reviewers::occupied_bot_ids(&team, 2, None).contains("bot-1"));
    team.council_since += 20;
    assert!(bot_reviewers::occupied_bot_ids(&team, 1, None).is_empty());
}

#[test]
fn automatic_quorum_edits_apply_to_issued_and_legacy_s2_seats() {
    let original = on(9.0, 10, 3);
    let mut team = team_with(original.clone(), vec![
        seat("a", 1, Some(9.0), SINCE + 1), seat("b", 1, Some(9.0), SINCE + 2),
    ]);
    for index in 0..team.workers.len() {
        let worker = &team.workers[index];
        let mut binding = reviewer_security::ReviewerBinding::native(
            &team, &worker.id, worker.created_at, &worker.provider, &worker.model, worker.council.as_ref().unwrap(),
        );
        assert_eq!(binding.required_reviewers, None);
        // Previously issued S2 automatic bindings incorrectly froze this count.
        binding.required_reviewers = Some(3);
        team.workers[index].reviewer_binding = Some(binding);
    }
    assert!(!council::complete(&team, &original, 1));
    team.config.council = Some(on(9.0, 10, 2));
    assert!(finish_refusal(&team).is_none(), "lowering the automatic count completes the round");
    team.config.council = Some(on(9.0, 10, 4));
    assert!(finish_refusal(&team).unwrap().contains("at least 4 needed"));
}

#[test]
fn selected_roster_change_recovers_a_settled_incomplete_round_without_losing_history() {
    let mut original = on(9.0, 10, 0);
    original.reviewer_bot_ids = vec!["bot-a".into(), "bot-b".into()];
    let mut team = team_with(original.clone(), vec![
        seat("a", 1, Some(5.0), SINCE + 1), seat("b", 1, None, SINCE + 2),
    ]);
    team.workers[1].status = "running".into();
    let mut next = original.clone();
    next.reviewer_bot_ids[1] = "bot-c".into();
    assert!(bot_reviewers::allow_roster_change(&team, &Some(next.clone())).unwrap_err().contains("Finish or stop"));
    team.workers[1].status = "stopped".into();
    assert!(bot_reviewers::allow_roster_change(&team, &Some(next.clone())).is_ok());
    bot_reviewers::recover_roster_change(&mut team, &Some(next.clone()));
    team.config.council = Some(next.clone());
    assert!(team.workers[1].abandoned);
    assert!(!team.workers[0].abandoned);
    assert_eq!(team.workers[0].council.as_ref().unwrap().score, Some(5.0));
    assert!(finish_refusal(&team).unwrap().contains("closed by the user's reviewer selection change"));
    assert_eq!(council::round_for(&team, &next, None).unwrap(), 2, "a user selection change explicitly recovers, even after a low score");
    assert!(council::seat_for(&team, Some("new lens"), None).unwrap_err().contains("fixed"));
    let saved = serde_json::to_string(&team).unwrap();
    let restored: TeamSnapshot = serde_json::from_str(&saved).unwrap();
    assert_eq!(council::round_for(&restored, &next, None).unwrap(), 2);
    next.max_rounds = 1;
    team.config.council = Some(next.clone());
    assert!(council::round_for(&team, &next, None).unwrap_err().contains("limit"));
    assert_eq!(finish_refusal(&team), None, "recovery cannot bypass the native round limit");
}

#[test]
fn selected_roster_cannot_shrink_below_the_cycles_fixed_lenses() {
    let mut original = on(9.0, 10, 0);
    original.reviewer_bot_ids = vec!["bot-a".into(), "bot-b".into(), "bot-c".into()];
    let mut team = team_with(original.clone(), vec![
        seat("a", 1, Some(9.0), SINCE + 1), seat("b", 1, Some(9.0), SINCE + 2), seat("c", 1, Some(9.0), SINCE + 3),
    ]);
    let mut smaller = original;
    smaller.reviewer_bot_ids.pop();
    assert!(bot_reviewers::allow_roster_change(&team, &Some(smaller.clone())).unwrap_err().contains("fixed 3 lenses"));
    team.council_since += 100;
    assert!(bot_reviewers::allow_roster_change(&team, &Some(smaller.clone())).is_ok(), "previous-cycle lenses do not constrain a new roster");
    team.council_since = SINCE;
    team.config.council = Some(smaller);
    team.workers.pop();
    team.workers[1].council.as_mut().unwrap().score = None;
    team.workers[1].abandoned = true;
    assert!(council::seat_for(&team, Some("third lens"), None).unwrap_err().contains("cannot cover"));
}

#[test]
fn recovering_an_automatic_round_with_a_smaller_selected_quorum_needs_fresh_scores() {
    let mut team = team_with(on(9.0, 10, 3), vec![
        seat("a", 1, Some(9.0), SINCE + 1), seat("b", 1, Some(9.0), SINCE + 2),
    ]);
    let mut selected = on(9.0, 10, 0);
    selected.reviewer_bot_ids = vec!["bot-a".into(), "bot-b".into()];
    bot_reviewers::allow_roster_change(&team, &Some(selected.clone())).unwrap();
    bot_reviewers::recover_roster_change(&mut team, &Some(selected.clone()));
    team.config.council = Some(selected.clone());
    assert!(!council::complete(&team, &selected, 1));
    assert_eq!(council::status(&team).unwrap().state, "dispatch");
    assert!(finish_refusal(&team).unwrap().contains("closed"));
    assert_eq!(council::round_for(&team, &selected, None).unwrap(), 2);
}

#[test]
fn selected_round_freezes_all_profiles_before_split_dispatch_and_replacement() {
    let (store, mut team) = fixture();
    let project = store.session(&team.session_id).unwrap().project_id;
    for id in ["bot-a", "bot-b"] {
        store.db.execute("INSERT INTO bots(id,name,role,instructions,goals,soul,project_id,provider,model,mode,created_at,updated_at,skills) VALUES(?1,?1,'QA','Original instructions','Original goals','Original soul',?2,'codex','frozen-model','full',1,1,'[\"original-skill\"]')", params![id, project]).unwrap();
    }
    let mut config = on(9.0, 10, 0);
    config.reviewer_bot_ids = vec!["bot-a".into(), "bot-b".into()];
    config.review_permissions = Some(reviewer_security::ReviewerPermissionSelection {
        authorization_id: "user-task-grant".into(),
        grants: vec![reviewer_security::ReviewerCapabilityGrant {
            tool: "pandacode_open_file".into(), action: "call".into(), action_argument: None,
            resource_argument: "path".into(), resource: "src/main.ts".into(),
        }],
    });
    team.config.council = Some(config);
    team.council_since = SINCE;
    // Only executable discovery is disabled: this test executes the real DB resolver and policy.
    let first = bot_reviewers::seat_bot_inner(&store, &team, 1, None, false).unwrap().unwrap();
    let mut worker = seat("first", 1, None, SINCE + 1);
    worker.status = "failed".into();
    worker.provider = first.profile.provider.clone();
    worker.model = first.profile.model.clone();
    let mut binding = reviewer_security::ReviewerBinding::native(&team, &worker.id, worker.created_at, &worker.provider, &worker.model, worker.council.as_ref().unwrap());
    binding.reviewer_bot_id = Some(first.id.clone());
    binding.roster_bot_ids = first.roster_bot_ids.clone();
    binding.snapshot.bot_context = Some(first.context.clone());
    binding.snapshot.tool_policy = first.tool_policy.clone();
    reviewer_security::record_native_authority(&store, &team.session_id, &binding).unwrap();
    worker.reviewer_binding = Some(binding);
    team.workers.push(worker);
    store.db.execute("UPDATE bots SET provider='claude',model='edited-model',name='Edited',instructions='Edited instructions',goals='Edited goals',soul='Edited soul',reasoning_effort='high',skills='[\"edited-skill\"]'", []).unwrap();
    // Simulate a persisted reload between separate dispatches and an unrelated setting edit.
    team = serde_json::from_str(&serde_json::to_string(&team).unwrap()).unwrap();
    team.config.council.as_mut().unwrap().review_permissions = None;
    let second = bot_reviewers::seat_bot_inner(&store, &team, 1, None, false).unwrap().unwrap();
    assert_eq!(second.id, "bot-b");
    assert_eq!(second.profile.provider, "codex");
    assert_eq!(second.profile.model, "frozen-model");
    assert_eq!(second.profile.reasoning_effort, None);
    assert_eq!(second.context.instructions, "Original instructions");
    assert_eq!(second.context.goals, "Original goals");
    assert_eq!(second.context.soul, "Original soul");
    assert_eq!(second.context.skill_ids, vec!["original-skill"]);
    assert_eq!(second.tool_policy, first.tool_policy);
    let replacement = bot_reviewers::seat_bot_inner(&store, &team, 1, Some("first"), false).unwrap().unwrap();
    assert_eq!(replacement, first, "replacements retain persona, model, identity and task grants");
    let ordinary = crate::bots::get(&store, "bot-a").unwrap();
    assert_eq!(ordinary.model, "edited-model", "resolving frozen seats never rewrites the ordinary bot profile");
    assert_eq!(ordinary.instructions, "Edited instructions");
    let fresh = bot_reviewers::seat_bot_inner(&store, &team, 2, None, false).unwrap().unwrap();
    assert_eq!(fresh.profile.model, "edited-model", "a fresh round may adopt profile edits");
    assert_eq!(fresh.tool_policy, reviewer_security::ReviewerToolPolicy::NativeReadOnly);
    store.db.execute("UPDATE bots SET paused=1 WHERE id='bot-b'", []).unwrap();
    assert!(bot_reviewers::seat_bot_inner(&store, &team, 1, None, false).unwrap_err().contains("paused"));
    store.db.execute("DELETE FROM bots WHERE id='bot-b'", []).unwrap();
    assert!(bot_reviewers::seat_bot_inner(&store, &team, 1, None, false).unwrap_err().contains("removed"));
    team.workers[0].reviewer_binding.as_mut().unwrap().snapshot.bot_context.as_mut().unwrap().instructions = "forged".into();
    assert!(bot_reviewers::seat_bot_inner(&store, &team, 1, Some("first"), false).unwrap_err().contains("native authorization"));
}

#[test]
fn both_native_configuration_paths_recover_stopped_selected_rounds() {
    for use_setup in [false, true] {
        let (state, sid) = council_state(on(9.0, 10, 0));
        let mut team = saved(&state, &sid);
        team.config.council.as_mut().unwrap().reviewer_bot_ids = vec!["bot-a".into(), "bot-b".into()];
        team.workers = vec![seat("stopped", 1, None, storage::now())];
        team.workers[0].status = "stopped".into();
        team.status = "paused".into();
        store_team(&state, &mut team);
        let recovered = if use_setup {
            let store = state.store.lock().unwrap();
            let mut config = team.config.clone();
            config.council.as_mut().unwrap().reviewer_bot_ids.clear();
            let setup = profiles::TeamSetup { architect: profiles::ModelSelection::from_session(&store.session(&sid).unwrap()), config };
            apply_setup(&store, &mut team, &setup).unwrap();
            save(&store, &mut team).unwrap();
            team
        } else {
            council::set(&state, &sid, Some(on(9.0, 10, 0))).unwrap()
        };
        assert_eq!(recovered.status, "paused", "the user still controls Resume");
        assert!(recovered.workers[0].abandoned);
        assert!(recovered.workers[0].council.as_ref().unwrap().closed_by_selection);
        assert_eq!(council::round_for(&recovered, recovered.config.council.as_ref().unwrap(), None).unwrap(), 2);
    }
}

#[test]
fn selected_bot_authority_cannot_be_forged_in_a_legacy_reviewer() {
    let (state, sid) = council_state(on(9.0, 2, 2));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments": [reviewer_assignment("review", "security")]})).unwrap();
    let mut team = saved(&state, &sid);
    let session = team.workers[0].session_id.clone();
    let id = team.workers[0].id.clone();
    let binding = team.workers[0].reviewer_binding.as_mut().unwrap();
    binding.reviewer_bot_id = Some("forged-bot".into());
    binding.snapshot.bot_context = Some(reviewer_security::ReviewerBotContext {
        name: "Forged".into(), role: "Reviewer".into(), instructions: String::new(),
        goals: String::new(), soul: String::new(), reasoning_effort: String::new(), skill_ids: vec![],
    });
    let store = state.store.lock().unwrap();
    store.db.execute("DELETE FROM architect_requests WHERE worker_id=?1 AND kind='reviewer_authority'", [&id]).unwrap();
    save(&store, &mut team).unwrap();
    let admission = Admission { parent: sid.clone(), generation: team.generation };
    assert!(reviewer_policy(&store, &session, Some(&admission)).unwrap_err().contains("capability_not_authorized"));
}


/// `max_rounds` 0 here stands for the most a config allows.
fn on(target: f64, max_rounds: usize, reviewers: usize) -> CouncilConfig {
    CouncilConfig { target, max_rounds: if max_rounds == 0 { 100 } else { max_rounds }, reviewers, ..CouncilConfig::default() }
}
/// A settled reviewer of the current cycle.
fn seat(id: &str, round: usize, score: Option<f64>, created: i64) -> Worker {
    let mut reviewer = worker(id, "completed", &[], &[]);
    reviewer.read_only = true;
    reviewer.created_at = created;
    reviewer.updated_at = created;
    reviewer.council = Some(CouncilSeat { round, lens: format!("lens {id}"), score, started_at: created, closed_by_selection: false });
    reviewer
}
fn writer(id: &str, wrote_at: i64) -> Worker {
    let mut writer = worker(id, "completed", &["src"], &[]);
    writer.created_at = wrote_at;
    writer.updated_at = wrote_at;
    writer.wrote_at = wrote_at;
    writer
}
fn team_with(config: CouncilConfig, workers: Vec<Worker>) -> TeamSnapshot {
    let (_, mut team) = fixture();
    team.config.council = Some(config);
    team.council_since = SINCE;
    team.workers = workers;
    team
}
/// A running team with Council on, saved in a fresh app state.
fn council_state(config: CouncilConfig) -> (AppState, String) {
    let (state, sid) = state_fixture();
    council::set(&state, &sid, Some(config)).unwrap();
    (state, sid)
}
fn reviewer_assignment(key: &str, lens: &str) -> Value {
    json!({"key":key,"profileId":"fast","name":key,"prompt":format!("Review {key}."),"files":[],"councilLens":lens})
}
#[test]
fn zcode_can_be_selected_as_a_council_reviewer_but_not_a_normal_architect_worker() {
    assert!(HARNESSES.contains(&"zcode"));
    assert!(validate_worker_role("zcode", true).is_ok());
    assert!(validate_worker_role("zcode", false).unwrap_err().contains("reviewer seats"));
    for name in ["codex", "claude", "opencode", "github-copilot", "muse", "pi", "omp", "hermes"] {
        assert!(validate_worker_role(name, true).is_ok());
        assert!(validate_worker_role(name, false).is_ok());
    }
}

#[test]
fn full_access_parent_council_reviewer_enforces_native_read_only_without_profile_mutation() {
    let (state, sid) = council_state(on(9.0, 2, 2));
    {
        let store = state.store.lock().unwrap();
        store.db.execute("UPDATE sessions SET mode='full' WHERE id=?1", [&sid]).unwrap();
    }
    call(&state, &sid, "pandacode_dispatch_workers", json!({
        "assignments": [reviewer_assignment("secure-review", "security")]
    })).unwrap();
    let action = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    let (run, _) = claim(&state, action);
    assert_eq!(run.config.mode, "plan", "Council must not inherit full access");
    assert!(run.config.reviewer_boundary);
    assert_eq!(run.config.reviewer_policy, Some(reviewer_security::ReviewerToolPolicy::NativeReadOnly));
    assert!(run.config.skill.is_none());
    assert!(run.config.goal.is_none());
    assert!(run.config.codex_settings.is_none());
    let store = state.store.lock().unwrap();
    assert_eq!(store.session(&sid).unwrap().mode, "full", "ordinary bot session remains unchanged");
    let saved = load(&store, &sid).unwrap().unwrap();
    let worker = &saved.workers[0];
    let binding = worker.reviewer_binding.as_ref().unwrap();
    assert_eq!(binding.seat_id, worker.id);
    assert_eq!(binding.round, 1);
    assert_eq!(binding.snapshot.provider, worker.provider);
    assert_eq!(binding.snapshot.tool_policy, reviewer_security::ReviewerToolPolicy::NativeReadOnly);
    assert!(binding.delivery_id.is_none());
    assert!(binding.reviewer_bot_id.is_none());
}

#[test]
fn mismatched_immutable_reviewer_binding_blocks_admission() {
    let (state, sid) = council_state(on(9.0, 2, 2));
    call(&state, &sid, "pandacode_dispatch_workers", json!({
        "assignments": [reviewer_assignment("review", "correctness")]
    })).unwrap();
    let mut team = saved(&state, &sid);
    let reviewer_session = team.workers[0].session_id.clone();
    team.workers[0].reviewer_binding.as_mut().unwrap().snapshot.provider = "forged".into();
    store_team(&state, &mut team);
    let admission = Admission { parent: sid.clone(), generation: team.generation };
    let error = reviewer_boundary(&state.store.lock().unwrap(), &reviewer_session, Some(&admission))
        .unwrap_err();
    assert!(error.contains("reviewer_session_mismatch"), "{error}");
}

#[test]
fn persisted_pre_s1_automatic_council_seat_gets_the_same_native_guard() {
    let (state, sid) = council_state(on(9.0, 2, 2));
    call(&state, &sid, "pandacode_dispatch_workers", json!({
        "assignments": [reviewer_assignment("legacy-review", "security")]
    })).unwrap();
    let mut team = saved(&state, &sid);
    let reviewer_session = team.workers[0].session_id.clone();
    team.workers[0].reviewer_binding = None;
    store_team(&state, &mut team);
    let admission = Admission { parent: sid.clone(), generation: team.generation };
    assert!(reviewer_boundary(&state.store.lock().unwrap(), &reviewer_session, Some(&admission)).unwrap());
    assert!(!reviewer_boundary(&state.store.lock().unwrap(), &sid, Some(&admission)).unwrap());
}

#[test]
fn frozen_revision_and_unbound_bot_delivery_ids_are_validated() {
    let (state, sid) = council_state(on(9.0, 2, 2));
    call(&state, &sid, "pandacode_dispatch_workers", json!({
        "assignments": [reviewer_assignment("revision-guard", "security")]
    })).unwrap();
    let mut team = saved(&state, &sid);
    let session = team.workers[0].session_id.clone();
    let admission = Admission { parent: sid.clone(), generation: team.generation };
    let original = team.workers[0].reviewer_binding.clone().unwrap();
    for changed in 0..3 {
        let mut binding = original.clone();
        match changed {
            0 => binding.artifact_revision = "forged".into(),
            1 => binding.reviewer_bot_id = Some("unexpected-bot".into()),
            _ => binding.delivery_id = Some("unexpected-delivery".into()),
        }
        team.workers[0].reviewer_binding = Some(binding);
        store_team(&state, &mut team);
        let error = reviewer_policy(&state.store.lock().unwrap(), &session, Some(&admission))
            .unwrap_err();
        assert!(error.contains("reviewer_session_mismatch"), "{error}");
    }
    team.workers[0].reviewer_binding = Some(original);
    store_team(&state, &mut team);
    assert!(reviewer_policy(&state.store.lock().unwrap(), &session, Some(&admission))
        .unwrap().is_some());
}

#[test]
fn forged_but_well_formed_revision_is_rejected_against_native_issuance() {
    let (state, sid) = council_state(on(9.0, 2, 2));
    call(&state, &sid, "pandacode_dispatch_workers", json!({
        "assignments": [reviewer_assignment("revision-integrity", "correctness")]
    })).unwrap();
    let mut team = saved(&state, &sid);
    let session = team.workers[0].session_id.clone();
    let original = team.workers[0].reviewer_binding.clone().unwrap();
    let parts = original.artifact_revision.split(':').collect::<Vec<_>>();
    assert_eq!(parts.len(), 4);
    let next_revision = parts[1].parse::<u64>().unwrap() + 1;
    team.dispatch_revision = next_revision;
    team.workers[0].reviewer_binding.as_mut().unwrap().artifact_revision =
        format!("{}:{}:{}:{}", parts[0], next_revision, parts[2], parts[3]);
    store_team(&state, &mut team);
    let admission = Admission { parent: sid.clone(), generation: team.generation };
    let error = reviewer_policy(&state.store.lock().unwrap(), &session, Some(&admission)).unwrap_err();
    assert!(error.contains("reviewer_session_mismatch"), "{error}");
}

#[test]
fn scoped_reviewer_grants_round_trip_into_native_run_config() {
    use reviewer_security::{ReviewerCapabilityGrant, ReviewerPhase, ReviewerToolPolicy};
    let (state, sid) = council_state(on(9.0, 2, 2));
    call(&state, &sid, "pandacode_dispatch_workers", json!({
        "assignments": [reviewer_assignment("granted-review", "correctness")]
    })).unwrap();
    let mut team = saved(&state, &sid);
    let grant = ReviewerCapabilityGrant {
        tool: "pandacode_github_issue".into(),
        action: "create".into(),
        action_argument: Some("action".into()),
        resource_argument: "repository".into(),
        resource: "owner/repo".into(),
    };
    let policy = ReviewerToolPolicy::from_user_grants(
        ReviewerPhase::Review, "user-approval-1", std::slice::from_ref(&grant), std::slice::from_ref(&grant),
    ).unwrap();
    // A valid policy created from inputs is still not authority. It must be
    // committed by the native trusted-user path, independent of worker JSON.
    let binding = team.workers[0].reviewer_binding.clone().unwrap();
    team.workers[0].reviewer_binding.as_mut().unwrap().snapshot.tool_policy = policy.clone();
    store_team(&state, &mut team);
    let session = team.workers[0].session_id.clone();
    let admission = Admission { parent: sid.clone(), generation: team.generation };
    assert!(reviewer_policy(&state.store.lock().unwrap(), &session, Some(&admission)).is_err());
    let actually_issued = reviewer_security::authorize_user_grants(
        &state.store.lock().unwrap(), &sid, &binding, ReviewerPhase::Review,
        "user-approval-1", std::slice::from_ref(&grant), std::slice::from_ref(&grant),
    ).unwrap();
    assert_eq!(actually_issued, policy);
    assert_eq!(reviewer_policy(&state.store.lock().unwrap(), &session, Some(&admission)).unwrap(), Some(policy.clone()));
    let action = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    let (run, _) = claim(&state, action);
    assert_eq!(run.config.reviewer_policy, Some(policy));
    assert!(run.config.reviewer_boundary);
    assert_eq!(run.config.mode, "plan");
}

fn saved(state: &AppState, sid: &str) -> TeamSnapshot {
    load(&state.store.lock().unwrap(), sid).unwrap().unwrap()
}
fn store_team(state: &AppState, team: &mut TeamSnapshot) {
    save(&state.store.lock().unwrap(), team).unwrap();
}

#[test]
fn score_parsing_takes_the_last_marked_line() {
    assert_eq!(parse_score("Fine.\n\nCouncil score: 8/10"), Some(8.0));
    assert_eq!(parse_score("**Council score: 9.5/10**"), Some(9.5));
    assert_eq!(parse_score("**Council score:** 7"), Some(7.0));
    assert_eq!(parse_score("## council SCORE: 10 / 10\n"), Some(10.0));
    assert_eq!(parse_score("Council score: 0/10"), Some(0.0));
    // Only a line that starts with it counts.
    assert_eq!(parse_score("The council score was 3/10 last time."), None);
    assert_eq!(parse_score("Report.\nMy council score is 3.\nDone."), None);
    // The last one wins, even when it is unusable.
    assert_eq!(parse_score("Council score: 3/10\nFixes.\nCouncil score: 9/10\nThanks."), Some(9.0));
    assert_eq!(parse_score("Council score: 9/10\nCouncil score: N/10"), None);
    // Out of range or malformed.
    assert_eq!(parse_score("Council score: 11/10"), None);
    assert_eq!(parse_score("Council score: 10.5"), None);
    assert_eq!(parse_score("Council score: -3/10"), None);
    assert_eq!(parse_score("Council score: 8/100"), None);
    assert_eq!(parse_score("Council score: 8/5"), None);
    assert_eq!(parse_score("Council scores: 8/10"), None);
    assert_eq!(parse_score("Council score: 1e400"), None);
    assert_eq!(parse_score("Council score:"), None);
    assert_eq!(parse_score(""), None);
}

#[test]
fn config_validation_bounds_every_field() {
    for good in [on(1.0, 0, 0), on(10.0, 100, 8), on(9.5, 3, 2)] {
        assert!(good.validate().is_ok());
    }
    for bad in [on(0.5, 0, 0), on(10.5, 0, 0), on(f64::NAN, 0, 0), on(f64::INFINITY, 0, 0), on(9.0, 101, 0), on(9.0, 0, 9)] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    let mut team_config = config();
    team_config.council = Some(on(11.0, 0, 0));
    assert!(team_config.validate().is_err());
}

#[test]
fn saved_json_from_before_council_still_parses() {
    let (_, mut team) = fixture();
    team.workers = vec![worker("a", "completed", &[], &[])];
    let value = serde_json::to_value(&team).unwrap();
    assert!(value["config"].get("council").is_none(), "skipped when off");
    assert!(value["workers"][0].get("council").is_none(), "skipped when off");
    let mut old = value;
    old.as_object_mut().unwrap().remove("councilSince");
    let row = old["workers"][0].as_object_mut().unwrap();
    row.remove("wroteAt");
    row.remove("scoreNudged");
    let parsed: TeamSnapshot = serde_json::from_value(old).unwrap();
    assert_eq!(parsed.council_since, 0);
    assert_eq!(parsed.workers[0].wrote_at, 0);
    assert!(parsed.config.council.is_none());
    // A config sent without its optional fields takes the defaults.
    let config: CouncilConfig = serde_json::from_value(json!({"target":9.5})).unwrap();
    assert_eq!(config, on(9.5, 10, 0), "ten rounds unless the user says otherwise");
    // Rounds are always bounded.
    let none: CouncilConfig = serde_json::from_value(json!({"target":9.0,"maxRounds":0})).unwrap();
    assert!(none.validate().is_err());
    assert_eq!(serde_json::to_value(on(9.5, 3, 2)).unwrap(), json!({"target":9.5,"maxRounds":3,"reviewers":2}));
}

#[test]
fn lens_needs_council_and_a_valid_name() {
    let (state, sid) = state_fixture();
    let refused = call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "correctness")]})).unwrap_err();
    assert!(refused.contains("Council is off"), "{refused}");
    assert!(saved(&state, &sid).workers.is_empty());
    council::set(&state, &sid, Some(on(9.0, 0, 0))).unwrap();
    for lens in ["", "   ", &"x".repeat(81), "two\nlines"] {
        let refused = call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", lens)]})).unwrap_err();
        assert!(refused.contains("councilLens"), "{refused}");
    }
}

#[test]
fn a_lens_forces_read_only_and_joins_one_round_per_batch() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut first = reviewer_assignment("r1", "correctness");
    first["readOnly"] = json!(false);
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[first, reviewer_assignment("r2", "security")]})).unwrap();
    let team = saved(&state, &sid);
    assert!(team.workers.iter().all(|w| w.read_only));
    assert_eq!(team.workers.iter().map(|w| w.council.as_ref().unwrap().round).collect::<Vec<_>>(), [1, 1]);
    assert_eq!(team.workers[1].council.as_ref().unwrap().lens, "security");
}

#[test]
fn round_joins_while_open_and_opens_the_next_when_complete() {
    let (state, sid) = council_state(on(9.0, 2, 0));
    let mut team = saved(&state, &sid);
    team.council_since = SINCE;
    // One scored seat is below the minimum of two: still open.
    team.workers = vec![seat("a", 1, Some(9.0), SINCE)];
    store_team(&state, &mut team);
    let dispatch = |key: &str| call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment(key, "ux")]}));
    dispatch("joins").unwrap();
    assert_eq!(saved(&state, &sid).workers.last().unwrap().council.as_ref().unwrap().round, 1);
    let mut team = saved(&state, &sid);
    team.workers.last_mut().unwrap().status = "completed".into();
    team.workers.last_mut().unwrap().council.as_mut().unwrap().score = Some(5.0);
    store_team(&state, &mut team);
    // Complete below the target with nothing changed since: no re-roll.
    let refused = dispatch("reroll").unwrap_err();
    assert!(refused.contains("nothing changed"), "{refused}");
    let mut team = saved(&state, &sid);
    team.workers.push(writer("fix", storage::now()));
    store_team(&state, &mut team);
    // Complete and fixed since: the next reviewer opens round 2, the configured limit.
    dispatch("next").unwrap();
    assert_eq!(saved(&state, &sid).workers.last().unwrap().council.as_ref().unwrap().round, 2);
    let mut team = saved(&state, &sid);
    team.workers.last_mut().unwrap().status = "completed".into();
    team.workers.last_mut().unwrap().council.as_mut().unwrap().score = Some(5.0);
    // Every lens round 1 seated sits again.
    team.workers.push(lensed("c", 2, Some(5.0), storage::now(), "lens a"));
    store_team(&state, &mut team);
    let refused = dispatch("past").unwrap_err();
    assert!(refused.contains("limit") && refused.contains('2'), "{refused}");
}

#[test]
fn the_configured_reviewer_count_keeps_a_round_open() {
    let team = team_with(on(9.0, 0, 3), vec![seat("a", 1, Some(9.0), SINCE), seat("b", 1, Some(9.0), SINCE)]);
    assert_eq!(council::round_for(&team, team.config.council.as_ref().unwrap(), None).unwrap(), 1);
    assert!(!council::complete(&team, team.config.council.as_ref().unwrap(), 1));
}

#[test]
fn an_abandoned_unscored_seat_leaves_its_round_but_a_score_stands() {
    // A seat that never ran (its dependency failed) drops out freely.
    let mut failed = seat("failed", 1, None, SINCE);
    failed.status = "failed".into();
    failed.abandoned = true;
    failed.council.as_mut().unwrap().started_at = 0;
    let team = team_with(on(9.0, 0, 0), vec![writer("w", SINCE), failed, seat("a", 1, Some(9.0), SINCE + 1), seat("b", 1, Some(9.5), SINCE + 1)]);
    assert_eq!(finish_refusal(&team), None);
    // An abandoned seat with a score still counts, low score and all.
    let mut low = seat("low", 1, Some(2.0), SINCE);
    low.abandoned = true;
    let mut team = team;
    team.workers.push(low);
    assert!(finish_refusal(&team).unwrap().contains("lowest 2"));
    // Unscored abandoned seats do not count toward the minimum.
    team.workers.pop();
    team.workers[2].council.as_mut().unwrap().score = None;
    team.workers[2].abandoned = true;
    assert!(finish_refusal(&team).unwrap().contains("not complete"));
}

#[test]
fn a_scored_seat_cannot_be_cancelled_or_replaced() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    let mut failed = seat("failed", 1, None, storage::now());
    failed.status = "failed".into();
    team.workers = vec![seat("a", 1, Some(4.0), storage::now()), failed];
    store_team(&state, &mut team);
    let refused = call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":"a","reason":"Too harsh."})).unwrap_err();
    assert_eq!(refused, council::SCORED_STANDS);
    let mut replacement = reviewer_assignment("again", "ux");
    replacement["replaces"] = json!("a");
    assert_eq!(call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[replacement]})).unwrap_err(), council::SCORED_STANDS);
    let team = saved(&state, &sid);
    assert!(!team.workers[0].abandoned && team.workers.len() == 2);
    // An unscored seat keeps today's paths.
    call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":"failed","reason":"It crashed."})).unwrap();
    assert!(saved(&state, &sid).workers[1].abandoned);
}

#[test]
fn a_replacement_joins_the_seat_it_replaces_and_keeps_its_lens() {
    let (state, sid) = council_state(on(9.0, 1, 0));
    let mut team = saved(&state, &sid);
    let mut failed = seat("b", 1, None, storage::now());
    failed.status = "failed".into();
    team.workers = vec![seat("a", 1, Some(9.0), storage::now()), seat("c", 1, Some(9.0), storage::now()), failed];
    store_team(&state, &mut team);
    let mut replacement = reviewer_assignment("again", "ignored");
    replacement.as_object_mut().unwrap().remove("councilLens");
    replacement["replaces"] = json!("b");
    // The round is at its limit, yet the replacement belongs to it.
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[replacement]})).unwrap();
    let team = saved(&state, &sid);
    let new = team.workers.last().unwrap();
    assert!(new.read_only);
    assert_eq!(new.council, Some(CouncilSeat { round: 1, lens: "lens b".into(), score: None, started_at: 0, closed_by_selection: false }));
    assert!(team.workers[2].abandoned);
}

#[test]
fn gate_refuses_below_target_with_the_numbers() {
    let team = team_with(on(9.5, 0, 0), vec![writer("w", SINCE), seat("a", 2, Some(8.3), SINCE + 1), seat("b", 2, Some(9.9), SINCE + 1)]);
    let refusal = finish_refusal(&team).unwrap();
    assert_eq!(refusal, "Council round 2: lowest 8.3, target 9.5. Fix the blockers with workers, then dispatch a new round with councilLens. If a remaining blocker should not be fixed, end your turn and tell the user, who can lower the target or turn Council off; if they say to accept the current scores, tell them to press Accept and finish on the Council card.");
    assert!(review(&team_with(on(9.5, 0, 0), vec![writer("w", SINCE), seat("a", 2, Some(8.3), SINCE + 1), seat("b", 2, Some(9.9), SINCE + 1)])).unwrap()["next"].as_str().unwrap().contains("lower the target"));
}

#[test]
fn gate_passes_on_the_lowest_score_not_the_average() {
    let team = team_with(on(9.0, 0, 0), vec![writer("w", SINCE), seat("a", 1, Some(9.0), SINCE + 1), seat("b", 1, Some(10.0), SINCE + 1)]);
    assert_eq!(finish_refusal(&team), None);
    let team = team_with(on(9.0, 0, 0), vec![writer("w", SINCE), seat("a", 1, Some(8.9), SINCE + 1), seat("b", 1, Some(10.0), SINCE + 1)]);
    assert!(finish_refusal(&team).is_some());
}

#[test]
fn gate_refuses_a_round_older_than_a_writer() {
    let team = team_with(on(9.0, 0, 0), vec![seat("a", 1, Some(9.5), SINCE + 1), seat("b", 1, Some(9.5), SINCE + 5), writer("w", SINCE + 2)]);
    let refusal = finish_refusal(&team).unwrap();
    assert!(refusal.contains("round 1") && refusal.contains("new round"), "{refusal}");
}

/// A reviewer with a given lens.
fn lensed(id: &str, round: usize, score: Option<f64>, created: i64, lens: &str) -> Worker {
    let mut reviewer = seat(id, round, score, created);
    reviewer.council.as_mut().unwrap().lens = lens.into();
    reviewer
}

#[test]
fn gate_at_the_round_limit() {
    // Two completed rounds below target with a limit of 2: the lead may finish and say so.
    let limit = on(9.0, 2, 0);
    let rounds = vec![writer("w", SINCE), lensed("a1", 1, Some(7.0), SINCE + 1, "ux"), lensed("b1", 1, Some(8.0), SINCE + 1, "sec"), writer("fix", SINCE + 2), lensed("a2", 2, Some(8.0), SINCE + 3, "ux"), lensed("b2", 2, Some(8.5), SINCE + 3, "sec")];
    let team = team_with(limit.clone(), rounds.clone());
    assert_eq!(finish_refusal(&team), None);
    // A writer since then cannot hold the team open: no further round can run.
    let mut later = rounds.clone();
    later.push(writer("late", SINCE + 4));
    let team = team_with(limit.clone(), later);
    assert_eq!(finish_refusal(&team), None);
    assert!(review(&team).unwrap()["next"].as_str().unwrap().contains("round limit"));
    // One completed round of two is the ordinary refusal.
    let team = team_with(limit, rounds[..3].to_vec());
    assert!(finish_refusal(&team).unwrap().contains("Fix the blockers"));
}

#[test]
fn gate_ignores_teams_without_writers_and_with_council_off() {
    // Reviewers alone, or nothing at all: as today.
    assert_eq!(finish_refusal(&team_with(on(9.0, 0, 0), vec![])), None);
    // Once a round exists its scores bind, writers or not.
    let low = team_with(on(9.0, 0, 0), vec![seat("a", 1, Some(1.0), SINCE), seat("b", 1, Some(9.0), SINCE)]);
    assert!(finish_refusal(&low).unwrap().contains("lowest 1"));
    assert!(!review(&low).unwrap()["next"].as_str().unwrap().contains("no council is needed"));
    // A writer that never ran changed nothing.
    assert_eq!(finish_refusal(&team_with(on(9.0, 0, 0), vec![writer("w", 0)])), None);
    let mut off = team_with(on(9.0, 0, 0), vec![writer("w", SINCE)]);
    off.config.council = None;
    assert_eq!(finish_refusal(&off), None);
    let refusal = finish_refusal(&team_with(on(9.0, 0, 3), vec![writer("w", SINCE)])).unwrap();
    assert!(refusal.contains("3 reviewers") && refusal.contains("councilLens"), "{refusal}");
}

#[test]
fn a_new_cycle_ignores_earlier_rounds_and_writers() {
    let old = vec![writer("old", SINCE - 10), seat("a", 4, Some(1.0), SINCE - 5), seat("b", 4, Some(1.0), SINCE - 5)];
    let team = team_with(on(9.0, 2, 0), old.clone());
    assert_eq!(finish_refusal(&team), None, "no writer in this cycle");
    assert_eq!(council::round_for(&team, team.config.council.as_ref().unwrap(), None).unwrap(), 1, "rounds restart");
    let mut current = old;
    current.push(writer("new", SINCE + 1));
    let refusal = finish_refusal(&team_with(on(9.0, 2, 0), current)).unwrap();
    assert!(refusal.contains("No Council round"), "{refusal}");
}

#[test]
fn finish_team_runs_the_gate_after_the_existing_checks() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    team.workers = vec![writer("w", storage::now())];
    store_team(&state, &mut team);
    let refused = call(&state, &sid, "pandacode_finish_team", json!({"summary":"Done."})).unwrap_err();
    assert!(refused.contains("councilLens"), "{refused}");
    assert_eq!(saved(&state, &sid).status, "running");
    let mut team = saved(&state, &sid);
    team.workers.push(seat("a", 1, Some(9.0), storage::now() + 10));
    team.workers.push(seat("b", 1, Some(9.0), storage::now() + 10));
    store_team(&state, &mut team);
    call(&state, &sid, "pandacode_finish_team", json!({"summary":"Done."})).unwrap();
    assert_eq!(saved(&state, &sid).status, "completed");
}

#[test]
fn start_state_begins_a_new_cycle_in_the_same_clock_as_workers() {
    let (state, sid) = state_fixture();
    let mut team = saved(&state, &sid);
    team.status = "ready".into();
    store_team(&state, &mut team);
    let before = storage::now();
    start_state(&state, &sid, "A new objective", None, None).unwrap();
    let since = saved(&state, &sid).council_since;
    assert!(since >= before && since <= storage::now());
    // Worker timestamps come from the same millisecond clock.
    let (run, _) = running_worker(&state, &sid, "w", "src");
    let team = saved(&state, &sid);
    assert!(team.workers[0].created_at >= since && team.workers[0].wrote_at >= since);
    drop(run);
}

#[test]
fn only_real_work_moves_a_writers_mark() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let (run, actor) = running_worker(&state, &sid, "w", "src");
    let started = saved(&state, &sid).workers[0].wrote_at;
    assert!(started > 0, "launch marks it");
    // A checkpoint is not an edit, though it moves updated_at.
    let mut team = saved(&state, &sid);
    team.workers[0].wrote_at = 5;
    store_team(&state, &mut team);
    coordinate(&state, &sid, &actor, "pandacode_checkpoint_worker", json!({"key":"c1","summary":"s","nextSteps":"n"})).unwrap();
    assert_eq!(saved(&state, &sid).workers[0].wrote_at, 5);
    finish(&state, &run, "Done.", true);
    let settled = saved(&state, &sid).workers[0].wrote_at;
    assert!(settled >= started, "a run's end marks it");
    // Cancelling or replacing a settled writer changes nothing in the checkout.
    call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":saved(&state, &sid).workers[0].id,"reason":"Not needed."})).unwrap();
    let team = saved(&state, &sid);
    assert_eq!(team.workers[0].wrote_at, settled);
    assert!(team.workers[0].abandoned);
}

#[test]
fn cancelling_a_live_writer_marks_it_and_a_queued_one_stays_unmarked() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let (_run, _) = running_worker(&state, &sid, "w", "src");
    let mut team = saved(&state, &sid);
    team.workers[0].wrote_at = 5;
    store_team(&state, &mut team);
    call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":team.workers[0].id,"reason":"Wrong."})).unwrap();
    assert!(saved(&state, &sid).workers[0].wrote_at > 5, "it ran until now");
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[assignment("q", "fast", "tests")]})).unwrap();
    let id = saved(&state, &sid).workers[1].id.clone();
    call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":id,"reason":"Not needed."})).unwrap();
    assert_eq!(saved(&state, &sid).workers[1].wrote_at, 0);
}

#[test]
fn a_reviewers_score_comes_from_the_tail_of_a_long_report() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "correctness")]})).unwrap();
    let (run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let report = format!("{}\n\n**Council score: 8.5/10**", "Finding. ".repeat(4000));
    assert!(report.len() > SUMMARY_PREVIEW);
    finish(&state, &run, &report, true);
    let team = saved(&state, &sid);
    assert_eq!(team.workers[0].status, "completed");
    assert_eq!(team.workers[0].council.as_ref().unwrap().score, Some(8.5));
    assert_eq!(team.workers[0].wrote_at, 0, "a reviewer is no writer");
    assert!(!team.workers[0].score_nudged);
}

#[test]
fn a_missing_score_is_asked_for_once_then_fails_without_looping() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "correctness")]})).unwrap();
    let (run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let reviewer_session = run.config.session_id.clone();
    finish(&state, &run, "Looks fine overall.", true);
    let team = saved(&state, &sid);
    assert_eq!(team.workers[0].status, "queued");
    assert!(team.workers[0].score_nudged);
    let (again, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    assert_eq!(again.config.session_id, reviewer_session, "same conversation");
    assert!(again.config.prompt.contains("Council score: N/10"), "{}", again.config.prompt);
    // A reply resets `nudged`; this flag must not reset, or it would ask forever.
    finish(&state, &again, "I already reported.", true);
    let team = saved(&state, &sid);
    assert_eq!(team.workers[0].status, "failed");
    assert!(team.workers[0].error.as_deref().unwrap().contains("Council score"));
    assert_eq!(team.workers[0].council.as_ref().unwrap().score, None);
    // The lead wakes for it; the reviewer does not run again.
    let next = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert_eq!(next.sid, sid);
}

#[test]
fn a_nudged_reviewer_scores_and_keeps_its_report() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "correctness")]})).unwrap();
    let (run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    finish(&state, &run, "The parser drops the last token.", true);
    let (again, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    assert!(again.config.prompt.contains("in full"), "{}", again.config.prompt);
    finish(&state, &again, "The parser drops the last token.\n\nCouncil score: 6/10", true);
    let team = saved(&state, &sid);
    assert_eq!(team.workers[0].status, "completed");
    assert_eq!(team.workers[0].council.as_ref().unwrap().score, Some(6.0));
    assert!(team.workers[0].summary.contains("drops the last token"), "{}", team.workers[0].summary);
}

#[test]
fn failed_and_parked_reviewers_keep_their_paths() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "correctness")]})).unwrap();
    let (run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    finish(&state, &run, "Council score: 9/10", false);
    let team = saved(&state, &sid);
    assert_eq!(team.workers[0].status, "failed");
    assert_eq!(team.workers[0].council.as_ref().unwrap().score, None, "a failed run has no score");
    assert!(!team.workers[0].score_nudged);
    // A sub-worker still out parks the reviewer without asking for its score.
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "correctness")]})).unwrap();
    let (run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let mut team = saved(&state, &sid);
    let mut child = worker("child", "running", &[], &[]);
    child.parent_worker = Some(team.workers[0].id.clone());
    team.workers.push(child);
    store_team(&state, &mut team);
    finish(&state, &run, "Waiting on my sub-worker.", true);
    let team = saved(&state, &sid);
    assert_eq!(team.workers[0].status, "parked");
    assert!(!team.workers[0].score_nudged);
}

#[test]
fn followup_is_refused_for_a_reviewer() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    team.workers = vec![seat("a", 1, Some(5.0), storage::now())];
    store_team(&state, &mut team);
    let refused = call(&state, &sid, "pandacode_followup_worker", json!({"workerId":"a","key":"more","prompt":"Look again."})).unwrap_err();
    assert_eq!(refused, "Dispatch a fresh reviewer for the next round");
}

#[test]
fn sub_workers_cannot_convene() {
    assert!(catalog().iter().any(|t| t["name"] == "pandacode_dispatch_workers" && t["inputSchema"]["properties"]["assignments"]["items"]["properties"]["councilLens"]["description"].as_str().is_some_and(|d| d.contains("lens"))));
    assert!(sub_worker_catalog().iter().all(|t| t["inputSchema"]["properties"]["assignments"]["items"]["properties"].get("councilLens").is_none()));
    let (state, sid) = council_state(on(9.0, 0, 0));
    let (_run, actor) = running_worker(&state, &sid, "w", "src");
    let coordination::Actor::Worker { id, attempt } = actor else { panic!("a worker") };
    let team = saved(&state, &sid);
    let refused = dispatch_as(&state, &sid, team.generation, Some((&id, attempt)), "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "ux")]}), &AtomicBool::new(false), &routing::AutoRoutes::default()).unwrap_err();
    assert!(refused.contains("lead"), "{refused}");
}

#[test]
fn set_council_validates_and_applies_at_once() {
    let (state, sid) = state_fixture();
    assert!(council::set(&state, &sid, Some(on(0.0, 0, 0))).is_err());
    assert!(council::set(&state, "no-such-task", Some(on(9.0, 0, 0))).unwrap_err().contains("team"));
    let revision = saved(&state, &sid).dispatch_revision;
    let team = council::set(&state, &sid, Some(on(9.0, 4, 2))).unwrap();
    assert_eq!(team.config.council, Some(on(9.0, 4, 2)));
    assert_eq!(saved(&state, &sid).config.council, Some(on(9.0, 4, 2)));
    assert_eq!(team.dispatch_revision, revision, "no review turn for a setting");
    assert_eq!(council::set(&state, &sid, None).unwrap().config.council, None);
}

#[test]
fn set_council_reaches_a_queued_setup() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    let mut queued = team.config.clone();
    queued.council = Some(on(5.0, 0, 0));
    team.pending_setup = Some(profiles::TeamSetup { architect: profiles::ModelSelection::from_session(&state.store.lock().unwrap().session(&sid).unwrap()), config: queued });
    store_team(&state, &mut team);
    let team = council::set(&state, &sid, None).unwrap();
    assert_eq!(team.pending_setup.unwrap().config.council, None, "turning it off is not undone when the setup applies");
}

#[test]
fn a_new_setup_without_council_keeps_it() {
    let (state, sid) = council_state(on(9.0, 3, 0));
    let mut other = config();
    other.max_parallel = 1;
    let team = configure_state(&state, &sid, Some(other), None, None).unwrap().unwrap();
    assert_eq!(team.config.max_parallel, 1);
    assert_eq!(team.config.council, Some(on(9.0, 3, 0)));
    // One that names its own takes effect.
    let mut own = config();
    own.council = Some(on(7.0, 0, 0));
    let team = configure_state(&state, &sid, Some(own), None, None).unwrap().unwrap();
    assert_eq!(team.config.council, Some(on(7.0, 0, 0)));
}

#[test]
fn prompts_carry_config_only_and_reviewers_never_see_the_target() {
    let (state, sid) = council_state(on(9.5, 3, 2));
    let (_run, _) = running_worker(&state, &sid, "w", "src");
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "security")]})).unwrap();
    let store = state.store.lock().unwrap();
    let mut team = load(&store, &sid).unwrap().unwrap();
    let (lead, _) = rules(&store, &team, &sid).unwrap();
    assert!(lead.contains("councilLens") && lead.contains("9.5") && lead.contains("exactly 2") && lead.contains("3 rounds"), "{lead}");
    assert!(council::lead_rules(&team, false).contains("a different roster model for each reviewer"));
    assert!(!council::lead_rules(&team, true).contains("different roster model"));
    assert!(lead.contains("visual quality, pacing"));
    // Live status would resend the rules on every change.
    let before = lead.clone();
    team.workers[1].council.as_mut().unwrap().score = Some(4.0);
    assert_eq!(rules(&store, &team, &sid).unwrap().0, before);
    let (reviewer, _) = rules(&store, &team, &team.workers[1].session_id.clone()).unwrap();
    assert!(reviewer.contains("`Council score: N/10`") && reviewer.contains("security") && reviewer.contains("Score strictly") && !reviewer.contains("9+"), "{reviewer}");
    assert!(!reviewer.contains("9.5"), "{reviewer}");
    assert!(reviewer.contains("frames, renders") && reviewer.contains("the work your assignment names"), "{reviewer}");
    assert!(reviewer.contains("Do not change the work under review") && reviewer.contains("temporary directory"), "{reviewer}");
    assert!(!reviewer.contains("the whole checkout") && !reviewer.contains("writing follow-up"), "{reviewer}");
    let (plain, _) = rules(&store, &team, &team.workers[0].session_id.clone()).unwrap();
    assert!(!plain.contains("Council"), "{plain}");
    team.config.council = None;
    assert!(!rules(&store, &team, &sid).unwrap().0.contains("councilLens"));
}

#[test]
fn the_review_payload_reports_the_round() {
    let team = team_with(on(9.0, 3, 0), vec![writer("w", SINCE), seat("a", 2, Some(8.0), SINCE + 1), seat("b", 2, Some(9.0), SINCE + 1)]);
    let review = review_summaries(&team, true);
    let council = &review["council"];
    assert_eq!(council["round"], 2);
    assert_eq!(council["target"], 9.0);
    assert_eq!(council["lowest"], 8.0);
    assert_eq!(council["complete"], true);
    assert_eq!(council["passed"], false);
    assert_eq!(council["roundsLeft"], 2, "only round 2 completed");
    assert!(council["next"].as_str().unwrap().contains("Fix the blockers"));
    let passing = team_with(on(8.0, 0, 0), team.workers.clone());
    let council = &review_summaries(&passing, true)["council"];
    assert_eq!(council["passed"], true);
    assert_eq!(council["roundsLeft"], 99, "one completed round of 100");
    let (_, plain) = fixture();
    assert!(review_summaries(&plain, true).get("council").is_none());
    // Reviewers' seats are in list_workers too, for the lead's final table.
    assert_eq!(summaries(&team, 0, None)["workers"][1]["council"]["score"], 8.0);
}

#[test]
fn one_roster_model_keeps_auto_in_the_rules() {
    let mut team = team_with(on(9.0, 0, 0), vec![]);
    team.config.workers.truncate(1);
    assert!(!council::lead_rules(&team, false).contains("different roster model"));
}

#[test]
fn a_new_message_continues_an_unfinished_cycle() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    team.status = "paused".into();
    team.council_since = SINCE;
    team.workers = vec![writer("w", SINCE + 1), seat("a", 1, Some(5.0), SINCE + 2), seat("b", 1, Some(9.0), SINCE + 2)];
    store_team(&state, &mut team);
    start_state(&state, &sid, "Keep going", None, None).unwrap();
    let team = saved(&state, &sid);
    assert_eq!(team.council_since, SINCE, "the cycle and its rounds continue");
    assert_eq!(team.council_status.as_ref().unwrap().round, 1);
    // A passed cycle ends: the next message starts a new one.
    let mut team = team;
    team.status = "ready".into();
    team.workers[1].council.as_mut().unwrap().score = Some(9.5);
    store_team(&state, &mut team);
    start_state(&state, &sid, "Something new", None, None).unwrap();
    assert!(saved(&state, &sid).council_since > SINCE);
}

/// A turn of `session` that recorded changes to `paths`, at `at`.
fn turn_change(state: &AppState, session: &str, id: &str, at: i64, paths: &[&str], sources: &[&str]) {
    let store = state.store.lock().unwrap();
    let message: i64 = store.db.query_row("INSERT INTO messages(session_id,role,content,created_at) VALUES(?1,'changes','',?2) RETURNING id", params![session, at], |r| r.get(0)).unwrap();
    let files: Vec<Value> = paths.iter().map(|p| json!({"path":p,"status":"modified","additions":1,"deletions":0,"binary":false,"truncated":false})).collect();
    let mut summary = json!({"id":id,"files":files,"additions":1,"deletions":0,"truncated":false,"concurrent":false,"attributed":true});
    if !sources.is_empty() {
        summary["teamSources"] = json!(sources);
    }
    store.db.execute("INSERT INTO turn_changes(id,session_id,message_id,summary,created_at,diff_bytes) VALUES(?1,?2,?3,?4,?5,0)", params![id, session, message, summary.to_string(), at]).unwrap();
}

#[test]
fn recorded_turn_changes_count_as_changes_but_not_a_reviewers_own() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    // A reviewer that renders into the checkout does not outdate its own round.
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "ux")]})).unwrap();
    let reviewer = saved(&state, &sid).workers[0].session_id.clone();
    let mut team = saved(&state, &sid);
    team.workers[0].status = "completed".into();
    team.workers[0].council.as_mut().unwrap().score = Some(9.0);
    team.workers[0].council.as_mut().unwrap().started_at = storage::now() - 100;
    let mut other = seat("b", 1, Some(9.0), storage::now());
    other.council.as_mut().unwrap().started_at = storage::now() - 100;
    team.workers.push(other);
    store_team(&state, &mut team);
    assert_eq!(saved(&state, &sid).council_status.unwrap().state, "passed", "no writer at all");
    turn_change(&state, &reviewer, "c1", storage::now(), &["frames/001.png"], &[]);
    store_team(&state, &mut saved(&state, &sid));
    assert_eq!(saved(&state, &sid).council_status.unwrap().state, "passed");
    // A worker's change does.
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[assignment("w", "fast", "src")]})).unwrap();
    let writer = saved(&state, &sid).workers[2].session_id.clone();
    turn_change(&state, &writer, "c2", storage::now(), &["src/a.rs"], &[]);
    store_team(&state, &mut saved(&state, &sid));
    let status = saved(&state, &sid).council_status.unwrap();
    assert!(status.stale && status.state == "fixing", "{status:?}");
}

/// Saves a turn's changes to `paths` the way a finished run does.
fn persist_turn(state: &AppState, session: &str, paths: &[&str]) {
    let file = |path: &&str| crate::turn_changes::ChangedFile { path: path.to_string(), old_path: None, status: "modified".into(), additions: Some(1), deletions: Some(0), binary: false, truncated: false };
    let detail = |path: &&str| crate::turn_changes::TurnChangeDiff { path: path.to_string(), old_path: None, patch: "+x\n".into(), binary: false, truncated: false, error: None };
    let summary = crate::turn_changes::TurnChanges { id: uuid::Uuid::new_v4().to_string(), files: paths.iter().map(file).collect(), additions: paths.len(), deletions: 0, truncated: false, concurrent: false, attributed: true, error: None };
    crate::turn_changes::persist(&state.store.lock().unwrap(), session, crate::turn_changes::Captured { summary, details: paths.iter().map(detail).collect() }).unwrap();
}

/// A worker changed `src/a.rs`, then a passing round started; the lead's turn is still to be saved.
fn reviewed_worker_change() -> (AppState, String) {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[assignment("w", "fast", "src")]})).unwrap();
    let writer_session = saved(&state, &sid).workers[0].session_id.clone();
    let mut team = saved(&state, &sid);
    team.workers[0].status = "completed".into();
    store_team(&state, &mut team);
    persist_turn(&state, &writer_session, &["src/a.rs"]);
    std::thread::sleep(std::time::Duration::from_millis(3));
    let mut team = saved(&state, &sid);
    team.workers.push(seat("a", 1, Some(9.0), storage::now()));
    team.workers.push(seat("b", 1, Some(9.0), storage::now()));
    store_team(&state, &mut team);
    assert_eq!(finish_refusal(&saved(&state, &sid)), None);
    std::thread::sleep(std::time::Duration::from_millis(3));
    (state, sid)
}

#[test]
fn a_lead_turn_that_only_rolls_up_worker_changes_is_no_new_change() {
    let (state, sid) = reviewed_worker_change();
    persist_turn(&state, &sid, &[]);
    store_team(&state, &mut saved(&state, &sid));
    assert_eq!(finish_refusal(&saved(&state, &sid)), None);
}

#[test]
fn the_leads_own_edits_count_even_on_a_file_a_worker_changed() {
    let (state, sid) = reviewed_worker_change();
    // The lead's turn edits the worker's file and rolls the worker's change up with it.
    persist_turn(&state, &sid, &["src/a.rs"]);
    store_team(&state, &mut saved(&state, &sid));
    assert!(finish_refusal(&saved(&state, &sid)).unwrap().contains("out of date"));
}
