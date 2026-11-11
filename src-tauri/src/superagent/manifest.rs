//! Portable, configuration-only SuperAgent companion manifest.
//! Integrates with existing native Bot identities without importing credentials,
//! conversations, tasks, mandates or any other runtime/authority state.
//! Import preflights every reference, then commits only disabled new presets.

use super::Orchestration;
use crate::{bots, storage::{self, Store}, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};
use tauri::{Manager, State};

const MAX_BYTES: usize = 1024 * 1024;
const MAX_PRESETS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    orchestrations: Vec<Preset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Preset {
    name: String,
    coordinator_bot: String,
    worker_bot: String,
    enabled: bool,
    autonomy: String,
    max_concurrent_tasks: u8,
    max_attempts_per_assignment: u8,
    council: Council,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Council {
    enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub would_create: Vec<String>,
    pub unchanged: Vec<String>,
    /// Explicitly warn even when the exported source configuration was enabled.
    pub imported_presets_disabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub created: Vec<String>,
    pub unchanged: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
}

#[derive(Clone)]
struct Resolved {
    name: String,
    coordinator_id: String,
    worker_id: String,
    project_id: String,
    existing: bool,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.trim() == name && name.len() <= 120
        && !name.chars().any(char::is_control)
}

fn resolve(store: &Store, document: &serde_json::Value) -> Result<Vec<Resolved>> {
    let bytes = serde_json::to_vec(document).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("SuperAgent manifest exceeds the 1 MB limit.".into());
    }
    let manifest: Manifest = serde_json::from_value(document.clone())
        .map_err(|e| format!("Invalid SuperAgent companion manifest: {e}"))?;
    if manifest.schema_version != 1 || manifest.orchestrations.len() > MAX_PRESETS {
        return Err("Unsupported SuperAgent manifest schema or too many presets.".into());
    }
    let bots = bots::list(store)?;
    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(manifest.orchestrations.len());
    for preset in &manifest.orchestrations {
        if !valid_name(&preset.name) || !seen.insert(preset.name.to_ascii_lowercase()) {
            return Err("Invalid or duplicate orchestration name in manifest.".into());
        }
        // Only manual, Council-off, bounded single-worker presets are currently
        // executable. Reject unsupported policy instead of partially importing it.
        if preset.autonomy != "off" || preset.max_concurrent_tasks != 1
            || preset.max_attempts_per_assignment != 2 || preset.council.enabled
        {
            return Err("Unsupported SuperAgent autonomy, Council or execution policy.".into());
        }
        let lookup = |name: &str| -> Result<&bots::Bot> {
            if !valid_name(name) { return Err("Invalid portable bot name.".into()); }
            let matches: Vec<_> = bots.iter()
                .filter(|bot| bot.name.eq_ignore_ascii_case(name)).collect();
            match matches.as_slice() {
                [bot] => Ok(bot),
                [] => Err(format!("Referenced bot does not exist: {name}")),
                _ => Err(format!("Ambiguous portable bot name: {name}")),
            }
        };
        let coordinator = lookup(&preset.coordinator_bot)?;
        let worker = lookup(&preset.worker_bot)?;
        if coordinator.id == worker.id {
            return Err("Coordinator and worker must be different existing bots.".into());
        }
        let existing: Option<Orchestration> = store.db.query_row(
            "SELECT id,name,coordinator_bot_id,worker_bot_id,project_id,enabled,policy_revision
             FROM sa_orchestrations WHERE name=?1 COLLATE NOCASE",
            [&preset.name], super::orchestration_row,
        ).optional().map_err(|e| e.to_string())?;
        if let Some(old) = &existing {
            // Reimports must never change an existing preset's authorizations,
            // bot bindings or enabled state. Require an explicit edit workflow.
            if old.coordinator_bot_id != coordinator.id
                || old.worker_bot_id != worker.id || old.project_id != worker.project_id
            {
                return Err(format!("Existing orchestration conflicts with imported preset: {}",preset.name));
            }
        }
        resolved.push(Resolved {
            name: preset.name.clone(),
            coordinator_id: coordinator.id.clone(),
            worker_id: worker.id.clone(),
            project_id: worker.project_id.clone(),
            existing: existing.is_some(),
        });
    }
    Ok(resolved)
}

