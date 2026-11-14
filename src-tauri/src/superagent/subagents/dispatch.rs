//! Native deferred scheduling and descendant revocation. Subagent requests are
//! created by the admitted parent Worker; the provider session is never cloned
//! or given another Team's token, grant, checkout or reviewer authority.

use super::*;
use crate::superagent;
use rusqlite::Connection;

/// Descendant authority is anchored to the original Demand mandate, not
/// merely to its current Ticket state or the child's later-created grant.
/// Root expiration/revocation must fence every child and grandchild.
fn root_mandate_active(store:&Store,root:&str)->Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sa_tickets ticket
           JOIN sa_mandates grant ON grant.ticket_id=ticket.id
           JOIN sa_subagent_policies policy ON policy.root_ticket_id=ticket.id
           WHERE ticket.id=?1 AND ticket.state='queued'
             AND ticket.active_assignment_id IS NULL
             AND ticket.policy_revision=policy.policy_revision
             AND policy.revoked_at IS NULL
             AND grant.revoked=0 AND grant.expires_at>?2
             AND grant.policy_revision=policy.policy_revision
             AND grant.project_id=ticket.project_id
             AND grant.actor=ticket.actor
             AND grant.allowed_tools='[]' AND grant.allowed_side_effects=0)",
        params![root,storage::now()],|row|row.get(0),
    ).map_err(|e|e.to_string())
}

pub(crate) fn has_unsettled(store:&Store,parent_ticket_id:&str)->Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sa_subagent_nodes
            WHERE parent_ticket_id=?1 AND state<>'completed'
          )",
        [parent_ticket_id],|r|r.get(0),
    ).map_err(|e|e.to_string())
}

