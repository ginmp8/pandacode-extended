//! Repository catalogs and discovery against recorded-shape GitHub fixtures (no network).
use super::catalog::{self, State};
use super::*;
use base64::Engine;
use serde_json::{json, Value};
use std::{cell::RefCell, collections::HashMap};

/// GitHub REST responses by endpoint, plus failures, and a log of what was called.
#[derive(Default)]
struct Fake {
    responses: HashMap<String, Value>,
    failures: HashMap<String, String>,
    calls: RefCell<Vec<String>>,
}
impl Fake {
    fn call(&self, endpoint: &str) -> Result<Value, String> {
        self.calls.borrow_mut().push(endpoint.to_string());
        if let Some(error) = self.failures.get(endpoint) {
            return Err(error.clone());
        }
        self.responses.get(endpoint).cloned().ok_or_else(|| format!("gh: Not Found (HTTP 404) {endpoint}"))
    }
    /// A repository at `commit` holding `files` (path, text).
    fn repo(&mut self, repo: &str, commit: &str, files: &[(String, String)]) {
        self.responses.insert(format!("repos/{repo}/commits/HEAD"), json!({ "sha": commit }));
        self.responses.insert(format!("repos/{repo}/commits/{commit}"), json!({ "sha": commit }));
        let mut tree = Vec::new();
        let mut directories: HashMap<String, Vec<Value>> = HashMap::new();
        for (index, (path, text)) in files.iter().enumerate() {
            // Real Git file object IDs are exactly 40 hexadecimal digits.
            let sha = format!("{:040x}", index + 1);
            tree.push(json!({ "path": path, "type": "blob", "sha": sha, "size": text.len() }));
            let content = base64::engine::general_purpose::STANDARD.encode(text);
            self.responses.insert(format!("repos/{repo}/git/blobs/{sha}"), json!({ "content": content, "encoding": "base64" }));
            let parts: Vec<&str> = path.split('/').collect();
            for depth in 0..parts.len().saturating_sub(1) {
                let parent = parts[..depth].join("/");
                let folder = parts[..=depth].join("/");
                let entries = directories.entry(parent).or_default();
                if !entries.iter().any(|entry: &Value| entry["path"].as_str() == Some(folder.as_str())) {
                    entries.push(json!({ "path": folder, "name": parts[depth], "type": "dir" }));
                }
            }
            let parent = parts[..parts.len() - 1].join("/");
            directories.entry(parent).or_default().push(json!({
                "path": path, "name": parts[parts.len() - 1], "type": "file",
                "sha": sha, "size": text.len(),
            }));
        }
        for (directory, entries) in directories {
            self.responses.insert(format!("repos/{repo}/contents/{directory}?ref={commit}"), Value::Array(entries));
        }
        self.responses.insert(format!("repos/{repo}/git/trees/{commit}?recursive=1"), json!({ "sha": commit, "tree": tree, "truncated": false }));
    }
    fn page(&mut self, page: usize, repos: Value) {
        self.responses.insert(format!("user/repos?affiliation=owner&per_page=100&page={page}&sort=full_name"), repos);
    }
    fn api(&self) -> impl Fn(&str) -> Result<Value, String> + '_ {
        |endpoint| self.call(endpoint)
    }
    fn count(&self, prefix: &str) -> usize {
        self.calls.borrow().iter().filter(|c| c.starts_with(prefix)).count()
    }
}

struct Fixture {
    base: PathBuf,
    library: Library,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("pandacode-catalog-{}", uuid::Uuid::new_v4()));
        let home = base.join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(home.join(".codex")).unwrap();
        Self { library: Library::new(base.join("library"), home), base }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn skill(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Does {name}.\n---\n# {name}\n")
}
fn files(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(p, t)| (p.to_string(), t.to_string())).collect()
}
/// A pstack-like collection: 30 skills of 4 files each (120 files, far past 64 in total).
fn pstack() -> Vec<(String, String)> {
    (0..30).flat_map(|i| {
        let dir = format!("skills/skill-{i:02}");
        [(format!("{dir}/SKILL.md"), skill(&format!("skill-{i:02}"))), (format!("{dir}/scripts/run.sh"), "echo hi\n".into()), (format!("{dir}/reference.md"), "ref\n".into()), (format!("{dir}/templates/a.md"), "a\n".into())]
    }).collect()
}

