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

fn required_by_default() -> bool { true }

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
    /// Frozen per-skill acceptance role; old mandatory receipts stay mandatory.
    #[serde(default = "required_by_default")]
    pub required_for_acceptance: bool,
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
pub(crate) struct SkippedSkill {
    pub id: String,
    /// Exact resolver failure; unlike a model self-report, this is a native decision.
    pub reason: String,
    pub bot_assigned: bool,
    pub eligible: bool,
    pub supplied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReviewerSkills {
    pub pins: Vec<SkillPin>,
    /// Exact lower-trust instructions captured at admission; never regenerate from live library.
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<SkippedSkill>,
}

impl ReviewerSkills {
    /// Recheck staged supporting files on every reviewer admission, including restarts.
    pub fn verify(&self) -> Result<(), String> {
        if self.prompt.chars().count() > TOTAL_CHARS {
            return Err("reviewer_skill_drift: frozen skill prompt exceeds its budget.".into());
        }
        for skipped in &self.skipped {
            if skipped.id.is_empty() || skipped.reason.is_empty() || !skipped.bot_assigned
                || skipped.eligible || skipped.supplied || self.pins.iter().any(|pin| pin.id == skipped.id)
            {
                return Err("reviewer_skill_drift: invalid skipped skill receipt.".into());
            }
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

/// Reclaim only materialized reviewer packages no longer referenced by any
/// persisted round. Call while holding the app store lock, so a Council cannot
/// create an unfinished round between the reference scan and removal.
pub(crate) fn cleanup_orphaned(library: &Library, store: &crate::storage::Store) -> Result<usize, String> {
    let root = library.dir.join(STAGE_ROOT);
    if !root.exists() { return Ok(0); }
    safe_dir(&root)?;
    let mut retained = HashSet::new();
    {
        let mut statement = store.db.prepare(
            "SELECT content FROM architect_requests WHERE kind='reviewer_round'",
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let raw = row.map_err(|e| e.to_string())?;
            let round: Vec<serde_json::Value> = serde_json::from_str(&raw)
                .map_err(|_| "reviewer_skill_cleanup_blocked: invalid native round metadata.".to_owned())?;
            for seat in round {
                let Some(skills) = seat.get("skills").filter(|s| !s.is_null()) else { continue };
                let inputs: ReviewerSkills = serde_json::from_value(skills.clone())
                    .map_err(|_| "reviewer_skill_cleanup_blocked: malformed frozen skill receipt.".to_owned())?;
                for pin in &inputs.pins {
                    if let Some(path) = &pin.staged_path {
                        let path = Path::new(path);
                        if path.parent() == Some(root.as_path()) {
                            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                                retained.insert(name.to_owned());
                            }
                        }
                    }
                }
            }
        }
    }
    let mut removed = 0;
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if retained.contains(&name) { continue; }
        let candidate = entry.path();
        let metadata = fs::symlink_metadata(&candidate).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(format!("reviewer_skill_cleanup_blocked: unsafe staged entry {name}."));
        }
        // Refuse all unknown names instead of interpreting arbitrary user folders
        // as Council-created materialization.
        let named_snapshot = name.rsplit_once('-').is_some_and(|(id, digest)|
            super::valid_id(id) && digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let interrupted = name.strip_prefix(".pending-")
            .is_some_and(|suffix| uuid::Uuid::parse_str(suffix).is_ok());
        if !named_snapshot && !interrupted {
            return Err(format!("reviewer_skill_cleanup_blocked: unknown staged entry {name}."));
        }
        remove_staged_dir(&candidate)?;
        removed += 1;
    }
    Ok(removed)
}

fn remove_staged_dir(path: &Path) -> Result<(), String> {
    safe_dir(path)?;
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = entry.path();
        let metadata = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("reviewer_skill_cleanup_blocked: unexpected staged symlink.".into());
        }
        if metadata.is_dir() {
            remove_staged_dir(&target)?;
        } else if metadata.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(windows)]
            {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(false);
                fs::set_permissions(&target, permissions).map_err(|e| e.to_string())?;
            }
            fs::remove_file(&target).map_err(|e| e.to_string())?;
        } else {
            return Err("reviewer_skill_cleanup_blocked: unsupported staged file type.".into());
        }
    }
    fs::remove_dir(path).map_err(|e| e.to_string())
}

/// Resolve only explicitly bot-assigned skill IDs plus their required dependencies.
/// A missing/unsafe/drifted/truncated mandatory input fails closed, never silently truncates.
pub(crate) fn freeze(library: &Library, assigned: &[String], harness: &str) -> Result<ReviewerSkills, String> {
    freeze_with_optional(library, assigned, harness, &[])
}

