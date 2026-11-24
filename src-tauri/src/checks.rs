//! Optional turn-end project checks: one fast command the checkout itself
//! declares through its manifests, run once by the app after a clean turn
//! when the user turned checks on in Settings → Agents. Bounded like every
//! external tool; a failure goes back to the agent through the ordinary
//! message queue, where the user can edit or cancel the repair request.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::Read, path::{Path, PathBuf}, process::Command, time::Duration};

use crate::workspace::run_process;

const OUTPUT_LIMIT: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(120);
/// How much of a failed check's output rides back to the agent.
const TAIL_BYTES: usize = 4000;

const NO_PLAN: &str = "This project declares no fast check (no npm test script and no Cargo.toml).";

/// A bounded, deterministic snapshot of regular workspace files, excluding
/// generated/dependency directories and Git metadata. Only the aggregate
/// digest is retained; this is not an immutable or full artifact snapshot.
const MAX_SOURCE_ENTRIES: usize = 20_000;
const MAX_SOURCE_FILES: usize = 8_192;
const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SOURCE_DEPTH: usize = 16;

fn source_snapshot_sha256(root: &Path) -> Option<String> {
    fn collect(
        dir: &Path, files: &mut Vec<(PathBuf, u64)>, entries_seen: &mut usize,
        bytes: &mut u64, depth: usize,
    ) -> Option<()> {
        if depth > MAX_SOURCE_DEPTH { return None; }
        let mut entries = std::fs::read_dir(dir).ok()?
            .collect::<std::io::Result<Vec<_>>>().ok()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            *entries_seen = entries_seen.checked_add(1)?;
            if *entries_seen > MAX_SOURCE_ENTRIES { return None; }
            let name = entry.file_name();
            let name = name.to_str()?;
            // A linked worktree has a regular .git pointer instead of a directory.
            if name == ".git" { continue; }
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            // Never intentionally traverse symbolic links.
            if metadata.file_type().is_symlink() { return None; }
            if metadata.is_dir() {
                if matches!(name,
                    "node_modules" | "target" | "dist" | "build" |
                    "coverage" | "vendor" | ".venv" | ".next" | ".turbo" |
                    ".cache" | ".vite-cache" | "out"
                ) { continue; }
                collect(&path, files, entries_seen, bytes, depth + 1)?;
            } else if metadata.is_file() {
                *bytes = bytes.checked_add(metadata.len())?;
                if *bytes > MAX_SOURCE_BYTES || files.len() >= MAX_SOURCE_FILES {
                    return None;
                }
                files.push((path, metadata.len()));
            } else {
                // Devices, sockets and other non-regular files cannot be evidence.
                return None;
            }
        }
        Some(())
    }

    let (mut files, mut count, mut bytes) = (Vec::new(), 0, 0);
    collect(root, &mut files, &mut count, &mut bytes, 0)?;
    if files.is_empty() { return None; }
    let mut sha = Sha256::new();
    sha.update(b"pandacode:bounded-source-snapshot:v2");
    for (path, original_size) in files {
        let relative = path.strip_prefix(root).ok()?.to_str()?;
        // A file that grows after collection must not defeat the byte budget.
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        if !metadata.is_file() || metadata.len() != original_size { return None; }
        let file = std::fs::File::open(&path).ok()?;
        if !file.metadata().ok()?.is_file() { return None; }
        let mut data = Vec::new();
        file.take(original_size.checked_add(1)?).read_to_end(&mut data).ok()?;
        if data.len() as u64 != original_size { return None; }
        sha.update((relative.len() as u64).to_le_bytes());
        sha.update(relative.as_bytes());
        sha.update(original_size.to_le_bytes());
        sha.update(&data);
    }
    Some(format!("{:x}", sha.finalize()))
}

