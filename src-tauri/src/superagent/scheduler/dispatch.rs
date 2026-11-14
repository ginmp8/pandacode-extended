//! Native dependency-aware WorkItem dispatch and parent revocation.

use super::*;
use crate::superagent::{self,get_ticket};
use rusqlite::{params,OptionalExtension,Connection};
use tauri::Manager;

#[derive(Debug,Clone)]
struct ReadyItem {
    id:String, root:String, key:String, objective:String, worker:String,
}

pub(crate) fn parent_active(store:&Store,child_ticket_id:&str)->Result<bool> {
    let parent:Option<String>=store.db.query_row(
        "SELECT ticket_id FROM sa_work_items WHERE child_ticket_id=?1",
        [child_ticket_id],|r|r.get(0),
    ).optional().map_err(|e|e.to_string())?;
    let Some(parent)=parent else{return Ok(true)};
    let permitted:bool=store.db.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sa_demand_plans plan
           JOIN sa_tickets root ON root.id=plan.ticket_id
           JOIN sa_mandates grant ON grant.ticket_id=root.id
           JOIN sa_orchestrations team ON team.id=root.orchestration_id
           WHERE root.id=?1 AND root.state='queued' AND root.active_assignment_id IS NULL
             AND plan.state='active' AND root.policy_revision=plan.policy_revision
             AND team.enabled=1 AND team.policy_revision=plan.policy_revision
             AND grant.revoked=0 AND grant.expires_at>?2
             AND grant.policy_revision=plan.policy_revision
             AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0)",
        params![parent,storage::now()],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    Ok(permitted && !crate::feature_controls::get().bots_paused)
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
    statement.query_map([root],|r|r.get::<_,String>(0))
        .map_err(|e|e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())
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
    for sql in [
        format!("UPDATE sa_mandates SET revoked=1 WHERE ticket_id IN {members}"),
    ] {
        tx.execute(&sql,[root]).map_err(|e|e.to_string())?;
    }
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
    Ok(())
}

fn reconcile(store:&Store)->Result<()> {
    let now=storage::now();
    // Only actual native Ticket completion satisfies dependency edges.
    store.db.execute(
        "UPDATE sa_work_items SET state='completed',last_reason=NULL,updated_at=?1
         WHERE state IN ('running','waiting_review') AND child_ticket_id IN
         (SELECT id FROM sa_tickets WHERE state='completed')",[now],
    ).map_err(|e|e.to_string())?;
    store.db.execute(
        "UPDATE sa_work_items SET state='waiting_review',updated_at=?1
         WHERE state='running' AND child_ticket_id IN
         (SELECT id FROM sa_tickets WHERE state IN ('waiting_approval','waiting_review'))",[now],
    ).map_err(|e|e.to_string())?;
    store.db.execute(
        "UPDATE sa_work_items SET state='blocked',last_reason='Child Ticket needs recovery',updated_at=?1
         WHERE state IN ('running','waiting_review') AND child_ticket_id IN
         (SELECT id FROM sa_tickets WHERE state IN ('failed','blocked','stopped','interrupted','inconclusive'))",[now],
    ).map_err(|e|e.to_string())?;
    store.db.execute(
        "UPDATE sa_demand_plans SET state='ready_for_review',updated_at=?1
         WHERE state='active' AND NOT EXISTS(
           SELECT 1 FROM sa_work_items WHERE ticket_id=sa_demand_plans.ticket_id
             AND state<>'completed')",[now],
    ).map_err(|e|e.to_string())?;
    // One blocked item never freezes independent safe work. Only its
    // dependants wait; the plan remains active for other ready items.
    Ok(())
}

