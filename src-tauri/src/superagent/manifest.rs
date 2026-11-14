//! Portable, configuration-only SuperAgent companion manifest.
//! Integrates with existing native Bot identities without importing credentials,
//! conversations, tasks, mandates or any other runtime/authority state.
//! Import preflights every reference, then commits only disabled new presets.

use super::Orchestration;
use crate::{
    bots,
    storage::{self, Store},
    AppState, Result,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};
use tauri::{Manager, State};

const MAX_BYTES: usize = 1024 * 1024;
const MAX_PRESETS: usize = 64;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestV1 {
    schema_version: u32,
    orchestrations: Vec<PresetV1>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PresetV1 {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TeamPolicy {
    autonomy: String,
    max_concurrent_tasks: u8,
    max_attempts_per_assignment: u8,
    council_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestV2 {
    schema_version: u32,
    orchestrations: Vec<PresetV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PresetV2 {
    name: String,
    coordinator_bot: String,
    workers: Vec<String>,
    #[serde(default)]
    reviewers: Vec<String>,
    policy: TeamPolicy,
}

#[derive(Debug, Clone)]
struct PortablePreset {
    name: String,
    coordinator_bot: String,
    workers: Vec<String>,
    reviewers: Vec<String>,
    policy: TeamPolicy,
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
    worker_ids: Vec<String>,
    reviewer_ids: Vec<String>,
    project_id: String,
    existing: bool,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.len() <= 120
        && !name.chars().any(char::is_control)
}

fn policy_from_snapshot(snapshot: &super::TeamSnapshot) -> TeamPolicy {
    TeamPolicy {
        autonomy: snapshot.policy.autonomy.clone(),
        max_concurrent_tasks: snapshot.policy.max_concurrent_tasks,
        max_attempts_per_assignment: snapshot.policy.max_attempts_per_assignment,
        council_enabled: snapshot.policy.council_enabled,
    }
}

fn parse_manifest(document: &serde_json::Value) -> Result<Vec<PortablePreset>> {
    let bytes = serde_json::to_vec(document).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("SuperAgent manifest exceeds the 1 MB limit.".into());
    }
    let schema_version = document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .ok_or("Unsupported SuperAgent manifest schema.")?;
    let presets: Vec<PortablePreset> = match schema_version {
        1 => {
            let manifest: ManifestV1 = serde_json::from_value(document.clone())
                .map_err(|e| format!("Invalid SuperAgent companion manifest: {e}"))?;
            if manifest.schema_version != 1 {
                return Err("Unsupported SuperAgent manifest schema.".into());
            }
            manifest
                .orchestrations
                .into_iter()
                .map(|preset| PortablePreset {
                    name: preset.name,
                    coordinator_bot: preset.coordinator_bot,
                    workers: vec![preset.worker_bot],
                    reviewers: Vec::new(),
                    policy: TeamPolicy {
                        autonomy: preset.autonomy,
                        max_concurrent_tasks: preset.max_concurrent_tasks,
                        max_attempts_per_assignment: preset.max_attempts_per_assignment,
                        council_enabled: preset.council.enabled,
                    },
                })
                .collect()
        }
        2 => {
            let manifest: ManifestV2 = serde_json::from_value(document.clone())
                .map_err(|e| format!("Invalid SuperAgent companion manifest: {e}"))?;
            if manifest.schema_version != 2 {
                return Err("Unsupported SuperAgent manifest schema.".into());
            }
            manifest
                .orchestrations
                .into_iter()
                .map(|preset| PortablePreset {
                    name: preset.name,
                    coordinator_bot: preset.coordinator_bot,
                    workers: preset.workers,
                    reviewers: preset.reviewers,
                    policy: preset.policy,
                })
                .collect()
        }
        _ => return Err("Unsupported SuperAgent manifest schema.".into()),
    };
    if presets.len() > MAX_PRESETS {
        return Err("SuperAgent manifest contains too many Teams.".into());
    }
    for preset in &presets {
        if !valid_name(&preset.name) {
            return Err("Invalid orchestration name in manifest.".into());
        }
        if preset.workers.is_empty()
            || preset.workers.len() > super::MAX_WORKERS_PER_TEAM
            || preset.reviewers.len() > super::MAX_REVIEWERS_PER_TEAM
        {
            return Err(format!(
                "A Team needs 1–{} workers and at most {} reviewers.",
                super::MAX_WORKERS_PER_TEAM,
                super::MAX_REVIEWERS_PER_TEAM,
            ));
        }
        if preset.policy.autonomy != "off"
            || preset.policy.max_concurrent_tasks != 1
            || preset.policy.max_attempts_per_assignment != 2
            || preset.policy.council_enabled
        {
            return Err("Unsupported SuperAgent autonomy, Council or execution policy.".into());
        }
    }
    Ok(presets)
}

fn resolve(store: &Store, document: &serde_json::Value) -> Result<Vec<Resolved>> {
    let presets = parse_manifest(document)?;
    let available_bots = bots::list(store)?;
    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(presets.len());
    for preset in presets {
        if !seen.insert(preset.name.to_ascii_lowercase()) {
            return Err("Invalid or duplicate orchestration name in manifest.".into());
        }
        let lookup = |name: &str| -> Result<&bots::Bot> {
            if !valid_name(name) {
                return Err("Invalid portable bot name.".into());
            }
            let matches: Vec<_> = available_bots
                .iter()
                .filter(|bot| bot.name.eq_ignore_ascii_case(name))
                .collect();
            match matches.as_slice() {
                [bot] => Ok(bot),
                [] => Err(format!("Referenced bot is missing or was removed: {name}")),
                _ => Err(format!("Ambiguous portable bot name: {name}")),
            }
        };
        let coordinator = lookup(&preset.coordinator_bot)?;
        let workers = preset
            .workers
            .iter()
            .map(|name| lookup(name))
            .collect::<Result<Vec<_>>>()?;
        let reviewers = preset
            .reviewers
            .iter()
            .map(|name| lookup(name))
            .collect::<Result<Vec<_>>>()?;
        let mut identities = HashSet::new();
        for bot in std::iter::once(coordinator)
            .chain(workers.iter().copied())
            .chain(reviewers.iter().copied())
        {
            if !identities.insert(bot.id.as_str()) {
                return Err("A bot can have only one role in an imported Team.".into());
            }
        }
        if coordinator.paused
            || workers.iter().any(|bot| bot.paused)
            || reviewers.iter().any(|bot| bot.paused)
        {
            return Err("Paused bots cannot be assigned to an imported Team.".into());
        }
        let project_id = workers
            .first()
            .ok_or("A Team needs at least one worker.")?
            .project_id
            .clone();
        if workers.iter().any(|bot| bot.project_id != project_id) {
            return Err("Ordered Team workers must share one project scope.".into());
        }
        let existing: Option<Orchestration> = store
            .db
            .query_row(
                "SELECT id,name,coordinator_bot_id,worker_bot_id,project_id,enabled,policy_revision
             FROM sa_orchestrations WHERE name=?1 COLLATE NOCASE",
                [&preset.name],
                super::orchestration_row,
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(old) = &existing {
            // Reimports must never change an existing preset's authorizations,
            // bot bindings or enabled state. Require an explicit edit workflow.
            let old = super::configuration(store, &old.id)?;
            let old_snapshot =
                super::load_team_version_snapshot(&store.db, &old.id, old.team_version)?;
            if old.coordinator_bot_id != coordinator.id
                || old.worker_bot_ids
                    != workers.iter().map(|bot| bot.id.clone()).collect::<Vec<_>>()
                || old.reviewer_bot_ids
                    != reviewers
                        .iter()
                        .map(|bot| bot.id.clone())
                        .collect::<Vec<_>>()
                || old.project_id != project_id
                || policy_from_snapshot(&old_snapshot) != preset.policy
            {
                return Err(format!(
                    "Existing orchestration conflicts with imported Team: {}",
                    preset.name
                ));
            }
        }
        resolved.push(Resolved {
            name: preset.name.clone(),
            coordinator_id: coordinator.id.clone(),
            worker_ids: workers.iter().map(|bot| bot.id.clone()).collect(),
            reviewer_ids: reviewers.iter().map(|bot| bot.id.clone()).collect(),
            project_id,
            existing: existing.is_some(),
        });
    }
    Ok(resolved)
}

fn preview(store: &Store, document: &serde_json::Value) -> Result<ImportPreview> {
    let resolved = resolve(store, document)?;
    Ok(ImportPreview {
        would_create: resolved
            .iter()
            .filter(|r| !r.existing)
            .map(|r| r.name.clone())
            .collect(),
        unchanged: resolved
            .iter()
            .filter(|r| r.existing)
            .map(|r| r.name.clone())
            .collect(),
        imported_presets_disabled: true,
    })
}

fn apply(store: &Store, document: &serde_json::Value, confirmed: bool) -> Result<ImportResult> {
    if !confirmed {
        return Err("Confirm the preview before importing SuperAgent presets.".into());
    }
    // Full dependency and conflict preflight precedes the first mutation.
    let resolved = resolve(store, document)?;
    let mut result = ImportResult {
        created: Vec::new(),
        unchanged: Vec::new(),
        backup_path: None,
    };
    let tx = store
        .db
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    for candidate in resolved {
        if candidate.existing {
            result.unchanged.push(candidate.name);
            continue;
        }
        let now = storage::now();
        let id = storage::id();
        tx.execute(
            "INSERT INTO sa_orchestrations
                (id,name,coordinator_bot_id,worker_bot_id,project_id,enabled,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,0,?6,?6)",
            params![
                id,
                candidate.name,
                candidate.coordinator_id,
                candidate.worker_ids[0],
                candidate.project_id,
                now
            ],
        )
        .map_err(|e| format!("Atomic SuperAgent import aborted: {e}"))?;
        // The legacy projection keeps its first-worker column, while the
        // immutable Team version stores every ordered role.
        let snapshot = super::make_team_snapshot(
            &tx,
            &id,
            1,
            &candidate.name,
            &candidate.project_id,
            1,
            &candidate.coordinator_id,
            &candidate.worker_ids,
            &candidate.reviewer_ids,
            false,
            super::TeamSnapshotProvenance::Native,
        )?;
        super::insert_team_version(&tx, &snapshot)
            .map_err(|e| format!("Atomic SuperAgent import aborted: {e}"))?;
        result.created.push(candidate.name);
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

/* Take a verified private configuration snapshot before applying a new preset.
 * Existing native import remains transactional and rechecks dependencies.
 */
fn apply_with_backup(
    store: &Store,
    data_dir: &Path,
    document: &serde_json::Value,
    confirmed: bool,
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
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        storage::id(),
    ));
    crate::extended_bot_manifest::atomic_write_new(&path, &bytes)?;
    let mut result = apply(store, document, true)?;
    result.backup_path = Some(path.display().to_string());
    Ok(result)
}

fn export(store: &Store) -> Result<serde_json::Value> {
    let mut statement = store
        .db
        .prepare(
            "SELECT id,name,coordinator_bot_id,worker_bot_id,project_id,enabled,policy_revision
         FROM sa_orchestrations ORDER BY name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let configs = statement
        .query_map([], super::orchestration_row)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<Orchestration>>>()
        .map_err(|e| e.to_string())?;
    let mut presets = Vec::with_capacity(configs.len());
    for row in configs {
        let config = super::configuration(store, &row.id)?;
        let snapshot =
            super::load_team_version_snapshot(&store.db, &config.id, config.team_version)?;
        // Export only portable role names and supported policy. Local IDs,
        // effective bot profiles, enabled state and runtime/task data stay local.
        let coordinator = bots::get(store, &snapshot.coordinator_bot_id)?;
        let workers = snapshot
            .worker_bot_ids
            .iter()
            .map(|id| bots::get(store, id).map(|bot| bot.name))
            .collect::<Result<Vec<_>>>()?;
        let reviewers = snapshot
            .reviewer_bot_ids
            .iter()
            .map(|id| bots::get(store, id).map(|bot| bot.name))
            .collect::<Result<Vec<_>>>()?;
        let policy = policy_from_snapshot(&snapshot);
        presets.push(PresetV2 {
            name: snapshot.name,
            coordinator_bot: coordinator.name,
            workers,
            reviewers,
            policy,
        });
    }
    serde_json::to_value(ManifestV2 {
        schema_version: 2,
        orchestrations: presets,
    })
    .map_err(|e| e.to_string())
}

/* Headless configuration administration uses explicit local CLI actions and
 * the exact same native preview, backup, import, and export contracts.
 * Unlike desktop IPC, these entrypoints require no synthetic Webview.
 */
fn read_headless_document(path: &Path) -> Result<serde_json::Value> {
    let metadata =
        fs::metadata(path).map_err(|e| format!("Cannot inspect orchestration manifest: {e}"))?;
    if metadata.len() > MAX_BYTES as u64 {
        return Err("SuperAgent manifest exceeds the 1 MB limit.".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read orchestration manifest: {e}"))?;
    if bytes.len() > MAX_BYTES {
        return Err("SuperAgent manifest exceeds the 1 MB limit.".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid orchestration manifest JSON: {e}"))
}

/// Validates dependencies without importing presets or writing a backup.
pub(crate) fn run_headless_preview(app: &tauri::AppHandle, path: &Path) -> Result<ImportPreview> {
    let document = read_headless_document(path)?;
    let state = app.state::<AppState>();
    let store = state.store.lock().map_err(|e| e.to_string())?;
    preview(&store, &document)
}

pub(crate) fn run_headless_import(app: &tauri::AppHandle, path: &Path) -> Result<ImportResult> {
    let document = read_headless_document(path)?;
    let state = app.state::<AppState>();
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    apply_with_backup(&store, &dir, &document, true)
}

pub(crate) fn run_headless_export(app: &tauri::AppHandle, path: &Path) -> Result<()> {
    let state = app.state::<AppState>();
    let document = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        export(&store)?
    };
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    crate::headless_bots::write_export_atomically(path, &bytes)
}

fn main_window(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err("SuperAgent configuration is available only in the main window.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn superagent_manifest_export(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<serde_json::Value> {
    main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    export(&store)
}

#[tauri::command]
pub async fn superagent_manifest_preview(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    document: serde_json::Value,
) -> Result<ImportPreview> {
    main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    preview(&store, &document)
}

#[tauri::command]
pub async fn superagent_manifest_import(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    document: serde_json::Value,
    confirmed: bool,
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

    fn fixture_with_ids(
        router_id: &str,
        worker_id: &str,
        worker_two_id: &str,
        reviewer_id: &str,
    ) -> Store {
        let path = std::env::temp_dir().join(format!("pandacode-sa-manifest-{}.db", storage::id()));
        let store = Store::open(&path).unwrap();
        for (id, name, project) in [
            (router_id, "Router", "router-project"),
            (worker_id, "Builder", "worker-project"),
            (worker_two_id, "Builder Two", "worker-project"),
            (reviewer_id, "Reviewer", "reviewer-project"),
        ] {
            store
                .db
                .execute(
                    "INSERT INTO projects(id,name,path) VALUES(?1,?2,?3)",
                    params![project, name, format!("/tmp/pandacode-manifest-{project}")],
                )
                .unwrap();
            store
                .db
                .execute(
                    "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?2,?3,'codex','default','plan',?4,?4)",
                    params![id, name, project, storage::now()],
                )
                .unwrap();
        }
        store
    }

    fn fixture() -> Store {
        fixture_with_ids("router", "worker", "worker-two", "reviewer")
    }

    fn document(name: &str) -> serde_json::Value {
        json!({"schemaVersion":1,"orchestrations":[
            {"name":name,"coordinatorBot":"Router","workerBot":"Builder",
             "enabled":true,"autonomy":"off","maxConcurrentTasks":1,
             "maxAttemptsPerAssignment":2,"council":{"enabled":false}}
        ]})
    }

    fn document_v2(name: &str) -> serde_json::Value {
        json!({"schemaVersion":2,"orchestrations":[
            {"name":name,"coordinatorBot":"Router",
             "workers":["Builder","Builder Two"],"reviewers":["Reviewer"],
             "policy":{"autonomy":"off","maxConcurrentTasks":1,
                       "maxAttemptsPerAssignment":2,"councilEnabled":false}}
        ]})
    }

    fn orchestration_id(store: &Store, name: &str) -> String {
        store
            .db
            .query_row(
                "SELECT id FROM sa_orchestrations WHERE name=?1",
                [name],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn confirmed_import_preserves_private_configuration_snapshots_and_noop_reimport() {
        let store = fixture();
        let root = std::env::temp_dir().join(format!("sa-backup-test-{}", storage::id()));
        let first =
            apply_with_backup(&store, &root, &document("Software Development"), true).unwrap();
        let path = first.backup_path.unwrap();
        let snapshot: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(snapshot["schemaVersion"], 2);
        assert_eq!(snapshot["orchestrations"].as_array().unwrap().len(), 0);
        let repeat =
            apply_with_backup(&store, &root, &document("Software Development"), true).unwrap();
        assert_eq!(repeat.unchanged, vec!["Software Development"]);
        assert!(repeat.backup_path.is_none());
        let second = apply_with_backup(&store, &root, &document("Issue Resolution"), true).unwrap();
        let snapshot: serde_json::Value =
            serde_json::from_slice(&std::fs::read(second.backup_path.unwrap()).unwrap()).unwrap();
        assert_eq!(
            snapshot["orchestrations"][0]["name"],
            "Software Development"
        );
        assert_eq!(snapshot["orchestrations"][0]["workers"], json!(["Builder"]));
        assert!(snapshot["orchestrations"][0].get("enabled").is_none());
        let imported_enabled: bool = store
            .db
            .query_row(
                "SELECT enabled FROM sa_orchestrations WHERE name='Software Development'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!imported_enabled);
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
        let count: i64 = store
            .db
            .query_row("SELECT COUNT(*) FROM sa_orchestrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(blocked).unwrap();
    }

    #[test]
    fn v1_compatibility_round_trips_as_a_one_worker_team_and_maps_names_on_a_fresh_install() {
        let store = fixture();
        let doc = document("Legacy Team");
        let plan = preview(&store, &doc).unwrap();
        assert_eq!(plan.would_create, vec!["Legacy Team"]);
        assert!(plan.imported_presets_disabled);
        let count: i64 = store
            .db
            .query_row("SELECT COUNT(*) FROM sa_orchestrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        assert!(apply(&store, &doc, false).is_err());
        let applied = apply(&store, &doc, true).unwrap();
        assert_eq!(applied.created, vec!["Legacy Team"]);
        assert_eq!(
            apply(&store, &doc, true).unwrap().unchanged,
            vec!["Legacy Team"]
        );
        let imported =
            super::super::configuration(&store, &orchestration_id(&store, "Legacy Team")).unwrap();
        assert_eq!(imported.team_version, 1);
        assert_eq!(imported.worker_bot_ids, vec!["worker"]);
        assert!(imported.reviewer_bot_ids.is_empty());
        let exported = export(&store).unwrap();
        assert_eq!(exported["schemaVersion"], 2);
        assert!(exported["orchestrations"][0].get("enabled").is_none());
        assert_eq!(exported["orchestrations"][0]["coordinatorBot"], "Router");
        assert_eq!(exported["orchestrations"][0]["workers"], json!(["Builder"]));
        assert_eq!(exported["orchestrations"][0]["reviewers"], json!([]));
        let serialized = exported.to_string();
        for private in [
            "router-project",
            "worker-project",
            "sa_tickets",
            "session_id",
            "authorization_id",
        ] {
            assert!(
                !serialized.contains(private),
                "Manifest leaked runtime identity: {private}"
            );
        }
        let restored = fixture_with_ids(
            "fresh-router",
            "fresh-worker",
            "fresh-worker-two",
            "fresh-reviewer",
        );
        assert_eq!(
            apply(&restored, &exported, true).unwrap().created,
            vec!["Legacy Team"]
        );
        let restored =
            super::super::configuration(&restored, &orchestration_id(&restored, "Legacy Team")).unwrap();
        assert_eq!(restored.coordinator_bot_id, "fresh-router");
        assert_eq!(restored.worker_bot_ids, vec!["fresh-worker"]);
        assert!(restored.reviewer_bot_ids.is_empty());
        assert_eq!(restored.team_version, 1);
    }

    #[test]
    fn v2_multi_role_teams_round_trip_in_order_with_supported_policy_and_no_task_state() {
        let store = fixture();
        let doc = document_v2("Multi-role Team");
        let plan = preview(&store, &doc).unwrap();
        assert_eq!(plan.would_create, vec!["Multi-role Team"]);
        assert!(plan.imported_presets_disabled);
        let result = apply(&store, &doc, true).unwrap();
        assert_eq!(result.created, vec!["Multi-role Team"]);
        let imported =
            super::super::configuration(&store, &orchestration_id(&store, "Multi-role Team")).unwrap();
        assert_eq!(imported.coordinator_bot_id, "router");
        assert_eq!(imported.worker_bot_ids, vec!["worker", "worker-two"]);
        assert_eq!(imported.reviewer_bot_ids, vec!["reviewer"]);

        let exported = export(&store).unwrap();
        assert_eq!(exported["schemaVersion"], 2);
        assert_eq!(exported["orchestrations"][0]["coordinatorBot"], "Router");
        assert_eq!(
            exported["orchestrations"][0]["workers"],
            json!(["Builder", "Builder Two"])
        );
        assert_eq!(
            exported["orchestrations"][0]["reviewers"],
            json!(["Reviewer"])
        );
        assert_eq!(
            exported["orchestrations"][0]["policy"],
            json!({
                "autonomy":"off","maxConcurrentTasks":1,
                "maxAttemptsPerAssignment":2,"councilEnabled":false,
            })
        );
        assert!(exported["orchestrations"][0].get("enabled").is_none());
        assert_eq!(
            apply(&store, &exported, true).unwrap().unchanged,
            vec!["Multi-role Team"]
        );
        let serialized = exported.to_string();
        for private in [
            "router-project",
            "worker-project",
            "reviewer-project",
            "sa_tickets",
            "session_id",
            "authorization_id",
            "worker-two",
        ] {
            assert!(
                !serialized.contains(private),
                "Manifest leaked runtime identity: {private}"
            );
        }

        let fresh = fixture_with_ids(
            "fresh-router",
            "fresh-worker",
            "fresh-worker-two",
            "fresh-reviewer",
        );
        assert_eq!(
            apply(&fresh, &exported, true).unwrap().created,
            vec!["Multi-role Team"]
        );
        let restored =
            super::super::configuration(&fresh, &orchestration_id(&fresh, "Multi-role Team")).unwrap();
        assert_eq!(restored.coordinator_bot_id, "fresh-router");
        assert_eq!(
            restored.worker_bot_ids,
            vec!["fresh-worker", "fresh-worker-two"]
        );
        assert_eq!(restored.reviewer_bot_ids, vec!["fresh-reviewer"]);
    }

    #[test]
    fn v2_allows_omitted_reviewers_and_rejects_missing_ambiguous_or_paused_bots() {
        let store = fixture();
        let mut no_reviewers = document_v2("No reviewers");
        no_reviewers["orchestrations"][0]
            .as_object_mut()
            .unwrap()
            .remove("reviewers");
        assert!(preview(&store, &no_reviewers).is_ok());
        assert_eq!(
            apply(&store, &no_reviewers, true).unwrap().created,
            vec!["No reviewers"]
        );
        assert!(
            super::super::configuration(&store, &orchestration_id(&store, "No reviewers"))
                .unwrap()
                .reviewer_bot_ids
                .is_empty()
        );

        let mut missing = document_v2("Missing");
        missing["orchestrations"][0]["reviewers"][0] = json!("Removed reviewer");
        assert!(preview(&store, &missing)
            .unwrap_err()
            .contains("missing or was removed"));

        store
            .db
            .execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
             VALUES('ambiguous','builder','worker-project','codex','default','plan',?1,?1)",
                [storage::now()],
            )
            .unwrap();
        let ambiguous = document_v2("Ambiguous");
        assert!(preview(&store, &ambiguous)
            .unwrap_err()
            .contains("Ambiguous portable bot name"));

        let paused_store = fixture();
        paused_store
            .db
            .execute("UPDATE bots SET paused=1 WHERE id='worker-two'", [])
            .unwrap();
        assert!(preview(&paused_store, &document_v2("Paused"))
            .unwrap_err()
            .contains("Paused bots"));

        let removed_store = fixture();
        removed_store
            .db
            .execute("DELETE FROM bots WHERE id='reviewer'", [])
            .unwrap();
        assert!(preview(&removed_store, &document_v2("Removed"))
            .unwrap_err()
            .contains("missing or was removed"));
    }

    #[test]
    fn v2_rejects_duplicate_identities_invalid_bounds_and_unsupported_policy_during_preview() {
        let store = fixture();
        let mut bad = document_v2("Duplicate role");
        bad["orchestrations"][0]["reviewers"] = json!(["Builder"]);
        assert!(preview(&store, &bad).unwrap_err().contains("only one role"));

        bad = document_v2("No workers");
        bad["orchestrations"][0]["workers"] = json!([]);
        assert!(preview(&store, &bad).is_err());
        bad = document_v2("Too many workers");
        bad["orchestrations"][0]["workers"] =
            json!(vec!["Builder"; super::super::MAX_WORKERS_PER_TEAM + 1]);
        assert!(preview(&store, &bad).unwrap_err().contains("workers"));
        bad = document_v2("Too many reviewers");
        bad["orchestrations"][0]["reviewers"] =
            json!(vec!["Reviewer"; super::super::MAX_REVIEWERS_PER_TEAM + 1]);
        assert!(preview(&store, &bad).unwrap_err().contains("reviewers"));

        bad = document_v2("Unsupported policy");
        bad["orchestrations"][0]["policy"]["maxConcurrentTasks"] = json!(2);
        assert!(preview(&store, &bad)
            .unwrap_err()
            .contains("Unsupported SuperAgent"));
        bad = document_v2("Unsupported council");
        bad["orchestrations"][0]["policy"]["councilEnabled"] = json!(true);
        assert!(preview(&store, &bad)
            .unwrap_err()
            .contains("Unsupported SuperAgent"));

        let count: i64 = store
            .db
            .query_row("SELECT COUNT(*) FROM sa_orchestrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn v1_policy_and_duplicate_bot_references_are_rejected_without_mutations() {
        let store = fixture();
        let mut bad = document("Safe");
        bad["orchestrations"][0]["workerBot"] = json!("missing");
        assert!(preview(&store, &bad).is_err());
        bad = document("Safe");
        bad["orchestrations"][0]["autonomy"] = json!("auto");
        assert!(preview(&store, &bad).is_err());
        bad = document("Safe");
        bad["orchestrations"][0]["council"]["enabled"] = json!(true);
        assert!(preview(&store, &bad).is_err());
        bad = document("Safe");
        bad["orchestrations"]
            .as_array_mut()
            .unwrap()
            .push(document("safe")["orchestrations"][0].clone());
        assert!(preview(&store, &bad).is_err());
        let count: i64 = store
            .db
            .query_row("SELECT COUNT(*) FROM sa_orchestrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn conflicting_import_cannot_overwrite_an_enabled_existing_team() {
        let store = fixture();
        apply(&store, &document("Issue Resolution"), true).unwrap();
        store
            .db
            .execute(
                "UPDATE sa_orchestrations SET enabled=1 WHERE name='Issue Resolution'",
                [],
            )
            .unwrap();
        let old = export(&store).unwrap();
        assert!(old["orchestrations"][0].get("enabled").is_none());
        let result = apply(&store, &document("Issue Resolution"), true).unwrap();
        assert_eq!(result.created.len(), 0);
        let enabled: bool = store
            .db
            .query_row(
                "SELECT enabled FROM sa_orchestrations WHERE name='Issue Resolution'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(enabled);
        store
            .db
            .execute(
                "UPDATE sa_orchestrations SET worker_bot_id='router' WHERE name='Issue Resolution'",
                [],
            )
            .unwrap();
        assert!(apply(&store, &document("Issue Resolution"), true).is_err());
    }

    #[test]
    fn import_is_transactional_when_a_later_preset_cannot_be_inserted() {
        let store = fixture();
        let root = std::env::temp_dir().join(format!("sa-transaction-backup-{}", storage::id()));
        let mut doc = document_v2("First");
        doc["orchestrations"]
            .as_array_mut()
            .unwrap()
            .push(document_v2("Second")["orchestrations"][0].clone());
        store
            .db
            .execute_batch(
                "CREATE TEMP TRIGGER reject_second BEFORE INSERT ON sa_orchestrations
             WHEN NEW.name='Second'
             BEGIN SELECT RAISE(ABORT,'simulated late insert failure'); END;",
            )
            .unwrap();
        assert!(apply_with_backup(&store, &root, &doc, true).is_err());
        let count: i64 = store
            .db
            .query_row("SELECT COUNT(*) FROM sa_orchestrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "partial import escaped its SQLite transaction");
        let versions: i64 = store
            .db
            .query_row("SELECT COUNT(*) FROM sa_team_versions", [], |r| r.get(0))
            .unwrap();
        let members: i64 = store
            .db
            .query_row("SELECT COUNT(*) FROM sa_team_version_members", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            versions, 0,
            "Team versions escaped their SQLite transaction"
        );
        assert_eq!(
            members, 0,
            "Team role memberships escaped their SQLite transaction"
        );
        let backups = std::fs::read_dir(root.join("bot-backups"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(
            backups.len(),
            1,
            "pre-import backup should remain available after an aborted import"
        );
        let backup: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&backups[0]).unwrap()).unwrap();
        assert_eq!(backup["schemaVersion"], 2);
        assert!(backup["orchestrations"].as_array().unwrap().is_empty());
        store
            .db
            .execute_batch("DROP TRIGGER reject_second")
            .unwrap();
        assert_eq!(
            apply_with_backup(&store, &root, &doc, true)
                .unwrap()
                .created
                .len(),
            2
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
