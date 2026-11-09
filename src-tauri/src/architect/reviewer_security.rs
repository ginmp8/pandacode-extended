//! Task-scoped Council reviewer identity and native capability enforcement.
//! S2/S4 will supply user-authorized grants; S5 owns post-acceptance effects.
use super::{council::CouncilSeat, TeamSnapshot, Worker};
use crate::{providers::ToolHost, Result};
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
pub(crate) struct ReviewerCapabilityGrant {
    pub tool: String,
    /// "call" for an atomic tool; otherwise require a matching action argument.
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_argument: Option<String>,
    pub resource_argument: String,
    pub resource: String,
}

impl ReviewerCapabilityGrant {
    fn validate(&self) -> Result<()> {
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
        let grants = policy.grants.clone();
        let original = host.handler;
        let handler = Arc::new(move |name: String, args: Value| {
            let allowed = grants.iter().any(|grant| grant.permits(&name, &args));
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

impl ReviewerCapabilityGrant {
    fn clone_key(&self) -> String {
        format!("{}:{}:{}:{}", self.tool, self.action, self.resource_argument, self.resource)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerSnapshot {
    pub provider: String,
    pub model: String,
    pub lens: String,
    pub tool_policy: ReviewerToolPolicy,
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
    /// Native identity/revision token, not evidence of a content checksum.
    pub artifact_revision: String,
    pub snapshot: ReviewerSnapshot,
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
            artifact_revision: format!("{}:{}:{}:{}", team.council_since, team.dispatch_revision, created_at, worker_id),
            snapshot: ReviewerSnapshot {
                provider: provider.to_owned(),
                model: model.to_owned(),
                lens: seat.lens.clone(),
                tool_policy: ReviewerToolPolicy::NativeReadOnly,
            },
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
            // S2/S4 must bind selected bot/delivery to an authoritative parent record.
            || self.delivery_id.is_some()
            || self.reviewer_bot_id.is_some()
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
    #[test]
    fn scoped_policy_requires_explicit_user_and_configured_bot_grants() {
        let grant = issue_grant();
        assert!(ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "", &[grant.clone()], &[grant.clone()]).is_err());
        assert!(ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "user-1", &[], &[grant.clone()]).is_err());
        assert!(ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "user-1", &[grant.clone()], &[grant.clone(),grant.clone()]).is_err());
        assert!(ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "user-1", &[grant.clone()], &[grant]).is_ok());
    }
    #[tokio::test]
    async fn scoped_host_checks_tool_action_and_exact_resource_at_dispatch() {
        let grant = issue_grant();
        let policy = ReviewerToolPolicy::from_user_grants(ReviewerPhase::Review, "user-1", &[grant.clone()], &[grant]).unwrap();
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
        let other_phase = ReviewerToolPolicy::from_user_grants(ReviewerPhase::PostAcceptance, "user-1",
            &[issue_grant()], &[issue_grant()]).unwrap();
        assert!(other_phase.restrict_host(None).is_err());
    }
}
