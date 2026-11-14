//! Resolve existing persistent bots as immutable Council reviewer identities.
//! Never modify bot profiles, ordinary conversations, memories or permissions.
use super::{council::{CouncilCompositionCapabilities, CouncilConfig, CouncilReviewerCapability, CouncilStrategy, NestedCouncilConfig, SelectedReviewerMode}, TeamSnapshot, WorkerProfile, HARNESSES};
use crate::{bots, model_health, model_routing, providers, storage::Store, Result};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Only a direct bot conversation or explicitly assigned bot task establishes
/// executor identity. Sharing a project does not identify an executing bot.
pub(super) fn executor(store: &Store, session_id: &str) -> Result<Option<String>> {
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

/// Revalidate a persisted reviewer after restart/resume without reading mutable
/// persona or skill assignments into the in-flight seat. Deleting or pausing the
/// selected bot revokes further work; editing the bot leaves frozen inputs intact.
pub(super) fn validate_frozen_seat(
    store: &Store, binding: &super::reviewer_security::ReviewerBinding, check_executable: bool,
) -> Result<()> {
    let Some(bot_id) = binding.reviewer_bot_id.as_deref() else { return Ok(()) };
    let current = bots::get(store, bot_id)
        .map_err(|_| format!("reviewer_unavailable: selected bot {bot_id} was removed."))?;
    if current.paused {
        return Err("reviewer_unavailable: selected reviewer bot is paused.".into());
    }
    let provider = &binding.snapshot.provider;
    let model = &binding.snapshot.model;
    if !HARNESSES.contains(&provider.as_str())
        || model.is_empty()
        || (check_executable && (!model_routing::allowed(provider, model)
            || !model_health::is_healthy(provider, model)
            || providers::executable(provider).is_none()))
    {
        return Err("reviewer_unavailable: frozen reviewer harness/model is no longer available.".into());
    }
    Ok(())
}

pub(super) fn validate_frozen_seat_for_task(
    store: &Store, session_id: &str, binding: &super::reviewer_security::ReviewerBinding, check_executable: bool,
) -> Result<()> {
    if let Some(id) = binding.reviewer_bot_id.as_deref() {
        if executor(store, session_id)?.as_deref() == Some(id) {
            return Err("reviewer_unavailable: a bot cannot review its own task.".into());
        }
    }
    validate_frozen_seat(store, binding, check_executable)
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
            "skills": bot.skills,
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
    let roster = selected_roster(council);
    if roster.is_empty() { return Ok(()); }
    let executor = executor(store, session_id)?;
    let mut used = HashSet::new();
    let mut available_skills = HashSet::new();
    for (id, _, _) in &roster {
        if !used.insert(id) {
            return Err("reviewer_unavailable: repeated reviewer bot ID.".into());
        }
        let bot = bots::get(store, id).map_err(|_| format!("reviewer_unavailable: selected bot {id} was removed."))?;
        valid_bot(&bot, executor.as_deref(), true)?;
        available_skills.extend(bot.skills.iter().cloned());
        if let Some(permissions) = &council.review_permissions {
            review_policy(&bot, permissions)?;
        }
    }
    if council.optional_skill_ids.iter().any(|id| !available_skills.contains(id)) {
        return Err("reviewer_skill_unavailable: optional skill must be assigned to at least one selected reviewer bot.".into());
    }
    if council.required_check_skill_ids.iter().any(|id| !available_skills.contains(id)) {
        return Err("reviewer_skill_unavailable: native check must name an assigned reviewer bot skill.".into());
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

/// A composed automatic seat consumes one planned source slot. Abandoned unscored seats release
/// their slot, while scored seats remain part of the frozen round even if later abandoned.
fn ensure_automatic_slot(team: &TeamSnapshot, round: usize, replaces: Option<&str>, quota: usize) -> Result<()> {
    let used = team.workers.iter()
        .filter(|worker| worker.created_at >= team.council_since)
        .filter(|worker| worker.council.as_ref().is_some_and(|seat| seat.round == round))
        .filter(|worker| !worker.abandoned || worker.council.as_ref().is_some_and(|seat| seat.score.is_some()))
        .filter(|worker| Some(worker.id.as_str()) != replaces)
        .filter(|worker| worker.reviewer_binding.as_ref().and_then(|binding| binding.reviewer_bot_id.as_deref()).is_none())
        .count();
    if used >= quota {
        return Err("The composition's automatic reviewer quota is already filled for this round.".into());
    }
    Ok(())
}

/// All selected profiles are captured together, before the first seat is issued. Store this in a
/// native-only request record, separate from mutable worker snapshots and bot profiles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct SelectedReviewer {
    pub id: String,
    pub roster_bot_ids: Vec<String>,
    /// Composition mode is frozen with the selected bot profile for this round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition_mode: Option<SelectedReviewerMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested_config: Option<NestedCouncilConfig>,
    pub profile: WorkerProfile,
    pub context: super::reviewer_security::ReviewerBotContext,
    pub tool_policy: super::reviewer_security::ReviewerToolPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<crate::skills::reviewer::ReviewerSkills>,
}

/// Extract selected bot identities from the optional composition while leaving
/// legacy CouncilConfig reviewer_bot_ids untouched. CouncilConfig owns schema
/// validation; these adapters only consume already validated frozen values.
fn selected_roster(council: &CouncilConfig) -> Vec<(String, Option<SelectedReviewerMode>, Option<NestedCouncilConfig>)> {
    if let Some(composition) = &council.composition {
        composition.selected.iter().map(|seat| (seat.bot_id.clone(), Some(seat.mode), seat.nested.clone())).collect()
    } else {
        // Legacy M1/M2 rosters predate composition mode snapshots. Keep that
        // absence stable so replacements compare against the frozen round too.
        council.reviewer_bot_ids.iter().map(|id| (id.clone(), None, None)).collect()
    }
}

/// Native dispatch capabilities are the strategies admitted by the runtime gates below.
/// Escalated plans fail closed when no complete local cost ranking exists; two-phase plans
/// require a fresh, correlated challenge round before any terminal pass.
pub(super) fn composition_capabilities(
    store: &Store,
    session_id: &str,
    council: &CouncilConfig,
) -> Result<CouncilCompositionCapabilities> {
    validate_roster(store, session_id, council)?;
    let reviewers = selected_roster(council).into_iter().map(|(bot_id, mode, _)| {
        CouncilReviewerCapability {
            bot_id,
            direct: true,
            nested: mode == Some(SelectedReviewerMode::Nested),
        }
    }).collect();
    Ok(CouncilCompositionCapabilities {
        automatic_reviewers: true,
        supported_strategies: vec![
            CouncilStrategy::Parallel,
            CouncilStrategy::Escalated,
            CouncilStrategy::TwoPhase,
        ],
        reviewers,
    })
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
    let roster = selected_roster(config);
    for (id, mode, nested) in &roster {
        let bot = bots::get(store, id).map_err(|_| format!("reviewer_unavailable: selected bot {id} was removed."))?;
        valid_bot(&bot, executor.as_deref(), check_executable)?;
        let tool_policy = config.review_permissions.as_ref().map(|permissions| review_policy(&bot, permissions)).transpose()?.unwrap_or(super::reviewer_security::ReviewerToolPolicy::NativeReadOnly);
        let skills = if bot.skills.is_empty() { None } else if let Some(library) = library {
            let optional = config.optional_skill_ids.iter()
                .filter(|id| bot.skills.contains(id))
                .cloned().collect::<Vec<_>>();
            let checks = config.required_check_skill_ids.iter()
                .filter(|id| bot.skills.contains(id))
                .cloned().collect::<Vec<_>>();
            let scope = format!("{}:{}:{round}:{}", team.session_id, team.council_since, bot.id);
            Some(crate::skills::reviewer::freeze_for_round(
                library, &bot.skills, &bot.provider, &optional, &checks, &scope,
            )?)
        } else if check_executable {
            return Err("reviewer_skill_unavailable: central Skills Library could not be opened.".into());
        } else { None };
        reviewers.push(SelectedReviewer {
            id: id.clone(), roster_bot_ids: roster.iter().map(|(id, _, _)| id.clone()).collect(),
            composition_mode: *mode, nested_config: nested.clone(), profile: profile(&bot),
            context: super::reviewer_security::ReviewerBotContext::from_bot(&bot), tool_policy, skills,
        });
    }
    super::record_request(store, &team.session_id, &key, "reviewer_round", &serde_json::to_string(&reviewers).map_err(|e| e.to_string())?, "")?;
    Ok(reviewers)
}

/// Resolve a frozen selected reviewer. Live deletion, pause, executor identity and frozen-model
/// availability are still checked, without substituting a newly edited profile or another bot.
pub(super) fn seat_bot(store: &Store, team: &TeamSnapshot, round: usize, replaces: Option<&str>, library: Option<&crate::skills::Library>) -> Result<Option<SelectedReviewer>> {
    seat_bot_with_source(store, team, round, replaces, SeatSourceRequest::legacy(), true, library)
}
pub(super) fn seat_bot_for_assignment(
    store: &Store,
    team: &TeamSnapshot,
    round: usize,
    replaces: Option<&str>,
    source: Option<super::CouncilAssignmentSource>,
    reviewer_bot_id: Option<&str>,
    library: Option<&crate::skills::Library>,
) -> Result<Option<SelectedReviewer>> {
    seat_bot_with_source(store, team, round, replaces, SeatSourceRequest::explicit(source, reviewer_bot_id), true, library)
}
#[cfg(test)]
pub(super) fn seat_bot_inner(store: &Store, team: &TeamSnapshot, round: usize, replaces: Option<&str>, check_executable: bool) -> Result<Option<SelectedReviewer>> {
    seat_bot_with_source(store, team, round, replaces, SeatSourceRequest::legacy(), check_executable, None)
}
#[cfg(test)]
pub(super) fn seat_bot_inner_for_assignment(
    store: &Store,
    team: &TeamSnapshot,
    round: usize,
    replaces: Option<&str>,
    source: Option<super::CouncilAssignmentSource>,
    reviewer_bot_id: Option<&str>,
    check_executable: bool,
) -> Result<Option<SelectedReviewer>> {
    seat_bot_with_source(store, team, round, replaces, SeatSourceRequest::explicit(source, reviewer_bot_id), check_executable, None)
}
#[cfg(test)]
pub(super) fn seat_bot_inner_with_skills(store: &Store, team: &TeamSnapshot, round: usize, replaces: Option<&str>, library: &crate::skills::Library) -> Result<Option<SelectedReviewer>> {
    seat_bot_with_source(store, team, round, replaces, SeatSourceRequest::legacy(), false, Some(library))
}

#[derive(Clone, Copy)]
struct SeatSourceRequest<'a> {
    source: Option<super::CouncilAssignmentSource>,
    reviewer_bot_id: Option<&'a str>,
    required: bool,
}

impl<'a> SeatSourceRequest<'a> {
    fn legacy() -> Self {
        Self { source: None, reviewer_bot_id: None, required: false }
    }
    fn explicit(source: Option<super::CouncilAssignmentSource>, reviewer_bot_id: Option<&'a str>) -> Self {
        Self { source, reviewer_bot_id, required: true }
    }
}

fn seat_bot_with_source(
    store: &Store,
    team: &TeamSnapshot,
    round: usize,
    replaces: Option<&str>,
    request: SeatSourceRequest<'_>,
    check_executable: bool,
    library: Option<&crate::skills::Library>,
) -> Result<Option<SelectedReviewer>> {
    let source = request.source;
    let requested_bot_id = request.reviewer_bot_id;
    let Some(config) = team.config.council.as_ref() else { return Ok(None) };
    let composition = config.composition.is_some();
    let plan = config.seat_plan();
    let roster = selected_roster(config);
    let old = replaces.and_then(|id| team.workers.iter().find(|w| w.id == id));
    let inherited = old.and_then(|w| w.reviewer_binding.as_ref()).and_then(|b| b.reviewer_bot_id.as_deref());

    if request.required && plan.automatic > 0 && plan.selected_bots() > 0 && source.is_none() {
        return Err("council_source_required: every mixed Council seat must explicitly name its source.".into());
    }
    if source.is_none() && requested_bot_id.is_some() {
        return Err("council_source_invalid: reviewerBotId requires councilSource=selected_bot.".into());
    }
    if source == Some(super::CouncilAssignmentSource::Automatic) {
        if !composition || plan.automatic == 0 || requested_bot_id.is_some() || inherited.is_some() {
            return Err("council_source_invalid: this assignment cannot claim an automatic Council seat.".into());
        }
        ensure_automatic_slot(team, round, replaces, plan.automatic)?;
        return Ok(None);
    }
    if source == Some(super::CouncilAssignmentSource::SelectedBot) {
        let requested = requested_bot_id.ok_or("council_source_invalid: selected_bot seats require reviewerBotId.")?;
        if !roster.iter().any(|(id, _, _)| id == requested) {
            return Err("council_source_invalid: reviewerBotId is not selected in this frozen Council composition.".into());
        }
        if inherited.is_some_and(|id| id != requested) {
            return Err("reviewer_session_mismatch: a selected bot replacement must retain its original bot identity.".into());
        }
        if old.is_some_and(|worker| worker.council.is_some()) && inherited.is_none() {
            return Err("reviewer_session_mismatch: a selected bot cannot replace an automatic reviewer.".into());
        }
    }

    if roster.is_empty() {
        if !composition { return Ok(None); }
        if inherited.is_some() {
            return Err("reviewer_session_mismatch: replaced selected bot is absent from the frozen composition.".into());
        }
        if plan.automatic > 0 {
            ensure_automatic_slot(team, round, replaces, plan.automatic)?;
        }
        return Ok(None);
    }

    if old.is_some_and(|worker| worker.council.is_some()) && inherited.is_none() {
        if composition && plan.automatic > 0 {
            ensure_automatic_slot(team, round, replaces, plan.automatic)?;
            return Ok(None);
        }
        return Err("reviewer_session_mismatch: a selected seat cannot replace a native reviewer.".into());
    }

    let roster_ids = roster.iter().map(|(id, _, _)| id.clone()).collect::<Vec<_>>();
    crate::bots::shared_channels::validate_delivery_reviewers(store, &team.session_id, &roster_ids)?;
    let reviewers = round_reviewers(store, team, round, check_executable, library)?;
    let occupied = occupied_bot_ids(team, round, replaces);
    let selected = match inherited {
        Some(id) => id,
        None if source == Some(super::CouncilAssignmentSource::SelectedBot) => {
            let requested = requested_bot_id.ok_or("council_source_invalid: selected_bot seats require reviewerBotId.")?;
            if occupied.contains(requested) {
                return Err("reviewer_session_mismatch: reviewer identity is duplicated or no longer selected.".into());
            }
            requested
        }
        None => match reviewers
            .iter()
            .cycle()
            .skip(round.saturating_sub(1) % reviewers.len().max(1))
            .take(reviewers.len())
            .find(|bot| !occupied.contains(bot.id.as_str()))
        {
            Some(bot) => bot.id.as_str(),
            None if composition && plan.automatic > 0 => {
                ensure_automatic_slot(team, round, replaces, plan.automatic)?;
                return Ok(None);
            }
            None => return Err("All selected reviewer bots already have a seat in this round.".into()),
        },
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
            || binding.snapshot.tool_policy != frozen.tool_policy
            || (!binding.roster_bot_ids.is_empty() && binding.roster_bot_ids != frozen.roster_bot_ids)
            || binding.composition_mode.as_deref() != frozen.composition_mode.map(|mode| match mode {
                SelectedReviewerMode::Direct => "direct",
                SelectedReviewerMode::Nested => "nested",
            }))
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
        || old.map(|c| &c.optional_skill_ids) != next.map(|c| &c.optional_skill_ids)
        || old.map(|c| &c.required_check_skill_ids) != next.map(|c| &c.required_check_skill_ids)
        || old.map(|c| &c.composition) != next.map(|c| &c.composition)
        || old.and_then(|c| serde_json::to_value(c).ok()).and_then(|v| v.get("composition").cloned())
            != next.as_ref().and_then(|c| serde_json::to_value(c).ok()).and_then(|v| v.get("composition").cloned())
}

/// Live rounds must settle first. A user change can explicitly recover a stopped/incomplete
/// round, while fixed native lenses must remain satisfiable by distinct selected bots.
pub(super) fn allow_roster_change(team: &TeamSnapshot, next: &Option<CouncilConfig>) -> Result<()> {
    if !selection_changed(team, next) || next.is_none() { return Ok(()); }
    if let Some(config) = next.as_ref().filter(|c| c.composition.is_some() || !selected_roster(c).is_empty()) {
        let lenses = super::council::fixed_lens_count(team);
        if config.seat_plan().required() < lenses {
            let sources = if config.composition.is_some() { "configured reviewer seats" } else { "reviewer bots" };
            return Err(format!("This Council cycle fixed {lenses} lenses. Select at least {lenses} {sources}, or finish this cycle before reducing the roster."));
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
