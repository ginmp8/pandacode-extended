//! Extended-only bot context policy. Do not fork the upstream Bot/BotInput contract.
use super::{brief, channels, memories, owner_of, DESKTOP};
use crate::{storage::Store, AppState, Result};
use rusqlite::OptionalExtension;
use tauri::{Emitter, State};

/// A bot-only, additive table. Do not mutate upstream profile columns or queue schemas.
pub(crate) fn upgrade(db: &rusqlite::Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS extended_bot_context(
            bot_id TEXT PRIMARY KEY REFERENCES bots(id) ON DELETE CASCADE,
            fresh_each_message INTEGER NOT NULL DEFAULT 0 CHECK(fresh_each_message IN (0,1))
        );
        CREATE TABLE IF NOT EXISTS extended_bot_channel_history(
            bot_id TEXT PRIMARY KEY REFERENCES bots(id) ON DELETE CASCADE,
            include_history INTEGER NOT NULL DEFAULT 1 CHECK(include_history IN (0,1))
        );",
    ).map_err(|e| e.to_string())
}

pub(crate) fn enabled(store: &Store, bot_id: &str) -> Result<bool> {
    let value: Option<i64> = store.db.query_row(
        "SELECT fresh_each_message FROM extended_bot_context WHERE bot_id=?1",
        [bot_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    Ok(value.unwrap_or(0) == 1)
}

/// Whether a bot receives the existing shared-channel history alongside a new assignment.
/// This policy is independent of native harness context reuse and durable bot memories.
/// A missing row preserves the upstream behavior of including recent messages.
pub(crate) fn include_shared_channel_history(store: &Store, bot_id: &str) -> Result<bool> {
    let value: Option<i64> = store.db.query_row(
        "SELECT include_history FROM extended_bot_channel_history WHERE bot_id=?1",
        [bot_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    Ok(value.unwrap_or(1) == 1)
}

pub(crate) fn set_channel_history(store: &Store, bot_id: &str, include: bool) -> Result<bool> {
    super::get(store, bot_id)?;
    if include {
        store.db.execute("DELETE FROM extended_bot_channel_history WHERE bot_id=?1", [bot_id])
            .map_err(|e| e.to_string())?;
    } else {
        store.db.execute(
            "INSERT INTO extended_bot_channel_history(bot_id,include_history) VALUES(?1,0)
             ON CONFLICT(bot_id) DO UPDATE SET include_history=0",
            [bot_id],
        ).map_err(|e| e.to_string())?;
    }
    Ok(include)
}

/// Only bot-owned conversations are included; regular tasks linked to bots are unaffected.
pub(crate) fn enabled_for_session(store: &Store, session_id: &str) -> Result<bool> {
    let value: Option<i64> = store.db.query_row(
        "SELECT p.fresh_each_message FROM bot_threads t JOIN extended_bot_context p ON p.bot_id=t.bot_id WHERE t.session_id=?1",
        [session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    Ok(value.unwrap_or(0) == 1)
}

pub(crate) fn set(store: &Store, bot_id: &str, fresh: bool) -> Result<bool> {
    super::get(store, bot_id)?;
    if fresh {
        store.db.execute(
            "INSERT INTO extended_bot_context(bot_id,fresh_each_message) VALUES(?1,1) ON CONFLICT(bot_id) DO UPDATE SET fresh_each_message=1",
            [bot_id],
        ).map_err(|e| e.to_string())?;
    } else {
        store.db.execute("DELETE FROM extended_bot_context WHERE bot_id=?1", [bot_id]).map_err(|e| e.to_string())?;
    }
    Ok(fresh)
}

/// Run only after admission excludes an active task. Saved messages, queued work,
/// schedules and memories remain untouched. Previously queued messages can still
/// receive the latest brief if the setting was enabled after they entered the queue.
pub(crate) fn prepare_turn(store: &Store, session_id: &str, text: &str) -> Result<Option<String>> {
    if !enabled_for_session(store, session_id)? {
        return Ok(None);
    }
    let (bot, thread) = owner_of(store, session_id)?.ok_or("The bot conversation no longer exists.")?;
    let kind = match thread.channel_id.as_str() {
        DESKTOP => "desktop".to_owned(),
        "schedule" => "schedule".to_owned(),
        channel => channels::kind_of(store, channel).unwrap_or_else(|| "web".to_owned()),
    };
    let sender = if thread.channel_id == DESKTOP { "You" } else { "" };
    let brief = brief::compose(&bot, &memories(store, &bot.id)?, &kind, sender);
    // A queued gateway message may name a different sender than the desktop
    // fallback above. Match the stable brief before sender-specific etiquette,
    // avoiding duplicate instructions while still refreshing outdated briefs.
    let stable_brief = brief.split("\nThe message comes from ").next().unwrap_or(&brief);
    let prompt = if text.contains(stable_brief) { text.to_owned() } else { format!("{text}{brief}") };
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    // An ordinary harness switch replays the transcript. A fresh bot turn must not.
    tx.execute("DELETE FROM context_recovery WHERE session_id=?1", [session_id]).map_err(|e| e.to_string())?;
    tx.execute("UPDATE sessions SET resume_id=NULL,context_used_tokens=NULL,context_window=NULL WHERE id=?1", [session_id]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(Some(prompt))
}

/// Profile-only action; a bot may not change its own security/context policy via a tool.
#[tauri::command]
pub async fn bot_set_fresh_context(app: tauri::AppHandle, window: tauri::WebviewWindow,
    state: State<'_, AppState>, bot_id: String, enabled: bool) -> Result<bool> {
    if window.label() != "main" { return Err("Bots are managed from the main window.".into()); }
    let saved = set(&*state.store.lock().map_err(|e| e.to_string())?, &bot_id, enabled)?;
    let _ = app.emit("bots-changed", ());
    Ok(saved)
}

/// User-controlled setting for incoming context from local shared bot channels.
/// It never deletes stored channel posts, bot memories or queued work.
#[tauri::command]
pub async fn bot_set_channel_history(app: tauri::AppHandle, window: tauri::WebviewWindow,
    state: State<'_, AppState>, bot_id: String, enabled: bool) -> Result<bool> {
    if window.label() != "main" { return Err("Bots are managed from the main window.".into()); }
    let saved = set_channel_history(&*state.store.lock().map_err(|e| e.to_string())?, &bot_id, enabled)?;
    let _ = app.emit("bots-changed", ());
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Fixture { store: Store, root: std::path::PathBuf }
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.root); } }
    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!("pandacode-extended-context-{}", crate::storage::id()));
        std::fs::create_dir_all(&root).unwrap();
        Fixture { store: Store::open(std::path::Path::new(":memory:")).unwrap(), root }
    }
    fn bot(f: &Fixture, name: &str) -> super::super::Bot {
        super::super::create(&f.store, &super::super::BotInput {
            name: name.into(), instructions: "Follow current instructions.".into(),
            provider: "claude".into(), model: "default".into(), mode: "ask".into(), ..Default::default()
        }, &f.root).unwrap()
    }
    #[test]
    fn off_by_default_and_scoped_to_one_bot_conversation() {
        let f = fixture();
        let scout = bot(&f, "Scout");
        let other = bot(&f, "Other");
        let (session, _) = super::super::thread_session(&f.store, &scout, DESKTOP, DESKTOP, "Scout").unwrap();
        f.store.message(&session, "user", "Prior message").unwrap();
        f.store.resume(&session, "native-existing").unwrap();
        assert!(!enabled(&f.store, &scout.id).unwrap());
        assert!(prepare_turn(&f.store, &session, "Follow-up").unwrap().is_none());
        assert_eq!(f.store.session(&session).unwrap().resume_id.as_deref(), Some("native-existing"));
        set(&f.store, &scout.id, true).unwrap();
        assert!(!enabled(&f.store, &other.id).unwrap());
        let project_task = f.store.create_session(&scout.project_id, "claude", "default", "ask").unwrap();
        assert!(!enabled_for_session(&f.store, &project_task.id).unwrap());
        assert!(prepare_turn(&f.store, &project_task.id, "Work").unwrap().is_none());
        assert!(set(&f.store, "missing-bot", true).is_err());
        set(&f.store, &scout.id, false).unwrap();
        assert!(!enabled(&f.store, &scout.id).unwrap());
    }
    #[test]
    fn fresh_turn_preserves_history_active_status_queue_memories_and_schedule() {
        let f = fixture();
        let bot = bot(&f, "Scout");
        let (session, _) = super::super::thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout").unwrap();
        f.store.message(&session, "user", "Earlier private question").unwrap();
        f.store.message(&session, "assistant", "Earlier private answer").unwrap();
        f.store.resume(&session, "native-old").unwrap();
        crate::context_recovery::queue(&f.store, &session).unwrap();
        f.store.status(&session, "running").unwrap();
        crate::message_queue::add(&f.store, &session, "Queued request", &json!([])).unwrap();
        let memory = super::super::remember(&f.store, &bot.id, "Use company terminology", "user").unwrap();
        super::super::schedule::create(&f.store, &bot.id, &super::super::schedule::ScheduleInput {
            prompt: "Review the queue later".into(), kind: "interval".into(), interval_minutes: Some(60),
            ..Default::default()
        }, "bot", 0).unwrap();
        set(&f.store, &bot.id, true).unwrap();
        assert!(!crate::context_recovery::pending(&f.store, &f.store.session(&session).unwrap()).unwrap());
        let prompt = prepare_turn(&f.store, &session, "New request").unwrap().unwrap();
        assert!(prompt.contains("Instructions from the user:\nFollow current instructions."));
        assert!(prompt.contains("Use company terminology"));
        assert!(!prompt.contains("Earlier private question"));
        assert!(f.store.session(&session).unwrap().resume_id.is_none());
        assert_eq!(f.store.session(&session).unwrap().status, "running");
        let pending: i64 = f.store.db.query_row("SELECT count(*) FROM context_recovery WHERE session_id=?1", [&session], |r| r.get(0)).unwrap();
        assert_eq!(pending, 0);
        assert_eq!(f.store.messages(&session, None).unwrap().len(), 2);
        assert_eq!(crate::message_queue::list(&f.store, Some(&session)).unwrap()[0].text, "Queued request");
        assert!(super::super::memories(&f.store, &bot.id).unwrap().iter().any(|item| item.id == memory.id));
        assert_eq!(super::super::schedule::list(&f.store, &bot.id).unwrap().len(), 1);
        f.store.resume(&session, "native-second").unwrap();
        assert!(prepare_turn(&f.store, &session, "Another request").unwrap().is_some());
        assert!(f.store.session(&session).unwrap().resume_id.is_none());
    }
    #[test]
    fn enabling_with_a_populated_queue_rebriefs_without_rewriting_requests() {
        let f = fixture();
        let bot = bot(&f, "Scout");
        let (session, _) = super::super::thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout").unwrap();
        f.store.message(&session, "user", "Old conversation").unwrap();
        f.store.resume(&session, "native-old").unwrap();
        let queued = "Queued before enabling the setting";
        crate::message_queue::add(&f.store, &session, queued, &json!([])).unwrap();
        set(&f.store, &bot.id, true).unwrap();
        let prompt = prepare_turn(&f.store, &session, queued).unwrap().unwrap();
        assert!(prompt.starts_with(queued));
        assert!(prompt.contains("Follow current instructions."));
        assert_eq!(crate::message_queue::list(&f.store, Some(&session)).unwrap()[0].text, queued);
        assert_eq!(prepare_turn(&f.store, &session, &prompt).unwrap().unwrap(), prompt,
            "current instructions are not injected twice");
    }
    #[test]
    fn gateway_brief_with_a_different_sender_is_not_duplicated() {
        let f = fixture();
        let bot = bot(&f, "Scout");
        let (session, _) = super::super::thread_session(&f.store, &bot, DESKTOP, DESKTOP, "Scout").unwrap();
        set(&f.store, &bot.id, true).unwrap();
        let attached = brief::compose(&bot, &[], "desktop", "Message from Partner");
        let prompt = format!("Message from Partner:\n\nReview this{}", attached);
        let fresh = prepare_turn(&f.store, &session, &prompt).unwrap().unwrap();
        assert_eq!(fresh.matches(brief::HEADER).count(), 1, "gateway and launch must not inject two briefs");
        assert_eq!(fresh, prompt);
    }
    #[test]
    fn shared_channel_history_can_be_disabled_per_bot_without_changing_fresh_context() {
        let f = fixture();
        let scout = bot(&f, "Scout");
        let reviewer = bot(&f, "Reviewer");
        let channel = super::super::shared_channels::create(
            &f.store, "test", &[scout.id.clone(), reviewer.id.clone()]
        ).unwrap();
        f.store.db.execute(
            "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
             VALUES('prior-post',?1,'user','my phone is blue','completed',1)",
            [&channel.id],
        ).unwrap();
        f.store.db.execute(
            "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at)
             VALUES('current-post',?1,'user','What color is my phone?','in_progress',2)",
            [&channel.id],
        ).unwrap();
        let assign = |bot_id: &str| super::super::shared_channels::assignment_text(
            &f.store, &channel, bot_id, "current-post", "What color is my phone?"
        ).unwrap();

        assert!(include_shared_channel_history(&f.store, &scout.id).unwrap());
        assert!(assign(&scout.id).contains("my phone is blue"), "upstream history is on by default");

        set(&f.store, &scout.id, true).unwrap();
        assert!(assign(&scout.id).contains("my phone is blue"),
            "fresh native harness context must not automatically remove shared-channel history");

        assert!(!set_channel_history(&f.store, &scout.id, false).unwrap());
        let prompt = assign(&scout.id);
        assert_eq!(prompt, "Work requested in #test:\n\nWhat color is my phone?");
        assert!(!prompt.contains("my phone is blue"));
        assert!(assign(&reviewer.id).contains("my phone is blue"),
            "the second bot retains normal channel context");
        assert!(super::super::shared_channels::recent_history(
            &f.store, &channel.id, "current-post"
        ).unwrap().contains("my phone is blue"), "channel history is preserved");
        assert!(enabled(&f.store, &scout.id).unwrap(),
            "channel preference cannot change fresh native-context preference");
        assert!(set_channel_history(&f.store, "nonexistent", false).is_err());

        super::super::reset(&f.store, &scout.id).unwrap();
        assert!(!include_shared_channel_history(&f.store, &scout.id).unwrap(),
            "bot reset keeps the configuration just like other profile settings");
        set_channel_history(&f.store, &scout.id, true).unwrap();
        assert!(include_shared_channel_history(&f.store, &scout.id).unwrap());
        set_channel_history(&f.store, &scout.id, false).unwrap();
        super::super::delete(&f.store, &scout.id).unwrap();
        let count: i64 = f.store.db.query_row(
            "SELECT count(*) FROM extended_bot_channel_history WHERE bot_id=?1",
            [&scout.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0, "deleting the bot cascades the preference");
    }

    #[test]
    fn preference_survives_reset_and_is_removed_with_bot_deletion() {
        let f = fixture();
        let bot = bot(&f, "Scout");
        set(&f.store, &bot.id, true).unwrap();
        super::super::reset(&f.store, &bot.id).unwrap();
        assert!(enabled(&f.store, &bot.id).unwrap());
        super::super::delete(&f.store, &bot.id).unwrap();
        let rows: i64 = f.store.db.query_row("SELECT count(*) FROM extended_bot_context", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0);
    }
}
