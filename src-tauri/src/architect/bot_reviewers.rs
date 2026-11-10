//! Resolve existing persistent bots as immutable Council reviewer identities.
//! Never modify bot profiles, ordinary conversations, memories or permissions.
use super::{council::CouncilConfig, TeamSnapshot, WorkerProfile, HARNESSES};
use crate::{bots, model_health, model_routing, providers, storage::Store, Result};
use rusqlite::OptionalExtension;
use std::collections::HashSet;

/// A direct bot conversation or an explicitly assigned task has an executor.
/// A session in the bot's own project is also considered its execution context.
fn executor(store: &Store, session_id: &str) -> Result<Option<String>> {
    let linked: Option<String> = store.db.query_row(
        "SELECT bot_id FROM bot_threads WHERE session_id=?1 UNION ALL SELECT bot_id FROM bot_tasks WHERE session_id=?1 LIMIT 1",
        [session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if linked.is_some() { return Ok(linked); }
    let project = store.session(session_id)?.project_id;
    Ok(bots::list(store)?.into_iter().find(|b| b.project_id == project).map(|b| b.id))
}

fn valid_bot(bot: &bots::Bot, executor: Option<&str>, check_executable: bool) -> Result<()> {
    if executor == Some(bot.id.as_str()) {
        return Err("reviewer_unavailable: a bot cannot review its own task.".into());
    }
    if bot.paused {
        return Err(format!("reviewer_unavailable: selected bot {:?} is paused.", bot.name));
    }
    if !HARNESSES.contains(&bot.provider.as_str())
        || bot.model.is_empty()
        || !model_routing::allowed(&bot.provider, &bot.model)
        || !model_health::is_healthy(&bot.provider, &bot.model)
        || (check_executable && providers::executable(&bot.provider).is_none())
    {
        return Err(format!("reviewer_unavailable: selected bot {:?} has no compatible available harness/model.", bot.name));
    }
    providers::validate_reasoning_effort(&bot.provider, (!bot.reasoning_effort.is_empty()).then_some(bot.reasoning_effort.as_str()))
        .map_err(|reason| format!("reviewer_unavailable: {reason}"))?;
    Ok(())
}

/// Called on a user setting change and again before dispatch, not by model output.
pub(super) fn validate_roster(store: &Store, session_id: &str, council: &CouncilConfig) -> Result<()> {
    council.validate()?;
    if council.reviewer_bot_ids.is_empty() { return Ok(()); }
    let executor = executor(store, session_id)?;
    let mut used = HashSet::new();
    for id in &council.reviewer_bot_ids {
        if !used.insert(id) {
            return Err("reviewer_unavailable: repeated reviewer bot ID.".into());
        }
        let bot = bots::get(store, id).map_err(|_| format!("reviewer_unavailable: selected bot {id} was removed."))?;
        valid_bot(&bot, executor.as_deref(), true)?;
    }
    Ok(())
}

/// Resolve a selected persistent reviewer for one native seat. A replacement keeps its bot.
/// Never silently use a native worker or another bot when the intended seat is unavailable.
pub(super) fn seat_bot(store: &Store, team: &TeamSnapshot, round: usize, replaces: Option<&str>) -> Result<Option<bots::Bot>> {
    let Some(config) = team.config.council.as_ref() else { return Ok(None) };
    if config.reviewer_bot_ids.is_empty() { return Ok(None); }
    validate_roster(store, &team.session_id, config)?;
    let old = replaces.and_then(|id| team.workers.iter().find(|w| w.id == id));
    let inherited = old.and_then(|w| w.reviewer_binding.as_ref()).and_then(|b| b.reviewer_bot_id.as_deref());
    if old.is_some() && inherited.is_none() {
        return Err("reviewer_session_mismatch: a selected seat cannot replace a native reviewer.".into());
    }
    let occupied: HashSet<&str> = team.workers.iter()
        .filter(|w| w.council.as_ref().is_some_and(|s| s.round == round))
        .filter(|w| Some(w.id.as_str()) != replaces)
        .filter_map(|w| w.reviewer_binding.as_ref()?.reviewer_bot_id.as_deref())
        .collect();
    let selected = match inherited {
        Some(id) => id,
        None => config.reviewer_bot_ids.iter().find(|id| !occupied.contains(id.as_str()))
            .map(String::as_str)
            .ok_or("All selected reviewer bots already have a seat in this round.")?,
    };
    if occupied.contains(selected) || !config.reviewer_bot_ids.iter().any(|id| id == selected) {
        return Err("reviewer_session_mismatch: reviewer identity is duplicated or no longer selected.".into());
    }
    let bot = bots::get(store, selected).map_err(|_| format!("reviewer_unavailable: selected bot {selected} was removed."))?;
    valid_bot(&bot, executor(store, &team.session_id)?.as_deref(), true)?;
    Ok(Some(bot))
}

pub(super) fn profile(bot: &bots::Bot) -> WorkerProfile {
    WorkerProfile {
        id: format!("bot-{}", bot.id), name: bot.name.clone(),
        provider: bot.provider.clone(), model: bot.model.clone(),
        reasoning_effort: (!bot.reasoning_effort.is_empty()).then_some(bot.reasoning_effort.clone()),
    }
}

/// Changing the roster partway through a live or incomplete round would reassign
/// reviewers and silently change the required vote count.
pub(super) fn allow_roster_change(team: &TeamSnapshot, next: &Option<CouncilConfig>) -> Result<()> {
    let old_ids = team.config.council.as_ref().map(|c| &c.reviewer_bot_ids);
    let new_ids = next.as_ref().map(|c| &c.reviewer_bot_ids);
    if old_ids == new_ids || next.is_none() { return Ok(()); }
    let current = team.workers.iter().filter_map(|w| w.council.as_ref()).map(|seat| seat.round).max().unwrap_or(0);
    let incomplete = current > 0
        && !matches!(team.council_status.as_ref().map(|s| s.state.as_str()), Some("passed" | "fixing" | "limit" | "asking" | "accepted"));
    if incomplete {
        return Err("Finish or stop the current Council round before changing selected reviewer bots.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executor_and_paused_bots_are_never_reviewers() {
        let bot = bots::Bot {
            id: "reviewer-one".into(), name: "Review One".into(), role: "QA".into(),
            instructions: "Inspect".into(), goals: String::new(), soul: String::new(),
            avatar: String::new(), project_id: "project".into(), provider: "codex".into(),
            model: "gpt-5".into(), mode: "full".into(), heartbeat_minutes: 0, next_heartbeat_at: None,
            paused: false, memory_version: 0, notify_channel_id: None, notify_thread: None,
            created_at: 0, updated_at: 0, color: String::new(), reasoning_effort: String::new(),
            backup_models: vec![], fallback_mode: "ask".into(), notify: true, while_working: "queue".into(),
            show_plan: false, skills: vec![], mcp: vec![],
        };
        assert!(valid_bot(&bot, Some("reviewer-one"), false).unwrap_err().contains("own task"));
        assert!(valid_bot(&bots::Bot { paused: true, ..bot.clone() }, None, false).unwrap_err().contains("paused"));
    }
}
