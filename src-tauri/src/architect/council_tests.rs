use super::*;
#[path = "council_flow_tests.rs"]
mod flow;
use crate::architect::council::review;
use council::{finish_refusal, parse_score, CouncilConfig, CouncilSeat};

const SINCE: i64 = 1000;


#[test]
fn post_acceptance_github_action_requires_one_exact_repository_and_user_grant() {
    let mut config = CouncilConfig {
        post_acceptance_issue: Some(council::PostAcceptanceIssue {
            authorization_id: "local-user-approval".into(),
            repository: "owner/repo-name".into(),
        }),
        ..Default::default()
    };
    assert!(config.validate().is_ok());
    config.post_acceptance_issue.as_mut().unwrap().repository = "owner/../other".into();
    assert!(config.validate().unwrap_err().contains("owner/name"));
    config.post_acceptance_issue.as_mut().unwrap().repository = "owner/repo-name".into();
    config.post_acceptance_issue.as_mut().unwrap().authorization_id.clear();
    assert!(config.validate().unwrap_err().contains("user-authorized"));
}

#[test]
fn optional_reviewer_skills_only_apply_to_explicit_unique_bot_assignments() {
    let mut council = on(9.0, 10, 0);
    council.optional_skill_ids = vec!["optional".into()];
    assert!(council.validate().unwrap_err().contains("selected bots"));
    council.reviewer_bot_ids = vec!["reviewer-one".into(), "reviewer-two".into()];
    assert!(council.validate().is_ok());
    council.optional_skill_ids.push("optional".into());
    assert!(council.validate().unwrap_err().contains("unique"));
    council.optional_skill_ids = vec!["../escaped".into()];
    assert!(council.validate().unwrap_err().contains("valid IDs"));
}


#[test]
fn mandatory_skill_check_without_native_evidence_cannot_pass_on_scores_alone() {
    let config = on(9.0, 10, 2);
    let mut team = team_with(config, vec![
        seat("reviewer-a", 1, Some(9.8), SINCE + 1),
        seat("reviewer-b", 1, Some(9.7), SINCE + 2),
    ]);
    assert_eq!(council::status(&team).unwrap().state, "passed",
        "ordinary Council without skill obligations keeps its native score gate");

    let worker = &team.workers[0];
    let mut binding = reviewer_security::ReviewerBinding::native(
        &team, &worker.id, worker.created_at, &worker.provider, &worker.model,
        worker.council.as_ref().unwrap(),
    );
    binding.snapshot.skills = Some(crate::skills::reviewer::ReviewerSkills {
        pins: vec![crate::skills::reviewer::SkillPin {
            id: "required-check".into(),
            source_kind: "folder".into(),
            source_location: "local".into(),
            source_ref: None,
            source_commit: None,
            content_sha256: "a".repeat(64),
            file_sha256: Default::default(),
            dependencies: vec![],
            required_for_acceptance: true,
            check_required: true,
            delivery_mode: "bounded_prompt".into(),
            staged_path: None,
            imported: true,
            bot_assigned: true,
            eligible: true,
            supplied: true,
            invoked: false,
            check_executed: false,
            evidence_verified: false,
        }],
        prompt: String::new(),
        skipped: vec![],
    });
    team.workers[0].reviewer_binding = Some(binding);

    assert_eq!(council::status(&team).unwrap().state, "fixing",
        "an unverified mandatory check must never create an automatic passed verdict");
    assert!(finish_refusal(&team).unwrap().contains("reviewer_skill_evidence_unverified"));
    assert_eq!(review(&team).unwrap()["passed"].as_bool(), Some(false));

    // A mandatory methodology skill need not imply a required executable check.
    team.workers[0].reviewer_binding.as_mut().unwrap().snapshot.skills.as_mut().unwrap()
        .pins[0].check_required = false;
    assert_eq!(council::status(&team).unwrap().state, "passed");
    assert!(finish_refusal(&team).is_none());
}

#[test]
fn native_skill_tools_observe_retrieval_and_integrity_without_model_self_reports() {
    let (state, sid) = state_fixture();
    let mut team = load(&state.store.lock().unwrap(), &sid).unwrap().unwrap();
    team.config.council = Some(on(9.0, 10, 2));
    team.council_since = SINCE;
    let mut reviewer = seat("native-seat", 1, Some(9.8), SINCE + 1);
    reviewer.status = "running".into();
    let mut binding = reviewer_security::ReviewerBinding::native(
        &team, &reviewer.id, reviewer.created_at, &reviewer.provider,
        &reviewer.model, reviewer.council.as_ref().unwrap(),
    );
    binding.snapshot.skills = Some(crate::skills::reviewer::ReviewerSkills {
        pins: vec![crate::skills::reviewer::SkillPin {
            id: "native-check".into(), source_kind: "folder".into(),
            source_location: "test".into(), source_ref: None, source_commit: None,
            content_sha256: "b".repeat(64), file_sha256: Default::default(),
            dependencies: vec![], required_for_acceptance: true, check_required: true,
            delivery_mode: "bounded_prompt".into(), staged_path: None,
            imported: true, bot_assigned: true, eligible: true, supplied: true,
            invoked: false, check_executed: false, evidence_verified: false,
        }],
        prompt: "Frozen reviewer skill methodology.".into(), skipped: vec![],
    });
    {
        let store = state.store.lock().unwrap();
        reviewer_security::record_native_authority(&store, &sid, &binding).unwrap();
    }
    reviewer.reviewer_binding = Some(binding);
    team.workers.push(reviewer);
    team.workers.push(seat("peer", 1, Some(9.7), SINCE + 2));
    {
        let store = state.store.lock().unwrap();
        save(&store, &mut team).unwrap();
    }
    let generation = team.generation;
    let actor = coordination::Actor::Worker { id: "native-seat".into(), attempt: 1 };
    let args = || json!({"skillId": "native-check"});
    assert!(reviewer_skill_evidence::dispatch_state(&state, &sid, generation,
        &actor, "pandacode_reviewer_skill_verify", args()).unwrap_err().contains("first open"));
    assert!(reviewer_skill_evidence::dispatch_state(&state, &sid, generation,
        &actor, "pandacode_reviewer_skill_open", json!({"skillId": "missing"}))
        .unwrap_err().contains("not supplied"));
    assert!(reviewer_skill_evidence::dispatch_state(&state, &sid, generation,
        &actor, "pandacode_reviewer_skill_open", json!({"skillId": "native-check", "verified": true}))
        .unwrap_err().contains("invalid arguments"));
    assert!(reviewer_skill_evidence::dispatch_state(&state, &sid, generation,
        &coordination::Actor::Architect, "pandacode_reviewer_skill_open", args()).is_err());
    let (opened, _) = reviewer_skill_evidence::dispatch_state(&state, &sid, generation,
        &actor, "pandacode_reviewer_skill_open", args()).unwrap();
    assert!(opened["lowerTrustInstructions"].as_str().unwrap().contains("Frozen reviewer"));
    assert_eq!(opened["nativeRetrievalRecorded"], true);
    let (checked, _) = reviewer_skill_evidence::dispatch_state(&state, &sid, generation,
        &actor, "pandacode_reviewer_skill_verify", args()).unwrap();
    assert_eq!(checked["pinnedInputIntegrityVerified"], true);
    let (_, duplicate) = reviewer_skill_evidence::dispatch_state(&state, &sid, generation,
        &actor, "pandacode_reviewer_skill_verify", args()).unwrap();
    assert!(duplicate.is_none(), "duplicate native calls are idempotent");
    let mut persisted = load(&state.store.lock().unwrap(), &sid).unwrap().unwrap();
    persisted.workers[0].status = "completed".into();
    {
        let store = state.store.lock().unwrap();
        save(&store, &mut persisted).unwrap();
    }
    assert_eq!(council::status(&persisted).unwrap().state, "passed");
    let evidence = persisted.workers[0].reviewer_binding.as_ref().unwrap()
        .skill_evidence.get("native-check").unwrap();
    assert!(evidence.invoked && evidence.check_executed && evidence.evidence_verified);
    persisted.workers[0].reviewer_binding.as_mut().unwrap().skill_evidence
        .get_mut("native-check").unwrap().content_sha256 = "stale".into();
    assert_eq!(council::status(&persisted).unwrap().state, "fixing");
}

#[test]
fn composed_root_authority_denies_unissued_tampered_and_revoked_seats() {
    use council::AutomaticReviewerCount as Count;
    let (state, sid) = state_fixture();
    let store = state.store.lock().unwrap();
    let plan = composition(true, Count::Fixed(2), vec![]);
    let config = config_for_composition(plan);
    let mut team = team_with(config, vec![]);
    team.session_id = sid.clone();

    for (id, score, created) in [
        ("recorded-root-a", 9.8, SINCE + 1),
        ("recorded-root-b", 9.7, SINCE + 2),
    ] {
        let mut worker = seat(id, 1, Some(score), created);
        let mut binding = reviewer_security::ReviewerBinding::native(
            &team, id, worker.created_at, &worker.provider, &worker.model,
            worker.council.as_ref().unwrap(),
        );
        binding.composition_mode = Some("direct".into());
        worker.reviewer_binding = Some(binding);
        team.workers.push(worker);
    }
    assert!(!composed_native_authority_current(&store, &team, 1),
        "scored in-memory automatic seats cannot approve without native grants");
    let first = team.workers[0].reviewer_binding.as_ref().unwrap();
    assert!(reviewer_security::validate_issued_authority(&store, &sid, first).is_ok(),
        "legacy read-only fallback remains available outside composition");
    assert!(reviewer_security::validate_recorded_authority(&store, &sid, first).is_err());
    reviewer_security::record_native_authority(&store, &sid, first).unwrap();
    assert!(!composed_native_authority_current(&store, &team, 1),
        "one grant cannot substitute for a second required reviewer");
    reviewer_security::record_native_authority(
        &store, &sid, team.workers[1].reviewer_binding.as_ref().unwrap(),
    ).unwrap();
    // Persist real reviewer transcripts; an issued grant without a current
    // native callback is never sufficient to authenticate a composed score.
    let project = store.session(&sid).unwrap().project_id;
    for reviewer in &team.workers {
        store.db.execute(
            "INSERT INTO sessions(id,project_id,title,provider,model,mode,status,created_at,updated_at,archived)
             VALUES(?1,?2,'Native reviewer',?3,?4,'plan','completed',1,1,0)",
            rusqlite::params![reviewer.session_id, project, reviewer.provider, reviewer.model],
        ).unwrap();
    }
    assert!(!composed_native_authority_current(&store, &team, 1),
        "native grants with no actual reviewer callback cannot approve a root");
    for reviewer in &team.workers {
        store.message(&reviewer.session_id, "assistant",
            &format!("Independent review.\nCouncil score: {}/10",
                reviewer.council.as_ref().unwrap().score.unwrap())
        ).unwrap();
    }
    assert!(composed_native_authority_current(&store, &team, 1),
        "both root scores must match their actual persisted native reports");
    let mut forged_score = team.clone();
    forged_score.workers[0].council.as_mut().unwrap().score = Some(10.0);
    assert!(!composed_native_authority_current(&store, &forged_score, 1),
        "a forged root score cannot borrow its earlier authorization");
    store.message(&team.workers[1].session_id, "assistant",
        "A later independent evaluation.\nCouncil score: 7/10").unwrap();
    assert!(!composed_native_authority_current(&store, &team, 1),
        "a changed latest native reviewer report invalidates a cached pass");
    store.message(&team.workers[1].session_id, "assistant",
        "Revalidated result.\nCouncil score: 9.7/10").unwrap();
    assert!(composed_native_authority_current(&store, &team, 1));

    let mut tampered = team.clone();
    tampered.workers[0].reviewer_binding.as_mut().unwrap()
        .artifact_revision.push_str("-forged");
    assert!(!composed_native_authority_current(&store, &tampered, 1));
    let changed = store.db.execute(
        "DELETE FROM architect_requests WHERE parent_session_id=?1 AND key=?2",
        rusqlite::params![sid, "native-reviewer-authority:recorded-root-b"],
    ).unwrap();
    assert_eq!(changed, 1);
    assert!(!composed_native_authority_current(&store, &team, 1),
        "revoked authority cannot qualify a completed seat");

    let mut legacy = team.clone();
    legacy.config.council.as_mut().unwrap().composition = None;
    assert!(composed_native_authority_current(&store, &legacy, 1),
        "the pre-composition M1/M2 Council retains its compatibility path");
}

