//! Immutable, task-scoped Council skill inputs from the single Skills Library.
//! No global activation, bot workspace sync, script execution or tool grants.
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use serde::{Deserialize, Serialize};

use super::{adapter, content_hash, frontmatter, sha256_hex, Kind, Library, Meta, MAX_BYTES, MAX_FILES};

const SKILL_CHARS: usize = 16_384;
const TOTAL_CHARS: usize = 65_536;
const STAGE_ROOT: &str = ".council-reviewer-snapshots";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SkillPin {
    pub id: String,
    pub source_kind: String,
    pub source_location: String,
    pub source_ref: Option<String>,
    pub source_commit: Option<String>,
    pub content_sha256: String,
    pub file_sha256: BTreeMap<String, String>,
    pub dependencies: Vec<String>,
    pub delivery_mode: String,
    pub staged_path: Option<String>,
    pub imported: bool,
    pub bot_assigned: bool,
    pub eligible: bool,
    pub supplied: bool,
    // Neither a model-written claim nor an advertised slash command sets these.
    pub invoked: bool,
    pub check_executed: bool,
    pub evidence_verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerSkills {
    pub pins: Vec<SkillPin>,
    /// Exact lower-trust instructions captured at admission; never regenerate from live library.
    pub prompt: String,
}

impl ReviewerSkills {
    /// Recheck staged supporting files on every reviewer admission, including restarts.
    pub fn verify(&self) -> Result<(), String> {
        if self.prompt.chars().count() > TOTAL_CHARS {
            return Err("reviewer_skill_drift: frozen skill prompt exceeds its budget.".into());
        }
        for pin in &self.pins {
            if !pin.eligible || !pin.supplied || pin.invoked || pin.check_executed || pin.evidence_verified {
                return Err("reviewer_skill_drift: invalid immutable skill receipt.".into());
            }
            if let Some(root) = &pin.staged_path {
                let root = Path::new(root);
                safe_dir(root)?;
                for (relative, expected) in &pin.file_sha256 {
                    let path = checked_path(root, relative)?;
                    let bytes = fs::read(&path).map_err(|_| format!("reviewer_skill_drift: missing staged file for {}.", pin.id))?;
                    if sha256_hex(&bytes) != *expected {
                        return Err(format!("reviewer_skill_drift: staged file changed for {}.", pin.id));
                    }
                }
                let observed = super::read_local(root, true)
                    .map_err(|_| format!("reviewer_skill_drift: staged package unavailable for {}.", pin.id))?;
                if observed.len() != pin.file_sha256.len() || content_hash(&observed) != pin.content_sha256 {
                    return Err(format!("reviewer_skill_drift: staged package file set changed for {}.", pin.id));
                }
            }
        }
        Ok(())
    }
}

fn safe_dir(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|_| "reviewer_skill_unavailable: cannot inspect a skill directory.".to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("reviewer_skill_unavailable: unsafe skill directory.".into());
    }
    Ok(())
}

fn checked_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty() || relative.split('/').any(|part| part.is_empty() || part == "." || part == ".." || part == ".git")
        || relative.contains('\\') || relative.starts_with('/')
    {
        return Err("reviewer_skill_unavailable: unsafe supporting file path.".into());
    }
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
        let meta = fs::symlink_metadata(&path)
            .map_err(|_| "reviewer_skill_unavailable: missing supporting file.".to_string())?;
        if meta.file_type().is_symlink() {
            return Err("reviewer_skill_unavailable: linked skill files are forbidden.".into());
        }
    }
    if !path.is_file() {
        return Err("reviewer_skill_unavailable: supporting file is not regular.".into());
    }
    Ok(path)
}

