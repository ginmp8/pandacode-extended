//! Council across turns: cycles, changes, rounds over time, resting and the user's answers.
use super::*;

#[test]
fn turning_council_on_reviews_only_what_comes_next() {
    let (state, sid) = state_fixture();
    let mut team = saved(&state, &sid);
    team.status = "ready".into();
    store_team(&state, &mut team);
    // Earlier work in this thread, before Council was on.
    persist_turn(&state, &sid, &["index.html"]);
    std::thread::sleep(std::time::Duration::from_millis(3));
    council::set(&state, &sid, Some(on(9.0, 0, 0))).unwrap();
    let (_, admission) = start_state(&state, &sid, "What does it do?", None, None).unwrap();
    let (lead, _rx) = claim(&state, Launch { sid: sid.clone(), prompt: "What does it do?".into(), images: None, admission });
    finish(&state, &lead, "It does X.", true);
    assert!(next_action_state(&state, &mut |_| {}).unwrap().is_none(), "a plain answer is not woken");
    let team = saved(&state, &sid);
    assert_eq!(team.status, "ready");
    assert_eq!(finish_refusal(&team), None);
    // Changing the setting while it is on does not restart the cycle.
    let since = team.council_since;
    std::thread::sleep(std::time::Duration::from_millis(3));
    assert_eq!(council::set(&state, &sid, Some(on(8.0, 0, 0))).unwrap().council_since, since);
}

#[test]
fn a_replacement_keeps_the_replaced_lens() {
    let mut failed = seat("b", 1, None, SINCE + 1);
    failed.status = "failed".into();
    let team = team_with(on(9.0, 0, 0), vec![seat("a", 1, Some(9.0), SINCE + 1), failed]);
    assert!(council::seat_for(&team, Some("other"), Some("b")).unwrap_err().contains("lens b"));
    assert_eq!(council::seat_for(&team, Some("lens b"), Some("b")).unwrap().unwrap().lens, "lens b");
    assert_eq!(council::seat_for(&team, Some(" LENS B "), Some("b")).unwrap().unwrap().lens, "lens b", "replacement inputs keep the issued lens spelling");
}

#[test]
fn a_lead_that_edits_alone_is_brought_back_once_then_paused_with_the_reason() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let lead = objective_turn(&state, &sid);
    turn_change(&state, &sid, "solo", storage::now(), &["index.html"], &[]);
    finish(&state, &lead, "I edited index.html myself.", true);
    let nudge = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert_eq!(nudge.sid, sid);
    assert!(nudge.prompt.contains("Council has not passed") && nudge.prompt.contains("No Council round yet"), "{}", nudge.prompt);
    let (run, _rx) = claim(&state, nudge);
    finish(&state, &run, "Still done.", true);
    assert!(next_action_state(&state, &mut |_| {}).unwrap().is_none());
    let team = saved(&state, &sid);
    assert_eq!(team.status, "paused");
    assert!(team.error.as_deref().unwrap().contains("No Council round yet"), "{:?}", team.error);
}

#[test]
fn a_finish_whose_own_turn_then_records_changes_is_reopened() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let lead = objective_turn(&state, &sid);
    call(&state, &sid, "pandacode_finish_team", json!({"summary":"Done."})).unwrap();
    // The turn's changes are saved as it ends, after the finish call.
    turn_change(&state, &sid, "solo", storage::now(), &["index.html"], &[]);
    finish(&state, &lead, "Done.", true);
    let team = saved(&state, &sid);
    assert_eq!(team.status, "waiting");
    assert!(team.error.as_deref().unwrap().contains("No Council round yet"));
    assert_eq!(next_action_state(&state, &mut |_| {}).unwrap().unwrap().sid, sid, "the lead reviews it");
}

#[test]
fn rounds_are_timed_from_each_seats_first_run() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "ux")]})).unwrap();
    // A writer that settles after the reviewer was dispatched but before it ran.
    let mut team = saved(&state, &sid);
    team.workers.push(writer("w", storage::now() + 1));
    store_team(&state, &mut team);
    std::thread::sleep(std::time::Duration::from_millis(3));
    let (run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let team = saved(&state, &sid);
    let started = team.workers[0].council.as_ref().unwrap().started_at;
    assert!(started > team.workers[1].wrote_at, "{started}");
    assert!(serde_json::to_value(&team).unwrap()["workers"][0]["council"]["startedAt"].is_i64());
    assert!(!team.council_status.unwrap().stale);
    finish(&state, &run, "Council score: 9/10", true);
    assert_eq!(saved(&state, &sid).workers[0].council.as_ref().unwrap().started_at, started, "only the first run counts");
}

#[test]
fn failing_over_clears_a_reviewers_nudge_summary_and_score() {
    let (state, sid, _run) = failover_state("gpt-5", &["codex", "cline"]);
    let store = state.store.lock().unwrap();
    let mut team = load(&store, &sid).unwrap().unwrap();
    team.workers[0].status = "failed".into();
    team.workers[0].score_nudged = true;
    team.workers[0].summary = "The old session's report.".into();
    team.workers[0].council = Some(CouncilSeat { round: 1, lens: "ux".into(), score: Some(3.0), started_at: 1, closed_by_selection: false });
    assert!(fail_over(&store, &mut team, 0, "harness error").unwrap());
    assert!(!team.workers[0].score_nudged);
    assert!(team.workers[0].summary.is_empty());
    assert_eq!(team.workers[0].council.as_ref().unwrap().score, None);
}

#[test]
fn a_failed_reviewer_turn_says_cancel_and_dispatch_fresh() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "ux"), assignment("w", "fast", "src")]})).unwrap();
    let (reviewer, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let (_writer, _rw) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    finish(&state, &reviewer, "Crashed.", false);
    let turn = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert!(turn.prompt.contains("cancel it (it has no score) and dispatch a fresh reviewer"), "{}", turn.prompt);
    let (_, plain) = fixture();
    assert!(!failed_body(&plain).contains("Council"));
}

#[test]
fn the_review_payload_has_first_scores_per_lens() {
    let mut first = seat("a1", 1, Some(6.5), SINCE + 1);
    first.council.as_mut().unwrap().lens = "ux".into();
    let mut later = seat("a2", 2, Some(9.0), SINCE + 5);
    later.council.as_mut().unwrap().lens = "ux".into();
    let mut security = seat("b2", 2, Some(9.5), SINCE + 5);
    security.council.as_mut().unwrap().lens = "security".into();
    let team = team_with(on(9.0, 0, 0), vec![first, seat("x", 1, Some(7.5), SINCE + 1), later, security]);
    let council = review(&team).unwrap();
    assert_eq!(council["firstScores"], json!({"ux":6.5,"lens x":7.5,"security":9.5}));
}

#[test]
fn council_status_follows_the_gate() {
    let state_of = |team: &TeamSnapshot| council::status(team).unwrap().state;
    assert_eq!(state_of(&team_with(on(9.0, 0, 0), vec![writer("w", SINCE)])), "waiting");
    let mut open = seat("b", 1, None, SINCE + 1);
    open.status = "running".into();
    assert_eq!(state_of(&team_with(on(9.0, 0, 0), vec![seat("a", 1, Some(9.0), SINCE + 1), open])), "reviewing");
    let pair = |x: f64| vec![writer("w", SINCE), seat("a", 1, Some(x), SINCE + 1), seat("b", 1, Some(9.5), SINCE + 1)];
    assert_eq!(state_of(&team_with(on(9.0, 0, 0), pair(9.0))), "passed");
    assert_eq!(state_of(&team_with(on(9.0, 0, 0), pair(8.0))), "fixing");
    assert_eq!(state_of(&team_with(on(9.0, 1, 0), pair(8.0))), "limit");
    let status = council::status(&team_with(on(9.5, 0, 0), pair(8.0))).unwrap();
    assert_eq!(status, CouncilStatus { state: "fixing".into(), round: 1, lowest: Some(8.0), target: 9.5, stale: false, notice: None });
    assert_eq!(serde_json::to_value(&status).unwrap(), json!({"state":"fixing","round":1,"lowest":8.0,"target":9.5,"stale":false}));
    // Saved and emitted snapshots carry it while Council is on, and drop it when it is off.
    let (state, sid) = council_state(on(9.0, 0, 0));
    assert_eq!(saved(&state, &sid).council_status.unwrap().state, "waiting");
    assert_eq!(event_payload(&saved(&state, &sid)).council_status.unwrap().state, "waiting");
    council::set(&state, &sid, None).unwrap();
    assert!(serde_json::to_value(saved(&state, &sid)).unwrap().get("councilStatus").is_none());
}

#[test]
fn enabling_during_a_request_still_reviews_its_settled_writes() {
    let (state, sid) = state_fixture();
    let (run, _) = running_worker(&state, &sid, "w", "src");
    finish(&state, &run, "Done.", true);
    let since = saved(&state, &sid).council_since;
    let team = council::set(&state, &sid, Some(on(9.0, 0, 0))).unwrap();
    assert_eq!(team.council_since, since, "the running request keeps its start");
    assert!(finish_refusal(&team).unwrap().contains("No Council round yet"));
}

/// A complete round 1 below the target, after a writer: the lead was told to fix it or ask the user.
fn low_round(state: &AppState, sid: &str) {
    let mut team = saved(state, sid);
    team.workers = vec![writer("w", storage::now() - 50), seat("a", 1, Some(5.0), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(state, &mut team);
}

#[test]
fn a_low_round_gets_one_continue_then_asks_the_user() {
    // The user's own turn.
    let (state, sid) = council_state(on(9.0, 0, 0));
    let lead = objective_turn(&state, &sid);
    low_round(&state, &sid);
    finish(&state, &lead, "Should I keep fixing the parser?", true);
    let nudge = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert!(nudge.prompt.contains("lowest 5") && nudge.prompt.contains("Fix the blockers through workers and convene a new round"), "{}", nudge.prompt);
    let (run, _r) = claim(&state, nudge);
    finish(&state, &run, "Asking the user.", true);
    assert!(next_action_state(&state, &mut |_| {}).unwrap().is_none(), "one Continue only");
    let team = saved(&state, &sid);
    assert_eq!(team.status, "ready");
    assert!(team.error.as_deref().unwrap().contains("lowest 5"), "{:?}", team.error);
    // A turn PandaCode started.
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[assignment("x", "fast", "docs")]})).unwrap();
    let (worker, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    finish(&state, &worker, "Done.", true);
    let (review, _rr) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let mut team = saved(&state, &sid);
    team.workers.extend([writer("w", storage::now() - 50), seat("a", 1, Some(5.0), storage::now()), seat("b", 1, Some(9.0), storage::now())]);
    store_team(&state, &mut team);
    finish(&state, &review, "Asking the user.", true);
    let nudge = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert!(nudge.prompt.contains("only if a remaining blocker should not be fixed"), "{}", nudge.prompt);
    let (run, _r) = claim(&state, nudge);
    finish(&state, &run, "Asking the user.", true);
    assert!(next_action_state(&state, &mut |_| {}).unwrap().is_none());
    let team = saved(&state, &sid);
    assert_eq!(team.status, "ready");
    assert!(team.error.as_deref().unwrap().contains("lower the target"));
}

#[test]
fn a_reopened_finish_from_a_review_turn_says_why_and_keeps_its_error() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[assignment("x", "fast", "docs")]})).unwrap();
    let (worker, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    finish(&state, &worker, "Done.", true);
    // The review turn: Council passes, the lead finishes, then its own edit is saved as the turn ends.
    let (review, _rr) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    assert!(saved(&state, &sid).last_turn_kind.is_some());
    let mut team = saved(&state, &sid);
    team.workers.extend([seat("a", 1, Some(9.0), storage::now() + 5), seat("b", 1, Some(9.0), storage::now() + 5)]);
    store_team(&state, &mut team);
    call(&state, &sid, "pandacode_finish_team", json!({"summary":"Done."})).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    persist_turn(&state, &sid, &["index.html"]);
    finish(&state, &review, "Done.", true);
    let refusal = saved(&state, &sid).error.unwrap();
    assert!(refusal.contains("out of date"), "{refusal}");
    let again = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert!(again.prompt.contains("Your finish was reopened") && again.prompt.contains("out of date"), "{}", again.prompt);
    assert_eq!(saved(&state, &sid).error.as_deref(), Some(refusal.as_str()));
    // It ends that turn without acting: the next one names the refusal, not a missing dispatch.
    let (run, _r) = claim(&state, again);
    finish(&state, &run, "Hmm.", true);
    let next = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert!(next.prompt.contains("Council has not passed this work") && !next.prompt.contains("without dispatching"), "{}", next.prompt);
    assert_eq!(saved(&state, &sid).error.as_deref(), Some(refusal.as_str()));
}

