use super::{catalog, tests::Fixture, *};
use base64::Engine;
use serde_json::{json, Value};
use std::{cell::RefCell, collections::BTreeMap};
const COMMIT: &str = "1111111111111111111111111111111111111111";
const NEXT: &str = "2222222222222222222222222222222222222222";
fn body(name: &str, text: &str) -> String { format!("---\nname: {name}\ndescription: {name} description\n---\n{text}\n") }
struct Repo { frames: RefCell<BTreeMap<String, Value>>, calls: RefCell<Vec<String>> }
impl Repo {
    fn new(entries: &[(&str, &str, &str)]) -> Self {
        let mut frames = BTreeMap::new(); let mut tree = Vec::new();
        frames.insert("repos/acme/skills/commits/HEAD".into(), json!({"sha": COMMIT}));
        frames.insert(format!("repos/acme/skills/commits/{COMMIT}"), json!({"sha": COMMIT}));
        for (index, (path, mode, text)) in entries.iter().enumerate() {
            let sha = format!("{:040x}", index + 10);
            tree.push(json!({"path":path,"mode":mode,"type":"blob","sha":sha,"size":text.len()}));
            frames.insert(format!("repos/acme/skills/git/blobs/{sha}"), json!({"sha":sha,"encoding":"base64","content":base64::engine::general_purpose::STANDARD.encode(text)}));
        }
        frames.insert(format!("repos/acme/skills/git/trees/{COMMIT}?recursive=1"), json!({"truncated":false,"tree":tree}));
        Self { frames: RefCell::new(frames), calls: RefCell::new(vec![]) }
    }
    fn api(&self, endpoint: &str) -> Result<Value, String> {
        self.calls.borrow_mut().push(endpoint.into());
        self.frames.borrow().get(endpoint).cloned().ok_or_else(|| format!("Unexpected endpoint: {endpoint}"))
    }
}
#[test]
fn discovery_is_nested_bounded_and_does_not_import_or_activate() {
    let fx = Fixture::new();
    let root = body("Root", "Root instructions"); let child = body("Child", "Child instructions");
    let repo = Repo::new(&[("SKILL.md", "100644", &root), ("nested/child/SKILL.md", "100644", &child), ("nested/child/install.sh", "100755", "exit 99"), ("link/SKILL.md", "120000", "../../secret")]);
    let catalog = fx.library.catalog_add(&|p| repo.api(p), "acme/skills", None).unwrap();
    assert_eq!(catalog.sources[0].skills.len(), 2);
    assert_eq!(catalog.sources[0].skills[0].files, 1);
    assert!(fx.library.list().unwrap().is_empty());
    assert!(!fx.home.join(".claude/skills/root").exists());
    assert_eq!(repo.calls.borrow().len(), 4); // commit/tree/two descriptions; scripts untouched
    let result = fx.library.catalog_import(&|p| repo.api(p), "acme/skills", &["nested/child".into()]).unwrap();
    assert_eq!(result.imported, ["Child"]); assert!(result.skipped.is_empty());
    let item = fx.library.list().unwrap().remove(0);
    assert_eq!(item.meta.source.commit.as_deref(), Some(COMMIT));
    assert_eq!(item.meta.source.location, "acme/skills/nested/child");
    assert_eq!(item.files, ["SKILL.md", "install.sh"]);
    assert!(item.meta.activations.is_empty()); assert_eq!(item.meta.sha256.len(), 64);
    assert!(repo.calls.borrow().contains(&format!("repos/acme/skills/commits/{COMMIT}")));
}
#[test]
fn pinned_import_refuses_moved_commit_and_edited_content() {
    let fx = Fixture::new(); let text = body("Review", "Review changes");
    let repo = Repo::new(&[("review/SKILL.md", "100644", &text)]);
    fx.library.catalog_add(&|p| repo.api(p), "acme/skills", None).unwrap();
    repo.frames.borrow_mut().insert(format!("repos/acme/skills/commits/{COMMIT}"), json!({"sha": NEXT}));
    assert!(fx.library.catalog_import(&|p| repo.api(p), "acme/skills", &["review".into()]).unwrap_err().contains("pinned commit"));
    repo.frames.borrow_mut().insert(format!("repos/acme/skills/commits/{COMMIT}"), json!({"sha": COMMIT}));
    fx.library.catalog_import(&|p| repo.api(p), "acme/skills", &["review".into()]).unwrap();
    fs::write(fx.library.dir.join("review/SKILL.md"), "user edit").unwrap();
    let result = fx.library.catalog_import(&|p| repo.api(p), "acme/skills", &["review".into()]).unwrap();
    assert!(result.imported.is_empty()); assert!(result.skipped[0].reason.contains("edited on disk"));
    assert_eq!(fs::read_to_string(fx.library.dir.join("review/SKILL.md")).unwrap(), "user edit");
}
#[test]
fn catalog_state_and_removal_do_not_change_library_items() {
    let fx = Fixture::new(); let text = body("Review", "Review changes");
    let repo = Repo::new(&[("review/SKILL.md", "100644", &text)]);
    fx.library.catalog_add(&|p| repo.api(p), "acme/skills", None).unwrap();
    for state in [catalog::State::Disabled, catalog::State::Ignored] {
        fx.library.catalog_state("acme/skills", Some(state)).unwrap();
        assert!(fx.library.catalog_import(&|p| repo.api(p), "acme/skills", &["review".into()]).is_err());
        assert!(fx.library.catalog_refresh(&|p| repo.api(p), "acme/skills").is_err());
    }
    fx.library.catalog_state("acme/skills", Some(catalog::State::Enabled)).unwrap();
    fx.library.catalog_import(&|p| repo.api(p), "acme/skills", &["review".into()]).unwrap();
    assert!(fx.library.catalog_state("acme/skills", None).unwrap().sources.is_empty());
    assert_eq!(fx.library.list().unwrap().len(), 1);
}
#[test]
fn catalog_reports_source_name_conflicts_without_overwriting() {
    let fx = Fixture::new(); let text = body("Review", "Review changes");
    let path = fx.folder("review", &[("SKILL.md", &text)]); fx.import(Kind::Skill, &path);
    let repo = Repo::new(&[("nested/review/SKILL.md", "100644", &text)]);
    let catalog = fx.library.catalog_add(&|p| repo.api(p), "acme/skills", None).unwrap();
    assert_eq!(catalog.sources[0].skills[0].conflicts.len(), 1);
    let result = fx.library.catalog_import(&|p| repo.api(p), "acme/skills", &["nested/review".into()]).unwrap();
    assert!(result.imported.is_empty()); assert!(result.skipped[0].reason.contains("another source"));
    assert_eq!(fx.library.list().unwrap()[0].meta.source.kind, "folder");
}
#[test]
fn malformed_ids_truncated_trees_links_and_path_traversal_fail_closed() {
    let text = body("Valid", "Instructions");
    let repo = Repo::new(&[("../escape/SKILL.md", "100644", &text), (".hidden/SKILL.md", "100644", &text), ("sym/SKILL.md", "120000", &text), ("valid/SKILL.md", "100644", &text)]);
    let (_, skills) = catalog::scan(&|p| repo.api(p), "acme/skills", None).unwrap();
    assert_eq!(skills.len(), 1); assert_eq!(skills[0].path, "valid");
    repo.frames.borrow_mut().get_mut(&format!("repos/acme/skills/git/trees/{COMMIT}?recursive=1")).unwrap()["truncated"] = json!(true);
    assert!(catalog::scan(&|p| repo.api(p), "acme/skills", None).is_err());
    repo.frames.borrow_mut().insert("repos/acme/skills/commits/HEAD".into(), json!({"sha":"../../bad"}));
    assert!(catalog::scan(&|p| repo.api(p), "acme/skills", None).is_err());
    assert!(catalog::scan(&|p| repo.api(p), "--flag/repo", None).is_err());
}
#[test]
fn import_rejects_oversize_wrong_blob_size_unsafe_and_duplicate_paths() {
    let fx = Fixture::new(); let text = body("Review", "Instructions");
    let repo = Repo::new(&[("review/SKILL.md", "100644", &text)]);
    let endpoint = format!("repos/acme/skills/git/blobs/{:040x}", 10);
    repo.frames.borrow_mut().get_mut(&endpoint).unwrap()["content"] = json!(base64::engine::general_purpose::STANDARD.encode("changed bytes"));
    assert!(catalog::scan(&|p| repo.api(p), "acme/skills", None).is_err());
    let source = Source { kind: "folder".into(), location: "fixture".into(), git_ref: None, commit: None };
    for files in [vec![("../bad".into(), text.as_bytes().to_vec())], vec![("SKILL.md".into(), vec![0; MAX_BYTES+1])], vec![("SKILL.md".into(), text.as_bytes().to_vec()), ("skill.md".into(), text.as_bytes().to_vec())]] {
        assert!(fx.library.import(Kind::Skill, files, "Review", source.clone()).is_err());
    }
    assert!(fx.library.list().unwrap().is_empty());
}