/// User-enabled project checks must not run in task-owned Architect workers
/// or Council child/reviewer sessions. No reviewer is granted shell tools.
pub(crate) fn eligible_turn_check_project(
    store: &crate::storage::Store, session_id: &str,
) -> crate::Result<String> {
    let session = store.session(session_id)?;
    if crate::architect::parent_of(store, session_id)?.is_some() {
        return Err("reviewer_policy_denied: task-owned Architect workers cannot run project checks.".into());
    }
    // Recovery detaches retired child reviewer grants from architect_workers.
    // The session survives for audit and must never become a normal task with
    // project-level execution rights. Also recognize pre-marker retired leaves
    // through their original child TeamSnapshot.
    let retired: i64 = store.db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM architect_requests
            WHERE parent_session_id=?1 AND key='native-council-revoked-leaf'
              AND kind='council_revoked_leaf' AND worker_id=?1
         ) OR EXISTS(
            SELECT 1 FROM architect_teams AS team,
                 json_each(CASE WHEN json_valid(team.content) THEN team.content ELSE '{}' END, '$.workers') AS worker
            WHERE json_extract(worker.value, '$.sessionId')=?1
         )",
        [session_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if retired != 0 {
        return Err("reviewer_policy_denied: retired task-owned reviewers cannot run project checks.".into());
    }
    Ok(session.project_id)
}

/// Revalidate the exact workspace that the host actually checked. A project
/// can be relinked without changing its ID or the session's Council pins.
/// This is a pre/post execution guard, not an immutable artifact snapshot.
pub(crate) fn validate_native_check_workspace(
    store: &crate::storage::Store, session_id: &str, project_id: &str,
    expected_root: &Path,
) -> crate::Result<()> {
    if eligible_turn_check_project(store, session_id)? != project_id
        || store.workspace_root(project_id, Some(session_id))?.as_path() != expected_root
    {
        return Err("council_task_stale: host check workspace changed during execution.".into());
    }
    Ok(())
}

/// One fast check a checkout declares through its own manifests.
pub(crate) struct Plan {
    pub name: String,
    program: String,
    args: Vec<String>,
}

/// The npm `test` script when package.json declares one, otherwise `cargo
/// check` for a Rust checkout. Nothing is invented beyond the manifests.
pub(crate) fn plan(root: &Path) -> Option<Plan> {
    if let Ok(package) = std::fs::read_to_string(root.join("package.json")) {
        let declares_test = serde_json::from_str::<Value>(&package)
            .ok()
            .is_some_and(|package| {
                package["scripts"]["test"]
                    .as_str()
                    .map(str::trim)
                    .is_some_and(|script| !script.is_empty())
            });
        if declares_test {
            return Some(Plan { name: "npm test".into(), program: "npm".into(), args: vec!["test".into(), "--silent".into()] });
        }
    }
    if root.join("Cargo.toml").is_file() {
        return Some(Plan { name: "cargo check".into(), program: "cargo".into(), args: vec!["check".into()] });
    }
    None
}

/// Runs the checkout's declared check and reports the outcome as data for the
/// composer queue. A missing plan is a result, not an error.
/// A host-executed observation only: never a reviewer score, Council pass,
/// or proof that the whole source tree was reviewed. The SHA-256 covers
/// just the manifest declaring this bounded, user-enabled check.
fn manifest_sha256(root: &Path, plan: &Plan) -> Option<String> {
    let name = match plan.name.as_str() {
        "npm test" => "package.json",
        "cargo check" => "Cargo.toml",
        _ => return None,
    };
    std::fs::read(root.join(name))
        .ok().map(|bytes| format!("{:x}", Sha256::digest(bytes)))
}

pub(crate) fn turn_report(root: &Path) -> Value {
    let Some(plan) = plan(root) else {
        return json!({"checked": false, "reason": NO_PLAN});
    };
    report_for_plan(root, &plan)
}

/// Only admitted native Council tasks pay for a bounded source snapshot.
/// Ordinary turn checks retain their existing manifest-only behavior.
pub(crate) fn turn_report_for_native_task(root: &Path) -> Value {
    let Some(plan) = plan(root) else {
        return json!({"checked": false, "reason": NO_PLAN});
    };
    report_for_plan_inner(root, &plan, true)
}

/// Only the existing, explicitly enabled host path invokes this runner.
/// A missing or changed check manifest cannot produce an execution receipt;
/// failures are observed failures, not passing evidence.
fn report_for_plan(root: &Path, plan: &Plan) -> Value {
    report_for_plan_inner(root, plan, false)
}

