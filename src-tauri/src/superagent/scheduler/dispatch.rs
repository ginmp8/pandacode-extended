//! Native dependency-aware WorkItem dispatch and parent revocation.

use super::*;
use crate::superagent::{self,get_ticket};
use rusqlite::{params,OptionalExtension,Connection};
use tauri::Manager;

#[derive(Debug,Clone)]
struct ReadyItem {
    id:String, root:String, key:String, objective:String, worker:String,
    requires_write:bool,
}

pub(crate) fn parent_active(store:&Store,child_ticket_id:&str)->Result<bool> {
    let parent:Option<String>=store.db.query_row(
        "SELECT ticket_id FROM sa_work_items WHERE child_ticket_id=?1",
        [child_ticket_id],|r|r.get(0),
    ).optional().map_err(|e|e.to_string())?;
    let Some(parent)=parent else {
        // A dynamic subagent may have another WorkItem child as its parent.
        // Its entire immutable ancestry must remain authorized.
        return superagent::subagents::lineage_active(store,child_ticket_id);
    };
    let permitted:bool=store.db.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sa_demand_plans plan
           JOIN sa_tickets root ON root.id=plan.ticket_id
           JOIN sa_mandates grant ON grant.ticket_id=root.id
           JOIN sa_orchestrations team ON team.id=root.orchestration_id
           WHERE root.id=?1 AND root.state='queued' AND root.active_assignment_id IS NULL
             AND plan.state='active' AND root.policy_revision=plan.policy_revision
             AND team.enabled=1 AND team.policy_revision=plan.policy_revision
             AND team.project_id=root.project_id
             AND grant.revoked=0 AND grant.expires_at>?2
             AND grant.policy_revision=plan.policy_revision
             AND grant.project_id=root.project_id AND grant.actor=root.actor
             AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0)",
        params![parent,storage::now()],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    Ok(permitted && superagent::auto::ticket_authorized(store,&parent)?
        && !crate::feature_controls::get().bots_paused)
}

/// Capture active descendant native provider sessions BEFORE Stop clears
/// their active-assignment state. The caller cancels processes after the
/// shared SQLite Stop transaction has revoked their authority.
pub(crate) fn sessions_for_root(store:&Store,root:&str)->Result<Vec<String>> {
    let mut statement=store.db.prepare(
        "SELECT assignment.session_id
         FROM sa_work_items item
         JOIN sa_assignments assignment ON assignment.ticket_id=item.child_ticket_id
         WHERE item.ticket_id=?1 AND assignment.state='running'
         ORDER BY assignment.created_at,assignment.id"
    ).map_err(|e|e.to_string())?;
    let sessions=statement.query_map([root],|r|r.get::<_,String>(0))
        .map_err(|e|e.to_string())?;
    let mut recorded=sessions.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    recorded.extend(superagent::subagents::descendant_sessions(store,root)?);
    recorded.sort();
    recorded.dedup();
    Ok(recorded)
}

/// Invoked within the original parent's Stop transaction. This revokes all
/// child provider/effect authority BEFORE a delayed callback can commit.
pub(crate) fn revoke_descendants(tx:&Connection,root:&str,now:i64)->Result<()> {
    let exists:bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_demand_plans WHERE ticket_id=?1)",
        [root],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    if !exists {return Ok(())}
    tx.execute(
        "UPDATE sa_demand_plans SET state='stopped',updated_at=?2
         WHERE ticket_id=?1 AND state NOT IN ('stopped','completed')",
        params![root,now],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        "UPDATE sa_work_items SET state='blocked',last_reason='Parent Demand stopped',
         updated_at=?2 WHERE ticket_id=?1 AND state<>'completed'",
        params![root,now],
    ).map_err(|e|e.to_string())?;
    let members="(SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1
                   AND child_ticket_id IS NOT NULL)";
    let sql = format!("UPDATE sa_mandates SET revoked=1 WHERE ticket_id IN {members}");
    tx.execute(&sql,[root]).map_err(|e|e.to_string())?;
    for sql in [
        format!("UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2 WHERE state='pending' AND ticket_id IN {members}"),
        format!("UPDATE sa_assignments SET state='stopped',updated_at=?2 WHERE state='running' AND ticket_id IN {members}"),
        format!("UPDATE sa_council_requests SET state='blocked',updated_at=?2 WHERE state IN ('admitted','bound','starting','dispatched') AND ticket_id IN {members}"),
        format!("UPDATE sa_tickets SET state='stopped',active_assignment_id=NULL,revision=revision+1,updated_at=?2
                  WHERE id IN {members} AND state IN ('queued','running','waiting_review','waiting_approval','blocked')"),
    ] {
        tx.execute(&sql,params![root,now]).map_err(|e|e.to_string())?;
    }
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         SELECT child_ticket_id,'parent_stopped','Parent Demand revoked',?2
         FROM sa_work_items WHERE ticket_id=?1 AND child_ticket_id IS NOT NULL",
        params![root,now],
    ).map_err(|e|e.to_string())?;
    // All dynamically created descendants are revoked inside the same root
    // transaction; no child/grandchild may continue after Stop.
    superagent::subagents::revoke_descendants(tx,root,now)?;
    Ok(())
}

