//! A separate native Demand test effect. Ordinary chat checks are never task
//! authority. An immutable one-time local grant selects a fixed command, and
//! results are bound to the latest Worker proofs and exact Git snapshot.
use super::*;
use std::{path::Path,process::Command,time::Duration};
use tauri::Manager;

pub(crate) const SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS sa_demand_test_grants (
 ticket_id TEXT PRIMARY KEY REFERENCES sa_tickets(id) ON DELETE RESTRICT,
 team_version INTEGER NOT NULL, policy_revision INTEGER NOT NULL,
 project_id TEXT NOT NULL, checkout_root TEXT NOT NULL,
 command TEXT NOT NULL CHECK(command IN ('npm test','cargo test')),
 manifest_sha256 TEXT NOT NULL CHECK(length(manifest_sha256)=64),
 authorized_at INTEGER NOT NULL
);
CREATE TRIGGER IF NOT EXISTS sa_test_grant_immutable BEFORE UPDATE ON sa_demand_test_grants
 BEGIN SELECT RAISE(ABORT,'Demand test grant is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_test_grant_no_delete BEFORE DELETE ON sa_demand_test_grants
 BEGIN SELECT RAISE(ABORT,'Demand test grant is durable'); END;
CREATE TABLE IF NOT EXISTS sa_demand_test_runs (
 ticket_id TEXT PRIMARY KEY REFERENCES sa_demand_test_grants(ticket_id) ON DELETE RESTRICT,
 plan_sha256 TEXT NOT NULL, snapshot_sha256 TEXT NOT NULL,
 worker_proofs_sha256 TEXT NOT NULL DEFAULT '',
 command TEXT NOT NULL, state TEXT NOT NULL
   CHECK(state IN ('running','passed','failed','uncertain')),
 started_at INTEGER NOT NULL, finished_at INTEGER,
 exit_code INTEGER, stdout_sha256 TEXT, stderr_sha256 TEXT
);
-- Finished failures are immutable; a changed source snapshot may be tested
-- again only twice. Running/uncertain outcomes cannot be replayed.
CREATE TABLE IF NOT EXISTS sa_demand_test_attempts (
 ticket_id TEXT NOT NULL REFERENCES sa_demand_test_grants(ticket_id) ON DELETE RESTRICT,
 attempt INTEGER NOT NULL CHECK(attempt BETWEEN 1 AND 2),
 plan_sha256 TEXT NOT NULL, snapshot_sha256 TEXT NOT NULL,
 worker_proofs_sha256 TEXT NOT NULL DEFAULT '',
 command TEXT NOT NULL, state TEXT NOT NULL CHECK(state='failed'),
 started_at INTEGER NOT NULL, finished_at INTEGER NOT NULL,
 exit_code INTEGER, stdout_sha256 TEXT, stderr_sha256 TEXT,
 PRIMARY KEY(ticket_id,attempt)
);
CREATE TRIGGER IF NOT EXISTS sa_test_attempt_immutable
 BEFORE UPDATE ON sa_demand_test_attempts
 BEGIN SELECT RAISE(ABORT,'Test attempt history is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_test_attempt_no_delete
 BEFORE DELETE ON sa_demand_test_attempts
 BEGIN SELECT RAISE(ABORT,'Test attempt history is durable'); END;
"#;

fn digest(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}

/// Fixed command vectors only. Cargo is locked/offline. The npm command runs
/// the project's explicitly confirmed test script, never model-provided argv.
pub(super) fn manifest_hash(root:&Path,command:&str)->Result<String> {
    let name=match command {
        "npm test"=>"package.json","cargo test"=>"Cargo.toml",
        _=>return Err("Only npm test or cargo test is supported.".into()),
    };
    let bytes=std::fs::read(root.join(name))
        .map_err(|_|"The selected test manifest is unavailable.")?;
    if command=="npm test" {
        let manifest:serde_json::Value=serde_json::from_slice(&bytes)
            .map_err(|_|"Invalid package.json test manifest.")?;
        if !manifest["scripts"]["test"].as_str().is_some_and(|s|!s.trim().is_empty()) {
            return Err("No declared npm test script.".into());
        }
    } else if !root.join("Cargo.lock").is_file() {
        return Err("Cargo.lock is required for locked offline tests.".into());
    }
    let mut sha=Sha256::new();
    sha.update(&bytes);
    if command=="cargo test" {
        sha.update(std::fs::read(root.join("Cargo.lock"))
            .map_err(|_|"Cargo.lock unavailable.")?);
    }
    Ok(format!("{:x}",sha.finalize()))
}

/// A request-key replay may never expand or replace the original process grant.
pub(super) fn replay_matches(db:&rusqlite::Connection,team:&str,key:&str,
    request:Option<(i64,i64,&str,&str,&str,&str)>)->Result<bool> {
    let old:Option<String>=db.query_row(
        "SELECT id FROM sa_tickets WHERE orchestration_id=?1 AND request_key=?2",
        params![team,key],|r|r.get(0),
    ).optional().map_err(|e|e.to_string())?;
    let Some(id)=old else {return Ok(false)};
    let saved:Option<(i64,i64,String,String,String,String)>=db.query_row(
        "SELECT team_version,policy_revision,project_id,checkout_root,command,manifest_sha256
         FROM sa_demand_test_grants WHERE ticket_id=?1",
        [&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)),
    ).optional().map_err(|e|e.to_string())?;
    match (saved,request) {
        (None,None)=>Ok(true),
        (Some((a,b,c,d,e,f)),Some((v,p,project,root,command,hash)))
            if a==v && b==p && c==project && d==root && e==command && f==hash =>Ok(true),
        _=>Err("Demand replay cannot change previously authorized tests.".into()),
    }
}

/// Parameters already confirmed by the local-user consent. Keep the complete
/// native test grant together so callers cannot accidentally permute scope.
pub(super) struct TestGrant<'a> {
    pub team_version:i64,
    pub policy_revision:i64,
    pub project_id:&'a str,
    pub checkout_root:&'a str,
    pub command:&'a str,
    pub manifest_sha256:&'a str,
}