fn trusted_package(library: &Library, meta: &Meta) -> Result<(super::Files, BTreeMap<String, String>), String> {
    let folder = library.dir.join(&meta.id);
    safe_dir(&folder)?;
    let files = library.files(&meta.id)
        .map_err(|_| format!("reviewer_skill_unavailable: {} package cannot be read.", meta.id))?;
    if files.is_empty() || files.len() > MAX_FILES || files.iter().map(|(_, b)| b.len()).sum::<usize>() > MAX_BYTES
        || !files.iter().any(|(path, _)| path == "SKILL.md")
    {
        return Err(format!("reviewer_skill_unavailable: incomplete package for {}.", meta.id));
    }
    let hashes = files.iter().map(|(path, bytes)| {
        checked_path(&folder, path)?;
        Ok((path.clone(), sha256_hex(bytes)))
    }).collect::<Result<BTreeMap<_, _>, String>>()?;
    if content_hash(&files) != meta.sha256 || (!meta.file_hashes.is_empty() && meta.file_hashes != hashes) {
        return Err(format!("reviewer_skill_drift: library contents changed for {}.", meta.id));
    }
    if super::peer_dependencies(&files).map_err(|_| format!("reviewer_skill_drift: invalid dependency manifest for {}.", meta.id))? != meta.required_skills {
        return Err(format!("reviewer_skill_drift: dependency metadata changed for {}.", meta.id));
    }
    let findings = files.iter().flat_map(|(name, bytes)| {
        super::safety::scan(name, &String::from_utf8_lossy(bytes))
    }).collect::<Vec<_>>();
    if findings.iter().any(|f| f.rule == "secret")
        || (!findings.is_empty() && meta.acknowledged.as_deref() != Some(meta.sha256.as_str()))
    {
        return Err(format!("reviewer_skill_untrusted: {} needs a safe, acknowledged library revision.", meta.id));
    }
    if meta.source.kind == "github" && !meta.source.commit.as_deref().is_some_and(|commit|
        commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(format!("reviewer_skill_source_unpinned: {} requires a resolved GitHub commit.", meta.id));
    }
    Ok((files, hashes))
}

