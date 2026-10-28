use rusqlite::params;
use super::channels::{self, discord, http, slack, telegram, web, whatsapp};
use super::{brief, gateway, guard, schedule, *};
use serde_json::json;
use super::shared_channels::Running;

fn scheduled_native_run(
    state: &crate::AppState,
    session_id: &str,
    admission: &schedule::Admission,
) -> crate::Result<crate::PreparedRun> {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    crate::prepare_task_run_in_project(
        state,
        session_id.into(),
        Some("Scheduled test work".into()),
        vec![],
        tx,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        crate::RunAdmission {
            scheduled: Some(admission),
            ..Default::default()
        },
    )
}

fn scheduled_state() -> (Fixture, std::sync::Arc<crate::AppState>, Bot, String) {
    let f = fixture();
    let (state, _) = crate::concurrent_task_tests::test_state();
    let (bot, session) = {
        let store = state.store.lock().unwrap();
        let mut profile = input("Scheduled native bot");
        profile.mode = "full".into();
        let bot = create(&store, &profile, &f.root).unwrap();
        let (session, _) =
            thread_session(&store, &bot, "schedule", "schedule", "Scheduled work").unwrap();
        (bot, session)
    };
    (f, state, bot, session)
}

fn due_once(store: &Store, bot: &Bot, created_by: &str) -> schedule::Schedule {
    let now = crate::storage::now();
    schedule::create(
        store,
        &bot.id,
        &schedule::ScheduleInput {
            prompt: "Do the approved work".into(),
            kind: "once".into(),
            run_at: Some(now - 1),
            ..Default::default()
        },
        created_by,
        now,
    )
    .unwrap()
}

fn finish_scheduled(state: &crate::AppState, session: &str) {
    let mut agents = state.agents.lock().unwrap();
    let store = state.store.lock().unwrap();
    store.finish_task_turn(session, true, false).unwrap();
    agents.remove(session);
}