fn preview(store: &Store, document: &serde_json::Value) -> Result<ImportPreview> {
    let resolved = resolve(store, document)?;
    Ok(ImportPreview {
        would_create: resolved.iter().filter(|r| !r.existing).map(|r| r.name.clone()).collect(),
        unchanged: resolved.iter().filter(|r| r.existing).map(|r| r.name.clone()).collect(),
        imported_presets_disabled: true,
    })
}

fn apply(store: &Store, document: &serde_json::Value, confirmed: bool) -> Result<ImportResult> {
    if !confirmed {
        return Err("Confirm the preview before importing SuperAgent presets.".into());
    }
    // Full dependency and conflict preflight precedes the first mutation.
    let resolved = resolve(store, document)?;
    let mut result = ImportResult { created: Vec::new(), unchanged: Vec::new(), backup_path: None };
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    for candidate in resolved {
        if candidate.existing {
            result.unchanged.push(candidate.name);
            continue;
        }
        let now = storage::now();
        tx.execute(
            "INSERT INTO sa_orchestrations
                (id,name,coordinator_bot_id,worker_bot_id,project_id,enabled,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,0,?6,?6)",
            params![storage::id(),candidate.name,candidate.coordinator_id,
                candidate.worker_id,candidate.project_id,now],
        ).map_err(|e| format!("Atomic SuperAgent import aborted: {e}"))?;
        result.created.push(candidate.name);
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}


/* Take a verified private configuration snapshot before applying a new preset.
 * Existing native import remains transactional and rechecks dependencies.
 */
fn apply_with_backup(
    store: &Store, data_dir: &Path, document: &serde_json::Value, confirmed: bool,
) -> Result<ImportResult> {
    if !confirmed {
        return Err("Confirm the preview before importing SuperAgent presets.".into());
    }
    let plan = preview(store, document)?;
    if plan.would_create.is_empty() {
        return apply(store, document, true);
    }
    let folder = data_dir.join("bot-backups");
    crate::extended_bot_manifest::set_private_directory(&folder)?;
    let snapshot = export(store)?;
    let mut bytes = serde_json::to_vec_pretty(&snapshot).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    let path = folder.join(format!(
        "orchestrations-{}-{}.json",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"), storage::id(),
    ));
    crate::extended_bot_manifest::atomic_write_new(&path, &bytes)?;
    let mut result = apply(store, document, true)?;
    result.backup_path = Some(path.display().to_string());
    Ok(result)
}

fn export(store: &Store) -> Result<serde_json::Value> {
    let mut statement = store.db.prepare(
        "SELECT id,name,coordinator_bot_id,worker_bot_id,project_id,enabled,policy_revision
         FROM sa_orchestrations ORDER BY name COLLATE NOCASE",
    ).map_err(|e| e.to_string())?;
    let configs = statement.query_map([],super::orchestration_row)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<Orchestration>>>()
        .map_err(|e| e.to_string())?;
    let mut presets = Vec::with_capacity(configs.len());
    for config in configs {
        // Export the real names, not unstable local IDs or task state.
        let coordinator = bots::get(store, &config.coordinator_bot_id)?;
        let worker = bots::get(store, &config.worker_bot_id)?;
        presets.push(Preset {
            name: config.name,
            coordinator_bot: coordinator.name,
            worker_bot: worker.name,
            enabled: config.enabled,
            autonomy: "off".into(),
            max_concurrent_tasks: 1,
            max_attempts_per_assignment: 2,
            council: Council { enabled: false },
        });
    }
    serde_json::to_value(Manifest { schema_version: 1, orchestrations: presets })
        .map_err(|e| e.to_string())
}

fn main_window(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err("SuperAgent configuration is available only in the main window.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn superagent_manifest_export(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
) -> Result<serde_json::Value> {
    main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    export(&store)
}

#[tauri::command]
pub async fn superagent_manifest_preview(
    window: tauri::WebviewWindow, state: State<'_, AppState>, document: serde_json::Value,
) -> Result<ImportPreview> {
    main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    preview(&store, &document)
}

#[tauri::command]
pub async fn superagent_manifest_import(
    app: tauri::AppHandle, window: tauri::WebviewWindow, state: State<'_, AppState>,
    document: serde_json::Value, confirmed: bool,
) -> Result<ImportResult> {
    main_window(&window)?;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    apply_with_backup(&store, &dir, &document, confirmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Store {
        let path = std::env::temp_dir().join(format!("pandacode-sa-manifest-{}.db", storage::id()));
        let store = Store::open(&path).unwrap();
        for (id,name,project) in [
            ("router", "Router", "router-project"),
            ("worker", "Builder", "worker-project"),
        ] {
            store.db.execute(
                "INSERT INTO projects(id,name,path) VALUES(?1,?2,?3)",
                params![project,name,format!("/tmp/pandacode-manifest-{project}")],
            ).unwrap();
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?2,?3,'codex','default','plan',?4,?4)",
                params![id,name,project,storage::now()],
            ).unwrap();
        }
        store
    }

    fn document(name: &str) -> serde_json::Value {
        json!({"schemaVersion":1,"orchestrations":[
            {"name":name,"coordinatorBot":"Router","workerBot":"Builder",
             "enabled":true,"autonomy":"off","maxConcurrentTasks":1,
             "maxAttemptsPerAssignment":2,"council":{"enabled":false}}
        ]})
    }

    #[test]
    fn confirmed_import_preserves_private_configuration_snapshots_and_noop_reimport() {
        let store = fixture();
        let root = std::env::temp_dir().join(format!("sa-backup-test-{}", storage::id()));
        let first = apply_with_backup(&store, &root, &document("Software Development"), true).unwrap();
        let path = first.backup_path.unwrap();
        let snapshot: serde_json::Value = serde_json::from_slice(
            &std::fs::read(path).unwrap(),
        ).unwrap();
        assert_eq!(snapshot["schemaVersion"], 1);
        assert_eq!(snapshot["orchestrations"].as_array().unwrap().len(), 0);
        let repeat = apply_with_backup(&store, &root, &document("Software Development"), true).unwrap();
        assert_eq!(repeat.unchanged, vec!["Software Development"]);
        assert!(repeat.backup_path.is_none());
        let second = apply_with_backup(&store, &root, &document("Issue Resolution"), true).unwrap();
        let snapshot: serde_json::Value = serde_json::from_slice(
            &std::fs::read(second.backup_path.unwrap()).unwrap(),
        ).unwrap();
        assert_eq!(snapshot["orchestrations"][0]["name"], "Software Development");
        assert_eq!(snapshot["orchestrations"][0]["enabled"], false);
        let count = std::fs::read_dir(root.join("bot-backups")).unwrap().count();
        assert_eq!(count, 2);
        assert!(apply_with_backup(&store, &root, &document("Not confirmed"), false).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_snapshot_never_partially_imports_a_preset() {
        let store = fixture();
        let blocked = std::env::temp_dir().join(format!("sa-blocked-backup-{}", storage::id()));
        std::fs::write(&blocked, "file prevents private backup directory").unwrap();
        assert!(apply_with_backup(&store, &blocked, &document("Not inserted"), true).is_err());
        let count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM sa_orchestrations", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(blocked).unwrap();
    }

    #[test]
    fn portable_presets_preview_then_import_disabled_and_round_trip_without_task_state() {
        let store = fixture();
        let doc = document("Software Development");
        let plan = preview(&store,&doc).unwrap();
        assert_eq!(plan.would_create,vec!["Software Development"]);
        assert!(plan.imported_presets_disabled);
        let count: i64 = store.db.query_row("SELECT COUNT(*) FROM sa_orchestrations",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0);
        assert!(apply(&store,&doc,false).is_err());
        let applied = apply(&store,&doc,true).unwrap();
        assert_eq!(applied.created,vec!["Software Development"]);
        assert_eq!(apply(&store,&doc,true).unwrap().unchanged,vec!["Software Development"]);
        let exported = export(&store).unwrap();
        assert_eq!(exported["orchestrations"][0]["enabled"],false);
        assert_eq!(exported["orchestrations"][0]["coordinatorBot"],"Router");
        assert_eq!(exported["orchestrations"][0]["workerBot"],"Builder");
        let serialized = exported.to_string();
        for private in ["router-project","worker-project","sa_tickets","session_id","authorization_id"] {
            assert!(!serialized.contains(private),"Manifest leaked runtime identity: {private}");
        }
    }

    #[test]
    fn missing_ambiguous_and_duplicate_bot_references_are_rejected_without_mutations() {
        let store = fixture();
        let mut bad = document("Safe");
        bad["orchestrations"][0]["workerBot"] = json!("missing");
        assert!(preview(&store,&bad).is_err());
        bad = document("Safe");
        bad["orchestrations"][0]["autonomy"] = json!("auto");
        assert!(apply(&store,&bad,true).is_err());
        bad = document("Safe");
        bad["orchestrations"][0]["council"]["enabled"] = json!(true);
        assert!(apply(&store,&bad,true).is_err());
        bad = document("Safe");
        bad["orchestrations"].as_array_mut().unwrap().push(document("safe")["orchestrations"][0].clone());
        assert!(preview(&store,&bad).is_err());
        store.db.execute(
            "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('duplicate','builder','worker-project','codex','default','plan',?1,?1)",
            [storage::now()],
        ).unwrap();
        assert!(apply(&store,&document("Ambiguous"),true).is_err());
        let count: i64 = store.db.query_row("SELECT COUNT(*) FROM sa_orchestrations",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0);
    }

    #[test]
    fn conflicting_import_cannot_overwrite_an_enabled_existing_preset() {
        let store = fixture();
        apply(&store,&document("Issue Resolution"),true).unwrap();
        store.db.execute("UPDATE sa_orchestrations SET enabled=1 WHERE name='Issue Resolution'",[]).unwrap();
        let old = export(&store).unwrap();
        assert_eq!(old["orchestrations"][0]["enabled"],true);
        let result = apply(&store,&document("Issue Resolution"),true).unwrap();
        assert_eq!(result.created.len(),0);
        let enabled: bool = store.db.query_row(
            "SELECT enabled FROM sa_orchestrations WHERE name='Issue Resolution'",[],|r|r.get(0),
        ).unwrap();
        assert!(enabled);
        store.db.execute("UPDATE sa_orchestrations SET worker_bot_id='router' WHERE name='Issue Resolution'",[]).unwrap();
        assert!(apply(&store,&document("Issue Resolution"),true).is_err());
    }

    #[test]
    fn import_is_transactional_when_a_later_preset_cannot_be_inserted() {
        let store = fixture();
        let mut doc = document("First");
        doc["orchestrations"].as_array_mut().unwrap().push(document("Second")["orchestrations"][0].clone());
        store.db.execute_batch(
            "CREATE TEMP TRIGGER reject_second BEFORE INSERT ON sa_orchestrations
             WHEN NEW.name='Second'
             BEGIN SELECT RAISE(ABORT,'simulated late insert failure'); END;",
        ).unwrap();
        assert!(apply(&store,&doc,true).is_err());
        let count: i64 = store.db.query_row("SELECT COUNT(*) FROM sa_orchestrations",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0,"partial import escaped its SQLite transaction");
        store.db.execute_batch("DROP TRIGGER reject_second").unwrap();
        assert_eq!(apply(&store,&doc,true).unwrap().created.len(),2);
    }
}