#[test]
fn detects_root_collections_and_nested_skills_by_frontmatter() {
    let mut fake = Fake::default();
    fake.repo("me/kit", "c1", &files(&[
        ("SKILL.md", &skill("Kit")),
        ("helper.md", "root helper"),
        ("skills/pdf/SKILL.md", &skill("PDF")),
        ("skills/pdf/forms.md", "forms"),
        ("collections/writing/skills/blog/SKILL.md", "---\nname: Blog\ndescription: >-\n  Writes blog\n  posts.\n---\nbody"),
        ("skills/broken/SKILL.md", "---\nname: Broken\n---\nno description"),
        ("skills/plain/SKILL.md", "# no frontmatter"),
        (".github/skills/hidden/SKILL.md", &skill("Hidden")),
        (".github/skills/hidden/.config/settings.md", "hidden supporting config"),
        ("README.md", "skills!"),
    ]));
    let (commit, skills) = catalog::scan(&fake.api(), "me/kit", None).unwrap();
    assert_eq!(commit, "c1");
    let found: Vec<(&str, &str, usize)> = skills.iter().map(|s| (s.path.as_str(), s.name.as_str(), s.files)).collect();
    // The root skill keeps its own files, never a nested skill's.
    assert_eq!(found, vec![("", "Kit", 3), (".github/skills/hidden", "Hidden", 2),
        ("collections/writing/skills/blog", "Blog", 1), ("skills/pdf", "PDF", 2)]);
    assert_eq!(skills[2].description, "Writes blog posts.");
}

#[test]
fn collections_past_64_files_import_per_skill_with_provenance_and_no_activation() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    fake.repo("acme/pstack", "abc123", &pstack());
    let catalogs = fx.library.add_catalog(&fake.api(), "acme/pstack", None).unwrap();
    assert_eq!(catalogs.sources[0].skills.len(), 30);
    let chosen = vec!["skills/skill-03".to_string()];
    let one = fx.library.import_catalog(&fake.api(), "acme/pstack", Some(&chosen)).unwrap();
    assert_eq!(one.imported, vec!["skill-03"]);
    assert_eq!(one.items.len(), 1);
    let all = fx.library.import_catalog(&fake.api(), "acme/pstack", None).unwrap();
    assert_eq!((all.imported.len(), all.skipped.len(), all.items.len()), (30, 0, 30));
    let item = all.items.iter().find(|i| i.meta.id == "skill-07").unwrap();
    assert_eq!(item.files, vec!["SKILL.md", "reference.md", "scripts/run.sh", "templates/a.md"]);
    assert_eq!(item.meta.source.location, "acme/pstack/skills/skill-07");
    assert_eq!(item.meta.source.commit.as_deref(), Some("abc123"));
    // Importing is not activation: nothing written for any harness.
    assert!(all.items.iter().all(|i| i.meta.activations.is_empty()));
    assert!(fs::read_dir(fx.base.join("home/.claude")).unwrap().next().is_none());
    let annotated = fx.library.catalogs().unwrap();
    assert!(annotated.sources[0].skills.iter().all(|s| s.imported.as_deref() == Some("abc123") && s.conflicts.is_empty()));
}

#[test]
fn per_skill_and_per_catalog_limits_have_clear_errors() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let mut big: Vec<(String, String)> = vec![("SKILL.md".into(), skill("Big"))];
    big.extend((0..MAX_FILES).map(|i| (format!("extra/{i}.md"), "x".to_string())));
    fake.repo("me/big", "b1", &big);
    fx.library.add_catalog(&fake.api(), "me/big", None).unwrap();
    let result = fx.library.import_catalog(&fake.api(), "me/big", None).unwrap();
    assert!(result.skipped[0].reason.contains(&format!("more than {MAX_FILES} files")), "{:?}", result.skipped);

    let many: Vec<(String, String)> = (0..=catalog::MAX_SKILLS).map(|i| (format!("s/{i}/SKILL.md"), skill(&format!("s{i}")))).collect();
    fake.repo("me/many", "m1", &many);
    assert!(fx.library.add_catalog(&fake.api(), "me/many", None).unwrap_err().contains("more than 300 SKILL.md"));
    fake.responses.insert("repos/me/huge/commits/HEAD".into(), json!({ "sha": "h1" }));
    fake.responses.insert("repos/me/huge/git/trees/h1?recursive=1".into(), json!({ "tree": [], "truncated": true }));
    assert!(fx.library.add_catalog(&fake.api(), "me/huge", None).unwrap_err().contains("too large"));
    fake.repo("me/notes", "n1", &files(&[("README.md", "skills")]));
    assert!(fx.library.add_catalog(&fake.api(), "me/notes", None).unwrap_err().contains("no SKILL.md"));
}

#[test]
fn same_name_skills_from_different_sources_are_shown_and_never_overwritten() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    fake.repo("me/one", "o1", &files(&[("skills/pdf/SKILL.md", &skill("PDF")), ("skills/pdf/a.md", "mine")]));
    fake.repo("you/two", "t1", &files(&[("pdf/SKILL.md", &skill("PDF")), ("pdf/b.md", "theirs")]));
    fx.library.add_catalog(&fake.api(), "me/one", None).unwrap();
    let catalogs = fx.library.add_catalog(&fake.api(), "you/two", None).unwrap();
    let conflicts = |repo: &str| catalogs.sources.iter().find(|s| s.repo == repo).unwrap().skills[0].conflicts.clone();
    assert_eq!(conflicts("me/one"), vec!["you/two/pdf"]);
    assert_eq!(conflicts("you/two"), vec!["me/one/skills/pdf"]);

    fx.library.import_catalog(&fake.api(), "me/one", None).unwrap();
    let second = fx.library.import_catalog(&fake.api(), "you/two", None).unwrap();
    assert!(second.imported.is_empty());
    assert!(second.skipped[0].reason.contains("already in the library from me/one/skills/pdf"));
    let pdf = second.items.iter().find(|i| i.meta.id == "pdf").unwrap();
    assert_eq!(pdf.meta.source.location, "me/one/skills/pdf");
    assert!(pdf.files.contains(&"a.md".to_string()));
}