#[test]
fn a_seat_awaiting_its_score_cannot_be_cancelled_or_replaced() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    let mut nudged = seat("n", 1, None, storage::now());
    nudged.status = "queued".into();
    nudged.score_nudged = true;
    team.workers = vec![nudged];
    store_team(&state, &mut team);
    assert_eq!(call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":"n","reason":"Slow."})).unwrap_err(), council::NUDGE_PENDING);
    let mut team = saved(&state, &sid);
    team.workers[0].status = "failed".into();
    store_team(&state, &mut team);
    // Settled after two misses, it is replaced as its error says.
    let mut again = reviewer_assignment("again", "lens n");
    again["replaces"] = json!("n");
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[again]})).unwrap();
}

#[test]
fn council_is_never_a_saved_default() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut with = config();
    with.council = Some(on(7.0, 0, 0));
    configure_state(&state, &sid, Some(with.clone()), None, None).unwrap();
    let store = state.store.lock().unwrap();
    assert_eq!(profiles::read(&store).unwrap().last_setup.unwrap().config.council, None);
}

#[test]
fn enabling_on_a_paused_request_keeps_its_writers_under_review() {
    let (state, sid) = state_fixture();
    let (run, _) = running_worker(&state, &sid, "w", "src");
    finish(&state, &run, "Done.", true);
    control_state(&state, &sid, "pause").unwrap();
    let since = saved(&state, &sid).council_since;
    let team = council::set(&state, &sid, Some(on(9.0, 0, 0))).unwrap();
    assert_eq!(team.council_since, since);
    assert!(finish_refusal(&team).unwrap().contains("No Council round yet"));
}

#[test]
fn an_overtaken_rounds_unscored_seats_stop() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r1", "ux")]})).unwrap();
    let (_run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let mut team = saved(&state, &sid);
    team.workers.push(writer("w", storage::now() + 5));
    store_team(&state, &mut team);
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r2", "ux")]})).unwrap();
    let team = saved(&state, &sid);
    let old = &team.workers[0];
    assert_eq!((old.status.as_str(), old.abandoned), ("stopped", true));
    assert!(old.error.as_deref().unwrap().contains("round 2 replaces it"), "{:?}", old.error);
    assert_eq!(team.workers[2].council.as_ref().unwrap().round, 2);
    assert_eq!(council::status(&team).unwrap().round, 2, "the stopped seat blocks nothing");
}

#[test]
fn a_lens_below_target_must_sit_again() {
    let mut workers = vec![writer("w", SINCE), lensed("a1", 1, Some(5.0), SINCE + 1, "ux"), lensed("b1", 1, Some(9.5), SINCE + 1, "sec"), writer("fix", SINCE + 2)];
    workers.extend([lensed("c2", 2, Some(9.5), SINCE + 3, "perf"), lensed("d2", 2, Some(9.5), SINCE + 3, "sec")]);
    let team = team_with(on(9.0, 0, 0), workers.clone());
    let refusal = finish_refusal(&team).unwrap();
    assert!(refusal.contains("not complete") && refusal.contains("ux") && !refusal.contains("sec"), "{refusal}");
    workers.push(lensed("a2", 2, Some(9.2), SINCE + 3, "ux"));
    assert_eq!(finish_refusal(&team_with(on(9.0, 0, 0), workers)), None);
}

#[test]
fn a_setting_that_lifts_the_refusal_clears_its_error() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    team.status = "ready".into();
    team.workers = vec![writer("w", storage::now() - 50), seat("a", 1, Some(5.0), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(&state, &mut team);
    let mut team = saved(&state, &sid);
    team.error = finish_refusal(&team);
    store_team(&state, &mut team);
    assert!(council::set(&state, &sid, Some(on(8.0, 0, 0))).unwrap().error.unwrap().contains("target 8"), "still below 8, and says so");
    assert_eq!(council::set(&state, &sid, Some(on(5.0, 0, 0))).unwrap().error, None);
    // An unrelated error stays.
    let mut team = saved(&state, &sid);
    team.error = Some("Team paused.".into());
    store_team(&state, &mut team);
    assert_eq!(council::set(&state, &sid, None).unwrap().error.as_deref(), Some("Team paused."));
}

#[test]
fn a_lead_change_is_dated_by_its_last_write() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let wrote = storage::now();
    std::thread::sleep(std::time::Duration::from_millis(3));
    let mut team = saved(&state, &sid);
    team.workers = vec![seat("a", 1, Some(9.0), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(&state, &mut team);
    std::thread::sleep(std::time::Duration::from_millis(3));
    // The lead edited, then convened, and its turn ended after the round started.
    let summary = crate::turn_changes::TurnChanges { id: uuid::Uuid::new_v4().to_string(), files: vec![crate::turn_changes::ChangedFile { path: "index.html".into(), old_path: None, status: "modified".into(), additions: Some(1), deletions: Some(0), binary: false, truncated: false }], additions: 1, deletions: 0, truncated: false, concurrent: false, attributed: true, error: None };
    let detail = crate::turn_changes::TurnChangeDiff { path: "index.html".into(), old_path: None, patch: "+x\n".into(), binary: false, truncated: false, error: None };
    crate::turn_changes::persist_dated(&state.store.lock().unwrap(), &sid, crate::turn_changes::Captured { summary, details: vec![detail] }, wrote).unwrap();
    store_team(&state, &mut saved(&state, &sid));
    assert_eq!(saved(&state, &sid).changed_at, wrote);
    assert_eq!(finish_refusal(&saved(&state, &sid)), None, "the round saw that edit");
}

#[test]
fn stopping_marks_only_a_live_writer() {
    let mut parked = writer("p", 5);
    parked.status = "parked".into();
    council::note_stopped(&mut parked);
    assert_eq!(parked.wrote_at, 5);
    let mut stopped = writer("s", 5);
    stopped.status = "stopped".into();
    council::note_stopped(&mut stopped);
    assert_eq!(stopped.wrote_at, 5);
    // Pausing a live writer marks it: its run may have written until then.
    let (state, sid) = council_state(on(9.0, 0, 0));
    let (_run, _) = running_worker(&state, &sid, "w", "src");
    let mut team = saved(&state, &sid);
    team.workers[0].wrote_at = 5;
    store_team(&state, &mut team);
    control_state(&state, &sid, "pause").unwrap();
    assert!(saved(&state, &sid).workers[0].wrote_at > 5);
}

#[test]
fn an_overtaken_parked_seat_settles_and_never_relaunches() {
    let mut parked = seat("p", 1, None, SINCE + 1);
    parked.status = "parked".into();
    let mut team = team_with(on(9.0, 0, 0), vec![parked, writer("w", SINCE + 2)]);
    assert!(council::overtake(&mut team, 2).is_empty(), "a parked seat has no run to stop");
    assert_eq!((team.workers[0].status.as_str(), team.workers[0].abandoned), ("stopped", true));
    // An abandoned worker is never launched or resumed.
    let mut team = team_with(on(9.0, 0, 0), vec![seat("q", 1, None, SINCE + 1), seat("r", 1, None, SINCE + 1)]);
    team.workers[0].status = "queued".into();
    team.workers[0].abandoned = true;
    team.workers[1].status = "parked".into();
    team.workers[1].abandoned = true;
    assert!(!eligible(&team, 0));
    assert!(!resumable(&team, 1));
}

#[test]
fn overtaken_seats_stop_only_when_the_batch_commits() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r1", "ux")]})).unwrap();
    let (_run, mut rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let mut team = saved(&state, &sid);
    team.workers.push(writer("w", storage::now() + 5));
    store_team(&state, &mut team);
    // A batch refused at its second assignment stops nothing.
    let bad = json!({"assignments":[reviewer_assignment("r2", "ux"), {"key":"bad","name":"b","prompt":"p"}]});
    assert!(call(&state, &sid, "pandacode_dispatch_workers", bad).is_err());
    assert!(rx.try_recv().is_err());
    assert_eq!(saved(&state, &sid).workers[0].status, "running");
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r3", "ux")]})).unwrap();
    assert!(matches!(rx.try_recv(), Ok(RunInput::Stop)));
}

#[test]
fn turning_council_off_stops_its_running_reviewers() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r1", "ux")]})).unwrap();
    let (_run, mut rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let team = council::set(&state, &sid, None).unwrap();
    assert_eq!((team.workers[0].status.as_str(), team.workers[0].abandoned), ("stopped", true));
    assert!(team.workers[0].error.as_deref().unwrap().contains("turned off"));
    assert!(matches!(rx.try_recv(), Ok(RunInput::Stop)));
}

#[test]
fn round_completeness_is_computed_once_per_round() {
    // Forty overtaken rounds of one seat each: a recursive check would take exponential time.
    let mut workers = vec![];
    for round in 1..=40 {
        workers.push(seat(&format!("s{round}"), round, Some(9.0), SINCE + round as i64 * 2));
        workers.push(writer(&format!("w{round}"), SINCE + round as i64 * 2 + 1));
    }
    let team = team_with(on(9.0, 0, 0), workers);
    let started = std::time::Instant::now();
    assert_eq!(council::status(&team).unwrap().state, "dispatch", "no seat is live");
    assert!(started.elapsed() < std::time::Duration::from_secs(1), "{:?}", started.elapsed());
}

#[test]
fn a_reviewers_sub_workers_do_not_outdate_its_round() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "ux")]})).unwrap();
    let (run, actor) = {
        let (run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
        let team = saved(&state, &sid);
        (run, coordination::Actor::resolve(&team, &team.workers[0].session_id).unwrap())
    };
    let coordination::Actor::Worker { id, attempt } = actor else { panic!("a worker") };
    let generation = saved(&state, &sid).generation;
    dispatch_as(&state, &sid, generation, Some((&id, attempt)), "pandacode_dispatch_workers", json!({"assignments":[{"key":"render","profileId":"fast","name":"render","prompt":"Render frames.","files":[],"readOnly":true}]}), &AtomicBool::new(false), &routing::AutoRoutes::default(), None).unwrap();
    let child = saved(&state, &sid).workers[1].session_id.clone();
    let mut team = saved(&state, &sid);
    team.workers[0].council.as_mut().unwrap().started_at = storage::now() - 100;
    team.workers.push(seat("b", 1, Some(9.0), storage::now()));
    store_team(&state, &mut team);
    turn_change(&state, &child, "frames", storage::now(), &["frames/001.png"], &[]);
    store_team(&state, &mut saved(&state, &sid));
    assert!(!saved(&state, &sid).council_status.unwrap().stale);
    drop(run);
}

/// A team resting while its lead asks the user about a round below the target.
fn resting_asking() -> (AppState, String) {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let lead = objective_turn(&state, &sid);
    low_round(&state, &sid);
    finish(&state, &lead, "Should I keep fixing the parser?", true);
    let (run, _r) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    finish(&state, &run, "Asking the user.", true);
    assert!(next_action_state(&state, &mut |_| {}).unwrap().is_none());
    assert_eq!(saved(&state, &sid).council_status.unwrap().state, "asking");
    (state, sid)
}

