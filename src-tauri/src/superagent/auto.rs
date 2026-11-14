//! Native GitHub Auto intake, strictly opt-in and limited to authenticated,
//! bounded issue-opened metadata. Incoming issue content is never a permission.
//! Auto may create read-only Demand Tickets; it cannot approve results, write
//! files, push, comment, merge, deploy or broaden the original Team scope.

use super::{configuration,events,require_main_window};
use crate::{bots,storage::{self,Store},AppState,Result};
use rusqlite::{params,OptionalExtension};
use serde::Serialize;
use sha2::{Digest,Sha256};
use tauri::{Manager,State};

mod intake;
pub(crate) use intake::tick;

const GRANT_TTL_MS:i64=24*60*60*1000;
const MAX_DAILY:u8=20;

pub(crate) const SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_auto_source_grants (
    source_id TEXT PRIMARY KEY REFERENCES sa_event_sources(id) ON DELETE RESTRICT,
    source_revision INTEGER NOT NULL CHECK(source_revision>0),
    repository TEXT NOT NULL,
    actor_filter TEXT NOT NULL,
    event_kind TEXT NOT NULL CHECK(event_kind='issues.opened'),
    operator_login TEXT NOT NULL,
    max_daily_tasks INTEGER NOT NULL CHECK(max_daily_tasks BETWEEN 1 AND 20),
    authorized_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL CHECK(expires_at>authorized_at),
    last_polled_at INTEGER NOT NULL DEFAULT 0 CHECK(last_polled_at>=0),
    revoked_at INTEGER,
    CHECK(revoked_at IS NULL OR revoked_at>=authorized_at)
);
CREATE TABLE IF NOT EXISTS sa_auto_intake (
    source_id TEXT NOT NULL REFERENCES sa_auto_source_grants(source_id) ON DELETE RESTRICT,
    delivery_id TEXT NOT NULL,
    subject_key TEXT NOT NULL,
    event_actor TEXT NOT NULL,
    repository TEXT NOT NULL,
    event_kind TEXT NOT NULL CHECK(event_kind='issues.opened'),
    source_revision INTEGER NOT NULL,
    occurred_at INTEGER NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('pending','admitting','admitted','blocked')),
    ticket_id TEXT UNIQUE REFERENCES sa_tickets(id) ON DELETE RESTRICT,
    reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(source_id,delivery_id),
    FOREIGN KEY(source_id,delivery_id)
      REFERENCES sa_event_decisions(source_id,delivery_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS sa_auto_intake_pending ON sa_auto_intake(state,created_at);
CREATE INDEX IF NOT EXISTS sa_auto_intake_source_time ON sa_auto_intake(source_id,created_at);
"#;

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct AutoStatus {
    pub source_id:String,
    pub enabled:bool,
    pub mode:String,
    pub repository:String,
    pub actor_filter:String,
    pub event_kind:String,
    pub max_daily_tasks:Option<i64>,
    pub used_last_day:i64,
    pub expires_at:Option<i64>,
    pub operator_login:Option<String>,
    pub pending:i64,
    pub blocked:i64,
}

fn eligible(src:&events::EventSource)->bool {
    src.mode=="observe" && src.event_kind=="issues.opened"
}

fn authenticated_login()->Result<String> {
    let response=crate::github::run_gh(
        &std::env::temp_dir(),&["api","user"],crate::github::TIMEOUT,
    )?;
    let json=crate::github::json(&response)?;
    let login=json.get("login").and_then(serde_json::Value::as_str)
        .ok_or("GitHub CLI did not report an authenticated operator.")?;
    if login.is_empty() || login.len()>80 || !login.bytes().all(|byte|
        byte.is_ascii_alphanumeric() || byte==b'-') {
        return Err("Invalid GitHub operator login.".into());
    }
    Ok(login.to_ascii_lowercase())
}

fn active_grant(
    store:&Store,src:&events::EventSource,at:i64,
)->Result<Option<(String,i64,i64)>> {
    if !eligible(src) {return Ok(None)}
    let result:Option<(String,i64,i64)>=store.db.query_row(
        "SELECT operator_login,max_daily_tasks,expires_at
         FROM sa_auto_source_grants WHERE source_id=?1 AND source_revision=?2
           AND repository=?3 AND lower(actor_filter)=lower(?4)
           AND event_kind=?5 AND revoked_at IS NULL AND expires_at>?6",
        params![src.id,src.policy_revision,src.repository,src.actor_filter,src.event_kind,at],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())?;
    Ok(result)
}

pub(crate) fn ticket_authorized(store:&Store,ticket_id:&str)->Result<bool> {
    let linked:Option<(String,String,String)>=store.db.query_row(
        "SELECT source_id,delivery_id,ticket_id FROM sa_auto_intake
         WHERE ticket_id=?1 OR ticket_id=(
             SELECT ticket_id FROM sa_work_items WHERE child_ticket_id=?1 LIMIT 1)
           OR ticket_id=(
             SELECT root_ticket_id FROM sa_subagent_nodes WHERE child_ticket_id=?1 LIMIT 1)",
        [ticket_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((source_id,delivery_id,root_ticket_id))=linked else {
        return Ok(true);
    };
    let src=events::source(store,&source_id)?;
    if active_grant(store,&src,storage::now())?.is_none() {return Ok(false)}
    let current:bool=store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_auto_intake
           WHERE source_id=?1 AND delivery_id=?2 AND ticket_id=?3
             AND state='admitted' AND source_revision=?4)",
        params![source_id,delivery_id,root_ticket_id,src.policy_revision],
        |r|r.get(0),
    ).map_err(|e|e.to_string())?;
    Ok(current)
}

fn status(store:&Store,source_id:&str)->Result<AutoStatus> {
    let src=events::source(store,source_id)?;
    let now=storage::now();
    let grant:Option<(i64,i64,String,i64)>=store.db.query_row(
        "SELECT max_daily_tasks,expires_at,operator_login,source_revision
         FROM sa_auto_source_grants WHERE source_id=?1",
        [source_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).optional().map_err(|e|e.to_string())?;
    let (max_daily_tasks,expires_at,operator_login,rev)=match grant {
        Some((max,expiry,operator,rev))=>(Some(max),Some(expiry),Some(operator),rev),
        None=>(None,None,None,0),
    };
    let used_last_day:i64=store.db.query_row(
        "SELECT COUNT(*) FROM sa_auto_intake
         WHERE source_id=?1 AND created_at>?2",
        params![source_id,now.saturating_sub(86_400_000)],
        |r|r.get(0),
    ).map_err(|e|e.to_string())?;
    let (pending,blocked):(i64,i64)=store.db.query_row(
        "SELECT COUNT(*) FILTER(WHERE state='pending'),
         COUNT(*) FILTER(WHERE state='blocked')
         FROM sa_auto_intake WHERE source_id=?1",
        [source_id],|r|Ok((r.get(0)?,r.get(1)?)),
    ).map_err(|e|e.to_string())?;
    let enabled=eligible(&src) && rev==src.policy_revision &&
        active_grant(store,&src,now)?.is_some();
    Ok(AutoStatus {
        source_id:src.id,enabled,
        mode:if enabled {"auto".into()} else {src.mode.clone()},
        repository:src.repository,actor_filter:src.actor_filter,
        event_kind:src.event_kind,max_daily_tasks,used_last_day,expires_at,
        operator_login,pending,blocked,
    })
}

/// Pure transaction-bound intake: only an authenticated Observe adapter may
/// call this, after verified identity/actor/event filters and durable receipt.
/// The raw GitHub body is never stored in the task/outbox.
pub(super) fn enqueue_authenticated(
    tx:&rusqlite::Transaction<'_>,
    src:&events::EventSource,event:&events::EventEnvelope,
    decision:&events::EventDecision,occurred_at:i64,signed_in:&str,
)->Result<()> {
    if decision.duplicate || decision.decision!="observed_permitted" ||
        src.mode!="observe" || src.event_kind!="issues.opened" ||
        event.event_kind!="issues.opened" || !event.subject_key.starts_with("issue:") {
        return Ok(());
    }
    let now=storage::now();
    let grant:Option<(String,i64,i64,i64)>=tx.query_row(
        "SELECT operator_login,max_daily_tasks,authorized_at,expires_at
         FROM sa_auto_source_grants
         WHERE source_id=?1 AND source_revision=?2 AND repository=?3
           AND lower(actor_filter)=lower(?4)
           AND event_kind='issues.opened' AND revoked_at IS NULL
           AND expires_at>?5",
        params![src.id,src.policy_revision,src.repository,src.actor_filter,now],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((operator,max_daily,authorized_at,expiry))=grant else{return Ok(())};
    if !operator.eq_ignore_ascii_case(signed_in) || occurred_at<authorized_at ||
        now>=expiry || !event.actor.eq_ignore_ascii_case(&src.actor_filter) {
        return Ok(());
    }
    let count:i64=tx.query_row(
        "SELECT COUNT(*) FROM sa_auto_intake
         WHERE source_id=?1 AND created_at>?2",
        params![src.id,now.saturating_sub(86_400_000)],
        |r|r.get(0),
    ).map_err(|e|e.to_string())?;
    let permitted=count<max_daily;
    tx.execute(
        "INSERT INTO sa_auto_intake(source_id,delivery_id,subject_key,event_actor,
            repository,event_kind,source_revision,occurred_at,state,reason,
            created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,'issues.opened',?6,?7,?8,?9,?10,?10)",
        params![src.id,event.delivery_id,event.subject_key,event.actor,
            event.repository,src.policy_revision,occurred_at,
            if permitted {"pending"} else {"blocked"},
            if permitted {None} else {Some("Source daily admission budget exhausted")},
            now],
    ).map_err(|e|e.to_string())?;
    Ok(())
}

/// One native periodic authenticated Auto service for both desktop and
/// serve. No GitHub request is made without a persisted, unexpired user grant.
/// Source revocation and shutdown cancel future polling/admission.
pub(crate) fn start_native(app:tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let mut last_error=String::new();
        loop {
            if app.state::<AppState>().shutdown.load(
                std::sync::atomic::Ordering::SeqCst,
            )!=0 {break}
            match tick(&app).await {
                Ok(_) => last_error.clear(),
                Err(error) if error!=last_error => {
                    eprintln!("SuperAgent bounded GitHub Auto: {error}");
                    last_error=error;
                }
                Err(_) => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

#[tauri::command]
pub async fn superagent_auto_status(
    window:tauri::WebviewWindow,state:State<'_,AppState>,source_id:String,
)->Result<AutoStatus> {
    require_main_window(&window)?;
    let store=state.store.lock().map_err(|e|e.to_string())?;
    status(&store,&source_id)
}

#[tauri::command]
pub async fn superagent_auto_authorize(
    window:tauri::WebviewWindow,state:State<'_,AppState>,
    source_id:String,expected_revision:i64,max_daily_tasks:u8,
)->Result<AutoStatus> {
    require_main_window(&window)?;
    if !(1..=MAX_DAILY).contains(&max_daily_tasks) {
        return Err("Auto tasks per day must be between 1 and 20.".into());
    }
    // The repository's authenticated operator is frozen as part of this
    // local grant. Changing gh credentials cannot silently grant an Auto run.
    let login=tokio::task::spawn_blocking(authenticated_login)
        .await.map_err(|e|e.to_string())??;
    let (src,team_name)={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let src=events::source(&store,&source_id)?;
        let team=configuration(&store,&src.orchestration_id)?;
        if src.policy_revision!=expected_revision || !eligible(&src) ||
            !team.enabled || team.worker_bot_ids.is_empty() ||
            crate::feature_controls::get().bots_paused {
            return Err("Auto authorization requires an unchanged, enabled Team and authenticated Observe source for issues.opened.".into());
        }
        if active_grant(&store,&src,storage::now())?.is_some() {
            return Err("Auto is already authorized; revoke before changing its scope.".into());
        }
        (src,team.name)
    };
    use tauri_plugin_dialog::{DialogExt,MessageDialogButtons};
    let dialog=window.dialog().message(format!(
        "Authorize bounded automatic read-only Demand admission?\n\n\
         Team: {team_name}\nRepository: {}\nAllowed GitHub actor: {}\n\
         Event: issues.opened\nSigned-in operator: {login}\n\
         Maximum Demands per rolling 24h: {max_daily_tasks}\n\
         Grant expiration: 24 hours\n\n\
         Only authenticated new issue events can create read-only native Tickets. \
         This does NOT authorize file writes, shell/MCP tools, comments, pushes, \
         merges or releases; any required WorkItem file writes need fresh local confirmation.",
        src.repository,src.actor_filter,
    )).title("Authorize GitHub Auto (read-only)")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Authorize read-only Auto".into(),"Cancel".into()
        ));
    #[cfg(desktop)]
    let dialog=dialog.parent(&window);
    let (send,recv)=tokio::sync::oneshot::channel();
    dialog.show(move |approved|{let _=send.send(approved);});
    if !recv.await.unwrap_or(false) {return Err("GitHub Auto was not authorized.".into());}
    let store=state.store.lock().map_err(|e|e.to_string())?;
    let current=events::source(&store,&source_id)?;
    let team=configuration(&store,&current.orchestration_id)?;
    if current.policy_revision!=expected_revision || !eligible(&current) || !team.enabled ||
        current.repository!=src.repository || current.actor_filter!=src.actor_filter ||
        current.event_kind!=src.event_kind ||
        active_grant(&store,&current,storage::now())?.is_some() {
        return Err("GitHub source or Team changed during authorization; reload and retry.".into());
    }
    let now=storage::now();
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO sa_auto_source_grants(source_id,source_revision,repository,
           actor_filter,event_kind,operator_login,max_daily_tasks,authorized_at,
           expires_at,revoked_at)
         VALUES(?1,?2,?3,?4,'issues.opened',?5,?6,?7,?8,NULL)
         ON CONFLICT(source_id) DO UPDATE SET source_revision=excluded.source_revision,
           repository=excluded.repository,actor_filter=excluded.actor_filter,
           event_kind=excluded.event_kind,operator_login=excluded.operator_login,
           max_daily_tasks=excluded.max_daily_tasks,authorized_at=excluded.authorized_at,
           expires_at=excluded.expires_at,revoked_at=NULL",
        params![src.id,src.policy_revision,src.repository,src.actor_filter,
            login,i64::from(max_daily_tasks),now,now+GRANT_TTL_MS],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    status(&store,&source_id)
}

#[tauri::command]
pub async fn superagent_auto_revoke(
    window:tauri::WebviewWindow,app:tauri::AppHandle,
    state:State<'_,AppState>,source_id:String,expected_revision:i64,
)->Result<AutoStatus> {
    require_main_window(&window)?;
    let sessions={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let src=events::source(&store,&source_id)?;
        if src.policy_revision!=expected_revision {
            return Err("Auto source policy changed; revoke using the latest revision.".into());
        }
        let now=storage::now();
        let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
        tx.execute(
            "UPDATE sa_auto_source_grants SET revoked_at=?2
             WHERE source_id=?1 AND revoked_at IS NULL",
            params![source_id,now],
        ).map_err(|e|e.to_string())?;
        tx.execute(
            "UPDATE sa_auto_intake SET state='blocked',
                reason='Source Auto authorization was revoked',updated_at=?2
             WHERE source_id=?1 AND state IN ('pending','admitting')",
            params![source_id,now],
        ).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
        let mut running=Vec::new();
        let mut statement=store.db.prepare(
            "SELECT ticket_id FROM sa_auto_intake
             WHERE source_id=?1 AND ticket_id IS NOT NULL AND state='admitted'"
        ).map_err(|e|e.to_string())?;
        let rows=statement.query_map([&source_id],|r|r.get::<_,String>(0))
            .map_err(|e|e.to_string())?;
        let ids=rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
        drop(statement);
        for id in ids {
            let children=super::scheduler::sessions_for_root(&store,&id)?;
            if let Ok((_ticket,session))=super::stop(&store,&id) {
                running.extend(children);
                if let Some(session)=session {running.push(session);}
            }
        }
        running
    };
    for session in sessions {
        let _=crate::stop_session(app.clone(),app.state::<AppState>(),session).await;
    }
    let store=state.store.lock().map_err(|e|e.to_string())?;
    status(&store,&source_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture()->(Store,events::EventSource) {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE sa_orchestrations(id TEXT PRIMARY KEY);
             CREATE TABLE sa_tickets(id TEXT PRIMARY KEY);
             CREATE TABLE sa_work_items(ticket_id TEXT,child_ticket_id TEXT);
             CREATE TABLE sa_subagent_nodes(child_ticket_id TEXT,root_ticket_id TEXT);
             INSERT INTO sa_orchestrations(id) VALUES('team');",
        ).unwrap();
        db.execute_batch(events::SCHEMA).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        let now=storage::now();
        db.execute(
            "INSERT INTO sa_event_sources(
               id,orchestration_id,repository,actor_filter,event_kind,mode,
               policy_revision,created_at,updated_at
             ) VALUES('source','team','owner/repo','contributor',
               'issues.opened','observe',1,?1,?1)",
            [now-60_000],
        ).unwrap();
        let store=Store{db,path:std::path::PathBuf::from(":memory:")};
        let source=events::source(&store,"source").unwrap();
        (store,source)
    }

    fn decision(id:&str,number:i64)->(events::EventEnvelope,events::EventDecision) {
        let event=events::EventEnvelope{
            source_id:"source".into(),delivery_id:id.into(),
            repository:"owner/repo".into(),actor:"contributor".into(),
            event_kind:"issues.opened".into(),subject_key:format!("issue:{number}"),
            sequence:number,
        };
        let decision=events::EventDecision{
            source_id:event.source_id.clone(),delivery_id:event.delivery_id.clone(),
            decision:"observed_permitted".into(),policy_revision:1,
            sequence:event.sequence,at:storage::now(),duplicate:false,
        };
        (event,decision)
    }

    fn record(store:&Store,event:&events::EventEnvelope,decision:&events::EventDecision,
        source:&events::EventSource,operator:&str
    )->Result<()> {
        let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
        tx.execute(
            "INSERT INTO sa_event_decisions(source_id,delivery_id,repository,actor,
               event_kind,subject_key,sequence,decision,policy_revision,at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![event.source_id,event.delivery_id,event.repository,event.actor,
                event.event_kind,event.subject_key,event.sequence,
                decision.decision,decision.policy_revision,decision.at],
        ).map_err(|e|e.to_string())?;
        enqueue_authenticated(&tx,source,event,decision,storage::now(),operator)?;
        tx.commit().map_err(|e|e.to_string())
    }

    #[test]
    fn auto_needs_authentic_grant_and_never_infers_permission_from_observe() {
        let (store,src)=fixture();
        let (event,decision)=decision("first",77);
        record(&store,&event,&decision,&src,"operator").unwrap();
        let count:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_auto_intake",[],|r|r.get(0),
        ).unwrap();
        assert_eq!(count,0,"Observe alone must never admit an Auto issue.");
        assert!(!status(&store,"source").unwrap().enabled);
    }


    #[test]
    fn auto_child_and_nested_subagent_revalidate_the_original_root_grant() {
        let (store,source)=fixture();
        let now=storage::now();
        store.db.execute(
            "INSERT INTO sa_auto_source_grants(
               source_id,source_revision,repository,actor_filter,event_kind,
               operator_login,max_daily_tasks,authorized_at,expires_at
             ) VALUES('source',1,'owner/repo','contributor','issues.opened',
                'operator',2,?1,?2)",
            params![now-10,now+60_000],
        ).unwrap();
        let (event,decision)=decision("root-delivery",77);
        record(&store,&event,&decision,&source,"operator").unwrap();
        store.db.execute_batch(
            "INSERT INTO sa_tickets(id) VALUES('root'),('worker'),('grandchild'),('manual');
             INSERT INTO sa_work_items(ticket_id,child_ticket_id) VALUES('root','worker');
             INSERT INTO sa_subagent_nodes(child_ticket_id,root_ticket_id)
             VALUES('grandchild','root');
             UPDATE sa_auto_intake SET state='admitted',ticket_id='root'
             WHERE source_id='source' AND delivery_id='root-delivery';",
        ).unwrap();

        for id in ["root","worker","grandchild"] {
            assert!(ticket_authorized(&store,id).unwrap(),
                "the descendant must use the original authorized root Ticket");
        }
        assert!(ticket_authorized(&store,"manual").unwrap(),
            "unrelated manual Tickets do not need a GitHub Auto grant");

        store.db.execute(
            "UPDATE sa_auto_source_grants SET revoked_at=?1 WHERE source_id='source'",
            [storage::now()],
        ).unwrap();
        for id in ["root","worker","grandchild"] {
            assert!(!ticket_authorized(&store,id).unwrap(),
                "revoking the root grant must revoke all descendant authority");
        }
        assert!(ticket_authorized(&store,"manual").unwrap());
    }

    #[test]
    fn auto_grant_is_exact_actor_operator_revision_ttl_and_daily_budget() {
        let (store,src)=fixture();
        let now=storage::now();
        store.db.execute(
            "INSERT INTO sa_auto_source_grants(
               source_id,source_revision,repository,actor_filter,event_kind,
               operator_login,max_daily_tasks,authorized_at,expires_at
             ) VALUES('source',1,'owner/repo','contributor','issues.opened',
                 'operator',1,?1,?2)",
            params![now-10,now+60_000],
        ).unwrap();
        assert!(status(&store,"source").unwrap().enabled);
        let (first,first_decision)=decision("first",77);
        record(&store,&first,&first_decision,&src,"operator").unwrap();
        let accepted:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_auto_intake WHERE state='pending'",[],
            |r|r.get(0),
        ).unwrap();
        assert_eq!(accepted,1);
        let (second,second_decision)=decision("second",78);
        record(&store,&second,&second_decision,&src,"operator").unwrap();
        let rejected:i64=store.db.query_row(
            "SELECT COUNT(*) FROM sa_auto_intake WHERE state='blocked'",[],
            |r|r.get(0),
        ).unwrap();
        assert_eq!(rejected,1,"Daily cap is native, not prompt guidance.");
        let (third,third_decision)=decision("third",79);
        record(&store,&third,&third_decision,&src,"different-operator").unwrap();
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_auto_intake",[],|r|r.get::<_,i64>(0),
        ).unwrap(),2,"An unexpected signed-in account must never admit.");
        store.db.execute(
            "UPDATE sa_auto_source_grants SET revoked_at=?1 WHERE source_id='source'",
            [storage::now()],
        ).unwrap();
        assert!(!status(&store,"source").unwrap().enabled);
        let (fourth,fourth_decision)=decision("fourth",80);
        record(&store,&fourth,&fourth_decision,&src,"operator").unwrap();
        assert_eq!(store.db.query_row(
            "SELECT COUNT(*) FROM sa_auto_intake",[],|r|r.get::<_,i64>(0),
        ).unwrap(),2,"Revocation prevents another durable intake.");
    }
}
