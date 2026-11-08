//! GitHub-backed, SHA-pinned bot manifest sources. No repository code is executed.
//! Source content is validated before the shared import+backup gate is entered.
use crate::{bots, extended_bot_manifest, headless_bots, storage::Store, AppState, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use tauri::{Emitter, Manager};

const MAX_SOURCES: usize = 16;
const MAX_FILES: usize = 32;
const MAX_SOURCE_BYTES: usize = 512 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024; // Base64 adds ~34% before JSON wrapping.

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestSource {
    pub id: String,
    pub repository: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub manifests: Vec<String>,
    #[serde(default)]
    pub auto_sync: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_checked_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_applied_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_applied_hash: Option<String>,
    /// Fingerprint of the local portable configuration at the last successful apply.
    /// An unchanged Git document must still be reapplied when a user changed a local bot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_applied_local_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SourceRegistry {
    pub version: u32,
    pub sources: Vec<ManifestSource>,
}

impl Default for SourceRegistry {
    fn default() -> Self { Self { version: 1, sources: Vec::new() } }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceReceipt {
    pub id: String,
    pub repository: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub commit: String,
    pub paths: Vec<String>,
    pub hashes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncOutcome {
    pub sources: Vec<SourceReceipt>,
    pub backup_path: Option<String>,
    pub lines: Vec<String>,
    pub failed: usize,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub started_at: i64,
    pub finished_at: i64,
    pub applied: bool,
    pub no_changes: bool,
}

fn valid_component(value: &str) -> bool {
    !value.is_empty() && value.len() <= 100 &&
        value.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.')) &&
        value != "." && value != ".."
}

fn github_repo(repository: &str) -> Result<(String, String)> {
    let url = tauri::Url::parse(repository).map_err(|_| "Use an https://github.com/owner/repo URL.")?;
    if url.scheme() != "https" || url.host_str() != Some("github.com") || url.port().is_some()
        || !url.username().is_empty() || url.password().is_some()
        || url.query().is_some() || url.fragment().is_some() {
        return Err("Only token-free https://github.com/owner/repo sources are supported.".into());
    }
    let parts: Vec<_> = url.path_segments().ok_or("Invalid Git repository path.")?
        .filter(|part| !part.is_empty()).collect();
    if parts.len() != 2 { return Err("Use https://github.com/owner/repo without extra path segments.".into()); }
    let name = parts[1].strip_suffix(".git").unwrap_or(parts[1]);
    if !valid_component(parts[0]) || !valid_component(name) {
        return Err("Invalid GitHub repository owner or name.".into());
    }
    Ok((parts[0].to_owned(), name.to_owned()))
}

fn validate_registry(registry: &SourceRegistry) -> Result<()> {
    if registry.version != 1 { return Err("Unsupported manifest source registry version.".into()); }
    if registry.sources.len() > MAX_SOURCES { return Err("Configure at most 16 Git sources.".into()); }
    let mut seen = std::collections::BTreeSet::new();
    let mut files = 0usize;
    for source in &registry.sources {
        if !valid_component(&source.id) || !seen.insert(source.id.as_str()) {
            return Err("Each Git source needs a unique safe ID.".into());
        }
        if source.auto_sync {
            return Err("Automatic Git source polling is not available. Disable autoSync and use an explicit sync.".into());
        }
        github_repo(&source.repository)?;
        if source.reference.is_empty() || source.reference.len() > 200
            || source.reference.contains("..") || source.reference.starts_with('-')
            || !source.reference.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'.' | b'-' | b'_')) {
            return Err(format!("Source {} has an invalid Git ref.", source.id));
        }
        if source.manifests.is_empty() { return Err(format!("Source {} needs a manifest path.", source.id)); }
        files += source.manifests.len();
        for path in &source.manifests {
            if path.len() > 300 || path.starts_with('/') || path.contains('\\')
                || path.split('/').any(|part| !valid_component(part))
                || !path.ends_with(".json") {
                return Err(format!("Source {} has an unsafe manifest path.", source.id));
            }
        }
    }
    if files > MAX_FILES { return Err("A sync batch can load at most 32 manifest files.".into()); }
    Ok(())
}

fn api_url(owner: &str, repo: &str, suffix: &[&str]) -> Result<tauri::Url> {
    let mut url = tauri::Url::parse("https://api.github.com/repos/").map_err(|error| error.to_string())?;
    url.path_segments_mut().map_err(|_| "Invalid GitHub API path.")?
        .extend([owner, repo].into_iter().chain(suffix.iter().copied()));
    Ok(url)
}

fn client() -> Result<reqwest::Client> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(concat!("PandaCode/", env!("CARGO_PKG_VERSION")))
        .build().map_err(|error| error.to_string())
}

