//! GitHub-backed, SHA-pinned bot manifest sources. No repository code is executed.
//! Source content is validated before the shared import+backup gate is entered.
use crate::{bots, extended_bot_manifest, storage::Store, AppState, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use tauri::{Emitter, Manager};

const MAX_SOURCES: usize = 16;
const MAX_FILES: usize = 32;
const MAX_SOURCE_BYTES: usize = 512 * 1024;

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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SourceRegistry {
    pub version: u32,
    pub sources: Vec<ManifestSource>,
}

impl Default for SourceRegistry {
    fn default() -> Self { Self { version: 1, sources: Vec::new() } }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceReceipt {
    pub id: String,
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
    pub applied: bool,
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
    if response.content_length().is_some_and(|size| size as usize > MAX_SOURCE_BYTES) {
        return Err("GitHub manifest response exceeds the file-size limit.".into());
    }
    let bytes = response.bytes().await.map_err(|_| "Could not read the GitHub response.")?;
    if bytes.len() > MAX_SOURCE_BYTES { return Err("GitHub response is too large.".into()); }
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
        receipts.push(SourceReceipt { id: source.id.clone(), commit: sha, paths: source.manifests.clone(), hashes });
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

async fn check_sources(registry: &mut SourceRegistry) -> Result<SyncOutcome> {
    let (_, receipts) = fetch_batch(registry).await?;
    for source in &mut registry.sources {
        source.last_checked_sha = receipts.iter().find(|item| item.id == source.id).map(|item| item.commit.clone());
    }
    Ok(SyncOutcome {
        sources: receipts, backup_path: None, lines: vec!["All selected Git manifests were validated without changing bots.".into()],
        failed: 0, applied: false,
    })
}

pub(crate) async fn sync(
    app: &tauri::AppHandle, registry: &mut SourceRegistry,
) -> Result<SyncOutcome> {
    let (document, receipts) = fetch_batch(registry).await?;
    let dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let (report, backup) = {
        let store: std::sync::MutexGuard<'_, Store> = state.store.lock().map_err(|error| error.to_string())?;
        extended_bot_manifest::apply_with_backup(&store, &dir, &document, crate::storage::now())?
    };
    let mut result = SyncOutcome {
        sources: receipts, backup_path: Some(backup.display().to_string()),
        lines: report.lines, failed: report.failed, applied: report.failed == 0,
    };
    for added in &report.added_channels {
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
                source.last_applied_hash = Some(read_hash(receipt.hashes.join("\n").as_bytes()));
            }
        }
    }
    if report.failed > 0 {
        result.lines.push("The batch was only partially applied; last-applied revisions were not advanced.".into());
    }
    let _ = app.emit("bots-changed", ());
    let _ = app.emit("bot-shared-channel-changed", json!({}));
    Ok(result)
}

pub(crate) async fn run_headless(
    app: &tauri::AppHandle, path: &Path,
) -> Result<SyncOutcome> {
    let registry = read_registry(path)?;
    let mut registry = registry;
    if registry.sources.is_empty() { return Err("The Git source registry has no sources.".into()); }
    sync(app, &mut registry).await
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
    save_registry(&registry_path(&app)?, &registry)?;
    Ok(registry)
}

#[tauri::command]
pub async fn bot_manifest_sources_check(
    app: tauri::AppHandle, window: tauri::WebviewWindow,
) -> Result<SyncOutcome> {
    check_main(&window)?;
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