#[test]
fn scheduled_authorization_outlives_activity_retention_and_creation_is_atomic() {
    let f = fixture();
    let mut profile = input("Durable Plan");
    profile.mode = "plan".into();
    let bot = create(&f.store, &profile, &f.root).unwrap();
    let item = schedule::create(
        &f.store,
        &bot.id,
        &schedule::ScheduleInput {
            prompt: "Keep the original ceiling".into(),
            kind: "interval".into(),
            interval_minutes: Some(30),
            ..Default::default()
        },
        "user",
        0,
    )
    .unwrap();
    let transaction = f.store.db.unchecked_transaction().unwrap();
    for _ in 0..5000 {
        f.store.db.execute("INSERT INTO bot_activity(bot_id,kind,summary,detail,at) VALUES(?1,'inbound','routine','{}',0)", [&bot.id]).unwrap();
    }
    log(
        &f.store,
        &bot.id,
        "inbound",
        "Prune old activity",
        json!({}),
    )
    .unwrap();
    transaction.commit().unwrap();
    assert_eq!(
        f.store
            .db
            .query_row(
                "SELECT COUNT(*) FROM bot_activity WHERE kind='schedule'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        schedule::execution_mode(&f.store, &item, "full").unwrap(),
        "plan"
    );
    assert_eq!(
        schedule::execution_mode(&f.store, &item, "review").unwrap(),
        "plan"
    );

    for table in ["bot_schedule_authorizations", "bot_activity"] {
        let before = schedule::list(&f.store, &bot.id).unwrap().len();
        f.store.db.execute_batch(&format!("CREATE TRIGGER refuse_policy BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'policy write failed'); END;")).unwrap();
        let result = schedule::create(
            &f.store,
            &bot.id,
            &schedule::ScheduleInput {
                prompt: "Must not persist after failure".into(),
                kind: "interval".into(),
                interval_minutes: Some(30),
                ..Default::default()
            },
            "user",
            0,
        );
        assert!(result.is_err());
        assert_eq!(schedule::list(&f.store, &bot.id).unwrap().len(), before);
        assert_eq!(
            f.store
                .db
                .query_row(
                    "SELECT COUNT(*) FROM bot_schedule_authorizations",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            before as i64
        );
        f.store
            .db
            .execute_batch("DROP TRIGGER refuse_policy;")
            .unwrap();
    }
}

#[test]
fn migration_32_preserves_known_ceilings_and_safely_adopts_legacy_schedules() {
    let f = fixture();
    let path = f.root.join("schedule-migration.db");
    let ids = {
        let store = Store::open(&path).unwrap();
        let mut profile = input("Legacy schedules");
        profile.mode = "plan".into();
        let bot = create(&store, &profile, &f.root.join("bots")).unwrap();
        let plan = due_once(&store, &bot, "user");
        profile.mode = "full".into();
        let full_bot = update(&store, &bot.id, &profile).unwrap();
        let full = due_once(&store, &full_bot, "user");
        let unknown = due_once(&store, &full_bot, "user");
        let generated = due_once(&store, &full_bot, "bot");
        let malformed = due_once(&store, &full_bot, "user");
        store.db.execute("UPDATE bot_activity SET detail=json_set(detail,'$.permissionMode','unknown-mode') WHERE json_extract(detail,'$.scheduleId')=?1", [&malformed.id]).unwrap();
        store
            .db
            .execute(
                "DELETE FROM bot_activity WHERE json_extract(detail,'$.scheduleId')=?1",
                [&unknown.id],
            )
            .unwrap();
        store.db.execute_batch("DROP TABLE bot_scheduled_runs; DROP TABLE bot_schedule_authorizations; PRAGMA user_version=31;").unwrap();
        // Normal writes reject malformed JSON. Inject a deliberately corrupted
        // legacy row only in this temporary database, then restore enforcement
        // before migration: corrupt logs must not confer authority or block it.
        let corrupt_activity = "INSERT INTO bot_activity(bot_id,kind,summary,detail,at) VALUES(?1,'schedule','malformed','not-json',0)";
        assert!(store.db.execute(corrupt_activity, [&bot.id]).is_err());
        store.db.pragma_update(None, "ignore_check_constraints", true).unwrap();
        store.db.execute(corrupt_activity, [&bot.id]).unwrap();
        store.db.pragma_update(None, "ignore_check_constraints", false).unwrap();
        assert!(store.db.execute(corrupt_activity, [&bot.id]).is_err());
        [plan.id, full.id, unknown.id, generated.id, malformed.id]
    };
    for _ in 0..2 {
        let store = Store::open(&path).unwrap();
        for (id, expected) in ids.iter().zip(["plan", "full", "plan", "plan", "plan"]) {
            assert_eq!(
                schedule::execution_mode(&store, &schedule::get(&store, id).unwrap(), "full")
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            store
                .db
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            crate::storage::SCHEMA_VERSION
        );
    }
}

#[test]
fn pause_closes_admission_before_the_stop_scan_can_await() {
    let (_f, state, bot, session) = scheduled_state();
    let (admission, desktop) = {
        let store = state.store.lock().unwrap();
        let admission =
            schedule::Admission::schedule(&due_once(&store, &bot, "user"), false).unwrap();
        let (desktop, _) = thread_session(&store, &bot, DESKTOP, DESKTOP, "Desktop").unwrap();
        (admission, desktop)
    };
    let scanning = std::sync::Arc::new(std::sync::Barrier::new(2));
    let finish_scan = std::sync::Arc::new(std::sync::Barrier::new(2));
    let pausing = {
        let state = state.clone();
        let id = bot.id.clone();
        let scanning = scanning.clone();
        let finish_scan = finish_scan.clone();
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(pause_with_stop(&state, &id, true, async {
                    // This is the production stop/await boundary, after it could
                    // have observed an idle conversation but before Pause returns.
                    scanning.wait();
                    finish_scan.wait();
                    Ok(())
                }))
        })
    };
    scanning.wait();
    let lifecycle = lifecycle_lock(&bot.id).unwrap();
    let operation_is_serialized = lifecycle.try_lock().is_err();
    let scheduled = scheduled_native_run(&state, &session, &admission);
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let ordinary = crate::prepare_task_run(
        &state,
        desktop.clone(),
        Some("Start while pausing".into()),
        vec![],
        tx,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    let console_denied = {
        let store = state.store.lock().unwrap();
        tasks::check_console_context(&store, &store.session(&desktop).unwrap()).is_err()
    };
    finish_scan.wait();
    assert!(pausing.join().unwrap().unwrap().paused);
    assert!(
        operation_is_serialized,
        "Resume/reset/delete must wait for this operation"
    );
    assert!(
        scheduled.is_err(),
        "a captured Full schedule cannot slip past the stop snapshot"
    );
    assert!(
        ordinary.is_err(),
        "ordinary owned threads must honor the same admission gate"
    );
    assert!(console_denied);
    assert!(state.agents.lock().unwrap().is_empty());
    let store = state.store.lock().unwrap();
    assert!(store.messages(&session, None).unwrap().is_empty());
    assert!(store.messages(&desktop, None).unwrap().is_empty());
}

#[tokio::test]
async fn failed_pause_keeps_admission_closed_until_explicit_resume() {
    let (_f, state, bot, session) = scheduled_state();
    let admission = {
        let store = state.store.lock().unwrap();
        schedule::Admission::schedule(&due_once(&store, &bot, "user"), false).unwrap()
    };
    assert!(pause_with_stop(&state, &bot.id, true, async {
        Err("Stopping failed".into())
    })
    .await
    .is_err());
    assert!(get(&state.store.lock().unwrap(), &bot.id).unwrap().paused);
    assert!(scheduled_native_run(&state, &session, &admission).is_err());
    let resumed = pause_with_stop(&state, &bot.id, false, async {
        panic!("Resume must not stop an agent")
    })
    .await
    .unwrap();
    assert!(!resumed.paused);
    assert_eq!(
        scheduled_native_run(&state, &session, &admission)
            .unwrap()
            .config
            .mode,
        "full"
    );
}

#[test]
fn legacy_pruned_authorization_cannot_be_recreated_by_full_mode_and_allow_rules() {
    let f = fixture();
    let path = f.root.join("pruned-legacy.db");
    let (bot_id, schedule_id) = {
        let store = Store::open(&path).unwrap();
        let mut profile = input("Legacy Plan");
        profile.mode = "plan".into();
        let bot = create(&store, &profile, &f.root.join("bots")).unwrap();
        let item = due_once(&store, &bot, "user");
        profile.mode = "full".into();
        update(&store, &bot.id, &profile).unwrap();
        guard::add_rule(&store, &bot.id, "write_file", "allow").unwrap();
        let transaction = store.db.unchecked_transaction().unwrap();
        for _ in 0..5000 {
            store.db.execute("INSERT INTO bot_activity(bot_id,kind,summary,detail,at) VALUES(?1,'inbound','routine','{}',0)", [&bot.id]).unwrap();
        }
        log(&store, &bot.id, "inbound", "Prune old policy", json!({})).unwrap();
        transaction.commit().unwrap();
        assert_eq!(
            store
                .db
                .query_row(
                    "SELECT COUNT(*) FROM bot_activity WHERE kind='schedule'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        store.db.execute_batch("DROP TABLE bot_scheduled_runs; DROP TABLE bot_schedule_authorizations; PRAGMA user_version=31;").unwrap();
        (bot.id, item.id)
    };
    let (state, _) = crate::concurrent_task_tests::test_state();
    let (admission, session) = {
        let mut store = state.store.lock().unwrap();
        *store = Store::open(&path).unwrap();
        let bot = get(&store, &bot_id).unwrap();
        assert_eq!(bot.mode, "full");
        assert_eq!(
            guard::decide(&store, &bot_id, "write_file report.md").unwrap(),
            guard::Decision::Allow
        );
        let item = schedule::get(&store, &schedule_id).unwrap();
        let (session, _) =
            thread_session(&store, &bot, "schedule", "schedule", "Scheduled work").unwrap();
        (
            schedule::Admission::schedule(&item, false).unwrap(),
            session,
        )
    };
    let prepared = scheduled_native_run(&state, &session, &admission).unwrap();
    assert_eq!(
        prepared.config.mode, "plan",
        "unproven legacy policy must not reach approval-rule execution as Ask"
    );
    let store = state.store.lock().unwrap();
    assert_eq!(store.session(&session).unwrap().mode, "plan");
    assert_eq!(
        store
            .db
            .query_row(
                "SELECT permission_mode FROM bot_scheduled_runs WHERE message_id=?1",
                [prepared.turn_message_id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "plan"
    );
}

#[test]
fn native_schedule_admission_binds_each_prompt_to_its_own_ceiling() {
    let (_f, state, bot, session) = scheduled_state();
    let (restricted, full) = {
        let store = state.store.lock().unwrap();
        let a = due_once(&store, &bot, "bot");
        let b = due_once(&store, &bot, "user");
        // Both admissions target one shared conversation; a persisted Full mode
        // must not authorize A's bot-authored prompt.
        store
            .update(&session, None, None, None, Some("full".into()))
            .unwrap();
        (
            schedule::Admission::schedule(&a, false).unwrap(),
            schedule::Admission::schedule(&b, true).unwrap(),
        )
    };
    let a = scheduled_native_run(&state, &session, &restricted).unwrap();
    assert_eq!(a.config.mode, "plan");
    assert!(a.config.prompt.contains("Do the approved work"));
    assert!(
        !a.config.prompt.contains("Scheduled test work"),
        "the admission uses the stored schedule prompt, not a substituted caller prompt"
    );
    assert!(
        scheduled_native_run(&state, &session, &full).is_err(),
        "busy admission must not queue a bare privileged prompt"
    );
    {
        let store = state.store.lock().unwrap();
        assert_eq!(store.session(&session).unwrap().mode, "plan");
        assert_eq!(
            store
                .db
                .query_row(
                    "SELECT permission_mode FROM bot_scheduled_runs WHERE message_id=?1",
                    [a.turn_message_id],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "plan"
        );
    }
    finish_scheduled(&state, &session);
    let b = scheduled_native_run(&state, &session, &full).unwrap();
    assert_eq!(b.config.mode, "full");
    assert_ne!(a.turn_message_id, b.turn_message_id);
}

#[test]
fn concurrent_scheduled_claims_cannot_exchange_prompt_authority() {
    let (_f, state, bot, session) = scheduled_state();
    let admissions = {
        let store = state.store.lock().unwrap();
        [
            (
                schedule::Admission::schedule(&due_once(&store, &bot, "bot"), false).unwrap(),
                "plan",
            ),
            (
                schedule::Admission::schedule(&due_once(&store, &bot, "user"), true).unwrap(),
                "full",
            ),
        ]
    };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let workers: Vec<_> = admissions
        .iter()
        .cloned()
        .map(|(admission, expected)| {
            let state = state.clone();
            let session = session.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let result =
                    scheduled_native_run(&state, &session, &admission).map(|run| run.config.mode);
                (admission, expected, result)
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|(_, _, result)| result.is_ok())
            .count(),
        1
    );
    for (_, expected, result) in &results {
        if let Ok(mode) = result {
            assert_eq!(mode.as_str(), *expected);
        }
    }
    finish_scheduled(&state, &session);
    let (retry, expected, _) = results
        .into_iter()
        .find(|(_, _, result)| result.is_err())
        .unwrap();
    assert_eq!(
        scheduled_native_run(&state, &session, &retry)
            .unwrap()
            .config
            .mode,
        expected
    );
    let store = state.store.lock().unwrap();
    assert_eq!(
        store
            .db
            .query_row("SELECT COUNT(*) FROM bot_scheduled_runs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert!(crate::message_queue::list(&store, Some(&session))
        .unwrap()
        .is_empty());
}

#[test]
fn native_schedule_rechecks_current_policy_pause_and_cancellation() {
    for change in ["mode", "pause", "disable", "delete"] {
        let (_f, state, bot, session) = scheduled_state();
        let admission = {
            let store = state.store.lock().unwrap();
            let item = due_once(&store, &bot, "user");
            let admission = schedule::Admission::schedule(&item, false).unwrap();
            match change {
                "mode" => {
                    store
                        .db
                        .execute("UPDATE bots SET mode='plan' WHERE id=?1", [&bot.id])
                        .unwrap();
                }
                "pause" => {
                    set_paused(&store, &bot.id, true).unwrap();
                }
                "disable" => {
                    schedule::set_enabled(&store, &item.id, false, crate::storage::now()).unwrap();
                }
                "delete" => {
                    schedule::delete(&store, &bot.id, &item.id).unwrap();
                }
                _ => unreachable!(),
            }
            admission
        };
        let result = scheduled_native_run(&state, &session, &admission);
        if change == "mode" {
            assert_eq!(
                result.unwrap().config.mode,
                "plan",
                "current restriction intersects the original ceiling"
            );
        } else {
            assert!(
                result.is_err(),
                "{change} must revoke a prepared occurrence"
            );
            assert!(state.agents.lock().unwrap().is_empty());
            let store = state.store.lock().unwrap();
            assert!(store.messages(&session, None).unwrap().is_empty());
            assert_eq!(
                store
                    .db
                    .query_row("SELECT COUNT(*) FROM bot_scheduled_runs", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    }
}

#[test]
fn busy_once_retries_and_is_accepted_exactly_once_without_queueing() {
    let (_f, state, bot, session) = scheduled_state();
    let (item, admission) = {
        let store = state.store.lock().unwrap();
        let item = due_once(&store, &bot, "bot");
        let admission = schedule::Admission::schedule(&item, false).unwrap();
        (item, admission)
    };
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    crate::prepare_task_run(
        &state,
        session.clone(),
        Some("An existing turn".into()),
        vec![],
        tx,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
    .unwrap();
    assert!(scheduled_native_run(&state, &session, &admission).is_err());
    {
        let store = state.store.lock().unwrap();
        let pending = schedule::get(&store, &item.id).unwrap();
        assert!(pending.enabled);
        assert_eq!(pending.next_run_at, item.next_run_at);
        assert!(pending.last_run_at.is_none());
        assert_ne!(
            brief_version_of(&store, &session).unwrap(),
            get(&store, &bot.id).unwrap().memory_version,
            "a busy attempt must not consume its brief"
        );
        assert!(crate::message_queue::list(&store, Some(&session))
            .unwrap()
            .is_empty());
    }
    finish_scheduled(&state, &session);
    let prepared = scheduled_native_run(&state, &session, &admission).unwrap();
    assert_eq!(prepared.config.mode, "plan");
    assert!(prepared.config.prompt.contains(brief::HEADER));
    finish_scheduled(&state, &session);
    assert!(
        scheduled_native_run(&state, &session, &admission).is_err(),
        "the same occurrence cannot be claimed again"
    );
    let store = state.store.lock().unwrap();
    let done = schedule::get(&store, &item.id).unwrap();
    assert!(!done.enabled && done.next_run_at.is_none() && done.last_run_at.is_some());
    assert_eq!(
        store
            .db
            .query_row("SELECT COUNT(*) FROM bot_scheduled_runs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .messages(&session, None)
            .unwrap()
            .iter()
            .filter(|message| message.content.starts_with("Scheduled:"))
            .count(),
        1
    );
}

#[test]
fn due_selection_keeps_other_bots_eligible_while_old_work_retries() {
    let f = fixture();
    let busy = bot(&f);
    let ready = create(&f.store, &input("Ready bot"), &f.root).unwrap();
    for _ in 0..21 {
        schedule::create(
            &f.store,
            &busy.id,
            &schedule::ScheduleInput {
                prompt: "Old busy work".into(),
                kind: "once".into(),
                run_at: Some(1),
                ..Default::default()
            },
            "bot",
            0,
        )
        .unwrap();
    }
    let next = schedule::create(
        &f.store,
        &ready.id,
        &schedule::ScheduleInput {
            prompt: "Independent work".into(),
            kind: "once".into(),
            run_at: Some(2),
            ..Default::default()
        },
        "bot",
        0,
    )
    .unwrap();
    for _ in 0..2 {
        let due = schedule::due(&f.store, 3).unwrap().0;
        assert!(
            due.iter().any(|item| item.id == next.id),
            "unaccepted old work must not starve another bot"
        );
        assert_eq!(due.len(), 22);
    }
}

#[test]
fn scheduled_acceptance_rolls_back_all_writes_and_retry_is_safe() {
    for (table, operation) in [
        ("messages", "INSERT"),
        ("bot_scheduled_runs", "INSERT"),
        ("bot_schedules", "UPDATE"),
        ("bot_threads", "UPDATE"),
        ("bot_activity", "INSERT"),
    ] {
        let (_f, state, bot, session) = scheduled_state();
        let (item, admission) = {
            let store = state.store.lock().unwrap();
            let item = due_once(&store, &bot, "bot");
            let admission = schedule::Admission::schedule(&item, false).unwrap();
            store.db.execute_batch(&format!("CREATE TRIGGER refuse_acceptance BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT,'acceptance write failed'); END;")).unwrap();
            (item, admission)
        };
        assert!(
            scheduled_native_run(&state, &session, &admission).is_err(),
            "{table} failure must reach the caller"
        );
        assert!(state.agents.lock().unwrap().is_empty());
        {
            let store = state.store.lock().unwrap();
            assert_eq!(store.session(&session).unwrap().status, "idle");
            assert!(store.messages(&session, None).unwrap().is_empty());
            assert_eq!(schedule::get(&store, &item.id).unwrap(), item);
            assert_ne!(
                brief_version_of(&store, &session).unwrap(),
                get(&store, &bot.id).unwrap().memory_version
            );
            assert_eq!(
                store
                    .db
                    .query_row("SELECT COUNT(*) FROM bot_scheduled_runs", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            store
                .db
                .execute_batch("DROP TRIGGER refuse_acceptance;")
                .unwrap();
        }
        assert!(scheduled_native_run(&state, &session, &admission).is_ok());
        let store = state.store.lock().unwrap();
        assert_eq!(
            store
                .db
                .query_row("SELECT COUNT(*) FROM bot_scheduled_runs", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}

#[test]
fn scheduled_commit_is_atomic_across_restart_before_and_after_acceptance() {
    let f = fixture();
    let path = f.root.join("schedule-restart.db");
    let (session, id, admission) = {
        let store = Store::open(&path).unwrap();
        let bot = create(&store, &input("Restart"), &f.root.join("bots")).unwrap();
        let item = due_once(&store, &bot, "bot");
        let (session, _) =
            thread_session(&store, &bot, "schedule", "schedule", "Scheduled work").unwrap();
        let admission = schedule::Admission::schedule(&item, false).unwrap();
        // A failed commit leaves no accepted turn and the item is still due
        // when the process reopens the database.
        store.db.execute_batch("CREATE TRIGGER refuse_receipt BEFORE INSERT ON bot_scheduled_runs BEGIN SELECT RAISE(ABORT,'receipt failed'); END;").unwrap();
        assert!(store
            .begin_scheduled_task_turn(&session, "Retry after restart", &admission)
            .is_err());
        store
            .db
            .execute_batch("DROP TRIGGER refuse_receipt;")
            .unwrap();
        (session, item.id, admission)
    };
    {
        let store = Store::open(&path).unwrap();
        assert!(schedule::get(&store, &id).unwrap().enabled);
        assert!(store.messages(&session, None).unwrap().is_empty());
        store
            .begin_scheduled_task_turn(&session, "Retry after restart", &admission)
            .unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.session(&session).unwrap().status, "interrupted");
    assert!(!schedule::get(&store, &id).unwrap().enabled);
    assert_eq!(store.messages(&session, None).unwrap().len(), 1);
    assert!(store
        .begin_scheduled_task_turn(&session, "Must not duplicate", &admission)
        .is_err());
    assert_eq!(store.messages(&session, None).unwrap().len(), 1);
    assert_eq!(
        store
            .db
            .query_row("SELECT COUNT(*) FROM bot_scheduled_runs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn recurring_manual_runs_and_heartbeats_share_atomic_admission() {
    for kind in ["interval", "daily", "weekdays"] {
        let (_f, state, bot, session) = scheduled_state();
        let (item, admission) = {
            let store = state.store.lock().unwrap();
            let item = schedule::create(
                &store,
                &bot.id,
                &schedule::ScheduleInput {
                    prompt: "Recurring work".into(),
                    kind: kind.into(),
                    interval_minutes: Some(30),
                    time_of_day: Some("09:30".into()),
                    ..Default::default()
                },
                "user",
                crate::storage::now(),
            )
            .unwrap();
            // Explicit Run Now works for a disabled future recurring schedule.
            schedule::set_enabled(&store, &item.id, false, crate::storage::now()).unwrap();
            let admission = schedule::Admission::schedule(&item, true).unwrap();
            (item, admission)
        };
        let prepared = scheduled_native_run(&state, &session, &admission).unwrap();
        assert_eq!(prepared.config.mode, "full");
        finish_scheduled(&state, &session);
        assert!(
            scheduled_native_run(&state, &session, &admission).is_err(),
            "a manual receipt cannot be replayed"
        );
        let store = state.store.lock().unwrap();
        let after = schedule::get(&store, &item.id).unwrap();
        assert!(
            !after.enabled,
            "Run Now must not reenable a paused recurring schedule"
        );
        assert_eq!(
            after.next_run_at, item.next_run_at,
            "Run Now must not consume the next timer occurrence"
        );
        assert_eq!(after.last_run_at, item.last_run_at);
        let enabled = schedule::set_enabled(&store, &item.id, true, crate::storage::now()).unwrap();
        let timer = schedule::Admission::schedule(&enabled, false).unwrap();
        assert!(
            schedule::resolve(&store, &timer, enabled.next_run_at.unwrap()).is_ok(),
            "the next timer is not a replay of Run Now"
        );
    }

    for kind in ["once", "interval", "daily", "weekdays"] {
        let (_f, state, bot, session) = scheduled_state();
        let (item, manual, timer) = {
            let store = state.store.lock().unwrap();
            let now = crate::storage::now();
            let item = if kind == "once" {
                due_once(&store, &bot, "bot")
            } else {
                schedule::create(
                    &store,
                    &bot.id,
                    &schedule::ScheduleInput {
                        prompt: "Overdue recurring work".into(),
                        kind: kind.into(),
                        interval_minutes: Some(30),
                        time_of_day: Some("09:30".into()),
                        ..Default::default()
                    },
                    "bot",
                    now - 7 * DAY,
                )
                .unwrap()
            };
            assert!(item.next_run_at.unwrap() <= now);
            let manual = schedule::Admission::schedule(&item, true).unwrap();
            let timer = schedule::Admission::schedule(&item, false).unwrap();
            (item, manual, timer)
        };
        assert_eq!(
            scheduled_native_run(&state, &session, &manual)
                .unwrap()
                .config
                .mode,
            "plan"
        );
        finish_scheduled(&state, &session);
        assert_eq!(
            scheduled_native_run(&state, &session, &timer)
                .unwrap()
                .config
                .mode,
            "plan"
        );
        finish_scheduled(&state, &session);
        assert!(scheduled_native_run(&state, &session, &manual).is_err());
        assert!(scheduled_native_run(&state, &session, &timer).is_err());
        let manual_again = {
            let store = state.store.lock().unwrap();
            assert_eq!(
                store
                    .db
                    .query_row("SELECT COUNT(*) FROM bot_scheduled_runs", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            let after = schedule::get(&store, &item.id).unwrap();
            assert!(after.last_run_at.is_some());
            if kind == "once" {
                assert!(!after.enabled && after.next_run_at.is_none());
            } else {
                assert!(after.enabled && after.next_run_at.unwrap() > after.last_run_at.unwrap());
            }
            schedule::Admission::schedule(&after, true).unwrap()
        };
        // A fresh explicit Run Now request remains possible after a once timer
        // has completed, but replaying the same request never starts twice.
        assert_eq!(
            scheduled_native_run(&state, &session, &manual_again)
                .unwrap()
                .config
                .mode,
            "plan"
        );
        finish_scheduled(&state, &session);
        assert!(scheduled_native_run(&state, &session, &manual_again).is_err());
    }

    let (_f, state, bot, session) = scheduled_state();
    let admission = {
        let store = state.store.lock().unwrap();
        store
            .db
            .execute(
                "UPDATE bots SET heartbeat_minutes=30,next_heartbeat_at=?2 WHERE id=?1",
                params![bot.id, crate::storage::now() - 1],
            )
            .unwrap();
        schedule::Admission::heartbeat(&get(&store, &bot.id).unwrap()).unwrap()
    };
    let prepared = scheduled_native_run(&state, &session, &admission).unwrap();
    assert_eq!(
        prepared.config.mode, "plan",
        "a Full bot's heartbeat remains read-only"
    );
    finish_scheduled(&state, &session);
    assert!(scheduled_native_run(&state, &session, &admission).is_err());
    let store = state.store.lock().unwrap();
    assert!(get(&store, &bot.id).unwrap().next_heartbeat_at.unwrap() > crate::storage::now());
}

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
fn edited_bot_task_keeps_current_brief_without_adopting_the_source_thread() {
    let f = fixture();
    let bot = bot(&f);
    remember(&f.store, &bot.id, "Use metric units", "user").unwrap();
    let (source_id, _) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout · You").unwrap();
    f.store.message(&source_id, "user", "Original prompt").unwrap();
    let message_id = f.store.db.last_insert_rowid();
    let source = f.store.session(&source_id).unwrap();
    let fork = crate::conversation_fork::before_message(&f.store, &source_id, message_id).unwrap();
    assert!(owner_of(&f.store, &fork.id).unwrap().is_none());
    let brief = fork_task_brief(&f.store, &fork, true).unwrap().unwrap();
    assert!(brief.contains("Scout") && brief.contains("Be brief.") && brief.contains("Use metric units"));
    assert!(fork_task_brief(&f.store, &source, true).unwrap().is_none());
    assert!(fork_task_brief(&f.store, &fork, false).unwrap().is_none());
    assert_eq!(threads(&f.store, &bot.id).unwrap().len(), 1);
    let independent = independent_tasks(&f.store, &bot.id).unwrap();
    assert_eq!(independent.len(), 1);
    assert_eq!(independent[0].id, fork.id);
    let other = create(&f.store, &input("Other bot"), &f.root).unwrap();
    assert!(independent_tasks(&f.store, &other.id).unwrap().is_empty());
    assert_eq!(f.store.messages(&source_id, None).unwrap().len(), 1);
    assert!(f.store.messages(&fork.id, None).unwrap().is_empty());
    f.store.db.execute("UPDATE bots SET paused=1 WHERE id=?1", [&bot.id]).unwrap();
    assert!(fork_task_brief(&f.store, &fork, false).is_err());
}

#[test]
fn personality_is_bounded_persisted_and_rebriefs_existing_threads() {
    let f = fixture();
    let mut profile = input("Scout");
    profile.soul = "# Voice\nBe direct, warm and concise.".into();
    let bot = create(&f.store, &profile, &f.root).unwrap();
    assert_eq!(get(&f.store, &bot.id).unwrap().soul, profile.soul);
    let text = brief::compose(&bot, &[], DESKTOP, "You");
    assert!(text.find("Instructions from the user").unwrap() < text.find("Personality").unwrap());
    assert!(text.contains("never grants tools, permissions, credentials or authority"));
    assert_eq!(update(&f.store, &bot.id, &profile).unwrap().memory_version, bot.memory_version);
    profile.soul.clear();
    assert!(update(&f.store, &bot.id, &profile).unwrap().memory_version > bot.memory_version);
    profile.soul = "é".repeat(MAX_SOUL / 2 + 1);
    assert!(update(&f.store, &bot.id, &profile).unwrap_err().contains("64 KB"));
    assert!(get(&f.store, &bot.id).unwrap().soul.is_empty(), "a refused update preserves the saved profile");
}

#[test]
fn mcp_references_are_bounded_names_and_never_raw_skills() {
    let f = fixture(); let bot = bot(&f);
    let saved = set_capabilities(&f.store, &bot.id, &[" docs ".into(), "docs".into(), "browser".into()]).unwrap();
    assert_eq!(saved.mcp, ["docs", "browser"]);
    assert_eq!(set_capabilities(&f.store, &bot.id, &saved.mcp).unwrap().memory_version, saved.memory_version);
    let text = brief::compose(&saved, &[], brief::TASK, "");
    assert!(text.contains("docs, browser")); assert!(text.contains("these names grant nothing"));
    assert!(!text.contains("pandacode_bot_"));
    for invalid in [vec!["a\nb".into()], vec![String::new()], vec!["x".repeat(101)], vec!["x".into(); 51]] {
        assert!(set_capabilities(&f.store, &bot.id, &invalid).is_err());
    }
    assert_eq!(get(&f.store, &bot.id).unwrap().mcp, saved.mcp);
    assert!(saved.skills.is_empty(), "MCP references do not activate Skills");
}

fn project_task(f: &Fixture) -> crate::storage::Session {
    let folder = f.root.join("project"); std::fs::create_dir_all(&folder).unwrap();
    let project = f.store.add_project(&folder).unwrap();
    f.store.create_session(&project.id, "codex", "default", "ask").unwrap()
}

#[test]
fn ordinary_task_binding_keeps_checkout_permissions_and_channel_identity_separate() {
    let f = fixture(); let bot = bot(&f); let task = project_task(&f);
    tasks::assign(&f.store, &task.id, Some(&bot.id)).unwrap();
    let saved = f.store.session(&task.id).unwrap();
    assert_eq!(saved.project_id, task.project_id); assert_eq!(saved.mode, "ask");
    assert_eq!(saved.provider, bot.provider); assert_eq!(saved.model, bot.model);
    assert_eq!(tasks::links(&f.store).unwrap().get(&task.id), Some(&bot.id));
    assert!(owner_of(&f.store, &task.id).unwrap().is_none());
    assert!(threads(&f.store, &bot.id).unwrap().is_empty());
    assert!(get(&f.store, &bot.id).unwrap().notify_channel_id.is_none());
    let brief = tasks::prepare(&f.store, &saved, true).unwrap().unwrap();
    assert!(brief.text.contains("own checkout")); assert!(!brief.text.contains("pandacode_bot_"));
    tasks::assign(&f.store, &task.id, None).unwrap();
    assert!(tasks::linked(&f.store, &task.id).unwrap().is_none());
    let (thread, _) = thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Bot chat").unwrap();
    assert!(tasks::assign(&f.store, &thread, Some(&bot.id)).is_err());
}

#[test]
fn linked_brief_commits_with_the_turn_and_failed_delivery_retries_it() {
    let f = fixture(); let bot = bot(&f); let task = project_task(&f);
    tasks::assign(&f.store, &task.id, Some(&bot.id)).unwrap();
    let saved = f.store.session(&task.id).unwrap();
    assert!(tasks::check_console_context(&f.store, &saved).is_err());
    let context = tasks::prepare(&f.store, &saved, true).unwrap().unwrap();
    f.store.db.execute_batch("CREATE TRIGGER refuse_bot_message BEFORE INSERT ON messages BEGIN SELECT RAISE(ABORT,'No insert'); END;").unwrap();
    assert!(f.store.begin_task_turn_with_context(&task.id, "Hello", None, Some(&context)).is_err());
    assert_eq!(tasks::linked(&f.store, &task.id).unwrap().unwrap().1, -1);
    assert_eq!(f.store.session(&task.id).unwrap().status, "idle");
    f.store.db.execute_batch("DROP TRIGGER refuse_bot_message;").unwrap();
    f.store.begin_task_turn_with_context(&task.id, "Hello", None, Some(&context)).unwrap();
    assert_eq!(f.store.messages(&task.id, None).unwrap()[0].content, "Hello");
    assert!(tasks::assign(&f.store, &task.id, None).is_err());
    assert!(tasks::prepare(&f.store, &saved, false).unwrap().is_none());
    f.store.finish_task_turn(&task.id, false, false).unwrap();
    assert!(tasks::prepare(&f.store, &saved, false).unwrap().is_some());
    f.store.begin_task_turn_with_context(&task.id, "Retry", None, Some(&context)).unwrap();
    f.store.finish_task_turn(&task.id, true, false).unwrap();
    assert!(tasks::prepare(&f.store, &saved, false).unwrap().is_none());
    remember(&f.store, &bot.id, "Prefers concise reports", "user").unwrap();
    assert!(tasks::prepare(&f.store, &saved, false).unwrap().unwrap().text.contains("Prefers concise reports"));
    set_paused(&f.store, &bot.id, true).unwrap();
    assert!(tasks::prepare(&f.store, &saved, false).is_err());
    delete(&f.store, &bot.id).unwrap();
    assert!(f.store.session(&task.id).is_ok());
    assert!(tasks::linked(&f.store, &task.id).unwrap().is_none());
}

#[test]
fn native_admission_applies_the_profile_without_frontend_context_or_bot_tools() {
    for paused in [false, true] {
        let f = fixture();
        let (state, project) = crate::concurrent_task_tests::test_state();
        let (bot, task) = {
            let store = state.store.lock().unwrap();
            let mut profile = input("Native Scout"); profile.soul = "Warm and precise.".into(); profile.mode = "full".into();
            let bot = create(&store, &profile, &f.root).unwrap();
            set_capabilities(&store, &bot.id, &["documentation".into()]).unwrap();
            let task = store.create_session(&project, "codex", "default", "ask").unwrap();
            tasks::assign(&store, &task.id, Some(&bot.id)).unwrap();
            if paused { set_paused(&store, &bot.id, true).unwrap(); }
            (bot, task)
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let prepared = crate::prepare_task_run(&state, task.id.clone(), Some("Review the project".into()), vec![], tx, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
        if paused {
            assert!(prepared.err().unwrap().contains("paused"));
            assert!(state.agents.lock().unwrap().is_empty());
            let store = state.store.lock().unwrap();
            assert!(store.messages(&task.id, None).unwrap().is_empty());
            assert_eq!(tasks::linked(&store, &task.id).unwrap().unwrap().1, -1);
        } else {
            let prepared = prepared.unwrap();
            assert_eq!(prepared.config.mode, "ask");
            assert!(prepared.config.prompt.contains("Warm and precise."));
            assert!(prepared.config.prompt.contains("documentation"));
            assert!(!prepared.config.prompt.contains("pandacode_bot_"));
            let store = state.store.lock().unwrap();
            assert_eq!(store.messages(&task.id, None).unwrap()[0].content, "Review the project");
            assert!(owner_of(&store, &task.id).unwrap().is_none());
            assert_eq!(tasks::linked(&store, &task.id).unwrap().unwrap().0.id, bot.id);
        }
    }
}

#[test]
fn interrupted_bot_task_rebriefs_after_restart() {
    let f = fixture(); let path = f.root.join("restart.db");
    let id = {
        let store = Store::open(&path).unwrap();
        let project = store.add_project(&f.root).unwrap();
        let bot = create(&store, &input("Scout"), &f.root.join("bots")).unwrap();
        let task = store.create_session(&project.id, "claude", "default", "ask").unwrap();
        tasks::assign(&store, &task.id, Some(&bot.id)).unwrap();
        let context = tasks::prepare(&store, &task, true).unwrap().unwrap();
        store.begin_task_turn_with_context(&task.id, "Started before restart", None, Some(&context)).unwrap();
        store.resume(&task.id, "existing-native-conversation").unwrap();
        task.id
    };
    let reopened = Store::open(&path).unwrap();
    let task = reopened.session(&id).unwrap();
    assert_eq!(task.status, "interrupted");
    assert!(task.resume_id.is_some());
    assert!(tasks::prepare(&reopened, &task, false).unwrap().is_some());
}

#[test]
fn binding_refuses_started_or_paused_tasks_without_partial_defaults() {
    let f = fixture(); let bot = bot(&f); let task = project_task(&f);
    set_paused(&f.store, &bot.id, true).unwrap();
    assert!(tasks::assign(&f.store, &task.id, Some(&bot.id)).is_err());
    assert_eq!(f.store.session(&task.id).unwrap().provider, task.provider);
    set_paused(&f.store, &bot.id, false).unwrap();
    f.store.message(&task.id, "user", "Started").unwrap();
    assert!(tasks::assign(&f.store, &task.id, Some(&bot.id)).is_err());
    assert!(tasks::linked(&f.store, &task.id).unwrap().is_none());
}

#[test]
fn migration_29_preserves_existing_bot_issue_and_attachment_data_and_reopens() {
    let f = fixture(); let path = f.root.join("migration.db");
    let (bot_id, task_id) = {
        let store = Store::open(&path).unwrap();
        let bot = create(&store, &input("Existing"), &f.root.join("bots")).unwrap();
        let task = store.create_session(&bot.project_id, "codex", "default", "ask").unwrap();
        store.message(&task.id, "user", "Keep this conversation").unwrap();
        let message: i64 = store.db.query_row("SELECT id FROM messages WHERE session_id=?1", [&task.id], |r| r.get(0)).unwrap();
        store.db.execute("INSERT INTO github_issue_actions(session_id,action,repository,number,url,approved_at) VALUES(?1,'close','owner/project',24,'https://github.com/owner/project/issues/24',123)", [&task.id]).unwrap();
        store.db.execute("INSERT INTO message_attachments(message_id,session_id,id,kind,name,mime,bytes,file,source) VALUES(?1,?2,'image-1','image','Keep.png','image/png',10,'image-1.png','file')", params![message, task.id]).unwrap();
        store.db.execute_batch("DROP TABLE bot_tasks; ALTER TABLE bots DROP COLUMN soul; ALTER TABLE bots DROP COLUMN mcp; PRAGMA user_version=28;").unwrap();
        (bot.id, task.id)
    };
    for _ in 0..2 {
        let store = Store::open(&path).unwrap(); let bot = get(&store, &bot_id).unwrap();
        assert_eq!(bot.name, "Existing"); assert_eq!(bot.instructions, "Be brief."); assert!(bot.soul.is_empty() && bot.mcp.is_empty());
        assert_eq!(store.messages(&task_id, None).unwrap()[0].content, "Keep this conversation");
        assert_eq!(store.db.query_row("SELECT COUNT(*) FROM message_attachments", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        assert!(store.github_issue_action(&task_id).unwrap().is_some());
        assert_eq!(store.db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), crate::storage::SCHEMA_VERSION);
    }
}

#[test]
fn migration_29_failure_rolls_back_new_columns_and_schema_version() {
    let f = fixture(); let path = f.root.join("rollback.db");
    {
        let store = Store::open(&path).unwrap();
        create(&store, &input("Existing"), &f.root.join("bots")).unwrap();
        store.db.execute_batch("DROP TABLE bot_tasks; ALTER TABLE bots DROP COLUMN soul; ALTER TABLE bots DROP COLUMN mcp; CREATE INDEX bot_tasks ON bots(name); PRAGMA user_version=28;").unwrap();
    }
    assert!(Store::open(&path).is_err());
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 28);
    assert!(!db.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('bots') WHERE name='soul')", [], |r| r.get::<_, bool>(0)).unwrap());
    assert_eq!(db.query_row("SELECT name FROM bots", [], |r| r.get::<_, String>(0)).unwrap(), "Existing");
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
    assert_eq!(gateway::parse_command("yes", true), gateway::Command::ApprovalIdRequired);
    assert_eq!(gateway::parse_command("always", true), gateway::Command::ApprovalIdRequired);
    assert_eq!(gateway::parse_command("no", true), gateway::Command::ApprovalIdRequired);
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
fn schedules_validate_and_advance_only_with_an_accepted_turn() {
    let f = fixture();
    let bot = bot(&f);
    let once = schedule::ScheduleInput { title: "Reminder".into(), prompt: "Remind me to stretch".into(), kind: "once".into(), run_at: Some(10 * MINUTE), ..Default::default() };
    let created = schedule::create(&f.store, &bot.id, &once, "bot", 0).unwrap();
    assert_eq!(created.created_by, "bot");
    let empty = schedule::ScheduleInput { kind: "daily".into(), time_of_day: Some("09:00".into()), ..Default::default() };
    assert!(schedule::create(&f.store, &bot.id, &empty, "user", 0).is_err());
    let (due, _) = schedule::due(&f.store, 11 * MINUTE).unwrap();
    assert_eq!(due.len(), 1);
    let (session, _) =
        thread_session(&f.store, &bot, "schedule", "schedule", "Scheduled work").unwrap();
    let admission = schedule::Admission::schedule(&due[0], false).unwrap();
    f.store
        .begin_scheduled_task_turn(&session, "Accepted reminder", &admission)
        .unwrap();
    let after = &schedule::list(&f.store, &bot.id).unwrap()[0];
    assert!(
        !after.enabled,
        "one-off schedules turn off only with a persisted accepted turn"
    );
    assert_eq!(
        f.store.messages(&session, None).unwrap()[0].content,
        "Accepted reminder"
    );
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
    let body = br#"{"object":"whatsapp_business_account","entry":[{"changes":[{"value":{"metadata":{"phone_number_id":"123"},"contacts":[{"wa_id":"351900","profile":{"name":"Ana"}}],"messages":[{"id":"message-1","timestamp":"1000","from":"351900","type":"text","text":{"body":"hello"}},{"from":"351900","type":"image"}]}}]}]}"#;
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(b"app-secret").unwrap();
    mac.update(body);
    let signature: String = mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    assert!(whatsapp::signature_valid("app-secret", body, &format!("sha256={signature}")));
    assert!(!whatsapp::signature_valid("other", body, &format!("sha256={signature}")));
    assert!(!whatsapp::signature_valid("app-secret", body, &signature));
    let payload: serde_json::Value = serde_json::from_slice(body).unwrap();
    assert_eq!(whatsapp::messages(&payload, "123"), vec![whatsapp::WebhookMessage { id: "message-1".into(), timestamp_ms: 1_000_000, from: "351900".into(), name: "Ana".into(), text: "hello".into() }]);
    assert!(whatsapp::messages(&payload, "another-number").is_empty());
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
fn scheduled_bot_work_cannot_inherit_later_full_access() {
    let f = fixture();
    let bot = bot(&f);
    let item = schedule::ScheduleInput { prompt: "Read a status page".into(), kind: "interval".into(), interval_minutes: Some(30), ..Default::default() };
    let generated = schedule::create(&f.store, &bot.id, &item, "bot", 0).unwrap();
    assert_eq!(schedule::execution_mode(&f.store, &generated, "full").unwrap(), "plan");
    assert_eq!(schedule::execution_mode(&f.store, &generated, "review").unwrap(), "plan");
    let user = schedule::create(&f.store, &bot.id, &item, "user", 0).unwrap();
    assert_eq!(schedule::execution_mode(&f.store, &user, "full").unwrap(), "ask");
    assert_eq!(schedule::execution_mode(&f.store, &user, "ask").unwrap(), "ask");
    f.store.db.execute("DELETE FROM bot_activity WHERE kind='schedule' AND json_extract(detail,'$.scheduleId')=?1", [&user.id]).unwrap();
    assert_eq!(
        schedule::execution_mode(&f.store, &user, "full").unwrap(),
        "ask"
    );
    assert_eq!(
        schedule::execution_mode(&f.store, &user, "plan").unwrap(),
        "plan"
    );
    f.store
        .db
        .execute(
            "DELETE FROM bot_schedule_authorizations WHERE schedule_id=?1",
            [&user.id],
        )
        .unwrap();
    assert_eq!(
        schedule::execution_mode(&f.store, &user, "full").unwrap(),
        "plan",
        "unexpected policy loss must fail read-only"
    );
}

#[test]
fn user_schedules_keep_the_original_explicit_mode_only() {
    let f = fixture();
    let mut profile = input("User schedule");
    profile.mode = "full".into();
    let bot = create(&f.store, &profile, &f.root).unwrap();
    let item = schedule::ScheduleInput { prompt: "Review a PR".into(), kind: "daily".into(), time_of_day: Some("09:30".into()), ..Default::default() };
    let schedule = schedule::create(&f.store, &bot.id, &item, "user", 0).unwrap();
    assert_eq!(schedule::execution_mode(&f.store, &schedule, "full").unwrap(), "full");
    assert_eq!(schedule::execution_mode(&f.store, &schedule, "ask").unwrap(), "ask");
    assert_eq!(schedule::execution_mode(&f.store, &schedule, "plan").unwrap(), "plan");
    let generated = schedule::create(&f.store, &bot.id, &item, "bot", 0).unwrap();
    assert_eq!(schedule::execution_mode(&f.store, &generated, "full").unwrap(), "plan");
}

#[test]
fn only_registered_bot_tools_skip_approval() {
    assert!(super::tools::self_scoped("pandacode_bot_remember"));
    assert!(!super::tools::self_scoped("pandacode_bot_notify"));
    assert!(!super::tools::self_scoped("pandacode_bot_schedule"));
    assert!(!super::tools::self_scoped("pandacode_bot_anything_else"));
    assert!(!super::tools::self_scoped("pandacode_open_browser"));
    assert!(super::tools::catalog().iter().all(|tool| tool["name"].as_str().is_some_and(|name| super::tools::self_scoped(name) || super::tools::ALWAYS_ASK.contains(&name))));
    assert!(guard::dangerous("Allow pandacode_bot_notify?"));
    assert!(guard::dangerous("Allow pandacode_bot_schedule?"));
}

#[test]
fn outbound_bot_tools_cannot_be_covered_by_broad_allow_rules() {
    let f = fixture();
    let bot = bot(&f);
    guard::add_rule(&f.store, &bot.id, "*pandacode*", "allow").unwrap();
    for action in ["Allow pandacode_bot_notify?", "Allow pandacode_bot_schedule?", "Allow pandacode_cloud_cli?"] {
        assert_eq!(guard::decide(&f.store, &bot.id, action).unwrap(), guard::Decision::Ask);
    }
    assert_eq!(guard::decide(&f.store, &bot.id, "Allow pandacode_bot_remember?").unwrap(), guard::Decision::Allow);
}

#[test]
fn bot_admission_rechecks_channel_revocation_and_ownership() {
    let f = fixture();
    let bot = bot(&f);
    let channel = channels::add(&f.store, &bot.id, &channels::ChannelInput { kind: "web".into(), ..Default::default() }).unwrap();
    f.store.db.execute("UPDATE bot_channels SET enabled=1 WHERE id=?1", [&channel.id]).unwrap();
    assert!(gateway::validate_channel(&f.store, &bot.id, &channel.id).is_ok());
    assert!(gateway::validate_channel(&f.store, "another-bot", &channel.id).is_err());
    f.store.db.execute("UPDATE bot_channels SET enabled=0 WHERE id=?1", [&channel.id]).unwrap();
    assert!(gateway::validate_channel(&f.store, &bot.id, &channel.id).is_err());
    assert!(gateway::validate_channel(&f.store, &bot.id, DESKTOP).is_ok());
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
    // Prefix-only placeholders preserve the invalid-credential check without shipping token-shaped secrets.
    let slack = channels::verify(&input("slack", &[("token", "xoxb-"), ("app_token", "xapp-")], json!({}))).await.unwrap_err();
    println!("slack: {slack}");
    assert!(slack.contains("Slack didn't accept the bot token"), "{slack}");
    let discord = channels::verify(&input("discord", &[("token", &format!("{}.{}.{}", "MTA".repeat(8), "Gabcde", "x".repeat(38)))], json!({}))).await.unwrap_err();
    println!("discord: {discord}");
    assert!(discord.contains("Discord rejected the bot token"), "{discord}");
    let whatsapp = channels::verify(&input("whatsapp", &[("access_token", "EAAG_fake_token")], json!({"phoneNumberId": "123456789012345"}))).await.unwrap_err();
    println!("whatsapp: {whatsapp}");
    assert!(whatsapp.contains("Meta rejected the access token") || whatsapp.starts_with("WhatsApp:"), "{whatsapp}");
}