#[test]
fn an_import_that_lands_while_another_fetches_is_never_overwritten() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    fake.repo("you/two", "t1", &files(&[("pdf/SKILL.md", &skill("PDF")), ("pdf/b.md", "theirs")]));
    fx.library.add_catalog(&fake.api(), "you/two", None).unwrap();
    // The id is free when the import checks; another source takes it during the fetch.
    let raced = std::cell::Cell::new(false);
    let api = |endpoint: &str| {
        if endpoint.contains("/git/blobs/") && !raced.replace(true) {
            let mine = vec![("SKILL.md".to_string(), skill("PDF").into_bytes()), ("a.md".to_string(), b"mine".to_vec())];
            fx.library.import(Kind::Skill, mine, "pdf", Source { kind: "folder".into(), location: "/me/pdf".into(), git_ref: None, commit: None }).unwrap();
        }
        fake.call(endpoint)
    };
    let result = fx.library.import_catalog(&api, "you/two", None).unwrap();
    assert!(raced.get());
    assert!(result.imported.is_empty());
    assert!(result.skipped[0].reason.contains("already in the library from /me/pdf"), "{:?}", result.skipped);
    let pdf = result.items.iter().find(|i| i.meta.id == "pdf").unwrap();
    assert_eq!(pdf.meta.source.location, "/me/pdf");
    assert!(pdf.files.contains(&"a.md".to_string()));
}

#[test]
fn refresh_records_upstream_changes_without_touching_the_library() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    fake.repo("me/kit", "v1", &files(&[("SKILL.md", &skill("Kit"))]));
    fx.library.add_catalog(&fake.api(), "me/kit", None).unwrap();
    fx.library.import_catalog(&fake.api(), "me/kit", None).unwrap();
    fake.repo("me/kit", "v2", &files(&[("SKILL.md", &skill("Kit")), ("new.md", "new")]));
    let catalogs = fx.library.refresh_catalog(&fake.api(), "me/kit").unwrap();
    assert_eq!(catalogs.sources[0].commit.as_deref(), Some("v2"));
    assert_eq!(catalogs.sources[0].skills[0].imported.as_deref(), Some("v1"));
    assert_eq!(fx.library.list().unwrap()[0].files, vec!["SKILL.md"]);
}

fn repos(names: &[(&str, &str)]) -> Value {
    Value::Array(names.iter().map(|(name, pushed)| json!({ "full_name": name, "pushed_at": pushed, "private": name.contains("private"), "size": 10 })).collect())
}

#[test]
fn discovery_pages_registers_only_skill_repos_and_is_idempotent() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let mut first: Vec<(String, String)> = (0..99).map(|i| (format!("me/app-{i:02}"), "t1".to_string())).collect();
    first.push(("me/private-skills".into(), "t1".into()));
    let first: Vec<(&str, &str)> = first.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    fake.page(1, repos(&first));
    fake.page(2, repos(&[("me/skills-named-but-empty", "t1"), ("me/tools", "t1")]));
    for i in 0..99 {
        fake.repo(&format!("me/app-{i:02}"), &format!("a{i}"), &files(&[("README.md", "app")]));
    }
    fake.repo("me/private-skills", "p1", &files(&[("skills/a/SKILL.md", &skill("A")), ("skills/b/SKILL.md", &skill("B"))]));
    fake.repo("me/skills-named-but-empty", "e1", &files(&[("skills/README.md", "soon")]));
    fake.repo("me/tools", "w1", &files(&[("SKILL.md", &skill("Tools"))]));

    let catalogs = fx.library.discover(&fake.api()).unwrap();
    let repos_found: Vec<&str> = catalogs.sources.iter().map(|s| s.repo.as_str()).collect();
    assert_eq!(repos_found, vec!["me/private-skills", "me/tools"]);
    assert!(catalogs.sources.iter().all(|s| s.origin == "discovered" && s.state == State::Enabled));
    assert!(catalogs.sources[0].private);
    let report = catalogs.discovery.unwrap();
    assert_eq!((report.checked, report.registered.len()), (102, 2));
    assert_eq!(fake.count("user/repos"), 2);
    // Discovery never imports or activates.
    assert!(fx.library.list().unwrap().is_empty());

    let trees = fake.count("repos/");
    let again = fx.library.discover(&fake.api()).unwrap();
    assert_eq!(again.sources.len(), 2);
    assert!(again.discovery.unwrap().registered.is_empty());
    assert_eq!(fake.count("repos/"), trees, "unchanged repositories are not read again");
}

