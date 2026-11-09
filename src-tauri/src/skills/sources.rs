//! User-configured GitHub skill collections. A scan is read-only and ephemeral;
//! only the explicitly selected packages are downloaded into the library.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, sync::{atomic::{AtomicBool, Ordering}, Mutex}};

use super::{catalog, github, now_ms, sha256_hex, Kind, Library, Source, MAX_BYTES, MAX_FILES};

const MAX_SAVED_SOURCES: usize = 64;
static SOURCES_LOCK: Mutex<()> = Mutex::new(());
static IMPORT_ACTIVE: AtomicBool = AtomicBool::new(false);

struct ImportGuard;
impl Drop for ImportGuard {
    fn drop(&mut self) { IMPORT_ACTIVE.store(false, Ordering::Release); }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSource {
    pub id: String,
    pub repo: String,
    /// A folder inside the repository, empty for the root.
    pub path: String,
    pub git_ref: Option<String>,
    /// Last inspected commit used to import selected packages.
    pub commit: String,
    pub selected_paths: Vec<String>,
    pub saved_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SourceStore {
    #[serde(default)]
    sources: Vec<SavedSource>,
}

/// Recovery intent written before installing any package. Only packages whose
/// committed metadata matches this exact import are enrolled after a crash.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingSourceImport {
    id: String,
    repo: String,
    path: String,
    git_ref: Option<String>,
    commit: String,
    selected_paths: Vec<String>,
    started_at: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePreview {
    pub repo: String,
    pub path: String,
    pub git_ref: Option<String>,
    pub commit: String,
    pub skills: Vec<catalog::CatalogSkill>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceImport {
    pub source: SavedSource,
    pub imported: Vec<String>,
    pub skipped: Vec<catalog::Skipped>,
    /// Authoritative per-package terminal results even if a webview missed events.
    pub results: Vec<SourceProgress>,
}

/// A frozen import request; grouping source coordinates prevents argument drift.
pub struct SourceImportRequest<'a> {
    pub repo: &'a str,
    pub path: &'a str,
    pub git_ref: Option<&'a str>,
    pub commit: &'a str,
    pub selected_paths: Vec<String>,
    pub allow_replace: bool,
}

/// Incremental progress for one package in a frozen GitHub import batch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProgress {
    pub path: String,
    pub name: String,
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    pub detail: Option<String>,
}

fn id_for(repo: &str, path: &str, git_ref: Option<&str>) -> String {
    let digest = sha256_hex(format!("{}\n{path}\n{}", repo.to_ascii_lowercase(), git_ref.unwrap_or("")).as_bytes());
    digest[..16].to_string()
}

impl Library {
    fn sources_path(&self) -> std::path::PathBuf { self.dir.join(".skill-sources.json") }
    fn sources_backup_path(&self) -> std::path::PathBuf { self.dir.join(".skill-sources.json.rollback") }
    fn pending_import_path(&self) -> std::path::PathBuf { self.dir.join(".skill-sources.pending.json") }

    fn stage_pending_import(&self, pending: &PendingSourceImport) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.pending_import_path();
        let temp = self.dir.join(".skill-sources.pending.json.tmp");
        let content = serde_json::to_vec(pending).map_err(|e| e.to_string())?;
        let result = fs::write(&temp, content).and_then(|_| fs::rename(&temp, &path));
        if result.is_err() { let _ = fs::remove_file(&temp); }
        result.map_err(|e| format!("Could not stage skill source recovery: {e}"))
    }

