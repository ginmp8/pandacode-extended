//! Extended-owned portable bot configuration, backup and desktop adapters.
//! All mutations still use the upstream headless_bots importer.
use crate::{bots, headless_bots, storage::Store, AppState, Result};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tauri::{Emitter, Manager, State};

const BACKUP_KEEP: usize = 10;
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportOutcome {
    pub backup_path: String,
    pub lines: Vec<String>,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportOutcome {
    pub document: Value,
    pub omitted_shared_channels: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportFolderOutcome {
    pub paths: Vec<String>,
    pub omitted_shared_channels: Vec<String>,
}

pub(crate) fn verify_manifest(doc: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(doc).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err("The bot manifest exceeds the 1 MB limit.".into());
    }
    let bots = doc.get("bots").and_then(Value::as_array)
        .ok_or("The import file needs a \"bots\" array.")?;
    if bots.len() > 250 {
        return Err("A manifest can define up to 250 bots.".into());
    }
    if doc.get("sharedChannels").is_some_and(|v| !v.is_null() && !v.is_array()) {
        return Err("sharedChannels must be an array or null.".into());
    }
    Ok(())
}

pub(crate) fn set_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|error| format!("Unable to create private configuration directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("Unable to protect the configuration directory: {error}"))?;
    }
    Ok(())
}

fn atomic_write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("Choose a destination directory.")?;
    if !parent.is_dir() { return Err("The destination directory does not exist.".into()); }
    if path.exists() { return Err(format!("The destination already exists: {}", path.display())); }
    let temp = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|error| format!("Could not open temporary export: {error}"))?;
        file.write_all(bytes).map_err(|error| format!("Could not write export: {error}"))?;
        file.sync_all().map_err(|error| format!("Could not flush export: {error}"))?;
        drop(file);
        // Hard-link creation is exclusive and atomic on the same filesystem; rename
        // could overwrite a file created by another process after our exists check.
        fs::hard_link(&temp, path).map_err(|error| format!("Could not finalize export: {error}"))?;
        let actual = fs::read(path).map_err(|error| format!("Could not verify export: {error}"))?;
        if actual != bytes {
            let _ = fs::remove_file(path);
            return Err("The exported file failed verification.".into());
        }
        fs::remove_file(&temp).map_err(|error| format!("Could not remove the temporary export: {error}"))?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}