fn report_for_plan_inner(root: &Path, plan: &Plan, snapshot_sources: bool) -> Value {
    let before = manifest_sha256(root, plan);
    let source_before = snapshot_sources.then(|| source_snapshot_sha256(root));
    match run(root, plan) {
        Ok(mut report) => {
            if let (Some(a), Some(b)) = (before, manifest_sha256(root, plan)) {
                if a == b {
                    // A missing, oversized, linked or concurrently mutated source
                    // snapshot is explicitly NOT a verified native task receipt.
                    let source = if snapshot_sources {
                        match (source_before.flatten(), source_snapshot_sha256(root)) {
                            (Some(before), Some(after)) if before == after => Some(before),
                            _ => return report,
                        }
                    } else {
                        None
                    };
                    let scope = if snapshot_sources {
                        "bounded_source_snapshot_not_full_artifact_verification"
                    } else {
                        "manifest_only_not_artifact_verification"
                    };
                    report["hostObservation"] = json!({
                        "kind": "manifest_declared_check",
                        "name": plan.name,
                        "manifestSha256": a,
                        "observedAt": crate::storage::now(),
                        "passed": report["ok"] == true,
                        "outputTruncated": report["truncated"] == true,
                        "provenance": "native_host",
                        "scope": scope,
                    });
                    if let Some(source) = source {
                        report["hostObservation"]["sourceSnapshotSha256"] = json!(source);
                    }
                }
            }
            report
        }
        // No started/completed process: no observed test/build evidence.
        Err(error) => json!({"checked": false, "reason": error}),
    }
}

fn run(root: &Path, plan: &Plan) -> Result<Value, String> {
    // npm is npm.cmd on Windows, which std cannot exec directly.
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(&plan.program).args(&plan.args);
        command
    } else {
        let mut command = Command::new(&plan.program);
        command.args(&plan.args);
        command
    };
    command.current_dir(root);
    let output = run_process(command, &plan.name, OUTPUT_LIMIT, TIMEOUT)?;
    let ok = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout.bytes);
    let combined = if stdout.trim().is_empty() {
        String::from_utf8_lossy(&output.stderr.bytes).into_owned()
    } else {
        stdout.into_owned()
    };
    Ok(json!({
        "checked": true,
        "name": plan.name,
        "ok": ok,
        "output": if ok { String::new() } else { tail(&combined) },
        "truncated": output.stdout.truncated || output.stderr.truncated,
    }))
}