    /// Recover an interrupted file replacement on Windows and Unix.
    fn read_sources(&self) -> Result<SourceStore, String> {
        let current = self.sources_path();
        let backup = self.sources_backup_path();
        if backup.exists() {
            if current.exists() {
                fs::remove_file(&backup).map_err(|e| format!("Could not finish source state recovery: {e}"))?;
            } else {
                fs::rename(&backup, &current).map_err(|e| format!("Could not restore saved source state: {e}"))?;
            }
        }
        let mut store: SourceStore = match fs::read(current) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("Saved skill sources are unreadable: {e}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => SourceStore::default(),
            Err(error) => return Err(error.to_string()),
        };
        // Another view may inspect sources while an import is active. Only a
        // subsequent process (or an idle process after failure) reconciles the journal.
        if !IMPORT_ACTIVE.load(Ordering::Acquire) && self.pending_import_path().exists() {
            let pending: PendingSourceImport = serde_json::from_slice(
                &fs::read(self.pending_import_path()).map_err(|e| e.to_string())?
            ).map_err(|e| format!("Saved skill import recovery is unreadable: {e}"))?;
            let installed = self.list()?;
            let complete: BTreeSet<String> = pending.selected_paths.iter().filter(|path| {
                let location = catalog::location(&pending.repo, path);
                installed.iter().any(|item| item.meta.kind == Kind::Skill
                    && item.meta.source.kind == "github"
                    && item.meta.source.location == location
                    && item.meta.source.git_ref == pending.git_ref
                    && item.meta.source.commit.as_deref() == Some(pending.commit.as_str())
                    && item.meta.imported_at >= pending.started_at)
            }).cloned().collect();
            if !complete.is_empty() {
                let mut selected = complete;
                if let Some(previous) = store.sources.iter().find(|s| s.id == pending.id) {
                    selected.extend(previous.selected_paths.iter().cloned());
                }
                let recovered = SavedSource {
                    id: pending.id.clone(), repo: pending.repo.clone(),
                    path: pending.path.clone(), git_ref: pending.git_ref.clone(),
                    commit: pending.commit.clone(), selected_paths: selected.into_iter().collect(),
                    saved_at: now_ms(),
                };
                if let Some(existing) = store.sources.iter_mut().find(|s| s.id == pending.id) {
                    *existing = recovered;
                } else if store.sources.len() < MAX_SAVED_SOURCES {
                    store.sources.push(recovered);
                } else {
                    return Err("Cannot recover a skill import: saved source limit reached.".into());
                }
                store.sources.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.path.cmp(&b.path)));
                self.save_sources(&store)?;
            }
            fs::remove_file(self.pending_import_path()).map_err(|e| format!("Could not clear recovered skill source intent: {e}"))?;
        }
        Ok(store)
    }

    fn save_sources(&self, store: &SourceStore) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?;
        let temporary = self.dir.join(".skill-sources.json.tmp");
        let target = self.sources_path();
        let backup = self.sources_backup_path();
        fs::write(&temporary, bytes).map_err(|e| format!("Could not stage selected skill sources: {e}"))?;
        if target.exists() {
            if let Err(error) = fs::rename(&target, &backup) {
                let _ = fs::remove_file(&temporary);
                return Err(format!("Could not stage the previous source configuration: {error}"));
            }
        }
        if let Err(error) = fs::rename(&temporary, &target) {
            if backup.exists() { let _ = fs::rename(&backup, &target); }
            let _ = fs::remove_file(&temporary);
            return Err(format!("Could not save selected skill sources: {error}"));
        }
        if backup.exists() { let _ = fs::remove_file(&backup); }
        Ok(())
    }

    pub fn saved_skill_sources(&self) -> Result<Vec<SavedSource>, String> {
        let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
        Ok(self.read_sources()?.sources)
    }

    /// Uses gh api through the supplied adapter. No persistence, checkout or package install.
    pub fn preview_skill_source(&self, api: catalog::Api<'_>, repo: &str, path: &str, git_ref: Option<&str>) -> Result<SourcePreview, String> {
        let (repo, path, git_ref) = github::selection(repo, path, git_ref)?;
        let (commit, skills) = catalog::scan_path(api, &repo, &path, git_ref.as_deref())
            .map_err(|e| catalog::explain(&e))?;
        if skills.is_empty() {
            return Err(format!("No valid SKILL.md packages found in {repo}/{}.", if path.is_empty() { "(root)" } else { &path }));
        }
        Ok(SourcePreview { repo, path, git_ref, commit, skills })
    }

    /// Imports only exact paths selected from the scan, and persists only successfully
    /// imported paths. Reconfiguring a source never silently deletes old library copies.
    pub fn import_skill_source(
        &self, api: catalog::Api<'_>, repo: &str, path: &str, git_ref: Option<&str>,
        commit: &str, selected_paths: Vec<String>,
    ) -> Result<SourceImport, String> {
        self.import_skill_source_progress(api, SourceImportRequest {
            repo, path, git_ref, commit, selected_paths, allow_replace: false,
        }, |_| {})
    }

    /// An explicit user choice may replace a different GitHub/ZIP origin, never implicitly.
    pub fn import_skill_source_progress(
        &self, api: catalog::Api<'_>, request: SourceImportRequest<'_>,
        mut progress: impl FnMut(SourceProgress),
    ) -> Result<SourceImport, String> {
        let SourceImportRequest { repo, path, git_ref, commit, selected_paths, allow_replace } = request;
        let (repo, path, git_ref) = github::selection(repo, path, git_ref)?;
        if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Rescan this source: the selected commit is not a full Git SHA.".into());
        }
        let chosen: BTreeSet<String> = selected_paths.into_iter().collect();
        if chosen.is_empty() || chosen.len() > catalog::MAX_SKILLS {
            return Err("Select between one and 300 skills from the scanned collection.".into());
        }
        let id = id_for(&repo, &path, git_ref.as_deref());
        {
            let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
            let existing = self.read_sources()?;
            if existing.sources.len() >= MAX_SAVED_SOURCES
                && existing.sources.iter().all(|s| s.repo != repo || s.path != path || s.git_ref != git_ref) {
                return Err(format!("At most {MAX_SAVED_SOURCES} saved sources are supported. Remove an unused configuration first."));
            }
        }
        let (verified_commit, indexed) = catalog::scan_path(api, &repo, &path, Some(commit))
            .map_err(|e| catalog::explain(&e))?;
        if verified_commit != commit {
            return Err("The Git commit changed during verification. Rescan the source.".into());
        }
        let indexed: std::collections::BTreeMap<String, catalog::CatalogSkill> =
            indexed.into_iter().map(|s| (s.path.clone(), s)).collect();
        if chosen.iter().any(|path| !indexed.contains_key(path)) {
            return Err("At least one selected skill is outside this collection or no longer exists. Rescan.".into());
        }
        let mut seen = BTreeSet::new();
        if chosen.iter().any(|path| !seen.insert(indexed[path].id.clone())) {
            return Err("Two selected paths use the same skill ID. Choose only one source for that ID.".into());
        }

        if IMPORT_ACTIVE.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return Err("Another GitHub skill import is still running.".into());
        }
        let _guard = ImportGuard;
        self.stage_pending_import(&PendingSourceImport {
            id: id.clone(), repo: repo.clone(), path: path.clone(),
            git_ref: git_ref.clone(), commit: commit.to_string(),
            selected_paths: chosen.iter().cloned().collect(), started_at: now_ms(),
        })?;
        let mut imported = Vec::new();
        let mut skipped = Vec::new();
        let mut results = Vec::new();
        let mut successful_paths = Vec::new();
        let total = chosen.len();
        let mut completed = 0usize;
        for skill_path in &chosen {
            let skill = &indexed[skill_path];
            let skip = |reason: String| catalog::Skipped {
                path: skill_path.clone(), name: skill.name.clone(), reason,
            };
            let mut report = |phase: &str, current: usize, detail: Option<String>| {
                let event = SourceProgress {
                    path: skill_path.clone(), name: skill.name.clone(), phase: phase.into(),
                    completed: current, total, detail,
                };
                if matches!(phase, "installed" | "updated" | "unchanged" | "skipped" | "failed") {
                    results.push(event.clone());
                }
                progress(event);
            };
            if skill.files > MAX_FILES || skill.bytes > MAX_BYTES as u64 {
                let reason = format!("{} exceeds the validated package size or file limit.", skill.name);
                skipped.push(skip(reason.clone()));
                completed += 1;
                report("skipped", completed, Some(reason));
                continue;
            }
            report("fetching", completed, None);
            let result = (|| -> Result<&'static str, String> {
                let (files, actual_commit) = github::fetch_using(&repo, skill_path, Some(commit), api)
                    .map_err(|e| catalog::explain(&e))?;
                if actual_commit != commit { return Err("The fetched commit does not match the selected revision.".into()); }
                report("validating", completed, None);
                let location = catalog::location(&repo, skill_path);
                let previous = self.load(&skill.id).ok();
                if previous.as_ref().is_some_and(|meta| meta.source.kind == "github"
                    && meta.source.location == location && meta.source.git_ref == git_ref
                    && meta.source.commit.as_deref() == Some(commit)
                    && meta.sha256 == super::content_hash(&files)) {
                    return Ok("unchanged");
                }
                report("installing", completed, None);
                self.import_as(Kind::Skill, files, &skill.name, Source {
                    kind: "github".into(), location,
                    git_ref: git_ref.clone(),
                    commit: Some(commit.to_string()),
                }, !allow_replace)?;
                Ok(if previous.is_some() { "updated" } else { "installed" })
            })();
            completed += 1;
            match result {
                Ok(status) => {
                    if status != "unchanged" { imported.push(skill.name.clone()); }
                    successful_paths.push(skill_path.clone());
                    report(status, completed, None);
                }
                Err(error) => {
                    let status = if error.contains("already in the library from") { "skipped" } else { "failed" };
                    report(status, completed, Some(error.clone()));
                    skipped.push(skip(error));
                }
            }
        }
        if successful_paths.is_empty() {
            return Err(format!("No selected skill was imported. {}", skipped.iter().map(|s| s.reason.as_str()).collect::<Vec<_>>().join("; ")));
        }
        let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
        let mut store = self.read_sources()?;
        // A retry must never discard successful selections from the preceding batch.
        let mut combined: BTreeSet<String> = successful_paths.into_iter().collect();
        if let Some(previous) = store.sources.iter().find(|s| s.id == id) {
            combined.extend(previous.selected_paths.iter().cloned());
        }
        let source = SavedSource {
            id: id.clone(), repo, path, git_ref,
            commit: commit.to_string(), selected_paths: combined.into_iter().collect(), saved_at: now_ms(),
        };
        if let Some(existing) = store.sources.iter_mut().find(|s| s.repo == source.repo && s.path == source.path && s.git_ref == source.git_ref) {
            *existing = source.clone();
        } else {
            if store.sources.len() >= MAX_SAVED_SOURCES {
                return Err("Saved source limit reached while importing; imported skills remain in the library.".into());
            }
            store.sources.push(source.clone());
        }
        store.sources.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.path.cmp(&b.path)));
        self.save_sources(&store)?;
        fs::remove_file(self.pending_import_path())
            .map_err(|e| format!("The import completed but its recovery marker could not be cleared: {e}"))?;
        Ok(SourceImport { source, imported, skipped, results })
    }


    /// Change only future synchronization tracking; unselected installed packages remain.
    /// Paths cannot enroll an uninstalled package without a fresh explicit import.
    pub fn set_saved_source_paths(&self, id: &str, paths: Vec<String>) -> Result<SavedSource, String> {
        let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
        let mut store = self.read_sources()?;
        let source = store.sources.iter_mut().find(|s| s.id == id)
            .ok_or("Saved GitHub source not found.")?;
        let normalized: BTreeSet<String> = paths.into_iter().collect();
        let installed: BTreeSet<String> = self.list()?.into_iter()
            .filter(|item| item.meta.kind == Kind::Skill && item.meta.source.kind == "github"
                && item.meta.source.git_ref == source.git_ref
                && item.meta.source.location.starts_with(&format!("{}/", source.repo)))
            .map(|item| item.meta.source.location)
            .collect();
        if normalized.iter().any(|path| {
            (!source.path.is_empty() && path != &source.path
                && !path.starts_with(&format!("{}/", source.path)))
                || !installed.contains(&catalog::location(&source.repo, path))
        }) {
            return Err("Track only skills already installed from this GitHub source; import additions first.".into());
        }
        source.selected_paths = normalized.into_iter().collect();
        source.saved_at = now_ms();
        let result = source.clone();
        self.save_sources(&store)?;
        Ok(result)
    }

    /// Removes only the source configuration, not imported files or harness state.
    pub fn remove_skill_source(&self, id: &str) -> Result<Vec<SavedSource>, String> {
        let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
        let mut store = self.read_sources()?;
        let original = store.sources.len();
        store.sources.retain(|s| s.id != id);
        if store.sources.len() == original {
            return Err("Saved skill source not found.".into());
        }
        self.save_sources(&store)?;
        Ok(store.sources)
    }

    /// A configured source synchronizes only the paths the user previously imported.
    pub fn selected_skill_ids(&self, id: &str) -> Result<Vec<String>, String> {
        let (locations, reference): (BTreeSet<String>, Option<String>) = {
            let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
            let store = self.read_sources()?;
            let source = store.sources.iter().find(|s| s.id == id).ok_or("Saved source not found.")?;
            (source.selected_paths.iter().map(|path| catalog::location(&source.repo, path)).collect(), source.git_ref.clone())
        };
        Ok(self.list()?.into_iter()
            .filter(|item| item.meta.kind == Kind::Skill && item.meta.source.kind == "github"
                && locations.contains(&item.meta.source.location) && item.meta.source.git_ref == reference)
            .map(|item| item.meta.id).collect())
    }
}