#[test]
fn ignored_and_disabled_sources_stay_that_way() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    fake.page(1, repos(&[("me/kit", "t1"), ("me/more", "t1")]));
    fake.repo("me/kit", "k1", &files(&[("SKILL.md", &skill("Kit"))]));
    fake.repo("me/more", "m1", &files(&[("SKILL.md", &skill("More"))]));
    fx.library.discover(&fake.api()).unwrap();
    fx.library.set_catalog_state("me/kit", State::Ignored).unwrap();
    fx.library.set_catalog_state("me/more", State::Disabled).unwrap();
    fake.page(1, repos(&[("me/kit", "t2"), ("me/more", "t2")]));
    let catalogs = fx.library.discover(&fake.api()).unwrap();
    let state = |repo: &str| catalogs.sources.iter().find(|s| s.repo == repo).unwrap().state;
    assert_eq!((state("me/kit"), state("me/more")), (State::Ignored, State::Disabled));
    assert!(fx.library.import_catalog(&fake.api(), "me/kit", None).unwrap_err().contains("ignored"));
    assert!(fx.library.import_catalog(&fake.api(), "me/more", None).unwrap_err().contains("disabled"));
    assert!(fx.library.refresh_catalog(&fake.api(), "me/kit").unwrap_err().contains("ignored"));
}

#[test]
fn access_failures_are_explained_and_progress_is_kept() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    fake.failures.insert("user/repos?affiliation=owner&per_page=100&page=1&sort=full_name".into(), "gh: To get started with GitHub CLI, please run:  gh auth login".into());
    assert!(fx.library.discover(&fake.api()).unwrap_err().contains("not signed in"));

    fake.failures.clear();
    fake.page(1, repos(&[("me/a-kit", "t1"), ("me/b-locked", "t1"), ("me/c-later", "t1")]));
    fake.repo("me/a-kit", "k1", &files(&[("SKILL.md", &skill("Kit"))]));
    fake.failures.insert("repos/me/b-locked/commits/HEAD".into(), "gh: Repository access blocked (HTTP 451)".into());
    fake.failures.insert("repos/me/c-later/commits/HEAD".into(), "gh: API rate limit exceeded for user ID 1. (HTTP 403)".into());
    let error = fx.library.discover(&fake.api()).unwrap_err();
    assert!(error.contains("rate limit"), "{error}");
    let saved = fx.library.catalogs().unwrap();
    assert_eq!(saved.sources.len(), 1, "sources found before the rate limit are kept");
    assert!(saved.discovery.unwrap().failures[0].starts_with("me/b-locked: "));
    assert!(fx.library.add_catalog(&fake.api(), "me/missing", None).unwrap_err().contains("not found, or your GitHub login cannot read it"));
}

#[test]
fn automatic_sync_never_discovers_unselected_repository_skills() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "a".repeat(40);
    fake.repo("me/kit", &commit, &files(&[
        ("skills/engineering/alpha/SKILL.md", &skill("Alpha")),
        ("skills/security/secret/SKILL.md", &skill("Secret")),
    ]));
    fake.responses.insert("repos/me/kit/commits/main".into(), json!({"sha": commit}));
    fx.library.sync_trust_scope_ref("me/kit", "skills", Some("main"), true).unwrap();
    fx.library.sync_set_global(true).unwrap();
    // Legacy trust metadata may exist; it cannot authorize downloading any new skill.
    let report = fx.library.synchronize(&fake.api(), super::sync::Selection::All, true).unwrap();
    assert!(report.installed.is_empty());
    assert!(fx.library.list().unwrap().is_empty());
    assert_eq!(fake.count("repos/me/kit/"), 0, "startup must not enumerate or clone a repository");
}