/// Called only from trusted main-window admission AFTER the one-time dialog.
pub(super) fn issue(store:&Store,id:&str,grant:TestGrant<'_>)->Result<()> {
    let TestGrant{
        team_version,policy_revision:policy,project_id:project,
        checkout_root:checkout,command,manifest_sha256:hash,
    }=grant;
    let now=storage::now();
    let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
    let n=tx.execute(
        "INSERT INTO sa_demand_test_grants
         (ticket_id,team_version,policy_revision,project_id,checkout_root,
          command,manifest_sha256,authorized_at)
         SELECT t.id,?2,?3,?4,?5,?6,?7,?8
         FROM sa_tickets t JOIN sa_mandates m ON m.ticket_id=t.id
         JOIN sa_orchestrations team ON team.id=t.orchestration_id
         WHERE t.id=?1 AND t.actor='local-user' AND t.source='manual'
           AND t.state='running' AND t.active_assignment_id IS NOT NULL
           AND t.policy_revision=?3 AND t.project_id=?4
           AND team.enabled=1 AND team.policy_revision=?3 AND team.project_id=?4
           AND EXISTS(SELECT 1 FROM sa_team_versions v
             WHERE v.orchestration_id=team.id AND v.version=?2
             AND v.version=(SELECT MAX(version) FROM sa_team_versions
               WHERE orchestration_id=team.id))
           AND m.actor=t.actor AND length(trim(m.authorization_id))>0
           AND m.revoked=0 AND m.expires_at>?8
           AND m.policy_revision=?3 AND m.project_id=?4
           AND m.allowed_tools='[]' AND m.allowed_side_effects=0",
        params![id,team_version,policy,project,checkout,command,hash,now],
    ).map_err(|e|e.to_string())?;
    if n!=1 {return Err("Test consent changed before admission.".into());}
    tx.execute(
        "INSERT INTO sa_audit(ticket_id,kind,detail,at)
         VALUES(?1,'demand_test_authorized',?2,?3)",
        params![id,format!("Exact approved test command: {command}"),now],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())
}

/// A saved Full Auto Team mode is itself a mandatory evidence gate, even
/// for older Demands admitted before native test consent was available.
pub(super) fn required(db:&rusqlite::Connection,root:&str)->Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_demand_test_grants WHERE ticket_id=?1)
         OR EXISTS(SELECT 1 FROM sa_ticket_team_snapshots snap
           WHERE snap.ticket_id=?1 AND snap.source_kind='native'
             AND json_extract(snap.snapshot_json,'$.policy.autonomy')='full_auto')",
        [root],|r|r.get(0),
    ).map_err(|e|e.to_string())
}

/// Stable identity of fresh, fully accepted native Worker corrections.
/// Unrelated Git mutations cannot change this digest or spend retry budget.
fn worker_proofs_hash(db:&rusqlite::Connection,root:&str)->Result<String> {
    let expected:i64=db.query_row(
        "SELECT COUNT(*) FROM sa_work_items WHERE ticket_id=?1",
        [root],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    let mut stmt=db.prepare(
        "SELECT item.id,proof.assignment_id,proof.evidence_sha256,child.evidence
         FROM sa_work_items item
         JOIN sa_tickets child ON child.id=item.child_ticket_id
         JOIN sa_work_item_evidence_receipts proof
           ON proof.item_id=item.id AND proof.child_ticket_id=child.id
         JOIN sa_assignments latest ON latest.id=proof.assignment_id
         WHERE item.ticket_id=?1 AND item.state='completed'
           AND child.state='completed'
           AND latest.ticket_id=child.id AND latest.state='finished'
           AND latest.result=child.evidence
           AND latest.attempt=(SELECT MAX(a.attempt) FROM sa_assignments a
             WHERE a.ticket_id=child.id)
           AND proof.reviewed_at>=latest.updated_at
           AND (item.requires_write=0 OR EXISTS(
              SELECT 1 FROM sa_worker_write_receipts write
              WHERE write.ticket_id=child.id
                AND write.assignment_id=latest.id AND write.state='applied'))
         ORDER BY item.ordinal,item.id",
    ).map_err(|e|e.to_string())?;
    let rows=stmt.query_map([root],|r|Ok((
        r.get::<_,String>(0)?,r.get::<_,String>(1)?,
        r.get::<_,String>(2)?,r.get::<_,String>(3)?,
    ))).map_err(|e|e.to_string())?;
    let receipts=rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e|e.to_string())?;
    if expected==0 || receipts.len() as i64!=expected {
        return Err("Current independently approved Worker proofs are required.".into());
    }
    let mut sha=Sha256::new();
    for (item,assignment,evidence,output) in receipts {
        if digest(output.as_bytes())!=evidence {
            return Err("Worker approval no longer matches its latest output.".into());
        }
        for field in [&item,&assignment,&evidence] {
            sha.update((field.len() as u64).to_le_bytes());
            sha.update(field.as_bytes());
        }
    }
    Ok(format!("{:x}",sha.finalize()))
}

