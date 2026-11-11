//! Read-only bridge from an original SuperAgent worker session to the
//! existing Architect/Council #126 state. The native Council owns reviewer
//! eligibility, snapshots, verdicts and tool restrictions; never reimplement
//! scores or treat its team result as authority for an unbound ticket artifact.
use super::Ticket;
use crate::{architect, storage::Store, Result};
use rusqlite::OptionalExtension;
use serde::Serialize;

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
    let session: Option<String> = store.db.query_row(
        "SELECT a.session_id FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE t.id=?1 AND a.bot_id=t.worker_bot_id
           AND a.state='finished' AND a.result=t.evidence
           AND a.attempt=(SELECT MAX(attempt) FROM sa_assignments
                          WHERE ticket_id=?1)",
        [&ticket.id], |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let Some(session) = session else { return Ok(None) };
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
    Ok(Some(CouncilObservation {
        native_status,
        native_round: status.round,
        native_stale: status.stale,
        native_pass_verified: verified,
        ticket_artifact_bound: false,
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
