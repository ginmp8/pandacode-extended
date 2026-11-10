//! Resolve existing persistent bots as immutable Council reviewer identities.
//! Never modify bot profiles, ordinary conversations, memories or permissions.
use super::{council::CouncilConfig, TeamSnapshot, WorkerProfile, HARNESSES};
use crate::{bots, model_health, model_routing, providers, storage::Store, Result};
use rusqlite::OptionalExtension;
use std::collections::HashSet;

/// Only a direct bot conversation or explicitly assigned bot task establishes
/// executor identity. Sharing a project does not identify an executing bot.
fn executor(store: &Store, session_id: &str) -> Result<Option<String>> {
    let linked: Option<String> = store.db.query_row(
        "SELECT bot_id FROM bot_threads WHERE session_id=?1 UNION ALL SELECT bot_id FROM bot_tasks WHERE session_id=?1 LIMIT 1",
        [session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if linked.is_some() { return Ok(linked); }
    // Project membership is not executor identity: several bots may share a project.
    // Only explicit bot task/thread ownership proves who is executing this session.
    Ok(None)
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

/// Return the same server-side eligibility rules used for actual dispatch, including
/// executable installation and model health. A UI can never authorize a bot itself.
pub(super) fn options(store: &Store, session_id: &str) -> Result<Vec<serde_json::Value>> {
    let executor = executor(store, session_id)?;
    let mut result = Vec::new();
    for bot in bots::list(store)? {
        let reason = valid_bot(&bot, executor.as_deref(), true).err()
            .map(|e| e.trim_start_matches("reviewer_unavailable: ").to_string());
        result.push(serde_json::json!({
            "id": bot.id, "name": bot.name, "role": bot.role,
            "provider": bot.provider, "model": bot.model, "paused": bot.paused,
            "available": reason.is_none(), "reason": reason
        }));
    }
    Ok(result)
}

/// S2 admits only host actions whose scope is provable at the native boundary.
/// A requested action cannot self-authorize a tool or inherit an external MCP server.
pub(super) fn review_policy(
    bot: &bots::Bot,
    permissions: &super::reviewer_security::ReviewerPermissionSelection,
) -> Result<super::reviewer_security::ReviewerToolPolicy> {
    use super::reviewer_security::{ReviewerCapabilityGrant, ReviewerPhase, ReviewerToolPolicy};
    // The bot's normal native host catalog is the configured capability source,
    // never the untrusted request. ZCode has no host-tool bridge.
    if bot.provider == "zcode"
        || !crate::agent_tools::catalog().iter().any(|tool| {
            tool["name"] == "pandacode_open_file"
                && tool["inputSchema"]["properties"]["path"]["type"] == "string"
        })
    {
        return Err("policy_unsupported: this bot has no enforceable host-tool bridge.".into());
    }
    let mut configured: Vec<ReviewerCapabilityGrant> = Vec::new();
    for grant in &permissions.grants {
        // Opening one exact project-relative file in the app inspector cannot mutate
        // the reviewed checkout, bot state or external account. Other native tool
        // actions need a verified bot capability map and must fail closed here.
        let valid_path = !grant.resource.starts_with('/')
            && !grant.resource.starts_with('\\')
            && !grant.resource.contains(':')
            && !grant.resource.split(&['/', '\\'][..]).any(|part| part == ".." || part.is_empty());
        if grant.tool != "pandacode_open_file" || grant.action != "call"
            || grant.action_argument.is_some() || grant.resource_argument != "path" || !valid_path
        {
            return Err("policy_unsupported: no configured, enforceable scoped reviewer host action matches that grant.".into());
        }
        configured.push(grant.clone());
    }
    // No ambient tool grants: the configured host manifest and exact S1 wrapper
    // intersect with the user's explicit task authorization on every invocation.
    ReviewerToolPolicy::from_user_grants(
        ReviewerPhase::Review, &permissions.authorization_id, &configured, &permissions.grants,
    )
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
        if let Some(permissions) = &council.review_permissions {
            review_policy(&bot, permissions)?;
        }
    }
    Ok(())
}

/// A previously scored bot reserves its identity; an abandoned unscored or
/// historical-cycle seat does not. This must be identical for retries and new rounds.
pub(super) fn occupied_bot_ids<'a>(team: &'a TeamSnapshot, round: usize, replaces: Option<&str>) -> HashSet<&'a str> {
    team.workers.iter()
        .filter(|w| w.created_at >= team.council_since)
        .filter(|w| w.council.as_ref().is_some_and(|s| s.round == round))
        .filter(|w| !w.abandoned || w.council.as_ref().is_some_and(|s| s.score.is_some()))
        .filter(|w| Some(w.id.as_str()) != replaces)
        .filter_map(|w| w.reviewer_binding.as_ref()?.reviewer_bot_id.as_deref())
        .collect()
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
    let occupied = occupied_bot_ids(team, round, replaces);
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
    let old_policy = team.config.council.as_ref().and_then(|c| c.review_permissions.as_ref());
    let new_policy = next.as_ref().and_then(|c| c.review_permissions.as_ref());
    if (old_ids == new_ids && old_policy == new_policy) || next.is_none() { return Ok(()); }
    let current = team.workers.iter().filter(|w| w.created_at >= team.council_since).filter_map(|w| w.council.as_ref()).map(|seat| seat.round).max().unwrap_or(0);
    let incomplete = current > 0 && super::council::current_round_incomplete(team);
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

        let grant = super::super::reviewer_security::ReviewerCapabilityGrant {
            tool: "pandacode_open_file".into(), action: "call".into(),
            action_argument: None, resource_argument: "path".into(), resource: "src/main.ts".into(),
        };
        let user = super::super::reviewer_security::ReviewerPermissionSelection {
            authorization_id: "user-selected-audit-review".into(), grants: vec![grant.clone()],
        };
        assert!(matches!(review_policy(&bot, &user), Ok(super::super::reviewer_security::ReviewerToolPolicy::Scoped(_))));
        let escaped = super::super::reviewer_security::ReviewerPermissionSelection {
            grants: vec![super::super::reviewer_security::ReviewerCapabilityGrant { resource: "../secrets".into(), ..grant.clone() }],
            ..user.clone()
        };
        assert!(review_policy(&bot, &escaped).unwrap_err().contains("policy_unsupported"));
        let unsupported = super::super::reviewer_security::ReviewerPermissionSelection {
            grants: vec![super::super::reviewer_security::ReviewerCapabilityGrant { tool: "pandacode_create_task".into(), ..grant }],
            ..user
        };
        assert!(review_policy(&bot, &unsupported).unwrap_err().contains("policy_unsupported"));
    }
}
