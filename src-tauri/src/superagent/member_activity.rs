//! Team-scoped, read-only assignment activity for the persistent Orchestrations workspace.
//! Activity is not a provider completion, a Council verdict, or a tool grant.

use super::{configuration, require_main_window, CHANNEL};
use crate::{storage::Store, AppState, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemberActivityCursor {
    pub created_at: i64,
    pub assignment_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberActivityItem {
    pub assignment_id: String,
    pub ticket_id: String,
    pub orchestration_id: String,
    pub bot_id: String,
    pub session_id: String,
    pub assignment_state: String,
    pub ticket_state: String,
    pub role: String,
    pub attempt: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub has_result: bool,
    pub chat_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberActivityPage {
    pub orchestration_id: String,
    pub bot_id: String,
    pub items: Vec<MemberActivityItem>,
    pub has_more: bool,
    pub next_cursor: Option<MemberActivityCursor>,
}

/// Query by BOTH immutable Ticket Team and Assignment bot identity before
/// limiting. A bot reused across Teams cannot expose foreign Ticket history.
/// Missing/removed sessions remain visible as history, but never as open chats.
pub(super) fn page(
    store: &Store,
    orchestration_id: &str,
    bot_id: &str,
    before: Option<MemberActivityCursor>,
    limit: i64,
) -> Result<MemberActivityPage> {
    if !(1..=100).contains(&limit) {
        return Err("Team member activity page size must be between 1 and 100.".into());
    }
    if bot_id.trim().is_empty() || bot_id.len() > 128 || bot_id.chars().any(char::is_control) {
        return Err("A valid existing or historical bot identity is required.".into());
    }
    if let Some(cursor) = &before {
        if cursor.created_at <= 0 || cursor.assignment_id.is_empty()
            || cursor.assignment_id.len() > 128
            || !cursor.assignment_id.bytes().all(|b| b.is_ascii_alphanumeric()
                || b == b'-' || b == b'_')
        {
            return Err("Invalid Team member activity cursor.".into());
        }
    }
    configuration(store, orchestration_id)?;
    let cursor_at = before.as_ref().map(|cursor| cursor.created_at);
    let cursor_id = before.as_ref().map(|cursor| cursor.assignment_id.as_str());
    let mut stmt = store.db.prepare(
        "SELECT a.id,t.id,t.orchestration_id,a.bot_id,a.session_id,
                a.state,t.state,a.attempt,a.created_at,a.updated_at,
                length(trim(COALESCE(a.result,'')))>0,
                EXISTS(SELECT 1 FROM bot_threads thread
                  WHERE thread.session_id=a.session_id AND thread.bot_id=a.bot_id
                    AND thread.channel_id=?6),
                CASE WHEN a.attempt=1 AND EXISTS(
                  SELECT 1 FROM sa_audit initial
                  WHERE initial.ticket_id=t.id AND initial.kind='admitted'
                    AND initial.detail='manual readonly coordinator reserved'
                ) THEN 'coordinator' ELSE 'worker' END
         FROM sa_assignments a JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE t.orchestration_id=?1 AND a.bot_id=?2
           AND (?3 IS NULL OR a.created_at<?3 OR
                (a.created_at=?3 AND a.id<?4))
         ORDER BY a.created_at DESC,a.id DESC LIMIT ?5"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(
        params![orchestration_id, bot_id, cursor_at, cursor_id, limit + 1, CHANNEL],
        |row| Ok(MemberActivityItem {
            assignment_id: row.get(0)?,
            ticket_id: row.get(1)?,
            orchestration_id: row.get(2)?,
            bot_id: row.get(3)?,
            session_id: row.get(4)?,
            assignment_state: row.get(5)?,
            ticket_state: row.get(6)?,
            attempt: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            has_result: row.get::<_, i64>(10)? != 0,
            chat_available: row.get::<_, i64>(11)? != 0,
            role: row.get(12)?,
        }),
    ).map_err(|e| e.to_string())?;
    let mut items = rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let has_more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next_cursor = if has_more {
        items.last().map(|item| MemberActivityCursor {
            created_at: item.created_at,
            assignment_id: item.assignment_id.clone(),
        })
    } else {
        None
    };
    Ok(MemberActivityPage {
        orchestration_id: orchestration_id.into(),
        bot_id: bot_id.into(),
        items,
        has_more,
        next_cursor,
    })
}

#[tauri::command]
pub async fn superagent_member_activity_page(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    orchestration_id: String,
    bot_id: String,
    before: Option<MemberActivityCursor>,
    limit: Option<i64>,
) -> Result<MemberActivityPage> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    page(&store, &orchestration_id, &bot_id, before, limit.unwrap_or(25))
}
