//! User-configured GitHub skill collections. A scan is read-only and ephemeral;
//! only the explicitly selected packages are downloaded into the library.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, sync::Mutex};

use super::{catalog, github, now_ms, sha256_hex, Kind, Library, Source, MAX_BYTES, MAX_FILES};

const MAX_SAVED_SOURCES: usize = 64;
static SOURCES_LOCK: Mutex<()> = Mutex::new(());

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
}

fn id_for(repo: &str, path: &str) -> String {
    let digest = sha256_hex(format!("{}\n{path}", repo.to_ascii_lowercase()).as_bytes());
    digest[..16].to_string()
}

impl Library {
    fn sources_path(&self) -> std::path::PathBuf { self.dir.join(".skill-sources.json") }
    fn sources_backup_path(&self) -> std::path::PathBuf { self.dir.join(".skill-sources.json.rollback") }

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
        match fs::read(current) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("Saved skill sources are unreadable: {e}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SourceStore::default()),
            Err(error) => Err(error.to_string()),
        }
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
        let (repo, path, git_ref) = github::selection(repo, path, git_ref)?;
        if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Rescan this source: the selected commit is not a full Git SHA.".into());
        }
        let chosen: BTreeSet<String> = selected_paths.into_iter().collect();
        if chosen.is_empty() || chosen.len() > catalog::MAX_SKILLS {
            return Err("Select between one and 300 skills from the scanned collection.".into());
        }
        let id = id_for(&repo, &path);
        {
            let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
            let existing = self.read_sources()?;
            if existing.sources.len() >= MAX_SAVED_SOURCES && existing.sources.iter().all(|s| s.id != id) {
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

        let mut imported = Vec::new();
        let mut skipped = Vec::new();
        let mut successful_paths = Vec::new();
        for skill_path in &chosen {
            let skill = &indexed[skill_path];
            let skip = |reason: String| catalog::Skipped {
                path: skill_path.clone(), name: skill.name.clone(), reason,
            };
            if skill.files > MAX_FILES || skill.bytes > MAX_BYTES as u64 {
                skipped.push(skip(format!("{} exceeds the validated package size or file limit.", skill.name)));
                continue;
            }
            let result = (|| -> Result<(), String> {
                let (files, actual_commit) = github::fetch_using(&repo, skill_path, Some(commit), api)
                    .map_err(|e| catalog::explain(&e))?;
                if actual_commit != commit { return Err("The fetched commit does not match the selected revision.".into()); }
                self.import_as(Kind::Skill, files, &skill.name, Source {
                    kind: "github".into(),
                    location: catalog::location(&repo, skill_path),
                    git_ref: git_ref.clone(),
                    commit: Some(commit.to_string()),
                }, true)?;
                Ok(())
            })();
            match result {
                Ok(()) => { imported.push(skill.name.clone()); successful_paths.push(skill_path.clone()); }
                Err(error) => skipped.push(skip(error)),
            }
        }
        if successful_paths.is_empty() {
            return Err(format!("No selected skill was imported. {}", skipped.iter().map(|s| s.reason.as_str()).collect::<Vec<_>>().join("; ")));
        }
        let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
        let mut store = self.read_sources()?;
        let source = SavedSource {
            id: id.clone(), repo, path, git_ref,
            commit: commit.to_string(), selected_paths: successful_paths, saved_at: now_ms(),
        };
        if let Some(existing) = store.sources.iter_mut().find(|s| s.id == id) {
            *existing = source.clone();
        } else {
            if store.sources.len() >= MAX_SAVED_SOURCES {
                return Err("Saved source limit reached while importing; imported skills remain in the library.".into());
            }
            store.sources.push(source.clone());
        }
        store.sources.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.path.cmp(&b.path)));
        self.save_sources(&store)?;
        Ok(SourceImport { source, imported, skipped })
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
        let locations: BTreeSet<String> = {
            let _guard = SOURCES_LOCK.lock().map_err(|e| e.to_string())?;
            let store = self.read_sources()?;
            let source = store.sources.iter().find(|s| s.id == id).ok_or("Saved source not found.")?;
            source.selected_paths.iter().map(|path| catalog::location(&source.repo, path)).collect()
        };
        Ok(self.list()?.into_iter()
            .filter(|item| item.meta.kind == Kind::Skill && item.meta.source.kind == "github"
                && locations.contains(&item.meta.source.location))
            .map(|item| item.meta.id).collect())
    }
}
