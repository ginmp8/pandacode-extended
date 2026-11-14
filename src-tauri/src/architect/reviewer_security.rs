//! Task-scoped Council reviewer identity and native capability enforcement.
//! S2/S4 will supply user-authorized grants; S5 owns post-acceptance effects.
use super::{council::CouncilSeat, TeamSnapshot, Worker};
use crate::{providers::ToolHost, storage::Store, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(crate) enum ReviewerPhase {
    Review,
    PostAcceptance,
}

/// Exact host-tool grant, not an arbitrary MCP/shell privilege.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewerCapabilityGrant {
    pub tool: String,
    /// "call" for an atomic tool; otherwise require a matching action argument.
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_argument: Option<String>,
    pub resource_argument: String,
    pub resource: String,
}

impl ReviewerCapabilityGrant {
    pub(crate) fn validate(&self) -> Result<()> {
        let name = |s: &str| !s.is_empty() && s.len() <= 64
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if !name(&self.tool) || !name(&self.resource_argument)
            || self.action_argument.as_deref().is_some_and(|a| !name(a))
            || self.action.is_empty() || self.action.len() > 64
            || !self.action.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
            || self.resource.is_empty() || self.resource.len() > 256
            || self.resource.chars().any(char::is_control)
            || (self.action_argument.is_none() && self.action != "call")
            || (self.action_argument.is_some() && self.action == "call")
        {
            return Err("capability_not_authorized: invalid exact tool/action/resource grant.".into());
        }
        Ok(())
    }