/// Recheck both native and external unsettled effects inside the same SQLite
/// transaction that transitions a Demand to completed. Earlier WorkItem
/// approvals cannot launder an uncertain write after a process restart.
fn unsettled_root_effects(db:&Connection,root:&str)->Result<bool> {
    db.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sa_effect_receipts effect
           WHERE effect.state IN ('pending','uncertain')
             AND (effect.ticket_id=?1 OR effect.ticket_id IN (
               SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1)
               OR effect.ticket_id IN (SELECT child_ticket_id FROM sa_subagent_nodes
                 WHERE root_ticket_id=?1))
           UNION ALL
           SELECT 1 FROM sa_worker_write_receipts receipt
           WHERE receipt.state IN ('reserved','uncertain')
             AND (receipt.ticket_id=?1 OR receipt.ticket_id IN (
               SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1)
               OR receipt.ticket_id IN (SELECT child_ticket_id FROM sa_subagent_nodes
                 WHERE root_ticket_id=?1)))",
        [root],|row|row.get(0),
    ).map_err(|error|error.to_string())
}

fn reconcile(store:&Store)->Result<()> {
    let now=storage::now();
    // An explicitly retried child resumes its original WorkItem and all
    // immutable root grants. A blocked WorkItem may not stay permanently
    // blocked after its original native child gets a new authorized attempt.
    store.db.execute(
        "UPDATE sa_work_items SET state='running',last_reason=NULL,updated_at=?1
         WHERE state='blocked' AND child_ticket_id IN
           (SELECT id FROM sa_tickets WHERE state='running')",[now],
    ).map_err(|e|e.to_string())?;
    // Do not accept model-reported completion or a mutable Ticket state.
    // Persist the exact independent human/Council decision first, under the
    // existing native effect/dispatch fence.
    evidence::record_accepted(store,now)?;
    store.db.execute(
        "UPDATE sa_work_items SET state='waiting_review',updated_at=?1
         WHERE state IN ('running','blocked') AND child_ticket_id IN
         (SELECT id FROM sa_tickets WHERE state IN ('waiting_approval','waiting_review'))",[now],
    ).map_err(|e|e.to_string())?;
    store.db.execute(
        "UPDATE sa_work_items SET state='blocked',last_reason='Child Ticket needs recovery',updated_at=?1
         WHERE state IN ('running','waiting_review') AND child_ticket_id IN
         (SELECT id FROM sa_tickets WHERE state IN ('failed','blocked','stopped','interrupted','inconclusive'))",[now],
    ).map_err(|e|e.to_string())?;
    // Historical completed child Tickets without proof cannot become an
    // implicitly verified dependency during a restart.
    store.db.execute(
        "UPDATE sa_work_items SET state='waiting_review',
         last_reason='Native child approval receipt unavailable',updated_at=?1
         WHERE state IN ('running','waiting_review') AND child_ticket_id IN
           (SELECT id FROM sa_tickets WHERE state='completed')
           AND NOT EXISTS(SELECT 1 FROM sa_work_item_evidence_receipts receipt
             WHERE receipt.item_id=sa_work_items.id)",[now],
    ).map_err(|e|e.to_string())?;
    // A local user authorized the full Demand once. All child Tickets
    // already passed their own native human/Council/effect gates, so no
    // additional per-step "continue" prompt is necessary for final state.
    // Never call this "completed" if even one WorkItem lacks native proof.
    let roots={
        let mut query=store.db.prepare(
            "SELECT plan.ticket_id FROM sa_demand_plans plan
             JOIN sa_tickets root ON root.id=plan.ticket_id
             JOIN sa_mandates grant ON grant.ticket_id=root.id
             JOIN sa_orchestrations team ON team.id=root.orchestration_id
             WHERE plan.state='active'
               AND root.state='queued' AND root.active_assignment_id IS NULL
               AND root.policy_revision=plan.policy_revision
               AND root.revision=plan.ticket_revision
               AND grant.revoked=0 AND grant.expires_at>?1
               AND grant.policy_revision=plan.policy_revision
               AND grant.project_id=root.project_id AND grant.actor=root.actor
               AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0
               AND team.enabled=1 AND team.policy_revision=plan.policy_revision
               AND team.project_id=root.project_id
               AND EXISTS(SELECT 1 FROM sa_work_items WHERE ticket_id=plan.ticket_id)
               AND NOT EXISTS(
                 SELECT 1 FROM sa_work_items item
                 LEFT JOIN sa_tickets child ON child.id=item.child_ticket_id
                 WHERE item.ticket_id=plan.ticket_id
                   AND (item.state<>'completed' OR child.state IS NULL
                        OR child.state<>'completed'
                        OR NOT EXISTS(SELECT 1 FROM sa_work_item_evidence_receipts evidence
                          WHERE evidence.item_id=item.id
                            AND evidence.child_ticket_id=item.child_ticket_id)
                        OR (item.requires_write=1 AND NOT EXISTS(
                          SELECT 1 FROM sa_work_item_evidence_receipts proof
                          JOIN sa_worker_write_receipts applied
                            ON applied.ticket_id=proof.child_ticket_id
                           AND applied.assignment_id=proof.assignment_id
                          WHERE proof.item_id=item.id
                            AND proof.child_ticket_id=item.child_ticket_id
                            AND applied.state='applied'))))
               AND NOT EXISTS(
                 SELECT 1 FROM sa_subagent_nodes descendant
                 WHERE descendant.root_ticket_id=plan.ticket_id
                   AND descendant.state<>'completed')
               AND NOT EXISTS(
                 SELECT 1 FROM sa_effect_receipts effect
                 WHERE effect.state IN ('pending','uncertain')
                   AND (effect.ticket_id=plan.ticket_id
                        OR effect.ticket_id IN (
                           SELECT child_ticket_id FROM sa_work_items
                           WHERE ticket_id=plan.ticket_id)))"
        ).map_err(|e|e.to_string())?;
        let rows=query.query_map([now],|row|row.get::<_,String>(0))
            .map_err(|e|e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e|e.to_string())?
    };
    for root in roots {
        if !superagent::auto::ticket_authorized(store,&root)? {
            continue;
        }
        let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
        // A prior, independently verified WorkItem may have acquired an
        // uncertain file or GitHub receipt since its approval. Do not finalize.
        if unsettled_root_effects(&tx,&root)?
            || !root_work_item_proofs_current(&tx,&root)? {
            continue;
        }
        let moved=tx.execute(
            "UPDATE sa_tickets SET state='completed',revision=revision+1,updated_at=?2
             WHERE id=?1 AND state='queued'
               AND revision=(SELECT ticket_revision FROM sa_demand_plans WHERE ticket_id=?1)
               AND EXISTS (
                 SELECT 1 FROM sa_mandates grant
                 JOIN sa_orchestrations team ON team.id=sa_tickets.orchestration_id
                 JOIN sa_demand_plans plan ON plan.ticket_id=sa_tickets.id
                 WHERE grant.ticket_id=sa_tickets.id AND grant.revoked=0
                   AND grant.expires_at>?2
                   AND grant.policy_revision=plan.policy_revision
                   AND grant.project_id=sa_tickets.project_id
                   AND grant.actor=sa_tickets.actor
                   AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0
                   AND team.enabled=1 AND team.policy_revision=plan.policy_revision
                   AND team.project_id=sa_tickets.project_id
               )
               AND EXISTS(SELECT 1 FROM sa_work_items item WHERE item.ticket_id=?1)
             AND NOT EXISTS(
                 SELECT 1 FROM sa_subagent_nodes descendant
                 WHERE descendant.root_ticket_id=?1 AND descendant.state<>'completed')
             AND NOT EXISTS(
                 SELECT 1 FROM sa_work_items item
                 LEFT JOIN sa_tickets child ON child.id=item.child_ticket_id
                 WHERE item.ticket_id=?1
                   AND (item.state<>'completed' OR child.state IS NULL
                        OR child.state<>'completed'
                        OR NOT EXISTS(SELECT 1 FROM sa_work_item_evidence_receipts evidence
                          WHERE evidence.item_id=item.id
                            AND evidence.child_ticket_id=item.child_ticket_id)
                        OR (item.requires_write=1 AND NOT EXISTS(
                          SELECT 1 FROM sa_work_item_evidence_receipts proof
                          JOIN sa_worker_write_receipts applied
                            ON applied.ticket_id=proof.child_ticket_id
                           AND applied.assignment_id=proof.assignment_id
                          WHERE proof.item_id=item.id
                            AND proof.child_ticket_id=item.child_ticket_id
                            AND applied.state='applied'))))",
            params![root,now],
        ).map_err(|e|e.to_string())?;
        if moved==1 {
            tx.execute(
                "UPDATE sa_demand_plans SET state='completed',updated_at=?2
                 WHERE ticket_id=?1 AND state='active'",
                params![root,now],
            ).map_err(|e|e.to_string())?;
            tx.execute(
                "INSERT INTO sa_audit(ticket_id,kind,detail,at)
                 VALUES(?1,'demand_completed',
                   'All admitted WorkItem Tickets finished under native review and effect gates',?2)",
                params![root,now],
            ).map_err(|e|e.to_string())?;
        }
        tx.commit().map_err(|e|e.to_string())?;
    }
    // One blocked item never freezes independent safe work. Only its
    // dependants wait; the plan remains active for other ready items.
    Ok(())
}

