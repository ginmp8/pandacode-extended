//! Explicit, read-only GitHub catalogs. Network reads use `gh api` only; imports are pinned
//! to the scanned commit and reuse the ordinary library's drift/activation protections.
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::{BTreeMap, BTreeSet}, fs, sync::Mutex, time::{Duration, Instant}};
use super::{frontmatter, github, now_ms, slug, Files, Item, Kind, Library, Source, MAX_BYTES, MAX_FILES};

pub(super) type Api<'a> = &'a dyn Fn(&str) -> Result<Value, String>;
const MAX_ENTRIES: usize = 20_000;
const MAX_SKILLS: usize = 128;
const MAX_SOURCES: usize = 64;
const SCAN_BYTES: usize = 8 * MAX_BYTES;
static CATALOG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State { Enabled, Disabled, Ignored }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub path: String, pub id: String, pub name: String, pub description: String,
    pub files: usize, pub bytes: u64,
    #[serde(default, skip_deserializing)] pub imported: Option<String>,
    #[serde(default, skip_deserializing)] pub conflicts: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub repo: String, pub git_ref: Option<String>, pub commit: String,
    pub state: State, pub scanned_at: u64, pub skills: Vec<Skill>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalogs { pub sources: Vec<Catalog> }
#[derive(Debug, Serialize)]
pub struct Skipped { pub path: String, pub name: String, pub reason: String }
#[derive(Debug, Serialize)]
pub struct Imported { pub items: Vec<Item>, pub imported: Vec<String>, pub skipped: Vec<Skipped> }
struct Blob { path: String, sha: String, size: u64 }
struct Tree { commit: String, blobs: Vec<Blob>, dirs: Vec<String> }
fn oid(value: &str) -> bool { value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) }
fn under(path: &str, dir: &str) -> bool { dir.is_empty() || path.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/')) }
fn location(repo: &str, path: &str) -> String { if path.is_empty() { repo.into() } else { format!("{repo}/{path}") } }
impl Tree {
    fn files(&self, dir: &str) -> Vec<&Blob> {
        self.blobs.iter().filter(|b| under(&b.path, dir) && !self.dirs.iter().any(|d| d != dir && under(d, dir) && under(&b.path, d))).collect()
    }
}
fn tree(api: Api, repo: &str, git_ref: Option<&str>) -> Result<Tree, String> {
    if !github::valid_repo(repo) { return Err("Enter the repository as owner/name.".into()); }
    if git_ref.is_some_and(|r| !github::valid_ref(r)) { return Err("That branch, tag or commit is not valid.".into()); }
    let commit = api(&format!("repos/{repo}/commits/{}", git_ref.unwrap_or("HEAD")))?["sha"].as_str().filter(|s| oid(s)).ok_or("GitHub returned an invalid commit.")?.to_ascii_lowercase();
    if git_ref.is_some_and(|r| oid(r) && !r.eq_ignore_ascii_case(&commit)) { return Err("GitHub did not return the requested pinned commit.".into()); }
    let value = api(&format!("repos/{repo}/git/trees/{commit}?recursive=1"))?;
    if value["truncated"].as_bool() != Some(false) { return Err("The repository tree is incomplete. Import a smaller path instead.".into()); }
    let entries = value["tree"].as_array().ok_or("GitHub returned an invalid repository tree.")?;
    if entries.len() > MAX_ENTRIES { return Err("The repository has too many entries to scan. Import a smaller path instead.".into()); }
    let mut blobs = Vec::new(); let mut paths = BTreeSet::new();
    for entry in entries {
        if entry["type"] != "blob" { continue; }
        let path = entry["path"].as_str().ok_or("GitHub returned a file without a path.")?;
        if !github::valid_path(path) || path.split('/').any(|p| p.starts_with('.')) { continue; }
        // Git symlinks and submodules are never packaged or followed.
        if !matches!(entry["mode"].as_str(), Some("100644" | "100755")) { continue; }
        if !paths.insert(path.to_string()) { return Err("GitHub returned duplicate paths.".into()); }
        let sha = entry["sha"].as_str().filter(|s| oid(s)).ok_or("GitHub returned an invalid blob id.")?;
        let size = entry["size"].as_u64().ok_or("GitHub returned a file without its size.")?;
        blobs.push(Blob { path: path.into(), sha: sha.into(), size });
    }
    let dirs: Vec<_> = blobs.iter().filter(|b| b.path == "SKILL.md" || b.path.ends_with("/SKILL.md")).map(|b| b.path.rsplit_once('/').map(|(d, _)| d).unwrap_or("").to_string()).collect();
    if dirs.len() > MAX_SKILLS { return Err(format!("This catalog exceeds {MAX_SKILLS} skills. Import a smaller path instead.")); }
    Ok(Tree { commit, blobs, dirs })
}
fn blob(api: Api, repo: &str, file: &Blob) -> Result<Vec<u8>, String> {
    if file.size > MAX_BYTES as u64 { return Err("A skill file exceeds 1 MiB.".into()); }
    let value = api(&format!("repos/{repo}/git/blobs/{}", file.sha))?;
    if !value["sha"].as_str().is_some_and(|sha| sha.eq_ignore_ascii_case(&file.sha)) { return Err("GitHub returned a different blob than the pinned tree requested.".into()); }
    if value["encoding"] != "base64" { return Err("GitHub returned an unsupported file encoding.".into()); }
    let encoded = value["content"].as_str().ok_or("GitHub returned an empty blob response.")?;
    if encoded.len() > 2 * MAX_BYTES { return Err("A skill response exceeds the size limit.".into()); }
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded.replace(['\r', '\n'], "")).map_err(|_| "GitHub returned unreadable content.")?;
    if bytes.len() != file.size as usize || bytes.len() > MAX_BYTES { return Err("The skill file size does not match its pinned tree.".into()); }
    Ok(bytes)
}
fn budget(start: Instant) -> Result<(), String> {
    if start.elapsed() > Duration::from_secs(60) { Err("Catalog operation reached its time limit. Try a smaller selection.".into()) } else { Ok(()) }
}
pub(super) fn scan(api: Api, repo: &str, git_ref: Option<&str>) -> Result<(String, Vec<Skill>), String> {
    let start = Instant::now(); let tree = tree(api, repo, git_ref)?; let mut skills = Vec::new(); let mut total = 0usize;
    for dir in &tree.dirs {
        budget(start)?;
        let main = if dir.is_empty() { "SKILL.md".into() } else { format!("{dir}/SKILL.md") };
        let entry = tree.blobs.iter().find(|b| b.path == main).ok_or("Skill entry is missing.")?;
        if entry.size > MAX_BYTES as u64 { continue; }
        total = total.saturating_add(entry.size as usize);
        if total > SCAN_BYTES { return Err("Catalog descriptions exceed the scan budget. Import a smaller path instead.".into()); }
        let bytes = blob(api, repo, entry)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "A SKILL.md is not UTF-8 text.")?;
        let (Some(name), Some(description), _) = frontmatter(text) else { continue };
        if name.len() > 200 || description.len() > 4000 { continue; }
        let id = slug(&name); if id.is_empty() { continue; }
        let files = tree.files(dir);
        skills.push(Skill { path: dir.clone(), id, name, description, files: files.len(), bytes: files.iter().fold(0u64, |sum, b| sum.saturating_add(b.size)), imported: None, conflicts: vec![] });
    }
    skills.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((tree.commit, skills))
}
impl Library {
    fn load_catalogs(&self) -> Result<Catalogs, String> {
        match fs::read(self.dir.join(".catalogs.json")) {
            Ok(bytes) if bytes.len() <= 4 * MAX_BYTES => serde_json::from_slice(&bytes).map_err(|e| format!("The catalog list is damaged: {e}")),
            Ok(_) => Err("The catalog list exceeds its size limit.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Catalogs::default()),
            Err(e) => Err(format!("Could not read catalogs: {e}")),
        }
    }
    fn save_catalogs(&self, value: &Catalogs) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        if bytes.len() > 4 * MAX_BYTES { return Err("The catalog list exceeds its size limit.".into()); }
        let temp = self.dir.join(".catalogs.json.tmp");
        fs::write(&temp, bytes).and_then(|_| fs::rename(&temp, self.dir.join(".catalogs.json"))).map_err(|e| e.to_string())
    }
    pub fn catalogs(&self) -> Result<Catalogs, String> {
        let mut catalogs = self.load_catalogs()?; let items = self.list()?;
        let mut holders: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for item in &items { holders.entry(item.meta.id.clone()).or_default().push(item.meta.source.location.clone()); }
        for source in &catalogs.sources { for skill in &source.skills { holders.entry(skill.id.clone()).or_default().push(location(&source.repo, &skill.path)); } }
        for source in &mut catalogs.sources { for skill in &mut source.skills {
            let here = location(&source.repo, &skill.path);
            skill.conflicts = holders.get(&skill.id).into_iter().flatten().filter(|l| !github::same_location(l, &here)).cloned().collect(); skill.conflicts.sort(); skill.conflicts.dedup();
            skill.imported = items.iter().find(|i| i.meta.id == skill.id && i.meta.source.kind == "github" && github::same_location(&i.meta.source.location, &here)).and_then(|i| i.meta.source.commit.clone());
        }}
        Ok(catalogs)
    }
    pub fn catalog_add(&self, api: Api, repo: &str, git_ref: Option<String>) -> Result<Catalogs, String> {
        let _guard = CATALOG_LOCK.lock().map_err(|e| e.to_string())?;
        let mut catalogs = self.load_catalogs()?;
        if catalogs.sources.len() >= MAX_SOURCES && !catalogs.sources.iter().any(|s| s.repo.eq_ignore_ascii_case(repo)) { return Err(format!("Use at most {MAX_SOURCES} catalogs.")); }
        let (commit, skills) = scan(api, repo, git_ref.as_deref())?;
        if skills.is_empty() { return Err("No SKILL.md with a name and description was found in this repository.".into()); }
        catalogs.sources.retain(|s| !s.repo.eq_ignore_ascii_case(repo));
        catalogs.sources.push(Catalog { repo: repo.into(), git_ref, commit, state: State::Enabled, scanned_at: now_ms(), skills });
        catalogs.sources.sort_by_key(|s| s.repo.to_ascii_lowercase());
        self.save_catalogs(&catalogs)?; self.catalogs()
    }
    pub fn catalog_refresh(&self, api: Api, repo: &str) -> Result<Catalogs, String> {
        let _guard = CATALOG_LOCK.lock().map_err(|e| e.to_string())?;
        let mut catalogs = self.load_catalogs()?;
        let source = catalogs.sources.iter_mut().find(|s| s.repo.eq_ignore_ascii_case(repo)).ok_or("Unknown catalog.")?;
        if source.state != State::Enabled { return Err("Enable this catalog before refreshing it.".into()); }
        let (commit, skills) = scan(api, &source.repo, source.git_ref.as_deref())?;
        source.commit = commit; source.skills = skills; source.scanned_at = now_ms();
        self.save_catalogs(&catalogs)?; self.catalogs()
    }
    pub fn catalog_state(&self, repo: &str, state: Option<State>) -> Result<Catalogs, String> {
        let _guard = CATALOG_LOCK.lock().map_err(|e| e.to_string())?;
        let mut catalogs = self.load_catalogs()?;
        if let Some(state) = state { catalogs.sources.iter_mut().find(|s| s.repo.eq_ignore_ascii_case(repo)).ok_or("Unknown catalog.")?.state = state; }
        else { catalogs.sources.retain(|s| !s.repo.eq_ignore_ascii_case(repo)); }
        self.save_catalogs(&catalogs)?; self.catalogs()
    }
    pub fn catalog_import(&self, api: Api, repo: &str, paths: &[String]) -> Result<Imported, String> {
        let _guard = CATALOG_LOCK.lock().map_err(|e| e.to_string())?;
        if paths.is_empty() || paths.len() > MAX_SKILLS { return Err("Choose between 1 and 128 skills to import.".into()); }
        let catalogs = self.load_catalogs()?;
        let source = catalogs.sources.iter().find(|s| s.repo.eq_ignore_ascii_case(repo)).ok_or("Unknown catalog.")?;
        if source.state != State::Enabled { return Err("Enable this catalog before importing.".into()); }
        if paths.iter().any(|p| !source.skills.iter().any(|s| s.path == *p)) { return Err("A selected skill is no longer in this catalog. Refresh it first.".into()); }
        let start = Instant::now(); let tree = tree(api, &source.repo, Some(&source.commit))?;
        let mut imported = Vec::new(); let mut skipped = Vec::new(); let mut fetched_bytes = 0usize;
        for skill in source.skills.iter().filter(|s| paths.contains(&s.path)) {
            let result = (|| {
                budget(start)?;
                let files = tree.files(&skill.path);
                if files.is_empty() || files.len() > MAX_FILES || files.iter().fold(0u64, |sum, b| sum.saturating_add(b.size)) > MAX_BYTES as u64 { return Err("Skill exceeds 64 files or 1 MiB.".to_string()); }
                let mut fetched: Files = Vec::new();
                for file in files {
                    budget(start)?;
                    fetched_bytes = fetched_bytes.saturating_add(file.size as usize);
                    if fetched_bytes > SCAN_BYTES { return Err("Import batch exceeded 8 MiB. Import a smaller selection.".into()); }
                    let relative = if skill.path.is_empty() { file.path.clone() } else { file.path[skill.path.len() + 1..].into() };
                    fetched.push((relative, blob(api, &source.repo, file)?));
                }
                let main = fetched.iter().find(|(p, _)| p == "SKILL.md").ok_or("SKILL.md is missing.")?;
                let (name, _, _) = frontmatter(std::str::from_utf8(&main.1).map_err(|_| "SKILL.md is not UTF-8.")?);
                if name.as_deref() != Some(&skill.name) { return Err("The imported skill does not match the indexed name.".into()); }
                let origin = Source { kind: "github".into(), location: location(&source.repo, &skill.path), git_ref: source.git_ref.clone(), commit: Some(source.commit.clone()) };
                self.import(Kind::Skill, fetched, &skill.name, origin)
            })();
            match result { Ok(item) => imported.push(item.meta.name), Err(reason) => skipped.push(Skipped { path: skill.path.clone(), name: skill.name.clone(), reason }) }
        }
        Ok(Imported { items: self.list()?, imported, skipped })
    }
}
