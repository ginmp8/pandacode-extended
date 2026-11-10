//! Resolve existing persistent bots as immutable Council reviewer identities.
//! Never modify bot profiles, ordinary conversations, memories or permissions.
use super::{council::CouncilConfig, TeamSnapshot, WorkerProfile, HARNESSES};
use crate::{bots, model_health, model_routing, providers, storage::Store, Result};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
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

/// All selected profiles are captured together, before the first seat is issued. Store this in a
/// native-only request record, separate from mutable worker snapshots and bot profiles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct SelectedReviewer {
    pub id: String,
    pub roster_bot_ids: Vec<String>,
    pub profile: WorkerProfile,
    pub context: super::reviewer_security::ReviewerBotContext,
    pub tool_policy: super::reviewer_security::ReviewerToolPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<crate::skills::reviewer::ReviewerSkills>,
}

fn round_reviewers(store: &Store, team: &TeamSnapshot, round: usize, check_executable: bool, library: Option<&crate::skills::Library>) -> Result<Vec<SelectedReviewer>> {
    let key = format!("native-reviewer-round:{}:{round}", team.council_since);
    let saved: Option<String> = store.db.query_row(
        "SELECT content FROM architect_requests WHERE parent_session_id=?1 AND key=?2 AND kind='reviewer_round'",
        rusqlite::params![team.session_id, key], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if let Some(raw) = saved {
        let frozen: Vec<SelectedReviewer> = serde_json::from_str(&raw)
            .map_err(|_| "reviewer_session_mismatch: invalid frozen reviewer round.".to_string())?;
        if check_executable && frozen.iter().any(|r| !r.context.skill_ids.is_empty() && r.skills.is_none()) {
            return Err("reviewer_skill_unavailable: legacy round has assigned skills without immutable pins; start a new round.".into());
        }
        for reviewer in &frozen {
            if let Some(skills) = &reviewer.skills { skills.verify()?; }
        }
        return Ok(frozen);
    }
    // Older S2 rounds only pinned profiles as seats were issued. Re-reading the remaining bots
    // would invent a snapshot; require explicit recovery instead.
    if team.workers.iter().any(|w| w.created_at >= team.council_since && w.council.as_ref().is_some_and(|s| s.round == round)) {
        return Err("reviewer_session_mismatch: this legacy round has no frozen roster. Stop its reviewers and change the selected roster to start a new round.".into());
    }
    let config = team.config.council.as_ref().ok_or("Council is off.")?;
    config.validate()?;
    let executor = executor(store, &team.session_id)?;
    let mut reviewers = Vec::new();
    for id in &config.reviewer_bot_ids {
        let bot = bots::get(store, id).map_err(|_| format!("reviewer_unavailable: selected bot {id} was removed."))?;
        valid_bot(&bot, executor.as_deref(), check_executable)?;
        let tool_policy = config.review_permissions.as_ref().map(|permissions| review_policy(&bot, permissions)).transpose()?.unwrap_or(super::reviewer_security::ReviewerToolPolicy::NativeReadOnly);
        let skills = if bot.skills.is_empty() { None } else if let Some(library) = library {
            Some(crate::skills::reviewer::freeze(library, &bot.skills, &bot.provider)?)
        } else if check_executable {
            return Err("reviewer_skill_unavailable: central Skills Library could not be opened.".into());
        } else { None };
        reviewers.push(SelectedReviewer {
            id: id.clone(), roster_bot_ids: config.reviewer_bot_ids.clone(), profile: profile(&bot),
            context: super::reviewer_security::ReviewerBotContext::from_bot(&bot), tool_policy, skills,
        });
    }
    super::record_request(store, &team.session_id, &key, "reviewer_round", &serde_json::to_string(&reviewers).map_err(|e| e.to_string())?, "")?;
    Ok(reviewers)
}

/// Resolve a frozen selected reviewer. Live deletion, pause, executor identity and frozen-model
/// availability are still checked, without substituting a newly edited profile or another bot.
pub(super) fn seat_bot(store: &Store, team: &TeamSnapshot, round: usize, replaces: Option<&str>, library: Option<&crate::skills::Library>) -> Result<Option<SelectedReviewer>> {
    seat_bot_with_skills(store, team, round, replaces, true, library)
}
#[cfg(test)]
pub(super) fn seat_bot_inner(store: &Store, team: &TeamSnapshot, round: usize, replaces: Option<&str>, check_executable: bool) -> Result<Option<SelectedReviewer>> {
    seat_bot_with_skills(store, team, round, replaces, check_executable, None)
}
#[cfg(test)]
pub(super) fn seat_bot_inner_with_skills(store: &Store, team: &TeamSnapshot, round: usize, replaces: Option<&str>, library: &crate::skills::Library) -> Result<Option<SelectedReviewer>> {
    seat_bot_with_skills(store, team, round, replaces, false, Some(library))
}
fn seat_bot_with_skills(store: &Store, team: &TeamSnapshot, round: usize, replaces: Option<&str>, check_executable: bool, library: Option<&crate::skills::Library>) -> Result<Option<SelectedReviewer>> {
    let Some(config) = team.config.council.as_ref() else { return Ok(None) };
    if config.reviewer_bot_ids.is_empty() { return Ok(None); }
    let reviewers = round_reviewers(store, team, round, check_executable, library)?;
    let old = replaces.and_then(|id| team.workers.iter().find(|w| w.id == id));
    let inherited = old.and_then(|w| w.reviewer_binding.as_ref()).and_then(|b| b.reviewer_bot_id.as_deref());
    if old.is_some() && inherited.is_none() {
        return Err("reviewer_session_mismatch: a selected seat cannot replace a native reviewer.".into());
    }
    let occupied = occupied_bot_ids(team, round, replaces);
    let selected = match inherited {
        Some(id) => id,
        None => reviewers.iter().find(|bot| !occupied.contains(bot.id.as_str()))
            .map(|bot| bot.id.as_str())
            .ok_or("All selected reviewer bots already have a seat in this round.")?,
    };
    if occupied.contains(selected) {
        return Err("reviewer_session_mismatch: reviewer identity is duplicated or no longer selected.".into());
    }
    let frozen = reviewers.iter().find(|bot| bot.id == selected)
        .ok_or("reviewer_session_mismatch: replacement bot is absent from the frozen round.")?;
    let mut available = bots::get(store, selected).map_err(|_| format!("reviewer_unavailable: selected bot {selected} was removed."))?;
    available.provider = frozen.profile.provider.clone();
    available.model = frozen.profile.model.clone();
    available.reasoning_effort = frozen.context.reasoning_effort.clone();
    valid_bot(&available, executor(store, &team.session_id)?.as_deref(), check_executable)?;
    if let Some(binding) = old.and_then(|w| w.reviewer_binding.as_ref()) {
        super::reviewer_security::validate_issued_authority(store, &team.session_id, binding)?;
        if binding.round == round && (binding.snapshot.provider != frozen.profile.provider || binding.snapshot.model != frozen.profile.model
            || binding.snapshot.bot_context.as_ref() != Some(&frozen.context)
            || binding.snapshot.tool_policy != frozen.tool_policy || binding.roster_bot_ids != frozen.roster_bot_ids)
        {
            return Err("reviewer_session_mismatch: replacement differs from its frozen round.".into());
        }
    }
    Ok(Some(frozen.clone()))
}

fn profile(bot: &bots::Bot) -> WorkerProfile {
    WorkerProfile {
        id: format!("bot-{}", bot.id), name: bot.name.clone(),
        provider: bot.provider.clone(), model: bot.model.clone(),
        reasoning_effort: (!bot.reasoning_effort.is_empty()).then_some(bot.reasoning_effort.clone()),
    }
}

fn selection_changed(team: &TeamSnapshot, next: &Option<CouncilConfig>) -> bool {
    let old = team.config.council.as_ref();
    let next = next.as_ref();
    old.map(|c| &c.reviewer_bot_ids) != next.map(|c| &c.reviewer_bot_ids)
        || old.and_then(|c| c.review_permissions.as_ref()) != next.and_then(|c| c.review_permissions.as_ref())
}

/// Live rounds must settle first. A user change can explicitly recover a stopped/incomplete
/// round, while fixed native lenses must remain satisfiable by distinct selected bots.
pub(super) fn allow_roster_change(team: &TeamSnapshot, next: &Option<CouncilConfig>) -> Result<()> {
    if !selection_changed(team, next) || next.is_none() { return Ok(()); }
    if let Some(config) = next.as_ref().filter(|c| !c.reviewer_bot_ids.is_empty()) {
        let lenses = super::council::fixed_lens_count(team);
        if config.reviewer_bot_ids.len() < lenses {
            return Err(format!("This Council cycle fixed {lenses} lenses. Select at least {lenses} reviewer bots, or finish this cycle before reducing the roster."));
        }
    }
    if team.workers.iter().any(|w| w.created_at >= team.council_since && w.council.is_some() && super::unsettled(&w.status)) {
        return Err("Finish or stop the current Council round before changing selected reviewer bots.".into());
    }
    Ok(())
}

/// Call only after both selection-change and eligibility validation succeeded.
pub(super) fn recover_roster_change(team: &mut TeamSnapshot, next: &Option<CouncilConfig>) {
    if next.is_some() && selection_changed(team, next) {
        super::council::recover_round(team);
    }
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