async fn get_api(client: &reqwest::Client, url: tauri::Url) -> Result<Value> {
    let mut request = client.get(url);
    let token = std::env::var("GH_TOKEN").ok().filter(|value| !value.is_empty())
        .or_else(|| std::env::var("GITHUB_TOKEN").ok().filter(|value| !value.is_empty()));
    if let Some(token) = token { request = request.bearer_auth(token); }
    let response = request.header("Accept", "application/vnd.github+json")
        .send().await.map_err(|error| if error.is_timeout() {
            "GitHub request timed out.".to_owned()
        } else { "Could not connect to GitHub. Check network access and credentials.".to_owned() })?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("GitHub HTTP {status}. Check the source repository, ref, path, rate limits and GH_TOKEN/GITHUB_TOKEN access."));
    }
    if response.content_length().is_some_and(|size| size > MAX_RESPONSE_BYTES as u64) {
        return Err("GitHub API response exceeds the 1 MB limit.".into());
    }
    let bytes = response.bytes().await.map_err(|_| "Could not read the GitHub response.")?;
    if bytes.len() > MAX_RESPONSE_BYTES { return Err("GitHub API response exceeds the 1 MB limit.".into()); }
    serde_json::from_slice(&bytes).map_err(|_| "GitHub did not return valid JSON.".into())
}

