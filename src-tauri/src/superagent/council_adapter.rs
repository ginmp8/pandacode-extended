//! Read-only bridge from an original SuperAgent worker session to the
//! existing Architect/Council #126 state. The native Council owns reviewer
//! eligibility, snapshots, verdicts and tool restrictions; never reimplement
//! scores or treat its team result as authority for an unbound ticket artifact.
use super::Ticket;
use crate::{architect, storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CouncilObservation {
    /// The verified native worker session has an Architect team with Council
    /// enabled; this is **advisory**, not task approval.
    pub native_status: String,
    pub native_round: usize,
    pub native_stale: bool,
    pub native_pass_verified: bool,
    /// A positive root result is exposed only after a task-bound request,
    /// exact frozen-target correlation and native evidence revalidation.
    pub native_approved: bool,
    /// A frozen task artifact is only a review target, never proof that the
    /// native Council saw, approved or completed this ticket.
    pub ticket_artifact_bound: bool,
    /// Exact immutable assignment and ticket revision that were pinned.
    /// A pin does not certify that reviewers saw or accepted the artifact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frozen_assignment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frozen_ticket_revision: Option<i64>,
    /// All reviewers in the native latest round must have started after the
    /// artifact was frozen; still informational until native approval is wired.
    pub review_started_after_binding: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_receipt: Option<NativeCouncilTaskReceipt>,
    /// Current verified finding and receipt evidence, readable only with the
    /// correlated positive terminal receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_evidence: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeCouncilTaskRequest {
    schema_version: u8,
    request_id: String,
    request_key: String,
    idempotency_key: String,
    caller_id: String,
    ticket_id: String,
    task_id: String,
    assignment_id: String,
    ticket_revision: i64,
    policy_revision: i64,
    cycle_id: String,
    council_generation: i64,
    artifact_sha256: String,
    council_config_sha256: String,
    policy_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeCouncilTaskReceipt {
    pub schema_version: u8,
    pub request_id: String,
    pub request_key: String,
    pub idempotency_key: String,
    pub caller_id: String,
    pub ticket_id: String,
    pub task_id: String,
    pub assignment_id: String,
    pub ticket_revision: i64,
    pub policy_revision: i64,
    pub cycle_id: String,
    pub council_generation: i64,
    pub artifact_sha256: String,
    pub council_config_sha256: String,
    pub policy_fingerprint: String,
    pub council_artifact_revision: String,
    pub council_policy_fingerprint: String,
    pub verdict: String,
    pub approved: bool,
    pub terminal_revision: u64,
    pub evidence_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}


/// A task's frozen native review target. Named fields keep cycle, policy and
/// artifact identity distinct instead of silently mixing tuple indices.
struct FrozenTarget {
    assignment_id: String,
    session_id: String,
    ticket_revision: i64,
    policy_revision: i64,
    artifact_sha256: String,
    council_since: i64,
    council_generation: i64,
    council_config_sha256: String,
    bound_at: i64,
}

/// A full unique round must be newer than the frozen artifact.
fn reviewers_started_after_binding<'a>(
    required: usize, seats: impl Iterator<Item = (&'a str, i64)>, bound_at: i64,
) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut total = 0_usize;
    for (id, started_at) in seats {
        if id.is_empty() || started_at < bound_at || !seen.insert(id) {
            return false;
        }
        total += 1;
    }
    required > 0 && total == required
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Idempotent native records are immutable. A reused request key with any
/// different caller, task, policy or artifact data is a replay conflict.
fn persist_task_record(
    store: &Store,
    task_id: &str,
    key: &str,
    kind: &str,
    content: &str,
    caller_id: &str,
) -> Result<()> {
    store.db.execute(
        "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
         VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(parent_session_id,key) DO NOTHING",
        params![task_id, key, kind, content, caller_id],
    ).map_err(|error| error.to_string())?;
    let saved: (String, String, String) = store.db.query_row(
        "SELECT kind,content,worker_id FROM architect_requests
         WHERE parent_session_id=?1 AND key=?2",
        params![task_id, key],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|error| error.to_string())?;
    if saved != (kind.to_string(), content.to_string(), caller_id.to_string()) {
        return Err("council_task_replay_mismatch: the native request key is already bound to different task data.".into());
    }
    Ok(())
}

/// Atomically freeze a finished worker's exact ticket artifact against the
/// existing #126 native team/cycle. This is called *inside* the worker-result
/// transaction, and has no effect when Council is off or not configured.
/// It never creates a team, reviewer, synthetic verdict or external action.
pub(super) fn bind_finished(
    store: &Store, ticket_id: &str, assignment_id: &str, session_id: &str,
) -> Result<()> {
    let Some(team) = architect::load(store, session_id)? else { return Ok(()) };
    let Some(council) = team.config.council.as_ref() else { return Ok(()) };
    if team.session_id != session_id || team.council_since <= 0 {
        return Ok(());
    }
    let ticket = super::get_ticket(store, ticket_id)?;
    if ticket.state != "waiting_approval" || ticket.evidence.as_deref().is_none_or(str::is_empty) {
        return Ok(());
    }
    // A caller cannot bind a coordinator plan, another ticket's assignment,
    // an uncommitted/failed callback, or a substituted evidence string.
    let matched: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE a.id=?1 AND a.ticket_id=?2 AND a.session_id=?3
           AND a.bot_id=t.worker_bot_id AND a.state='finished'
           AND a.result=t.evidence
           AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments WHERE ticket_id=?2))",
        params![assignment_id, ticket_id, session_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if !matched || !super::assignment_thread_current(store, session_id, ticket_id)? {
        return Ok(());
    }
    let generation = i64::try_from(team.generation)
        .map_err(|_| "Council generation exceeds supported range.")?;
    let config_bytes = serde_json::to_vec(council).map_err(|e| e.to_string())?;
    let policy_revision = super::mandate(store, ticket_id)?.policy_revision;
    store.db.execute(
        "INSERT INTO sa_council_targets(
           ticket_id,assignment_id,session_id,ticket_revision,policy_revision,
           artifact_sha256,council_since,council_generation,council_config_sha256,bound_at
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(ticket_id) DO UPDATE SET
           assignment_id=excluded.assignment_id,session_id=excluded.session_id,
           ticket_revision=excluded.ticket_revision,policy_revision=excluded.policy_revision,
           artifact_sha256=excluded.artifact_sha256,council_since=excluded.council_since,
           council_generation=excluded.council_generation,
           council_config_sha256=excluded.council_config_sha256,bound_at=excluded.bound_at",
        params![ticket_id,assignment_id,session_id,ticket.revision,policy_revision,
            sha256(ticket.evidence.as_deref().unwrap_or_default().as_bytes()),
            team.council_since,generation,sha256(&config_bytes),storage::now()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Only the latest, finished, exactly correlated worker evidence is eligible
/// for a Council lookup. A different session, coordinator plan, stale attempt
/// or remapped bot thread cannot borrow Council status from another task.
pub(super) fn inspect(store: &Store, ticket: &Ticket) -> Result<Option<CouncilObservation>> {
    if !matches!(ticket.state.as_str(), "waiting_approval" | "waiting_review")
        || ticket.evidence.as_deref().is_none_or(str::is_empty)
        || super::mandate(store, &ticket.id)?.revoked
    {
        return Ok(None);
    }
    let grant = super::mandate(store, &ticket.id)?;
    if grant.expires_at <= storage::now() || grant.ticket_id != ticket.id {
        return Ok(None);
    }
    let assignment: Option<(String, String)> = store.db.query_row(
        "SELECT a.id,a.session_id FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE t.id=?1 AND a.bot_id=t.worker_bot_id
           AND a.state='finished' AND a.result=t.evidence
           AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments
                          WHERE ticket_id=?1)",
        [&ticket.id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((assignment_id, session)) = assignment else { return Ok(None) };
    if !super::assignment_thread_current(store, &session, &ticket.id)? {
        return Ok(None);
    }
    let Some(team) = architect::load(store, &session)? else { return Ok(None) };
    if team.session_id != session || team.config.council.is_none() {
        return Ok(None);
    }
    let Some(status) = team.council_status.as_ref() else { return Ok(None) };
    // Recompute through the *existing* native Council gate. Its cached
    // "passed" or "accepted" labels never authorize a SuperAgent Ticket.
    let verified = architect::verified_council_pass(store, &team);
    let native_status = if matches!(status.state.as_str(),"passed" | "accepted") && !verified {
        "unverified".into()
    } else {
        status.state.clone()
    };

    let target: Option<FrozenTarget> = store.db.query_row(
        "SELECT assignment_id,session_id,ticket_revision,policy_revision,
                artifact_sha256,council_since,council_generation,council_config_sha256,bound_at
         FROM sa_council_targets WHERE ticket_id=?1",
        [&ticket.id], |r| Ok(FrozenTarget {
            assignment_id: r.get(0)?, session_id: r.get(1)?,
            ticket_revision: r.get(2)?, policy_revision: r.get(3)?,
            artifact_sha256: r.get(4)?, council_since: r.get(5)?,
            council_generation: r.get(6)?, council_config_sha256: r.get(7)?,
            bound_at: r.get(8)?,
        }),
    ).optional().map_err(|e| e.to_string())?;
    let bound_at = target.as_ref().and_then(|target| {
        let council = team.config.council.as_ref()?;
        let config = serde_json::to_vec(council).ok()?;
        (target.assignment_id == assignment_id
            && target.session_id == session
            && target.ticket_revision == ticket.revision
            && target.policy_revision == grant.policy_revision
            && target.artifact_sha256 == sha256(ticket.evidence.as_deref()?.as_bytes())
            && target.council_since == team.council_since
            && i64::try_from(team.generation).ok() == Some(target.council_generation)
            && target.council_config_sha256 == sha256(&config)).then_some(target.bound_at)
    });
    let request = if bound_at.is_some() {
        let caller_id = format!("superagent-ticket:{}", ticket.id);
        let cycle_id = format!("{}:{}", session, team.council_since);
        let config = team.config.council.as_ref().ok_or("Native Council configuration disappeared.")?;
        let config_bytes = serde_json::to_vec(config).map_err(|error| error.to_string())?;
        let policy_bytes = serde_json::to_vec(&grant).map_err(|error| error.to_string())?;
        let request_id = format!(
            "superagent-council-request:{}:{}:{}:{}",
            ticket.id, assignment_id, ticket.revision, cycle_id,
        );
        let idempotency_key = format!("superagent-council-terminal:{request_id}");
        let request = NativeCouncilTaskRequest {
            schema_version: 1,
            request_id: request_id.clone(),
            request_key: request_id,
            idempotency_key: idempotency_key.clone(),
            caller_id: caller_id.clone(),
            ticket_id: ticket.id.clone(),
            task_id: session.clone(),
            assignment_id: assignment_id.clone(),
            ticket_revision: ticket.revision,
            policy_revision: grant.policy_revision,
            cycle_id,
            council_generation: i64::try_from(team.generation).map_err(|_| "Council generation exceeds supported range.")?,
            artifact_sha256: sha256(ticket.evidence.as_deref().ok_or("Ticket evidence disappeared.")?.as_bytes()),
            council_config_sha256: sha256(&config_bytes),
            policy_fingerprint: sha256(&policy_bytes),
        };
        let request_json = serde_json::to_string(&request).map_err(|error| error.to_string())?;
        persist_task_record(
            store,
            &session,
            &request.request_key,
            "council_task_request",
            &request_json,
            &caller_id,
        )?;
        Some(request)
    } else {
        None
    };
    let review_started_after_binding = bound_at.is_some_and(|frozen_at| {
        let Some(config) = team.config.council.as_ref() else { return false };
        let required = if config.reviewer_bot_ids.is_empty() {
            config.reviewers.max(2)
        } else {
            config.reviewer_bot_ids.len()
        };
        status.round > 0 && reviewers_started_after_binding(
            required,
            team.workers.iter()
                .filter(|worker| worker.created_at >= team.council_since)
                .filter_map(|worker| worker.council.as_ref()
                    .filter(|seat| seat.round == status.round)
                    .map(|seat| (worker.session_id.as_str(), seat.started_at))),
            frozen_at,
        )
    });
    let mut approval_receipt = None;
    let mut authorized_evidence = None;
    if let Some(request) = request.as_ref() {
        let verified_evidence = architect::verified_council_task_evidence(store, &team)
            .filter(|evidence| evidence.cycle_id == request.cycle_id);
        let approved = verified && review_started_after_binding && verified_evidence.is_some();
        let terminal = approved || matches!(team.status.as_str(), "completed" | "failed" | "stopped" | "interrupted");
        if terminal {
            let evidence_json = verified_evidence.as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| error.to_string())?;
            let evidence_bytes = evidence_json.as_ref()
                .map(serde_json::to_vec)
                .transpose()
                .map_err(|error| error.to_string())?
                .unwrap_or_default();
            let receipt = NativeCouncilTaskReceipt {
                schema_version: 1,
                request_id: request.request_id.clone(),
                request_key: request.request_key.clone(),
                idempotency_key: request.idempotency_key.clone(),
                caller_id: request.caller_id.clone(),
                ticket_id: request.ticket_id.clone(),
                task_id: request.task_id.clone(),
                assignment_id: request.assignment_id.clone(),
                ticket_revision: request.ticket_revision,
                policy_revision: request.policy_revision,
                cycle_id: request.cycle_id.clone(),
                council_generation: request.council_generation,
                artifact_sha256: request.artifact_sha256.clone(),
                council_config_sha256: request.council_config_sha256.clone(),
                policy_fingerprint: request.policy_fingerprint.clone(),
                council_artifact_revision: verified_evidence.as_ref()
                    .map_or_else(String::new, |evidence| evidence.artifact_revision.clone()),
                council_policy_fingerprint: verified_evidence.as_ref()
                    .map_or_else(String::new, |evidence| evidence.policy_fingerprint.clone()),
                verdict: if approved { "approved".into() } else { format!("not_approved:{}", status.state) },
                approved,
                terminal_revision: verified_evidence.as_ref()
                    .map_or(team.dispatch_revision, |evidence| evidence.terminal_revision),
                evidence_sha256: sha256(&evidence_bytes),
                reason: (!approved).then(|| status.notice.clone().unwrap_or_else(|| {
                    if verified { "native_pass_correlation_failed".into() } else { format!("native_council_{}", native_status) }
                })),
            };
            let receipt_json = serde_json::to_string(&receipt).map_err(|error| error.to_string())?;
            persist_task_record(
                store,
                &request.task_id,
                &request.idempotency_key,
                "council_task_receipt",
                &receipt_json,
                &request.caller_id,
            )?;
            if approved {
                authorized_evidence = evidence_json;
            }
            approval_receipt = Some(receipt);
        }
    }
    let native_approved = approval_receipt.as_ref().is_some_and(|receipt| receipt.approved);
    Ok(Some(CouncilObservation {
        native_status,
        native_round: status.round,
        native_stale: status.stale,
        native_pass_verified: verified,
        native_approved,
        ticket_artifact_bound: bound_at.is_some(),
        frozen_assignment_id: bound_at.map(|_| assignment_id),
        frozen_ticket_revision: bound_at.map(|_| ticket.revision),
        review_started_after_binding,
        approval_receipt,
        authorized_evidence,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bots, storage::{self, Store}, superagent::{self, NewOrchestration}};
    use rusqlite::params;
    use serde_json::json;

    #[test]
    fn partial_duplicate_and_stale_rounds_cannot_claim_fresh_review() {
        assert!(!reviewers_started_after_binding(2, [("a",10)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",10),("a",11)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",9),("b",11)].into_iter(), 10));
        assert!(!reviewers_started_after_binding(2, [("a",10),("b",11),("c",12)].into_iter(), 10));
        assert!(reviewers_started_after_binding(2, [("a",10),("b",11)].into_iter(), 10));
    }

    fn fixture() -> Store {
        let path = std::env::temp_dir().join(format!("sa-council-{}.sqlite3",storage::id()));
        let store = Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path)
             VALUES('project','Disposable','/tmp/disposable-council-bridge')",[],
        ).unwrap();
        for (id,name) in [("router","Router"),("worker","Builder")] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?2,'project','codex','default','plan',?3,?3)",
                params![id,name,storage::now()],
            ).unwrap();
        }
        assert!(bots::get(&store,"worker").is_ok());
        store
    }
    fn settled(store: &mut Store, key: &str) -> (superagent::Ticket,String) {
        let cfg = superagent::create_configuration(store,NewOrchestration {
            name: format!("Council {key}"),
            coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1
             WHERE id=?1", [&cfg.id],
        ).unwrap();
        let (ticket, launched)=superagent::admit(store,&cfg.id,"Inspect safely",key,false).unwrap();
        assert!(launched);
        let session=superagent::assignment_session(store,&ticket.id).unwrap().unwrap();
        assert!(superagent::on_run_finished(store,&session,true,"Worker evidence").unwrap());
        (superagent::get_ticket(store,&ticket.id).unwrap(),session)
    }

    fn attach_native_council(store: &Store, session: &str) {
        let team=json!({
            "sessionId":session,
            "config":{
                "workers":[],"maxParallel":1,"maxRounds":2,"maxAssignments":2,
                "workspace":"/tmp/disposable-council-bridge","summaryLimit":3000,
                "council":{"target":9.0,"maxRounds":2,"reviewers":2}
            },
            "status":"ready","round":1,"revision":1,"workers":[],
            "error":null,"summary":null,"updatedAt":storage::now(),
            "councilSince":storage::now(),"generation":1,
            "councilStatus":{"state":"waiting","round":0,"lowest":null,
                             "target":9.0,"stale":false}
        });
        store.db.execute(
            "INSERT INTO architect_teams(session_id,content) VALUES(?1,?2)",
            params![session,team.to_string()],
        ).unwrap();
    }

    fn attach_correlated_passing_reviews(store: &Store, session: &str) {
        let mut team = architect::load(store, session).unwrap().unwrap();
        let bound_at: i64 = store.db.query_row(
            "SELECT bound_at FROM sa_council_targets WHERE ticket_id=(
               SELECT ticket_id FROM sa_assignments WHERE session_id=?1
             )",
            [session], |row| row.get(0),
        ).unwrap();
        team.status = "completed".into();
        team.round = 1;
        team.dispatch_revision = 30;
        team.changed_at = 0;
        team.council_status = Some(architect::CouncilStatus {
            state: "passed".into(), round: 1, lowest: Some(9.5), target: 9.0,
            stale: false, notice: None,
        });
        let project_id = store.session(session).unwrap().project_id;
        for (index, id) in ["native-review-a", "native-review-b"].into_iter().enumerate() {
            let created_at = bound_at + index as i64 + 1;
            let reviewer_session = store.create_session(&project_id, "codex", "default", "ask").unwrap();
            let seat = architect::CouncilSeat {
                round: 1, lens: format!("lens-{index}"), score: Some(9.6 - index as f64 / 10.0),
                closed_by_selection: false, started_at: created_at,
            };
            let mut worker = architect::Worker {
                id: id.into(), session_id: reviewer_session.id, name: id.into(),
                profile_id: "automatic".into(), provider: "codex".into(), model: "default".into(),
                reasoning_effort: None, status: "completed".into(), assignment: "Review the task.".into(),
                summary: "Verified current task evidence.".into(), error: None, depends_on: Vec::new(),
                files: vec!["src".into()], read_only: true, attempt: 1, created_at,
                updated_at: created_at + 1, keys: vec![format!("review-{index}")], abandoned: false,
                result_revision: 20 + index as u64, checkpoint: None, timed_out: false, failovers: 0,
                parent_worker: None, resume_prompt: None, tool_catalog_version: 0, nudged: false,
                reported: false, council: Some(seat.clone()), reviewer_binding: None,
                score_nudged: false, wrote_at: 0,
            };
            let binding = architect::reviewer_security::ReviewerBinding::native(
                &team, &worker.id, created_at, &worker.provider, &worker.model, &seat,
            );
            architect::reviewer_security::record_native_authority(store, session, &binding).unwrap();
            worker.reviewer_binding = Some(binding.clone());
            let frozen_policy = serde_json::to_vec(&(
                &binding.snapshot.tool_policy, &binding.reviewer_bot_id,
                &binding.snapshot.bot_context, &binding.snapshot.skills,
                &binding.composition_mode, &binding.delivery_id, &binding.parent_council,
            )).unwrap();
            let cycle_id = binding.council_cycle_id.clone();
            let request_id = format!("native-council-request:{cycle_id}:{}:1", worker.id);
            let receipt = json!({
                "schemaVersion":1,"requestId":request_id,"cycleId":cycle_id,
                "taskId":session,"assignmentId":worker.id,
                "policyFingerprint":sha256(&frozen_policy),
                "artifactDigest":sha256(binding.artifact_revision.as_bytes()),
                "verdict":"passed","findingsRefs":[],"evidenceRefs":[],
                "terminalRevision":worker.result_revision,
                "idempotencyKey":format!("native-council-terminal:{}:{}:1",binding.council_cycle_id,worker.id)
            });
            store.db.execute(
                "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
                 VALUES(?1,?2,'council_review_receipt',?3,?4)",
                params![session,format!("native-council-review-receipt:{}:{}:1",binding.council_cycle_id,worker.id),receipt.to_string(),worker.id],
            ).unwrap();
            team.workers.push(worker);
        }
        store.db.execute(
            "UPDATE architect_teams SET content=?2 WHERE session_id=?1",
            params![session,serde_json::to_string(&team).unwrap()],
        ).unwrap();
    }

    #[test]
    fn finished_worker_pins_native_council_cycle_and_exact_artifact_without_approval() {
        let mut store=fixture();
        let cfg=superagent::create_configuration(&store,NewOrchestration {
            name:"Pinned native review".into(),coordinator_bot_id:"router".into(),
            worker_bot_id:"worker".into(),
        }).unwrap();
        store.db.execute("UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&cfg.id]).unwrap();
        let (ticket, launched)=superagent::admit(&mut store,&cfg.id,"Review exact evidence","pin",false).unwrap();
        assert!(launched);
        let session=superagent::assignment_session(&store,&ticket.id).unwrap().unwrap();
        attach_native_council(&store,&session);
        assert!(superagent::on_run_finished(&store,&session,true,"Artifact revision A").unwrap());
        let saved=superagent::get_ticket(&store,&ticket.id).unwrap();
        let observed=inspect(&store,&saved).unwrap().unwrap();
        assert!(observed.ticket_artifact_bound);
        assert_eq!(observed.frozen_ticket_revision,Some(saved.revision));
        assert!(observed.frozen_assignment_id.is_some());
        assert!(!observed.native_pass_verified);
        assert!(!observed.native_approved);
        assert!(!observed.review_started_after_binding);
        assert_eq!(saved.state,"waiting_approval");
        let request: (String, String, String) = store.db.query_row(
            "SELECT key,content,worker_id FROM architect_requests
             WHERE parent_session_id=?1 AND kind='council_task_request'",
            [&session], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).unwrap();
        let request: NativeCouncilTaskRequest=serde_json::from_str(&request.1).unwrap();
        assert_eq!(request.ticket_id,saved.id);
        assert_eq!(request.task_id,session);
        assert_eq!(request.assignment_id,observed.frozen_assignment_id.unwrap());
        assert_eq!(request.ticket_revision,saved.revision);
        assert_eq!(request.cycle_id,format!("{}:{}",request.task_id,request.cycle_id.rsplit(':').next().unwrap()));
        assert_eq!(store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_receipt'",[&session],|row|row.get(0),
        ).unwrap(),0);

        // Even if a stale Council team reports a passing label, it cannot
        // complete a Ticket or bypass its native evidence/authority checks.
        store.db.execute(
            "UPDATE architect_teams SET content=json_set(content,
              '$.councilStatus.state','passed','$.councilStatus.round',1)
             WHERE session_id=?1", [&session],
        ).unwrap();
        assert!(!inspect(&store,&saved).unwrap().unwrap().native_pass_verified);
        assert_eq!(superagent::get_ticket(&store,&saved.id).unwrap().state,"waiting_approval");

        // Changing the native Council cycle, frozen policy or ticket revision
        // invalidates the bind without recycling the old passing observation.
        store.db.execute(
            "UPDATE architect_teams SET content=json_set(content,
              '$.councilSince',json_extract(content,'$.councilSince')+1)
             WHERE session_id=?1", [&session],
        ).unwrap();
        assert!(!inspect(&store,&saved).unwrap().unwrap().ticket_artifact_bound);
        store.db.execute(
            "UPDATE architect_teams SET content=json_set(content,
              '$.councilSince',json_extract(content,'$.councilSince')-1)
             WHERE session_id=?1", [&session],
        ).unwrap();
        store.db.execute("UPDATE sa_tickets SET revision=revision+1 WHERE id=?1",
            [&saved.id]).unwrap();
        let revised=superagent::get_ticket(&store,&saved.id).unwrap();
        assert!(!inspect(&store,&revised).unwrap().unwrap().ticket_artifact_bound);
    }

    #[test]
    fn council_target_never_leaks_across_distinct_tickets_or_remapped_threads() {
        let mut store=fixture();
        let (first,session)=settled(&mut store,"other");
        // No native Council during the first task: there is nothing to bind.
        assert!(inspect(&store,&first).unwrap().is_none());
        attach_native_council(&store,&session);
        // A post-settlement team cannot retroactively approve old evidence.
        assert!(inspect(&store,&first).unwrap().is_some());
        assert!(!inspect(&store,&first).unwrap().unwrap().ticket_artifact_bound);
        let (second,other_session)=settled(&mut store,"second");
        attach_native_council(&store,&other_session);
        assert!(!inspect(&store,&second).unwrap().unwrap().ticket_artifact_bound);
        store.db.execute(
            "UPDATE bot_threads SET external_id='foreign-ticket' WHERE session_id=?1",
            [&session],
        ).unwrap();
        assert!(inspect(&store,&first).unwrap().is_none());
    }

    #[test]
    fn council_off_manual_ticket_has_no_implicit_reviewer_or_gate() {
        let mut store=fixture();
        let (ticket,_) = settled(&mut store,"off");
        assert_eq!(ticket.state,"waiting_approval");
        assert!(inspect(&store,&ticket).unwrap().is_none());
        let snapshot=superagent::ticket_snapshot(&store,&ticket.id,10).unwrap();
        assert!(snapshot.council.is_none());
        // Existing manual Council-Off acceptance is unaffected.
        let accepted=superagent::accept(&store,&ticket.id,ticket.revision).unwrap();
        assert_eq!(accepted.state,"completed");
    }

    #[test]
    fn correlated_current_native_pass_returns_one_approved_receipt_and_authorized_evidence() {
        let mut store=fixture();
        let cfg=superagent::create_configuration(&store,NewOrchestration {
            name:"Correlated native approval".into(),coordinator_bot_id:"router".into(),worker_bot_id:"worker".into(),
        }).unwrap();
        store.db.execute("UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1", [&cfg.id]).unwrap();
        let (ticket, launched)=superagent::admit(&mut store,&cfg.id,"Review the exact ticket artifact","positive",false).unwrap();
        assert!(launched);
        let session=superagent::assignment_session(&store,&ticket.id).unwrap().unwrap();
        attach_native_council(&store,&session);
        assert!(superagent::on_run_finished(&store,&session,true,"Exact worker artifact").unwrap());
        let saved=superagent::get_ticket(&store,&ticket.id).unwrap();
        attach_correlated_passing_reviews(&store,&session);

        let first=inspect(&store,&saved).unwrap().unwrap();
        assert!(first.native_pass_verified, "native observation: {first:?}");
        assert!(first.ticket_artifact_bound);
        assert!(first.review_started_after_binding);
        assert!(first.native_approved);
        assert_eq!(first.approval_receipt.as_ref().unwrap().verdict,"approved");
        assert_eq!(first.approval_receipt.as_ref().unwrap().ticket_id,saved.id);
        assert!(first.authorized_evidence.as_ref().is_some_and(|evidence| evidence["cycleId"].is_string()));

        let replay=inspect(&store,&saved).unwrap().unwrap();
        assert_eq!(replay.approval_receipt,first.approval_receipt,"the correlated terminal receipt is immutable across replay");
        assert_eq!(replay.authorized_evidence,first.authorized_evidence);
        assert_eq!(store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_request'",[&session],|row|row.get(0),
        ).unwrap(),1);
        assert_eq!(store.db.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM architect_requests WHERE parent_session_id=?1 AND kind='council_task_receipt'",[&session],|row|row.get(0),
        ).unwrap(),1);

        store.db.execute("UPDATE sa_tickets SET revision=revision+1 WHERE id=?1",[&saved.id]).unwrap();
        let revised=superagent::get_ticket(&store,&saved.id).unwrap();
        let denied=inspect(&store,&revised).unwrap().unwrap();
        assert!(!denied.ticket_artifact_bound);
        assert!(!denied.native_approved);
        assert!(denied.authorized_evidence.is_none());
    }

    #[test]
    fn native_task_request_replay_with_changed_caller_or_content_is_denied() {
        let mut store=fixture();
        let (_,session)=settled(&mut store,"request-replay");
        persist_task_record(&store,&session,"native-request-key","council_task_request","exact-request","superagent-ticket:a").unwrap();
        persist_task_record(&store,&session,"native-request-key","council_task_request","exact-request","superagent-ticket:a").unwrap();
        assert!(persist_task_record(&store,&session,"native-request-key","council_task_request","changed-request","superagent-ticket:a").unwrap_err().contains("council_task_replay_mismatch"));
        assert!(persist_task_record(&store,&session,"native-request-key","council_task_request","exact-request","superagent-ticket:b").unwrap_err().contains("council_task_replay_mismatch"));
    }

    #[test]
    fn forged_or_unbound_council_status_cannot_become_ticket_approval() {
        let mut store=fixture();
        let (ticket,session)=settled(&mut store,"advisory");
        // A persisted Architect team is native input, but a cached "passed"
        // label without seats, frozen evidence or a ticket-artifact binding
        // must not acquire SuperAgent completion authority.
        let snapshot=json!({
            "sessionId":session,
            "config":{
                "workers":[],"maxParallel":1,"maxRounds":2,"maxAssignments":2,
                "workspace":"/tmp/disposable-council-bridge","summaryLimit":3000,
                "council":{"target":9.0,"maxRounds":2,"reviewers":1}
            },
            "status":"ready","round":1,"revision":1,"workers":[],
            "error":null,"summary":null,"updatedAt":storage::now(),
            "councilStatus":{
                "state":"passed","round":1,"lowest":9.5,
                "target":9.0,"stale":false
            }
        });
        store.db.execute(
            "INSERT INTO architect_teams(session_id,content) VALUES(?1,?2)",
            params![&session,snapshot.to_string()],
        ).unwrap();
        let observation=inspect(&store,&ticket).unwrap().unwrap();
        assert_eq!(observation.native_status,"unverified");
        assert!(!observation.native_pass_verified);
        assert!(!observation.ticket_artifact_bound);
        assert_eq!(superagent::get_ticket(&store,&ticket.id).unwrap().state,"waiting_approval");
        // Detaching the original worker thread removes even advisory status.
        store.db.execute(
            "UPDATE bot_threads SET external_id='another-ticket' WHERE session_id=?1",
            [&session],
        ).unwrap();
        assert!(inspect(&store,&ticket).unwrap().is_none());
    }
}