#[test]
fn composed_terminal_receipt_selector_rejects_duplicates_and_replays() {
    assert_eq!(unique_council_receipt(&[1], |item| *item == 1), Some(&1));
    assert_eq!(unique_council_receipt(&[1, 2], |item| *item == 2), Some(&2));
    assert_eq!(unique_council_receipt(&[1, 2], |item| *item == 3), None);
    assert_eq!(unique_council_receipt(&[1, 1], |item| *item == 1), None,
        "two native records for one current seat cannot approve it");
    assert_eq!(unique_council_receipt(&[1, 2, 1], |item| *item == 1), None,
        "a replayed receipt remains ambiguous even with other seats present");
}

#[test]
fn composed_reviewer_tool_authority_denies_unissued_and_modified_bindings() {
    use council::AutomaticReviewerCount as Count;
    let (state, sid) = state_fixture();
    let store = state.store.lock().unwrap();
    let config = config_for_composition(composition(true, Count::Fixed(2), vec![]));
    let mut team = team_with(config, vec![]);
    team.session_id = sid.clone();
    let mut reviewer = seat("native-composed", 1, Some(9.8), SINCE + 1);
    let binding = reviewer_security::ReviewerBinding::native(
        &team, &reviewer.id, reviewer.created_at, &reviewer.provider,
        &reviewer.model, reviewer.council.as_ref().unwrap(),
    );
    reviewer.reviewer_binding = Some(binding.clone());
    team.workers.push(reviewer);

    assert!(validate_current_reviewer_authority(&store, &team, &binding)
        .unwrap_err().contains("capability_not_authorized"),
        "composed reviewer admission must require a stored native grant");
    reviewer_security::record_native_authority(&store, &sid, &binding).unwrap();
    assert!(validate_current_reviewer_authority(&store, &team, &binding).is_ok());

    let mut corrupted = binding.clone();
    corrupted.artifact_revision.push_str("-unissued");
    assert!(validate_current_reviewer_authority(&store, &team, &corrupted).is_err());

    let removed = store.db.execute(
        "DELETE FROM architect_requests WHERE parent_session_id=?1 AND key=?2",
        rusqlite::params![sid, "native-reviewer-authority:native-composed"],
    ).unwrap();
    assert_eq!(removed, 1);
    assert!(validate_current_reviewer_authority(&store, &team, &binding).is_err(),
        "a revoked grant must prevent a reviewer from resuming or reporting");

    team.config.council.as_mut().unwrap().composition = None;
    assert!(validate_current_reviewer_authority(&store, &team, &binding).is_ok(),
        "legacy read-only reviewer sessions keep their pre-composition behavior");
}

#[test]
fn mandatory_nested_leaf_budget_is_validated_before_any_dispatch() {
    use council::{AutomaticReviewerCount as Count, CouncilStrategy as Strategy};
    let mut plan = composition(false, Count::Fixed(0), vec![
        nested_reviewer("nested-one"), nested_reviewer("nested-two"),
    ]);
    plan.limits.max_leaf_reviews_per_round = 3;
    assert!(matches!(plan.validate(), Err(council::CouncilCompositionError::InvalidLimit(_))),
        "two mandatory nested Councils each need two leaf reviewers");
    plan.limits.max_leaf_reviews_per_round = 4;
    assert!(plan.validate().is_ok(), "exact minimum is schedulable");
    plan.strategy = Strategy::TwoPhase;
    plan.limits.max_leaf_reviews_per_round = 7;
    assert!(matches!(plan.validate(), Err(council::CouncilCompositionError::InvalidLimit(_))),
        "each mandatory child needs fresh phase-two leaf challengers");
    plan.limits.max_leaf_reviews_per_round = 8;
    assert!(plan.validate().is_ok(), "both native phases fit exactly");

    for selected in &mut plan.selected {
        selected.nested.as_mut().unwrap().escalation = council::NestedEscalation::OnTrigger;
    }
    plan.limits.max_leaf_reviews_per_round = 1;
    assert!(plan.validate().is_ok(),
        "on-trigger children do not consume mandatory leaf budget until escalation");
}

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
fn resumed_selected_seat_fails_closed_if_bot_was_paused_or_removed() {
    let (store, mut team) = fixture();
    let project = store.session(&team.session_id).unwrap().project_id;
    store.db.execute(
        "INSERT INTO bots(id,name,role,project_id,provider,model,mode,created_at,updated_at)
         VALUES('bot-a','Reviewer','QA',?1,'codex','frozen-model','full',1,1)",
        [&project],
    ).unwrap();
    let mut worker = seat("resume-reviewer", 1, None, SINCE + 1);
    worker.status = "running".into();
    team.council_since = SINCE;
    team.config.council = Some(on(9.0, 10, 2));
    let mut binding = reviewer_security::ReviewerBinding::native(
        &team, &worker.id, worker.created_at, &worker.provider,
        &worker.model, worker.council.as_ref().unwrap(),
    );
    binding.snapshot.provider = "codex".into();
    binding.snapshot.model = "frozen-model".into();
    binding.reviewer_bot_id = Some("bot-a".into());
    binding.snapshot.bot_context = Some(
        reviewer_security::ReviewerBotContext::from_bot(&crate::bots::get(&store, "bot-a").unwrap()),
    );
    assert!(bot_reviewers::validate_frozen_seat(&store, &binding, false).is_ok());
    store.db.execute(
        "UPDATE bots SET name='Changed',instructions='New instructions',model='new-model'
         WHERE id='bot-a'",
        [],
    ).unwrap();
    assert!(bot_reviewers::validate_frozen_seat(&store, &binding, false).is_ok(),
        "profile edits must not rewrite or invalidate an already frozen seat");
    assert_eq!(binding.snapshot.model, "frozen-model");
    assert_eq!(binding.snapshot.bot_context.as_ref().unwrap().name, "Reviewer");
    store.db.execute("UPDATE bots SET paused=1 WHERE id='bot-a'", []).unwrap();
    assert!(bot_reviewers::validate_frozen_seat(&store, &binding, false)
        .unwrap_err().contains("paused"));
    store.db.execute("DELETE FROM bots WHERE id='bot-a'", []).unwrap();
    assert!(bot_reviewers::validate_frozen_seat(&store, &binding, false)
        .unwrap_err().contains("removed"));
}