#[test]
fn required_peer_metadata_blocks_manual_activation_and_legacy_metadata_stays_opted_out() {
    let fx = Fixture::new();
    let source = Source { kind: "folder".into(), location: "/home/needs".into(), git_ref: None, commit: None };
    let files = vec![
        ("SKILL.md".into(), skill("Needs").into_bytes()),
        ("skill-dependencies.json".into(), br#"{"requiredSkills":["provider"]}"#.to_vec()),
    ];
    let item = fx.library.import(Kind::Skill, files, "needs", source).unwrap();
    assert!(item.meta.required_skills.contains(&"provider".into()));
    assert!(!item.meta.auto_available && item.meta.auto_update.is_none());
    assert!(fx.library.activate("needs", "claude", Scope::User, None, false).unwrap_err().contains("Install required peer"));
    let legacy = serde_json::json!({
        "id":"old", "name":"Old", "kind":"skill", "description":"Old", "source":{"kind":"folder","location":"/legacy","gitRef":null,"commit":null},
        "sha256":"abc", "importedAt":0, "acknowledged":null, "activations":[]
    });
    let old: Meta = serde_json::from_value(legacy).unwrap();
    assert!(!old.auto_available && old.auto_update.is_none() && old.required_skills.is_empty());
}

#[test]
fn configured_source_preview_is_ephemeral_and_imports_only_explicitly_selected_paths() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "a".repeat(40);
    fake.repo("me/kit", &commit, &files(&[
        ("skills/engineering/alpha/SKILL.md", &skill("Alpha")),
        ("skills/engineering/alpha/reference.md", "Useful reference"),
        ("skills/engineering/beta/SKILL.md", &skill("Beta")),
        ("skills/security/secret/SKILL.md", &skill("Secret")),
    ]));
    fake.responses.insert("repos/me/kit/commits/main".into(), json!({"sha": commit}));

    let preview = fx.library.preview_skill_source(&fake.api(), "me/kit", "skills/engineering", Some("main")).unwrap();
    assert_eq!(preview.skills.len(), 2);
    assert_eq!(preview.path, "skills/engineering");
    assert!(fx.library.saved_skill_sources().unwrap().is_empty(), "scanning must not save unused repositories");
    assert!(fx.library.list().unwrap().is_empty(), "scanning must not install skills");

    let result = fx.library.import_skill_source(&fake.api(), "me/kit", "skills/engineering", Some("main"), &commit,
        vec!["skills/engineering/alpha".into()]).unwrap();
    assert_eq!(result.imported, ["Alpha"]);
    assert_eq!(result.source.selected_paths, ["skills/engineering/alpha"]);
    assert!(fx.library.load("alpha").is_ok());
    assert!(fx.library.load("beta").is_err());
    assert!(fx.library.load("secret").is_err());
    assert!(fx.library.files("alpha").unwrap().iter().any(|(path, _)| path == "reference.md"));

    let saved = fx.library.saved_skill_sources().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].repo, "me/kit");
    assert_eq!(saved[0].git_ref.as_deref(), Some("main"));
    assert_eq!(fx.library.selected_skill_ids(&saved[0].id).unwrap(), ["alpha"]);
    assert_eq!(fake.count("repos/me/kit/commits/main"), 1);
    assert!(fake.calls.borrow().iter().all(|path| !path.contains("clone")), "GitHub CLI API only, no local clone");
}

#[test]
fn multiple_paths_in_same_repo_are_independent_and_can_be_removed_without_deleting_skills() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "b".repeat(40);
    fake.repo("me/kit", &commit, &files(&[
        ("skills/core/alpha/SKILL.md", &skill("Alpha")),
        ("skills/extra/beta/SKILL.md", &skill("Beta")),
    ]));
    for (path, skill_path) in [
        ("skills/core", "skills/core/alpha"),
        ("skills/extra", "skills/extra/beta"),
    ] {
        fx.library.import_skill_source(&fake.api(), "me/kit", path, None, &commit,
            vec![skill_path.to_string()]).unwrap();
    }
    let saved = fx.library.saved_skill_sources().unwrap();
    assert_eq!(saved.len(), 2);
    assert_ne!(saved[0].id, saved[1].id);
    fx.library.remove_skill_source(&saved[0].id).unwrap();
    assert_eq!(fx.library.saved_skill_sources().unwrap().len(), 1);
    assert!(fx.library.load("alpha").is_ok(), "remove source must not delete library content");
    assert!(fx.library.load("beta").is_ok());
}

#[test]
fn configured_source_rejects_out_of_scope_and_duplicate_id_selections() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "c".repeat(40);
    fake.repo("me/kit", &commit, &files(&[
        ("skills/one/a/SKILL.md", &skill("Duplicate")),
        ("skills/one/b/SKILL.md", &skill("Duplicate")),
        ("skills/two/outside/SKILL.md", &skill("Outside")),
    ]));
    let rejected = fx.library.import_skill_source(&fake.api(), "me/kit", "skills/one", None, &commit,
        vec!["skills/two/outside".into()]).unwrap_err();
    assert!(rejected.contains("outside this collection"), "{rejected}");
    let duplicates = fx.library.import_skill_source(&fake.api(), "me/kit", "skills/one", None, &commit,
        vec!["skills/one/a".into(), "skills/one/b".into()]).unwrap_err();
    assert!(duplicates.contains("same skill ID"), "{duplicates}");
    assert!(fx.library.list().unwrap().is_empty());
    assert!(fx.library.saved_skill_sources().unwrap().is_empty());
}

#[test]
fn auto_sync_master_switch_never_downloads_when_off_even_for_per_skill_enabled() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "d".repeat(40);
    fake.repo("me/kit", &commit, &files(&[("skills/alpha/SKILL.md", &skill("Alpha"))]));
    fx.library.import_skill_source(&fake.api(), "me/kit", "skills", None, &commit,
        vec!["skills/alpha".into()]).unwrap();
    fx.library.sync_set_override("alpha", Some(true)).unwrap();
    let before = fake.count("repos/me/kit/");
    assert!(!fx.library.sync_policy().unwrap().auto_on_start);
    fx.library.synchronize(&fake.api(), super::sync::Selection::All, true).unwrap();
    assert_eq!(fake.count("repos/me/kit/"), before, "global disabled must prevent network calls");
    fx.library.sync_set_global(true).unwrap();
    fx.library.synchronize(&fake.api(), super::sync::Selection::All, true).unwrap();
    assert!(fake.count("repos/me/kit/") > before);
}


