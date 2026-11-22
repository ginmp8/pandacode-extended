//! Optional turn-end project checks: one fast command the checkout itself
//! declares through its manifests, run once by the app after a clean turn
//! when the user turned checks on in Settings → Agents. Bounded like every
//! external tool; a failure goes back to the agent through the ordinary
//! message queue, where the user can edit or cancel the repair request.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command, time::Duration};

use crate::workspace::run_process;

const OUTPUT_LIMIT: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(120);
/// How much of a failed check's output rides back to the agent.
const TAIL_BYTES: usize = 4000;

const NO_PLAN: &str = "This project declares no fast check (no npm test script and no Cargo.toml).";

/// User-enabled project checks must not run in task-owned Architect workers
/// or Council child/reviewer sessions. No reviewer is granted shell tools.
pub(crate) fn eligible_turn_check_project(
    store: &crate::storage::Store, session_id: &str,
) -> crate::Result<String> {
    let session = store.session(session_id)?;
    if crate::architect::parent_of(store, session_id)?.is_some() {
        return Err("reviewer_policy_denied: task-owned Architect workers cannot run project checks.".into());
    }
    Ok(session.project_id)
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

/// Only the existing, explicitly enabled host path invokes this runner.
/// A missing or changed check manifest cannot produce an execution receipt;
/// failures are observed failures, not passing evidence.
fn report_for_plan(root: &Path, plan: &Plan) -> Value {
    let before = manifest_sha256(root, plan);
    match run(root, plan) {
        Ok(mut report) => {
            if let (Some(a), Some(b)) = (before, manifest_sha256(root, plan)) {
                if a == b {
                    report["hostObservation"] = json!({
                        "kind": "manifest_declared_check",
                        "name": plan.name,
                        "manifestSha256": a,
                        "observedAt": crate::storage::now(),
                        "passed": report["ok"] == true,
                        "outputTruncated": report["truncated"] == true,
                        "provenance": "native_host",
                        "scope": "manifest_only_not_artifact_verification",
                    });
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
    }

    #[test]
    fn tails_cut_at_char_boundaries() {
        let text = "ж".repeat(TAIL_BYTES); // 2 bytes each, so the cut lands mid-character
        let cut = tail(&text);
        assert_eq!(cut.chars().count(), TAIL_BYTES / 2);
        assert_eq!(tail("done"), "done");
    }
}