#[test]
fn lifting_the_refusal_wakes_the_lead_once_to_finish() {
    let (state, sid) = resting_asking();
    council::set(&state, &sid, Some(on(5.0, 0, 0))).unwrap();
    let turn = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert_eq!(turn.sid, sid);
    assert!(turn.prompt.contains("Verify the work and call finish_team") && turn.prompt.contains("first and final score"), "{}", turn.prompt);
    assert!(next_action_state(&state, &mut |_| {}).unwrap().is_none(), "once");
    // Turning Council off asks for no table; a refusal still standing wakes nobody.
    let (state, sid) = resting_asking();
    council::set(&state, &sid, Some(on(8.0, 0, 0))).unwrap();
    assert!(next_action_state(&state, &mut |_| {}).unwrap().is_none(), "5 is still below 8");
    council::set(&state, &sid, None).unwrap();
    let turn = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    // Accept and finish: the council block is gone, so the scores come in the prompt.
    assert!(turn.prompt.contains("first and final score (lens a: first 5, final 5; lens b: first 9, final 9)"), "{}", turn.prompt);
}

#[test]
fn a_low_round_without_new_writers_still_brings_the_lead_back_then_asks() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let lead = objective_turn(&state, &sid);
    // The cycle's only writer ran before it: a user message answered without delegating.
    let mut team = saved(&state, &sid);
    team.workers = vec![seat("a", 1, Some(5.0), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(&state, &mut team);
    finish(&state, &lead, "It is slow because of X.", true);
    let (run, _r) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    finish(&state, &run, "Asking the user.", true);
    assert!(next_action_state(&state, &mut |_| {}).unwrap().is_none());
    let team = saved(&state, &sid);
    assert_eq!(team.status, "ready");
    assert!(team.error.as_deref().is_some_and(|e| e.contains("lowest 5")), "never ready with no error: {:?}", team.error);
}

#[test]
fn every_round_starts_count_toward_the_limit() {
    let config = on(9.0, 2, 0);
    // Round 1 started and was overtaken; round 2 opens and counts too.
    let overtaken = team_with(config.clone(), vec![seat("a", 1, Some(9.0), SINCE + 1), writer("w", SINCE + 2)]);
    assert_eq!(council::round_for(&overtaken, &config, None).unwrap(), 2);
    let mut both = overtaken.workers.clone();
    both.extend([seat("b", 2, Some(9.0), SINCE + 3), writer("x", SINCE + 4)]);
    let team = team_with(config.clone(), both);
    assert!(council::round_for(&team, &config, None).unwrap_err().contains("limit of 2"));
    // At the limit, with nothing live, the lead finishes even though the round is incomplete.
    assert_eq!(finish_refusal(&team), None);
    assert_eq!(council::status(&team).unwrap().state, "limit");
    // A live seat holds the finish until it settles.
    let mut running = team.clone();
    running.workers[2].status = "running".into();
    running.workers[2].council.as_mut().unwrap().score = None;
    assert!(finish_refusal(&running).is_some());
    // A round whose seats have not run yet does not count.
    let mut queued = team_with(config.clone(), vec![seat("q", 1, None, SINCE + 1)]);
    queued.workers[0].status = "queued".into();
    queued.workers[0].council.as_mut().unwrap().started_at = 0;
    assert_eq!(council::round_for(&queued, &config, None).unwrap(), 1);
}

#[test]
fn a_full_round_counts_abandoned_seats_and_gives_way_to_the_next() {
    let mut workers = vec![writer("w", SINCE), seat("a", 1, Some(9.0), SINCE + 1)];
    for i in 0..7 {
        let mut dropped = lensed(&format!("d{i}"), 1, None, SINCE + 1, "lens a");
        dropped.status = "stopped".into();
        dropped.abandoned = true;
        workers.push(dropped);
    }
    let team = team_with(on(9.0, 0, 0), workers);
    // Eight seats, seven dropped: the round has no room, so the next one opens.
    assert_eq!(council::round_for(&team, team.config.council.as_ref().unwrap(), None).unwrap(), 2);
    // Abandoned seats that never scored are out of completeness; only one scored, so it is short.
    assert!(finish_refusal(&team).unwrap().contains("1 scored, at least 2"));
}

#[test]
fn round_one_fixes_the_lenses() {
    let workers = vec![writer("w", SINCE), lensed("u1", 1, Some(9.5), SINCE + 1, "UX "), lensed("s1", 1, Some(7.0), SINCE + 1, "sec"), writer("fix", SINCE + 2)];
    let team = team_with(on(9.0, 0, 0), workers.clone());
    let refused = council::seat_for(&team, Some("perf"), None).unwrap_err();
    assert!(refused.contains("fixed by round 1") && refused.contains("UX") && refused.contains("sec"), "{refused}");
    // Names match without case or spaces.
    assert_eq!(council::seat_for(&team, Some("ux"), None).unwrap().unwrap().round, 2);
    // Every first-round lens sits again, a passing one too.
    let mut next = workers;
    next.push(lensed("s2", 2, Some(9.5), SINCE + 3, "Sec"));
    next.push(lensed("x2", 2, Some(9.5), SINCE + 3, "sec"));
    let refusal = finish_refusal(&team_with(on(9.0, 0, 0), next.clone())).unwrap();
    assert!(refusal.contains("lenses round 1 seated: UX"), "{refusal}");
    next.push(lensed("u2", 2, Some(9.5), SINCE + 3, "ux"));
    assert_eq!(finish_refusal(&team_with(on(9.0, 0, 0), next)), None);
}

#[test]
fn notice_only_while_a_refusal_holds_the_team() {
    let mut low = team_with(on(9.5, 0, 0), vec![writer("w", SINCE), seat("a", 3, Some(8.3), SINCE + 1), seat("b", 3, Some(9.6), SINCE + 1)]);
    let status = council::status(&low).unwrap();
    assert_eq!((status.state.as_str(), status.notice), ("fixing", None));
    low.status = "ready".into();
    low.error = finish_refusal(&low);
    let status = council::status(&low).unwrap();
    assert_eq!(status.state, "asking");
    assert!(serde_json::to_value(&status).unwrap().get("ask").is_none());
    assert_eq!(status.notice.as_deref(), Some("The lead is asking how to continue: lowest score 8.3, target 9.5, after round 3."));
    // Whole scores print with one decimal, and a stop below the target reads the same way.
    let mut whole = team_with(on(9.0, 0, 0), vec![writer("w", SINCE), seat("a", 1, Some(8.0), SINCE + 1), seat("b", 1, Some(9.0), SINCE + 1)]);
    whole.status = "paused".into();
    whole.error = Some(format!("The architect ended without dispatching work or finishing. {} Resume to continue.", finish_refusal(&whole).unwrap()));
    assert_eq!(council::status(&whole).unwrap().notice.as_deref(), Some("The team stopped after Council round 1: lowest score 8.0, target 9.0."));
    whole.status = "ready".into();
    whole.error = finish_refusal(&whole);
    assert_eq!(council::status(&whole).unwrap().notice.as_deref(), Some("The lead is asking how to continue: lowest score 8.0, target 9.0, after round 1."));
    assert!(low.error.unwrap().contains("Accept and finish"));
    assert!(council::lead_rules(&team_with(on(9.0, 0, 0), vec![]), true).contains("Accept and finish"));
}

#[test]
fn a_saved_team_profile_never_carries_council() {
    let (state, _) = state_fixture();
    let store = state.store.lock().unwrap();
    let mut setup_config = config();
    setup_config.council = Some(on(9.0, 0, 0));
    let architect = profiles::ModelSelection { provider: "claude".into(), model: "default".into(), reasoning_effort: None };
    let library = profiles::save_profile(&store, None, "Reviewed".into(), profiles::TeamSetup { architect, config: setup_config }).unwrap();
    assert!(library.profiles.iter().all(|p| p.setup.config.council.is_none()));
}

#[test]
fn lifting_a_refusal_on_a_paused_team_keeps_its_pause_note() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    team.status = "paused".into();
    team.workers = vec![writer("w", storage::now() - 50), seat("a", 1, Some(5.0), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(&state, &mut team);
    let mut team = saved(&state, &sid);
    team.error = Some(format!("The architect ended without dispatching work or finishing. {} Resume to continue.", finish_refusal(&team).unwrap()));
    store_team(&state, &mut team);
    let team = council::set(&state, &sid, Some(on(5.0, 0, 0))).unwrap();
    assert_eq!(team.error.as_deref(), Some("The architect ended without dispatching work or finishing. Resume to continue."));
}

#[test]
fn every_round_one_seat_fixes_its_lens() {
    // A scored, B overtaken without a score: round 2 must seat both again.
    let mut b = lensed("b", 1, None, SINCE + 1, "B");
    b.status = "stopped".into();
    b.abandoned = true;
    let workers = vec![writer("w", SINCE), lensed("a", 1, Some(9.5), SINCE + 1, "A"), b, writer("fix", SINCE + 2), lensed("a2", 2, Some(9.5), SINCE + 3, "A"), lensed("x2", 2, Some(9.5), SINCE + 3, "A")];
    let refusal = finish_refusal(&team_with(on(9.0, 0, 0), workers.clone())).unwrap();
    assert!(refusal.contains("lenses round 1 seated: B"), "{refusal}");
    let mut both = workers;
    both.push(lensed("b2", 2, Some(9.5), SINCE + 3, "B"));
    assert_eq!(finish_refusal(&team_with(on(9.0, 0, 0), both)), None);
}

#[test]
fn reconvening_after_cancelling_every_round_one_seat_works() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r1", "A"), reviewer_assignment("r2", "B")]})).unwrap();
    // Both ran and stopped, then were cancelled; a writer changed the work after.
    let mut team = saved(&state, &sid);
    for w in &mut team.workers {
        w.status = "stopped".into();
        w.council.as_mut().unwrap().started_at = storage::now() - 100;
    }
    store_team(&state, &mut team);
    for id in saved(&state, &sid).workers.iter().map(|w| w.id.clone()).collect::<Vec<_>>() {
        call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":id,"reason":"Wrong brief."})).unwrap();
    }
    let mut team = saved(&state, &sid);
    team.workers.push(writer("w", storage::now()));
    store_team(&state, &mut team);
    // The new round keeps both lenses, and only those.
    assert!(call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r3", "C")]})).unwrap_err().contains("fixed by round 1: A, B"));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r4", "A"), reviewer_assignment("r5", "b")]})).unwrap();
    let team = saved(&state, &sid);
    assert!(team.workers.iter().filter(|w| !w.abandoned && w.council.is_some()).all(|w| w.council.as_ref().unwrap().round == 2));
}

#[test]
fn no_round_one_lenses_refuse_nothing() {
    let team = team_with(on(9.0, 0, 0), vec![seat("only", 2, Some(9.0), SINCE + 1), writer("w", SINCE + 2)]);
    assert!(council::seat_for(&team, Some("anything"), None).unwrap().is_some());
}

#[test]
fn a_running_reviewer_cannot_be_cancelled_or_replaced() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r", "A")]})).unwrap();
    let (_run, _rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let id = saved(&state, &sid).workers[0].id.clone();
    assert_eq!(call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":id,"reason":"Slow."})).unwrap_err(), council::RUNNING_SEAT);
    // Stopped, it can be cancelled.
    let mut team = saved(&state, &sid);
    team.workers[0].status = "stopped".into();
    store_team(&state, &mut team);
    call(&state, &sid, "pandacode_cancel_worker", json!({"workerId":id,"reason":"Stopped."})).unwrap();
}

#[test]
fn a_stopped_seat_is_named_for_cancelling() {
    let mut stopped = lensed("st", 1, None, SINCE + 1, "B");
    stopped.status = "stopped".into();
    let team = team_with(on(9.0, 0, 0), vec![writer("w", SINCE), lensed("a", 1, Some(9.5), SINCE + 1, "A"), stopped]);
    let refusal = finish_refusal(&team).unwrap();
    assert!(refusal.contains("Cancel the stopped reviewers st (st)") && !refusal.contains("wait for"), "{refusal}");
}

#[test]
fn a_full_round_with_a_low_score_cannot_open_a_fresh_one_without_a_change() {
    let mut workers = vec![writer("w", SINCE), seat("a", 1, Some(5.0), SINCE + 1)];
    for i in 0..7 {
        let mut dropped = lensed(&format!("d{i}"), 1, None, SINCE + 1, "lens a");
        dropped.status = "stopped".into();
        dropped.abandoned = true;
        workers.push(dropped);
    }
    let config = on(9.0, 0, 0);
    let team = team_with(config.clone(), workers.clone());
    assert!(council::round_for(&team, &config, None).unwrap_err().contains("nothing changed"));
    workers.push(writer("fix", SINCE + 2));
    assert_eq!(council::round_for(&team_with(config.clone(), workers), &config, None).unwrap(), 2);
}

#[test]
fn a_parked_reviewer_that_started_cannot_be_unseated() {
    let mut parked = seat("p", 1, None, SINCE + 1);
    parked.status = "parked".into();
    assert_eq!(council::refuse_unseat(&parked).unwrap_err(), council::RUNNING_SEAT);
    parked.status = "stopped".into();
    assert!(council::refuse_unseat(&parked).is_ok());
}

#[test]
fn a_reopened_finish_clears_the_continue_mark() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    persist_turn(&state, &sid, &["index.html"]);
    let store = state.store.lock().unwrap();
    let mut team = load(&store, &sid).unwrap().unwrap();
    team.status = "completed".into();
    team.continued_revision = Some(team.dispatch_revision);
    assert!(reopen_for_council(&store, &mut team));
    assert_eq!(team.continued_revision, None, "the lead is woken, not paused as on a second stop");
}