#[test]
fn targeted_source_scan_resolves_immutable_directory_before_recursing() {
    let mut fake = Fake::default();
    fake.repo("me/scoped", "c1", &files(&[
        ("README.md", "readme"),
        ("projects/demo/SKILL.md", &skill("Scoped")),
        ("projects/demo/guide.md", "guide"),
        ("other/SKILL.md", &skill("Other")),
    ]));
    fake.responses.insert("repos/me/scoped/commits/HEAD".into(), json!({
        "sha": "c1", "commit": {"tree": {"sha": "root-tree"}}
    }));
    fake.responses.insert("repos/me/scoped/git/trees/root-tree".into(), json!({
        "tree": [{"path":"projects","type":"tree","sha":"projects-tree"}]
    }));
    fake.responses.insert("repos/me/scoped/git/trees/projects-tree".into(), json!({
        "tree": [{"path":"demo","type":"tree","sha":"demo-tree"}]
    }));
    fake.responses.insert("repos/me/scoped/git/trees/demo-tree?recursive=1".into(), json!({
        "truncated":false,"tree":[
            {"path":"SKILL.md","type":"blob","sha":format!("{:040x}",2),"size":skill("Scoped").len()},
            {"path":"guide.md","type":"blob","sha":format!("{:040x}",3),"size":5}
        ]
    }));
    let (revision, found) = catalog::scan_path(&fake.api(), "me/scoped", "projects/demo", None).unwrap();
    assert_eq!(revision,"c1");
    assert_eq!(found.len(),1);
    assert_eq!(found[0].path,"projects/demo");
    assert_eq!(found[0].files,2);
    assert!(!fake.calls.borrow().iter().any(|call| call == "repos/me/scoped/git/trees/c1?recursive=1"));
}

#[test]
fn truncated_recursive_trees_fall_back_to_bounded_nonrecursive_walk() {
    let mut fake=Fake::default();
    fake.repo("me/walk", "c1", &files(&[("skills/demo/SKILL.md",&skill("Walked"))]));
    fake.responses.insert("repos/me/walk/git/trees/c1?recursive=1".into(),json!({"truncated":true,"tree":[]}));
    fake.responses.insert("repos/me/walk/git/trees/c1".into(),json!({
        "truncated":false,"tree":[{"path":"skills","type":"tree","sha":"dir1"}]
    }));
    fake.responses.insert("repos/me/walk/git/trees/dir1".into(),json!({
        "truncated":false,"tree":[{"path":"demo","type":"tree","sha":"dir2"}]
    }));
    fake.responses.insert("repos/me/walk/git/trees/dir2".into(),json!({
        "truncated":false,"tree":[{"path":"SKILL.md","type":"blob","sha":format!("{:040x}",1),"size":skill("Walked").len()}]
    }));
    let (_,found)=catalog::scan(&fake.api(),"me/walk",None).unwrap();
    assert_eq!(found.len(),1);
    assert_eq!(found[0].path,"skills/demo");
    assert_eq!(fake.count("repos/me/walk/git/trees/"),4);
}

#[test]
fn a_batch_sync_freezes_moving_default_branch_once_for_all_packages() {
    let fx = Fixture::new();
    let old = "a".repeat(40);
    let next = "b".repeat(40);
    let mut fake = Fake::default();
    fake.repo("me/frozen", &old, &files(&[
        ("skills/alpha/SKILL.md", &skill("Alpha")),
        ("skills/beta/SKILL.md", &skill("Beta")),
    ]));
    fx.library.import_skill_source(&fake.api(), "me/frozen", "skills", None, &old,
        vec!["skills/alpha".into(), "skills/beta".into()]).unwrap();
    let mut changed = Fake::default();
    changed.repo("me/frozen", &next, &files(&[
        ("skills/alpha/SKILL.md", &skill("Alpha")),
        ("skills/alpha/added.md", "alpha changed"),
        ("skills/beta/SKILL.md", &skill("Beta")),
        ("skills/beta/added.md", "beta changed"),
    ]));
    let calls = std::cell::Cell::new(0);
    let api = |endpoint: &str| {
        if endpoint == "repos/me/frozen/commits/HEAD" {
            calls.set(calls.get() + 1);
            if calls.get() > 1 {
                return Ok(json!({"sha": "c".repeat(40)}));
            }
        }
        changed.call(endpoint)
    };
    let report = fx.library.synchronize(&api, super::sync::Selection::All, false).unwrap();
    assert_eq!(calls.get(), 1, "the moving source ref must resolve once per batch");
    assert_eq!(report.updated.len(), 2);
    for id in ["alpha", "beta"] {
        assert_eq!(fx.library.load(id).unwrap().source.commit.as_deref(), Some(next.as_str()));
    }
}