    fn permits(&self, tool: &str, arguments: &Value) -> bool {
        self.tool == tool
            && arguments.get(self.resource_argument.as_str()).and_then(Value::as_str) == Some(&self.resource)
            && self.action_argument.as_ref().is_none_or(|key| {
                arguments.get(key.as_str()).and_then(Value::as_str) == Some(&self.action)
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerScopedPolicy {
    pub phase: ReviewerPhase,
    /// Recorded user action; a model-written prompt cannot create this record.
    pub authorization_id: String,
    pub grants: Vec<ReviewerCapabilityGrant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewerToolPolicy {
    /// Backward-compatible default, including pre-S1 native reviewer seats.
    NativeReadOnly,
    /// An explicitly authorized, bounded host-tool policy, not ambient MCP access.
    Scoped(ReviewerScopedPolicy),
}

impl ReviewerToolPolicy {
    #[allow(dead_code)] // S2/S4 will populate grants from trusted user task settings.
    pub(crate) fn from_user_grants(
        phase: ReviewerPhase,
        authorization_id: &str,
        configured: &[ReviewerCapabilityGrant],
        requested: &[ReviewerCapabilityGrant],
    ) -> Result<Self> {
        if authorization_id.trim().is_empty() || authorization_id.len() > 128
            || authorization_id.chars().any(char::is_control) || requested.is_empty()
            || requested.len() > 16
        {
            return Err("capability_not_authorized: a bounded user authorization is required.".into());
        }
        let mut seen = HashSet::new();
        for grant in requested {
            grant.validate()?;
            if !configured.contains(grant) || !seen.insert(grant.clone_key()) {
                return Err("capability_not_authorized: grant is absent from the bot capabilities or duplicated.".into());
            }
        }
        let policy = Self::Scoped(ReviewerScopedPolicy {
            phase,
            authorization_id: authorization_id.into(),
            grants: requested.to_vec(),
        });
        policy.validate()?;
        Ok(policy)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let Self::Scoped(policy) = self else { return Ok(()) };
        if policy.authorization_id.trim().is_empty()
            || policy.authorization_id.len() > 128
            || policy.authorization_id.chars().any(char::is_control)
            || policy.grants.is_empty() || policy.grants.len() > 16
        {
            return Err("capability_not_authorized: invalid reviewer authorization.".into());
        }
        let mut seen = HashSet::new();
        for grant in &policy.grants {
            grant.validate()?;
            if !seen.insert(grant.clone_key()) {
                return Err("capability_not_authorized: repeated capability grant.".into());
            }
        }
        Ok(())
    }

    /// Prevent registering an unguarded app tool even if an older harness requests it.
    /// The wrapped handler independently checks every call, including MCP transports.
    pub(crate) fn restrict_host(&self, host: Option<ToolHost>) -> Result<Option<ToolHost>> {
        self.validate()?;
        let Self::Scoped(policy) = self else { return Ok(None) };
        if policy.phase != ReviewerPhase::Review {
            return Err("policy_unsupported: post-acceptance tools cannot run as Council reviewers.".into());
        }
        let host = host.ok_or("policy_unsupported: requested native tools are unavailable.")?;
        let mut tools = Vec::new();
        let mut seen = HashSet::new();
        for grant in &policy.grants {
            // Some host operations always require fresh confirmation. A task grant
            // may not silently override this stronger app-level policy.
            if crate::cloud::deploy::ALWAYS_ASK.contains(&grant.tool.as_str()) {
                return Err("policy_unsupported: this native tool always requires separate approval.".into());
            }
            let spec = host.tools.iter().find(|spec| {
                spec["type"] == "function" && spec["name"] == grant.tool
            }).ok_or("policy_unsupported: an authorized native tool is unavailable.")?;
            let properties = &spec["inputSchema"]["properties"];
            if properties[grant.resource_argument.as_str()]["type"] != "string"
                || grant.action_argument.as_ref().is_some_and(|key| properties[key.as_str()]["type"] != "string"
                    && !properties[key]["enum"].is_array())
                || (grant.action_argument.is_none() && !properties["action"].is_null())
            {
                return Err("policy_unsupported: tool cannot enforce exact action and resource arguments.".into());
            }
            if seen.insert(grant.tool.clone()) {
                tools.push(spec.clone());
            }
        }
        // Only these exact PandaCode-defined, immutable read-only skill tools
        // may accompany scoped user grants. A skill cannot introduce tool names.
        let mut native_skill_tools = HashSet::new();
        for spec in super::reviewer_skill_evidence::tools() {
            if host.tools.iter().any(|candidate| candidate == &spec) {
                let name = spec["name"].as_str().unwrap_or_default().to_string();
                if seen.insert(name.clone()) {
                    tools.push(spec);
                    native_skill_tools.insert(name);
                }
            }
        }
        let grants = policy.grants.clone();
        let original = host.handler;
        let handler = Arc::new(move |name: String, args: Value| {
            let allowed = native_skill_tools.contains(&name) || grants.iter().any(|grant| grant.permits(&name, &args));
            let original = original.clone();
            let future: crate::providers::ToolFuture = Box::pin(async move {
                if !allowed {
                    return Err("capability_not_authorized: Council tool action or target is outside the user grant.".into());
                }
                (original)(name, args).await
            });
            future
        });
        Ok(Some(ToolHost { tools, handler, approval_tools: HashSet::new() }))
    }
}

/// User's task-scoped choice. The native adapter must independently resolve the
/// intersection with a bot's actual configured host tools before issuing a seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewerPermissionSelection {
    pub authorization_id: String,
    pub grants: Vec<ReviewerCapabilityGrant>,
}

impl ReviewerCapabilityGrant {
    fn clone_key(&self) -> String {
        format!("{}:{}:{}:{}", self.tool, self.action, self.resource_argument, self.resource)
    }
}

/// Authenticated native-issued identity, independent of the mutable worker snapshot.
/// User-scope authorization (S2/S4) must update this via trusted native code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewerAuthority {
    council_cycle_id: String,
    seat_id: String,
    artifact_revision: String,
    tool_policy: ReviewerToolPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reviewer_bot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bot_context: Option<ReviewerBotContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skills: Option<crate::skills::reviewer::ReviewerSkills>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    roster_bot_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    required_reviewers: Option<usize>,
}
fn authority_key(seat: &str) -> String { format!("native-reviewer-authority:{seat}") }
pub(crate) fn record_native_authority(store: &Store, parent: &str, binding: &ReviewerBinding) -> Result<()> {
    let record = ReviewerAuthority {
        council_cycle_id: binding.council_cycle_id.clone(),
        seat_id: binding.seat_id.clone(),
        artifact_revision: binding.artifact_revision.clone(),
        tool_policy: binding.snapshot.tool_policy.clone(),
        reviewer_bot_id: binding.reviewer_bot_id.clone(),
        bot_context: binding.snapshot.bot_context.clone(),
        skills: binding.snapshot.skills.clone(),
        roster_bot_ids: binding.roster_bot_ids.clone(),
        required_reviewers: binding.required_reviewers,
    };
    let body = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    store.db.execute(
        "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id) VALUES(?1,?2,'reviewer_authority',?3,?4)",
        params![parent, authority_key(&binding.seat_id), body, binding.seat_id],
    ).map_err(|e| format!("Cannot record reviewer authority: {e}"))?;
    Ok(())
}
pub(crate) fn validate_issued_authority(store: &Store, parent: &str, binding: &ReviewerBinding) -> Result<()> {
    let row: Option<(String, String, String)> = store.db.query_row(
        "SELECT kind,content,worker_id FROM architect_requests WHERE parent_session_id=?1 AND key=?2",
        params![parent, authority_key(&binding.seat_id)],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).optional().map_err(|e| e.to_string())?;
    match row {
        Some((kind, raw, worker_id)) if kind == "reviewer_authority" && worker_id == binding.seat_id => {
            let original: ReviewerAuthority = serde_json::from_str(&raw)
                .map_err(|_| "reviewer_session_mismatch: invalid native authorization record.".to_string())?;
            if original.council_cycle_id != binding.council_cycle_id
                || original.seat_id != binding.seat_id
                || original.artifact_revision != binding.artifact_revision
                || original.tool_policy != binding.snapshot.tool_policy
                || original.reviewer_bot_id != binding.reviewer_bot_id
                || original.bot_context != binding.snapshot.bot_context
                || original.skills != binding.snapshot.skills
                || original.roster_bot_ids != binding.roster_bot_ids
                || original.required_reviewers != binding.required_reviewers
            {
                return Err("reviewer_session_mismatch: saved review revision or grant differs from native authorization.".into());
            }
            Ok(())
        }
        // Backward-compatible S1/native seats issued before authority records;
        // they can never acquire a scoped grant through this fallback.
        None if binding.snapshot.tool_policy == ReviewerToolPolicy::NativeReadOnly
            && binding.reviewer_bot_id.is_none() && binding.snapshot.bot_context.is_none() => Ok(()),
        _ => Err("capability_not_authorized: no matching native reviewer authorization.".into()),
    }
}

/// New composed Councils require a recorded native grant even for read-only
/// automatic reviewers. The legacy automatic-reviewer fallback in
/// validate_issued_authority is deliberately not used for this stricter gate.
pub(crate) fn validate_recorded_authority(
    store: &Store, parent: &str, binding: &ReviewerBinding,
) -> Result<()> {
    let recorded: i64 = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM architect_requests
         WHERE parent_session_id=?1 AND key=?2 AND kind='reviewer_authority' AND worker_id=?3)",
        params![parent, authority_key(&binding.seat_id), binding.seat_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if recorded != 1 {
        return Err("capability_not_authorized: a composed reviewer has no recorded native authority.".into());
    }
    validate_issued_authority(store, parent, binding)
}

/// Called by the future S2/S4 trusted user-action adapter; never from a model
/// prompt. Both the user's requested grants and bot-configured grants must match.
#[allow(dead_code)]
pub(crate) fn authorize_user_grants(
    store: &Store,
    parent: &str,
    binding: &ReviewerBinding,
    phase: ReviewerPhase,
    authorization_id: &str,
    configured: &[ReviewerCapabilityGrant],
    requested: &[ReviewerCapabilityGrant],
) -> Result<ReviewerToolPolicy> {
    validate_issued_authority(store, parent, binding)?;
    let policy = ReviewerToolPolicy::from_user_grants(phase, authorization_id, configured, requested)?;
    let old = ReviewerAuthority {
        council_cycle_id: binding.council_cycle_id.clone(),
        seat_id: binding.seat_id.clone(),
        artifact_revision: binding.artifact_revision.clone(),
        tool_policy: binding.snapshot.tool_policy.clone(),
        reviewer_bot_id: binding.reviewer_bot_id.clone(),
        bot_context: binding.snapshot.bot_context.clone(),
        skills: binding.snapshot.skills.clone(),
        roster_bot_ids: binding.roster_bot_ids.clone(),
        required_reviewers: binding.required_reviewers,
    };
    let mut new = old.clone();
    new.tool_policy = policy.clone();
    let old = serde_json::to_string(&old).map_err(|e| e.to_string())?;
    let next = serde_json::to_string(&new).map_err(|e| e.to_string())?;
    let count = store.db.execute(
        "UPDATE architect_requests SET content=?1 WHERE parent_session_id=?2 AND key=?3 AND kind='reviewer_authority' AND content=?4",
        params![next, parent, authority_key(&binding.seat_id), old],
    ).map_err(|e| e.to_string())?;
    if count != 1 {
        return Err("capability_not_authorized: reviewer authority was changed or is absent.".into());
    }
    Ok(policy)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerSnapshot {
    pub provider: String,
    pub model: String,
    pub lens: String,
    pub tool_policy: ReviewerToolPolicy,
    /// Frozen user-configured persona for S2; never a live bot profile or a permission grant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_context: Option<ReviewerBotContext>,
    /// Immutable reviewer-assigned library inputs, never the bot's global skills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<crate::skills::reviewer::ReviewerSkills>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerBotContext {
    pub name: String,
    pub role: String,
    pub instructions: String,
    pub goals: String,
    pub soul: String,
    pub reasoning_effort: String,
    /// Assigned skill references only. S3 owns pinning/materialization/verification.
    #[serde(default)]
    pub skill_ids: Vec<String>,
}

impl ReviewerBotContext {
    pub fn from_bot(bot: &crate::bots::Bot) -> Self {
        Self {
            name: bot.name.clone(), role: bot.role.clone(), instructions: bot.instructions.clone(),
            goals: bot.goals.clone(), soul: bot.soul.clone(), reasoning_effort: bot.reasoning_effort.clone(),
            skill_ids: bot.skills.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerBinding {
    pub council_cycle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_id: Option<String>,
    pub round: usize,
    pub seat_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer_bot_id: Option<String>,
    /// Frozen seat roster; original council round history is independent of future selection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roster_bot_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_reviewers: Option<usize>,
    /// Native identity/revision token, not evidence of a content checksum.
    pub artifact_revision: String,
    pub snapshot: ReviewerSnapshot,
    /// Native-observed skill events are separate from immutable reviewer authority.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub skill_evidence: std::collections::BTreeMap<String, super::reviewer_skill_evidence::ObservedSkillEvidence>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewerState {
    Queued,
    Running,
    Completed,
    Blocked,
    Failed,
    Inconclusive,
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewerFailureCode {
    ReadOnlyUnsupported,
    ReadOnlyPolicyViolation,
    CapabilityNotAuthorized,
    PolicyUnsupported,
    ReviewerUnavailable,
    ReviewerSessionMismatch,
    ReviewRevisionStale,
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewerVerdict {
    Pending,
    Passed,
    NeedsCorrection,
    Blocked,
    Failed,
    Inconclusive,
    AcceptedByUser,
}
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub(crate) enum ReviewerEvent {
    Queued { seat_id: String, artifact_revision: String },
    Started { seat_id: String },
    Blocked { seat_id: String, code: ReviewerFailureCode },
    Reported { seat_id: String, verdict: ReviewerVerdict },
}

impl ReviewerBinding {
    pub fn native(
        team: &TeamSnapshot,
        worker_id: &str,
        created_at: i64,
        provider: &str,
        model: &str,
        seat: &CouncilSeat,
    ) -> Self {
        Self {
            council_cycle_id: format!("{}:{}", team.session_id, team.council_since),
            delivery_id: None,
            round: seat.round,
            seat_id: worker_id.to_owned(),
            reviewer_bot_id: None,
            roster_bot_ids: Vec::new(),
            // Automatic Council keeps its editable count. Only selected bot rounds pin quorum.
            required_reviewers: team.config.council.as_ref().filter(|c| !c.reviewer_bot_ids.is_empty()).map(|c| c.needed()),
            artifact_revision: format!("{}:{}:{}:{}", team.council_since, team.dispatch_revision, created_at, worker_id),
            snapshot: ReviewerSnapshot {
                provider: provider.to_owned(),
                model: model.to_owned(),
                lens: seat.lens.clone(),
                tool_policy: ReviewerToolPolicy::NativeReadOnly,
                bot_context: None,
                skills: None,
            },
            skill_evidence: Default::default(),
        }
    }

    pub fn validate(&self, team: &TeamSnapshot, worker: &Worker) -> Result<()> {
        let seat = worker.council.as_ref().ok_or("reviewer_session_mismatch: not a Council seat.")?;
        let revision = self.artifact_revision.split(':').collect::<Vec<_>>();
        let frozen_revision_matches = revision.len() == 4
            && revision[0] == team.council_since.to_string()
            && revision[1].parse::<u64>().is_ok_and(|r| r <= team.dispatch_revision)
            && revision[2] == worker.created_at.to_string()
            && revision[3] == worker.id.as_str();
        if self.council_cycle_id != format!("{}:{}", team.session_id, team.council_since)
            || self.seat_id != worker.id
            || self.round != seat.round
            || self.snapshot.lens != seat.lens
            || self.snapshot.provider != worker.provider
            || self.snapshot.model != worker.model
            || !frozen_revision_matches
            // S4 delivery identity remains unbound; S2 binds a real bot to a frozen persona.
            || self.delivery_id.is_some()
            || self.reviewer_bot_id.is_some() != self.snapshot.bot_context.is_some()
            || self.reviewer_bot_id.as_deref().is_some_and(|id| !super::valid_id(id))
            || self.required_reviewers.is_some_and(|n| !(2..=8).contains(&n))
            || (!self.roster_bot_ids.is_empty() && (self.required_reviewers != Some(
                team.config.council.as_ref()
                    .filter(|config| config.automatic_reviewers > 0)
                    .map_or(self.roster_bot_ids.len(), |config| config.needed()))
                || self.reviewer_bot_id.as_ref().is_none_or(|id| !self.roster_bot_ids.contains(id))))
        {
            return Err("reviewer_session_mismatch: immutable reviewer binding differs from scheduled worker.".into());
        }
        self.snapshot.tool_policy.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn issue_grant() -> ReviewerCapabilityGrant {
        ReviewerCapabilityGrant {
            tool: "pandacode_github_issue".into(),
            action: "create".into(),
            action_argument: Some("action".into()),
            resource_argument: "repository".into(),
            resource: "owner/repo".into(),
        }
    }
    #[tokio::test]
    async fn scoped_reviewer_keeps_only_preapproved_grants_and_native_skill_reads() {
        let grant = issue_grant();
        let policy = ReviewerToolPolicy::from_user_grants(
            ReviewerPhase::Review, "owner", std::slice::from_ref(&grant), std::slice::from_ref(&grant),
        ).unwrap();
        let mut catalog = vec![json!({"type":"function","name":"pandacode_github_issue",
            "inputSchema":{"type":"object","properties":{"action":{"type":"string"},"repository":{"type":"string"}}}})];
        catalog.extend(super::super::reviewer_skill_evidence::tools());
        let original = ToolHost {
            tools: catalog,
            approval_tools: HashSet::new(),
            handler: Arc::new(|_, _| Box::pin(async { Ok(json!({"executed": true})) })),
        };
        let filtered = policy.restrict_host(Some(original)).unwrap().unwrap();
        assert_eq!(filtered.tools.len(), 3);
        assert!((filtered.handler)("pandacode_reviewer_skill_open".into(), json!({"skillId":"assigned"})).await.is_ok());
        assert!((filtered.handler)("pandacode_reviewer_skill_verify".into(), json!({"skillId":"assigned"})).await.is_ok());
        assert!((filtered.handler)("pandacode_unknown_tool".into(), json!({})).await.is_err());
        assert!((filtered.handler)("pandacode_github_issue".into(), json!({"action":"create","repository":"other/repo"})).await.is_err());
        assert!((filtered.handler)("pandacode_github_issue".into(), json!({"action":"create","repository":"owner/repo"})).await.is_ok());
    }

    #[test]
    fn scoped_policy_requires_explicit_user_and_configured_bot_grants() {
        let grant = issue_grant();
        assert!(ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "", std::slice::from_ref(&grant), std::slice::from_ref(&grant)).is_err());
        assert!(ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "user-1", &[], std::slice::from_ref(&grant)).is_err());
        assert!(ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "user-1", std::slice::from_ref(&grant), &[grant.clone(), grant.clone()]).is_err());
        assert!(ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "user-1", std::slice::from_ref(&grant), std::slice::from_ref(&grant)).is_ok());
    }
    #[tokio::test]
    async fn scoped_host_checks_tool_action_and_exact_resource_at_dispatch() {
        let grant = issue_grant();
        let policy = ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "user-1", std::slice::from_ref(&grant), std::slice::from_ref(&grant)).unwrap();
        let executed = Arc::new(AtomicUsize::new(0));
        let count = executed.clone();
        let host = ToolHost {
            tools: vec![json!({"type":"function","name":"pandacode_github_issue","description":"Create an issue",
                "inputSchema":{"type":"object","properties":{"action":{"type":"string"},"repository":{"type":"string"}}}})],
            approval_tools: HashSet::new(),
            handler: Arc::new(move |_, _| {
                let count = count.clone();
                Box::pin(async move { count.fetch_add(1, Ordering::SeqCst); Ok(json!({"created":true})) })
            }),
        };
        let host = policy.restrict_host(Some(host)).unwrap().unwrap();
        assert_eq!(host.tools.len(), 1);
        for args in [
            json!({"action":"delete","repository":"owner/repo"}),
            json!({"action":"create","repository":"someone/else"}),
            json!({"action":"create"}),
        ] {
            assert!((host.handler)("pandacode_github_issue".into(), args).await.is_err());
        }
        assert!((host.handler)("pandacode_other".into(), json!({"action":"create","repository":"owner/repo"})).await.is_err());
        assert_eq!(executed.load(Ordering::SeqCst), 0);
        assert!((host.handler)("pandacode_github_issue".into(), json!({"action":"create","repository":"owner/repo"})).await.is_ok());
        assert_eq!(executed.load(Ordering::SeqCst), 1);
        // Even a preauthorized tool cannot bypass the app's ALWAYS_ASK policy.
        let mut always = issue_grant();
        always.tool = "pandacode_cloud_cli".into();
        let special = ReviewerToolPolicy::from_user_grants(
            ReviewerPhase::Review, "user-2", std::slice::from_ref(&always), std::slice::from_ref(&always),
        ).unwrap();
        let special_host = ToolHost {
            tools: vec![json!({"type":"function","name":"pandacode_cloud_cli","description":"Protected operation",
                "inputSchema":{"type":"object","properties":{"action":{"type":"string"},"repository":{"type":"string"}}}})],
            approval_tools: ["pandacode_cloud_cli".to_string()].into(),
            handler: Arc::new(|_, _| Box::pin(async { Ok(json!({})) })),
        };
        assert!(special.restrict_host(Some(special_host)).is_err());
        let other_phase = ReviewerToolPolicy::from_user_grants(ReviewerPhase::PostAcceptance, "user-1",
            &[issue_grant()], &[issue_grant()]).unwrap();
        assert!(other_phase.restrict_host(None).is_err());
    }
}
