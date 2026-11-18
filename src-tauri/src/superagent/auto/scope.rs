//! Trusted, finite source authorization for GitHub Auto Demands.
//! Neither an issue body nor a Coordinator proposal can create or widen this
//! grant. The native user confirms paths and test class once, before polling.

use super::*;
use rusqlite::OptionalExtension;
use std::path::Path;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sa_auto_native_scopes (
  source_id TEXT NOT NULL REFERENCES sa_auto_source_grants(source_id) ON DELETE RESTRICT,
  source_revision INTEGER NOT NULL CHECK(source_revision>0),
  team_version INTEGER NOT NULL CHECK(team_version>0),
  policy_revision INTEGER NOT NULL CHECK(policy_revision>0),
  project_id TEXT NOT NULL,
  checkout_root TEXT NOT NULL,
  allowed_paths TEXT NOT NULL CHECK(json_valid(allowed_paths)),
  test_command TEXT NOT NULL CHECK(test_command IN ('npm test','cargo test')),
  test_manifest_sha256 TEXT NOT NULL CHECK(length(test_manifest_sha256)=64),
  authorized_at INTEGER NOT NULL,
  PRIMARY KEY(source_id,source_revision)
);
CREATE TRIGGER IF NOT EXISTS sa_auto_native_scope_immutable
 BEFORE UPDATE ON sa_auto_native_scopes
 BEGIN SELECT RAISE(ABORT,'GitHub Auto source scope is immutable'); END;
CREATE TRIGGER IF NOT EXISTS sa_auto_native_scope_no_delete
 BEFORE DELETE ON sa_auto_native_scopes
 BEGIN SELECT RAISE(ABORT,'GitHub Auto source scope is retained as consent evidence'); END;
"#;

#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct NativeScope {
    pub team_version:i64,
    pub policy_revision:i64,
    pub project_id:String,
    pub checkout_root:String,
    pub allowed_paths:Vec<String>,
    pub command:String,
    pub manifest:String,
}

fn pin_checkout(store:&Store,project:&str)->Result<String> {
    let root=store.workspace_root(project,None)?;
    let canonical=std::fs::canonicalize(root)
        .map_err(|_|"GitHub Auto's project checkout is unavailable.")?;
    Ok(canonical.to_string_lossy().into_owned())
}

/// Validate a new *local user* decision without changing the database.
/// A Team's Full Access policy cannot silently become read-only automation.
pub(super) fn preflight(store:&Store,src:&events::EventSource,
    paths:&[String],command:&str)->Result<Option<NativeScope>> {
    let team=configuration(store,&src.orchestration_id)?;
    if paths.is_empty() && command.is_empty() {return Ok(None);}
    if paths.is_empty() || command.is_empty() {
        return Err("Auto source execution consent must include BOTH exact file paths and a native test command.".into());
    }
    if !team.enabled || !crate::workspace::scoped_writes_supported() {
        return Err("Auto scoped files are unsupported or the Team is disabled.".into());
    }
    let eligible=team.worker_bot_ids.iter().any(|id| {
        crate::bots::get(store,id).is_ok_and(|bot|
            bot.provider=="codex" && !bot.paused && bot.project_id==team.project_id)
    });
    if !eligible {
        return Err("No native Codex Worker is eligible for scoped Auto source files.".into());
    }
    let checkout=pin_checkout(store,&team.project_id)?;
    super::super::write_grants::validate_paths_in_root(Path::new(&checkout),paths)?;
    let manifest=super::super::scheduler::test_runner::manifest_hash(
        Path::new(&checkout),command,
    )?;
    Ok(Some(NativeScope {
        team_version:team.team_version,
        policy_revision:team.policy_revision,
        project_id:team.project_id,
        checkout_root:checkout,
        allowed_paths:paths.to_vec(),
        command:command.into(),
        manifest,
    }))
}

/// Persist only after the native confirmation and source-grant insertion,
/// in the SAME transaction. Reauthorization cannot widen an existing scope.
pub(super) fn persist(db:&rusqlite::Connection,src:&events::EventSource,
    proposed:Option<&NativeScope>,now:i64)->Result<()> {
    let prior:Option<(i64,i64,String,String,String,String,String)>=db.query_row(
        "SELECT team_version,policy_revision,project_id,checkout_root,
            allowed_paths,test_command,test_manifest_sha256
         FROM sa_auto_native_scopes WHERE source_id=?1 AND source_revision=?2",
        params![src.id,src.policy_revision],
        |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,
            row.get(4)?,row.get(5)?,row.get(6)?)),
    ).optional().map_err(|e|e.to_string())?;
    let encoded=proposed.map(|s|serde_json::to_string(&s.allowed_paths))
        .transpose().map_err(|e|e.to_string())?;
    match (prior,proposed) {
        (None,None)=>Ok(()),
        (Some(_),None)=>Err("GitHub Auto scope cannot be dropped by reauthorization.".into()),
        (Some((v,p,project,checkout,paths,command,manifest)),Some(s)) =>
            if v==s.team_version && p==s.policy_revision && project==s.project_id
                && checkout==s.checkout_root && paths==encoded.as_deref().unwrap_or("")
                && command==s.command && manifest==s.manifest {
                Ok(())
            } else {
                Err("GitHub Auto scope cannot change within the original source revision.".into())
            },
        (None,Some(s))=>{
            let count=db.execute(
                "INSERT INTO sa_auto_native_scopes(
                   source_id,source_revision,team_version,policy_revision,
                   project_id,checkout_root,allowed_paths,test_command,
                   test_manifest_sha256,authorized_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![src.id,src.policy_revision,s.team_version,s.policy_revision,
                    s.project_id,s.checkout_root,encoded,s.command,s.manifest,now],
            ).map_err(|e|e.to_string())?;
            if count!=1 {return Err("GitHub Auto source consent was not persisted.".into());}
            Ok(())
        }
    }
}

