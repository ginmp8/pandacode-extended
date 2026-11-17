//! Native reviewer skill events are recorded by the host, not inferred from model reports.
use super::*;
use sha2::{Digest, Sha256};
const INTEGRITY: &str = "pinned_input_integrity";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NativeCriterionEvidenceReceipt {
    pub schema_version: u32,
    pub cycle_id: String,
    pub criterion_id: String,
    pub criterion_lens: String,
    pub round: usize,
    pub worker_id: String,
    pub reviewer_session_id: String,
    /// Legacy receipts remain readable but must never authorize a different
    /// task objective, mandatory criterion policy or frozen Council policy.
    #[serde(default)]
    pub objective_sha256: String,
    #[serde(default)]
    pub criterion_policy_sha256: String,
    #[serde(default)]
    pub council_policy_sha256: String,
    pub artifact_revision: String,
    pub content_sha256: String,
    pub check_kind: String,
}
impl NativeCriterionEvidenceReceipt {
    /// Only the native pinned-input check may issue this narrow receipt.
    /// It does not attest arbitrary project tests or correctness criteria.
    pub(super) fn from_host_check(
        team: &TeamSnapshot, worker: &Worker, criterion: &council::CouncilCriterion,
        skill_id: &str, content_sha256: &str,
    ) -> Option<Self> {
        let binding = worker.reviewer_binding.as_ref()?;
        let seat = worker.council.as_ref()?;
        let check = criterion.native_evidence.as_ref()?;
        if !worker.read_only || worker.abandoned || worker.wrote_at != 0
            || !worker.files.is_empty() || !criterion.required || !criterion.evidence_required
            || check.kind != INTEGRITY || check.skill_id != skill_id
            || criterion.lens.trim().to_lowercase() != format!("Pinned input integrity: {skill_id}").to_lowercase()
            || seat.lens.trim().to_lowercase() != criterion.lens.trim().to_lowercase()
            || binding.validate(team, worker).is_err()
        {
            return None;
        }
        Some(Self {
            schema_version: 3,
            cycle_id: binding.council_cycle_id.clone(),
            criterion_id: criterion.id.clone(),
            criterion_lens: criterion.lens.clone(),
            round: seat.round,
            worker_id: worker.id.clone(),
            reviewer_session_id: worker.session_id.clone(),
            objective_sha256: format!("{:x}", Sha256::digest(team.objective.as_bytes())),
            criterion_policy_sha256: format!("{:x}", Sha256::digest(
                serde_json::to_vec(criterion).ok()?,
            )),
            council_policy_sha256: format!("{:x}", Sha256::digest(
                serde_json::to_vec(team.config.council.as_ref()?).ok()?,
            )),
            artifact_revision: binding.artifact_revision.clone(),
            content_sha256: content_sha256.to_owned(),
            check_kind: INTEGRITY.into(),
        })
    }