#[test]
fn lifting_the_ask_with_a_refusal_still_standing_says_it() {
    let (state, sid) = resting_asking();
    // More reviewers than the round has: the question is answered, but the round is now short.
    council::set(&state, &sid, Some(on(5.0, 0, 3))).unwrap();
    let turn = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert!(turn.prompt.contains("Council still refuses") && turn.prompt.contains("at least 3 needed"), "{}", turn.prompt);
}

#[test]
fn accepting_finishes_this_cycle_and_keeps_council_on() {
    let (state, sid) = resting_asking();
    assert!(council::accept(&state, "no-such-task").is_err());
    let team = council::accept(&state, &sid).unwrap();
    assert!(team.config.council.is_some());
    assert_eq!(finish_refusal(&team), None);
    let status = team.council_status.clone().unwrap();
    assert_eq!((status.state.as_str(), status.lowest, status.target), ("accepted", Some(5.0), 9.0));
    assert_eq!(team.error, None, "the refusal is gone from the error");
    let turn = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert!(turn.prompt.contains("accepted the current Council scores") && turn.prompt.contains("lens a: first 5, final 5"), "{}", turn.prompt);
    // The next cycle reviews as usual.
    let mut team = saved(&state, &sid);
    team.status = "ready".into();
    team.pending_turn = None;
    store_team(&state, &mut team);
    std::thread::sleep(std::time::Duration::from_millis(3));
    start_state(&state, &sid, "Next request", None, None).unwrap();
    assert_ne!(saved(&state, &sid).council_status.unwrap().state, "accepted");
    // With Council off there is nothing to accept.
    council::set(&state, &sid, None).unwrap();
    assert!(council::accept(&state, &sid).unwrap_err().contains("off"));
}

#[test]
fn a_council_pause_has_a_plain_notice() {
    let mut team = team_with(on(9.0, 0, 0), vec![writer("w", SINCE), seat("a", 1, Some(9.5), SINCE + 1)]);
    team.status = "paused".into();
    team.error = Some(format!("The architect ended without dispatching work or finishing. {} Resume to continue.", finish_refusal(&team).unwrap()));
    assert_eq!(council::status(&team).unwrap().notice.as_deref(), Some("The team stopped with Council round 1 unfinished."));
    team.error = Some("Team paused.".into());
    assert_eq!(council::status(&team).unwrap().notice, None);
}

#[test]
fn a_second_reopen_in_a_cycle_rests_the_team() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    persist_turn(&state, &sid, &["index.html"]);
    let store = state.store.lock().unwrap();
    let mut team = load(&store, &sid).unwrap().unwrap();
    team.status = "completed".into();
    assert!(reopen_for_council(&store, &mut team));
    assert_eq!(team.status, "waiting", "the first reopen wakes the lead");
    team.status = "completed".into();
    assert!(reopen_for_council(&store, &mut team));
    assert_eq!(team.status, "paused");
    assert!(team.error.as_deref().unwrap().contains("No Council round yet"), "{:?}", team.error);
    assert!(council::status(&team).unwrap().notice.is_some());
}

#[test]
fn a_reviewer_that_writes_the_work_outdates_its_round() {
    let (state, sid) = council_state(on(9.0, 0, 0));
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
    let summary = crate::turn_changes::TurnChanges { id: uuid::Uuid::new_v4().to_string(), files: vec![crate::turn_changes::ChangedFile { path: "src/app.rs".into(), old_path: None, status: "modified".into(), additions: Some(1), deletions: Some(0), binary: false, truncated: false }], additions: 1, deletions: 0, truncated: false, concurrent: false, attributed: true, error: None };
    let detail = crate::turn_changes::TurnChangeDiff { path: "src/app.rs".into(), old_path: None, patch: "+x\n".into(), binary: false, truncated: false, error: None };
    crate::turn_changes::persist_dated(&state.store.lock().unwrap(), &reviewer, crate::turn_changes::Captured { summary, details: vec![detail] }, storage::now()).unwrap();
    store_team(&state, &mut saved(&state, &sid));
    assert!(saved(&state, &sid).council_status.unwrap().stale, "a real write by the reviewer counts");
}

#[test]
fn accepting_stops_live_reviewers_and_wakes_only_when_it_lifts_a_refusal() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r1", "ux")]})).unwrap();
    let (_run, mut rx) = claim(&state, next_action_state(&state, &mut |_| {}).unwrap().unwrap());
    let team = council::accept(&state, &sid).unwrap();
    assert_eq!((team.workers[0].status.as_str(), team.workers[0].abandoned), ("stopped", true));
    assert!(matches!(rx.try_recv(), Ok(RunInput::Stop)));
    // A cycle that already passes has nothing to wake the lead for.
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    team.status = "ready".into();
    store_team(&state, &mut team);
    assert_eq!(finish_refusal(&saved(&state, &sid)), None);
    assert!(council::accept(&state, &sid).unwrap().pending_turn.is_none());
}

#[test]
fn dispatch_judges_rounds_on_changes_saved_since_the_last_save() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[assignment("w", "fast", "src")]})).unwrap();
    let writer_session = saved(&state, &sid).workers[0].session_id.clone();
    let mut team = saved(&state, &sid);
    team.workers[0].status = "completed".into();
    team.workers.push(seat("a", 1, Some(9.0), storage::now() - 100));
    store_team(&state, &mut team);
    // The writer's turn is recorded after the round started, with no team save since.
    turn_change(&state, &writer_session, "late", storage::now(), &["src/a.rs"], &[]);
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[reviewer_assignment("r2", "lens a")]})).unwrap();
    assert_eq!(saved(&state, &sid).workers.last().unwrap().council.as_ref().unwrap().round, 2, "round 1 is out of date");
}