#[test]
fn selected_council_failover_never_changes_its_frozen_provider_or_permissions() {
    let (state, sid, _run) = failover_state("gpt-5", &["codex", "cline"]);
    let store = state.store.lock().unwrap();
    let mut team = load(&store, &sid).unwrap().unwrap();
    team.workers[0].status = "failed".into();
    team.workers[0].council = Some(CouncilSeat {
        round: 1, lens: "security".into(), score: None, started_at: 1, closed_by_selection: false, ..CouncilSeat::default()
    });
    let worker = &team.workers[0];
    let mut binding = reviewer_security::ReviewerBinding::native(
        &team, &worker.id, worker.created_at, &worker.provider,
        &worker.model, worker.council.as_ref().unwrap(),
    );
    binding.reviewer_bot_id = Some("selected-bot".into());
    binding.snapshot.bot_context = Some(reviewer_security::ReviewerBotContext {
        name: "Selected".into(), role: "QA".into(), instructions: "Review".into(),
        goals: String::new(), soul: String::new(), reasoning_effort: String::new(),
        skill_ids: Vec::new(),
    });
    team.workers[0].reviewer_binding = Some(binding.clone());
    let before = (team.workers[0].provider.clone(), team.workers[0].model.clone(),
        team.workers[0].session_id.clone());
    assert!(!fail_over(&store, &mut team, 0, "provider failure").unwrap(),
        "a cross-harness fallback cannot inherit a selected seat's tool authority");
    assert_eq!((team.workers[0].provider.clone(),team.workers[0].model.clone(),
        team.workers[0].session_id.clone()), before);
    assert_eq!(team.workers[0].reviewer_binding.as_ref(),Some(&binding));
    assert_eq!(team.workers[0].status, "failed");
    assert!(team.workers[0].error.as_deref().unwrap().contains("policy_unsupported"));
    assert_eq!(store.session(&before.2).unwrap().provider, before.0,
        "the native session must not be silently changed on an unsupported fallback");
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
fn selected_reviewer_skills_are_frozen_with_native_round_without_global_activation() {
    use crate::skills::{Kind, Library, Source};
    let (store, mut team) = fixture();
    let root = std::env::temp_dir().join(format!("council-s3-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(root.join("skills")).unwrap();
    let library = Library::new(root.join("skills"), root.clone());
    let imported = library.import(
        Kind::Skill,
        vec![("SKILL.md".into(), b"Use a source-linked checklist for this review.".to_vec())],
        "review-checklist",
        Source { kind: "folder".into(), location: "review-checklist".into(), git_ref: None, commit: None },
    ).unwrap();
    let skill_id = imported.meta.id;
    let project = store.session(&team.session_id).unwrap().project_id;
    for id in ["bot-a", "bot-b"] {
        store.db.execute(
            "INSERT INTO bots(id,name,role,instructions,goals,soul,project_id,provider,model,mode,created_at,updated_at,skills) VALUES(?1,?1,'Reviewer','Inspect sources','','',?2,'codex','frozen-model','full',1,1,?3)",
            params![id, project, serde_json::json!([skill_id.clone()]).to_string()],
        ).unwrap();
    }
    let mut config = on(9.0, 10, 0);
    config.reviewer_bot_ids = vec!["bot-a".into(), "bot-b".into()];
    team.config.council = Some(config);
    team.council_since = SINCE;
    let first = bot_reviewers::seat_bot_inner_with_skills(&store, &team, 1, None, &library).unwrap().unwrap();
    let initial = first.skills.as_ref().unwrap();
    assert_eq!(initial.pins.len(), 1);
    assert_eq!(initial.pins[0].id, skill_id);
    assert_eq!(initial.pins[0].delivery_mode, "isolated_files");
    assert!(initial.prompt.contains("source-linked checklist"));
    assert!(initial.verify().is_ok());
    assert!(library.list().unwrap().iter().all(|entry| entry.meta.activations.is_empty()));

    let worker = seat("first", 1, None, SINCE + 1);
    let mut binding = reviewer_security::ReviewerBinding::native(
        &team, &worker.id, worker.created_at, &worker.provider, &worker.model, worker.council.as_ref().unwrap(),
    );
    binding.reviewer_bot_id = Some(first.id.clone());
    binding.roster_bot_ids = first.roster_bot_ids.clone();
    binding.required_reviewers = Some(first.roster_bot_ids.len());
    binding.snapshot.bot_context = Some(first.context.clone());
    binding.snapshot.tool_policy = first.tool_policy.clone();
    binding.snapshot.skills = first.skills.clone();
    reviewer_security::record_native_authority(&store, &team.session_id, &binding).unwrap();
    let mut worker = worker;
    worker.reviewer_binding = Some(binding);
    team.workers.push(worker);
    let issued = team.workers[0].reviewer_binding.as_ref().unwrap().clone();
    assert!(reviewer_security::validate_issued_authority(&store, &team.session_id, &issued).is_ok());
    let mut tampered = issued;
    tampered.snapshot.skills.as_mut().unwrap().prompt.push_str("\nForged instruction");
    assert!(reviewer_security::validate_issued_authority(&store, &team.session_id, &tampered)
        .unwrap_err().contains("reviewer_session_mismatch"));

    // A library replacement between separately dispatched seats cannot alter this round.
    library.import(
        Kind::Skill,
        vec![("SKILL.md".into(), b"This new revision must wait for the next Council round.".to_vec())],
        "review-checklist",
        Source { kind: "folder".into(), location: "review-checklist".into(), git_ref: None, commit: None },
    ).unwrap();
    let second = bot_reviewers::seat_bot_inner_with_skills(&store, &team, 1, None, &library).unwrap().unwrap();
    assert_eq!(second.id, "bot-b");
    let second_skills = second.skills.as_ref().unwrap();
    assert_eq!(second_skills.pins[0].content_sha256, initial.pins[0].content_sha256,
        "an in-flight Council must retain its original pinned package digest");
    assert_eq!(second_skills.pins[0].file_sha256, initial.pins[0].file_sha256,
        "a replacement in the central library must not alter the frozen file set");
    assert_ne!(second_skills.pins[0].staged_path, initial.pins[0].staged_path,
        "distinct reviewers must not share an immutable staging directory");
    assert!(second_skills.verify().is_ok());
    assert!(second_skills.prompt.contains("source-linked checklist"));
    assert!(!second.skills.as_ref().unwrap().prompt.contains("new revision"));
    assert!(library.list().unwrap().iter().all(|entry| entry.meta.activations.is_empty()));
    let staged = initial.pins[0].staged_path.as_ref().unwrap();
    let file = std::path::Path::new(staged).join("SKILL.md");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(windows)]
    crate::skills::reviewer::clear_staged_readonly(&file).unwrap();
    std::fs::remove_dir_all(root).unwrap();
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
    reviewer.council = Some(CouncilSeat { round, lens: format!("lens {id}"), score, started_at: created, closed_by_selection: false, ..CouncilSeat::default() });
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
    assert_eq!(new.council, Some(CouncilSeat { round: 1, lens: "lens b".into(), score: None, started_at: 0, closed_by_selection: false, ..CouncilSeat::default() }));
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
    let refused = dispatch_as(&state, &sid, team.generation, Some((&id, attempt)), "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "ux")]}), &AtomicBool::new(false), &routing::AutoRoutes::default(), None).unwrap_err();
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

fn composition_limits() -> council::CouncilCompositionLimits {
    council::CouncilCompositionLimits {
        max_depth: 2,
        max_root_seats: 8,
        max_concurrent_leaves: 4,
        max_leaf_reviews_per_round: 32,
        timeout_ms: 3_600_000,
        max_estimated_cost_micros: Some(10_000_000),
    }
}

fn direct_reviewer(bot_id: &str) -> council::SelectedCouncilReviewer {
    council::SelectedCouncilReviewer { bot_id: bot_id.into(), mode: council::SelectedReviewerMode::Direct, required: true, nested: None }
}

fn nested_reviewer(bot_id: &str) -> council::SelectedCouncilReviewer {
    council::SelectedCouncilReviewer {
        bot_id: bot_id.into(),
        mode: council::SelectedReviewerMode::Nested,
        required: true,
        nested: Some(council::NestedCouncilConfig {
            template: council::NestedCouncilTemplate::ValidatedDefault,
            child_criteria_source: council::CouncilCriteriaSource::Architect,
            child_reviewer_count: 2,
            target: 9.0,
            max_rounds: 5,
            escalation: council::NestedEscalation::Always,
            user_forced: false,
        }),
    }
}

fn on_trigger_fixture(
    score: f64,
) -> (
    TeamSnapshot,
    council::CouncilCriterionOutcomeReceipt,
    council::CouncilCriterionOutcomeReceipt,
) {
    use council::{AutomaticReviewerCount as Count, CouncilCriterionOrigin as Origin, CouncilCriteriaSource as Source};
    let mut reviewer = nested_reviewer("nested-on-trigger");
    reviewer.nested.as_mut().unwrap().escalation = council::NestedEscalation::OnTrigger;
    let mut plan = composition(true, Count::Fixed(1), vec![reviewer]);
    plan.criteria_source = Source::Architect;
    plan.criteria.push(council::CouncilCriterion {
        id: "security".into(),
        lens: "security".into(),
        description: "Evaluate security evidence.".into(),
        required: true,
        target: 9.0,
        evidence_required: true,
        source: Origin::Architect,
    });
    let config = config_for_composition(plan);
    let mut automatic = seat("on-trigger-automatic", 1, Some(9.5), SINCE + 2);
    automatic.council.as_mut().unwrap().lens = "correctness".into();
    let mut team = team_with(config, vec![seat("on-trigger-seat", 1, Some(score), SINCE + 1), automatic]);
    let worker = &team.workers[0];
    let council_seat = worker.council.as_ref().unwrap().clone();
    let mut binding = reviewer_security::ReviewerBinding::native(
        &team, &worker.id, worker.created_at, &worker.provider, &worker.model, &council_seat,
    );
    binding.reviewer_bot_id = Some("nested-on-trigger".into());
    binding.roster_bot_ids = vec!["nested-on-trigger".into()];
    binding.required_reviewers = Some(1);
    binding.composition_mode = Some("nested".into());
    team.workers[0].reviewer_binding = Some(binding);
    let automatic_worker = &team.workers[1];
    let automatic_seat = automatic_worker.council.as_ref().unwrap().clone();
    let mut automatic_binding = reviewer_security::ReviewerBinding::native(
        &team,
        &automatic_worker.id,
        automatic_worker.created_at,
        &automatic_worker.provider,
        &automatic_worker.model,
        &automatic_seat,
    );
    automatic_binding.required_reviewers = Some(2);
    automatic_binding.composition_mode = Some("direct".into());
    team.workers[1].reviewer_binding = Some(automatic_binding);
    let worker = &team.workers[0];
    let cycle_id = format!("{}:{}", team.session_id, team.council_since);
    let make_receipt = |reviewer: &Worker, score: f64, evidence: &str| {
        let binding = reviewer.reviewer_binding.as_ref().unwrap();
        let receipt = council::CouncilCriterionOutcomeReceipt {
            schema_version: 1,
            root_cycle_id: cycle_id.clone(),
            task_id: team.session_id.clone(),
            task_cycle_id: cycle_id.clone(),
            child_cycle_id: None,
            round: 1,
            seat_id: reviewer.id.clone(),
            parent_seat_id: None,
            attempt: reviewer.attempt,
            artifact_revision: binding.artifact_revision.clone(),
            outcome: council::CouncilCriterionOutcome {
                criterion_id: "security".into(),
                score,
                verified_evidence_refs: vec![evidence.into()],
            },
        };
        receipt.validate().unwrap();
        receipt
    };
    let receipt = make_receipt(worker, 9.5, "verified:security");
    let peer_receipt = make_receipt(&team.workers[1], 9.5, "verified:automatic-security");
    (team, receipt, peer_receipt)
}

fn trigger_finding(
    team: &TeamSnapshot,
    severity: council::CouncilFindingSeverity,
) -> council::CouncilFinding {
    let worker = &team.workers[0];
    let binding = worker.reviewer_binding.as_ref().unwrap();
    let cycle_id = format!("{}:{}", team.session_id, team.council_since);
    council::CouncilFinding {
        id: format!("finding-{:?}", severity).to_ascii_lowercase(),
        content_digest: String::new(),
        cycle_id: cycle_id.clone(),
        round_id: format!("{cycle_id}:1"),
        seat_id: worker.id.clone(),
        artifact_digest: council_artifact_digest(&binding.artifact_revision),
        criterion_id: "security".into(),
        severity,
        category: "security".into(),
        summary: "A verified review finding.".into(),
        title: None, explanation: None, evidence_refs: vec!["finding-evidence".into()],
        location_status: council::CouncilFindingLocationStatus::Missing,
        location: None,
        suggested_fix: None,
    }
}

#[test]
fn on_trigger_selects_every_issue_trigger_and_only_complete_preliminary_passes() {
    use council::{CouncilFindingSeverity as Severity, NestedEscalationTrigger as Trigger};
    let (team, mut outcome, peer_outcome) = on_trigger_fixture(9.5);
    let worker = &team.workers[0];
    assert_eq!(
        council::nested_preliminary_trigger(&team, worker, &[outcome.clone(), peer_outcome.clone()], &[], false),
        None,
        "a passing preliminary result with required evidence should avoid child escalation",
    );
    assert_eq!(
        council::status_with_criterion_outcomes(&team, &[outcome.clone(), peer_outcome.clone()]).unwrap().state,
        "passed",
        "the no-trigger direct result counts only with its pinned criterion and verified evidence: {:?}",
        council::finish_refusal_with_criterion_outcomes(&team, &[outcome.clone(), peer_outcome.clone()]),
    );

    outcome.outcome.score = 8.5;
    assert_eq!(
        council::nested_preliminary_trigger(&team, worker, &[outcome.clone()], &[], false),
        Some(Trigger::BelowTarget),
    );

    outcome.outcome.score = 9.5;
    for (severity, expected) in [
        (Severity::Critical, Trigger::CriticalFinding),
        (Severity::High, Trigger::HighFinding),
    ] {
        assert_eq!(
            council::nested_preliminary_trigger(&team, worker, &[outcome.clone()], &[trigger_finding(&team, severity)], false),
            Some(expected),
        );
    }

    let mut missing_evidence = outcome.clone();
    missing_evidence.outcome.verified_evidence_refs.clear();
    assert_eq!(
        council::nested_preliminary_trigger(&team, worker, &[missing_evidence.clone()], &[], false),
        Some(Trigger::MissingMandatoryEvidence),
    );
    assert_ne!(
        council::status_with_criterion_outcomes(&team, &[missing_evidence, peer_outcome]).unwrap().state,
        "passed",
        "the no-trigger path must retain the native evidence gate",
    );

    let cycle_id = format!("{}:{}", team.session_id, team.council_since);
    let binding = worker.reviewer_binding.as_ref().unwrap();
    let conflicting = council::CouncilCriterionOutcomeReceipt {
        schema_version: 1,
        root_cycle_id: cycle_id.clone(),
        task_id: team.session_id.clone(),
        task_cycle_id: cycle_id,
        child_cycle_id: None,
        round: 1,
        seat_id: "other-seat".into(),
        parent_seat_id: None,
        attempt: 1,
        artifact_revision: binding.artifact_revision.clone(),
        outcome: council::CouncilCriterionOutcome {
            criterion_id: "security".into(),
            score: 8.0,
            verified_evidence_refs: vec!["verified:other-security".into()],
        },
    };
    assert_eq!(
        council::nested_preliminary_trigger(&team, worker, &[outcome.clone(), conflicting], &[], false),
        Some(Trigger::SameCriterionConflict),
    );
    assert_eq!(
        council::nested_preliminary_trigger(&team, worker, &[], &[], true),
        Some(Trigger::UserForced),
    );
}

#[test]
fn s2_candidate_ranking_uses_only_local_prices_and_one_output_cap() {
    let prompt = "Frozen Council assignment with a pinned review criterion.";
    let profile = |id: &str, model: &str| WorkerProfile {
        id: id.into(),
        name: id.into(),
        provider: "codex".into(),
        model: model.into(),
        reasoning_effort: None,
    };
    let nano = crate::usage_ledger::estimate_council_review_cost("gpt-5-nano", prompt).unwrap();
    let pro = crate::usage_ledger::estimate_council_review_cost("gpt-5-pro", prompt).unwrap();
    assert!(nano.amount_micros < pro.amount_micros);
    assert_eq!(nano.output_tokens, crate::usage_ledger::COUNCIL_REVIEW_OUTPUT_TOKEN_CAP);
    assert_eq!(nano.pricing_source, "panda-local-bundled-price-cards");
    assert!(nano.pricing_version.starts_with("sha256:"));

    let ranked = rank_s2_candidates(
        vec![
            profile("unpriced", "no-local-council-price-card"),
            profile("pro", "gpt-5-pro"),
            profile("nano", "gpt-5-nano"),
        ],
        prompt,
    );
    assert_eq!(ranked.iter().map(|(profile, _)| profile.id.as_str()).collect::<Vec<_>>(), vec!["nano", "pro"]);
    assert!(ranked.windows(2).all(|pair| pair[0].1.amount_micros <= pair[1].1.amount_micros));
    assert!(rank_s2_candidates(vec![profile("unpriced-only", "no-local-council-price-card")], prompt).is_empty());
}

fn composition(
    automatic_enabled: bool,
    automatic_count: council::AutomaticReviewerCount,
    selected: Vec<council::SelectedCouncilReviewer>,
) -> council::CouncilComposition {
    council::CouncilComposition {
        schema_version: 1,
        automatic: council::CouncilAutomaticReviewers { enabled: automatic_enabled, count: automatic_count },
        selected,
        criteria_source: council::CouncilCriteriaSource::Architect,
        criteria: Vec::new(),
        strategy: council::CouncilStrategy::Parallel,
        limits: composition_limits(),
    }
}

fn config_for_composition(composition: council::CouncilComposition) -> CouncilConfig {
    let reviewer_bot_ids = composition.selected.iter().map(|reviewer| reviewer.bot_id.clone()).collect();
    let reviewers = if composition.automatic.enabled {
        match composition.automatic.count { council::AutomaticReviewerCount::Fixed(count) => count, council::AutomaticReviewerCount::Auto(_) => 0 }
    } else {
        0
    };
    CouncilConfig { reviewer_bot_ids, reviewers, composition: Some(composition), ..on(9.0, 5, 0) }
}

// Pure snapshot fixtures cover native gate logic; runtime dispatch coverage lives in
// `architect/tests.rs` and exercises persisted root and child task allocation.

#[test]
fn composed_lead_rules_keep_nested_bot_quorum_and_native_source_quotas_explicit() {
    use council::AutomaticReviewerCount as Count;
    let modes = [
        ("M4", 0, 0, 2, composition(false, Count::Fixed(0), vec![nested_reviewer("n-a"), nested_reviewer("n-b")])),
        ("M5", 1, 0, 1, composition(true, Count::Fixed(1), vec![nested_reviewer("n-a")])),
        ("M6", 0, 1, 1, composition(false, Count::Fixed(0), vec![direct_reviewer("d-a"), nested_reviewer("n-a")])),
        ("M7", 1, 1, 1, composition(true, Count::Fixed(1), vec![direct_reviewer("d-a"), nested_reviewer("n-a")])),
    ];
    for (mode, automatic, direct, nested, composition) in modes {
        let config = config_for_composition(composition);
        let team = team_with(config.clone(), vec![]);
        let rules = council::lead_rules(&team, false);
        let count = format!("{nested} are task-owned child Councils and {direct} are direct reviewers; {automatic} additional automatic root reviewers");
        assert!(rules.contains(&count), "{mode}: {rules}");
        assert!(rules.contains("Each selected nested bot occupies exactly one root seat"), "{mode}");
        assert!(rules.contains("Never dispatch child leaves as extra root votes"), "{mode}");
        assert!(rules.contains("native child terminal receipt is missing"), "{mode}");
        assert_eq!(config.seat_plan().required(), automatic + direct + nested);
    }
    let legacy = team_with(on(9.0, 5, 0), vec![]);
    assert!(!council::lead_rules(&legacy, false).contains("task-owned child Councils"),
        "legacy M1 is unchanged");
}

#[test]
fn synthetic_composition_gate_fixtures_cover_modes_and_strategies() {
    use council::{AutomaticReviewerCount as Count, CouncilCompositionMode as Mode};
    let cases = vec![
        (composition(true, Count::Auto(council::AutomaticReviewerCountAuto::Auto), vec![]), Mode::Architect, "M1", 2, 2, 0, 0),
        (composition(false, Count::Fixed(0), vec![direct_reviewer("direct-a"), direct_reviewer("direct-b")]), Mode::Bots, "M2", 2, 0, 2, 0),
        (composition(true, Count::Fixed(1), vec![direct_reviewer("direct-a")]), Mode::BotsAndArchitect, "M3", 2, 1, 1, 0),
        (composition(false, Count::Fixed(0), vec![nested_reviewer("nested-a"), nested_reviewer("nested-b")]), Mode::BotsWithOwnArchitect, "M4", 2, 0, 0, 2),
        (composition(true, Count::Fixed(1), vec![nested_reviewer("nested-a")]), Mode::BotsWithOwnArchitectAndArchitect, "M5", 2, 1, 0, 1),
        (composition(false, Count::Fixed(0), vec![direct_reviewer("direct-a"), nested_reviewer("nested-a")]), Mode::MixedBots, "M6", 2, 0, 1, 1),
        (composition(true, Count::Fixed(1), vec![direct_reviewer("direct-a"), nested_reviewer("nested-a")]), Mode::MixedBotsAndArchitect, "M7", 3, 1, 1, 1),
    ];
    for (composition, expected_mode, expected_id, expected_seats, automatic, direct, nested) in cases {
        let config = config_for_composition(composition);
        assert_eq!(config.validate_composition().unwrap(), Some(expected_mode));
        assert_eq!(expected_mode.id(), expected_id);
        assert_eq!(config.needed(), expected_seats, "{} must not get implicit seats", expected_mode.id());
        assert_eq!(config.seat_plan(), council::CouncilSeatPlan { automatic, direct_bots: direct, nested_bots: nested }, "{} source plan", expected_mode.id());

        let capabilities = council::CouncilCompositionCapabilities {
            automatic_reviewers: true,
            supported_strategies: vec![
                council::CouncilStrategy::Parallel,
                council::CouncilStrategy::Escalated,
                council::CouncilStrategy::TwoPhase,
            ],
            reviewers: config.composition.as_ref().unwrap().selected.iter().map(|reviewer| {
                council::CouncilReviewerCapability {
                    bot_id: reviewer.bot_id.clone(),
                    direct: true,
                    nested: true,
                }
            }).collect(),
        };
        for strategy in [council::CouncilStrategy::Parallel, council::CouncilStrategy::Escalated, council::CouncilStrategy::TwoPhase] {
            let mut candidate = config.clone();
            candidate.composition.as_mut().unwrap().strategy = strategy;
            assert!(candidate.validate_for_dispatch(None, &capabilities).is_ok(), "{} × {strategy:?}", expected_id);
            assert_eq!(candidate.validate_composition().unwrap(), Some(expected_mode));
            if strategy == council::CouncilStrategy::TwoPhase {
                let (phase1, phase1_receipts) = executable_composition_fixture(candidate.clone(), 1);
                assert_eq!(council::status_with_criterion_outcomes(&phase1, &phase1_receipts).unwrap().state, "dispatch",
                    "{expected_id} × TwoPhase must request its challenger phase");
                let (challenged, receipts) = executable_composition_fixture(candidate, 2);
                assert_eq!(council::status_with_criterion_outcomes(&challenged, &receipts).unwrap().state, "passed",
                    "{expected_id} × TwoPhase must pass only after independent challenged seats execute");
            } else {
                let (executed, receipts) = executable_composition_fixture(candidate, 1);
                assert_eq!(council::status_with_criterion_outcomes(&executed, &receipts).unwrap().state, "passed",
                    "{expected_id} × {strategy:?} must execute its score and evidence gates");
            }
        }
    }
}


#[test]
fn all_seven_compositions_and_three_strategies_reject_unverified_terminal_counterpaths() {
    use council::AutomaticReviewerCount as Count;
    let cases = [
        ("M1", composition(true, Count::Auto(council::AutomaticReviewerCountAuto::Auto), vec![])),
        ("M2", composition(false, Count::Fixed(0), vec![direct_reviewer("d-a"), direct_reviewer("d-b")])),
        ("M3", composition(true, Count::Fixed(1), vec![direct_reviewer("d-a")])),
        ("M4", composition(false, Count::Fixed(0), vec![nested_reviewer("n-a"), nested_reviewer("n-b")])),
        ("M5", composition(true, Count::Fixed(1), vec![nested_reviewer("n-a")])),
        ("M6", composition(false, Count::Fixed(0), vec![direct_reviewer("d-a"), nested_reviewer("n-a")])),
        ("M7", composition(true, Count::Fixed(1), vec![direct_reviewer("d-a"), nested_reviewer("n-a")])),
    ];
    for (mode, plan) in cases {
        for strategy in [
            council::CouncilStrategy::Parallel,
            council::CouncilStrategy::Escalated,
            council::CouncilStrategy::TwoPhase,
        ] {
            let mut plan = plan.clone();
            plan.strategy = strategy;
            let config = config_for_composition(plan);
            let rounds = if strategy == council::CouncilStrategy::TwoPhase { 2 } else { 1 };
            let (team, receipts) = executable_composition_fixture(config.clone(), rounds);
            let status = |snapshot: &TeamSnapshot, evidence: &[council::CouncilCriterionOutcomeReceipt]| {
                council::status_with_criterion_outcomes(snapshot, evidence).unwrap().state
            };
            assert_eq!(status(&team, &receipts), "passed",
                "{mode} x {strategy:?} positive fixture must reach its native gate");

            let mut absent_result = team.clone();
            let worker = absent_result.workers.last_mut().unwrap();
            worker.council.as_mut().unwrap().score = None;
            worker.status = "failed".into();
            assert_ne!(status(&absent_result, &receipts), "passed",
                "{mode} x {strategy:?}: absent last-seat score cannot be averaged away");

            let mut below_target = team.clone();
            below_target.workers.last_mut().unwrap().council.as_mut().unwrap().score = Some(8.0);
            assert_ne!(status(&below_target, &receipts), "passed",
                "{mode} x {strategy:?}: below-target seat blocks the entire root");

            assert_ne!(status(&team, &[]), "passed",
                "{mode} x {strategy:?}: numeric scores without required criterion receipts are not approval");

            let mut mismatched_receipts = receipts.clone();
            for receipt in &mut mismatched_receipts {
                receipt.root_cycle_id = "stale-root:old-cycle".into();
            }
            assert_ne!(status(&team, &mismatched_receipts), "passed",
                "{mode} x {strategy:?}: stale-cycle evidence cannot validate current reviewers");

            let mut duplicate_session = team.clone();
            duplicate_session.workers.last_mut().unwrap().session_id = team.workers[0].session_id.clone();
            assert_ne!(status(&duplicate_session, &receipts), "passed",
                "{mode} x {strategy:?}: a reused native reviewer session cannot provide independent approval");

            if strategy == council::CouncilStrategy::TwoPhase {
                let (only_first_phase, phase1_receipts) =
                    executable_composition_fixture(config, 1);
                assert_ne!(status(&only_first_phase, &phase1_receipts), "passed",
                    "{mode} x {strategy:?}: first-phase approval cannot bypass independent challengers");
            }
        }
    }
}

fn executable_composition_fixture(
    mut config: CouncilConfig,
    rounds: usize,
) -> (TeamSnapshot, Vec<council::CouncilCriterionOutcomeReceipt>) {
    use council::{CouncilCriterionOrigin as Origin, CouncilCriteriaSource as Source, SelectedReviewerMode as Mode};
    let composition = config.composition.as_mut().unwrap();
    composition.criteria_source = Source::Architect;
    composition.criteria.push(council::CouncilCriterion {
        id: "coverage".into(),
        lens: "coverage".into(),
        description: "Verify the frozen task evidence.".into(),
        required: true,
        target: 9.0,
        evidence_required: true,
        source: Origin::Architect,
    });
    config.validate_composition().unwrap();

    #[derive(Clone)]
    struct SourceSeat {
        bot_id: Option<String>,
        mode: Option<Mode>,
        model: String,
    }
    let plan = config.seat_plan();
    let selected = config.composition.as_ref().unwrap().selected.clone();
    let mut sources = Vec::new();
    for index in 0..plan.automatic {
        sources.push(SourceSeat {
            bot_id: None,
            mode: None,
            model: format!("automatic-source-{index}"),
        });
    }
    for reviewer in selected {
        sources.push(SourceSeat {
            bot_id: Some(reviewer.bot_id),
            mode: Some(reviewer.mode),
            model: "selected-reviewer-model".into(),
        });
    }
    assert_eq!(sources.len(), config.needed(), "the executable fixture covers the full source plan");
    let roster = config.composition.as_ref().unwrap().selected.iter()
        .map(|reviewer| reviewer.bot_id.clone()).collect::<Vec<_>>();
    let (_, mut team) = fixture();
    team.config.council = Some(config.clone());
    team.council_since = SINCE;
    for round in 1..=rounds {
        for lens_index in 0..sources.len() {
            let source_index = if round == 1 { lens_index } else { (lens_index + 1) % sources.len() };
            let source = &sources[source_index];
            let id = format!("matrix-r{round}-s{lens_index}");
            let mut reviewer = seat(&id, round, Some(9.7), SINCE + (round * 10 + lens_index) as i64);
            reviewer.council.as_mut().unwrap().lens = format!("lens-{lens_index}");
            reviewer.profile_id = source.bot_id.as_ref().map_or_else(
                || format!("profile-{}", source.model),
                |bot_id| format!("bot-{bot_id}"),
            );
            reviewer.model = source.model.clone();
            reviewer.provider = "codex".into();
            let council_seat = reviewer.council.as_ref().unwrap().clone();
            let mut binding = reviewer_security::ReviewerBinding::native(
                &team, &reviewer.id, reviewer.created_at, &reviewer.provider, &reviewer.model, &council_seat,
            );
            binding.required_reviewers = Some(if source.bot_id.is_some() { roster.len() } else { config.needed() });
            if let Some(bot_id) = &source.bot_id {
                binding.reviewer_bot_id = Some(bot_id.clone());
                binding.roster_bot_ids = roster.clone();
                binding.composition_mode = Some(match source.mode.unwrap() {
                    Mode::Direct => "direct".into(),
                    Mode::Nested => "nested".into(),
                });
            } else if config.composition.is_some() {
                binding.composition_mode = Some("direct".into());
            }
            reviewer.reviewer_binding = Some(binding);
            team.workers.push(reviewer);
        }
    }

    let cycle_id = format!("{}:{}", team.session_id, team.council_since);
    let mut outcomes = Vec::new();
    for reviewer in &team.workers {
        let seat = reviewer.council.as_ref().unwrap();
        let binding = reviewer.reviewer_binding.as_ref().unwrap();
        let nested = binding.composition_mode.as_deref() == Some("nested");
        let child_task_id = format!("child-{}", reviewer.id);
        let task_cycle_id = if nested { format!("{child_task_id}:{}", seat.round) } else { cycle_id.clone() };
        let receipt = council::CouncilCriterionOutcomeReceipt {
            schema_version: 1,
            root_cycle_id: cycle_id.clone(),
            task_id: if nested { child_task_id } else { team.session_id.clone() },
            task_cycle_id: task_cycle_id.clone(),
            child_cycle_id: nested.then_some(task_cycle_id),
            round: seat.round,
            seat_id: if nested { format!("leaf-{}", reviewer.id) } else { reviewer.id.clone() },
            parent_seat_id: nested.then(|| reviewer.id.clone()),
            attempt: reviewer.attempt,
            artifact_revision: binding.artifact_revision.clone(),
            outcome: council::CouncilCriterionOutcome {
                criterion_id: "coverage".into(),
                score: 9.6,
                verified_evidence_refs: vec![format!("verified:{}", reviewer.id)],
            },
        };
        receipt.validate().unwrap();
        outcomes.push(receipt);
    }
    (team, outcomes)
}

fn two_phase_fixture(with_challengers: bool) -> (TeamSnapshot, Vec<council::CouncilCriterionOutcomeReceipt>) {
    use council::AutomaticReviewerCount as Count;
    let mut plan = composition(true, Count::Fixed(2), vec![]);
    plan.strategy = council::CouncilStrategy::TwoPhase;
    plan.criteria_source = council::CouncilCriteriaSource::User;
    plan.criteria.push(council::CouncilCriterion {
        id: "coverage".into(),
        lens: "coverage".into(),
        description: "Verify the frozen task evidence.".into(),
        required: true,
        target: 9.0,
        evidence_required: true,
        source: council::CouncilCriterionOrigin::User,
    });
    let config = config_for_composition(plan);
    let mut team = team_with(config, Vec::new());
    team.council_since = storage::now().max(2);
    for round in 1..=if with_challengers { 2 } else { 1 } {
        for (index, lens) in ["security", "correctness"].into_iter().enumerate() {
            let id = format!("phase-{round}-{index}");
            let mut reviewer = seat(&id, round, Some(9.7), team.council_since + round as i64 * 10 + index as i64);
            reviewer.council.as_mut().unwrap().lens = lens.into();
            reviewer.provider = "codex".into();
            reviewer.model = if round == 2 && !with_challengers {
                format!("phase1-model-{index}")
            } else {
                format!("phase{round}-model-{index}")
            };
            let binding = reviewer_security::ReviewerBinding::native(
                &team,
                &reviewer.id,
                reviewer.created_at,
                &reviewer.provider,
                &reviewer.model,
                reviewer.council.as_ref().unwrap(),
            );
            reviewer.reviewer_binding = Some(binding);
            team.workers.push(reviewer);
        }
    }
    let cycle_id = format!("{}:{}", team.session_id, team.council_since);
    let outcomes = team.workers.iter().map(|reviewer| {
        let seat = reviewer.council.as_ref().unwrap();
        let binding = reviewer.reviewer_binding.as_ref().unwrap();
        council::CouncilCriterionOutcomeReceipt {
            schema_version: 1,
            root_cycle_id: cycle_id.clone(),
            task_id: team.session_id.clone(),
            task_cycle_id: cycle_id.clone(),
            child_cycle_id: None,
            round: seat.round,
            seat_id: reviewer.id.clone(),
            parent_seat_id: None,
            attempt: reviewer.attempt,
            artifact_revision: binding.artifact_revision.clone(),
            outcome: council::CouncilCriterionOutcome {
                criterion_id: "coverage".into(),
                score: 9.6,
                verified_evidence_refs: vec![format!("verified:{}", reviewer.id)],
            },
        }
    }).collect();
    (team, outcomes)
}

#[test]
fn scored_council_round_rejects_duplicate_worker_or_native_session() {
    let config = on(9.0, 10, 2);
    let team = team_with(config, vec![
        seat("identity-first", 1, Some(9.8), SINCE + 1),
        seat("identity-second", 1, Some(9.7), SINCE + 2),
    ]);
    assert!(council::complete(&team, team.config.council.as_ref().unwrap(), 1));
    assert_eq!(council::status(&team).unwrap().state, "passed");

    let mut duplicated_session = team.clone();
    duplicated_session.workers[1].session_id = team.workers[0].session_id.clone();
    assert!(!council::complete(&duplicated_session, duplicated_session.config.council.as_ref().unwrap(), 1));
    assert_ne!(council::status(&duplicated_session).unwrap().state, "passed");

    let mut duplicated_worker = team.clone();
    duplicated_worker.workers[1].id = team.workers[0].id.clone();
    assert!(!council::complete(&duplicated_worker, duplicated_worker.config.council.as_ref().unwrap(), 1));
    assert_ne!(council::status(&duplicated_worker).unwrap().state, "passed");
}

#[test]
fn two_phase_requires_both_correlated_rounds_and_independent_challengers() {
    let (phase1_only, phase1_receipts) = two_phase_fixture(false);
    let phase1_status = council::status_with_criterion_outcomes(&phase1_only, &phase1_receipts).unwrap();
    assert_eq!(phase1_status.state, "dispatch");
    assert!(council::finish_refusal_with_criterion_outcomes(&phase1_only, &phase1_receipts).unwrap().contains("phase2_required"));

    let (passed, passed_receipts) = two_phase_fixture(true);
    assert_eq!(council::status_with_criterion_outcomes(&passed, &passed_receipts).unwrap().state, "passed");

    let mut same_authors = passed.clone();
    for (index, worker) in same_authors.workers.iter_mut().filter(|worker| worker.council.as_ref().is_some_and(|seat| seat.round == 2)).enumerate() {
        worker.model = format!("phase1-model-{index}");
        if let Some(binding) = &mut worker.reviewer_binding {
            binding.snapshot.model = worker.model.clone();
        }
    }
    assert_eq!(council::status_with_criterion_outcomes(&same_authors, &passed_receipts).unwrap().state, "blocked");

    let mut reused_session = passed.clone();
    reused_session.workers[2].session_id = passed.workers[0].session_id.clone();
    assert_ne!(council::status_with_criterion_outcomes(&reused_session, &passed_receipts).unwrap().state, "passed",
        "a fresh two-phase challenger must not reuse a phase-one native session");
}

#[test]
fn escalated_strategy_executes_its_pinned_criteria_gate() {
    use council::AutomaticReviewerCount as Count;
    let mut plan = composition(true, Count::Fixed(2), vec![]);
    plan.strategy = council::CouncilStrategy::Escalated;
    let (team, receipts) = executable_composition_fixture(config_for_composition(plan), 1);
    assert_eq!(
        council::status_with_criterion_outcomes(&team, &receipts).unwrap().state,
        "passed",
        "Escalated uses the same pinned score and evidence gate after S2 selected a priced source",
    );
}

#[test]
fn actual_composed_gate_requires_current_persisted_criterion_outcomes_and_evidence() {
    use council::{AutomaticReviewerCount as Count, CouncilCriterionOrigin as Origin, CouncilCriteriaSource as Source};
    let mut plan = composition(true, Count::Fixed(2), vec![]);
    plan.criteria_source = Source::User;
    plan.criteria.push(council::CouncilCriterion {
        id: "authorization".into(),
        lens: "authorization".into(),
        description: "Reject access outside the task scope.".into(),
        required: true,
        target: 9.0,
        evidence_required: true,
        source: Origin::User,
    });
    let config = config_for_composition(plan);
    let mut team = team_with(config, vec![
        seat("criteria-a", 1, Some(9.8), SINCE + 1),
        seat("criteria-b", 1, Some(9.7), SINCE + 2),
    ]);
    team.council_since = SINCE;
    for index in 0..team.workers.len() {
        let (id, created_at, provider, model, seat) = {
            let worker = &team.workers[index];
            (worker.id.clone(), worker.created_at, worker.provider.clone(), worker.model.clone(), worker.council.as_ref().unwrap().clone())
        };
        team.workers[index].reviewer_binding = Some(reviewer_security::ReviewerBinding::native(
            &team, &id, created_at, &provider, &model, &seat,
        ));
    }
    assert_ne!(council::status(&team).unwrap().state, "passed", "a high average cannot cover missing required outcomes");

    let artifact_revision = team.workers[0].reviewer_binding.as_ref().unwrap().artifact_revision.clone();
    let cycle_id = format!("{}:{}", team.session_id, team.council_since);
    let task_id = team.session_id.clone();
    let seat_id = team.workers[0].id.clone();
    let attempt = team.workers[0].attempt;
    let receipt = |artifact_revision: String, score: f64, evidence: Vec<String>| {
        council::CouncilCriterionOutcomeReceipt {
            schema_version: 1,
            root_cycle_id: cycle_id.clone(),
            task_id: task_id.clone(),
            task_cycle_id: cycle_id.clone(),
            child_cycle_id: None,
            round: 1,
            seat_id: seat_id.clone(),
            parent_seat_id: None,
            attempt,
            artifact_revision,
            outcome: council::CouncilCriterionOutcome {
                criterion_id: "authorization".into(),
                score,
                verified_evidence_refs: evidence,
            },
        }
    };
    let valid = receipt(
        artifact_revision.clone(),
        9.4,
        vec!["file:src/security.rs#sha256:abcd".into()],
    );
    assert_eq!(
        council::status_with_criterion_outcomes(&team, &[valid.clone()])
            .unwrap()
            .state,
        "passed"
    );

    let stale = receipt("stale-artifact-revision".into(), 10.0, vec!["file:src/security.rs#sha256:abcd".into()]);
    assert_ne!(
        council::status_with_criterion_outcomes(&team, &[stale.clone()])
            .unwrap()
            .state,
        "passed"
    );
    assert_eq!(
        council::status_with_criterion_outcomes(&team, &[valid, stale])
            .unwrap()
            .state,
        "passed",
        "stale extra receipts cannot displace a valid receipt"
    );

    let missing_evidence = receipt(artifact_revision, 10.0, vec![]);
    assert!(matches!(
        team.config.council.as_ref().unwrap().composition.as_ref().unwrap()
            .validate_required_criteria_gate(9.0, &[vec![missing_evidence.outcome]]),
        Err(council::CouncilCompositionError::MissingCriterionEvidence(_))
    ));
}

#[test]
fn mixed_composition_quorum_requires_automatic_seats_in_addition_to_selected_bots() {
    use council::AutomaticReviewerCount as Count;
    let cases = [
        composition(true, Count::Fixed(1), vec![direct_reviewer("direct-a")]),
        composition(true, Count::Fixed(1), vec![nested_reviewer("nested-a")]),
        composition(true, Count::Fixed(1), vec![direct_reviewer("direct-a"), nested_reviewer("nested-a")]),
    ];
    for plan in cases {
        let config = config_for_composition(plan);
        let roster = config.reviewer_bot_ids.clone();
        let since = storage::now().max(2);
        let mut team = team_with(config.clone(), roster.iter().enumerate()
            .map(|(index, _id)| seat(&format!("selected-{index}"), 1, Some(9.8), since + index as i64 + 1))
            .collect());
        team.council_since = since;
        for (index, id) in roster.iter().enumerate() {
            let worker = &team.workers[index];
            let mut binding = reviewer_security::ReviewerBinding::native(
                &team, &worker.id, worker.created_at, &worker.provider, &worker.model,
                worker.council.as_ref().unwrap(),
            );
            binding.reviewer_bot_id = Some(id.clone());
            binding.roster_bot_ids = roster.clone();
            // This is the selected roster size written by architect.rs. It must not replace the
            // source-aware total on a mixed composition round.
            binding.required_reviewers = Some(roster.len());
            team.workers[index].reviewer_binding = Some(binding);
        }
        let quota = config.seat_plan();
        assert!(quota.automatic > 0 && quota.selected_bots() == roster.len());
        assert!(!council::complete(&team, &config, 1), "selected bots alone cannot satisfy M3/M5/M7");
        assert!(finish_refusal(&team).unwrap().contains(&format!("at least {} needed", quota.required())));

        for index in 0..quota.automatic {
            let mut automatic = seat(&format!("automatic-{index}"), 1, Some(9.7), team.council_since + 20 + index as i64);
            let worker = &automatic;
            automatic.reviewer_binding = Some(reviewer_security::ReviewerBinding::native(
                &team, &worker.id, worker.created_at, &worker.provider, &worker.model,
                worker.council.as_ref().unwrap(),
            ));
            team.workers.push(automatic);
        }
        assert!(council::complete(&team, &config, 1), "the full source plan satisfies the quota");
    }
}

#[test]
fn mixed_seat_resolution_freezes_bot_identities_and_reserves_only_planned_automatic_slots() {
    use council::AutomaticReviewerCount as Count;
    let (store, mut team) = fixture();
    let project = store.session(&team.session_id).unwrap().project_id;
    for id in ["bot-a", "bot-b"] {
        store.db.execute(
            "INSERT INTO bots(id,name,role,instructions,goals,soul,project_id,provider,model,mode,created_at,updated_at,skills) VALUES(?1,?1,'QA','Original instructions','Original goals','Original soul',?2,'codex','frozen-model','full',1,1,'[]')",
            params![id, project],
        ).unwrap();
    }
    let config = config_for_composition(composition(
        true,
        Count::Fixed(1),
        vec![direct_reviewer("bot-a"), nested_reviewer("bot-b")],
    ));
    team.config.council = Some(config.clone());
    team.council_since = SINCE;

    let first = bot_reviewers::seat_bot_inner(&store, &team, 1, None, false).unwrap().unwrap();
    assert_eq!(first.id, "bot-a");
    assert_eq!(first.composition_mode, Some(council::SelectedReviewerMode::Direct));
    let mut first_worker = seat("mixed-direct", 1, None, SINCE + 1);
    first_worker.provider = first.profile.provider.clone();
    first_worker.model = first.profile.model.clone();
    let mut first_binding = reviewer_security::ReviewerBinding::native(
        &team, &first_worker.id, first_worker.created_at, &first_worker.provider,
        &first_worker.model, first_worker.council.as_ref().unwrap(),
    );
    first_binding.reviewer_bot_id = Some(first.id.clone());
    first_binding.roster_bot_ids = first.roster_bot_ids.clone();
    first_binding.required_reviewers = Some(first.roster_bot_ids.len());
    first_binding.composition_mode = Some("direct".into());
    first_binding.snapshot.bot_context = Some(first.context.clone());
    first_worker.reviewer_binding = Some(first_binding);
    team.workers.push(first_worker);

    store.db.execute(
        "UPDATE bots SET name='Edited reviewer',model='edited-model',instructions='Edited instructions' WHERE id='bot-b'",
        [],
    ).unwrap();
    let second = bot_reviewers::seat_bot_inner(&store, &team, 1, None, false).unwrap().unwrap();
    assert_eq!(second.id, "bot-b");
    assert_eq!(second.composition_mode, Some(council::SelectedReviewerMode::Nested));
    assert_eq!(second.profile.model, "frozen-model");
    assert_eq!(second.context.name, "bot-b");
    let mut second_worker = seat("mixed-nested", 1, None, SINCE + 2);
    second_worker.provider = second.profile.provider.clone();
    second_worker.model = second.profile.model.clone();
    let mut second_binding = reviewer_security::ReviewerBinding::native(
        &team, &second_worker.id, second_worker.created_at, &second_worker.provider,
        &second_worker.model, second_worker.council.as_ref().unwrap(),
    );
    second_binding.reviewer_bot_id = Some(second.id.clone());
    second_binding.roster_bot_ids = second.roster_bot_ids.clone();
    second_binding.required_reviewers = Some(second.roster_bot_ids.len());
    second_binding.composition_mode = Some("nested".into());
    second_binding.snapshot.bot_context = Some(second.context.clone());
    second_worker.reviewer_binding = Some(second_binding);
    team.workers.push(second_worker);

    assert!(bot_reviewers::seat_bot_inner(&store, &team, 1, None, false).unwrap().is_none(),
        "once selected identities are occupied, the next planned seat is automatic");
    let mut automatic = seat("mixed-automatic", 1, None, SINCE + 3);
    automatic.reviewer_binding = Some(reviewer_security::ReviewerBinding::native(
        &team, &automatic.id, automatic.created_at, &automatic.provider, &automatic.model,
        automatic.council.as_ref().unwrap(),
    ));
    team.workers.push(automatic);
    assert!(bot_reviewers::seat_bot_inner(&store, &team, 1, Some("mixed-automatic"), false).unwrap().is_none(),
        "replacing a native seat preserves its automatic source");
    assert!(bot_reviewers::seat_bot_inner(&store, &team, 1, None, false).unwrap_err()
        .contains("automatic reviewer quota is already filled"));
}

#[test]
fn all_three_strategies_validate_without_changing_the_roster_or_quorum() {
    let mut config = config_for_composition(composition(true, council::AutomaticReviewerCount::Fixed(1), vec![direct_reviewer("direct-a"), nested_reviewer("nested-a")]));
    let roster = config.reviewer_bot_ids.clone();
    let quorum = config.needed();
    for strategy in [council::CouncilStrategy::Parallel, council::CouncilStrategy::Escalated, council::CouncilStrategy::TwoPhase] {
        config.composition.as_mut().unwrap().strategy = strategy;
        assert!(config.validate().is_ok(), "{strategy:?}");
        assert_eq!(config.needed(), quorum, "strategy cannot silently add reviewers");
        assert_eq!(config.reviewer_bot_ids, roster);
        assert_eq!(config.reviewers, 1);
    }
}

#[test]
fn composition_rejects_empty_sources_duplicate_or_invalid_roster_and_mirror_drift() {
    let empty = composition(false, council::AutomaticReviewerCount::Fixed(0), vec![]);
    assert!(matches!(empty.validate(), Err(council::CouncilCompositionError::MissingReviewerSource)));

    let mut invalid = composition(false, council::AutomaticReviewerCount::Fixed(0), vec![direct_reviewer("../bad"), direct_reviewer("second")]);
    assert!(matches!(invalid.validate(), Err(council::CouncilCompositionError::InvalidRosterId(_))));
    invalid.selected = vec![direct_reviewer("same"), direct_reviewer("same")];
    assert!(matches!(invalid.validate(), Err(council::CouncilCompositionError::DuplicateReviewerId(_))));

    let valid = composition(false, council::AutomaticReviewerCount::Fixed(0), vec![direct_reviewer("one"), direct_reviewer("two")]);
    let mut config = config_for_composition(valid);
    config.reviewer_bot_ids.swap(0, 1);
    assert!(matches!(config.validate_composition(), Err(council::CouncilCompositionError::MirrorMismatch("reviewerBotIds"))));
    config.reviewer_bot_ids.swap(0, 1);
    config.reviewers = 1;
    assert!(matches!(config.validate_composition(), Err(council::CouncilCompositionError::MirrorMismatch("reviewers"))));
}

#[test]
fn composition_rejects_bad_identity_quorum_child_settings_and_root_limits() {
    use council::{AutomaticReviewerCount as Count, CouncilCompositionError as Error};
    let one_direct = composition(false, Count::Fixed(0), vec![direct_reviewer("direct-a")]);
    assert!(matches!(one_direct.validate(), Err(Error::RootQuorum)));

    let mut optional = composition(false, Count::Fixed(0), vec![direct_reviewer("direct-a"), direct_reviewer("direct-b")]);
    optional.selected[0].required = false;
    assert!(matches!(optional.validate(), Err(Error::ReviewerMustBeRequired(_))));

    let mut nested_missing = composition(false, Count::Fixed(0), vec![nested_reviewer("nested-a"), nested_reviewer("nested-b")]);
    nested_missing.selected[0].nested = None;
    assert!(matches!(nested_missing.validate(), Err(Error::MissingNestedConfig(_))));
    nested_missing.selected[0] = nested_reviewer("nested-a");
    nested_missing.selected[0].nested.as_mut().unwrap().child_reviewer_count = 1;
    assert!(matches!(nested_missing.validate(), Err(Error::InvalidNestedConfig { .. })));

    let too_many = composition(true, Count::Fixed(1), (0..8).map(|n| direct_reviewer(&format!("direct-{n}"))).collect());
    assert!(matches!(too_many.validate(), Err(Error::RootSeatLimit { actual: 9, maximum: 8 })));

    let mut limits = composition(false, Count::Fixed(0), vec![direct_reviewer("direct-a"), direct_reviewer("direct-b")]);
    limits.limits.max_depth = 1;
    assert!(matches!(limits.validate(), Err(Error::InvalidLimit("maxDepth must be 2"))));
    limits.limits = composition_limits();
    limits.limits.max_root_seats = 7;
    assert!(matches!(limits.validate(), Err(Error::InvalidLimit("maxRootSeats must be 8"))));
    limits.limits = composition_limits();
    limits.limits.max_concurrent_leaves = 0;
    assert!(matches!(limits.validate(), Err(Error::InvalidLimit("maxConcurrentLeaves"))));
    limits.limits = composition_limits();
    limits.limits.max_leaf_reviews_per_round = 129;
    assert!(matches!(limits.validate(), Err(Error::InvalidLimit("maxLeafReviewsPerRound"))));
    limits.limits = composition_limits();
    limits.limits.timeout_ms = 0;
    assert!(matches!(limits.validate(), Err(Error::InvalidLimit("timeoutMs"))));
    limits.limits = composition_limits();
    limits.limits.max_estimated_cost_micros = Some(0);
    assert!(matches!(limits.validate(), Err(Error::InvalidLimit("maxEstimatedCostMicros"))));
}

#[test]
fn dispatch_validation_fails_closed_for_self_review_and_unsupported_capabilities() {
    use council::{AutomaticReviewerCount as Count, CouncilCompositionError as Error};
    let mut config = config_for_composition(composition(false, Count::Fixed(0), vec![direct_reviewer("bot-a"), direct_reviewer("bot-b")]));
    let capabilities = council::CouncilCompositionCapabilities {
        automatic_reviewers: true,
        supported_strategies: vec![council::CouncilStrategy::Parallel, council::CouncilStrategy::Escalated, council::CouncilStrategy::TwoPhase],
        reviewers: vec![
            council::CouncilReviewerCapability { bot_id: "bot-a".into(), direct: true, nested: false },
            council::CouncilReviewerCapability { bot_id: "bot-b".into(), direct: true, nested: false },
        ],
    };
    assert!(matches!(config.validate_for_dispatch(Some("bot-a"), &capabilities), Err(Error::SelfReview(id)) if id == "bot-a"));
    assert_eq!(config.validate_for_dispatch(None, &capabilities).unwrap(), Some(council::CouncilCompositionMode::Bots));

    config.composition.as_mut().unwrap().selected[0] = nested_reviewer("bot-a");
    config.composition.as_mut().unwrap().selected[1] = nested_reviewer("bot-b");
    assert!(matches!(config.validate_for_dispatch(None, &capabilities), Err(Error::UnsupportedCapability { bot_id: Some(_), capability: "nested Council review" })));

    config.composition.as_mut().unwrap().selected = vec![direct_reviewer("bot-a"), direct_reviewer("bot-b")];
    config.composition.as_mut().unwrap().automatic.enabled = true;
    config.composition.as_mut().unwrap().automatic.count = Count::Fixed(1);
    config.reviewers = 1;
    assert!(matches!(config.validate_for_dispatch(None, &council::CouncilCompositionCapabilities { automatic_reviewers: false, ..capabilities.clone() }), Err(Error::UnsupportedCapability { capability: "automatic root reviewers", .. })));

    config.composition.as_mut().unwrap().automatic.enabled = false;
    config.reviewers = 0;
    config.composition.as_mut().unwrap().strategy = council::CouncilStrategy::TwoPhase;
    assert!(matches!(config.validate_for_dispatch(None, &council::CouncilCompositionCapabilities { supported_strategies: vec![council::CouncilStrategy::Parallel], ..capabilities }), Err(Error::UnsupportedCapability { capability: "selected execution strategy", .. })));
}

#[test]
fn required_criteria_and_verified_evidence_are_frozen_and_gate_by_the_stricter_target() {
    use council::{AutomaticReviewerCount as Count, CouncilCriterionOrigin as Origin, CouncilCriteriaSource as Source};
    let mut plan = composition(false, Count::Fixed(0), vec![direct_reviewer("bot-a"), direct_reviewer("bot-b")]);
    plan.criteria_source = Source::Hybrid;
    plan.criteria = vec![
        council::CouncilCriterion { id: "authorization".into(), lens: "Authorization".into(), description: "Verify authorization boundaries.".into(), required: true, target: 9.4, evidence_required: true, source: Origin::User },
        council::CouncilCriterion { id: "compatibility".into(), lens: "Compatibility".into(), description: "Check API and migration compatibility.".into(), required: true, target: 9.0, evidence_required: false, source: Origin::User },
        council::CouncilCriterion { id: "architecture-note".into(), lens: "Architecture note".into(), description: "Add a nonduplicative design observation.".into(), required: false, target: 8.0, evidence_required: false, source: Origin::Architect },
    ];
    let config = config_for_composition(plan.clone());
    assert!(config.validate().is_ok());
    let serialized = serde_json::to_value(&config).unwrap();
    let reparsed: CouncilConfig = serde_json::from_value(serialized.clone()).unwrap();
    assert_eq!(reparsed, config);
    assert_eq!(serialized["composition"]["criteria"][0]["required"], true);
    assert_eq!(serialized["composition"]["criteria"][0]["evidenceRequired"], true);
    assert_eq!(serialized["composition"]["criteria"][0]["source"], "user");

    assert!(matches!(plan.validate_required_criteria_gate(9.6, &[]), Err(council::CouncilCompositionError::MissingRequiredCriterion(id)) if id == "authorization"));
    let no_evidence = vec![vec![council::CouncilCriterionOutcome { criterion_id: "authorization".into(), score: 9.8, verified_evidence_refs: vec![] }]];
    assert!(matches!(plan.validate_required_criteria_gate(9.0, &no_evidence), Err(council::CouncilCompositionError::MissingCriterionEvidence(id)) if id == "authorization"));
    let below_stricter = vec![vec![council::CouncilCriterionOutcome { criterion_id: "authorization".into(), score: 9.5, verified_evidence_refs: vec!["receipt:auth".into()] }]];
    assert!(matches!(plan.validate_required_criteria_gate(9.6, &below_stricter), Err(council::CouncilCompositionError::CriterionBelowTarget { id, target, .. }) if id == "authorization" && target == 9.6));
    let passing = vec![vec![
        council::CouncilCriterionOutcome { criterion_id: "authorization".into(), score: 9.8, verified_evidence_refs: vec!["receipt:auth".into()] },
        council::CouncilCriterionOutcome { criterion_id: "compatibility".into(), score: 9.7, verified_evidence_refs: vec![] },
    ]];
    assert!(plan.validate_required_criteria_gate(9.6, &passing).is_ok());
    let mut unknown = serialized;
    unknown["composition"]["surprise"] = json!(true);
    assert!(serde_json::from_value::<CouncilConfig>(unknown).is_err(), "composition rejects unknown contract fields");
}

#[test]
fn composed_finish_gate_does_not_pass_on_scores_without_required_criteria_coverage() {
    use council::{AutomaticReviewerCount as Count, CouncilCriterionOrigin as Origin, CouncilCriteriaSource as Source};
    let mut plan = composition(false, Count::Fixed(0), vec![direct_reviewer("bot-a"), direct_reviewer("bot-b")]);
    plan.criteria_source = Source::User;
    plan.criteria = vec![council::CouncilCriterion {
        id: "security".into(), lens: "Security".into(), description: "Check security boundaries.".into(), required: true,
        target: 9.0, evidence_required: true, source: Origin::User,
    }];
    let config = config_for_composition(plan);
    let since = storage::now().max(2);
    let mut team = team_with(config, vec![writer("writer", since), seat("review-a", 1, Some(9.9), since + 1), seat("review-b", 1, Some(9.8), since + 2)]);
    team.council_since = since;
    let refusal = finish_refusal(&team).unwrap();
    assert!(refusal.contains("council_criteria_gate_blocked"), "{refusal}");
    assert!(refusal.contains("Required Council criterion \"security\" has no scored reviewer coverage"), "{refusal}");
    assert_ne!(council::status(&team).unwrap().state, "passed");
}

#[test]
fn finding_receipt_and_usage_contracts_validate_correlation_and_unknown_measurements() {
    use council::{CouncilFindingDispositionKind as DispositionKind, CouncilReviewVerdict as Verdict, CouncilUsageValueStatus as ValueStatus};
    let finding = council::CouncilFinding {
        id: "finding-1".into(), content_digest: String::new(), cycle_id: "root-cycle".into(), round_id: "round-1".into(), seat_id: "seat-1".into(),
        artifact_digest: "a".repeat(64), criterion_id: "security".into(), severity: council::CouncilFindingSeverity::High,
        category: "authorization".into(), summary: "The handler accepts an unscoped identifier.".into(),
        title: None, explanation: None, evidence_refs: vec!["evidence-1".into()],
        location_status: council::CouncilFindingLocationStatus::Verified,
        location: Some(council::CouncilFindingLocation { file: "src/security.rs".into(), line: Some(12) }),
        suggested_fix: Some("Validate the identifier against the task scope.".into()),
    };
    assert!(finding.validate().is_ok());
    let mut unsafe_location = finding.clone();
    unsafe_location.location.as_mut().unwrap().file = "../private.txt".into();
    assert!(matches!(unsafe_location.validate(), Err(council::CouncilCompositionError::InvalidFinding(_))));
    let disposition = council::CouncilFindingDisposition {
        finding_id: finding.id.clone(), cycle_id: finding.cycle_id.clone(),
        artifact_digest: finding.artifact_digest.clone(), disposition: DispositionKind::Addressed, actor_id: "user:local".into(),
        rationale: "The change is included in the new revision.".into(), timestamp_ms: 1,
        artifact_links: vec!["artifact:revision-2".into()],
    };
    assert!(disposition.validate().is_ok());

    let child = council::ChildCouncilReceipt {
        root_cycle_id: "root-cycle".into(), parent_seat_id: "seat-1".into(), child_cycle_id: "child-cycle".into(),
        bot_id: "security-bot".into(), artifact_revision: "revision-7".into(), policy_fingerprint: "policy-fp".into(),
        status: council::ChildCouncilStatus::Passed, min_score: Some(9.4), leaf_score_refs: vec!["score-a".into(), "score-b".into()],
        findings_refs: vec![finding.id.clone()], evidence_refs: vec!["evidence-1".into()], snapshot_fingerprint: "snapshot-fp".into(),
    };
    assert!(child.validate_for("root-cycle", "seat-1", "security-bot", "revision-7", "policy-fp").is_ok());
    assert!(matches!(child.validate_for("root-cycle", "seat-1", "security-bot", "revision-8", "policy-fp"), Err(council::CouncilCompositionError::CorrelationMismatch("artifactRevision"))));
    let mut bad_child = child.clone();
    bad_child.leaf_score_refs.truncate(1);
    assert!(matches!(bad_child.validate(), Err(council::CouncilCompositionError::InvalidChildReceipt(_))));
    let mut unknown_child_field = serde_json::to_value(&child).unwrap();
    unknown_child_field["futureField"] = json!(true);
    assert!(serde_json::from_value::<council::ChildCouncilReceipt>(unknown_child_field).is_err());

    let receipt = council::CouncilReviewReceipt {
        schema_version: 1, request_id: "request-1".into(), cycle_id: "root-cycle".into(), task_id: Some("task-1".into()),
        assignment_id: None, policy_fingerprint: "policy-fp".into(), artifact_digest: "a".repeat(64), verdict: Verdict::Passed,
        findings_refs: vec![finding.id.clone()], evidence_refs: vec!["evidence-1".into()], terminal_revision: 2, idempotency_key: "delivery-1".into(),
    };
    assert!(receipt.validate_for("request-1", "root-cycle", &"a".repeat(64), "delivery-1").is_ok());
    assert_eq!(serde_json::to_value(&receipt).unwrap()["terminalRevision"], 2);

    let known = |value| council::CouncilUsageValue { status: ValueStatus::Measured, value: Some(value) };
    let unknown = council::CouncilUsageValue::<u64> { status: ValueStatus::Unknown, value: None };
    let usage = council::CouncilUsageRecord {
        id: "usage-1".into(), root_cycle_id: "root-cycle".into(), child_cycle_id: None, attempt_id: "attempt-1".into(),
        reviewer_id: "reviewer-1".into(), round_id: "round-1".into(), seat_id: "seat-1".into(), scope: council::CouncilUsageScope::Root,
        phase: council::CouncilUsagePhase::Review, strategy: council::CouncilStrategy::Parallel, decision_ids: Vec::new(), started_at_ms: Some(10), finished_at_ms: Some(110),
        queue_wait_ms: unknown.clone(), elapsed_ms: known(100u64), retries: council::CouncilUsageValue { status: ValueStatus::Measured, value: Some(0u32) },
        escalations: council::CouncilUsageValue { status: ValueStatus::Unknown, value: None }, provider: Some("codex".into()), model: Some("model-1".into()), harness: Some("codex".into()),
        input_tokens: known(120u64), output_tokens: known(40u64), request_count: known(1u64), terminal_status: Verdict::Passed,
        budget_consumption: council::CouncilResourceBudgetConsumption { concurrent_leaves: known(1u64), leaf_reviews: known(2u64), estimated_cost_micros: unknown },
        cost_estimate: Some(council::CouncilCostEstimate { amount_micros: 1200, currency: "USD".into(), pricing_source: "provider-card".into(), pricing_version: "2026-01".into(), calculated_at_ms: 120 }),
    };
    assert!(usage.validate().is_ok());
    assert_eq!(serde_json::to_value(&usage).unwrap()["rootCycleId"], "root-cycle");
    let mut fabricated = usage;
    fabricated.elapsed_ms = council::CouncilUsageValue { status: ValueStatus::Unknown, value: Some(100) };
    assert!(matches!(fabricated.validate(), Err(council::CouncilCompositionError::InvalidUsageRecord(_))));
}

#[test]
fn critical_findings_require_verified_or_current_human_disposition() {
    use council::{CouncilFindingDispositionKind as Kind, CouncilFindingSeverity as Severity};
    let make_finding = |seat: &str, summary: &str| {
        let mut finding = council::CouncilFinding {
            id: String::new(), content_digest: String::new(), cycle_id: "root:cycle-4".into(),
            round_id: "root:cycle-4:2".into(), seat_id: seat.into(),
            artifact_digest: "a".repeat(64), criterion_id: "security".into(),
            severity: Severity::Critical, category: "authorization".into(),
            summary: summary.into(), title: None, explanation: None, evidence_refs: vec!["file:src/auth.rs#sha256:".to_string() + &"b".repeat(64)],
            location_status: council::CouncilFindingLocationStatus::Missing, location: None,
            suggested_fix: None,
        };
        finding.content_digest = finding.calculated_content_digest();
        finding.id = format!("finding-{}", finding.content_digest);
        finding
    };
    let first = make_finding("seat-a", "Cross-task access is possible.");
    let second = make_finding("seat-b", "An untrusted caller can change scope.");
    assert_ne!(first.id, second.id);
    assert!(first.validate().is_ok());
    assert!(second.validate().is_ok());
    let findings = vec![first.clone(), second.clone()];
    assert_eq!(
        council::unresolved_critical_findings(&findings, &[], "root:cycle-4"),
        vec![first.id.clone(), second.id.clone()],
    );

    let disposition = |finding_id: &str, actor: &str, kind| {
        council::CouncilFindingDisposition {
            finding_id: finding_id.into(), cycle_id: "root:cycle-4".into(),
            artifact_digest: "a".repeat(64), disposition: kind, actor_id: actor.into(),
            rationale: "Reviewed against the current artifact.".into(), timestamp_ms: 10,
            artifact_links: Vec::new(),
        }
    };
    let forged_waiver = disposition(
        &first.id, "architect", Kind::WaivedByAuthorizedHuman,
    );
    let stale_waiver = disposition(
        "finding-stale-cycle", "user", Kind::WaivedByAuthorizedHuman,
    );
    let stale_artifact = disposition(
        "finding-stale-artifact", "user", Kind::WaivedByAuthorizedHuman,
    );
    let model_addressed = disposition(
        &first.id, "architect", Kind::Addressed,
    );
    assert_eq!(
        council::unresolved_critical_findings(
            &findings, &[forged_waiver, stale_waiver, stale_artifact, model_addressed],
            "root:cycle-4",
        ),
        vec![first.id.clone(), second.id.clone()],
    );
    let mut wrong_cycle = disposition(&first.id, "user", Kind::WaivedByAuthorizedHuman);
    wrong_cycle.cycle_id = "root:old-cycle".into();
    let mut wrong_artifact = disposition(&first.id, "user", Kind::WaivedByAuthorizedHuman);
    wrong_artifact.artifact_digest = "c".repeat(64);
    assert_eq!(
        council::unresolved_critical_findings(
            &findings, &[wrong_cycle, wrong_artifact], "root:cycle-4",
        ),
        vec![first.id.clone(), second.id.clone()],
        "a matching ID cannot let a stale cycle or artifact disposition close the finding"
    );
    let valid_waiver = disposition(
        &first.id, "user", Kind::WaivedByAuthorizedHuman,
    );
    assert_eq!(
        council::unresolved_critical_findings(&findings, &[valid_waiver], "root:cycle-4"),
        vec![second.id.clone()],
    );
    let native_verification = disposition(
        &first.id, "native:verification", Kind::Addressed,
    );
    assert_eq!(
        council::unresolved_critical_findings(
            &findings, &[native_verification], "root:cycle-4",
        ),
        vec![second.id],
    );
}

#[test]
fn every_structured_finding_requires_evidence_even_when_criterion_evidence_is_optional() {
    let mut finding = council::CouncilFinding {
        id: "finding-empty-evidence".into(), content_digest: String::new(),
        cycle_id: "root:cycle-1".into(), round_id: "root:cycle-1:1".into(),
        seat_id: "reviewer".into(), artifact_digest: "a".repeat(64),
        criterion_id: "lens".into(), severity: council::CouncilFindingSeverity::Low,
        category: "quality".into(), summary: "The report has no supporting evidence.".into(),
        title: None, explanation: None, evidence_refs: Vec::new(), location_status: council::CouncilFindingLocationStatus::Missing,
        location: None, suggested_fix: None,
    };
    finding.content_digest = finding.calculated_content_digest();
    finding.id = format!("finding-{}", finding.content_digest);
    assert!(matches!(
        finding.validate(),
        Err(council::CouncilCompositionError::InvalidFinding(_))
    ));
}

#[test]
fn escalation_and_challenge_usage_require_correlated_native_decision_ids() {
    use council::{CouncilUsagePhase as Phase, CouncilUsageValue as UsageValue};
    use council::CouncilUsageValueStatus as ValueStatus;
    let known_u32 = |value| UsageValue { status: ValueStatus::Measured, value: Some(value) };
    let known_u64 = |value| UsageValue { status: ValueStatus::Measured, value: Some(value) };
    let unknown = UsageValue::<u64> { status: ValueStatus::Unknown, value: None };
    let mut usage = council::CouncilUsageRecord {
        id: "usage-2".into(), root_cycle_id: "root-cycle".into(), child_cycle_id: None,
        attempt_id: "seat-2:1".into(), reviewer_id: "reviewer-2".into(), round_id: "task-cycle:2".into(),
        seat_id: "seat-2".into(), scope: council::CouncilUsageScope::Root,
        phase: Phase::Review, strategy: council::CouncilStrategy::Parallel, decision_ids: Vec::new(),
        started_at_ms: None, finished_at_ms: None, queue_wait_ms: unknown.clone(), elapsed_ms: unknown.clone(),
        retries: known_u32(0u32), escalations: known_u32(0u32), provider: None, model: None, harness: None,
        input_tokens: unknown.clone(), output_tokens: unknown.clone(), request_count: unknown.clone(),
        terminal_status: council::CouncilReviewVerdict::Passed,
        budget_consumption: council::CouncilResourceBudgetConsumption {
            concurrent_leaves: known_u64(1u64), leaf_reviews: known_u64(2u64), estimated_cost_micros: unknown,
        },
        cost_estimate: None,
    };

    usage.phase = Phase::Escalation;
    usage.strategy = council::CouncilStrategy::Escalated;
    usage.decision_ids = vec!["trigger-1".into()];
    usage.escalations = known_u32(1u32);
    assert!(usage.validate().is_ok());
    usage.decision_ids.clear();
    assert!(matches!(usage.validate(), Err(council::CouncilCompositionError::InvalidUsageRecord(_))));
    usage.decision_ids = vec!["trigger-1".into()];
    usage.escalations = known_u32(0u32);
    assert!(matches!(usage.validate(), Err(council::CouncilCompositionError::InvalidUsageRecord(_))));

    usage.phase = Phase::Challenge;
    usage.strategy = council::CouncilStrategy::TwoPhase;
    usage.decision_ids = vec!["challenge-1".into()];
    assert!(usage.validate().is_ok());
    usage.decision_ids.clear();
    assert!(matches!(usage.validate(), Err(council::CouncilCompositionError::InvalidUsageRecord(_))));
}

#[test]
fn content_digest_makes_changed_findings_distinct_and_keeps_legacy_rows_readable() {
    let mut original = council::CouncilFinding {
        id: String::new(), content_digest: String::new(),
        cycle_id: "root-cycle".into(), round_id: "root-cycle:1".into(), seat_id: "security-seat".into(),
        artifact_digest: "a".repeat(64), criterion_id: "security".into(),
        severity: council::CouncilFindingSeverity::High, category: "authorization".into(),
        summary: "A caller can cross the task boundary.".into(), title: None, explanation: None, evidence_refs: vec!["file:src/auth.rs#sha256:".to_string() + &"b".repeat(64)],
        location_status: council::CouncilFindingLocationStatus::Missing, location: None,
        suggested_fix: Some("Bind the caller to the task.".into()),
    };
    original.content_digest = original.calculated_content_digest();
    original.id = format!("finding-{}", original.content_digest);
    assert!(original.validate().is_ok());
    assert!(original.has_content_identity());

    let mut corrected = original.clone();
    corrected.summary = "The corrected artifact still needs an independent check.".into();
    corrected.content_digest = corrected.calculated_content_digest();
    corrected.id = format!("finding-{}", corrected.content_digest);
    assert!(corrected.validate().is_ok());
    assert_ne!(corrected.id, original.id, "changed content cannot collide with its earlier finding");

    let mut actionable = original.clone();
    actionable.title = Some("Cross-task authorization".into());
    actionable.explanation = Some("The caller can request another task's artifact.".into());
    actionable.content_digest = actionable.calculated_content_digest();
    actionable.id = format!("finding-{}", actionable.content_digest);
    assert!(actionable.validate().is_ok());
    assert_ne!(actionable.id, original.id,
        "structured details must be part of the immutable finding identity");
    let mut forged = actionable.clone();
    forged.explanation = Some("An injected replacement explanation.".into());
    assert!(!forged.has_content_identity(),
        "modifying text after persistence must invalidate its identity");
    forged.content_digest = forged.calculated_content_digest();
    forged.id = format!("finding-{}", forged.content_digest);
    assert!(forged.validate().is_ok());
    assert_ne!(forged.id, actionable.id);
    let mut unsafe_title = actionable.clone();
    unsafe_title.title = Some("Bad\u{0000}title".into());
    unsafe_title.content_digest = unsafe_title.calculated_content_digest();
    unsafe_title.id = format!("finding-{}", unsafe_title.content_digest);
    assert!(unsafe_title.validate().is_err(),
        "untrusted control characters must not be accepted in finding details");

    let mut old_json = serde_json::to_value(&original).unwrap();
    old_json.as_object_mut().unwrap().remove("contentDigest");
    old_json["id"] = json!("finding-legacy");
    let legacy: council::CouncilFinding = serde_json::from_value(old_json).unwrap();
    assert!(legacy.content_digest.is_empty());
    assert!(legacy.validate().is_ok(), "additive digest storage preserves legacy rows");
}

#[test]
fn legacy_m1_and_m2_json_keep_the_original_defaults_and_serialization() {
    let m1: CouncilConfig = serde_json::from_value(json!({"target":9.5})).unwrap();
    assert!(m1.composition.is_none());
    assert_eq!(m1, on(9.5, 10, 0));
    assert_eq!(m1.needed(), 2, "legacy automatic Council keeps its minimum quorum");
    assert_eq!(serde_json::to_value(&m1).unwrap(), json!({"target":9.5,"maxRounds":10,"reviewers":0}));

    let m2: CouncilConfig = serde_json::from_value(json!({"target":9.5,"maxRounds":3,"reviewers":0,"reviewerBotIds":["bot-a","bot-b"]})).unwrap();
    assert!(m2.composition.is_none());
    assert!(m2.validate().is_ok());
    assert_eq!(m2.needed(), 2, "legacy selected-bot Council keeps its pinned roster quorum");
    assert!(serde_json::to_value(&m2).unwrap().get("composition").is_none());
}