/// The last `TAIL_BYTES` bytes of a failed check's output, cut at a char
/// boundary: summaries and error counts sit at the end.
fn tail(text: &str) -> String {
    let text = text.trim();
    if text.len() <= TAIL_BYTES {
        return text.to_owned();
    }
    let mut start = text.len() - TAIL_BYTES;
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    text[start..].to_owned()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("pandacode-turn-checks-{name}"));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fake(path: &Path, body: &str) {
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn the_checkout_manifests_decide_the_check() {
        let temp = Scratch::new("plan");
        let root = temp.path();
        fs::write(root.join("package.json"), r#"{"scripts":{"test":"vitest run"}}"#).unwrap();
        let found = plan(root).unwrap();
        assert_eq!(found.name, "npm test");
        assert_eq!(found.args, vec!["test", "--silent"]);

        // An empty or malformed test script counts as no script at all.
        fs::write(root.join("package.json"), r#"{"scripts":{"test":""}}"#).unwrap();
        assert!(plan(root).is_none());
        fs::write(root.join("package.json"), "not json").unwrap();
        assert!(plan(root).is_none());

        // A Rust checkout falls back to cargo check.
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        let found = plan(root).unwrap();
        assert_eq!(found.name, "cargo check");
    }

    #[test]
    fn a_failing_check_carries_a_bounded_tail_and_a_passing_one_nothing() {
        let temp = Scratch::new("run");
        let root = temp.path();
        fs::write(root.join("package.json"), r#"{"scripts":{"test":"noop"}}"#).unwrap();
        let failing = root.join("npm");
        fake(&failing, "echo '  3 tests failed  ' >&2\nexit 1");
        let plan = Plan { name: "npm test".into(), program: failing.to_str().unwrap().into(), args: vec![] };
        let report = run(root, &plan).unwrap();
        assert_eq!(report["checked"], true);
        assert_eq!(report["ok"], false);
        assert_eq!(report["output"], "3 tests failed");

        fake(&failing, "echo 'all quiet on stdout'\nexit 0");
        let report = run(root, &plan).unwrap();
        assert_eq!(report["ok"], true);
        assert_eq!(report["output"], "");

        let long = "x".repeat(TAIL_BYTES + 500);
        fake(&failing, &format!("printf '%s' '{long}'\nexit 1"));
        let report = run(root, &plan).unwrap();
        assert_eq!(report["output"].as_str().unwrap().len(), TAIL_BYTES);
    }

    #[test]
    fn a_check_that_cannot_run_is_skipped_not_failed() {
        let temp = Scratch::new("spawn");
        let report = turn_report(temp.path());
        assert_eq!(report["checked"], false);
        assert!(report["reason"].as_str().unwrap().contains("no fast check"));

        let plan = Plan { name: "npm test".into(), program: "/nonexistent/pandacode-npm".into(), args: vec![] };
        let report = run(temp.path(), &plan).unwrap_err();
        assert!(report.contains("Could not start"), "{report}");
    }

    #[test]
    fn native_host_evidence_requires_real_execution_and_unchanged_manifest() {
        let temp = Scratch::new("native-host-evidence");
        let root = temp.path();
        fs::write(root.join("package.json"), r#"{"scripts":{"test":"echo ok"}}"#).unwrap();
        let runner = root.join("fake-npm");
        let plan = Plan {
            name: "npm test".into(),
            program: runner.to_str().unwrap().into(),
            args: vec![],
        };
        fake(&runner, "echo tests passed\nexit 0");
        let passed = report_for_plan(root, &plan);
        assert_eq!(passed["checked"], true);
        assert_eq!(passed["hostObservation"]["passed"], true);
        assert_eq!(passed["hostObservation"]["provenance"], "native_host");
        assert_eq!(passed["hostObservation"]["manifestSha256"].as_str().unwrap().len(), 64);
        assert_eq!(passed["hostObservation"]["scope"], "manifest_only_not_artifact_verification");

        fake(&runner, "echo tests failed >&2\nexit 1");
        let failed = report_for_plan(root, &plan);
        assert_eq!(failed["checked"], true);
        assert_eq!(failed["hostObservation"]["passed"], false);
        assert_eq!(failed["ok"], false);

        // A script that edits the manifest after starting is not proof that
        // its original immutable input still identifies the finished check.
        fake(&runner, "echo 'changed' > package.json\nexit 0");
        let mutated = report_for_plan(root, &plan);
        assert_eq!(mutated["checked"], true);
        assert!(mutated.get("hostObservation").is_none());

        // An executable that cannot even start never receives a receipt.
        let unavailable = Plan {
            name: "cargo check".into(), program: "/nonexistent/pandacode-cargo".into(),
            args: vec![],
        };
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"example\"").unwrap();
        let missing = report_for_plan(root, &unavailable);
        assert_eq!(missing["checked"], false);
        assert!(missing.get("hostObservation").is_none());
    }

    #[test]
    fn native_task_check_requires_a_stable_bounded_source_snapshot() {
        let temp = Scratch::new("native-task-source");
        let root = temp.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("package.json"), r#"{"scripts":{"test":"echo ok"}}"#).unwrap();
        fs::write(root.join("src/app.ts"), "export const value = 1;").unwrap();
        let runner = root.join("fake-npm");
        let plan = Plan {
            name: "npm test".into(), program: runner.to_str().unwrap().into(), args: vec![],
        };
        fake(&runner,
            "if [ -f mutate-source ]; then echo 'export const value = 2;' > src/app.ts; fi\necho tests passed\nexit 0");
        let first = report_for_plan_inner(root, &plan, true);
        assert_eq!(first["checked"], true);
        assert_eq!(first["hostObservation"]["passed"], true);
        assert_eq!(first["hostObservation"]["scope"],
            "bounded_source_snapshot_not_full_artifact_verification");
        let original = first["hostObservation"]["sourceSnapshotSha256"].as_str().unwrap().to_owned();
        assert_eq!(original.len(), 64);
        let repeated = report_for_plan_inner(root, &plan, true);
        assert_eq!(repeated["hostObservation"]["sourceSnapshotSha256"], original);

        // The process completed, but it changed what was under review.
        fs::write(root.join("mutate-source"), "yes").unwrap();
        let mutated = report_for_plan_inner(root, &plan, true);
        assert_eq!(mutated["checked"], true, "{mutated}");
        assert!(mutated.get("hostObservation").is_none(),
            "a passing process cannot certify a changed source snapshot");
        fs::remove_file(root.join("mutate-source")).unwrap();
        let after_change = report_for_plan_inner(root, &plan, true);
        assert_ne!(after_change["hostObservation"]["sourceSnapshotSha256"], original);

        // A symlink would make the file scope ambiguous; deny this stronger
        // receipt without breaking the ordinary manifest-declared check.
        std::os::unix::fs::symlink(root.join("src/app.ts"), root.join("src/alias.ts")).unwrap();
        let linked = report_for_plan_inner(root, &plan, true);
        assert_eq!(linked["checked"], true);
        assert!(linked.get("hostObservation").is_none());
        assert_eq!(report_for_plan(root, &plan)["hostObservation"]["scope"],
            "manifest_only_not_artifact_verification");
    }

    #[test]
    fn native_task_check_never_certifies_changed_review_documentation() {
        let temp = Scratch::new("native-document-snapshot");
        let root = temp.path();
        fs::write(root.join("package.json"), r#"{"scripts":{"test":"echo ok"}}"#).unwrap();
        fs::write(root.join("README.md"), "Original requirements").unwrap();
        let runner = root.join("fake-npm");
        fake(&runner,
            "if [ -f mutate-document ]; then echo 'Replaced requirements' > README.md; fi\necho checked\nexit 0");
        let plan = Plan {
            name: "npm test".into(), program: runner.to_str().unwrap().into(), args: vec![],
        };
        let original = report_for_plan_inner(root, &plan, true);
        assert_eq!(original["hostObservation"]["passed"], true);
        let original_hash = original["hostObservation"]["sourceSnapshotSha256"].as_str().unwrap();
        fs::write(root.join("mutate-document"), "yes").unwrap();
        let changed = report_for_plan_inner(root, &plan, true);
        assert_eq!(changed["checked"], true, "{changed}");
        assert!(changed.get("hostObservation").is_none(),
            "a changed review document must invalidate the host's bounded source receipt");
        fs::remove_file(root.join("mutate-document")).unwrap();
        let refreshed = report_for_plan_inner(root, &plan, true);
        assert_eq!(refreshed["hostObservation"]["passed"], true);
        assert_ne!(refreshed["hostObservation"]["sourceSnapshotSha256"], original_hash);
    }

    #[test]
    fn native_task_check_covers_binary_and_extensionless_workspace_artifacts() {
        let temp = Scratch::new("native-artifact-snapshot");
        let root = temp.path();
        fs::write(root.join("package.json"), r#"{"scripts":{"test":"echo ok"}}"#).unwrap();
        fs::write(root.join("LICENSE"), "original license").unwrap();
        fs::write(root.join("logo.bin"), [0_u8, 1, 2]).unwrap();
        fs::create_dir(root.join("build")).unwrap();
        fs::write(root.join("build/generated.bin"), [0_u8]).unwrap();
        let runner = root.join("fake-npm");
        fake(&runner,
            "if [ -f mutate-asset ]; then printf changed > logo.bin; fi\nif [ -f mutate-license ]; then echo changed > LICENSE; fi\necho generated > build/generated.bin\nexit 0");
        let plan = Plan {
            name: "npm test".into(), program: runner.to_str().unwrap().into(), args: vec![],
        };
        let original = report_for_plan_inner(root, &plan, true);
        assert_eq!(original["hostObservation"]["passed"], true);
        let repeated = report_for_plan_inner(root, &plan, true);
        assert_eq!(repeated["hostObservation"]["sourceSnapshotSha256"],
            original["hostObservation"]["sourceSnapshotSha256"]);
        fs::write(root.join("mutate-asset"), "yes").unwrap();
        let changed = report_for_plan_inner(root, &plan, true);
        assert_eq!(changed["checked"], true);
        assert!(changed.get("hostObservation").is_none());
        fs::remove_file(root.join("mutate-asset")).unwrap();
        fs::write(root.join("mutate-license"), "yes").unwrap();
        let changed = report_for_plan_inner(root, &plan, true);
        assert_eq!(changed["checked"], true);
        assert!(changed.get("hostObservation").is_none());
        fs::remove_file(root.join("mutate-license")).unwrap();
        let refreshed = report_for_plan_inner(root, &plan, true);
        assert_eq!(refreshed["hostObservation"]["passed"], true);
        assert_ne!(refreshed["hostObservation"]["sourceSnapshotSha256"],
            original["hostObservation"]["sourceSnapshotSha256"]);
        let oversized = fs::File::create(root.join("oversized.bin")).unwrap();
        oversized.set_len(MAX_SOURCE_BYTES + 1).unwrap();
        assert!(source_snapshot_sha256(root).is_none());
    }

    #[test]
    fn native_turn_checks_deny_task_owned_child_and_leaf_sessions() {
        let temp = Scratch::new("task-owned-check-denial");
        let store = crate::storage::Store::open(&temp.path().join("state.sqlite")).unwrap();
        let project = store.add_project(temp.path()).unwrap();
        let root = store.create_session(&project.id, "codex", "default", "ask").unwrap();
        let child = store.create_session(&project.id, "codex", "default", "plan").unwrap();
        let leaf = store.create_session(&project.id, "codex", "default", "plan").unwrap();
        assert_eq!(eligible_turn_check_project(&store, &root.id).unwrap(), project.id);
        for (parent, worker, session) in [
            (&root.id, "child-coordinator", &child.id),
            (&child.id, "read-only-leaf", &leaf.id),
        ] {
            store.db.execute(
                "INSERT INTO architect_workers(parent_session_id,worker_id,session_id) VALUES(?1,?2,?3)",
                rusqlite::params![parent, worker, session],
            ).unwrap();
        }
        for session in [&child.id, &leaf.id] {
            let denied = eligible_turn_check_project(&store, session).unwrap_err();
            assert!(denied.contains("reviewer_policy_denied"), "{denied}");
        }
        assert_eq!(eligible_turn_check_project(&store, &root.id).unwrap(), project.id,
            "ordinary user-owned tasks retain their existing check behavior");

        // A recovered leaf loses its scheduler owner but not its historical
        // task-only identity. This durable marker must survive detachment.
        store.db.execute(
            "INSERT INTO architect_requests(parent_session_id,key,kind,content,worker_id)
             VALUES(?1,'native-council-revoked-leaf','council_revoked_leaf','{}',?1)",
            [&leaf.id],
        ).unwrap();
        store.db.execute("DELETE FROM architect_workers WHERE session_id=?1", [&leaf.id]).unwrap();
        assert!(crate::architect::parent_of(&store, &leaf.id).unwrap().is_none());
        let denied = eligible_turn_check_project(&store, &leaf.id).unwrap_err();
        assert!(denied.contains("reviewer_policy_denied"), "{denied}");
        assert_eq!(eligible_turn_check_project(&store, &root.id).unwrap(), project.id);
    }

    #[test]
    fn native_host_check_rejects_relinked_workspace_even_with_same_project_id() {
        let temp = Scratch::new("native-check-relink");
        let original = temp.path().join("original");
        let replacement = temp.path().join("replacement");
        fs::create_dir(&original).unwrap();
        fs::create_dir(&replacement).unwrap();
        let store = crate::storage::Store::open(&temp.path().join("state.sqlite")).unwrap();
        let project = store.add_project(&original).unwrap();
        let root = store.create_session(&project.id, "codex", "default", "ask").unwrap();
        let checked_root = store.workspace_root(&project.id, Some(&root.id)).unwrap();
        validate_native_check_workspace(&store, &root.id, &project.id, &checked_root).unwrap();

        fs::write(original.join("package.json"), r#"{"scripts":{"test":"echo ok"}}"#).unwrap();
        let runner = original.join("fake-npm");
        fake(&runner, "echo checked\nexit 0");
        let plan = Plan {
            name: "npm test".into(),
            program: runner.to_str().unwrap().into(),
            args: vec![],
        };
        let report = report_for_plan_inner(&checked_root, &plan, true);
        assert_eq!(report["hostObservation"]["passed"], true);

        store.relink_project(&project.id, &replacement).unwrap();
        assert_eq!(eligible_turn_check_project(&store, &root.id).unwrap(), project.id);
        let error = validate_native_check_workspace(&store, &root.id, &project.id, &checked_root)
            .unwrap_err();
        assert!(error.contains("council_task_stale"), "{error}");

        // A fresh observation can still target the newly resolved workspace.
        let fresh = store.workspace_root(&project.id, Some(&root.id)).unwrap();
        validate_native_check_workspace(&store, &root.id, &project.id, &fresh).unwrap();
    }

    #[test]
    fn tails_cut_at_char_boundaries() {
        let text = "ж".repeat(TAIL_BYTES); // 2 bytes each, so the cut lands mid-character
        let cut = tail(&text);
        assert_eq!(cut.chars().count(), TAIL_BYTES / 2);
        assert_eq!(tail("done"), "done");
    }
}