fn snapshot_hash(db:&rusqlite::Connection,root:&str,checkout:&Path)->Result<String> {
    let mut stmt=db.prepare(
        "SELECT item.id,proof.assignment_id,proof.evidence_sha256
         FROM sa_work_items item JOIN sa_work_item_evidence_receipts proof
           ON proof.item_id=item.id AND proof.child_ticket_id=item.child_ticket_id
         WHERE item.ticket_id=?1 AND item.state='completed'
         ORDER BY item.ordinal,item.id",
    ).map_err(|e|e.to_string())?;
    let rows=stmt.query_map([root],|r|Ok((
        r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,
    ))).map_err(|e|e.to_string())?;
    let receipts=rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e|e.to_string())?;
    if receipts.is_empty() {return Err("Completed Worker proofs are required.".into());}
    let (head,status,diff,_)=super::super::delivery::observe_git(checkout)?;
    let mut sha=Sha256::new();
    for (item,assignment,proof) in receipts {
        for value in [&item,&assignment,&proof] {
            sha.update((value.len() as u64).to_le_bytes());
            sha.update(value.as_bytes());
        }
    }
    for value in [&head,&status,&diff] {
        sha.update((value.len() as u64).to_le_bytes());
        sha.update(value.as_bytes());
    }
    Ok(format!("{:x}",sha.finalize()))
}

/// True only for an executed passing test against the current plan/checkout.
pub(in crate::superagent) fn verified(db:&rusqlite::Connection,root:&str)->Result<bool> {
    let row:Option<(String,String,String,String,String,String,String,String)>=db.query_row(
        "SELECT grant.checkout_root,grant.command,grant.manifest_sha256,
           run.plan_sha256,run.snapshot_sha256,plan.plan_sha256,run.state,
           run.worker_proofs_sha256
         FROM sa_demand_test_grants grant
         JOIN sa_demand_plans plan ON plan.ticket_id=grant.ticket_id
         JOIN sa_demand_test_runs run ON run.ticket_id=grant.ticket_id
         WHERE grant.ticket_id=?1",
        [root],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
            r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((checkout,command,hash,plan,snapshot,current,state,worker_proofs))=row else {return Ok(false)};
    if state!="passed" || plan!=current {return Ok(false);}
    let path=Path::new(&checkout);
    if !std::fs::canonicalize(path).is_ok_and(|p|p==path) {return Ok(false);}
    Ok(manifest_hash(path,&command).ok().as_deref()==Some(hash.as_str())
        && worker_proofs_hash(db,root).ok().as_deref()==Some(worker_proofs.as_str())
        && snapshot_hash(db,root,path).ok().as_deref()==Some(snapshot.as_str()))
}

#[derive(Clone)]
struct Reserved {root:String,command:String,checkout:String,manifest:String,snapshot:String,expires:i64,plan:String,db_path:std::path::PathBuf}
struct Outcome {state:&'static str,code:Option<i32>,stdout:String,stderr:String}

/// Native state + live effect gate; a receipt is reserved once and never
/// re-executed blindly after a crash, timeout or uncertain outcome.
fn reserve(store:&Store)->Result<Option<Reserved>> {
    let now=storage::now();
    let mut stmt=store.db.prepare(
        "SELECT grant.ticket_id,grant.command,grant.checkout_root,
           grant.manifest_sha256,m.expires_at,plan.plan_sha256
         FROM sa_demand_test_grants grant
         JOIN sa_demand_plans plan ON plan.ticket_id=grant.ticket_id
         JOIN sa_tickets root ON root.id=grant.ticket_id
         JOIN sa_mandates m ON m.ticket_id=root.id
         JOIN sa_orchestrations team ON team.id=root.orchestration_id
         WHERE plan.state='active' AND root.state='queued'
           AND root.active_assignment_id IS NULL
           AND root.revision=plan.ticket_revision
           AND root.policy_revision=grant.policy_revision
           AND root.project_id=grant.project_id
           AND m.revoked=0 AND m.expires_at>?1
           AND m.policy_revision=grant.policy_revision
           AND m.project_id=grant.project_id AND m.actor=root.actor
           AND m.allowed_tools='[]' AND m.allowed_side_effects=0
           AND team.enabled=1 AND team.policy_revision=grant.policy_revision
           AND team.project_id=grant.project_id
           AND EXISTS(SELECT 1 FROM sa_ticket_team_snapshots pinned
             WHERE pinned.ticket_id=root.id
               AND pinned.team_version=grant.team_version
               AND pinned.policy_revision=grant.policy_revision
               AND pinned.source_kind='native')
         ORDER BY plan.created_at",
    ).map_err(|e|e.to_string())?;
    let rows=stmt.query_map([now],|r|Ok((
        r.get::<_,String>(0)?,r.get::<_,String>(1)?,
        r.get::<_,String>(2)?,r.get::<_,String>(3)?,
        r.get::<_,i64>(4)?,r.get::<_,String>(5)?,
    ))).map_err(|e|e.to_string())?;
    // Inspect all eligible Demands: limiting before checking existing receipts
    // can starve an independent Demand behind already-passed runs.
    let candidates=rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e|e.to_string())?;
    drop(stmt);
    for (root,command,checkout,manifest,expires,plan) in candidates {
        if !super::super::auto::ticket_authorized(store,&root)?
            || super::dispatch::unsettled_root_effects(&store.db,&root)?
            || !super::dispatch::root_work_item_proofs_current(&store.db,&root)? {
            continue;
        }
        let incomplete:bool=store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_work_items
               WHERE ticket_id=?1 AND state<>'completed') OR
               EXISTS(SELECT 1 FROM sa_subagent_nodes
               WHERE root_ticket_id=?1 AND state<>'completed')",
            [&root],|r|r.get(0),
        ).map_err(|e|e.to_string())?;
        if incomplete {continue;}
        if !std::fs::canonicalize(&checkout).is_ok_and(|p|p==Path::new(&checkout))
            || manifest_hash(Path::new(&checkout),&command).ok().as_deref()
                !=Some(manifest.as_str()) {continue;}
        let Ok(snapshot)=snapshot_hash(&store.db,&root,Path::new(&checkout)) else {continue;};
        let Ok(worker_proofs)=worker_proofs_hash(&store.db,&root) else {continue;};
        // Keep the old terminal failure immutable before replacing the
        // current pointer. Reserve, archive and audit under one transaction.
        let tx=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
        if worker_proofs_hash(&tx,&root).ok().as_deref()!=Some(worker_proofs.as_str())
            || !retry_allowed(&tx,&root,&snapshot,&worker_proofs)? {continue;}
        let existing:bool=tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_demand_test_runs WHERE ticket_id=?1)",
            [&root],|r|r.get(0),
        ).map_err(|e|e.to_string())?;
        let n=if existing {
            let archived:i64=tx.query_row(
                "SELECT COUNT(*) FROM sa_demand_test_attempts WHERE ticket_id=?1",
                [&root],|r|r.get(0),
            ).map_err(|e|e.to_string())?;
            let history=tx.execute(
                "INSERT INTO sa_demand_test_attempts
                 (ticket_id,attempt,plan_sha256,snapshot_sha256,worker_proofs_sha256,
                  command,state,started_at,finished_at,exit_code,stdout_sha256,stderr_sha256)
                 SELECT ticket_id,?2,plan_sha256,snapshot_sha256,worker_proofs_sha256,command,state,
                        started_at,finished_at,exit_code,stdout_sha256,stderr_sha256
                 FROM sa_demand_test_runs
                 WHERE ticket_id=?1 AND state='failed' AND finished_at IS NOT NULL
                   AND snapshot_sha256<>?3 AND worker_proofs_sha256<>?4
                   AND length(worker_proofs_sha256)=64",
                params![root,archived+1,snapshot,worker_proofs],
            ).map_err(|e|e.to_string())?;
            if history!=1 {continue;}
            tx.execute(
                "UPDATE sa_demand_test_runs
                 SET plan_sha256=?2,snapshot_sha256=?3,command=?4,
                     worker_proofs_sha256=?6,
                     state='running',started_at=?5,finished_at=NULL,
                     exit_code=NULL,stdout_sha256=NULL,stderr_sha256=NULL
                 WHERE ticket_id=?1 AND state='failed'
                   AND snapshot_sha256<>?3 AND worker_proofs_sha256<>?6
                   AND EXISTS(SELECT 1 FROM sa_demand_plans plan
                     JOIN sa_tickets root ON root.id=plan.ticket_id
                     JOIN sa_mandates m ON m.ticket_id=root.id
                     WHERE root.id=?1 AND root.state='queued'
                       AND root.revision=plan.ticket_revision
                       AND plan.state='active' AND plan.plan_sha256=?2
                       AND m.revoked=0 AND m.expires_at>?5)",
                params![root,plan,snapshot,command,now,worker_proofs],
            ).map_err(|e|e.to_string())?
        } else {
            tx.execute(
                "INSERT INTO sa_demand_test_runs
                 (ticket_id,plan_sha256,snapshot_sha256,worker_proofs_sha256,
                  command,state,started_at)
                 SELECT grant.ticket_id,?2,?3,?5,grant.command,'running',?4
                 FROM sa_demand_test_grants grant
                 JOIN sa_demand_plans plan ON plan.ticket_id=grant.ticket_id
                 JOIN sa_tickets ticket ON ticket.id=grant.ticket_id
                 JOIN sa_mandates m ON m.ticket_id=ticket.id
                 WHERE grant.ticket_id=?1 AND plan.state='active'
                   AND plan.plan_sha256=?2 AND ticket.state='queued'
                   AND m.revoked=0 AND m.expires_at>?4
                   AND NOT EXISTS(SELECT 1 FROM sa_demand_test_runs WHERE ticket_id=?1)",
                params![root,plan,snapshot,now,worker_proofs],
            ).map_err(|e|e.to_string())?
        };
        if n==1 {
            tx.execute(
                "INSERT INTO sa_audit(ticket_id,kind,detail,at)
                 VALUES(?1,'demand_test_started',?2,?3)",
                params![root,format!("Native {command}; pinned WorkItems and Git snapshot"),now],
            ).map_err(|e|e.to_string())?;
            tx.commit().map_err(|e|e.to_string())?;
            return Ok(Some(Reserved{root,command,checkout,manifest,snapshot,expires,plan,db_path:store.path.clone()}));
        }
    }
    Ok(None)
}