    pub(super) fn valid_for(
        &self, team: &TeamSnapshot, worker: &Worker, criterion: &council::CouncilCriterion,
        skill_id: &str, content_sha256: &str,
    ) -> bool {
        Self::from_host_check(team, worker, criterion, skill_id, content_sha256)
            .as_ref() == Some(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ObservedSkillEvidence {
    pub content_sha256: String,
    pub artifact_revision: String,
    pub invoked: bool,
    pub check_executed: bool,
    pub evidence_verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criterion_receipt: Option<NativeCriterionEvidenceReceipt>,
}
impl ObservedSkillEvidence {
    pub fn valid_for(&self, pin: &crate::skills::reviewer::SkillPin, revision: &str) -> bool {
        self.content_sha256 == pin.content_sha256 && self.artifact_revision == revision
            && self.invoked && self.check_executed && self.evidence_verified
            && self.check_kind.as_deref() == Some(INTEGRITY)
    }
}
/// A NativeReadOnly reviewer gets no general-purpose tools. Even an arbitrary
/// handler call with a hidden name must be denied after catalog filtering.
pub(super) fn readonly_host(host: ToolHost) -> Option<ToolHost> {
    let allowed_specs = tools();
    let exposed = host.tools.into_iter().filter(|spec| allowed_specs.contains(spec)).collect::<Vec<_>>();
    if exposed.is_empty() { return None; }
    let allowed = exposed.iter().filter_map(|spec| spec["name"].as_str().map(str::to_owned))
        .collect::<HashSet<_>>();
    let original = host.handler;
    let handler = Arc::new(move |name: String, args: Value| {
        if !allowed.contains(&name) {
            let denied: crate::providers::ToolFuture = Box::pin(async {
                Err("capability_not_authorized: this reviewer can use only native pinned-skill read-only tools.".into())
            });
            return denied;
        }
        (original)(name, args)
    });
    Some(ToolHost { tools: exposed, handler, approval_tools: HashSet::new() })
}

pub(super) fn tools() -> Vec<Value> {
    let properties = json!({"skillId":{"type":"string","description":"Exact assigned skill ID on this reviewer seat."}});
    vec![
        tool("reviewer_skill_open", "Retrieve the frozen skill receipt and record actual native retrieval; not evidence the model followed its methodology.", properties.clone(), &["skillId"]),
        tool("reviewer_skill_verify", "Run a read-only native pinned-input integrity check, not arbitrary skill scripts or project tests.", properties, &["skillId"]),
    ]
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request { skill_id: String }

pub(super) fn dispatch_state(
    state: &AppState, parent: &str, generation: u64, actor: &coordination::Actor,
    name: &str, args: Value,
) -> Result<(Value, Option<TeamSnapshot>)> {
    let request: Request = serde_json::from_value(args)
        .map_err(|err| format!("reviewer_skill_unavailable: invalid arguments: {err}"))?;
    let (id, attempt) = match actor {
        coordination::Actor::Worker { id, attempt } => (id, attempt),
        coordination::Actor::Architect => return Err("reviewer_skill_unavailable: only a reviewer can use these tools.".into()),
    };
    let store = state.store.lock().map_err(|err| err.to_string())?;
    let mut team = load(&store, parent)?.ok_or("reviewer_session_mismatch: Council unavailable.")?;
    // A persisted TeamSnapshot may still be running after the native root
    // session was interrupted, completed or archived. Never mint additional
    // host-observed evidence from a revoked task, even through an old tool host.
    let root_session = store.session(parent)
        .map_err(|_| "reviewer_session_mismatch: native root session unavailable.".to_string())?;
    if root_session.archived || root_session.task_stage == "completed"
        || matches!(root_session.status.as_str(), "interrupted" | "error")
    {
        return Err("reviewer_session_mismatch: native root Council session is no longer authorized.".into());
    }
    if team.generation != generation || !active(&team.status) {
        return Err("reviewer_session_mismatch: Council is no longer active.".into());
    }
    let index = team.workers.iter().position(|w| w.id == *id && w.attempt == *attempt
        && active(&w.status) && !w.abandoned && w.council.is_some())
        .ok_or("reviewer_session_mismatch: reviewer run is not active.")?;
    // A retained host can outlive its worker session while the persisted
    // worker row still appears active. Revoked or foreign leaf sessions must
    // not mint host-observed evidence, even with a valid root and seat.
    let reviewer_session = store.session(&team.workers[index].session_id)
        .map_err(|_| "reviewer_session_mismatch: native reviewer session unavailable.".to_string())?;
    // A retained reviewer tool host cannot mint evidence after its native
    // session is reassigned to another task, checkout, or permission mode.
    // Direct Council seats inherit their root's mode; nested child seats
    // inherit plan mode. Compare the original task scope, not a blanket
    // full/plan assumption that would break legacy M1/M2.
    if super::parent_of(&store, &reviewer_session.id)?.as_deref() != Some(parent)
        || reviewer_session.project_id != root_session.project_id
        || reviewer_session.worktree_path != root_session.worktree_path
        || reviewer_session.mode != root_session.mode
        || reviewer_session.archived || reviewer_session.task_stage == "completed"
        || matches!(reviewer_session.status.as_str(), "interrupted" | "error" | "completed")
    {
        return Err("reviewer_session_mismatch: native reviewer session is no longer authorized.".into());
    }
    let binding = team.workers[index].reviewer_binding.as_ref()
        .ok_or("reviewer_skill_unavailable: seat has no reviewer authority.")?;
    binding.validate(&team, &team.workers[index])?;
    // The original M1/M2 parallel path alone may consume a pre-#147 native
    // read-only fallback. Composed sources, S2/S3 and named criteria require
    // an independently issued host authority record.
    let legacy_grant = team.config.council.as_ref().is_some_and(|config| {
        matches!(config.mode(), "m1" | "m2")
            && config.strategy == council::CouncilExecutionStrategy::Parallel
            && config.criteria_source == council::CouncilCriteriaSource::Architect
            && config.criteria.is_empty()
    });
    if legacy_grant {
        reviewer_security::validate_issued_authority(&store, parent, binding)?;
    } else {
        reviewer_security::validate_recorded_authority(&store, parent, binding)?;
    }
    let frozen = binding.snapshot.skills.as_ref()
        .ok_or("reviewer_skill_unavailable: seat has no frozen skills.")?;
    frozen.verify()?;
    let pin = frozen.pins.iter().find(|p| p.id == request.skill_id && p.eligible && p.supplied)
        .ok_or("reviewer_skill_unavailable: this skill was not supplied to this seat.")?;
    let sha = pin.content_sha256.clone();
    let revision = binding.artifact_revision.clone();
    let mode = pin.delivery_mode.clone();
    let staged_path = pin.staged_path.clone();
    let lower_trust_instructions = frozen.prompt.clone();
    let criterion = team.config.council.as_ref().and_then(|config| config.criteria.iter()
        .find(|criterion| criterion.native_evidence.as_ref().is_some_and(|check|
            check.kind == INTEGRITY && check.skill_id == request.skill_id))).cloned();
    let previous = binding.skill_evidence.get(&request.skill_id).cloned();
    let mut observed = previous.clone().unwrap_or_else(|| ObservedSkillEvidence {
        content_sha256: sha.clone(), artifact_revision: revision.clone(),
        invoked: false, check_executed: false, evidence_verified: false, check_kind: None,
        criterion_receipt: None,
    });
    if observed.content_sha256 != sha || observed.artifact_revision != revision {
        return Err("reviewer_skill_drift: stale native evidence cannot be reused.".into());
    }
    match name {
        "pandacode_reviewer_skill_open" => observed.invoked = true,
        "pandacode_reviewer_skill_verify" => {
            if !observed.invoked {
                return Err("reviewer_skill_evidence_unverified: first open this pinned skill using its native host tool.".into());
            }
            // The native authority pins the snapshot; verify() validates staged bytes
            // for file harnesses. No skill script or project test is executed here.
            observed.check_executed = true;
            observed.evidence_verified = true;
            observed.check_kind = Some(INTEGRITY.into());
            observed.criterion_receipt = criterion.as_ref().and_then(|criterion|
                NativeCriterionEvidenceReceipt::from_host_check(
                    &team, &team.workers[index], criterion, &request.skill_id, &sha));
        }
        _ => return Err("Unknown reviewer skill tool.".into()),
    }
    let changed = previous.as_ref() != Some(&observed);
    if changed {
        team.workers[index].reviewer_binding.as_mut().unwrap().skill_evidence
            .insert(request.skill_id.clone(), observed.clone());
        save(&store, &mut team)?;
    }
    Ok((json!({
        "skillId": request.skill_id, "contentSha256": sha,
        "artifactRevision": revision, "deliveryMode": mode,
        "readOnlyPath": staged_path,
        // A native open delivers frozen lower-trust input, not merely a model claim.
        "lowerTrustInstructions": (name == "pandacode_reviewer_skill_open").then_some(lower_trust_instructions),
        "nativeRetrievalRecorded": observed.invoked,
        "pinnedInputIntegrityVerified": observed.check_executed && observed.evidence_verified,
        "criterionEvidenceBound": observed.criterion_receipt.is_some(),
        "note": "A native input-integrity check is not proof of project tests or methodological adherence."
    }), changed.then_some(team)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_readonly_host_rejects_any_unlisted_tool_at_dispatch() {
        let mut catalog = tools();
        catalog.push(tool("dispatch_workers", "Unavailable to reviewers",
            json!({"assignments":{"type":"array"}}), &["assignments"]));
        let raw = ToolHost {
            tools: catalog, approval_tools: HashSet::new(),
            handler: Arc::new(|_, _| Box::pin(async { Ok(json!({"executed": true})) })),
        };
        let filtered = readonly_host(raw).unwrap();
        assert_eq!(filtered.tools.len(), 2);
        assert!(!filtered.tools.iter().any(|spec| spec["name"] == "pandacode_dispatch_workers"));
        assert!((filtered.handler)("pandacode_dispatch_workers".into(), json!({})).await.is_err());
        assert!((filtered.handler)("pandacode_reviewer_skill_open".into(), json!({"skillId":"safe"})).await.is_ok());
    }
}