#[test]
fn accepting_a_paused_team_lifts_the_refusal_and_leaves_resume_to_the_user() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    let mut team = saved(&state, &sid);
    team.status = "paused".into();
    team.workers = vec![writer("w", storage::now() - 50), seat("a", 1, Some(5.0), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(&state, &mut team);
    let mut team = saved(&state, &sid);
    team.error = Some(format!("The architect ended without dispatching work or finishing. {} Resume to continue.", finish_refusal(&team).unwrap()));
    store_team(&state, &mut team);
    let team = council::accept(&state, &sid).unwrap();
    assert_eq!(team.status, "paused");
    assert!(team.pending_turn.is_none(), "Resume continues it, not a wake");
    assert_eq!(team.error.as_deref(), Some("The architect ended without dispatching work or finishing. Resume to continue."));
}

#[test]
fn at_the_limit_a_round_that_can_still_be_filled_comes_first() {
    let config = on(9.0, 1, 0);
    // Round 1 started and is current, one seat short: the lead fills it before finishing.
    let short = team_with(config.clone(), vec![writer("w", SINCE), seat("a", 1, Some(9.5), SINCE + 1)]);
    assert!(finish_refusal(&short).unwrap().contains("not complete"));
    assert_eq!(council::round_for(&short, &config, None).unwrap(), 1);
    // Full of dropped seats, it cannot be filled: finishing is allowed.
    let mut full = short.workers.clone();
    for i in 0..7 {
        let mut dropped = lensed(&format!("d{i}"), 1, None, SINCE + 1, "lens a");
        dropped.status = "stopped".into();
        dropped.abandoned = true;
        full.push(dropped);
    }
    assert_eq!(finish_refusal(&team_with(config.clone(), full)), None);
    // Complete below the target: finishing is allowed too.
    let mut done = short.workers.clone();
    done.push(seat("b", 1, Some(5.0), SINCE + 1));
    assert_eq!(finish_refusal(&team_with(config, done)), None);
}

#[test]
fn changed_at_reads_only_this_cycles_rows() {
    let (state, sid) = council_state(on(9.0, 0, 0));
    call(&state, &sid, "pandacode_dispatch_workers", json!({"assignments":[assignment("w", "fast", "src")]})).unwrap();
    let writer = saved(&state, &sid).workers[0].session_id.clone();
    // A row written before the lead's last message from before this cycle is old history, whatever its
    // timestamp says (a clock that jumped): it is not read.
    turn_change(&state, &writer, "old", storage::now() + 60_000, &["src/a.rs"], &[]);
    let since = saved(&state, &sid).council_since;
    state.store.lock().unwrap().db.execute("INSERT INTO messages(session_id,role,content,created_at) VALUES(?1,'user','Earlier',?2)", params![sid, since - 1]).unwrap();
    store_team(&state, &mut saved(&state, &sid));
    assert_eq!(saved(&state, &sid).changed_at, 0);
    // A row of this cycle is.
    turn_change(&state, &writer, "new", storage::now(), &["src/b.rs"], &[]);
    store_team(&state, &mut saved(&state, &sid));
    assert!(saved(&state, &sid).changed_at > 0);
}

#[test]
fn raising_the_limit_on_a_resting_team_wakes_the_lead_once() {
    let (state, sid) = council_state(on(9.0, 1, 0));
    let mut team = saved(&state, &sid);
    team.status = "completed".into();
    team.workers = vec![writer("w", storage::now() - 50), seat("a", 1, Some(5.0), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(&state, &mut team);
    let team = saved(&state, &sid);
    assert_eq!((team.council_status.unwrap().state.as_str(), finish_refusal(&saved(&state, &sid))), ("limit", None));
    // The same setting again changes nothing.
    assert!(council::set(&state, &sid, Some(on(9.0, 1, 0))).unwrap().pending_turn.is_none());
    let team = council::set(&state, &sid, Some(on(9.0, 2, 0))).unwrap();
    assert_eq!(team.status, "running");
    assert_eq!(team.council_status.as_ref().unwrap().state, "fixing");
    let turn = next_action_state(&state, &mut |_| {}).unwrap().unwrap();
    assert!(turn.prompt.starts_with("The user raised the Council round limit to 2, so Council refuses again: Council round 1: lowest 5, target 9."), "{}", turn.prompt);
    // A passed council has nothing to reopen.
    let (state, sid) = council_state(on(9.0, 1, 0));
    let mut team = saved(&state, &sid);
    team.status = "completed".into();
    team.workers = vec![writer("w", storage::now() - 50), seat("a", 1, Some(9.5), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(&state, &mut team);
    let team = council::set(&state, &sid, Some(on(9.0, 2, 0))).unwrap();
    assert_eq!((team.status.as_str(), team.pending_turn.is_none()), ("completed", true));
    // A paused team is the user's to resume: the raise wakes nobody.
    let (state, sid) = council_state(on(9.0, 1, 0));
    let mut team = saved(&state, &sid);
    team.status = "paused".into();
    team.workers = vec![writer("w", storage::now() - 50), seat("a", 1, Some(5.0), storage::now()), seat("b", 1, Some(9.0), storage::now())];
    store_team(&state, &mut team);
    let team = council::set(&state, &sid, Some(on(9.0, 2, 0))).unwrap();
    assert_eq!((team.status.as_str(), team.pending_turn.is_none()), ("paused", true));
}

#[test]
fn concurrent_roots_freeze_the_same_nested_bot_and_keep_child_approval_task_scoped() {
    use crate::skills::{Kind, Library, Source};
    use council::AutomaticReviewerCount as Count;

    const BOT_ID: &str = "shared-reviewer";
    struct NestedTask {
        root_id: String,
        child_id: String,
        parent_seat_id: String,
        binding: reviewer_security::ReviewerBinding,
        link: CouncilChildLink,
    }

    let (state, root_a_id) = super::super::state_fixture();
    let now = storage::now().max(2);
    let temp = std::env::temp_dir().join(format!("pandacode-council-isolation-{}", storage::id()));
    let library = Library::new(temp.join("library"), temp.join("home"));
    let import_guide = |instructions: &str| {
        library.import(
            Kind::Skill,
            vec![("SKILL.md".into(), format!(
                "---\nname: Review guide\ndescription: Task-scoped review guidance.\n---\n{instructions}\n"
            ).into_bytes())],
            "Review guide",
            Source { kind: "folder".into(), location: "disposable-test-fixture".into(), git_ref: None, commit: None },
        ).unwrap();
    };
    import_guide("Read the original task evidence.");

    let task_config = |authorization_id: &str, path: &str| {
        let mut plan =
            super::composition(true, Count::Fixed(1), vec![super::nested_reviewer(BOT_ID)]);
        plan.limits.max_estimated_cost_micros = None;
        plan.criteria.push(council::CouncilCriterion {
            id: "coverage".into(),
            lens: "coverage".into(),
            description: "Independently verify the task-scoped frozen artifact.".into(),
            required: true,
            target: 9.0,
            evidence_required: false,
            source: council::CouncilCriterionOrigin::Architect,
        });
        let mut config = super::config_for_composition(plan);
        config.review_permissions = Some(reviewer_security::ReviewerPermissionSelection {
            authorization_id: authorization_id.into(),
            grants: vec![reviewer_security::ReviewerCapabilityGrant {
                tool: "pandacode_open_file".into(),
                action: "call".into(),
                action_argument: None,
                resource_argument: "path".into(),
                resource: path.into(),
            }],
        });
        config
    };

    let root_b_id;
    {
        let store = state.store.lock().unwrap();
        let root_session = store.session(&root_a_id).unwrap();
        root_b_id = store
            .create_session(
                &root_session.project_id,
                &root_session.provider,
                &root_session.model,
                &root_session.mode,
            )
            .unwrap()
            .id;
        store.db.execute(
            "INSERT INTO bots(id,name,role,instructions,goals,soul,project_id,provider,model,mode,created_at,updated_at,skills) VALUES(?1,'Shared reviewer','QA','Original reviewer instructions','Original goals','Original persona',?2,'codex','gpt-5-nano','full',1,1,'[\"review-guide\"]')",
            params![BOT_ID, root_session.project_id],
        ).unwrap();
    }
    let mut root_a = saved(&state, &root_a_id);
    root_a.status = "running".into();
    root_a.council_since = now;
    root_a.config.council = Some(task_config("grant-root-a", "src/root-a.rs"));
    store_team(&state, &mut root_a);
    let mut root_b = root_a.clone();
    root_b.session_id = root_b_id.clone();
    root_b.workers.clear();
    root_b.council_since = now + 1;
    root_b.dispatch_revision = 0;
    root_b.revision = 0;
    root_b.config.council = Some(task_config("grant-root-b", "src/root-b.rs"));
    store_team(&state, &mut root_b);

    let install_nested_task = |root_id: &str| -> NestedTask {
        let store = state.store.lock().unwrap();
        let mut root = load(&store, root_id).unwrap().unwrap();
        let nested = root
            .config
            .council
            .as_ref()
            .unwrap()
            .composition
            .as_ref()
            .unwrap()
            .selected[0]
            .nested
            .as_ref()
            .unwrap()
            .clone();
        let selected = bot_reviewers::seat_bot_inner_with_skills(&store, &root, 1, None, &library)
            .unwrap()
            .unwrap();
        let created_at = storage::now().max(root.council_since + 1);
        let mut nested_seat = council::seat_for(&root, Some("nested-security"), None)
            .unwrap()
            .unwrap();
        nested_seat.started_at = created_at;
        let parent_session = store.session(root_id).unwrap();
        let child_session = store
            .create_session(
                &parent_session.project_id,
                &selected.profile.provider,
                &selected.profile.model,
                &parent_session.mode,
            )
            .unwrap();
        let child_id = child_session.id.clone();
        let mut parent_worker = super::super::worker("nested-review", "waiting", &["src"], &[]);
        parent_worker.session_id = child_id.clone();
        parent_worker.name = selected.context.name.clone();
        parent_worker.profile_id = selected.profile.id.clone();
        parent_worker.provider = selected.profile.provider.clone();
        parent_worker.model = selected.profile.model.clone();
        parent_worker.reasoning_effort = selected.profile.reasoning_effort.clone();
        parent_worker.created_at = created_at;
        parent_worker.updated_at = created_at;
        parent_worker.read_only = true;
        parent_worker.council = Some(nested_seat.clone());
        let mut binding = reviewer_security::ReviewerBinding::native(
            &root,
            &parent_worker.id,
            created_at,
            &parent_worker.provider,
            &parent_worker.model,
            &nested_seat,
        );
        binding.reviewer_bot_id = Some(selected.id.clone());
        binding.roster_bot_ids = selected.roster_bot_ids.clone();
        binding.composition_mode = Some("nested".into());
        binding.snapshot.bot_context = Some(selected.context.clone());
        binding.snapshot.tool_policy = selected.tool_policy.clone();
        binding.snapshot.skills = selected.skills.clone();
        parent_worker.reviewer_binding = Some(binding.clone());
        reviewer_security::record_native_authority(&store, root_id, &binding).unwrap();

        let mut auto_seat = council::seat_for(&root, Some("automatic-coverage"), None)
            .unwrap()
            .unwrap();
        auto_seat.started_at = created_at;
        let auto_session = store
            .create_session(
                &parent_session.project_id,
                "codex",
                "gpt-5-nano",
                &parent_session.mode,
            )
            .unwrap();
        let mut automatic = super::super::worker("automatic-review", "waiting", &["src"], &[]);
        automatic.session_id = auto_session.id;
        automatic.provider = "codex".into();
        automatic.model = "gpt-5-nano".into();
        automatic.created_at = created_at;
        automatic.updated_at = created_at;
        automatic.read_only = true;
        automatic.council = Some(auto_seat.clone());
        let mut auto_binding = reviewer_security::ReviewerBinding::native(
            &root,
            &automatic.id,
            created_at,
            &automatic.provider,
            &automatic.model,
            &auto_seat,
        );
        auto_binding.composition_mode = Some("direct".into());
        automatic.reviewer_binding = Some(auto_binding.clone());
        reviewer_security::record_native_authority(&store, root_id, &auto_binding).unwrap();

        store.db.execute(
            "INSERT INTO architect_workers(parent_session_id,worker_id,session_id) VALUES(?1,?2,?3)",
            params![root_id, parent_worker.id, child_id],
        ).unwrap();
        root.workers.extend([parent_worker.clone(), automatic]);
        save(&store, &mut root).unwrap();
        super::super::super::create_nested_child_team(
            &store,
            &root,
            &parent_worker,
            &nested,
            &child_id,
        )
        .unwrap();
        root.dispatch_revision = root.dispatch_revision.saturating_add(1);
        save(&store, &mut root).unwrap();
        let decision = council::NestedEscalationDecision {
            cycle_id: binding.council_cycle_id.clone(),
            seat_id: parent_worker.id.clone(),
            round: 1,
            attempt: 1,
            artifact_revision: binding.artifact_revision.clone(),
            phase: council::NestedEscalationPhase::Child,
            trigger: None,
            preliminary_score: None,
            finalized: true,
            decided_at_ms: created_at as u64,
            child_session_id: Some(child_id.clone()),
        };
        super::super::super::persist_nested_decision(&store, root_id, &decision).unwrap();
        let link = super::super::super::validate_council_child_link(&store, &child_id)
            .unwrap()
            .unwrap();
        assert_eq!(link.binding, binding);
        NestedTask {
            root_id: root_id.into(),
            child_id,
            parent_seat_id: parent_worker.id,
            binding,
            link,
        }
    };

    let task_a = install_nested_task(&root_a_id);
    {
        let store = state.store.lock().unwrap();
        store.db.execute(
            "UPDATE bots SET name='Reviewer version two',instructions='Task B reviewer instructions',goals='Task B goals',soul='Task B persona' WHERE id=?1",
            [BOT_ID],
        ).unwrap();
    }
    import_guide("Read the second task's evidence and use its own policy.");
    let task_b = install_nested_task(&root_b_id);

    assert_ne!(task_a.root_id, task_b.root_id);
    assert_ne!(task_a.child_id, task_b.child_id);
    assert_ne!(
        task_a.link.root_council_cycle_id,
        task_b.link.root_council_cycle_id
    );
    assert_ne!(
        task_a.link.child_council_cycle_id,
        task_b.link.child_council_cycle_id
    );
    assert_eq!(task_a.binding.reviewer_bot_id.as_deref(), Some(BOT_ID));
    assert_eq!(task_b.binding.reviewer_bot_id.as_deref(), Some(BOT_ID));
    assert_eq!(
        task_a
            .binding
            .snapshot
            .bot_context
            .as_ref()
            .unwrap()
            .instructions,
        "Original reviewer instructions"
    );
    assert_eq!(
        task_b
            .binding
            .snapshot
            .bot_context
            .as_ref()
            .unwrap()
            .instructions,
        "Task B reviewer instructions"
    );
    let skills_a = task_a.binding.snapshot.skills.as_ref().unwrap();
    let skills_b = task_b.binding.snapshot.skills.as_ref().unwrap();
    assert!(skills_a.prompt.contains("original task evidence"));
    assert!(skills_b.prompt.contains("second task's evidence"));
    assert_ne!(
        skills_a.pins[0].content_sha256,
        skills_b.pins[0].content_sha256
    );
    assert_ne!(
        task_a.binding.snapshot.tool_policy,
        task_b.binding.snapshot.tool_policy
    );
    assert_ne!(
        super::super::super::council_policy_fingerprint(&task_a.binding).unwrap(),
        super::super::super::council_policy_fingerprint(&task_b.binding).unwrap(),
    );
    assert!(skills_a.verify().is_ok() && skills_b.verify().is_ok());

    {
        let store = state.store.lock().unwrap();
        store.db.execute(
            "UPDATE bots SET name='Mutable profile after admission',instructions='Unrelated live edit',skills='[\"other-skill\"]' WHERE id=?1",
            [BOT_ID],
        ).unwrap();
    }
    import_guide("Live library text changed after both task admissions.");
    {
        let store = state.store.lock().unwrap();
        for task in [&task_a, &task_b] {
            let root = load(&store, &task.root_id).unwrap().unwrap();
            let parent = root
                .workers
                .iter()
                .find(|worker| worker.id == task.parent_seat_id)
                .unwrap();
            let persisted = parent.reviewer_binding.as_ref().unwrap();
            assert_eq!(persisted, &task.binding);
            assert!(
                super::super::super::validate_council_child_link(&store, &task.child_id)
                    .unwrap()
                    .is_some()
            );
            reviewer_security::validate_issued_authority(&store, &task.root_id, persisted).unwrap();
            persisted
                .snapshot
                .skills
                .as_ref()
                .unwrap()
                .verify()
                .unwrap();
        }
        assert_eq!(
            crate::bots::get(&store, BOT_ID).unwrap().instructions,
            "Unrelated live edit"
        );
    }

    let settle_nested_task = |task: &NestedTask, child_scores: [f64; 2], automatic_score: f64| {
        let store = state.store.lock().unwrap();
        let mut root = load(&store, &task.root_id).unwrap().unwrap();
        let link = super::super::super::validate_council_child_link(&store, &task.child_id)
            .unwrap()
            .unwrap();
        let mut child = load(&store, &task.child_id).unwrap().unwrap();
        let child_cycle_id = format!("{}:{}", child.session_id, child.council_since);
        let root_cycle_id = format!("{}:{}", root.session_id, root.council_since);
        let mut leaves = Vec::new();
        for (index, score) in child_scores.into_iter().enumerate() {
            let id = format!("{}-leaf-{index}", task.root_id);
            let at = child.council_since + 1 + index as i64;
            let seat = council::CouncilSeat {
                round: 1,
                lens: format!("child-lens-{index}"),
                score: Some(score),
                started_at: at,
                closed_by_selection: false,
            };
            let mut leaf = super::super::worker(&id, "completed", &["src"], &[]);
            leaf.profile_id = format!("local-reviewer-{index}");
            leaf.provider = "codex".into();
            leaf.model = "gpt-5-nano".into();
            let child_session = store.session(&task.child_id).unwrap();
            leaf.session_id = store.create_session(
                &child_session.project_id, &leaf.provider, &leaf.model, &child_session.mode,
            ).unwrap().id;
            leaf.created_at = at;
            leaf.updated_at = at;
            leaf.result_revision = index as u64 + 1;
            leaf.read_only = true;
            leaf.council = Some(seat.clone());
            let mut binding = reviewer_security::ReviewerBinding::native(
                &child,
                &leaf.id,
                at,
                &leaf.provider,
                &leaf.model,
                &seat,
            );
            binding.composition_mode = Some("direct".into());
            binding.parent_council = Some(reviewer_security::ReviewerParentBinding {
                session_id: root.session_id.clone(),
                council_cycle_id: root_cycle_id.clone(),
                artifact_revision: link.root_artifact_revision.clone(),
                seat_id: task.parent_seat_id.clone(),
                depth: 1,
            });
            reviewer_security::record_native_authority(&store, &task.child_id, &binding).unwrap();
            leaf.reviewer_binding = Some(binding);
            leaves.push(leaf);
        }
        child.workers = leaves;
        child.status = "completed".into();
        child.dispatch_revision = child.workers.len() as u64;
        save(&store, &mut child).unwrap();
        for leaf in &child.workers {
            let score = leaf.council.as_ref().unwrap().score.unwrap();
            store.message(&leaf.session_id, "assistant", &format!("Council score: {score}/10")).unwrap();
            super::super::super::persist_council_terminal_receipts(&store, &child, leaf).unwrap();
            let binding = leaf.reviewer_binding.as_ref().unwrap();
            for criterion in &child.config.council.as_ref().unwrap().composition.as_ref().unwrap().criteria {
                let outcome = council::CouncilCriterionOutcome { criterion_id: criterion.id.clone(),
                    score, verified_evidence_refs: Vec::new() };
                let local = council::CouncilCriterionOutcomeReceipt {
                    schema_version: 1, root_cycle_id: child_cycle_id.clone(),
                    task_id: child.session_id.clone(), task_cycle_id: child_cycle_id.clone(),
                    child_cycle_id: None, round: 1, seat_id: leaf.id.clone(),
                    parent_seat_id: None, attempt: leaf.attempt,
                    artifact_revision: binding.artifact_revision.clone(), outcome: outcome.clone(),
                };
                local.validate().unwrap();
                super::super::super::persist_council_record(&store, &child.session_id,
                    &format!("native-test-child-criterion:{}:{}", leaf.id, criterion.id),
                    "council_criterion_outcome", &local, &leaf.id).unwrap();
                let root_outcome = council::CouncilCriterionOutcomeReceipt {
                    root_cycle_id: root_cycle_id.clone(), child_cycle_id: Some(child_cycle_id.clone()),
                    parent_seat_id: Some(task.parent_seat_id.clone()),
                    artifact_revision: link.root_artifact_revision.clone(), ..local
                };
                root_outcome.validate().unwrap();
                super::super::super::persist_council_record(&store, &root.session_id,
                    &format!("native-test-root-leaf-criterion:{}:{}", leaf.id, criterion.id),
                    "council_criterion_outcome", &root_outcome, &leaf.id).unwrap();
            }
        }
        let child_status = super::super::super::native_council_status(&store, &child).unwrap();
        assert_eq!(
            child_status.state,
            "passed",
            "{}",
            child_status.notice.unwrap_or_default()
        );
        let receipt =
            super::super::super::create_child_council_receipt(&store, &link, &child).unwrap();

        let parent_index = root
            .workers
            .iter()
            .position(|worker| worker.id == task.parent_seat_id)
            .unwrap();
        root.workers[parent_index].status = "completed".into();
        root.workers[parent_index].updated_at = storage::now();
        root.workers[parent_index].result_revision = 3;
        root.workers[parent_index].council.as_mut().unwrap().score = child_status.lowest;
        let automatic_index = root
            .workers
            .iter()
            .position(|worker| {
                worker.reviewer_binding.as_ref().is_some_and(|binding| {
                    binding.composition_mode.as_deref() == Some("direct")
                        && binding.reviewer_bot_id.is_none()
                })
            })
            .unwrap();
        root.workers[automatic_index].status = "completed".into();
        root.workers[automatic_index].updated_at = storage::now();
        root.workers[automatic_index].result_revision = 4;
        root.workers[automatic_index]
            .council
            .as_mut()
            .unwrap()
            .score = Some(automatic_score);
        store.message(&root.workers[automatic_index].session_id, "assistant",
            &format!("Council score: {automatic_score}/10")).unwrap();
        root.status = "completed".into();
        root.dispatch_revision = 4;
        save(&store, &mut root).unwrap();
        super::super::super::persist_council_terminal_receipts(
            &store,
            &root,
            &root.workers[automatic_index],
        )
        .unwrap();
        let automatic = &root.workers[automatic_index];
        let binding = automatic.reviewer_binding.as_ref().unwrap();
        for criterion in &root.config.council.as_ref().unwrap().composition.as_ref().unwrap().criteria {
            let outcome = council::CouncilCriterionOutcomeReceipt {
                schema_version: 1, root_cycle_id: root_cycle_id.clone(),
                task_id: root.session_id.clone(), task_cycle_id: root_cycle_id.clone(),
                child_cycle_id: None, round: 1, seat_id: automatic.id.clone(),
                parent_seat_id: None, attempt: automatic.attempt,
                artifact_revision: binding.artifact_revision.clone(),
                outcome: council::CouncilCriterionOutcome { criterion_id: criterion.id.clone(),
                    score: automatic_score, verified_evidence_refs: Vec::new() },
            };
            outcome.validate().unwrap();
            super::super::super::persist_council_record(&store, &root.session_id,
                &format!("native-test-automatic-criterion:{}", criterion.id),
                "council_criterion_outcome", &outcome, &automatic.id).unwrap();
        }

        let status = super::super::super::native_council_status(&store, &root).unwrap();
        assert_eq!(
            status.state,
            "passed",
            "{}",
            status.notice.unwrap_or_default()
        );
        assert!(super::super::super::verify_composition_terminal_receipts(
            &store, &root, &status
        ));
        assert_eq!(
            receipt.min_score,
            Some(child_scores[0].min(child_scores[1]))
        );
        assert_eq!(receipt.root_cycle_id, root_cycle_id);
        assert_eq!(receipt.child_cycle_id, child_cycle_id);
        receipt
    };

    assert_eq!(saved(&state, &task_a.root_id).status, "running");
    assert_eq!(saved(&state, &task_b.root_id).status, "running");
    assert_eq!(saved(&state, &task_a.child_id).status, "running");
    assert_eq!(saved(&state, &task_b.child_id).status, "running");
    let receipt_a = settle_nested_task(&task_a, [9.3, 9.8], 9.7);
    {
        let store = state.store.lock().unwrap();
        let root_b = load(&store, &task_b.root_id).unwrap().unwrap();
        let root_a = load(&store, &task_a.root_id).unwrap().unwrap();
        let status_a = super::super::super::native_council_status(&store, &root_a).unwrap();
        assert_eq!(status_a.state, "passed");
        assert!(
            !super::super::super::verify_composition_terminal_receipts(&store, &root_b, &status_a),
            "root A's terminal pass and child receipt cannot approve root B"
        );
        let policy_b = super::super::super::council_policy_fingerprint(&task_b.binding).unwrap();
        assert!(receipt_a
            .validate_for(
                &task_b.link.root_council_cycle_id,
                &task_b.parent_seat_id,
                BOT_ID,
                &task_b.link.root_artifact_revision,
                &policy_b,
            )
            .is_err());
    }
    let receipt_b = settle_nested_task(&task_b, [9.1, 9.6], 9.5);
    assert_ne!(receipt_a.min_score, receipt_b.min_score);
    assert_ne!(receipt_a.root_cycle_id, receipt_b.root_cycle_id);
    assert_ne!(receipt_a.child_cycle_id, receipt_b.child_cycle_id);
    receipt_a
        .validate_for(
            &task_a.link.root_council_cycle_id,
            &task_a.parent_seat_id,
            BOT_ID,
            &task_a.link.root_artifact_revision,
            &super::super::super::council_policy_fingerprint(&task_a.binding).unwrap(),
        )
        .unwrap();
    receipt_b
        .validate_for(
            &task_b.link.root_council_cycle_id,
            &task_b.parent_seat_id,
            BOT_ID,
            &task_b.link.root_artifact_revision,
            &super::super::super::council_policy_fingerprint(&task_b.binding).unwrap(),
        )
        .unwrap();
    {
        let store = state.store.lock().unwrap();
        for task in [&task_a, &task_b] {
            let root = load(&store, &task.root_id).unwrap().unwrap();
            let status = super::super::super::native_council_status(&store, &root).unwrap();
            assert_eq!(status.state, "passed");
            assert!(super::super::super::verify_composition_terminal_receipts(
                &store, &root, &status
            ));
            let ledger = super::super::super::council_ledger_projection(&store, &root);
            assert_eq!(ledger.council_child_receipts.len(), 1);
            assert_eq!(
                ledger.council_child_receipts[0].root_cycle_id,
                task.link.root_council_cycle_id
            );
            assert_eq!(
                ledger.council_child_receipts[0].child_cycle_id,
                task.link.child_council_cycle_id
            );
            assert_eq!(ledger.council_child_leaves.len(), 2);
            assert!(ledger
                .council_child_leaves
                .iter()
                .all(|score| score.root_cycle_id == task.link.root_council_cycle_id));
            assert!(ledger
                .council_usage
                .iter()
                .all(|usage| usage.root_cycle_id == task.link.root_council_cycle_id));
            assert!(ledger
                .council_usage
                .iter()
                .any(|usage| usage.scope == council::CouncilUsageScope::Child));
        }
    }
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn native_concurrent_roots_dispatch_and_settle_the_same_bot_without_state_leakage() {
    use crate::skills::{Kind, Library, Source};
    use council::SelectedReviewerMode as Mode;

    const BOT_ID: &str = "ac22-shared-reviewer";
    const SKILL_ID: &str = "ac22-review-guide";

    struct Task {
        root_id: String,
        worker_id: String,
        auto_worker_id: String,
        child_id: String,
        binding: reviewer_security::ReviewerBinding,
        link: CouncilChildLink,
    }

    let (state, root_a_id) = super::super::state_fixture();
    let state = std::sync::Arc::new(state);
    let now = storage::now().max(2);
    let temp = std::env::temp_dir().join(format!("pandacode-ac22-native-{}", storage::id()));
    let library = Library::new(temp.join("library"), temp.join("home"));
    let import_guide = |instructions: &str| {
        library.import(
            Kind::Skill,
            vec![("SKILL.md".into(), format!(
                "---\nname: AC22 Review Guide\ndescription: Task-scoped review guidance.\n---\n{instructions}\n"
            ).into_bytes())],
            "AC22 Review Guide",
            Source { kind: "folder".into(), location: "disposable-ac22-fixture".into(), git_ref: None, commit: None },
        ).unwrap()
    };
    let imported = import_guide("Read only the original task's evidence.");
    assert_eq!(imported.meta.id, SKILL_ID);

    let task_config = |authorization_id: &str, resource: &str| {
        // The required nested bot plus one automatic reviewer form two root seats.
        let mut config = super::super::two_phase_test_config(1, &[(BOT_ID, Mode::Nested)]);
        config.composition.as_mut().unwrap().strategy = council::CouncilStrategy::Parallel;
        assert!(config.validate_composition().unwrap().is_some());
        config.review_permissions = Some(reviewer_security::ReviewerPermissionSelection {
            authorization_id: authorization_id.into(),
            grants: vec![reviewer_security::ReviewerCapabilityGrant {
                tool: "pandacode_open_file".into(),
                action: "call".into(),
                action_argument: None,
                resource_argument: "path".into(),
                resource: resource.into(),
            }],
        });
        config
    };

    let root_b_id = {
        let store = state.store.lock().unwrap();
        let parent = store.session(&root_a_id).unwrap();
        let root_b_id = store
            .create_session(
                &parent.project_id,
                &parent.provider,
                &parent.model,
                &parent.mode,
            )
            .unwrap()
            .id;
        store.db.execute(
            "INSERT INTO bots(id,name,role,instructions,goals,soul,project_id,provider,model,mode,created_at,updated_at,skills) VALUES(?1,'Task A reviewer','QA','Task A reviewer instructions','Task A goals','Task A persona',?2,'codex','gpt-5-nano','full',1,1,'[\"ac22-review-guide\"]')",
            params![BOT_ID, parent.project_id],
        ).unwrap();

        let mut root_a = load(&store, &root_a_id).unwrap().unwrap();
        root_a.status = "running".into();
        root_a.council_since = now;
        root_a.config.council = Some(task_config("ac22-grant-a", "src/root-a.rs"));
        save(&store, &mut root_a).unwrap();

        let mut root_b = root_a.clone();
        root_b.session_id = root_b_id.clone();
        root_b.workers.clear();
        root_b.council_since = now + 1;
        root_b.dispatch_revision = 0;
        root_b.revision = 0;
        root_b.config.council = Some(task_config("ac22-grant-b", "src/root-b.rs"));
        save(&store, &mut root_b).unwrap();
        root_b_id
    };

    let generation_a = load(&state.store.lock().unwrap(), &root_a_id)
        .unwrap()
        .unwrap()
        .generation;
    let generation_b = load(&state.store.lock().unwrap(), &root_b_id)
        .unwrap()
        .unwrap()
        .generation;
    let (lead_a, _lead_a_rx) = claim(
        &state,
        Launch {
            sid: root_a_id.clone(),
            prompt: "Build task A".into(),
            images: None,
            admission: Admission {
                parent: root_a_id.clone(),
                generation: generation_a,
            },
        },
    );
    let (lead_b, _lead_b_rx) = claim(
        &state,
        Launch {
            sid: root_b_id.clone(),
            prompt: "Build task B".into(),
            images: None,
            admission: Admission {
                parent: root_b_id.clone(),
                generation: generation_b,
            },
        },
    );
    assert_eq!(lead_a.config.session_id, root_a_id);
    assert_eq!(lead_b.config.session_id, root_b_id);
    assert_eq!(saved(&state, &root_a_id).status, "running");
    assert_eq!(saved(&state, &root_b_id).status, "running");

    // Seed each task's real frozen reviewer round with the disposable library.
    // The following dispatches use the normal native handler and consume these
    // persisted snapshots, while the test-only path skips provider discovery.
    {
        let store = state.store.lock().unwrap();
        let root_a = load(&store, &root_a_id).unwrap().unwrap();
        let frozen = bot_reviewers::seat_bot_inner_with_skills(&store, &root_a, 1, None, &library)
            .unwrap()
            .unwrap();
        assert_eq!(frozen.context.instructions, "Task A reviewer instructions");
        assert!(frozen
            .skills
            .as_ref()
            .unwrap()
            .prompt
            .contains("original task's evidence"));
    }
    {
        let store = state.store.lock().unwrap();
        store.db.execute(
            "UPDATE bots SET name='Task B reviewer',instructions='Task B reviewer instructions',goals='Task B goals',soul='Task B persona' WHERE id=?1",
            [BOT_ID],
        ).unwrap();
    }
    import_guide("Read only the second task's evidence.");
    {
        let store = state.store.lock().unwrap();
        let root_b = load(&store, &root_b_id).unwrap().unwrap();
        let frozen = bot_reviewers::seat_bot_inner_with_skills(&store, &root_b, 1, None, &library)
            .unwrap()
            .unwrap();
        assert_eq!(frozen.context.instructions, "Task B reviewer instructions");
        assert!(frozen
            .skills
            .as_ref()
            .unwrap()
            .prompt
            .contains("second task's evidence"));
    }

    let root_assignment_a =
        super::super::council_review_assignment("ac22-root-a", None, Some(BOT_ID), "security");
    let root_assignment_b =
        super::super::council_review_assignment("ac22-root-b", None, Some(BOT_ID), "quality");
    let root_auto_assignment_a = super::super::council_review_assignment(
        "ac22-root-a-direct",
        Some("fast"),
        None,
        "coverage",
    );
    let root_auto_assignment_b = super::super::council_review_assignment(
        "ac22-root-b-direct",
        Some("fast"),
        None,
        "coverage",
    );
    let root_worker_ids_a = super::super::dispatch_council_round(
        &state,
        &root_a_id,
        vec![root_assignment_a.clone(), root_auto_assignment_a],
    )
    .unwrap();
    let root_worker_ids_b = super::super::dispatch_council_round(
        &state,
        &root_b_id,
        vec![root_assignment_b.clone(), root_auto_assignment_b],
    )
    .unwrap();
    assert_eq!(root_worker_ids_a.len(), 2);
    assert_eq!(root_worker_ids_b.len(), 2);

    let task_for = |root_id: &str, worker_ids: &[String]| {
        let store = state.store.lock().unwrap();
        let root = load(&store, root_id).unwrap().unwrap();
        let workers = worker_ids
            .iter()
            .map(|worker_id| {
                root.workers
                    .iter()
                    .find(|worker| worker.id == *worker_id)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let worker = workers
            .iter()
            .find(|worker| {
                worker
                    .reviewer_binding
                    .as_ref()
                    .and_then(|binding| binding.composition_mode.as_deref())
                    == Some("nested")
            })
            .unwrap();
        let auto_worker = workers
            .iter()
            .find(|worker| {
                worker
                    .reviewer_binding
                    .as_ref()
                    .and_then(|binding| binding.composition_mode.as_deref())
                    != Some("nested")
            })
            .unwrap();
        let child_id = worker.session_id.clone();
        let link = super::super::super::validate_council_child_link(&store, &child_id)
            .unwrap()
            .unwrap();
        Task {
            root_id: root_id.into(),
            worker_id: worker.id.clone(),
            auto_worker_id: auto_worker.id.clone(),
            child_id,
            binding: worker.reviewer_binding.as_ref().unwrap().clone(),
            link,
        }
    };
    let task_a = task_for(&root_a_id, &root_worker_ids_a);
    let task_b = task_for(&root_b_id, &root_worker_ids_b);
    assert_ne!(task_a.child_id, task_b.child_id);
    assert_ne!(
        task_a.link.root_council_cycle_id,
        task_b.link.root_council_cycle_id
    );
    assert_ne!(
        task_a.link.child_council_cycle_id,
        task_b.link.child_council_cycle_id
    );
    assert_eq!(task_a.binding.reviewer_bot_id.as_deref(), Some(BOT_ID));
    assert_eq!(task_b.binding.reviewer_bot_id.as_deref(), Some(BOT_ID));
    let persona_a = task_a.binding.snapshot.bot_context.as_ref().unwrap();
    let persona_b = task_b.binding.snapshot.bot_context.as_ref().unwrap();
    assert_eq!(
        (
            persona_a.name.as_str(),
            persona_a.role.as_str(),
            persona_a.instructions.as_str(),
            persona_a.goals.as_str(),
            persona_a.soul.as_str()
        ),
        (
            "Task A reviewer",
            "QA",
            "Task A reviewer instructions",
            "Task A goals",
            "Task A persona"
        )
    );
    assert_eq!(
        (
            persona_b.name.as_str(),
            persona_b.role.as_str(),
            persona_b.instructions.as_str(),
            persona_b.goals.as_str(),
            persona_b.soul.as_str()
        ),
        (
            "Task B reviewer",
            "QA",
            "Task B reviewer instructions",
            "Task B goals",
            "Task B persona"
        )
    );
    assert_ne!(
        task_a.binding.snapshot.tool_policy,
        task_b.binding.snapshot.tool_policy
    );
    let policy_identity =
        |binding: &reviewer_security::ReviewerBinding| match &binding.snapshot.tool_policy {
            reviewer_security::ReviewerToolPolicy::Scoped(policy) => Some((
                policy.authorization_id.clone(),
                policy.grants[0].resource.clone(),
            )),
            reviewer_security::ReviewerToolPolicy::NativeReadOnly => None,
        };
    assert_eq!(
        policy_identity(&task_a.binding),
        Some(("ac22-grant-a".to_owned(), "src/root-a.rs".to_owned()))
    );
    assert_eq!(
        policy_identity(&task_b.binding),
        Some(("ac22-grant-b".to_owned(), "src/root-b.rs".to_owned()))
    );
    let skills_a = task_a.binding.snapshot.skills.as_ref().unwrap();
    let skills_b = task_b.binding.snapshot.skills.as_ref().unwrap();
    assert!(skills_a.prompt.contains("original task's evidence"));
    assert!(skills_b.prompt.contains("second task's evidence"));
    assert_ne!(
        skills_a.pins[0].content_sha256,
        skills_b.pins[0].content_sha256
    );
    assert_ne!(skills_a.pins[0].staged_path, skills_b.pins[0].staged_path);
    assert_ne!(
        super::super::super::council_policy_fingerprint(&task_a.binding).unwrap(),
        super::super::super::council_policy_fingerprint(&task_b.binding).unwrap(),
    );

    // Same-key replay returns the already-dispatched native seat. It cannot
    // recreate it from later live bot or library contents.
    let replayed_ids =
        super::super::dispatch_council_round(&state, &root_a_id, vec![root_assignment_a]).unwrap();
    assert_eq!(replayed_ids, vec![task_a.worker_id.clone()],
        "replaying one assignment returns only that existing seat, not unrelated sibling seats");
    assert_eq!(saved(&state, &root_a_id).workers.len(), 2);

    // Mutating both live sources after admission must leave both dispatched
    // tasks' snapshots, child guidance and staged skill files intact.
    {
        let store = state.store.lock().unwrap();
        store.db.execute(
            "UPDATE bots SET name='Mutable live reviewer',instructions='Live profile after admission',goals='Live goals',soul='Live persona',skills='[]' WHERE id=?1",
            [BOT_ID],
        ).unwrap();
    }
    import_guide("Live library text changed after both native admissions.");
    let child_a = saved(&state, &task_a.child_id);
    let child_b = saved(&state, &task_b.child_id);
    assert!(child_a
        .config
        .lead_guidance
        .as_ref()
        .unwrap()
        .contains("Task A reviewer instructions"));
    assert!(child_a
        .config
        .lead_guidance
        .as_ref()
        .unwrap()
        .contains("original task's evidence"));
    assert!(!child_a
        .config
        .lead_guidance
        .as_ref()
        .unwrap()
        .contains("Task B reviewer instructions"));
    assert!(child_b
        .config
        .lead_guidance
        .as_ref()
        .unwrap()
        .contains("Task B reviewer instructions"));
    assert!(child_b
        .config
        .lead_guidance
        .as_ref()
        .unwrap()
        .contains("second task's evidence"));
    assert!(!child_b
        .config
        .lead_guidance
        .as_ref()
        .unwrap()
        .contains("Task A reviewer instructions"));
    skills_a.verify().unwrap();
    skills_b.verify().unwrap();

    // The native scheduler claims both child cycles' leaf reviews and reserves
    // both child coordinator turns before either task is settled. No provider
    // runner is launched by this fixture.
    for task in [&task_a, &task_b] {
        let child_generation = saved(&state, &task.child_id).generation;
        let assignments = (0..2)
            .map(|index| {
                super::super::council_review_assignment(
                    &format!("{}-leaf-{index}", task.root_id),
                    Some("fast"),
                    None,
                    if index == 0 { "security" } else { "coverage" },
                )
            })
            .collect::<Vec<_>>();
        super::super::dispatch_state_runtime_test(
            &state,
            &task.child_id,
            child_generation,
            "pandacode_dispatch_workers",
            json!({"assignments":assignments}),
        )
        .unwrap_or_else(|error| {
            panic!("native child dispatch failed for {}: {error}", task.root_id)
        });
    }
    let mut claimed = Vec::new();
    while let Some(action) = next_action_state(&state, &mut |_| {}).unwrap() {
        claimed.push(action);
        assert!(
            claimed.len() <= 8,
            "only the two child cycles and their leaf reviews should be claimable"
        );
    }
    for task in [&task_a, &task_b] {
        let task_claims = claimed
            .iter()
            .filter(|action| action.admission.parent == task.child_id)
            .collect::<Vec<_>>();
        assert_eq!(
            task_claims.len(),
            3,
            "two child leaves and the coordinator turn are claimed per task"
        );
        assert!(task_claims.iter().any(|action| action.sid == task.child_id));
        let child = saved(&state, &task.child_id);
        assert!(child
            .workers
            .iter()
            .all(|worker| worker.status == "running"));
        assert!(child.pending_turn.as_ref().unwrap().reserved);
    }
    assert_eq!(saved(&state, &task_a.root_id).status, "running");
    assert_eq!(saved(&state, &task_b.root_id).status, "running");
    assert_eq!(saved(&state, &task_a.child_id).status, "running");
    assert_eq!(saved(&state, &task_b.child_id).status, "running");

    // Open and verify both frozen skills through the native evidence handler in
    // interleaved order; this checks pinned input integrity, not project tests.
    let skill_actor = |task: &Task| coordination::Actor::Worker {
        id: task.worker_id.clone(),
        attempt: 1,
    };
    let (opened_a, _) = reviewer_skill_evidence::dispatch_state(
        &state,
        &task_a.root_id,
        generation_a,
        &skill_actor(&task_a),
        "pandacode_reviewer_skill_open",
        json!({"skillId":SKILL_ID}),
    )
    .unwrap();
    let (opened_b, _) = reviewer_skill_evidence::dispatch_state(
        &state,
        &task_b.root_id,
        generation_b,
        &skill_actor(&task_b),
        "pandacode_reviewer_skill_open",
        json!({"skillId":SKILL_ID}),
    )
    .unwrap();
    assert!(opened_a["lowerTrustInstructions"]
        .as_str()
        .unwrap()
        .contains("original task's evidence"));
    assert!(opened_b["lowerTrustInstructions"]
        .as_str()
        .unwrap()
        .contains("second task's evidence"));
    assert_ne!(opened_a["readOnlyPath"], opened_b["readOnlyPath"]);
    let (verified_a, _) = reviewer_skill_evidence::dispatch_state(
        &state,
        &task_a.root_id,
        generation_a,
        &skill_actor(&task_a),
        "pandacode_reviewer_skill_verify",
        json!({"skillId":SKILL_ID}),
    )
    .unwrap();
    let (verified_b, _) = reviewer_skill_evidence::dispatch_state(
        &state,
        &task_b.root_id,
        generation_b,
        &skill_actor(&task_b),
        "pandacode_reviewer_skill_verify",
        json!({"skillId":SKILL_ID}),
    )
    .unwrap();
    assert_eq!(verified_a["pinnedInputIntegrityVerified"], true);
    assert_eq!(verified_b["pinnedInputIntegrityVerified"], true);

    let settle_child = |task: &Task, score: f64| {
        settle_native_council_round(&state, &task.child_id, 1, "completed", Some(score));
        let child_generation = {
            let store = state.store.lock().unwrap();
            let mut child = load(&store, &task.child_id).unwrap().unwrap();
            refresh_council(&store, &mut child);
            assert_eq!(
                child
                    .council_status
                    .as_ref()
                    .map(|status| status.state.as_str()),
                Some("passed")
            );
            let generation = child.generation;
            save(&store, &mut child).unwrap();
            generation
        };
        super::super::dispatch_state_runtime_test(
            &state,
            &task.child_id,
            child_generation,
            "pandacode_finish_team",
            json!({"summary":"The isolated child Council passed with its current native receipts."}),
        ).unwrap_or_else(|error| panic!("child Council finish refused for {}: {error}", task.root_id));
        let store = state.store.lock().unwrap();
        let mut root = load(&store, &task.root_id).unwrap().unwrap();
        let no_agents = std::collections::HashMap::new();
        super::super::super::reconcile_nested_child_seats(&store, &no_agents, &mut root).unwrap();
        refresh_council(&store, &mut root);
        save(&store, &mut root).unwrap();
    };

    settle_child(&task_a, 9.3);
    settle_native_council_round(&state, &task_a.root_id, 1, "completed", Some(9.3));
    let (status_a, receipt_a) = {
        let store = state.store.lock().unwrap();
        let root_a = load(&store, &task_a.root_id).unwrap().unwrap();
        let root_b = load(&store, &task_b.root_id).unwrap().unwrap();
        let status_a = super::super::super::native_council_status(&store, &root_a).unwrap();
        let status_b = super::super::super::native_council_status(&store, &root_b).unwrap();
        assert_eq!(status_a.state, "passed");
        assert_ne!(
            status_b.state, "passed",
            "task A's child result cannot approve task B"
        );
        assert!(super::super::super::verify_composition_terminal_receipts(
            &store, &root_a, &status_a
        ));
        assert!(!super::super::super::verify_composition_terminal_receipts(
            &store, &root_b, &status_a
        ));
        let receipt = super::super::super::council_ledger_projection(&store, &root_a)
            .council_child_receipts
            .into_iter()
            .next()
            .unwrap();
        let policy_b = super::super::super::council_policy_fingerprint(&task_b.binding).unwrap();
        assert!(
            receipt
                .validate_for(
                    &task_b.link.root_council_cycle_id,
                    &task_b.worker_id,
                    BOT_ID,
                    &task_b.link.root_artifact_revision,
                    &policy_b,
                )
                .is_err(),
            "task A's receipt is not valid for task B's frozen authority"
        );
        assert_eq!(receipt.min_score, Some(9.3));
        (status_a, receipt)
    };
    let premature_b = super::super::dispatch_state_runtime_test(
        &state,
        &task_b.root_id,
        generation_b,
        "pandacode_finish_team",
        json!({"summary":"Task B must remain unsettled until its own child receipts exist."}),
    )
    .unwrap_err();
    assert!(
        premature_b.contains("Every worker must complete"),
        "{premature_b}"
    );
    super::super::dispatch_state_runtime_test(
        &state,
        &task_a.root_id,
        generation_a,
        "pandacode_finish_team",
        json!({"summary":"Task A passed using only its own native child receipt."}),
    )
    .unwrap();
    finish(&state, &lead_a, "Task A native Council settled.", true);

    // Replaying task B's dispatch after A completes still resolves to B's
    // existing seat and leaves its in-flight child cycle untouched.
    let replayed_ids_b =
        super::super::dispatch_council_round(&state, &task_b.root_id, vec![root_assignment_b])
            .unwrap();
    assert_eq!(
        replayed_ids_b,
        vec![task_b.worker_id.clone()],
        "replay returns only the requested existing seat, not a sibling seat"
    );
    assert_eq!(saved(&state, &task_b.root_id).workers.len(), 2);
    assert_eq!(saved(&state, &task_b.child_id).status, "running");

    settle_child(&task_b, 9.7);
    settle_native_council_round(&state, &task_b.root_id, 1, "completed", Some(9.7));
    let status_b = {
        let store = state.store.lock().unwrap();
        let root_b = load(&store, &task_b.root_id).unwrap().unwrap();
        let status_b = super::super::super::native_council_status(&store, &root_b).unwrap();
        assert_eq!(status_b.state, "passed");
        assert!(super::super::super::verify_composition_terminal_receipts(
            &store, &root_b, &status_b
        ));
        let ledger_b = super::super::super::council_ledger_projection(&store, &root_b);
        assert_eq!(ledger_b.council_child_receipts.len(), 1);
        assert_eq!(
            ledger_b.council_child_receipts[0].root_cycle_id,
            task_b.link.root_council_cycle_id
        );
        assert_eq!(
            ledger_b.council_child_receipts[0].child_cycle_id,
            task_b.link.child_council_cycle_id
        );
        assert_eq!(ledger_b.council_child_receipts[0].min_score, Some(9.7));
        assert_eq!(ledger_b.council_child_leaves.len(), 2);
        assert!(ledger_b.council_child_leaves.iter().all(|score| {
            score.root_cycle_id == task_b.link.root_council_cycle_id && score.score == Some(9.7)
        }));
        status_b
    };
    super::super::dispatch_state_runtime_test(
        &state,
        &task_b.root_id,
        generation_b,
        "pandacode_finish_team",
        json!({"summary":"Task B passed using its own native child receipt."}),
    )
    .unwrap();
    finish(&state, &lead_b, "Task B native Council settled.", true);

    {
        let store = state.store.lock().unwrap();
        for (task, expected_score) in [(&task_a, 9.3), (&task_b, 9.7)] {
            let root = load(&store, &task.root_id).unwrap().unwrap();
            let required_root_seats = root
                .workers
                .iter()
                .filter(|worker| {
                    worker.council.as_ref().is_some_and(|seat| seat.round == 1) && !worker.abandoned
                })
                .collect::<Vec<_>>();
            assert_eq!(required_root_seats.len(), 2);
            assert!(required_root_seats.iter().all(|worker| {
                worker.status == "completed"
                    && worker
                        .council
                        .as_ref()
                        .is_some_and(|seat| seat.score.is_some())
            }));
            assert!(required_root_seats
                .iter()
                .any(|worker| worker.id == task.worker_id));
            assert!(required_root_seats
                .iter()
                .any(|worker| worker.id == task.auto_worker_id));
            let status = super::super::super::native_council_status(&store, &root).unwrap();
            assert_eq!(status.state, "passed");
            assert!(super::super::super::verify_composition_terminal_receipts(
                &store, &root, &status
            ));
            let ledger = super::super::super::council_ledger_projection(&store, &root);
            assert_eq!(ledger.council_child_receipts.len(), 1);
            assert_eq!(
                ledger.council_child_receipts[0].root_cycle_id,
                task.link.root_council_cycle_id
            );
            assert_eq!(
                ledger.council_child_receipts[0].child_cycle_id,
                task.link.child_council_cycle_id
            );
            assert_eq!(
                ledger.council_child_receipts[0].min_score,
                Some(expected_score)
            );
            assert_eq!(ledger.council_child_leaves.len(), 2);
            assert!(ledger.council_child_leaves.iter().all(|score| {
                score.root_cycle_id == task.link.root_council_cycle_id
                    && score.score == Some(expected_score)
            }));
            let persisted = root
                .workers
                .iter()
                .find(|worker| worker.id == task.worker_id)
                .unwrap()
                .reviewer_binding
                .as_ref()
                .unwrap();
            assert_eq!(
                persisted.snapshot.bot_context,
                task.binding.snapshot.bot_context
            );
            assert_eq!(
                persisted.snapshot.tool_policy,
                task.binding.snapshot.tool_policy
            );
            assert_eq!(persisted.snapshot.skills, task.binding.snapshot.skills);
            reviewer_security::validate_issued_authority(&store, &task.root_id, persisted).unwrap();
        }
        assert_eq!(status_a.state, "passed");
        assert_eq!(status_b.state, "passed");
        assert_eq!(receipt_a.min_score, Some(9.3));
        assert_ne!(receipt_a.root_cycle_id, task_b.link.root_council_cycle_id);
        let live_bot = crate::bots::get(&store, BOT_ID).unwrap();
        assert_eq!(live_bot.instructions, "Live profile after admission");
        assert!(live_bot.skills.is_empty());
    }
    let _ = std::fs::remove_dir_all(temp);
}