/// Retest only a new proof+Git snapshot after an explicitly failed,
/// settled test. A running/uncertain effect must be reconciled, not replayed.
fn retry_allowed(db:&rusqlite::Connection,root:&str,new_snapshot:&str,
    new_worker_proofs:&str)->Result<bool> {
    let previous:Option<(String,String,String)>=db.query_row(
        "SELECT state,snapshot_sha256,worker_proofs_sha256
         FROM sa_demand_test_runs WHERE ticket_id=?1",
        [root],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((state,old_snapshot,old_worker_proofs))=previous else {return Ok(true)};
    if state!="failed" || old_snapshot==new_snapshot
        || old_worker_proofs==new_worker_proofs
        || old_worker_proofs.len()!=64 || new_worker_proofs.len()!=64 {
        return Ok(false);
    }
    let archived:i64=db.query_row(
        "SELECT COUNT(*) FROM sa_demand_test_attempts WHERE ticket_id=?1",
        [root],|r|r.get(0),
    ).map_err(|e|e.to_string())?;
    Ok(archived<2)
}

/// Native mandate deadlines are Unix milliseconds, while process timeouts
/// are Durations. Never interpret an allowed lifetime as whole seconds.
fn bounded_test_timeout(now_ms:i64,expires_at_ms:i64)->Option<Duration> {
    let remaining_ms=expires_at_ms.checked_sub(now_ms)?;
    (remaining_ms>0).then(||Duration::from_millis(remaining_ms as u64)
        .min(Duration::from_secs(120)))
}

/// Re-check the frozen native plan, Team and mandate from a separate
/// read-only connection while a test child is running. Stop, expiry, policy
/// changes and failed DB reads all cancel the live process group.
fn test_effect_live(db:&rusqlite::Connection,job:&Reserved)->bool {
    let now=storage::now();
    if now>=job.expires {return false;}
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets root
          JOIN sa_mandates m ON m.ticket_id=root.id
          JOIN sa_demand_plans plan ON plan.ticket_id=root.id
          JOIN sa_orchestrations team ON team.id=root.orchestration_id
          WHERE root.id=?1 AND root.state='queued'
            AND root.active_assignment_id IS NULL
            AND root.revision=plan.ticket_revision
            AND plan.state='active' AND plan.plan_sha256=?2
            AND root.policy_revision=plan.policy_revision
            AND m.revoked=0 AND m.expires_at>?3
            AND m.expires_at=?4
            AND m.policy_revision=plan.policy_revision
            AND m.actor=root.actor AND m.project_id=root.project_id
            AND m.allowed_tools='[]' AND m.allowed_side_effects=0
            AND team.enabled=1 AND team.policy_revision=plan.policy_revision
            AND team.project_id=root.project_id)",
        params![job.root,job.plan,now,job.expires],
        |r|r.get(0),
    ).unwrap_or(false)
}