fn ready(store:&Store)->Result<Vec<ReadyItem>> {
    let mut query=store.db.prepare(
        "SELECT item.id,item.ticket_id,item.item_key,item.objective,item.worker_bot_id
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
           AND grant.revoked=0 AND grant.expires_at>?1
           AND grant.policy_revision=plan.policy_revision
           AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0
           AND NOT EXISTS(
             SELECT 1 FROM sa_work_dependencies edge
             JOIN sa_work_items prerequisite
               ON prerequisite.ticket_id=edge.ticket_id
              AND prerequisite.item_key=edge.depends_on
             WHERE edge.ticket_id=item.ticket_id
               AND edge.item_key=item.item_key
               AND prerequisite.state<>'completed')
         ORDER BY plan.created_at,item.ordinal,item.id LIMIT 32"
    ).map_err(|e|e.to_string())?;
    query.query_map([storage::now()],|r|Ok(ReadyItem {
        id:r.get(0)?,root:r.get(1)?,key:r.get(2)?,
        objective:r.get(3)?,worker:r.get(4)?,
    })).map_err(|e|e.to_string())?
    .collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())
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
                "SELECT EXISTS(SELECT 1 FROM sa_work_items
                  WHERE id=?1 AND ticket_id=?2 AND state='pending' AND child_ticket_id IS NULL)",
                params![item.id,item.root],|r|r.get(0),
            ).map_err(|e|e.to_string())?;
            if !pending {None} else {
                let root=get_ticket(&store,&item.root)?;
                let snapshot=ticket_team_snapshot(&store,&root.id)?;
                let team=configuration(&store,&root.orchestration_id)?;
                let root_ok:bool=store.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sa_demand_plans plan
                      JOIN sa_mandates grant ON grant.ticket_id=plan.ticket_id
                      WHERE plan.ticket_id=?1 AND plan.state='active'
                      AND grant.revoked=0 AND grant.expires_at>?2
                      AND grant.policy_revision=?3 AND grant.project_id=?4
                      AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0)",
                    params![root.id,storage::now(),snapshot.policy.policy_revision,snapshot.project_id],
                    |r|r.get(0),
                ).map_err(|e|e.to_string())?;
                if root.state!="queued" || !team.enabled || !root_ok ||
                    team.policy_revision!=snapshot.policy.policy_revision ||
                    team.team_version!=snapshot.team_version ||
                    !snapshot.worker_bot_ids.contains(&item.worker) ||
                    superagent::active_manual_conflict(
                        &store,&active,&item.worker,&snapshot.project_id,
                    )? {None}
                else {
                    let request_key=format!("work.{}",item.id);
                    let (child,launch)=superagent::admit_role_with_selected(
                        &mut store,&root.orchestration_id,&item.objective,
                        &request_key,false,false,false,Some(&item.worker),
                    )?;
                    let changed=store.db.execute(
                        "UPDATE sa_work_items SET child_ticket_id=?2,state=?3,updated_at=?4
                         WHERE id=?1 AND state='pending' AND child_ticket_id IS NULL",
                        params![item.id,child.id,if launch {"running"} else {"blocked"},storage::now()],
                    ).map_err(|e|e.to_string())?;
                    if changed!=1 {return Err("WorkItem claim changed during native admission.".into())}
                    if launch {Some(child)} else {None}
                }
            }
        };
        let Some(child)=claimed else {continue};
        let allowed={
            let state=app.state::<AppState>();
            let store=state.store.lock().map_err(|e|e.to_string())?;
            parent_active(&store,&child.id)?
        };
        if !allowed {continue}
        let result=bots::gateway::deliver(app,bots::gateway::Inbound {
            bot_id:item.worker.clone(),
            channel_id:superagent::CHANNEL.into(),
            thread:child.id.clone(),
            sender_id:"local-user".into(),
            sender_name:"Authorized Demand scheduler".into(),
            text:format!(
                "SuperAgent Demand {} / WorkItem {}\n\n{}\n\n\
                 Read-only authorized scope only. Do not write files or execute shell/MCP \
                 side effects. Report evidence; native review governs completion.",
                item.root,item.key,item.objective,
            ),
            trusted:true,image_ids:None,moodboard_ids:None,
        }).await;
        if let Err(error)=result {
            let state=app.state::<AppState>();
            if let Ok(store)=state.store.lock(){
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
    Ok(progressed)
}