#[test]
fn interrupted_import_recovers_only_finished_selected_packages() {
    let fx = Fixture::new();
    let commit = "a".repeat(40);
    let started_at = now_ms().saturating_sub(10);
    fx.library.import(Kind::Skill, vec![("SKILL.md".into(), skill("Alpha").into_bytes())],
        "Alpha", Source {
            kind: "github".into(), location: "me/kit/skills/alpha".into(),
            git_ref: Some("main".into()), commit: Some(commit.clone()),
        }).unwrap();
    let pending = serde_json::json!({
        "id": sha256_hex(b"me/kit\\nskills\\nmain")[..16].to_string(),
        "repo": "me/kit", "path": "skills", "gitRef": "main",
        "commit": commit, "selectedPaths": ["skills/alpha", "skills/missing"],
        "startedAt": started_at
    });
    // PendingSourceImport uses snake_case JSON fields internally.
    let pending = json!({
        "id": pending["id"], "repo": "me/kit", "path": "skills", "git_ref": "main",
        "commit": commit, "selected_paths": ["skills/alpha", "skills/missing"],
        "started_at": started_at
    });
    fs::create_dir_all(&fx.library.dir).unwrap();
    fs::write(fx.library.dir.join(".skill-sources.pending.json"), serde_json::to_vec(&pending).unwrap()).unwrap();
    let restored = fx.library.saved_skill_sources().unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].selected_paths, ["skills/alpha"]);
    assert!(!fx.library.dir.join(".skill-sources.pending.json").exists());
}

#[test]
fn tracked_batch_reports_real_terminal_progress_and_saves_only_successes() {
    let fx = Fixture::new();
    let commit = "a".repeat(40);
    let mut fake = Fake::default();
    fake.repo("me/mixed", &commit, &files(&[
        ("skills/alpha/SKILL.md", &skill("Alpha")),
        ("skills/beta/SKILL.md", &skill("Beta")),
        ("skills/gamma/SKILL.md", &skill("Gamma")),
    ]));
    fake.failures.insert(format!("repos/me/mixed/contents/skills/beta?ref={commit}"),
        "gh: Temporary API failure (HTTP 503)".into());
    fx.library.import(Kind::Skill, vec![("SKILL.md".into(), skill("Gamma").into_bytes())],
        "Gamma", Source {kind:"folder".into(),location:"/local/gamma".into(),git_ref:None,commit:None}).unwrap();
    let mut progress = Vec::new();
    let mut event_payloads = Vec::new();
    let result = fx.library.import_skill_source_progress(&fake.api(),
        super::sources::SourceImportRequest {
            repo:"me/mixed",path:"skills",git_ref:None,commit:&commit,
            selected_paths:vec!["skills/alpha".into(),"skills/beta".into(),"skills/gamma".into()],
            allow_replace:false,
        }, |event| {
            event_payloads.push(super::operation_progress_payload(
                "test-123", &event.path, &event.name, &event.phase,
                event.completed, event.total, event.detail.as_deref(),
            ));
            progress.push(event);
        }).unwrap();
    assert_eq!(result.source.selected_paths, ["skills/alpha"]);
    assert_eq!(result.imported, ["Alpha"]);
    assert_eq!(result.skipped.len(), 2);
    let statuses: Vec<&str> = result.results.iter().map(|entry| entry.phase.as_str()).collect();
    assert_eq!(statuses, ["installed", "failed", "skipped"]);
    assert_eq!(result.results.iter().map(|entry| entry.completed).collect::<Vec<_>>(), [1,2,3]);
    assert_eq!(progress.iter().filter(|entry| matches!(entry.phase.as_str(),
        "installed"|"failed"|"skipped")).count(), 3);
    assert!(event_payloads.iter().any(|event| event["phase"] == "fetching"
        && event["completed"] == 0 && event["total"] == 3));
    assert!(event_payloads.iter().any(|event| event["phase"] == "validating"
        && event["path"] == "skills/alpha"));
    assert_eq!(event_payloads.iter().filter(|event| {
        matches!(event["phase"].as_str(), Some("installed" | "failed" | "skipped"))
    }).count(), 3);
    assert!(event_payloads.iter().all(|event| event["operationId"] == "test-123"
        && event["total"] == 3 && event["path"].as_str().is_some()
        && event["name"].as_str().is_some()));
}

#[test]
fn removing_a_saved_source_during_its_import_is_rejected() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "a".repeat(40);
    fake.repo("me/locked", &commit, &files(&[
        ("skills/alpha/SKILL.md", &skill("Alpha")),
    ]));
    let original = fx.library.import_skill_source(&fake.api(), "me/locked", "skills", None, &commit,
        vec!["skills/alpha".into()]).unwrap().source;
    fake.calls.borrow_mut().clear();
    let observed_conflict = std::cell::Cell::new(false);
    let commit_endpoint = format!("repos/me/locked/commits/{commit}");
    let guarded_api = |endpoint: &str| {
        // The first commit lookup is validation; the second occurs inside the
        // installed-package fetch, while the import intent is active.
        if endpoint == commit_endpoint && fake.count(&commit_endpoint) == 1 {
            observed_conflict.set(true);
            let error = fx.library.remove_skill_source(&original.id).unwrap_err();
            assert!(error.contains("import in progress"), "{error}");
        }
        fake.call(endpoint)
    };
    fx.library.import_skill_source(&guarded_api, "me/locked", "skills", None, &commit,
        vec!["skills/alpha".into()]).unwrap();
    assert!(observed_conflict.get(), "the removal must be challenged inside the active import");
    assert_eq!(fx.library.saved_skill_sources().unwrap().len(), 1);
    assert!(fx.library.load("alpha").is_ok());
}