/// Run test scripts only inside a real, separately confined OS process.
/// A package manager's offline switch does not restrict its scripts. The
/// checkout is read-only, HOME and temporary writes are private, the ambient
/// environment is empty, and the process has no network namespace access.
/// Until native sandbox adapters exist, non-Linux platforms are unsupported
/// rather than launching unrestricted host-side commands.
#[cfg(target_os="linux")]
fn linux_sandbox(checkout:&str)->Result<Command> {
        if !Path::new("/usr/bin/bwrap").is_file() {
            return Err("Native test sandbox unavailable: bubblewrap is required.".into());
        }
        if !Path::new(checkout).is_absolute() ||
            !std::fs::canonicalize(checkout)
                .is_ok_and(|path|path==Path::new(checkout)) {
            return Err("Native test sandbox requires a canonical checkout.".into());
        }
        let mut command=Command::new("/usr/bin/bwrap");
        command.env_clear();
        command.args([
            "--die-with-parent","--unshare-all",
            "--ro-bind","/usr","/usr",
            "--symlink","usr/bin","/bin",
            "--symlink","usr/lib","/lib",
            "--symlink","usr/lib64","/lib64",
            "--dir","/workspace","--ro-bind",checkout,"/workspace",
            "--tmpfs","/tmp","--dir","/home",
            "--proc","/proc","--dev","/dev",
            "--clearenv","--setenv","HOME","/tmp",
            "--setenv","PATH","/usr/bin:/bin",
            "--setenv","TMPDIR","/tmp",
            "--setenv","CARGO_NET_OFFLINE","true",
            "--setenv","CARGO_TARGET_DIR","/tmp/target",
            "--setenv","npm_config_offline","true",
            "--chdir","/workspace",
        ]);
        Ok(command)
}

fn sandboxed_test_command(job:&Reserved)->Result<Command> {
    #[cfg(target_os="linux")]
    {
        let mut command=linux_sandbox(&job.checkout)?;
        match job.command.as_str() {
            "npm test" if Path::new("/usr/bin/npm").is_file() =>
                {command.args(["--","/usr/bin/npm","test","--silent"]);},
            "cargo test" if Path::new("/usr/bin/cargo").is_file() =>
                {command.args(["--","/usr/bin/cargo","test","--locked","--offline"]);},
            _=>return Err("The selected native test tool is unsupported in the isolated system runtime.".into()),
        }
        Ok(command)
    }
    #[cfg(not(target_os="linux"))]
    {
        let _=job;
        Err("No independently confined native test runner is supported on this platform.".into())
    }
}

fn execute(job:&Reserved)->Outcome {
    let blank=digest(b"");
    let Some(timeout)=bounded_test_timeout(storage::now(),job.expires) else {
        return Outcome{state:"uncertain",code:None,stdout:blank.clone(),stderr:blank};
    };
    let Ok(cmd)=sandboxed_test_command(job) else {
        // Never invoke arbitrary npm/cargo scripts with ambient host rights.
        return Outcome{state:"failed",code:None,stdout:blank.clone(),stderr:blank};
    };
    let Ok(read_only)=Store::open_read_only(&job.db_path) else {
        return Outcome{state:"uncertain",code:None,stdout:blank.clone(),stderr:blank};
    };
    match crate::workspace::run_process_guarded(
        cmd,"Demand test",64*1024,timeout,
        ||test_effect_live(&read_only.db,job),
    ) {
        Ok(output)=>Outcome {
            state:if output.status.success() {"passed"} else {"failed"},
            code:output.status.code(),
            stdout:digest(&output.stdout.bytes),stderr:digest(&output.stderr.bytes),
        },
        Err(_)=>Outcome{state:"uncertain",code:None,stdout:blank.clone(),stderr:blank},
    }
}

