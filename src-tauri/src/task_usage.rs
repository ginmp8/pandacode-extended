//! Per-task token totals from the agents' own local session logs, attributed through
//! the harness session ids each task has run under (see usage_ledger).

use crate::superagent::member_activity::{assignment_usage_scope, AssignmentUsageScope};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn task_token_usage(
    state: tauri::State<'_, crate::AppState>,
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    session_id: String,
    since: Option<i64>,
) -> crate::Result<Option<crate::usage_ledger::TaskUsage>> {
    if window.label() != "main" {
        return Err("Task usage is available only in the main application window.".into());
    }
    let (scope, ordinary_links) = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let scope = assignment_usage_scope(&store, &session_id)?;
        let links = if scope == AssignmentUsageScope::Ordinary {
            store.usage_links(&session_id)?
        } else {
            Vec::new()
        };
        (scope, links)
    };
    let usage = match &scope {
        AssignmentUsageScope::Unknown => return Ok(None),
        AssignmentUsageScope::Ordinary => {
            if ordinary_links.is_empty() {
                return Ok(None);
            }
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            crate::usage_ledger::task_usage(dir, ordinary_links, since).await?
        }
        AssignmentUsageScope::Bound { agent, links, .. } => {
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let usage = crate::usage_ledger::assignment_task_usage(
                dir, *agent, links.clone(), since,
            ).await?;
            // The log scan releases the database lock. Do not return stale
            // attribution if the session, thread or link ownership changed.
            let store = state.store.lock().map_err(|e| e.to_string())?;
            if assignment_usage_scope(&store, &session_id)? != scope {
                return Ok(None);
            }
            usage
        }
    };
    Ok(usage.last_used.map(|_| usage))
}