/// An active descendant must retain the root's original local-user mandate,
/// Team revision and the entire immediate parent lineage. This is called at
/// native provider/tool admission, not just at the scheduling tick.
pub(crate) fn lineage_active(store:&Store,child_ticket_id:&str)->Result<bool> {
    let node:Option<(String,String,i64)>=store.db.query_row(
        "SELECT root_ticket_id,parent_ticket_id,depth
         FROM sa_subagent_nodes WHERE child_ticket_id=?1",
        [child_ticket_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((root,parent,depth))=node else{return Ok(true)};
    if !(1..=MAX_DEPTH).contains(&depth) || root==parent || root==child_ticket_id {
        return Ok(false);
    }
    let settings=policy(store,&root)?.ok_or("Subagent root authorization was removed.")?;
    if settings.revoked || depth>settings.max_depth ||
        !super::super::auto::ticket_authorized(store,&root)? ||
        !root_mandate_active(store,&root)? {
        return Ok(false);
    }
    let root_ticket=get_ticket(store,&root)?;
    let parent_ticket=get_ticket(store,&parent)?;
    let root_team=configuration(store,&root_ticket.orchestration_id)?;
    if root_ticket.state!="queued" ||
        !root_team.enabled || root_team.project_id!=root_ticket.project_id ||
        root_team.policy_revision!=settings.policy_revision ||
        parent_ticket.state!="running" && parent_ticket.state!="waiting_approval"
            && parent_ticket.state!="waiting_review" {
        return Ok(false);
    }
    if depth==1 {
        let top:bool=store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_work_items
             WHERE ticket_id=?1 AND child_ticket_id=?2)",
            params![root,parent],|r|r.get(0),
        ).map_err(|e|e.to_string())?;
        return Ok(top);
    }
    // The recursive chain is depth-bounded. Each level must have immutable
    // native lineage to the same root and no stopped/missing intermediate.
    let link:Option<(String,i64)>=store.db.query_row(
        "SELECT parent_ticket_id,depth
         FROM sa_subagent_nodes WHERE root_ticket_id=?1 AND child_ticket_id=?2",
        params![root,parent],|r|Ok((r.get(0)?,r.get(1)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((grandparent,parent_depth))=link else{return Ok(false)};
    if parent_depth+1!=depth || grandparent==child_ticket_id {return Ok(false)}
    lineage_active(store,&parent)
}

pub(crate) fn descendant_sessions(store:&Store,root:&str)->Result<Vec<String>> {
    let mut query=store.db.prepare(
        "SELECT DISTINCT assignment.session_id
         FROM sa_subagent_nodes node
         JOIN sa_assignments assignment ON assignment.ticket_id=node.child_ticket_id
         WHERE node.root_ticket_id=?1 AND assignment.state='running'
         ORDER BY assignment.session_id LIMIT 24"
    ).map_err(|e|e.to_string())?;
    let rows=query.query_map([root],|r|r.get::<_,String>(0))
        .map_err(|e|e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())
}

/// Called from the root Stop transaction before releasing any native lease.
/// Delayed callbacks cannot resume a descendant even across a second host.
pub(crate) fn revoke_descendants(tx:&Connection,root:&str,at:i64)->Result<()> {
    let exists:bool=tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_subagent_policies WHERE root_ticket_id=?1)",
        [root],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    if !exists{return Ok(())}
    tx.execute(
        "UPDATE sa_subagent_policies SET revoked_at=?2
         WHERE root_ticket_id=?1 AND revoked_at IS NULL",
        params![root,at],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        "UPDATE sa_subagent_nodes SET state='stopped',reason='Root Demand stopped',
         updated_at=?2 WHERE root_ticket_id=?1 AND state<>'completed'",
        params![root,at],
    ).map_err(|e|e.to_string())?;
    let descendants="(SELECT child_ticket_id FROM sa_subagent_nodes
                     WHERE root_ticket_id=?1 AND child_ticket_id IS NOT NULL)";
    tx.execute(
        &format!("UPDATE sa_mandates SET revoked=1
                  WHERE ticket_id IN {descendants}"),[root],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        &format!("UPDATE sa_assignments SET state='stopped',updated_at=?2
                  WHERE state='running' AND ticket_id IN {descendants}"),
        params![root,at],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        &format!("UPDATE sa_council_requests SET state='blocked',updated_at=?2
                  WHERE state IN ('admitted','bound','starting','dispatched')
                  AND ticket_id IN {descendants}"),
        params![root,at],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        &format!("UPDATE sa_tickets SET state='stopped',active_assignment_id=NULL,
                  revision=revision+1,updated_at=?2
                  WHERE id IN {descendants}
                  AND state IN ('queued','running','waiting_approval','waiting_review','blocked')"),
        params![root,at],
    ).map_err(|e|e.to_string())?;
    tx.execute(
        &format!("UPDATE sa_effect_receipts SET state='uncertain',updated_at=?2
                  WHERE state='pending' AND ticket_id IN {descendants}"),
        params![root,at],
    ).map_err(|e|e.to_string())?;
    Ok(())
}

#[derive(Clone)]
struct Pending {
    id:String,root:String,parent:String,key:String,objective:String,worker:String,
    depth:i64,
}

fn ready(store:&Store)->Result<Vec<Pending>> {
    let mut query=store.db.prepare(
        "SELECT node.id,node.root_ticket_id,node.parent_ticket_id,
                node.item_key,node.objective,node.worker_bot_id,node.depth
         FROM sa_subagent_nodes node
         JOIN sa_subagent_policies policy ON policy.root_ticket_id=node.root_ticket_id
         JOIN sa_demand_plans plan ON plan.ticket_id=node.root_ticket_id
         JOIN sa_tickets root ON root.id=plan.ticket_id
         JOIN sa_tickets parent ON parent.id=node.parent_ticket_id
         WHERE node.state='pending' AND node.child_ticket_id IS NULL
           AND policy.revoked_at IS NULL AND node.depth<=policy.max_depth
           AND plan.state='active' AND root.state='queued'
           AND root.active_assignment_id IS NULL
           AND root.policy_revision=policy.policy_revision
           AND plan.policy_revision=policy.policy_revision
           AND parent.state IN ('waiting_approval','waiting_review')
           AND NOT EXISTS(SELECT 1 FROM sa_subagent_nodes other
               WHERE other.root_ticket_id=node.root_ticket_id
                 AND other.state='running')
         ORDER BY node.created_at,node.id LIMIT 12"
    ).map_err(|e|e.to_string())?;
    let rows=query.query_map([],|r|Ok(Pending{
        id:r.get(0)?,root:r.get(1)?,parent:r.get(2)?,
        key:r.get(3)?,objective:r.get(4)?,worker:r.get(5)?,depth:r.get(6)?,
    })).map_err(|e|e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())
}

fn reconcile(store:&Store)->Result<()> {
    let now=storage::now();
    store.db.execute(
        "UPDATE sa_subagent_nodes SET state='completed',reason=NULL,updated_at=?1
         WHERE state IN ('running','waiting_review')
           AND child_ticket_id IN (SELECT id FROM sa_tickets WHERE state='completed')
           AND NOT EXISTS(
               SELECT 1 FROM sa_subagent_nodes descendant
               WHERE descendant.parent_ticket_id=sa_subagent_nodes.child_ticket_id
                 AND descendant.state<>'completed')",[now],
    ).map_err(|e|e.to_string())?;
    store.db.execute(
        "UPDATE sa_subagent_nodes SET state='waiting_review',updated_at=?1
         WHERE state='running' AND child_ticket_id IN (
             SELECT id FROM sa_tickets WHERE state IN ('waiting_approval','waiting_review'))",
        [now],
    ).map_err(|e|e.to_string())?;
    store.db.execute(
        "UPDATE sa_subagent_nodes SET state='blocked',reason='Child needs recovery',updated_at=?1
         WHERE state IN ('running','waiting_review') AND child_ticket_id IN (
             SELECT id FROM sa_tickets
             WHERE state IN ('blocked','failed','interrupted','stopped','inconclusive'))",
        [now],
    ).map_err(|e|e.to_string())?;
    Ok(())
}

pub(crate) async fn advance(app:&tauri::AppHandle)->Result<usize> {
    if crate::feature_controls::get().bots_paused{return Ok(0)}
    let candidates={
        let state=app.state::<AppState>();
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let _fence=super::super::effects::lock_dispatch(&store.path)?;
        reconcile(&store)?;
        ready(&store)?
    };
    let mut admitted=0;
    for item in candidates.into_iter().take(8) {
        let candidate={
            let state=app.state::<AppState>();
            let agents=state.agents.lock().map_err(|e|e.to_string())?;
            let active=agents.keys().cloned().collect::<Vec<_>>();
            let mut store=state.store.lock().map_err(|e|e.to_string())?;
            let _fence=super::super::effects::lock_dispatch(&store.path)?;
            let pending:bool=store.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM sa_subagent_nodes
                 WHERE id=?1 AND state='pending' AND child_ticket_id IS NULL)",
                [&item.id],|r|r.get(0),
            ).map_err(|e|e.to_string())?;
            if !pending || !super::super::auto::ticket_authorized(&store,&item.root)? {
                None
            } else {
                let root=get_ticket(&store,&item.root)?;
                let parent=get_ticket(&store,&item.parent)?;
                let team=configuration(&store,&root.orchestration_id)?;
                let snapshot=ticket_team_snapshot(&store,&root.id)?;
                let policy=policy(&store,&root.id)?
                    .ok_or("Missing native subagent policy.")?;
                let allowed=allowed_children(&store,&root.id,&parent.id,
                    &super::super::ticket_worker(&store,&parent.id)?)?;
                let busy=super::super::active_manual_conflict(
                    &store,&active,&item.worker,&snapshot.project_id,
                )?;
                // A validated spawn request may wait before admission. The
                // selected Worker must still match its root Team profile.
                if superagent::roster::require_current_profile(
                    &store,&root.id,"worker",&item.worker,
                ).is_err() {
                    store.db.execute(
                        "UPDATE sa_subagent_nodes SET state='blocked',
                         reason='Worker profile changed after Demand admission',
                         updated_at=?2 WHERE id=?1 AND state='pending'
                         AND child_ticket_id IS NULL",
                        params![item.id,storage::now()],
                    ).map_err(|e|e.to_string())?;
                    None
                } else if policy.revoked || root.state!="queued" ||
                    !team.enabled || team.policy_revision!=policy.policy_revision ||
                    snapshot.team_version!=policy.team_version ||
                    parent.state!="waiting_approval" && parent.state!="waiting_review" ||
                    item.depth>policy.max_depth || !allowed.contains(&item.worker) ||
                    busy {None}
                else {
                    let key=format!("subagent.{}",item.id);
                    let (child,launch)=super::super::admit_role_with_selected(
                        &mut store,&root.orchestration_id,&item.objective,
                        &key,false,false,false,Some(&item.worker),
                    )?;
                    let changed=store.db.execute(
                        "UPDATE sa_subagent_nodes SET child_ticket_id=?2,
                         state=?3,updated_at=?4
                         WHERE id=?1 AND state='pending' AND child_ticket_id IS NULL",
                        params![item.id,child.id,if launch {"running"} else {"blocked"},storage::now()],
                    ).map_err(|e|e.to_string())?;
                    if changed!=1{return Err("Concurrent descendant admission conflict.".into())}
                    if launch {Some(child)} else {None}
                }
            }
        };
        let Some(child)=candidate else{continue};
        let allowed={
            let state=app.state::<AppState>();
            let store=state.store.lock().map_err(|e|e.to_string())?;
            lineage_active(&store,&child.id)?
        };
        if !allowed {continue}
        let result=bots::gateway::deliver(app,bots::gateway::Inbound{
            bot_id:item.worker.clone(),
            channel_id:super::super::CHANNEL.into(),
            thread:child.id.clone(),
            sender_id:"local-user".into(),
            sender_name:"Authorized native SuperAgent descendant".into(),
            text:format!(
                "Native SuperAgent descendant for Demand {}.\nParent Ticket: {}.\n\
                 Depth: {}. Subagent key: {}.\n\n{}\n\n\
                 This child is read-only: no inherited file, shell, MCP,
                 GitHub or Council authority. You may request another bounded
                 native child ONLY via pandacode_superagent_spawn_child when
                 this Demand's local-user policy permits it. Parent completion
                 is blocked until every required child settles under its own
                 real native evidence and review gate.",
                item.root,item.parent,item.depth,item.key,item.objective,
            ),
            trusted:true,image_ids:None,moodboard_ids:None,
        }).await;
        if let Err(error)=result {
            let state=app.state::<AppState>();
            if let Ok(store)=state.store.lock() {
                let _=super::super::fail_launch(&store,&child.id,&error);
                let _=store.db.execute(
                    "UPDATE sa_subagent_nodes SET state='blocked',
                     reason='Native descendant dispatch failed',updated_at=?2
                     WHERE id=?1 AND state='running'",
                    params![item.id,storage::now()],
                );
            }
        } else {admitted+=1}
    }
    Ok(admitted)
}


#[cfg(test)]
mod mandate_tests {
    use super::*;

    #[test]
    fn nested_dispatch_requires_an_unexpired_unrevoked_original_root_mandate() {
        let db=Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_tickets(
                id TEXT PRIMARY KEY,state TEXT,active_assignment_id TEXT,
                policy_revision INTEGER,project_id TEXT,actor TEXT);
             CREATE TABLE sa_subagent_policies(
                root_ticket_id TEXT PRIMARY KEY,policy_revision INTEGER,revoked_at INTEGER);
             CREATE TABLE sa_mandates(
                ticket_id TEXT PRIMARY KEY,revoked INTEGER,expires_at INTEGER,
                policy_revision INTEGER,project_id TEXT,actor TEXT,
                allowed_tools TEXT,allowed_side_effects INTEGER);
             INSERT INTO sa_tickets VALUES('root','queued',NULL,1,'project','local-user');
             INSERT INTO sa_subagent_policies VALUES('root',1,NULL);",
        ).unwrap();
        db.execute(
            "INSERT INTO sa_mandates VALUES(
                'root',0,?1,1,'project','local-user','[]',0)",
            [storage::now()+60_000],
        ).unwrap();
        let store=Store{db,path:std::path::PathBuf::from(":memory:")};
        assert!(root_mandate_active(&store,"root").unwrap());
        assert!(!root_mandate_active(&store,"unknown").unwrap());

        store.db.execute(
            "UPDATE sa_mandates SET revoked=1 WHERE ticket_id='root'",[],
        ).unwrap();
        assert!(!root_mandate_active(&store,"root").unwrap(),
            "revoking the parent must fence all descendants");

        store.db.execute(
            "UPDATE sa_mandates SET revoked=0,expires_at=1 WHERE ticket_id='root'",[],
        ).unwrap();
        assert!(!root_mandate_active(&store,"root").unwrap(),
            "descendant Mandates must not outlive the root's expiry");

        store.db.execute(
            "UPDATE sa_mandates SET expires_at=?1,project_id='other' WHERE ticket_id='root'",
            [storage::now()+60_000],
        ).unwrap();
        assert!(!root_mandate_active(&store,"root").unwrap(),
            "a moved or widened project must never be inherited");

        store.db.execute(
            "UPDATE sa_mandates SET project_id='project',allowed_tools='[\"shell\"]'
             WHERE ticket_id='root'",[],
        ).unwrap();
        assert!(!root_mandate_active(&store,"root").unwrap(),
            "a root with escalated tools is not a valid read-only ancestry");

        store.db.execute(
            "UPDATE sa_mandates SET allowed_tools='[]' WHERE ticket_id='root'",[],
        ).unwrap();
        store.db.execute(
            "UPDATE sa_subagent_policies SET revoked_at=?1 WHERE root_ticket_id='root'",
            [storage::now()],
        ).unwrap();
        assert!(!root_mandate_active(&store,"root").unwrap(),
            "explicit native delegation revocation must take effect");
    }
}