#[test]
fn github_path_case_is_part_of_source_identity() {
    assert!(github::same_location("ACME/Skills/Tools/Review", "acme/skills/Tools/Review"));
    assert!(!github::same_location("acme/skills/Tools/Review", "acme/skills/tools/review"));
    let fx = Fixture::new(); let text = body("Review", "Instructions");
    let source = |location: &str| Source { kind: "github".into(), location: location.into(), git_ref: None, commit: Some(COMMIT.into()) };
    fx.library.import(Kind::Skill, vec![("SKILL.md".into(), text.as_bytes().to_vec())], "Review", source("acme/skills/Tools/Review")).unwrap();
    assert!(fx.library.import(Kind::Skill, vec![("SKILL.md".into(), text.as_bytes().to_vec())], "Review", source("acme/skills/tools/review")).unwrap_err().contains("another source"));
}

#[test]
fn catalog_accepts_folded_descriptions_and_rejects_substituted_blob_ids() {
    let text = "---\nname: Review\ndescription: >-\n  Review code changes\n  and test edge cases.\n---\nInstructions.\n";
    let repo = Repo::new(&[("review/SKILL.md", "100644", text)]);
    let (_, skills) = catalog::scan(&|p| repo.api(p), "acme/skills", None).unwrap();
    assert_eq!(skills[0].description, "Review code changes and test edge cases.");
    let endpoint = format!("repos/acme/skills/git/blobs/{:040x}", 10);
    repo.frames.borrow_mut().get_mut(&endpoint).unwrap()["sha"] = json!(NEXT);
    assert!(catalog::scan(&|p| repo.api(p), "acme/skills", None).unwrap_err().contains("different blob"));
}