/// A caller explicitly designates optional bot assignments per review task.
/// Required dependencies of an optional skill must resolve together or the whole
/// optional package is skipped; required skills still fail closed.
pub(crate) fn freeze_with_optional(
    library: &Library, assigned: &[String], harness: &str, optional: &[String],
) -> Result<ReviewerSkills, String> {
    let mut known_optional = HashSet::new();
    for id in optional {
        if !assigned.contains(id) || !known_optional.insert(id) {
            return Err(format!("reviewer_skill_unavailable: optional skill {id} is not an unique bot assignment."));
        }
    }
    if !adapter::known(harness) {
        return Err("reviewer_skill_unavailable: unknown reviewer harness.".into());
    }
    #[derive(Clone)]
    struct ResolveState {
        seen: HashSet<String>,
        visiting: HashSet<String>,
        pins: Vec<SkillPin>,
        prompt: String,
    }
    let mut state = ResolveState {
        seen: HashSet::new(), visiting: HashSet::new(), pins: Vec::new(), prompt: String::new(),
    };
    fn visit(library: &Library, id: &str, assigned: &[String], harness: &str, required: bool, state: &mut ResolveState) -> Result<(), String> {
        if state.seen.contains(id) { return Ok(()); }
        if !state.visiting.insert(id.to_owned()) {
            return Err(format!("reviewer_skill_cycle: dependency cycle at {id}."));
        }
        let result = (|| -> Result<(), String> {
            let meta = library.load(id)
                .map_err(|_| format!("reviewer_skill_unavailable: assigned skill {id} is missing."))?;
            // The on-disk metadata is untrusted; never follow a different ID's folder.
            if meta.id != id {
                return Err(format!("reviewer_skill_drift: library metadata identity mismatch for {id}."));
            }
            if !matches!(meta.source.kind.as_str(), "folder" | "github") {
                return Err(format!("reviewer_skill_untrusted: unsupported source kind for {id}."));
            }
            if meta.kind != Kind::Skill {
                return Err(format!("reviewer_skill_unavailable: {id} is not a skill."));
            }
            let (files, hashes) = trusted_package(library, &meta)?;
            for dependency in &meta.required_skills {
                visit(library, dependency, assigned, harness, required, state)?;
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
                required_for_acceptance: required, delivery_mode: mode.into(), staged_path,
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
    // Materialize mandatory closures first regardless of the order in Bot.skills.
    // Otherwise an optional root could pin a shared mandatory dependency as optional,
    // or roll back that dependency when optional provisioning fails.
    for id in assigned.iter().filter(|id| !known_optional.contains(*id)) {
        visit(library, id, assigned, harness, true, &mut state)?;
    }
    let mut skipped = Vec::new();
    for id in assigned.iter().filter(|id| known_optional.contains(*id)) {
        let before = state.clone();
        if let Err(reason) = visit(library, id, assigned, harness, false, &mut state) {
            state = before;
            skipped.push(SkippedSkill {
                id: id.clone(), reason, bot_assigned: true,
                eligible: false, supplied: false,
            });
        }
    }
    let result = ReviewerSkills { pins: state.pins, prompt: state.prompt, skipped };
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
    fn forged_library_metadata_cannot_redirect_a_review_to_another_package() {
        let (library, root) = fixture();
        add(&library, "review", "Check source evidence.", &[]);
        let metadata_path = library.dir.join("review.json");
        let original = fs::read(&metadata_path).unwrap();
        let mut forged: Meta = serde_json::from_slice(&original).unwrap();
        forged.id = "../unrelated".into();
        fs::write(&metadata_path, serde_json::to_vec(&forged).unwrap()).unwrap();
        assert!(freeze(&library, &["review".into()], "codex").unwrap_err()
            .contains("reviewer_skill_drift"));
        forged.id = "review".into();
        forged.source.kind = "arbitrary".into();
        fs::write(&metadata_path, serde_json::to_vec(&forged).unwrap()).unwrap();
        assert!(freeze(&library, &["review".into()], "codex").unwrap_err()
            .contains("reviewer_skill_untrusted"));
        fs::write(&metadata_path, original).unwrap();
        assert!(freeze(&library, &["review".into()], "codex").is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn optional_missing_or_invalid_skills_are_disclosed_without_dropping_required_criteria() {
        let (library, root) = fixture();
        add(&library, "required", "Required review criteria.", &[]);
        add(&library, "optional", "Optional review criteria.", &["unavailable"]);
        let assigned = vec!["optional".into(), "required".into()];
        let frozen = freeze_with_optional(&library, &assigned, "kimi", &["optional".into()]).unwrap();
        assert_eq!(frozen.skipped.len(), 1);
        assert_eq!(frozen.skipped[0].id, "optional");
        assert!(frozen.skipped[0].reason.contains("unavailable"));
        assert_eq!(frozen.pins.len(), 1);
        assert_eq!(frozen.pins[0].id, "required");
        assert!(frozen.prompt.contains("Required review criteria"));
        assert!(!frozen.prompt.contains("Optional review criteria"));
        assert!(frozen.verify().is_ok());

        assert!(freeze(&library, &assigned, "kimi").unwrap_err().contains("unavailable"));
        assert!(freeze_with_optional(&library, &assigned, "kimi", &["unassigned".into()])
            .unwrap_err().contains("not an unique bot assignment"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mandatory_and_optional_skill_roles_are_frozen_independent_of_assignment_order() {
        let (library, root) = fixture();
        add(&library, "shared", "Shared dependency.", &[]);
        add(&library, "mandatory", "Mandatory criteria.", &["shared"]);
        add(&library, "optional", "Optional method.", &["shared"]);
        let assigned = vec!["optional".into(), "mandatory".into()];
        let snapshot = freeze_with_optional(&library, &assigned, "kimi", &["optional".into()]).unwrap();
        assert!(snapshot.skipped.is_empty());
        assert_eq!(snapshot.pins.iter().map(|pin| pin.id.as_str()).collect::<Vec<_>>(),
            ["shared", "mandatory", "optional"]);
        assert!(snapshot.pins[0].required_for_acceptance);
        assert!(snapshot.pins[1].required_for_acceptance);
        assert!(!snapshot.pins[2].required_for_acceptance);
        assert_eq!(snapshot.pins[2].bot_assigned, true);
        let old_receipt = serde_json::json!({"requiredForAcceptance": true});
        assert_eq!(old_receipt["requiredForAcceptance"], true);
        let mut legacy = serde_json::to_value(&snapshot.pins[1]).unwrap();
        legacy.as_object_mut().unwrap().remove("requiredForAcceptance");
        let parsed: SkillPin = serde_json::from_value(legacy).unwrap();
        assert!(parsed.required_for_acceptance, "historical pins must not silently become optional");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn optional_failure_keeps_shared_mandatory_dependencies() {
        let (library, root) = fixture();
        add(&library, "shared", "Required verification.", &[]);
        add(&library, "mandatory", "Use shared.", &["shared"]);
        add(&library, "optional", "Optional absent branch.", &["missing"]);
        let assigned = vec!["optional".into(), "mandatory".into()];
        let result = freeze_with_optional(&library, &assigned, "kimi", &["optional".into()]).unwrap();
        assert_eq!(result.pins.iter().map(|pin| pin.id.as_str()).collect::<Vec<_>>(),
            ["shared", "mandatory"]);
        assert!(result.pins.iter().all(|pin| pin.required_for_acceptance));
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].id, "optional");
        assert!(result.verify().is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn staged_cleanup_preserves_persisted_rounds_and_reclaims_orphans() {
        let (library, root) = fixture();
        add(&library, "stable", "Review the pinned source.", &[]);
        let store = crate::storage::Store::open(Path::new(":memory:")).unwrap();
        let project = store.add_project(&std::env::temp_dir()).unwrap();
        let session = store.create_session(&project.id, "codex", "default", "full").unwrap();
        let pinned = freeze(&library, &["stable".into()], "codex").unwrap();
        let dir = PathBuf::from(pinned.pins[0].staged_path.as_ref().unwrap());
        assert!(dir.exists());
        store.db.execute(
            "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
             VALUES(?1,'round','reviewer_round',?2,'')",
            rusqlite::params![session.id, serde_json::json!([{"skills": pinned}]).to_string()],
        ).unwrap();
        assert_eq!(cleanup_orphaned(&library, &store).unwrap(), 0);
        assert!(dir.exists(), "a persisted round must never lose its immutable files");

        store.db.execute("DELETE FROM architect_requests WHERE parent_session_id=?1", [&session.id]).unwrap();
        assert_eq!(cleanup_orphaned(&library, &store).unwrap(), 1);
        assert!(!dir.exists(), "unused read-only snapshots must be reclaimed safely");
        assert_eq!(cleanup_orphaned(&library, &store).unwrap(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_refuses_to_delete_unrecognized_pending_directories() {
        let (library, root) = fixture();
        let stage_root = library.dir.join(STAGE_ROOT);
        fs::create_dir(&stage_root).unwrap();
        let unrelated = stage_root.join(".pending-user-data");
        fs::create_dir(&unrelated).unwrap();
        fs::write(unrelated.join("keep.txt"), b"Do not delete this directory.").unwrap();
        let store = crate::storage::Store::open(Path::new(":memory:")).unwrap();
        assert!(cleanup_orphaned(&library, &store).unwrap_err().contains("cleanup_blocked"));
        assert_eq!(fs::read(unrelated.join("keep.txt")).unwrap(), b"Do not delete this directory.");
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
