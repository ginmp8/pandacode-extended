//! Native reviewer skill events are recorded by the host, not inferred from model reports.
use super::*;
const INTEGRITY: &str = "pinned_input_integrity";

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
}
impl ObservedSkillEvidence {
    pub fn valid_for(&self, pin: &crate::skills::reviewer::SkillPin, revision: &str) -> bool {
        self.content_sha256 == pin.content_sha256 && self.artifact_revision == revision
            && self.invoked && self.check_executed && self.evidence_verified
            && self.check_kind.as_deref() == Some(INTEGRITY)
    }
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
    if team.generation != generation || !active(&team.status) {
        return Err("reviewer_session_mismatch: Council is no longer active.".into());
    }
    let index = team.workers.iter().position(|w| w.id == *id && w.attempt == *attempt
        && active(&w.status) && !w.abandoned && w.council.is_some())
        .ok_or("reviewer_session_mismatch: reviewer run is not active.")?;
    let binding = team.workers[index].reviewer_binding.as_ref()
        .ok_or("reviewer_skill_unavailable: seat has no reviewer authority.")?;
    binding.validate(&team, &team.workers[index])?;
    reviewer_security::validate_issued_authority(&store, parent, binding)?;
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
    let previous = binding.skill_evidence.get(&request.skill_id).cloned();
    let mut observed = previous.clone().unwrap_or_else(|| ObservedSkillEvidence {
        content_sha256: sha.clone(), artifact_revision: revision.clone(),
        invoked: false, check_executed: false, evidence_verified: false, check_kind: None,
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
        "note": "A native input-integrity check is not proof of project tests or methodological adherence."
    }), changed.then_some(team)))
}