/// A saved approval is not proof that the *latest* native Worker attempt
/// still completed. A child can be interrupted or retried after an earlier
/// approval; neither its dependencies nor its parent Demand may reuse that
/// immutable receipt for a different attempt.
fn approved_child_attempt_current(db:&Connection,child_ticket_id:&str)->Result<bool> {
    db.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sa_work_item_evidence_receipts proof
           JOIN sa_tickets child ON child.id=proof.child_ticket_id
           JOIN sa_assignments latest ON latest.id=proof.assignment_id
           WHERE proof.child_ticket_id=?1 AND child.state='completed'
             AND latest.ticket_id=child.id AND latest.state='finished'
             AND latest.result=child.evidence
             AND latest.attempt=(SELECT MAX(a.attempt) FROM sa_assignments a
               WHERE a.ticket_id=child.id)
             AND proof.reviewed_at>=latest.updated_at)",
        [child_ticket_id],|row|row.get(0),
    ).map_err(|error|error.to_string())
}

/// Re-evaluate all native child approvals inside the Demand completion
/// transaction; the initial frontier query alone is not a finalization gate.
fn root_work_item_proofs_current(db:&Connection,root:&str)->Result<bool> {
    let mut query=db.prepare(
        "SELECT child_ticket_id FROM sa_work_items WHERE ticket_id=?1"
    ).map_err(|error|error.to_string())?;
    let children=query.query_map([root],|row|row.get::<_,Option<String>>(0))
        .map_err(|error|error.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error|error.to_string())?;
    for child in children {
        let Some(child)=child else {return Ok(false)};
        if !approved_child_attempt_current(db,&child)? {return Ok(false)}
    }
    Ok(true)
}

