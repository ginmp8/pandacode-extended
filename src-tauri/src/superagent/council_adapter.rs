//! Read-only bridge from an original SuperAgent worker session to the
//! existing Architect/Council #126 state. The native Council owns reviewer
//! eligibility, snapshots, verdicts and tool restrictions; never reimplement
//! scores or treat its team result as authority for an unbound ticket artifact.
use super::Ticket;
use crate::{architect, storage::{self, Store}, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
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
    /// No frozen ticket-artifact digest/cycle binding exists yet; S3 must
    /// never make a task completion decision from this observation.
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
}


fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

    let target: Option<(String, String, i64, i64, String, i64, i64, String, i64)> =
        store.db.query_row(
        "SELECT assignment_id,session_id,ticket_revision,policy_revision,
                artifact_sha256,council_since,council_generation,council_config_sha256,bound_at
         FROM sa_council_targets WHERE ticket_id=?1",
        [&ticket.id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
                              r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?)),
    ).optional().map_err(|e| e.to_string())?;
    let bound_at = target.as_ref().and_then(|(frozen_assignment,frozen_session,revision,policy,
            digest,cycle,generation,config_digest,bound_at)| {
        let council = team.config.council.as_ref()?;
        let config = serde_json::to_vec(council).ok()?;
        (frozen_assignment == &assignment_id
            && frozen_session == &session
            && *revision == ticket.revision
            && *policy == grant.policy_revision
            && *digest == sha256(ticket.evidence.as_deref()?.as_bytes())
            && *cycle == team.council_since
            && i64::try_from(team.generation).ok() == Some(*generation)
            && *config_digest == sha256(&config)).then_some(*bound_at)
    });
    let review_started_after_binding = bound_at.is_some_and(|frozen_at| {
        let mut seats = team.workers.iter()
            .filter(|w| w.created_at >= team.council_since)
            .filter_map(|w| w.council.as_ref())
            .filter(|seat| seat.round == status.round);
        status.round > 0 && seats.next().is_some_and(|s| s.started_at >= frozen_at)
            && seats.all(|s| s.started_at >= frozen_at)
    });
    Ok(Some(CouncilObservation {
        native_status,
        native_round: status.round,
        native_stale: status.stale,
        native_pass_verified: verified,
        ticket_artifact_bound: bound_at.is_some(),
        frozen_assignment_id: bound_at.map(|_| assignment_id),
        frozen_ticket_revision: bound_at.map(|_| ticket.revision),
        review_started_after_binding,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bots, storage::{self, Store}, superagent::{self, NewOrchestration}};
    use rusqlite::params;
    use serde_json::json;

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
        assert!(!observed.review_started_after_binding);
        assert_eq!(saved.state,"waiting_approval");

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
