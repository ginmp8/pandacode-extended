//! A bot profile may be selected for a fresh ordinary project task. These links
//! never create a channel identity, notification target or bot-tool ownership.

use super::{brief, get, memories, Bot};
use crate::{storage::{Session, Store}, AppState, Result};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use tauri::{Emitter, State};

pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS bot_tasks(
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  brief_version INTEGER NOT NULL DEFAULT -1);";

pub(crate) fn linked(store: &Store, session_id: &str) -> Result<Option<(Bot, i64)>> {
    let found: Option<(String, i64)> = store.db.query_row(
        "SELECT bot_id,brief_version FROM bot_tasks WHERE session_id=?1", [session_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    found.map(|(id, seen)| Ok((get(store, &id)?, seen))).transpose()
}

pub(crate) fn links(store: &Store) -> Result<HashMap<String, String>> {
    let mut statement = store.db.prepare("SELECT session_id,bot_id FROM bot_tasks").map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
}

/// Called under agents/store locks. Defaults and the link change together, while
/// the task retains its project, checkout and explicit permission mode.
pub(crate) fn assign(store: &Store, session_id: &str, bot_id: Option<&str>) -> Result<Option<Bot>> {
    let task = store.session(session_id)?;
    if task.remote_device.is_some() { return Err("Choose a bot for a local project task.".into()); }
    if super::project_ids(store)?.contains(&task.project_id) {
        return Err("A bot's own conversations already use that bot.".into());
    }
    if crate::chats::is_chat_folder(std::path::Path::new(&store.project(&task.project_id)?.path)) {
        return Err("Choose a bot for a project task, not a standalone chat.".into());
    }
    if crate::architect::load(store, session_id)?.is_some() || crate::architect::parent_of(store, session_id)?.is_some() {
        return Err("A team manages its own agents. Choose a bot for an ordinary project task.".into());
    }
    let started: bool = store.db.query_row("SELECT EXISTS(SELECT 1 FROM messages WHERE session_id=?1)", [session_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if started || task.resume_id.is_some() || task.archived || task.task_stage == "completed" || matches!(task.status.as_str(), "running" | "waiting") {
        return Err("Choose a bot before this task's first message.".into());
    }
    let bot = bot_id.map(|id| get(store, id)).transpose()?;
    if bot.as_ref().is_some_and(|bot| bot.paused) { return Err("Resume this bot before assigning it to a task.".into()); }
    let transaction = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    if let Some(bot) = &bot {
        store.configure(session_id, &bot.provider, &bot.model, None)?;
        store.db.execute("INSERT INTO bot_tasks(session_id,bot_id) VALUES(?1,?2) ON CONFLICT(session_id) DO UPDATE SET bot_id=excluded.bot_id,brief_version=-1", params![session_id, bot.id]).map_err(|e| e.to_string())?;
    } else {
        store.db.execute("DELETE FROM bot_tasks WHERE session_id=?1", [session_id]).map_err(|e| e.to_string())?;
    }
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(bot)
}

pub(crate) struct TaskBrief {
    pub bot_id: String,
    pub version: i64,
    pub text: String,
}

/// No frontend flag can suppress this: it runs for sends, queue delivery and
/// native task starts under the same admission lock as the saved task.
pub(crate) fn prepare(store: &Store, task: &Session, fresh: bool) -> Result<Option<TaskBrief>> {
    let Some((bot, seen)) = linked(store, &task.id)? else { return Ok(None); };
    if bot.paused { return Err("This bot is paused. Resume it before continuing this task.".into()); }
    if !fresh && seen == bot.memory_version { return Ok(None); }
    let text = brief::compose(&bot, &memories(store, &bot.id)?, brief::TASK, "");
    // Memory can grow independently of the personality. Refuse rather than silently
    // truncate instructions or the safety rules at the end of the brief.
    if text.len() > 128 * 1024 { return Err("This bot's context exceeds 128 KiB. Shorten its personality, instructions or saved memories before starting this task.".into()); }
    Ok(Some(TaskBrief { bot_id: bot.id, version: bot.memory_version, text }))
}

/// The initiating message and exact prepared brief version are committed together.
pub(crate) fn mark(store: &Store, session_id: &str, brief: &TaskBrief) -> Result<()> {
    let changed = store.db.execute("UPDATE bot_tasks SET brief_version=?3 WHERE session_id=?1 AND bot_id=?2", params![session_id, brief.bot_id, brief.version]).map_err(|e| e.to_string())?;
    if changed != 1 { return Err("This task's bot changed before its context was admitted.".into()); }
    Ok(())
}

pub(crate) fn check_console_context(store: &Store, task: &Session) -> Result<()> {
    if prepare(store, task, task.resume_id.is_none())?.is_some() {
        return Err("Send a normal message in this task to apply its bot profile before opening the harness console.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn bot_assign_task(app: tauri::AppHandle, window: tauri::WebviewWindow, state: State<'_, AppState>, session_id: String, bot_id: Option<String>) -> Result<Option<Bot>> {
    super::guard_main(&window)?;
    let agents = state.agents.lock().map_err(|e| e.to_string())?;
    if agents.contains_key(&session_id) || crate::terminal::harness_running(&state.terminals, &session_id)? {
        return Err("Stop this task's agent or harness console before choosing a bot.".into());
    }
    let bot = assign(&*state.store.lock().map_err(|e| e.to_string())?, &session_id, bot_id.as_deref())?;
    let _ = app.emit("bots-changed", ());
    let _ = app.emit("sessions-changed", ());
    Ok(bot)
}

#[tauri::command]
pub async fn bot_task_links(state: State<'_, AppState>) -> Result<HashMap<String, String>> {
    links(&*state.store.lock().map_err(|e| e.to_string())?)
}