fn read_hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn secret_reference_only(document: &Value) -> Result<()> {
    for bot in document["bots"].as_array().ok_or("Manifest bots must be an array.")? {
        let Some(channels) = bot.get("channels").and_then(Value::as_array) else { continue };
        for channel in channels {
            if let Some(secrets) = channel.get("secrets").filter(|value| !value.is_null()) {
                for (name, value) in secrets.as_object().ok_or("Channel secrets must be a name-to-env map.")? {
                    let reference = value.as_str().ok_or("Channel credentials must be env:NAME references.")?;
                    let env_name = reference.strip_prefix("env:").ok_or("Git manifests must not contain plaintext channel credentials.")?;
                    if env_name.is_empty() || !env_name.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') {
                        return Err(format!("Channel {name} has an invalid environment reference."));
                    }
                }
            }
            if let Some(config) = channel.get("config").filter(|value| !value.is_null()) {
                let object = config.as_object().ok_or("Channel config must be an object.")?;
                let kind = channel.get("kind").and_then(Value::as_str).unwrap_or("");
                for (key, value) in object {
                    if !matches!((kind, key.as_str()), ("web", "remote") if value.is_boolean())
                        && !matches!((kind, key.as_str()), ("web" | "whatsapp", "publicUrl") if value.is_string())
                        && !matches!((kind, key.as_str()), ("whatsapp", "phoneNumberId") if value.is_string()) {
                        return Err(format!("Git manifest has unsupported or sensitive channel config key: {key}"));
                    }
                    if key == "publicUrl" {
                        crate::headless_bots::validate_public_url(value.as_str().unwrap_or_default())?;
                    }
                    if key == "phoneNumberId" && !value.as_str()
                        .is_some_and(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit())) {
                        return Err("WhatsApp phoneNumberId must contain only digits.".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn aggregate(
    documents: Vec<(String, String, Value)>,
) -> Result<Value> {
    let mut bots_seen: BTreeMap<String, (Value, String)> = BTreeMap::new();
    let mut channels_seen: BTreeMap<String, (Value, String)> = BTreeMap::new();
    for (source, path, doc) in documents {
        extended_bot_manifest::verify_manifest(&doc)?;
        secret_reference_only(&doc)?;
        for entry in doc["bots"].as_array().ok_or("Missing bots array.")? {
            let name = entry.get("name").and_then(Value::as_str).ok_or("Every bot needs a name.")?;
            let key = bots::clean(name, 60);
            if key.is_empty() { return Err("Every bot needs a canonical name.".into()); }
            let origin = format!("{source}:{path}");
            if let Some((previous, from)) = bots_seen.get(&key) {
                if previous != entry { return Err(format!("Conflicting bot {key:?} in {from} and {origin}.")); }
            } else { bots_seen.insert(key, (entry.clone(), origin)); }
        }
        if let Some(channels) = doc.get("sharedChannels").and_then(Value::as_array) {
            for entry in channels {
                let name = entry.get("name").and_then(Value::as_str).ok_or("Every shared channel needs a name.")?;
                let key = name.trim_start_matches('#').trim().to_ascii_lowercase();
                if key.is_empty() { return Err("Every shared channel needs a name.".into()); }
                let origin = format!("{source}:{path}");
                if let Some((previous, from)) = channels_seen.get(&key) {
                    if previous != entry { return Err(format!("Conflicting shared channel {key:?} in {from} and {origin}.")); }
                } else { channels_seen.insert(key, (entry.clone(), origin)); }
            }
        }
    }
    let doc = json!({
        "bots": bots_seen.into_values().map(|(value, _)| value).collect::<Vec<_>>(),
        "sharedChannels": channels_seen.into_values().map(|(value, _)| value).collect::<Vec<_>>(),
    });
    extended_bot_manifest::verify_manifest(&doc)?;
    Ok(doc)
}

/// Each mutable Git ref is resolved once; all manifest files are read at that exact commit.
async fn fetch_batch(registry: &SourceRegistry) -> Result<(Value, Vec<SourceReceipt>)> {
    validate_registry(registry)?;
    if registry.sources.is_empty() { return Err("Configure at least one Git source before syncing.".into()); }
    let client = client()?;
    let mut documents = Vec::new();
    let mut receipts = Vec::new();
    for source in &registry.sources {
        let (owner, repo) = github_repo(&source.repository)?;
        let mut url = api_url(&owner, &repo, &["commits"])?;
        url.query_pairs_mut().append_pair("sha", &source.reference).append_pair("per_page", "1");
        let commits = get_api(&client, url).await?;
        let sha = commits.as_array().and_then(|arr| arr.first()).and_then(|commit| commit["sha"].as_str())
            .ok_or_else(|| format!("Source {} has no accessible commit for the selected ref.", source.id))?.to_owned();
        if sha.len() != 40 || !sha.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("Source {} did not resolve to a commit SHA.", source.id));
        }
        let mut hashes = Vec::new();
        for path in &source.manifests {
            let mut url = api_url(&owner, &repo, &["contents"])?;
            url.path_segments_mut().map_err(|_| "Invalid manifest path.")?.extend(path.split('/'));
            url.query_pairs_mut().append_pair("ref", &sha);
            let item = get_api(&client, url).await?;
            if item["type"] != "file" || item["encoding"] != "base64" {
                return Err(format!("Source {}: {} must be a regular JSON file.", source.id, path));
            }
            let data = item["content"].as_str().ok_or("GitHub did not include file contents.")?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data.as_bytes().iter().copied().filter(|b| !b.is_ascii_whitespace()).collect::<Vec<_>>())
                .map_err(|_| format!("Source {}: {} has invalid base64.", source.id, path))?;
            if bytes.len() > MAX_SOURCE_BYTES { return Err(format!("Source {}: {} is too large.", source.id, path)); }
            let doc: Value = serde_json::from_slice(&bytes)
                .map_err(|_| format!("Source {}: {} is not valid JSON.", source.id, path))?;
            hashes.push(read_hash(&bytes));
            documents.push((source.id.clone(), path.clone(), doc));
        }
        receipts.push(SourceReceipt { id: source.id.clone(), repository: source.repository.clone(),
            reference: source.reference.clone(), commit: sha, paths: source.manifests.clone(), hashes });
    }
    Ok((aggregate(documents)?, receipts))
}

fn registry_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf> {
    Ok(app.path().app_data_dir().map_err(|error| error.to_string())?.join("bot-manifest-sources.json"))
}

fn read_registry(path: &Path) -> Result<SourceRegistry> {
    if !path.exists() { return Ok(SourceRegistry::default()); }
    let bytes = fs::read(path).map_err(|error| format!("Cannot read source registry: {error}"))?;
    if bytes.len() > MAX_SOURCE_BYTES { return Err("Source registry is too large.".into()); }
    let registry: SourceRegistry = serde_json::from_slice(&bytes).map_err(|error| format!("Invalid source registry: {error}"))?;
    validate_registry(&registry)?;
    Ok(registry)
}

// Serialize registry mutation and remote sync across async Desktop commands.
fn registry_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Only the native backend can advance source provenance. Frontend edits to a source
/// cannot manufacture a "last applied" revision, even when the source ID is reused.
fn preserve_receipts(previous: &SourceRegistry, updated: &mut SourceRegistry) {
    for source in &mut updated.sources {
        let matching = previous.sources.iter().find(|saved| saved.id == source.id
            && saved.repository == source.repository
            && saved.reference == source.reference
            && saved.manifests == source.manifests);
        source.last_checked_sha = matching.and_then(|old| old.last_checked_sha.clone());
        source.last_applied_sha = matching.and_then(|old| old.last_applied_sha.clone());
        source.last_applied_hash = matching.and_then(|old| old.last_applied_hash.clone());
        source.last_applied_local_hash = matching.and_then(|old| old.last_applied_local_hash.clone());
    }
}

fn save_registry(path: &Path, registry: &SourceRegistry) -> Result<()> {
    validate_registry(registry)?;
    let parent = path.parent().ok_or("Missing source registry directory.")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(".manifest-sources-{}.json", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        fs::write(&temp, serde_json::to_vec_pretty(registry).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        fs::rename(&temp, path).map_err(|error| error.to_string())?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(temp); }
    result
}

/// Hash exported portable configuration, not transient process state or credentials.
fn local_state_hash(store: &Store) -> Result<String> {
    let current = headless_bots::export(store)?;
    Ok(read_hash(&serde_json::to_vec(&current).map_err(|error| error.to_string())?))
}

fn receipt_hash(receipt: &SourceReceipt) -> String {
    read_hash(receipt.hashes.join("\n").as_bytes())
}

fn same_applied_content(registry: &SourceRegistry, receipts: &[SourceReceipt], local_hash: &str) -> bool {
    !registry.sources.is_empty() && registry.sources.len() == receipts.len()
        && registry.sources.iter().all(|source| receipts.iter().any(|receipt|
            source.id == receipt.id
                && source.last_applied_hash.as_deref() == Some(receipt_hash(receipt).as_str())
                && source.last_applied_local_hash.as_deref() == Some(local_hash)))
}

/// Count actual portable-config deltas, not attempted SQL updates or log messages.
fn bot_delta_counts(doc: &Value, before: &Value, after: &Value) -> (usize, usize, usize) {
    let indexed = |root: &Value| -> BTreeMap<String, Value> {
        root["bots"].as_array().into_iter().flatten()
            .filter_map(|item| item.get("name").and_then(Value::as_str)
                .map(|name| (bots::clean(name, 60), item.clone())))
            .collect()
    };
    let old = indexed(before);
    let new = indexed(after);
    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let requested = indexed(doc);
    for name in requested.keys() {
        match (old.get(name), new.get(name)) {
            (None, Some(_)) => created += 1,
            (Some(before), Some(after)) if before != after => updated += 1,
            (Some(_), Some(_)) => unchanged += 1,
            _ => {},
        }
    }
    (created, updated, unchanged)
}

async fn check_sources(registry: &mut SourceRegistry) -> Result<SyncOutcome> {
    let started_at = crate::storage::now();
    let (_, receipts) = fetch_batch(registry).await?;
    for source in &mut registry.sources {
        source.last_checked_sha = receipts.iter().find(|item| item.id == source.id).map(|item| item.commit.clone());
    }
    Ok(SyncOutcome {
        sources: receipts, backup_path: None, lines: vec!["All selected Git manifests were validated without changing bots.".into()],
        failed: 0, created: 0, updated: 0, unchanged: 0,
        started_at, finished_at: crate::storage::now(), applied: false, no_changes: false,
    })
}

pub(crate) async fn sync(
    app: &tauri::AppHandle, registry: &mut SourceRegistry,
) -> Result<SyncOutcome> {
    let started_at = crate::storage::now();
    let (document, receipts) = fetch_batch(registry).await?;
    let dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let (result, local_hash, added_channels) = {
        let store: std::sync::MutexGuard<'_, Store> = state.store.lock().map_err(|error| error.to_string())?;
        let current_hash = local_state_hash(&store)?;
        if same_applied_content(registry, &receipts, &current_hash) {
            let unchanged = document["bots"].as_array().map_or(0, Vec::len);
            (SyncOutcome { sources: receipts, backup_path: None,
                lines: vec!["Git manifest content and local bot configuration are unchanged; no import was necessary.".into()],
                failed: 0, created: 0, updated: 0, unchanged,
                started_at, finished_at: crate::storage::now(), applied: true, no_changes: true,
            }, current_hash, Vec::new())
        } else {
            let before = headless_bots::export(&store)?;
            let (report, backup) = extended_bot_manifest::apply_with_backup(
                &store, &dir, &document, crate::storage::now())?;
            let after = headless_bots::export(&store)?;
            let (created, updated, unchanged) = bot_delta_counts(&document, &before, &after);
            let local_hash = read_hash(&serde_json::to_vec(&after).map_err(|error| error.to_string())?);
            (SyncOutcome { sources: receipts, backup_path: Some(backup.display().to_string()),
                lines: report.lines, failed: report.failed, created, updated, unchanged,
                started_at, finished_at: crate::storage::now(),
                applied: report.failed == 0, no_changes: false,
            }, local_hash, report.added_channels)
        }
    };
    let mut result = result;
    for added in &added_channels {
        let channel = {
            let store = state.store.lock().map_err(|error| error.to_string())?;
            bots::channels::get(&store, &added.id)?
        };
        bots::channels::start(app, channel);
    }
    for source in &mut registry.sources {
        if let Some(receipt) = result.sources.iter().find(|item| item.id == source.id) {
            source.last_checked_sha = Some(receipt.commit.clone());
            if result.applied {
                source.last_applied_sha = Some(receipt.commit.clone());
                source.last_applied_hash = Some(receipt_hash(receipt));
                source.last_applied_local_hash = Some(local_hash.clone());
            }
        }
    }
    if result.failed > 0 {
        result.lines.push("The batch was only partially applied; last-applied revisions were not advanced.".into());
    }
    if !result.no_changes {
        let _ = app.emit("bots-changed", ());
        let _ = app.emit("bot-shared-channel-changed", json!({}));
    }
    Ok(result)
}

/// Keep headless receipts in private application data, not beside or inside the
/// user-supplied source registry, which may be checked into Git or read-only.
fn headless_receipts_path(data_dir: &Path, source: &Path) -> Result<std::path::PathBuf> {
    let canonical = source.canonicalize()
        .map_err(|error| format!("Cannot resolve the Git source registry: {error}"))?;
    let folder = data_dir.join("bot-manifest-headless-state");
    extended_bot_manifest::set_private_directory(&folder)?;
    Ok(folder.join(format!("{}.json", read_hash(canonical.to_string_lossy().as_bytes()))))
}

/// The source file defines intent; only our private state file can supply trusted
/// last-checked and last-applied receipts for matching source identities.
fn read_headless_registry(source: &Path, receipts: &Path) -> Result<SourceRegistry> {
    let mut registry = read_registry(source)?;
    let persisted = read_registry(receipts)?;
    preserve_receipts(&persisted, &mut registry);
    Ok(registry)
}

pub(crate) async fn run_headless(
    app: &tauri::AppHandle, path: &Path,
) -> Result<SyncOutcome> {
    let _operation = registry_lock().lock().await;
    let data_dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
    let receipts = headless_receipts_path(&data_dir, path)?;
    let mut registry = read_headless_registry(path, &receipts)?;
    if registry.sources.is_empty() { return Err("The Git source registry has no sources.".into()); }
    let outcome = sync(app, &mut registry).await?;
    // Persist successful receipts and partial-failure check results before reporting
    // completion; a later one-shot run must not repeatedly import unchanged content.
    save_registry(&receipts, &registry)?;
    Ok(outcome)
}

fn check_main(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" { return Err("Git bot manifests can only be managed from the main window.".into()); }
    Ok(())
}

#[tauri::command]
pub async fn bot_manifest_sources_list(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<SourceRegistry> {
    check_main(&window)?;
    read_registry(&registry_path(&app)?)
}

#[tauri::command]
pub async fn bot_manifest_sources_save(
    app: tauri::AppHandle, window: tauri::WebviewWindow, registry: SourceRegistry,
) -> Result<SourceRegistry> {
    check_main(&window)?;
    let _operation = registry_lock().lock().await;
    let path = registry_path(&app)?;
    let previous = read_registry(&path)?;
    let mut updated_registry = registry;
    preserve_receipts(&previous, &mut updated_registry);
    save_registry(&path, &updated_registry)?;
    Ok(updated_registry)
}

#[tauri::command]
pub async fn bot_manifest_sources_check(
    app: tauri::AppHandle, window: tauri::WebviewWindow,
) -> Result<SyncOutcome> {
    check_main(&window)?;
    let _operation = registry_lock().lock().await;
    let path = registry_path(&app)?;
    let mut registry = read_registry(&path)?;
    let receipt = check_sources(&mut registry).await?;
    save_registry(&path, &registry)?;
    Ok(receipt)
}

#[tauri::command]
pub async fn bot_manifest_sources_sync(
    app: tauri::AppHandle, window: tauri::WebviewWindow,
) -> Result<SyncOutcome> {
    check_main(&window)?;
    let _operation = registry_lock().lock().await;
    let path = registry_path(&app)?;
    let mut registry = read_registry(&path)?;
    let result = sync(&app, &mut registry).await?;
    save_registry(&path, &registry)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_source_content_can_skip_import_only_when_local_config_has_not_drifted() {
        let receipt = SourceReceipt {
            id: "source".into(), repository: "https://github.com/team/config".into(),
            reference: "main".into(), commit: "new-pinned-sha".into(),
            paths: vec!["bots.json".into()], hashes: vec!["same-content".into()],
        };
        let registry = SourceRegistry { version: 1, sources: vec![ManifestSource {
            id: receipt.id.clone(), repository: receipt.repository.clone(),
            reference: receipt.reference.clone(), manifests: receipt.paths.clone(), auto_sync: false,
            last_checked_sha: Some("older-commit".into()),
            last_applied_sha: Some("older-commit".into()),
            last_applied_hash: Some(receipt_hash(&receipt)),
            last_applied_local_hash: Some("local-state".into()),
        }] };
        assert!(same_applied_content(&registry, std::slice::from_ref(&receipt), "local-state"));
        assert!(!same_applied_content(&registry, std::slice::from_ref(&receipt), "changed-locally"));
        let mut changed = receipt.clone();
        changed.hashes = vec!["changed-content".into()];
        assert!(!same_applied_content(&registry, &[changed], "local-state"));
        let output = serde_json::to_value(&receipt).unwrap();
        assert_eq!(output["repository"], receipt.repository);
        assert_eq!(output["ref"], receipt.reference);
        assert_eq!(output["paths"], json!(["bots.json"]));
    }

    #[test]
    fn sync_counts_actual_portable_configuration_changes() {
        let prior = json!({"bots":[
            {"name":"existing","mode":"ask"},
            {"name":"same","mode":"ask"}]});
        let after = json!({"bots":[
            {"name":"existing","mode":"full"},
            {"name":"same","mode":"ask"},
            {"name":"new","mode":"ask"}]});
        let request = json!({"bots":[{"name":"existing"},{"name":"same"},{"name":"new"}]});
        assert_eq!(bot_delta_counts(&request, &prior, &after), (1, 1, 1));
    }

    #[test]
    fn one_shot_headless_runs_reuse_private_receipts_without_mutating_source_registry() {
        let root = std::env::temp_dir().join(format!("pandacode-headless-receipts-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source_path = root.join("sources.json");
        let original = SourceRegistry { version: 1, sources: vec![ManifestSource {
            id: "team".into(), repository: "https://github.com/team/bots".into(),
            reference: "main".into(), manifests: vec!["bots.json".into()], auto_sync: false,
            last_checked_sha: None, last_applied_sha: None, last_applied_hash: None,
            last_applied_local_hash: None,
        }] };
        let source_bytes = serde_json::to_vec_pretty(&original).unwrap();
        fs::write(&source_path, &source_bytes).unwrap();
        let receipts_path = headless_receipts_path(&root, &source_path).unwrap();
        assert_eq!(headless_receipts_path(&root, &source_path).unwrap(), receipts_path);
        let mut first = read_headless_registry(&source_path, &receipts_path).unwrap();
        assert_eq!(first.sources[0].last_applied_sha, None);

        let receipt = SourceReceipt {
            id: "team".into(), repository: "https://github.com/team/bots".into(),
            reference: "main".into(), commit: "pinned-sha".into(),
            paths: vec!["bots.json".into()], hashes: vec!["content-hash".into()],
        };
        first.sources[0].last_checked_sha = Some(receipt.commit.clone());
        first.sources[0].last_applied_sha = Some(receipt.commit.clone());
        first.sources[0].last_applied_hash = Some(receipt_hash(&receipt));
        first.sources[0].last_applied_local_hash = Some("local-state".into());
        save_registry(&receipts_path, &first).unwrap();

        let resumed = read_headless_registry(&source_path, &receipts_path).unwrap();
        assert!(same_applied_content(&resumed, std::slice::from_ref(&receipt), "local-state"),
            "an unchanged second invocation must take the no-import/no-backup path");
        assert!(!same_applied_content(&resumed, std::slice::from_ref(&receipt), "local-drift"));
        assert_eq!(fs::read(&source_path).unwrap(), source_bytes,
            "the user-owned Git source registry must remain read-only");

        let mut changed = original.clone();
        changed.sources[0].manifests = vec!["different.json".into()];
        fs::write(&source_path, serde_json::to_vec_pretty(&changed).unwrap()).unwrap();
        let reconfigured = read_headless_registry(&source_path, &receipts_path).unwrap();
        assert_eq!(reconfigured.sources[0].last_applied_sha, None,
            "changing the source identity must invalidate prior receipts");

        let other_source = root.join("another-source.json");
        fs::write(&other_source, serde_json::to_vec_pretty(&original).unwrap()).unwrap();
        assert_ne!(headless_receipts_path(&root, &other_source).unwrap(), receipts_path,
            "different registry paths must not share receipt storage");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn registry_edits_cannot_forge_or_carry_stale_apply_receipts() {
        let saved = SourceRegistry { version: 1, sources: vec![ManifestSource {
            id: "source".into(), repository: "https://github.com/team/config".into(),
            reference: "main".into(), manifests: vec!["bots.json".into()], auto_sync: false,
            last_checked_sha: Some("trusted-checked".into()),
            last_applied_sha: Some("trusted-applied".into()),
            last_applied_hash: Some("trusted-hash".into()),
            last_applied_local_hash: Some("trusted-local".into()),
        }] };
        let mut forged = saved.clone();
        forged.sources[0].last_applied_sha = Some("forged".into());
        preserve_receipts(&saved, &mut forged);
        assert_eq!(forged.sources[0].last_applied_sha.as_deref(), Some("trusted-applied"));
        assert_eq!(forged.sources[0].last_applied_local_hash.as_deref(), Some("trusted-local"));
        forged.sources[0].manifests = vec!["different.json".into()];
        preserve_receipts(&saved, &mut forged);
        assert_eq!(forged.sources[0].last_applied_sha, None);
        assert_eq!(forged.sources[0].last_checked_sha, None);
        assert_eq!(forged.sources[0].last_applied_local_hash, None);
    }

    #[test]
    fn rejects_unsafe_remote_identity_and_paths() {
        for repo in ["https://user:token@github.com/acme/repo", "http://github.com/a/b",
            "https://github.com.evil.test/a/b", "https://github.com/a/b/extra",
            "https://github.com/a/b?token=oops"] {
            assert!(github_repo(repo).is_err(), "{repo}");
        }
        let good = SourceRegistry { version:1, sources:vec![ManifestSource {
            id:"team".into(), repository:"https://github.com/acme/config".into(),
            reference:"main".into(), manifests:vec!["bots/one.json".into()], auto_sync:false,
            last_checked_sha:None, last_applied_sha:None, last_applied_hash:None,
            last_applied_local_hash:None,
        }] };
        assert!(validate_registry(&good).is_ok());
        for bad in ["../secrets.json", "/abs.json", "bots/../secret.json", "bots\\secret.json"] {
            let mut invalid = good.clone();
            invalid.sources[0].manifests = vec![bad.into()];
            assert!(validate_registry(&invalid).is_err(), "{bad}");
        }
    }
    #[test]
    fn conflicting_canonical_bots_block_all_mutations() {
        let left = json!({"bots":[{"name":"A","provider":"claude","model":"x","mode":"auto"}]});
        let right = json!({"bots":[{"name":"A","provider":"claude","model":"y","mode":"auto"}]});
        let docs = vec![("one".into(),"one.json".into(),left),("two".into(),"two.json".into(),right)];
        assert!(aggregate(docs).unwrap_err().contains("Conflicting bot"));
    }
    #[test]
    fn identical_duplicates_are_deduplicated() {
        let doc = json!({"bots":[{"name":"A","provider":"claude","model":"x","mode":"auto"}]});
        let out = aggregate(vec![("one".into(),"a.json".into(),doc.clone()),("two".into(),"b.json".into(),doc)]).unwrap();
        assert_eq!(out["bots"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn remote_manifests_allow_env_bindings_but_not_plaintext() {
        let safe = json!({"bots":[{"name":"A","channels":[{"kind":"telegram","secrets":{"token":"env:TELEGRAM_TOKEN"}}]}]});
        assert!(secret_reference_only(&safe).is_ok());
        let bad = json!({"bots":[{"name":"A","channels":[{"kind":"telegram","secrets":{"token":"raw-password"}}]}]});
        assert!(secret_reference_only(&bad).is_err());
    }
}
