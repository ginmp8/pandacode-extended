use rusqlite::params;
use super::channels::{self, discord, http, slack, telegram, web, whatsapp};
use super::{brief, gateway, guard, schedule, *};
use serde_json::json;

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
    let raw_skills: bool = f.store.db.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('bots') WHERE name='skills')", [], |r| r.get(0)).unwrap();
    assert!(!raw_skills, "Extended keeps Skills in its scoped hash-pinned library");
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
        assert_eq!(store.db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 29);
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
    assert_eq!(web::new_token().len(), 64);
    assert_eq!(web::page("not a channel!").status, 404);
    let engine = http::Response::markdown_engine();
    assert_eq!((engine.status, engine.content_type), (200, "text/javascript; charset=utf-8"));
    assert!(std::str::from_utf8(&engine.body).unwrap().trim_end().ends_with("export default createMd4cModule;"));
}

#[test]
fn only_registered_bot_tools_skip_approval() {
    assert!(super::tools::self_scoped("pandacode_bot_remember"));
    assert!(!super::tools::self_scoped("pandacode_bot_notify"));
    assert!(!super::tools::self_scoped("pandacode_bot_anything_else"));
    assert!(!super::tools::self_scoped("pandacode_open_browser"));
    assert!(super::tools::catalog().iter().all(|tool| tool["name"].as_str().is_some_and(|name| super::tools::self_scoped(name) || super::tools::ALWAYS_ASK.contains(&name))));
    assert!(guard::dangerous("Allow pandacode_bot_notify?"));
}

#[test]
fn outbound_bot_tools_cannot_be_covered_by_broad_allow_rules() {
    let f = fixture();
    let bot = bot(&f);
    guard::add_rule(&f.store, &bot.id, "*pandacode*", "allow").unwrap();
    for action in ["Allow pandacode_bot_notify?", "Allow pandacode_cloud_cli?"] {
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