fn settle(store:&Store,job:&Reserved,result:Outcome)->Result<()> {
    let now=storage::now();
    // Use the same effect-time authorization predicate at settlement.
    // A changed plan, grant, active assignment or tool scope must never
    // turn a cancelled process into durable passing test evidence.
    let current=test_effect_live(&store.db,job)
        && manifest_hash(Path::new(&job.checkout),&job.command)
            .ok().as_deref()==Some(job.manifest.as_str())
        && snapshot_hash(&store.db,&job.root,Path::new(&job.checkout))
            .ok().as_deref()==Some(job.snapshot.as_str());
    let state=if current {result.state} else {"uncertain"};
    let n=store.db.execute(
        "UPDATE sa_demand_test_runs
         SET state=?2,exit_code=?3,stdout_sha256=?4,stderr_sha256=?5,finished_at=?6
         WHERE ticket_id=?1 AND state='running' AND snapshot_sha256=?7",
        params![job.root,state,result.code,result.stdout,result.stderr,now,job.snapshot],
    ).map_err(|e|e.to_string())?;
    if n==1 {
        store.db.execute(
            "INSERT INTO sa_audit(ticket_id,kind,detail,at)
             VALUES(?1,'demand_test_settled',?2,?3)",
            params![job.root,format!("Native {}: {state}; exit {:?}",job.command,result.code),now],
        ).map_err(|e|e.to_string())?;
    }
    Ok(())
}