/// Immutable, content-addressed read-only copies outside the assessed checkout.
/// A harness is told the exact path and never receives a global skill activation.
fn stage(library: &Library, meta: &Meta, files: &super::Files, hashes: &BTreeMap<String, String>) -> Result<String, String> {
    let stage_root = library.dir.join(STAGE_ROOT);
    if stage_root.exists() { safe_dir(&stage_root)?; }
    else { fs::create_dir(&stage_root).map_err(|e| format!("reviewer_skill_unavailable: cannot stage skills: {e}"))?; }
    let target = stage_root.join(format!("{}-{}", meta.id, meta.sha256));
    if target.exists() {
        safe_dir(&target)?;
    } else {
        let pending = stage_root.join(format!(".pending-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&pending).map_err(|e| format!("reviewer_skill_unavailable: cannot stage skills: {e}"))?;
        let written = (|| -> Result<(), String> {
            for (relative, bytes) in files {
                let path = pending.join(relative);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&path, bytes).map_err(|e| e.to_string())?;
                let mut permissions = fs::metadata(&path).map_err(|e| e.to_string())?.permissions();
                permissions.set_readonly(true);
                fs::set_permissions(&path, permissions).map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
        if let Err(error) = written {
            let _ = fs::remove_dir_all(&pending);
            return Err(format!("reviewer_skill_unavailable: staging {} failed: {error}", meta.id));
        }
        if let Err(error) = fs::rename(&pending, &target) {
            let _ = fs::remove_dir_all(&pending);
            if !target.is_dir() {
                return Err(format!("reviewer_skill_unavailable: staging {} failed: {error}", meta.id));
            }
        }
    }
    for (relative, expected) in hashes {
        let file = checked_path(&target, relative)?;
        let bytes = fs::read(file).map_err(|e| e.to_string())?;
        if sha256_hex(&bytes) != *expected {
            return Err(format!("reviewer_skill_drift: staged content differs for {}.", meta.id));
        }
    }
    Ok(target.to_string_lossy().into_owned())
}

/// Resolve only explicitly bot-assigned skill IDs plus their required dependencies.
/// A missing/unsafe/drifted/truncated mandatory input fails closed, never silently truncates.
pub(crate) fn freeze(library: &Library, assigned: &[String], harness: &str) -> Result<ReviewerSkills, String> {
    if !adapter::known(harness) {
        return Err("reviewer_skill_unavailable: unknown reviewer harness.".into());
    }
    struct ResolveState {
        seen: HashSet<String>,
        visiting: HashSet<String>,
        pins: Vec<SkillPin>,
        prompt: String,
    }
    let mut state = ResolveState {
        seen: HashSet::new(), visiting: HashSet::new(), pins: Vec::new(), prompt: String::new(),
    };
    fn visit(library: &Library, id: &str, assigned: &[String], harness: &str, state: &mut ResolveState) -> Result<(), String> {
        if state.seen.contains(id) { return Ok(()); }
        if !state.visiting.insert(id.to_owned()) {
            return Err(format!("reviewer_skill_cycle: dependency cycle at {id}."));
        }
        let result = (|| -> Result<(), String> {
            let meta = library.load(id)
                .map_err(|_| format!("reviewer_skill_unavailable: assigned skill {id} is missing."))?;
            if meta.kind != Kind::Skill {
                return Err(format!("reviewer_skill_unavailable: {id} is not a skill."));
            }
            let (files, hashes) = trusted_package(library, &meta)?;
            for dependency in &meta.required_skills {
                visit(library, dependency, assigned, harness, state)?;
            }
            let main = files.iter().find(|(path, _)| path == "SKILL.md")
                .ok_or_else(|| format!("reviewer_skill_unavailable: no SKILL.md for {id}."))?;
            let main_text = std::str::from_utf8(&main.1)
                .map_err(|_| format!("reviewer_skill_unavailable: SKILL.md of {id} is not UTF-8."))?;
            let (_, _, body) = frontmatter(main_text);
            let mut contents = body.to_owned();
            let mode = if adapter::FILE_HARNESSES.contains(&harness) { "isolated_files" } else { "bounded_prompt" };
            let staged_path = if mode == "isolated_files" {
                Some(stage(library, &meta, &files, &hashes)?)
            } else {
                for (name, bytes) in &files {
                    if name == "SKILL.md" { continue; }
                    let supporting = std::str::from_utf8(bytes).map_err(|_| {
                        format!("reviewer_skill_unavailable: {id} needs binary supporting file {name}.")
                    })?;
                    contents.push_str(&format!("\n\nSupporting file {name} (untrusted):\n{supporting}"));
                }
                None
            };
            if contents.chars().count() > SKILL_CHARS {
                return Err(format!("reviewer_skill_truncated: {id} needs more than {SKILL_CHARS} characters."));
            }
            let section = format!(
                "\n\n### Reviewer skill: {} ({})\nSource SHA-256: {}\nDelivery: {}\n{}{}\n",
                meta.name, meta.id, meta.sha256, mode,
                staged_path.as_deref().map_or_else(String::new, |path| format!("Read-only pinned files: {path}\n")),
                contents
            );
            if state.prompt.chars().count() + section.chars().count() > TOTAL_CHARS {
                return Err(format!("reviewer_skill_truncated: combined reviewer skills exceed {TOTAL_CHARS} characters."));
            }
            state.prompt.push_str(&section);
            state.pins.push(SkillPin {
                id: id.to_owned(), source_kind: meta.source.kind,
                source_location: meta.source.location, source_ref: meta.source.git_ref,
                source_commit: meta.source.commit, content_sha256: meta.sha256,
                file_sha256: hashes, dependencies: meta.required_skills,
                delivery_mode: mode.into(), staged_path,
                imported: true, bot_assigned: assigned.iter().any(|x| x == id),
                eligible: true, supplied: true,
                invoked: false, check_executed: false, evidence_verified: false,
            });
            state.seen.insert(id.to_owned());
            Ok(())
        })();
        state.visiting.remove(id);
        result
    }
    for id in assigned { visit(library, id, assigned, harness, &mut state)?; }
    let result = ReviewerSkills { pins: state.pins, prompt: state.prompt };
    result.verify()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Source;

    fn fixture() -> (Library, PathBuf) {
        let dir = std::env::temp_dir().join(format!("pandacode-reviewer-skills-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(dir.join("skills")).unwrap();
        (Library::new(dir.join("skills"), dir.clone()), dir)
    }
    fn add(library: &Library, id: &str, body: &str, required: &[&str]) {
        let dir = library.dir.join(id);
        fs::create_dir(&dir).unwrap();
        let mut files = vec![("SKILL.md".to_owned(), body.as_bytes().to_vec())];
        if !required.is_empty() {
            files.push(("skill-dependencies.json".into(),
                serde_json::to_vec(&serde_json::json!({"requiredSkills": required})).unwrap()));
        }
        for (path, bytes) in &files {
            fs::write(dir.join(path), bytes).unwrap();
        }
        let hashes = files.iter().map(|(p, v)| (p.clone(), sha256_hex(v))).collect();
        let meta = Meta {
            id: id.into(), name: id.into(), kind: Kind::Skill, description: String::new(),
            source: Source {kind: "folder".into(), location: id.into(),git_ref:None,commit:None},
            sha256: content_hash(&files), imported_at: 0, acknowledged: None, auto_update: None,
            auto_available: false, required_skills: required.iter().map(|s| s.to_string()).collect(),
            file_hashes: hashes, activations: vec![],
        };
        fs::write(library.dir.join(format!("{id}.json")), serde_json::to_vec(&meta).unwrap()).unwrap();
    }

    #[test]
    fn only_assigned_and_required_skills_are_supplied_without_global_activation() {
        let (library, root) = fixture();
        add(&library, "needed", "Check evidence carefully.", &[]);
        add(&library, "assigned", "Use the mandated review procedure.", &["needed"]);
        add(&library, "unassigned", "Ignore this unrelated skill.", &[]);
        let frozen = freeze(&library, &["assigned".into()], "opencode").unwrap();
        assert_eq!(frozen.pins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["needed", "assigned"]);
        assert!(frozen.prompt.contains("mandated review procedure"));
        assert!(!frozen.prompt.contains("unrelated skill"));
        assert!(frozen.pins.iter().all(|p| p.supplied && !p.invoked && !p.check_executed && !p.evidence_verified));
        assert!(library.list().unwrap().iter().all(|i| i.meta.activations.is_empty()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_dependencies_cycles_and_content_drift_fail_closed() {
        let (library, root) = fixture();
        add(&library, "first", "Read before reviewing.", &["second"]);
        assert!(freeze(&library, &["first".into()], "kimi").unwrap_err().contains("unavailable"));
        add(&library, "second", "Check", &["first"]);
        assert!(freeze(&library, &["first".into()], "kimi").unwrap_err().contains("reviewer_skill_cycle"));
        fs::write(library.dir.join("second/SKILL.md"), "modified").unwrap();
        assert!(freeze(&library, &["second".into()], "kimi").unwrap_err().contains("reviewer_skill_drift"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mutated_dependency_metadata_cannot_bypass_required_closure() {
        let (library, root) = fixture();
        add(&library, "review", "Review against evidence.", &["mandatory"]);
        add(&library, "mandatory", "Verify the checklist.", &[]);
        let path = library.dir.join("review.json");
        let mut meta: Meta = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        meta.required_skills.clear();
        fs::write(path, serde_json::to_vec(&meta).unwrap()).unwrap();
        assert!(freeze(&library, &["review".into()], "kimi").unwrap_err().contains("reviewer_skill_drift"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prompt_budget_is_never_silently_truncated() {
        let (library, root) = fixture();
        add(&library, "oversized", &"x".repeat(SKILL_CHARS + 1), &[]);
        assert!(freeze(&library, &["oversized".into()], "kimi").unwrap_err().contains("reviewer_skill_truncated"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_harness_stages_read_only_outside_checkout_and_detects_drift() {
        let (library, root) = fixture();
        add(&library, "file-skill", "Inspect the dependency tree.", &[]);
        let snapshot = freeze(&library, &["file-skill".into()], "codex").unwrap();
        let pinned = &snapshot.pins[0];
        let path = Path::new(pinned.staged_path.as_deref().unwrap()).join("SKILL.md");
        assert!(path.starts_with(&library.dir));
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        assert!(snapshot.verify().is_ok());
        let unrelated = Path::new(pinned.staged_path.as_deref().unwrap()).join("unexpected.md");
        fs::write(&unrelated, "Unpinned instructions").unwrap();
        assert!(snapshot.verify().unwrap_err().contains("reviewer_skill_drift"));
        fs::remove_file(unrelated).unwrap();
        assert!(snapshot.verify().is_ok());
        // A different version must never be silently used in a live reviewer.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        #[cfg(windows)]
        {
            let mut writable = fs::metadata(&path).unwrap().permissions();
            writable.set_readonly(false);
            fs::set_permissions(&path, writable).unwrap();
        }
        fs::write(&path, "changed").unwrap();
        assert!(snapshot.verify().unwrap_err().contains("reviewer_skill_drift"));
        fs::remove_dir_all(root).unwrap();
    }
}