fn document_bytes(doc: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(doc).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write and verify a complete portable configuration backup before any importer mutation.
/// No conversations, memory records, runtime state or credentials are serialized.
fn backup(store: &Store, data_dir: &Path) -> Result<PathBuf> {
    let folder = data_dir.join("bot-backups");
    set_private_directory(&folder)?;
    let document = headless_bots::export(store)?;
    let bytes = document_bytes(&document)?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let dest = folder.join(format!("bots-{}-{}.json", stamp, uuid::Uuid::new_v4()));
    atomic_write_new(&dest, &bytes)?;
    // Pruning is deliberately best-effort and only follows a verified new snapshot.
    let mut snapshots: Vec<_> = fs::read_dir(&folder)
        .map_err(|error| format!("Could not enumerate backup history: {error}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.file_name().and_then(|v| v.to_str())
            .is_some_and(|name| name.starts_with("bots-") && name.ends_with(".json")))
        .collect();
    snapshots.sort();
    let remove = snapshots.len().saturating_sub(BACKUP_KEEP);
    for path in snapshots.into_iter().take(remove) { let _ = fs::remove_file(path); }
    Ok(dest)
}

/// This is the only mutation gate used by desktop, local headless import and Git sync.
pub(crate) fn apply_with_backup(
    store: &Store,
    data_dir: &Path,
    document: &Value,
    now: i64,
) -> Result<(headless_bots::Report, PathBuf)> {
    verify_manifest(document)?;
    let backup_path = backup(store, data_dir)?;
    let report = headless_bots::apply(store, &bots::workspace_root(), document, now);
    Ok((report, backup_path))
}

pub(crate) fn export_selection(store: &Store, ids: &[String]) -> Result<ExportOutcome> {
    if ids.is_empty() { return Err("Select at least one bot to export.".into()); }
    let known = bots::list(store)?;
    let selected_ids: BTreeSet<_> = ids.iter().collect();
    if selected_ids.len() != ids.len() { return Err("A bot was selected twice.".into()); }
    let selected_names: BTreeSet<_> = known.iter()
        .filter(|bot| selected_ids.contains(&bot.id))
        .map(|bot| bot.name.as_str()).collect();
    if selected_names.len() != ids.len() { return Err("A selected bot no longer exists.".into()); }
    let mut doc = headless_bots::export(store)?;
    doc["bots"].as_array_mut().ok_or("Invalid bot export.")?
        .retain(|entry| entry.get("name").and_then(Value::as_str).is_some_and(|name| selected_names.contains(name)));
    let mut omitted = Vec::new();
    doc["sharedChannels"].as_array_mut().ok_or("Invalid shared channel export.")?.retain(|entry| {
        let members = entry.get("members").and_then(Value::as_array);
        let complete = members.is_some_and(|items| items.iter().all(|member|
            member.as_str().is_some_and(|name| selected_names.contains(name))));
        let selected_member = members.is_some_and(|items| items.iter().any(|member|
            member.as_str().is_some_and(|name| selected_names.contains(name))));
        if !complete && selected_member {
            if let Some(name) = entry.get("name").and_then(Value::as_str) { omitted.push(name.to_owned()); }
        }
        complete
    });
    // Channel-specific preferences cannot reference shared channels that were omitted
    // because other members were not part of the selected bot set.
    let included_channels: BTreeSet<String> = doc["sharedChannels"].as_array()
        .ok_or("Invalid shared channel export.")?.iter()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect();
    if let Some(entries) = doc["bots"].as_array_mut() {
        for entry in entries {
            if let Some(overrides) = entry.pointer_mut("/extended/sharedChannelHistoryOverrides")
                .and_then(Value::as_array_mut) {
                overrides.retain(|override_entry| override_entry.get("channel").and_then(Value::as_str)
                    .is_some_and(|name| included_channels.contains(name)));
            }
        }
    }
    Ok(ExportOutcome { document: doc, omitted_shared_channels: omitted })
}

fn check_main(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" { return Err("Bot manifests can only be managed from the main window.".into()); }
    Ok(())
}

#[tauri::command]
pub async fn bot_manifest_import(
    app: tauri::AppHandle, window: tauri::WebviewWindow, state: State<'_, AppState>, document: Value,
) -> Result<ImportOutcome> {
    check_main(&window)?;
    let dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
    let (report, path) = {
        let store = state.store.lock().map_err(|error| error.to_string())?;
        apply_with_backup(&store, &dir, &document, crate::storage::now())?
    };
    for added in &report.added_channels {
        let channel = {
            let store = state.store.lock().map_err(|error| error.to_string())?;
            bots::channels::get(&store, &added.id)?
        };
        bots::channels::start(&app, channel);
    }
    let _ = app.emit("bots-changed", ());
    let _ = app.emit("bot-shared-channel-changed", serde_json::json!({}));
    Ok(ImportOutcome { backup_path: path.display().to_string(), lines: report.lines, failed: report.failed })
}

#[tauri::command]
pub async fn bot_manifest_export(
    window: tauri::WebviewWindow, state: State<'_, AppState>, bot_ids: Vec<String>,
) -> Result<ExportOutcome> {
    check_main(&window)?;
    let store = state.store.lock().map_err(|error| error.to_string())?;
    export_selection(&store, &bot_ids)
}

#[tauri::command]
pub async fn bot_manifest_export_file(
    window: tauri::WebviewWindow, state: State<'_, AppState>, bot_ids: Vec<String>, path: String,
) -> Result<ExportOutcome> {
    check_main(&window)?;
    if !path.ends_with(".json") { return Err("Choose a .json destination.".into()); }
    let outcome = {
        let store = state.store.lock().map_err(|error| error.to_string())?;
        export_selection(&store, &bot_ids)?
    };
    atomic_write_new(Path::new(&path), &document_bytes(&outcome.document)?)?;
    Ok(outcome)
}

fn safe_filename(value: &str) -> String {
    let trimmed: String = value.chars().take(48).map(|c| {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '_' }
    }).collect();
    let trimmed = trimmed.trim_matches('_');
    if trimmed.is_empty() { "bot".into() } else { trimmed.into() }
}

/// Export each bot independently. Shared channels whose complete membership is not
/// in an individual manifest are intentionally omitted and reported to the caller.
fn export_folder(store: &Store, bot_ids: &[String], folder: &Path) -> Result<ExportFolderOutcome> {
    if !folder.is_dir() { return Err("Choose an existing export directory.".into()); }
    let known = bots::list(store)?;
    if bot_ids.is_empty() { return Err("Select at least one bot to export.".into()); }
    let mut paths = Vec::new();
    let mut omitted = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for id in bot_ids {
        if !seen.insert(id) { return Err("A bot was selected twice.".into()); }
        let bot = known.iter().find(|bot| &bot.id == id).ok_or("A selected bot no longer exists.")?;
        let outcome = export_selection(store, std::slice::from_ref(id))?;
        omitted.extend(outcome.omitted_shared_channels);
        // Suffix the local ID so bots with colliding names cannot overwrite one another.
        let path = folder.join(format!("{}-{}.json", safe_filename(&bot.name), safe_filename(&bot.id)));
        atomic_write_new(&path, &document_bytes(&outcome.document)?)?;
        paths.push(path.display().to_string());
    }
    Ok(ExportFolderOutcome { paths, omitted_shared_channels: omitted.into_iter().collect() })
}

#[tauri::command]
pub async fn bot_manifest_export_folder(
    window: tauri::WebviewWindow, state: State<'_, AppState>, bot_ids: Vec<String>, folder: String,
) -> Result<ExportFolderOutcome> {
    check_main(&window)?;
    let store = state.store.lock().map_err(|error| error.to_string())?;
    export_folder(&store, &bot_ids, Path::new(&folder))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_failure_prevents_mutation() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let blocked = std::env::temp_dir().join(format!("pandacode-manifest-blocked-{}", uuid::Uuid::new_v4()));
        fs::write(&blocked, b"not a directory").unwrap();
        let document = serde_json::json!({"bots":[{"name":"test","provider":"claude","model":"sonnet","mode":"auto"}]});
        assert!(apply_with_backup(&store, &blocked, &document, 100).is_err());
        assert!(bots::list(&store).unwrap().is_empty());
        let _ = fs::remove_file(blocked);
    }

    #[test]
    fn verified_backup_is_private_and_does_not_include_conversations() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let folder = std::env::temp_dir().join(format!("pandacode-manifest-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        let (report, path) = apply_with_backup(&store, &folder, &serde_json::json!({"bots":[]}), 100).unwrap();
        assert_eq!(report.failed, 0);
        let saved: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(saved["bots"], serde_json::json!([]));
        assert_eq!(saved["sharedChannels"], serde_json::json!([]));
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn individual_exports_report_only_relevant_omitted_shared_channels() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let root = std::env::temp_dir().join(format!("pandacode-export-selection-{}", uuid::Uuid::new_v4()));
        let folder = root.join("exports");
        fs::create_dir_all(&folder).unwrap();
        let create_bot = |name: &str| bots::create(&store, &bots::BotInput {
            name: name.into(), instructions: "Keep the manifest portable.".into(),
            provider: "claude".into(), model: "default".into(), mode: "ask".into(),
            ..Default::default()
        }, &root).unwrap();
        let scout = create_bot("Scout");
        let writer = create_bot("Writer");
        let other = create_bot("Other");
        bots::shared_channels::create(&store, "release", &[scout.id.clone(), writer.id.clone()]).unwrap();
        bots::shared_channels::create(&store, "unrelated", std::slice::from_ref(&other.id)).unwrap();

        let selected = export_selection(&store, std::slice::from_ref(&scout.id)).unwrap();
        assert_eq!(selected.omitted_shared_channels, vec!["release"]);
        assert_eq!(selected.document["sharedChannels"], serde_json::json!([]));
        let combined = export_selection(&store, &[scout.id.clone(), writer.id.clone()]).unwrap();
        assert!(combined.omitted_shared_channels.is_empty());
        assert_eq!(combined.document["sharedChannels"].as_array().unwrap().len(), 1);

        let separate = export_folder(&store, &[scout.id.clone(), writer.id.clone()], &folder).unwrap();
        assert_eq!(separate.paths.len(), 2);
        assert_eq!(separate.omitted_shared_channels, vec!["release"]);
        for path in separate.paths {
            let document: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            assert_eq!(document["bots"].as_array().unwrap().len(), 1);
            assert_eq!(document["sharedChannels"], serde_json::json!([]));
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn selection_is_bounded_and_cannot_export_missing_bots() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        assert!(export_selection(&store, &[]).is_err());
        assert!(export_selection(&store, &["missing".into()]).is_err());
        assert!(verify_manifest(&serde_json::json!({"bots":[]})).is_ok());
        assert!(verify_manifest(&serde_json::json!({"bots":42})).is_err());
    }
}