/// Recheck every predecessor at native child admission. An earlier approval
/// cannot release new dependent work while descendant effects remain uncertain.
fn prerequisites_settled(db:&Connection,root:&str,key:&str)->Result<bool> {
    let mut statement=db.prepare(
        "SELECT prior.child_ticket_id,
           COALESCE(prior.state='completed'
             AND EXISTS(SELECT 1 FROM sa_work_item_evidence_receipts proof
               WHERE proof.item_id=prior.id AND proof.child_ticket_id=prior.child_ticket_id)
             AND (prior.requires_write=0 OR EXISTS(
               SELECT 1 FROM sa_work_item_evidence_receipts proof
               JOIN sa_worker_write_receipts applied
                 ON applied.ticket_id=proof.child_ticket_id
                AND applied.assignment_id=proof.assignment_id
               WHERE proof.item_id=prior.id
                 AND proof.child_ticket_id=prior.child_ticket_id
                 AND applied.state='applied')),0)
         FROM sa_work_dependencies dependency
         LEFT JOIN sa_work_items prior
           ON prior.ticket_id=dependency.ticket_id
          AND prior.item_key=dependency.depends_on
         WHERE dependency.ticket_id=?1 AND dependency.item_key=?2"
    ).map_err(|error|error.to_string())?;
    let rows=statement.query_map(params![root,key],|row|Ok((
        row.get::<_,Option<String>>(0)?,row.get::<_,bool>(1)?,
    ))).map_err(|error|error.to_string())?;
    let predecessors=rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error|error.to_string())?;
    for (child,verified) in predecessors {
        let Some(child)=child else {return Ok(false)};
        if !verified || !approved_child_attempt_current(db,&child)?
            || !evidence::descendant_receipts_settled(db,&child)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn ready(store:&Store)->Result<Vec<ReadyItem>> {
    let mut query=store.db.prepare(
        "SELECT item.id,item.ticket_id,item.item_key,item.objective,item.worker_bot_id,item.requires_write
         FROM sa_work_items item
         JOIN sa_demand_plans plan ON plan.ticket_id=item.ticket_id
         JOIN sa_tickets root ON root.id=plan.ticket_id
         JOIN sa_mandates grant ON grant.ticket_id=root.id
         JOIN sa_orchestrations team ON team.id=root.orchestration_id
         WHERE item.state='pending' AND item.child_ticket_id IS NULL
           AND plan.state='active' AND root.state='queued'
           AND root.active_assignment_id IS NULL
           AND root.policy_revision=plan.policy_revision
           AND team.enabled=1 AND team.policy_revision=plan.policy_revision
           AND team.project_id=root.project_id
           AND grant.revoked=0 AND grant.expires_at>?1
           AND grant.policy_revision=plan.policy_revision
           AND grant.project_id=root.project_id AND grant.actor=root.actor
           AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0
           AND (item.requires_write=0 OR EXISTS(
               SELECT 1 FROM sa_work_item_write_scopes scope
               WHERE scope.item_id=item.id AND scope.root_ticket_revision=plan.ticket_revision))
           AND NOT EXISTS(
             SELECT 1 FROM sa_work_dependencies edge
             JOIN sa_work_items prerequisite
               ON prerequisite.ticket_id=edge.ticket_id
              AND prerequisite.item_key=edge.depends_on
             WHERE edge.ticket_id=item.ticket_id
               AND edge.item_key=item.item_key
               AND (prerequisite.state<>'completed'
                  OR NOT EXISTS(SELECT 1 FROM sa_work_item_evidence_receipts evidence
                      WHERE evidence.item_id=prerequisite.id
                        AND evidence.child_ticket_id=prerequisite.child_ticket_id)
                  OR (prerequisite.requires_write=1 AND NOT EXISTS(
                    SELECT 1 FROM sa_work_item_evidence_receipts proof
                    JOIN sa_worker_write_receipts applied
                      ON applied.ticket_id=proof.child_ticket_id
                     AND applied.assignment_id=proof.assignment_id
                    WHERE proof.item_id=prerequisite.id
                      AND proof.child_ticket_id=prerequisite.child_ticket_id
                      AND applied.state='applied'))))
         ORDER BY plan.created_at,item.ordinal,item.id"
    ).map_err(|e|e.to_string())?;
    // Do not limit candidates before native descendant/effect validation:
    // 32 blocked predecessors must not starve another Demand indefinitely.
    // Rows are streamed and at most eight eligible items are materialized.
    let rows=query.query_map([storage::now()],|r|Ok(ReadyItem {
        id:r.get(0)?,root:r.get(1)?,key:r.get(2)?,
        objective:r.get(3)?,worker:r.get(4)?,requires_write:r.get(5)?,
    })).map_err(|e|e.to_string())?;
    let mut verified=Vec::new();
    for row in rows {
        let item=row.map_err(|e|e.to_string())?;
        if prerequisites_settled(&store.db,&item.root,&item.key)? {
            verified.push(item);
            if verified.len()==8 { break; }
        }
    }
    Ok(verified)
}

pub(crate) async fn advance(app:&tauri::AppHandle)->Result<usize> {
    if crate::feature_controls::get().bots_paused {return Ok(0)}
    let candidates={
        let state=app.state::<AppState>();
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let _fence=superagent::effects::lock_dispatch(&store.path)?;
        reconcile(&store)?;
        ready(&store)?
    };
    let mut progressed=0;
    for item in candidates.into_iter().take(8) {
        let claimed={
            let state=app.state::<AppState>();
            let agents=state.agents.lock().map_err(|e|e.to_string())?;
            let active=agents.keys().cloned().collect::<Vec<_>>();
            let mut store=state.store.lock().map_err(|e|e.to_string())?;
            let _fence=superagent::effects::lock_dispatch(&store.path)?;
            let pending:bool=store.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM sa_work_items work
                  WHERE work.id=?1 AND work.ticket_id=?2
                    AND work.state='pending' AND work.child_ticket_id IS NULL
                    AND (work.requires_write=0 OR EXISTS(
                      SELECT 1 FROM sa_work_item_write_scopes scope
                      JOIN sa_demand_plans plan ON plan.ticket_id=work.ticket_id
                      WHERE scope.item_id=work.id AND scope.root_ticket_revision=plan.ticket_revision)))",
                params![item.id,item.root],|r|r.get(0),
            ).map_err(|e|e.to_string())?;
            if !pending || !prerequisites_settled(&store.db,&item.root,&item.key)? {
                None
            } else {
                let root=get_ticket(&store,&item.root)?;
                let snapshot=ticket_team_snapshot(&store,&root.id)?;
                let team=configuration(&store,&root.orchestration_id)?;
                let root_ok:bool=store.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sa_demand_plans plan
                      JOIN sa_mandates grant ON grant.ticket_id=plan.ticket_id
                      JOIN sa_tickets original ON original.id=plan.ticket_id
                      WHERE plan.ticket_id=?1 AND plan.state='active'
                      AND original.state='queued' AND original.active_assignment_id IS NULL
                      AND original.policy_revision=?3 AND original.project_id=?4
                      AND grant.revoked=0 AND grant.expires_at>?2
                      AND grant.policy_revision=?3 AND grant.project_id=?4
                      AND grant.actor=original.actor
                      AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0)",
                    params![root.id,storage::now(),snapshot.policy.policy_revision,snapshot.project_id],
                    |r|r.get(0),
                ).map_err(|e|e.to_string())?;
                // A queued WorkItem cannot inherit a later bot model, provider,
                // project or Skills change. Recheck the original Team profile
                // before creating the child Ticket and native Assignment.
                if superagent::roster::require_current_profile(
                    &store,&root.id,"worker",&item.worker,
                ).is_err() {
                    store.db.execute(
                        "UPDATE sa_work_items SET state='blocked',
                         last_reason='Worker profile changed after Demand admission',
                         updated_at=?2 WHERE id=?1 AND state='pending'
                         AND child_ticket_id IS NULL",
                        params![item.id,storage::now()],
                    ).map_err(|e|e.to_string())?;
                    None
                } else if root.state!="queued" || !team.enabled || !root_ok ||
                    !superagent::auto::ticket_authorized(&store,&root.id)? ||
                    team.policy_revision!=snapshot.policy.policy_revision ||
                    team.team_version!=snapshot.team_version ||
                    !snapshot.worker_bot_ids.contains(&item.worker) ||
                    superagent::active_manual_conflict(
                        &store,&active,&item.worker,&snapshot.project_id,
                    )? {None}
                else {
                    let paths:Option<Vec<String>>=if item.requires_write {
                        let json:String=store.db.query_row(
                            "SELECT scope.allowed_paths FROM sa_work_item_write_scopes scope
                             JOIN sa_demand_plans plan ON plan.ticket_id=?2
                             WHERE scope.item_id=?1 AND scope.root_ticket_revision=plan.ticket_revision",
                            params![item.id,item.root],|row|row.get(0),
                        ).map_err(|_|"WorkItem requires an unexpired native file grant.")?;
                        Some(serde_json::from_str::<Vec<String>>(&json)
                            .map_err(|_|"WorkItem file scope is invalid.")?)
                    } else {None};
                    let request_key=format!("work.{}",item.id);
                    let (child,launch)=superagent::admit_role_with_work_item(
                        &mut store,&root.orchestration_id,&item.objective,
                        &request_key,&item.worker,paths.as_deref(),&item.root,&item.id,
                    )?;
                    // The child ID, native lease and immutable WorkItem
                    // association are already committed as one transaction.
                    if launch {Some((child,paths))} else {None}
                }
            }
        };
        let Some((child,paths))=claimed else {continue};
        let allowed={
            let state=app.state::<AppState>();
            let store=state.store.lock().map_err(|e|e.to_string())?;
            parent_active(&store,&child.id)?
        };
        if !allowed {continue}
        let worker_instructions=if let Some(ref paths)=paths {
            format!("The local user confirmed EXACT file paths for the native {} tool:\n{}.\n\
                Other paths, provider-native shell/files and external MCP/GitHub effects remain denied.\n\
                Do not treat model instructions as additional grants.",
                superagent::write_grants::TOOL_NAME,paths.join("\n"))
        } else {
            "Read-only authorized scope. No file, shell, MCP or GitHub write effects are permitted.".into()
        };
        let result=bots::gateway::deliver(app,bots::gateway::Inbound {
            bot_id:item.worker.clone(),
            channel_id:superagent::CHANNEL.into(),
            thread:child.id.clone(),
            sender_id:"local-user".into(),
            sender_name:"Authorized Demand scheduler".into(),
            text:format!(
                "SuperAgent Demand {} / WorkItem {}\n\n{}\n\n\
                 {}\nReport concrete test evidence; native review governs completion.",
                item.root,item.key,item.objective,worker_instructions,
            ),
            trusted:true,image_ids:None,moodboard_ids:None,
        }).await;
        if let Err(error)=result {
            let state=app.state::<AppState>();
            let locked=state.store.lock();
            if let Ok(store)=locked {
                let _=superagent::fail_launch(&store,&child.id,&error);
                let _=store.db.execute(
                    "UPDATE sa_work_items SET state='blocked',
                      last_reason='Native Worker delivery failed',updated_at=?2
                     WHERE id=?1 AND state='running'",
                    params![item.id,storage::now()],
                );
            }
        } else {progressed+=1;}
    }
    // The same native scheduler advances worker-created descendants only
    // after their parent released the protected checkout/Assignment.
    let nested=superagent::subagents::advance(app).await?;
    Ok(progressed+nested)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsettled_frontier_candidates_cannot_starve_an_independent_demand() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_work_items(id TEXT,ticket_id TEXT,item_key TEXT,
                 ordinal INTEGER,objective TEXT,worker_bot_id TEXT,requires_write INTEGER,
                 state TEXT,child_ticket_id TEXT);
             CREATE TABLE sa_work_dependencies(ticket_id TEXT,item_key TEXT,depends_on TEXT);
             CREATE TABLE sa_demand_plans(ticket_id TEXT,state TEXT,created_at INTEGER,
                 policy_revision INTEGER,ticket_revision INTEGER);
             CREATE TABLE sa_tickets(id TEXT,orchestration_id TEXT,state TEXT,
                 active_assignment_id TEXT,policy_revision INTEGER,project_id TEXT,actor TEXT,
                 evidence TEXT);
             CREATE TABLE sa_assignments(id TEXT,ticket_id TEXT,state TEXT,
                 attempt INTEGER,result TEXT,updated_at INTEGER);
             CREATE TABLE sa_mandates(ticket_id TEXT,revoked INTEGER,expires_at INTEGER,
                 policy_revision INTEGER,project_id TEXT,actor TEXT,
                 allowed_tools TEXT,allowed_side_effects INTEGER);
             CREATE TABLE sa_orchestrations(id TEXT,enabled INTEGER,
                 policy_revision INTEGER,project_id TEXT);
             CREATE TABLE sa_work_item_evidence_receipts(
                 item_id TEXT,child_ticket_id TEXT,assignment_id TEXT,reviewed_at INTEGER);
             CREATE TABLE sa_worker_write_receipts(
                 ticket_id TEXT,state TEXT,assignment_id TEXT);
             CREATE TABLE sa_subagent_nodes(
                 parent_ticket_id TEXT,child_ticket_id TEXT,state TEXT);
             CREATE TABLE sa_effect_receipts(ticket_id TEXT,state TEXT);
             CREATE TABLE sa_work_item_write_scopes(item_id TEXT,root_ticket_revision INTEGER);
             INSERT INTO sa_orchestrations VALUES('team',1,1,'project');"
        ).unwrap();
        let expiry=storage::now().saturating_add(1_000_000);
        // Two earlier authorized Demands have 32 SQL-visible candidates,
        // but their completed predecessor has an unresolved native effect.
        // A later unrelated Demand must still reach the ready frontier.
        for (index,count) in [(0,31),(1,1),(2,1)] {
            let root=format!("root-{index}");
            db.execute(
                "INSERT INTO sa_tickets VALUES(?1,'team','queued',NULL,1,'project','local-user',NULL)",
                [&root],
            ).unwrap();
            db.execute(
                "INSERT INTO sa_mandates VALUES(?1,0,?2,1,'project',
                    'local-user','[]',0)",
                params![&root,expiry],
            ).unwrap();
            db.execute(
                "INSERT INTO sa_demand_plans VALUES(?1,'active',?2,1,1)",
                params![&root,index],
            ).unwrap();
            if index<2 {
                let build=format!("build-{index}");
                let child=format!("child-{index}");
                db.execute(
                    "INSERT INTO sa_work_items VALUES(
                        ?1,?2,'build',0,'build','worker',0,'completed',?3)",
                    params![&build,&root,&child],
                ).unwrap();
                let attempt=format!("attempt-{index}");
                db.execute(
                    "INSERT INTO sa_tickets VALUES(?1,'team','completed',NULL,1,
                        'project','local-user','native output')",
                    [&child],
                ).unwrap();
                db.execute(
                    "INSERT INTO sa_assignments VALUES(?1,?2,'finished',1,'native output',50)",
                    params![&attempt,&child],
                ).unwrap();
                db.execute(
                    "INSERT INTO sa_work_item_evidence_receipts VALUES(?1,?2,?3,60)",
                    params![&build,&child,&attempt],
                ).unwrap();
                db.execute(
                    "INSERT INTO sa_effect_receipts VALUES(?1,'uncertain')",
                    [&child],
                ).unwrap();
            }
            for position in 0..count {
                let key=format!("next-{position}");
                db.execute(
                    "INSERT INTO sa_work_items VALUES(?1,?2,?3,?4,
                        'work','worker',0,'pending',NULL)",
                    params![format!("pending-{index}-{position}"),&root,&key,
                        i64::from(position)+1],
                ).unwrap();
                if index<2 {
                    db.execute(
                        "INSERT INTO sa_work_dependencies VALUES(?1,?2,'build')",
                        params![&root,&key],
                    ).unwrap();
                }
            }
        }
        let store=Store{db,path:std::path::PathBuf::from(":memory:")};
        let candidates=ready(&store).unwrap();
        assert_eq!(candidates.len(),1);
        assert_eq!(candidates[0].root,"root-2");
        assert_eq!(candidates[0].key,"next-0");
    }


    #[test]
    fn dependent_worker_waits_for_predecessor_and_nested_effect_reconciliation() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_work_dependencies(ticket_id TEXT,item_key TEXT,depends_on TEXT);
             CREATE TABLE sa_work_items(id TEXT,ticket_id TEXT,item_key TEXT,
               child_ticket_id TEXT,state TEXT,requires_write INTEGER);
             CREATE TABLE sa_work_item_evidence_receipts(
               item_id TEXT,child_ticket_id TEXT,assignment_id TEXT,reviewed_at INTEGER);
             CREATE TABLE sa_tickets(id TEXT,state TEXT,evidence TEXT);
             CREATE TABLE sa_assignments(id TEXT,ticket_id TEXT,state TEXT,
               attempt INTEGER,result TEXT,updated_at INTEGER);
             CREATE TABLE sa_worker_write_receipts(
               ticket_id TEXT,state TEXT,assignment_id TEXT);
             CREATE TABLE sa_subagent_nodes(
               parent_ticket_id TEXT,child_ticket_id TEXT,state TEXT);
             CREATE TABLE sa_effect_receipts(ticket_id TEXT,state TEXT);
             INSERT INTO sa_work_items VALUES('build','root','build','child-build','completed',1);
             INSERT INTO sa_work_items VALUES('verify','root','verify',NULL,'pending',0);
             INSERT INTO sa_work_dependencies VALUES('root','verify','build');
             INSERT INTO sa_tickets VALUES('child-build','completed','native output');
             INSERT INTO sa_assignments
               VALUES('final-attempt','child-build','finished',1,'native output',50);
             INSERT INTO sa_work_item_evidence_receipts
               VALUES('build','child-build','final-attempt',60);
             INSERT INTO sa_worker_write_receipts
               VALUES('child-build','applied','final-attempt');"
        ).unwrap();
        assert!(prerequisites_settled(&db,"root","verify").unwrap());
        db.execute(
            "INSERT INTO sa_effect_receipts VALUES('child-build','uncertain')",[],
        ).unwrap();
        assert!(!prerequisites_settled(&db,"root","verify").unwrap());
        db.execute(
            "UPDATE sa_effect_receipts SET state='applied'
             WHERE ticket_id='child-build'",[],
        ).unwrap();
        assert!(prerequisites_settled(&db,"root","verify").unwrap());
        db.execute_batch(
            "INSERT INTO sa_subagent_nodes VALUES('child-build','grandchild','completed');
             INSERT INTO sa_worker_write_receipts VALUES('grandchild','reserved','nested-attempt');"
        ).unwrap();
        assert!(!prerequisites_settled(&db,"root","verify").unwrap());
        db.execute(
            "UPDATE sa_worker_write_receipts SET state='applied'
             WHERE ticket_id='grandchild'",[],
        ).unwrap();
        assert!(prerequisites_settled(&db,"root","verify").unwrap());
        db.execute(
            "INSERT INTO sa_effect_receipts VALUES('unrelated','pending')",[],
        ).unwrap();
        assert!(prerequisites_settled(&db,"root","verify").unwrap());
        db.execute(
            "UPDATE sa_worker_write_receipts SET state='failed'
             WHERE ticket_id='child-build'",[],
        ).unwrap();
        assert!(!prerequisites_settled(&db,"root","verify").unwrap());
        db.execute(
            "UPDATE sa_worker_write_receipts SET state='applied'
             WHERE ticket_id='child-build'",[],
        ).unwrap();
        assert!(prerequisites_settled(&db,"root","verify").unwrap());
        db.execute(
            "UPDATE sa_tickets SET state='running' WHERE id='child-build'",[],
        ).unwrap();
        assert!(!prerequisites_settled(&db,"root","verify").unwrap());
        db.execute(
            "UPDATE sa_tickets SET state='completed' WHERE id='child-build'",[],
        ).unwrap();
        db.execute(
            "INSERT INTO sa_assignments
             VALUES('later-attempt','child-build','running',2,NULL,75)",[],
        ).unwrap();
        assert!(!prerequisites_settled(&db,"root","verify").unwrap());
    }

    #[test]
    fn root_completion_refuses_a_stale_or_retried_child_approval() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_work_items(ticket_id TEXT,child_ticket_id TEXT);
             CREATE TABLE sa_work_item_evidence_receipts(
                 item_id TEXT,child_ticket_id TEXT,assignment_id TEXT,reviewed_at INTEGER);
             CREATE TABLE sa_tickets(id TEXT,state TEXT,evidence TEXT);
             CREATE TABLE sa_assignments(id TEXT,ticket_id TEXT,state TEXT,
                 attempt INTEGER,result TEXT,updated_at INTEGER);
             INSERT INTO sa_work_items VALUES('root','child');
             INSERT INTO sa_tickets VALUES('child','completed','verified output');
             INSERT INTO sa_assignments
               VALUES('first','child','finished',1,'verified output',50);
             INSERT INTO sa_work_item_evidence_receipts
               VALUES('item','child','first',60);",
        ).unwrap();
        assert!(root_work_item_proofs_current(&db,"root").unwrap());
        db.execute(
            "UPDATE sa_tickets SET state='running' WHERE id='child'",[],
        ).unwrap();
        assert!(!root_work_item_proofs_current(&db,"root").unwrap());
        db.execute(
            "UPDATE sa_tickets SET state='completed' WHERE id='child'",[],
        ).unwrap();
        db.execute(
            "INSERT INTO sa_assignments
             VALUES('second','child','running',2,NULL,70)",[],
        ).unwrap();
        assert!(!root_work_item_proofs_current(&db,"root").unwrap());
        db.execute(
            "DELETE FROM sa_assignments WHERE id='second'",[],
        ).unwrap();
        assert!(root_work_item_proofs_current(&db,"root").unwrap());
        db.execute(
            "UPDATE sa_assignments SET updated_at=70 WHERE id='first'",[],
        ).unwrap();
        assert!(!root_work_item_proofs_current(&db,"root").unwrap());
        db.execute(
            "UPDATE sa_work_item_evidence_receipts SET reviewed_at=80",[],
        ).unwrap();
        assert!(root_work_item_proofs_current(&db,"root").unwrap());
        db.execute(
            "INSERT INTO sa_work_items VALUES('root','missing-child')",[],
        ).unwrap();
        assert!(!root_work_item_proofs_current(&db,"root").unwrap());
    }

    #[test]
    fn root_finalization_rechecks_scoped_writes_and_external_effects() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_work_items(ticket_id TEXT,child_ticket_id TEXT);
             CREATE TABLE sa_subagent_nodes(root_ticket_id TEXT,child_ticket_id TEXT);
             CREATE TABLE sa_effect_receipts(ticket_id TEXT,state TEXT);
             CREATE TABLE sa_worker_write_receipts(ticket_id TEXT,state TEXT);
             INSERT INTO sa_work_items VALUES('root','child-a');
             INSERT INTO sa_work_items VALUES('other','child-b');
             INSERT INTO sa_worker_write_receipts VALUES('unrelated','uncertain');",
        ).unwrap();
        assert!(!unsettled_root_effects(&db,"root").unwrap());
        db.execute(
            "INSERT INTO sa_worker_write_receipts VALUES('child-a','reserved')",[],
        ).unwrap();
        assert!(unsettled_root_effects(&db,"root").unwrap());
        assert!(!unsettled_root_effects(&db,"other").unwrap());
        db.execute(
            "UPDATE sa_worker_write_receipts SET state='applied'
             WHERE ticket_id='child-a'",[],
        ).unwrap();
        assert!(!unsettled_root_effects(&db,"root").unwrap());
        db.execute(
            "INSERT INTO sa_worker_write_receipts VALUES('root','uncertain')",[],
        ).unwrap();
        assert!(unsettled_root_effects(&db,"root").unwrap());
        db.execute(
            "UPDATE sa_worker_write_receipts SET state='failed'
             WHERE ticket_id='root'",[],
        ).unwrap();
        db.execute(
            "INSERT INTO sa_effect_receipts VALUES('child-a','pending')",[],
        ).unwrap();
        assert!(unsettled_root_effects(&db,"root").unwrap());
        db.execute(
            "UPDATE sa_effect_receipts SET state='applied'
             WHERE ticket_id='child-a'",[],
        ).unwrap();
        assert!(!unsettled_root_effects(&db,"root").unwrap());
    }

    #[test]
    fn root_finalization_waits_for_nested_worker_effect_receipts() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_work_items(ticket_id TEXT,child_ticket_id TEXT);
             CREATE TABLE sa_subagent_nodes(root_ticket_id TEXT,child_ticket_id TEXT);
             CREATE TABLE sa_effect_receipts(ticket_id TEXT,state TEXT);
             CREATE TABLE sa_worker_write_receipts(ticket_id TEXT,state TEXT);
             INSERT INTO sa_work_items VALUES('root','worker-a');
             INSERT INTO sa_work_items VALUES('other','worker-b');
             INSERT INTO sa_subagent_nodes VALUES('root','nested-child');
             INSERT INTO sa_subagent_nodes VALUES('root','nested-grandchild');
             INSERT INTO sa_subagent_nodes VALUES('other','other-nested');
             INSERT INTO sa_worker_write_receipts VALUES('nested-child','reserved');
             INSERT INTO sa_effect_receipts VALUES('nested-grandchild','uncertain');",
        ).unwrap();
        assert!(unsettled_root_effects(&db,"root").unwrap());
        assert!(!unsettled_root_effects(&db,"other").unwrap());
        db.execute(
            "UPDATE sa_worker_write_receipts SET state='applied'
             WHERE ticket_id='nested-child'",[],
        ).unwrap();
        assert!(unsettled_root_effects(&db,"root").unwrap());
        db.execute(
            "UPDATE sa_effect_receipts SET state='applied'
             WHERE ticket_id='nested-grandchild'",[],
        ).unwrap();
        assert!(!unsettled_root_effects(&db,"root").unwrap());
        db.execute(
            "INSERT INTO sa_effect_receipts VALUES('other-nested','pending')",[],
        ).unwrap();
        assert!(unsettled_root_effects(&db,"other").unwrap());
        assert!(!unsettled_root_effects(&db,"root").unwrap());
    }
}