/// Return a source's original exact consent only if the source and Team are
/// still live and its current checkout/test manifest match immutable bytes.
pub(super) fn current(store:&Store,src:&events::EventSource)->Result<Option<NativeScope>> {
    if active_grant(store,src,storage::now())?.is_none() {return Ok(None);}
    let row:Option<(i64,i64,String,String,String,String,String)>=store.db.query_row(
        "SELECT team_version,policy_revision,project_id,checkout_root,
           allowed_paths,test_command,test_manifest_sha256
         FROM sa_auto_native_scopes WHERE source_id=?1 AND source_revision=?2",
        params![src.id,src.policy_revision],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
            r.get(4)?,r.get(5)?,r.get(6)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((version,policy,project,checkout,encoded,command,manifest))=row
        else {return Ok(None)};
    let team=configuration(store,&src.orchestration_id)?;
    if !team.enabled || team.team_version!=version ||
        team.policy_revision!=policy || team.project_id!=project ||
        pin_checkout(store,&project)?!=checkout {
        return Err("Auto source Team or checkout changed since local authorization.".into());
    }
    let paths:Vec<String>=serde_json::from_str(&encoded)
        .map_err(|_|"Auto source file consent is malformed.")?;
    super::super::write_grants::validate_paths_in_root(Path::new(&checkout),&paths)?;
    if super::super::scheduler::test_runner::manifest_hash(
        Path::new(&checkout),&command,
    )?.as_str()!=manifest {
        return Err("Auto source test manifest changed since local authorization.".into());
    }
    Ok(Some(NativeScope {
        team_version:version,policy_revision:policy,project_id:project,
        checkout_root:checkout,allowed_paths:paths,command,manifest,
    }))
}

pub(super) fn ready(store:&Store,src:&events::EventSource)->Result<bool> {
    if active_grant(store,src,storage::now())?.is_none() {return Ok(false);}
    let team=configuration(store,&src.orchestration_id)?;
    // A valid authenticated source grant is enough for the native bot's
    // Full Access. An optional legacy scope is still checked if present.
    let _ = current(store,src)?;
    Ok(true)
}

/// The Coordinator is STILL read-only. Before delivering its first message,
/// copy the previously confirmed source scope to the *new immutable Demand*.
/// A failure must stop the Ticket; no issue/model text can supply this scope.
pub(super) fn issue_for_ticket(store:&Store,src:&events::EventSource,
    ticket_id:&str)->Result<Vec<String>> {
    let team=configuration(store,&src.orchestration_id)?;
    let scope=current(store,src)?;
    let Some(scope)=scope else {return Ok(Vec::new());};
    if !ticket_authorized(store,ticket_id)? {
        return Err("Auto root source authority was revoked before Demand scope issuance.".into());
    }
    super::super::scheduler::issue_initial_demand_write_scope(
        store,ticket_id,&scope.allowed_paths,scope.team_version,
        scope.policy_revision,&scope.project_id,
    )?;
    super::super::scheduler::test_runner::issue(
        store,ticket_id,super::super::scheduler::test_runner::TestGrant {
            team_version:scope.team_version,policy_revision:scope.policy_revision,
            project_id:&scope.project_id,checkout_root:&scope.checkout_root,
            command:&scope.command,manifest_sha256:&scope.manifest,
        },
    )?;
    Ok(scope.allowed_paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_auto_source_scope_is_immutable_exact_and_requires_source_grant() {
        let (store,src)=super::super::tests::fixture();
        let now=storage::now();
        let candidate=NativeScope {
            team_version:3,policy_revision:2,project_id:"project".into(),
            checkout_root:"/pinned".into(),
            allowed_paths:vec!["src/a.rs".into()],
            command:"cargo test".into(),manifest:"a".repeat(64),
        };
        assert!(persist(&store.db,&src,Some(&candidate),now).is_err(),
            "Without the source grant the native scope must have no authority");
        store.db.execute(
            "INSERT INTO sa_auto_source_grants(
                source_id,source_revision,repository,actor_filter,event_kind,
                operator_login,max_daily_tasks,authorized_at,expires_at)
             VALUES('source',1,'owner/repo','contributor','issues.opened',
                'operator',1,?1,?2)",
            params![now-10,now+60_000],
        ).unwrap();
        persist(&store.db,&src,Some(&candidate),now).unwrap();
        persist(&store.db,&src,Some(&candidate),now).unwrap();
        let mut widened=candidate.clone();
        widened.allowed_paths.push("secrets.txt".into());
        assert!(persist(&store.db,&src,Some(&widened),now).is_err());
        assert!(persist(&store.db,&src,None,now).is_err());
        assert!(store.db.execute(
            "UPDATE sa_auto_native_scopes SET allowed_paths='[]'",
            [],
        ).is_err());
        assert!(store.db.execute(
            "DELETE FROM sa_auto_native_scopes",[],
        ).is_err());
        store.db.execute(
            "UPDATE sa_auto_source_grants SET revoked_at=?1 WHERE source_id='source'",
            [now],
        ).unwrap();
        assert!(current(&store,&src).unwrap().is_none(),
            "Revoked source cannot carry reusable file/test authority");
    }
}