#[test]
fn hidden_github_skill_directories_and_supporting_files_are_importable() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "a".repeat(40);
    fake.repo("me/hidden", &commit, &files(&[
        (".agents/skills/archify/SKILL.md", &skill("Archify")),
        (".agents/skills/archify/.config/setting.md", "Keep hidden package data"),
        (".github/skills/other/SKILL.md", &skill("Other")),
    ]));
    let scanned = fx.library.preview_skill_source(&fake.api(), "me/hidden", "", None).unwrap();
    assert_eq!(scanned.skills.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
        [".agents/skills/archify", ".github/skills/other"]);
    let imported = fx.library.import_skill_source(&fake.api(), "me/hidden", "", None, &commit,
        vec![".agents/skills/archify".into()]).unwrap();
    assert_eq!(imported.imported, ["Archify"]);
    assert!(fx.library.files("archify").unwrap().iter()
        .any(|(path, _)| path == ".config/setting.md"));
    assert!(fx.library.load("other").is_err(), "unselected skills must not be imported");
}

#[test]
fn an_installed_root_skill_can_remain_selected_in_its_saved_source() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "b".repeat(40);
    fake.repo("me/root", &commit, &files(&[
        ("SKILL.md", &skill("Root")),
        ("README.md", "Readme"),
    ]));
    let source = fx.library.import_skill_source(&fake.api(), "me/root", "", None, &commit,
        vec!["".into()]).unwrap().source;
    assert_eq!(fx.library.selected_skill_ids(&source.id).unwrap(), ["root"]);
    let updated = fx.library.set_saved_source_paths(&source.id, vec!["".into()]).unwrap();
    assert_eq!(updated.selected_paths, [""]);
    assert_eq!(fx.library.selected_skill_ids(&source.id).unwrap(), ["root"]);
}

#[test]
fn removing_a_source_during_initial_remote_validation_is_rejected() {
    let fx = Fixture::new();
    let mut fake = Fake::default();
    let commit = "c".repeat(40);
    fake.repo("me/slow", &commit, &files(&[
        ("skills/alpha/SKILL.md", &skill("Alpha")),
    ]));
    let existing = fx.library.import_skill_source(&fake.api(), "me/slow", "skills", None, &commit,
        vec!["skills/alpha".into()]).unwrap().source;
    let checked = std::cell::Cell::new(false);
    let endpoint = format!("repos/me/slow/commits/{commit}");
    let api = |path: &str| {
        if path == endpoint && !checked.get() {
            checked.set(true);
            let error = fx.library.remove_skill_source(&existing.id).unwrap_err();
            assert!(error.contains("import in progress"), "{error}");
        }
        fake.call(path)
    };
    fx.library.import_skill_source(&api, "me/slow", "skills", None, &commit,
        vec!["skills/alpha".into()]).unwrap();
    assert!(checked.get(), "test must attempt the conflicting removal before scanning");
    assert_eq!(fx.library.saved_skill_sources().unwrap().len(), 1);
}

#[test]
fn imports_in_distinct_library_directories_can_overlap_without_global_rejection() {
    let first = Fixture::new();
    let second = Fixture::new();
    let commit = "d".repeat(40);
    let mut first_api = Fake::default();
    let mut second_api = Fake::default();
    first_api.repo("me/first", &commit, &files(&[
        ("skills/alpha/SKILL.md", &skill("Alpha")),
    ]));
    second_api.repo("me/second", &commit, &files(&[
        ("skills/beta/SKILL.md", &skill("Beta")),
    ]));
    let observed = std::cell::Cell::new(false);
    let endpoint = format!("repos/me/first/commits/{commit}");
    let api = |path: &str| {
        if path == endpoint && !observed.get() {
            observed.set(true);
            let error = first.library.import_skill_source(&first_api.api(), "me/first", "skills", None, &commit,
                vec!["skills/alpha".into()]).unwrap_err();
            assert!(error.contains("import is still running"), "{error}");
            let concurrent = second.library.import_skill_source(&second_api.api(), "me/second", "skills", None, &commit,
                vec!["skills/beta".into()]).unwrap();
            assert_eq!(concurrent.imported, ["Beta"],
                "an unrelated profile must never fail because another source is importing");
        }
        first_api.call(path)
    };
    first.library.import_skill_source(&api, "me/first", "skills", None, &commit,
        vec!["skills/alpha".into()]).unwrap();
    assert!(observed.get());
    assert!(first.library.load("alpha").is_ok());
    assert!(second.library.load("beta").is_ok());
}
