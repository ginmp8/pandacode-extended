//! Explicit and opt-in startup synchronization of GitHub-backed library skills.
//! No scripts run during import or synchronization. Native harness copies are not touched.
use serde::{Deserialize, Serialize};
use std::{fs, sync::Mutex};

use super::{catalog, content_hash, frontmatter, github, now_ms, slug, Item, Kind, Library, Meta, Source};

static SYNC: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPolicy {
    #[serde(default)]
    pub auto_on_start: bool,
    #[serde(default)]
    pub trusted_scopes: Vec<TrustedScope>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedScope {
    pub repo: String,
    /// Empty means the entire repository, otherwise a bounded collection path.
    pub path: String,
    /// A user-approved branch, tag, or immutable SHA for newly discovered packages.
    #[serde(default)]
    pub git_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEvent {
    pub id: String,
    pub status: String,
    pub old_commit: Option<String>,
    pub new_commit: Option<String>,
    pub impacted_harnesses: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub checked_at: u64,
    /// Newly imported packages under the user's explicitly trusted source scope.
    #[serde(default)]
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
    pub events: Vec<SyncEvent>,
}

#[derive(Debug, Clone)]
pub enum Selection {
    All,
    One(String),
    Source(String, String),
}

fn split_location(location: &str) -> Option<(String, String)> {
    let mut parts = location.splitn(3, '/');
    let (Some(owner), Some(repo)) = (parts.next(), parts.next()) else { return None };
    let repo = format!("{owner}/{repo}");
    if !github::valid_repo(&repo) { return None }
    let path = parts.next().unwrap_or("").to_string();
    if !path.is_empty() && !github::valid_path(&path) { return None }
    Some((repo, path))
}

fn is_full_sha(reference: Option<&str>) -> bool {
    reference.is_some_and(|value| value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn scope_contains(trusted: &TrustedScope, repo: &str, path: &str, git_ref: Option<&str>) -> bool {
    trusted.repo.eq_ignore_ascii_case(repo) && trusted.git_ref.as_deref() == git_ref
        && (trusted.path.is_empty() || path == trusted.path
            || path.strip_prefix(trusted.path.as_str()).is_some_and(|part| part.starts_with('/')))
}

impl Library {
    fn policy_path(&self) -> std::path::PathBuf { self.dir.join(".sync-policy.json") }
    fn report_path(&self) -> std::path::PathBuf { self.dir.join(".sync-report.json") }

    pub fn sync_policy(&self) -> Result<SyncPolicy, String> {
        match fs::read(self.policy_path()) {
            Ok(data) => serde_json::from_slice(&data).map_err(|e| format!("The skill sync policy is damaged: {e}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SyncPolicy::default()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn save_sync_file<T: Serialize>(&self, file: &str, value: &T) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let data = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        let temporary = self.dir.join(format!(".{file}.tmp"));
        fs::write(&temporary, data).and_then(|_| fs::rename(&temporary, self.dir.join(file)))
            .map_err(|e| format!("Could not save the skill sync state: {e}"))
    }

    pub fn sync_set_global(&self, enabled: bool) -> Result<SyncPolicy, String> {
        let mut policy = self.sync_policy()?;
        policy.auto_on_start = enabled;
        self.save_sync_file(".sync-policy.json", &policy)?;
        Ok(policy)
    }

    pub fn sync_trust_scope_ref(&self, repo: &str, path: &str, git_ref: Option<&str>, trusted: bool) -> Result<SyncPolicy, String> {
        let (repo, path, git_ref) = github::selection(repo, path, git_ref)?;
        let mut policy = self.sync_policy()?;
        policy.trusted_scopes.retain(|s| !(s.repo.eq_ignore_ascii_case(&repo) && s.path == path));
        if trusted {
            policy.trusted_scopes.push(TrustedScope { repo, path, git_ref });
        }
        self.save_sync_file(".sync-policy.json", &policy)?;
        Ok(policy)
    }

    pub fn sync_set_override(&self, id: &str, choice: Option<bool>) -> Result<Item, String> {
        let _guard = super::LOCK.lock().map_err(|e| e.to_string())?;
        let mut meta = self.load(id)?;
        if meta.kind != Kind::Skill || meta.source.kind != "github" {
            return Err("Only GitHub-backed skills support startup synchronization.".into());
        }
        meta.auto_update = choice;
        self.save(&meta)?;
        self.item(meta)
    }

    pub fn sync_set_ref(&self, id: &str, reference: Option<String>) -> Result<Item, String> {
        let reference = reference.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
        if reference.as_deref().is_some_and(|r| !github::valid_ref(r)) {
            return Err("Enter a valid branch, tag or full commit SHA.".into());
        }
        let _guard = super::LOCK.lock().map_err(|e| e.to_string())?;
        let mut meta = self.load(id)?;
        if meta.kind != Kind::Skill || meta.source.kind != "github" {
            return Err("Only GitHub-backed skills have a Git ref.".into());
        }
        meta.source.git_ref = reference;
        self.save(&meta)?;
        self.item(meta)
    }

    pub fn last_sync_report(&self) -> Result<SyncReport, String> {
        match fs::read(self.report_path()) {
            Ok(data) => serde_json::from_slice(&data).map_err(|e| format!("The last sync report is damaged: {e}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SyncReport::default()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn sync_one(&self, api: catalog::Api, meta: &Meta, automatic: bool) -> Result<SyncEvent, String> {
        let (repo, path) = split_location(&meta.source.location).ok_or("The original GitHub source location is invalid.")?;
        let old_commit = meta.source.commit.clone();
        let impacted_harnesses = meta.activations.iter().map(|a| a.harness.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        if automatic && is_full_sha(meta.source.git_ref.as_deref()) {
            return Ok(SyncEvent { id: meta.id.clone(), status: "skipped".into(), old_commit: old_commit.clone(), new_commit: old_commit,
                impacted_harnesses, detail: "Pinned full-SHA ref; automatic updates never repoint it.".into() });
        }
        let (files, commit) = github::fetch_using(&repo, &path, meta.source.git_ref.as_deref(), api)?;
        let current = self.files(&meta.id)?;
        let edited = content_hash(&current) != meta.sha256;
        if meta.source.commit.as_deref() == Some(commit.as_str()) && !edited {
            return Ok(SyncEvent { id: meta.id.clone(), status: "skipped".into(), old_commit: old_commit.clone(), new_commit: old_commit,
                impacted_harnesses, detail: "Already at the selected source commit.".into() });
        }
        let main = files.iter().find(|(name, _)| name == "SKILL.md").ok_or("The upstream package no longer contains SKILL.md.")?;
        let text = std::str::from_utf8(&main.1).map_err(|_| "The upstream SKILL.md is not UTF-8.")?;
        let name = frontmatter(text).0.unwrap_or_else(|| meta.name.clone());
        if slug(&name) != meta.id {
            return Err(format!("The upstream skill identity changed from {} to {}; choose a source manually.", meta.id, slug(&name)));
        }
        self.import_as(Kind::Skill, files, &meta.name,
            Source { commit: Some(commit.clone()), ..meta.source.clone() }, true)?;
        Ok(SyncEvent {
            id: meta.id.clone(), status: "updated".into(), old_commit,
            new_commit: Some(commit), impacted_harnesses,
            detail: if edited { "Replaced locally edited PandaCode library files without a backup. Native harness files were not changed.".into() }
                else { "Updated the central library. Native harness files are unchanged until reactivated.".into() },
        })
    }

    /// Index one explicitly authorized scope. Incomplete or untrusted entries cannot become
    /// active; the catalog's SHA-pinned scan enforces identity and rejects symlinks.
    fn discover_scope(&self, api: catalog::Api<'_>, scope: &TrustedScope, report: &mut SyncReport) -> Result<(), String> {
        let (commit, indexed) = catalog::scan(api, &scope.repo, scope.git_ref.as_deref())?;
        let inside = |path: &str| scope.path.is_empty() || path == scope.path
            || path.strip_prefix(scope.path.as_str()).is_some_and(|rest| rest.starts_with('/'));
        let mut seen = std::collections::HashSet::new();
        let mut ambiguous = std::collections::HashSet::new();
        for candidate in &indexed {
            if inside(&candidate.path) && !seen.insert(candidate.id.clone()) {
                ambiguous.insert(candidate.id.clone());
            }
        }
        let mut collision_reported = std::collections::HashSet::new();
        for candidate in indexed {
            if !inside(&candidate.path) { continue; }
            if ambiguous.contains(&candidate.id) {
                if collision_reported.insert(candidate.id.clone()) {
                    report.skipped.push(format!("{}: multiple source paths have the same skill ID; no automatic source was chosen.", candidate.id));
                }
                continue;
            }
            let source_location = catalog::location(&scope.repo, &candidate.path);
            if let Ok(existing) = self.load(&candidate.id) {
                if existing.source.location.eq_ignore_ascii_case(&source_location) {
                    continue;
                }
                // Do not implicitly decide between conflicting origins.
                report.skipped.push(format!("{}: another origin already uses this skill ID; explicitly choose a source before activating.", candidate.id));
                continue;
            }
            let result = (|| -> Result<(String, bool), String> {
                if candidate.files > super::MAX_FILES || candidate.bytes as usize > super::MAX_BYTES {
                    return Err("Package exceeds the validated file or size budget.".into());
                }
                let (files, observed_commit) = github::fetch_using(&scope.repo, &candidate.path, Some(&commit), api)?;
                if observed_commit != commit {
                    return Err("The selected Git revision changed during the transaction.".into());
                }
                let imported = self.import_as(super::Kind::Skill, files, &candidate.name, Source {
                    kind: "github".into(), location: source_location.clone(),
                    git_ref: scope.git_ref.clone(), commit: Some(commit.clone()),
                }, true)?;
                let safe = imported.findings.is_empty() && self.missing_required(&imported.meta).is_empty();
                if safe {
                    let _guard = super::LOCK.lock().map_err(|e| e.to_string())?;
                    let mut meta = self.load(&imported.meta.id)?;
                    meta.auto_available = true;
                    self.save(&meta)?;
                }
                Ok((imported.meta.id, safe))
            })();
            match result {
                Ok((id, safe)) => {
                    report.installed.push(id.clone());
                    report.events.push(SyncEvent {
                        id, status: if safe { "installed".into() } else { "needs-approval".into() },
                        old_commit: None, new_commit: Some(commit.clone()), impacted_harnesses: Vec::new(),
                        detail: if safe { "Installed into the central library and available on the next bot brief.".into() }
                            else { "Installed but activation is blocked pending safety acknowledgement or required peer skills.".into() },
                    });
                }
                Err(error) => {
                    report.skipped.push(format!("{}: {}", candidate.id, catalog::explain(&error)));
                    report.events.push(SyncEvent {
                        id: candidate.id, status: "error".into(), old_commit: None,
                        new_commit: None, impacted_harnesses: Vec::new(), detail: catalog::explain(&error),
                    });
                }
            }
        }
        Ok(())
    }

    /// One writer for all skill sync operations. When busy, return an in-progress report rather
    /// than race. Errors are isolated per skill; the old complete package remains available.
    pub fn synchronize(&self, api: catalog::Api, selection: Selection, automatic: bool) -> Result<SyncReport, String> {
        let Ok(_guard) = SYNC.try_lock() else {
            return Ok(SyncReport { checked_at: now_ms(), skipped: vec!["A skill synchronization is already running.".into()], ..SyncReport::default() });
        };
        let policy = self.sync_policy()?;
        let mut report = SyncReport { checked_at: now_ms(), ..SyncReport::default() };
        let items = self.list()?;
        for item in items {
            let meta = item.meta;
            if meta.source.kind != "github" || meta.kind != Kind::Skill { continue; }
            let Some((repo, path)) = split_location(&meta.source.location) else { continue; };
            let selected = match &selection {
                Selection::All => true,
                Selection::One(id) => &meta.id == id,
                Selection::Source(source_repo, scope) => source_repo.eq_ignore_ascii_case(&repo)
                    && (scope.is_empty() || path == *scope || path.strip_prefix(scope.as_str()).is_some_and(|p| p.starts_with('/'))),
            };
            if !selected { continue; }
            if automatic && (!meta.auto_update.unwrap_or(policy.auto_on_start)
                || !policy.trusted_scopes.iter().any(|trusted| scope_contains(trusted, &repo, &path, meta.source.git_ref.as_deref()))) {
                continue;
            }
            let event = match self.sync_one(api, &meta, automatic) {
                Ok(event) => event,
                Err(error) => SyncEvent { id: meta.id.clone(), status: "error".into(), old_commit: meta.source.commit.clone(),
                    new_commit: None, impacted_harnesses: meta.activations.iter().map(|a| a.harness.clone()).collect(),
                    detail: catalog::explain(&error) },
            };
            if event.status == "updated" { report.updated.push(meta.id.clone()); }
            else { report.skipped.push(format!("{}: {}", meta.id, event.detail)); }
            report.events.push(event);
        }
        // New packages are never discovered from the bundled recommendation list itself.
        // Only explicit user-trusted source/path scopes are eligible, and global startup
        // discovery must be on. An individual per-skill override cannot grant repository trust.
        if automatic && policy.auto_on_start {
            for scope in &policy.trusted_scopes {
                if let Err(error) = self.discover_scope(api, scope, &mut report) {
                    let description = catalog::explain(&error);
                    report.skipped.push(format!("{}/{}: {description}", scope.repo, scope.path));
                    report.events.push(SyncEvent {
                        id: format!("{}/{}", scope.repo, scope.path), status: "error".into(),
                        old_commit: None, new_commit: None, impacted_harnesses: Vec::new(), detail: description,
                    });
                }
            }
            // A required peer can arrive after its dependent package in the same batch.
            // Enable inheritance only after checking the complete installed set.
            for id in &report.installed {
                if let Ok(item) = self.load(id).and_then(|meta| self.item(meta)) {
                    if item.meta.auto_available || !item.findings.is_empty() || !self.missing_required(&item.meta).is_empty() {
                        continue;
                    }
                    let _guard = super::LOCK.lock().map_err(|e| e.to_string())?;
                    let mut meta = self.load(id)?;
                    meta.auto_available = true;
                    self.save(&meta)?;
                }
            }
        }
        self.save_sync_file(".sync-report.json", &report)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_trust_never_expands_to_sibling_directories() {
        let trusted = TrustedScope { repo: "ginmp8/rhapsodia".into(), path: "skills/engineering".into(), git_ref: Some("main".into()) };
        assert!(scope_contains(&trusted, "ginmp8/rhapsodia", "skills/engineering/build", Some("main")));
        assert!(!scope_contains(&trusted, "ginmp8/rhapsodia", "skills/security/check", Some("main")));
        assert!(!scope_contains(&trusted, "ginmp8/rhapsodia", "skills/engineering/build", Some("feature/untrusted")));
        assert!(!scope_contains(&trusted, "ginmp8/rhapsodia", "skills/engineering/build", None));
        assert!(!scope_contains(&trusted, "evil/rhapsodia", "skills/engineering/build", Some("main")));
        assert!(is_full_sha(Some(&"a".repeat(40))));
        assert!(!is_full_sha(Some("main")));
    }

    #[test]
    fn legacy_policy_is_opt_in_and_can_be_trusted_exactly_once() {
        let root = std::env::temp_dir().join(format!("pandacode-skill-policy-{}", uuid::Uuid::new_v4()));
        let library = Library::new(root.join("library"), root.join("home"));
        assert!(!library.sync_policy().unwrap().auto_on_start);
        assert!(library.sync_policy().unwrap().trusted_scopes.is_empty());
        library.sync_trust_scope_ref("ginmp8/rhapsodia", "skills/review", None, true).unwrap();
        library.sync_trust_scope_ref("ginmp8/rhapsodia", "skills/review", None, true).unwrap();
        assert_eq!(library.sync_policy().unwrap().trusted_scopes.len(), 1);
        library.sync_trust_scope_ref("ginmp8/rhapsodia", "skills/review", None, false).unwrap();
        assert!(library.sync_policy().unwrap().trusted_scopes.is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