/// The scheduler reserves before the external effect and settles under the
/// existing native dispatch fence. No renderer/model can invoke this directly.
pub(super) async fn advance(app:&tauri::AppHandle)->Result<usize> {
    let reserved={
        let state=app.state::<AppState>();
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let _fence=super::super::effects::lock_dispatch(&store.path)?;
        reserve(&store)?
    };
    let Some(job)=reserved else {return Ok(0)};
    let for_process=job.clone();
    let outcome=tauri::async_runtime::spawn_blocking(move ||execute(&for_process))
        .await.map_err(|e|e.to_string())?;
    let state=app.state::<AppState>();
    let store=state.store.lock().map_err(|e|e.to_string())?;
    let _fence=super::super::effects::lock_dispatch(&store.path)?;
    settle(&store,&job,outcome)?;
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demand_test_deadline_uses_milliseconds_without_expanding_authority() {
        let now=1_700_000_000_000_i64;
        assert_eq!(bounded_test_timeout(now,now),None);
        assert_eq!(bounded_test_timeout(now,now-1),None);
        assert_eq!(bounded_test_timeout(now,now+50),Some(Duration::from_millis(50)));
        assert_eq!(bounded_test_timeout(now,now+3_000),Some(Duration::from_secs(3)));
        assert_eq!(bounded_test_timeout(now,now+180_000),Some(Duration::from_secs(120)));
        assert_eq!(bounded_test_timeout(i64::MIN,i64::MAX),None);
    }

    #[cfg(target_os="linux")]
    #[test]
    fn native_test_sandbox_denies_external_files_network_and_ambient_secrets() {
        use std::net::TcpListener;
        // CI installs bubblewrap; unsupported local hosts fail closed.
        if !Path::new("/usr/bin/bwrap").is_file() {return;}
        let root=std::env::temp_dir().join(format!("native-test-sandbox-{}",storage::id()));
        let external=std::env::temp_dir().join(format!("native-test-external-{}",storage::id()));
        std::fs::create_dir_all(&root).unwrap();
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();
        let port=listener.local_addr().unwrap().port();
        let script=format!(
            "echo SANDBOX_STARTED; if echo forbidden > /workspace/denied.txt 2>/dev/null; then echo FILE_ALLOWED; else echo FILE_DENIED; fi; echo private > {}; if /usr/bin/printenv FULLAUTO_TEST_SECRET >/dev/null 2>&1; then echo ENV_ALLOWED; else echo ENV_DENIED; fi; if echo ping > /dev/tcp/127.0.0.1/{} 2>/dev/null; then echo NET_ALLOWED; else echo NET_DENIED; fi",
            external.display(),port,
        );
        let mut command=linux_sandbox(root.to_str().unwrap()).unwrap();
        command.env("FULLAUTO_TEST_SECRET","must-not-be-inherited");
        command.args(["--","/usr/bin/bash","-c",&script]);
        let output=crate::workspace::run_process(
            command,"Sandbox test",16*1024,Duration::from_secs(10),
        ).unwrap();
        let stdout=String::from_utf8_lossy(&output.stdout.bytes);
        assert!(output.status.success(),"{stdout}");
        for witness in ["SANDBOX_STARTED","FILE_DENIED","ENV_DENIED","NET_DENIED"] {
            assert!(stdout.contains(witness),"missing {witness}: {stdout}");
        }
        assert!(!external.exists());
        assert!(!root.join("denied.txt").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unsupported_or_unconfined_native_process_never_falls_back_to_host_execution() {
        let job=Reserved {
            root:"root".into(),command:"sh -c npm test".into(),
            checkout:"/tmp".into(),manifest:"hash".into(),
            snapshot:"snapshot".into(),expires:storage::now()+60_000,
            plan:"plan".into(),db_path:std::path::PathBuf::new(),
        };
        assert!(sandboxed_test_command(&job).is_err());
        #[cfg(not(target_os="linux"))]
        {
            let mut valid=job;
            valid.command="npm test".into();
            assert!(sandboxed_test_command(&valid).is_err());
        }
    }

    #[test]
    fn revoked_or_expired_native_test_cannot_reuse_an_old_plan() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_tickets(
               id TEXT PRIMARY KEY,orchestration_id TEXT,actor TEXT,project_id TEXT,
               state TEXT,revision INTEGER,policy_revision INTEGER,active_assignment_id TEXT);
             CREATE TABLE sa_mandates(
               ticket_id TEXT,revoked INTEGER,expires_at INTEGER,policy_revision INTEGER,
               actor TEXT,project_id TEXT,allowed_tools TEXT,allowed_side_effects INTEGER);
             CREATE TABLE sa_demand_plans(
               ticket_id TEXT,state TEXT,plan_sha256 TEXT,ticket_revision INTEGER,
               policy_revision INTEGER);
             CREATE TABLE sa_orchestrations(
               id TEXT,enabled INTEGER,policy_revision INTEGER,project_id TEXT);
             INSERT INTO sa_orchestrations VALUES('team',1,2,'project');
             INSERT INTO sa_tickets VALUES
               ('root','team','local-user','project','queued',7,2,NULL);
             INSERT INTO sa_demand_plans VALUES('root','active','plan',7,2);",
        ).unwrap();
        let expires=storage::now()+60_000;
        db.execute(
            "INSERT INTO sa_mandates VALUES
              ('root',0,?1,2,'local-user','project','[]',0)",[expires],
        ).unwrap();
        let job=Reserved {
            root:"root".into(),command:"npm test".into(),checkout:"/project".into(),
            manifest:"hash".into(),snapshot:"snapshot".into(),
            expires,plan:"plan".into(),db_path:std::path::PathBuf::new(),
        };
        assert!(test_effect_live(&db,&job));
        db.execute("UPDATE sa_mandates SET revoked=1",[]).unwrap();
        assert!(!test_effect_live(&db,&job));
        db.execute("UPDATE sa_mandates SET revoked=0",[]).unwrap();
        db.execute("UPDATE sa_demand_plans SET plan_sha256='stale'",[]).unwrap();
        assert!(!test_effect_live(&db,&job));
        db.execute("UPDATE sa_demand_plans SET plan_sha256='plan'",[]).unwrap();
        db.execute("UPDATE sa_tickets SET state='stopped'",[]).unwrap();
        assert!(!test_effect_live(&db,&job));
        db.execute("UPDATE sa_tickets SET state='queued'",[]).unwrap();
        db.execute("UPDATE sa_tickets SET active_assignment_id='other'",[]).unwrap();
        assert!(!test_effect_live(&db,&job));
        db.execute("UPDATE sa_tickets SET active_assignment_id=NULL",[]).unwrap();
        db.execute("UPDATE sa_mandates SET allowed_tools='[\"shell\"]'",[]).unwrap();
        assert!(!test_effect_live(&db,&job));
        db.execute("UPDATE sa_mandates SET allowed_tools='[]'",[]).unwrap();
        db.execute("UPDATE sa_tickets SET revision=8",[]).unwrap();
        assert!(!test_effect_live(&db,&job));
        db.execute("UPDATE sa_tickets SET revision=7",[]).unwrap();
        db.execute("UPDATE sa_mandates SET expires_at=expires_at+1000",[]).unwrap();
        assert!(!test_effect_live(&db,&job));
    }
    #[test]
    fn test_command_is_literal_and_manifest_bound() {
        let root=std::env::temp_dir().join(format!("sa-test-{}",storage::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("package.json"),r#"{"scripts":{"test":"vitest run"}}"#).unwrap();
        assert_eq!(manifest_hash(&root,"npm test").unwrap().len(),64);
        for invalid in ["npm run build","npm test; curl example.com",
            "cargo test -- --nocapture","sh -c npm test"] {
            assert!(manifest_hash(&root,invalid).is_err());
        }
        std::fs::write(root.join("package.json"),r#"{"scripts":{"test":""}}"#).unwrap();
        assert!(manifest_hash(&root,"npm test").is_err());
        std::fs::write(root.join("Cargo.toml"),"[package]\nname=\"fixture\"\nversion=\"0.1.0\"\n").unwrap();
        assert!(manifest_hash(&root,"cargo test").is_err());
        std::fs::write(root.join("Cargo.lock"),"version = 3\n").unwrap();
        assert_eq!(manifest_hash(&root,"cargo test").unwrap().len(),64);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn full_auto_requires_executed_native_test_evidence_even_without_a_grant() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_tickets(id TEXT PRIMARY KEY);
             INSERT INTO sa_tickets VALUES('root');
             CREATE TABLE sa_ticket_team_snapshots(
               ticket_id TEXT,source_kind TEXT,snapshot_json TEXT);
             INSERT INTO sa_ticket_team_snapshots VALUES
               ('root','native','{\"policy\":{\"autonomy\":\"full_auto\"}}');
             CREATE TABLE sa_demand_plans(ticket_id TEXT,plan_sha256 TEXT);
             INSERT INTO sa_demand_plans VALUES('root','plan');
             CREATE TABLE sa_work_items(ticket_id TEXT, item_key TEXT, state TEXT);
             INSERT INTO sa_work_items VALUES
               ('root','plan','completed'),('root','implementation','completed'),
               ('root','review','completed');
            ",
        ).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        let finished:i64=db.query_row(
            "SELECT COUNT(*) FROM sa_work_items
             WHERE ticket_id='root' AND state='completed'",
            [],|r|r.get(0),
        ).unwrap();
        assert_eq!(finished,3,"Three completed WorkItems are not test evidence");
        assert!(required(&db,"root").unwrap());
        assert!(!required(&db,"unrelated").unwrap());
        assert!(!verified(&db,"root").unwrap());
        db.execute(
            "UPDATE sa_ticket_team_snapshots
             SET snapshot_json='{\"policy\":{\"autonomy\":\"approval\"}}'
             WHERE ticket_id='root'",[],
        ).unwrap();
        assert!(!required(&db,"root").unwrap());
        db.execute(
            "INSERT INTO sa_demand_test_grants VALUES(
              'root',1,1,'project','/missing','npm test',?1,100)",
            ["f".repeat(64)],
        ).unwrap();
        assert!(required(&db,"root").unwrap());
        assert!(!verified(&db,"root").unwrap());
    }

    #[test]
    fn failed_tests_require_new_evidence_and_have_finite_durable_retries() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_demand_test_runs(
               ticket_id TEXT PRIMARY KEY,plan_sha256 TEXT,snapshot_sha256 TEXT,
               worker_proofs_sha256 TEXT,command TEXT,state TEXT,started_at INTEGER,
               finished_at INTEGER,exit_code INTEGER,stdout_sha256 TEXT,stderr_sha256 TEXT);
             CREATE TABLE sa_demand_test_attempts(
               ticket_id TEXT NOT NULL,attempt INTEGER NOT NULL,
               plan_sha256 TEXT,snapshot_sha256 TEXT,worker_proofs_sha256 TEXT,
               command TEXT,state TEXT,started_at INTEGER,finished_at INTEGER,
               exit_code INTEGER,stdout_sha256 TEXT,stderr_sha256 TEXT,
               PRIMARY KEY(ticket_id,attempt));",
        ).unwrap();
        let first="a".repeat(64);
        assert!(retry_allowed(&db,"demand","first",&first).unwrap());
        db.execute(
            "INSERT INTO sa_demand_test_runs VALUES(
               'demand','plan','first',?1,'npm test','failed',1,2,1,'a','b')",
            [&first],
        ).unwrap();
        assert!(!retry_allowed(&db,"demand","first",&"b".repeat(64)).unwrap());
        assert!(!retry_allowed(&db,"demand","changed-git",&first).unwrap());
        assert!(retry_allowed(&db,"demand","second",&"b".repeat(64)).unwrap());
        for (attempt,old,new,next_worker) in [
            (1,"first","second","b".repeat(64)),
            (2,"second","third","c".repeat(64)),
        ] {
            db.execute(
                "INSERT INTO sa_demand_test_attempts
                 SELECT ticket_id,?1,plan_sha256,snapshot_sha256,worker_proofs_sha256,
                        command,state,started_at,finished_at,exit_code,
                        stdout_sha256,stderr_sha256
                 FROM sa_demand_test_runs WHERE ticket_id='demand'",
                [attempt],
            ).unwrap();
            db.execute(
                "UPDATE sa_demand_test_runs SET snapshot_sha256=?1,
                    worker_proofs_sha256=?2,state='failed',
                    started_at=started_at+2,finished_at=finished_at+2
                 WHERE ticket_id='demand'",
                params![new,next_worker],
            ).unwrap();
            let archived:String=db.query_row(
                "SELECT snapshot_sha256 FROM sa_demand_test_attempts
                 WHERE ticket_id='demand' AND attempt=?1",
                [attempt],|r|r.get(0),
            ).unwrap();
            assert_eq!(archived,old);
            assert!(!retry_allowed(&db,"demand",new,&"d".repeat(64)).unwrap());
        }
        assert!(!retry_allowed(&db,"demand","fourth",&"d".repeat(64)).unwrap());
        db.execute("UPDATE sa_demand_test_runs SET state='uncertain'",[]).unwrap();
        assert!(!retry_allowed(&db,"demand","fifth",&"e".repeat(64)).unwrap());
    }

    #[test]
    fn unrelated_git_changes_cannot_spend_a_worker_repair_attempt() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE sa_demand_test_runs(
                ticket_id TEXT PRIMARY KEY,state TEXT,snapshot_sha256 TEXT,
                worker_proofs_sha256 TEXT);
             CREATE TABLE sa_demand_test_attempts(ticket_id TEXT);
             INSERT INTO sa_demand_test_runs VALUES(
                 'root','failed','original','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');",
        ).unwrap();
        let worker_a="a".repeat(64);
        let worker_b="b".repeat(64);
        assert!(!retry_allowed(&db,"root","changed-git-only",&worker_a).unwrap());
        assert!(retry_allowed(&db,"root","changed-with-new-worker",&worker_b).unwrap());
        db.execute("UPDATE sa_demand_test_runs SET state='uncertain'",[]).unwrap();
        assert!(!retry_allowed(&db,"root","changed-with-new-worker",&worker_b).unwrap());
        db.execute("UPDATE sa_demand_test_runs SET state='passed'",[]).unwrap();
        assert!(!retry_allowed(&db,"root","changed-with-new-worker",&worker_b).unwrap());
    }

    #[test]
    fn immutable_grants_and_replay_fail_closed() {
        let db=rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE sa_tickets(id TEXT PRIMARY KEY,orchestration_id TEXT,request_key TEXT);
          INSERT INTO sa_tickets VALUES('ticket','team','request');").unwrap();
        db.execute_batch(SCHEMA).unwrap();
        let hash="a".repeat(64);
        db.execute("INSERT INTO sa_demand_test_grants
          (ticket_id,team_version,policy_revision,project_id,checkout_root,command,
           manifest_sha256,authorized_at)
          VALUES('ticket',1,2,'project','/repo','npm test',?1,100)",[&hash]).unwrap();
        assert!(replay_matches(&db,"team","request",
            Some((1,2,"project","/repo","npm test",&hash))).unwrap());
        assert!(replay_matches(&db,"team","request",
            Some((1,2,"project","/repo","cargo test",&hash))).is_err());
        assert!(replay_matches(&db,"team","request",None).is_err());
        assert!(db.execute("UPDATE sa_demand_test_grants SET command='cargo test'",[]).is_err());
        assert!(db.execute("DELETE FROM sa_demand_test_grants",[]).is_err());
    }
}
