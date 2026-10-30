#!/usr/bin/env python3
"""Isolated, one-shot synthetic three-way *candidate* import. Not a parity validator."""
from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path

EXT = "f0105e7db85a0d7bc2e70726716fcfa939fac182"
UPSTREAM = "0336a2fb5e3abaea35c095fcc63805652f49ea98"
BASE = "70bdc708303c8681d8eb0b46f2cdb0ff087aa6eb"
BRANCH = "integration/0.7.0-main-to-main"
ORIGINAL = os.environ["GITHUB_SHA"]
ARTIFACT = Path("/tmp/main-to-main-import")
ARTIFACT.mkdir(parents=True, exist_ok=True)

# Explicit preserve set: these protections must remain byte-identical to Extended main.
PRESERVE = [
    ".github/workflows", ".npmrc", "AGENTS.md", "LICENSE",
    "SECURITY-HARDENING.md", "SECURITY-INSTALLATION-REVIEW.md",
    "HARDENED-REVISION.json", "EXTENDED-RELEASE.json",
    "package.json", "package-lock.json",
    "mobile/app.json", "mobile/app.config.ts",
    "mobile/package.json", "mobile/package-lock.json",
    "relay/package.json", "relay/package-lock.json",
    "src-tauri/Cargo.toml", "src-tauri/Cargo.lock",
    "src-tauri/tauri.conf.json", "src-tauri/capabilities",
    "src/app-updates.ts", "src/app-updates-policy.ts",
    "src-tauri/src/agent_updates.rs", "src-tauri/src/agent_updates_install.rs",
    "src-tauri/src/local_signing.rs", "src-tauri/src/secret_store.rs",
    "docs/VERSIONING.md", "docs/releasing.md", "scripts/release",
]
ADDITIONAL_PRESERVE_FROM_LAUNCHER = [
    "docs/integration/main-to-main/README.md",
    "scripts/integration/upstream_main_merge.py",
]
# Keep release workflows from the Extended source; never import upstream distribution triggers.
FORBIDDEN_UPDATER_MARKERS = [
    ("src-tauri/src/lib.rs", "tauri_plugin_updater::"),
    ("src-tauri/src/lib.rs", "tauri_plugin_process::"),
    ("src-tauri/src/lib.rs", "mod app_updates;"),
    ("src-tauri/src/lib.rs", "app_updates::app_update_install_supported"),
]

def run(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        list(args), text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False,
    )
    if check and result.returncode:
        raise RuntimeError(f"{' '.join(args[:3])} returned {result.returncode}: {result.stdout[:2000]}")
    return result

def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run("git", *args, check=check)

def rev(text: str) -> str:
    return git("rev-parse", text).stdout.strip()

def tree_file(ref: str, path: str) -> str | None:
    result = git("rev-parse", f"{ref}:{path}", check=False)
    return result.stdout.strip() if result.returncode == 0 else None

def restore(ref: str, path: str) -> None:
    # With --source, this also deletes a newly added file absent in the preferred source.
    out = git("restore", f"--source={ref}", "--staged", "--worktree", "--", path, check=False)
    if out.returncode:
        # Missing from source and index is harmless; any other error is fatal.
        present = git("ls-files", "--", path).stdout.strip()
        source_present = git("ls-tree", "-r", "--name-only", ref, "--", path).stdout.strip()
        if present or source_present:
            raise RuntimeError(f"Could not restore {path}: {out.stdout[:500]}")

if rev("HEAD") != ORIGINAL:
    raise RuntimeError("Launcher HEAD changed unexpectedly; refuse import.")
if git("status", "--porcelain").stdout.strip():
    raise RuntimeError("Dirty checkout before synthetic merge.")
if rev(f"{EXT}^{{commit}}") != EXT:
    raise RuntimeError("Extended main source is not present.")

git("config", "user.name", "github-actions[bot]")
git("config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com")
git("remote", "add", "fork", "https://github.com/ginmp8/pandacode.git")
git("-c", "protocol.version=2", "fetch", "--no-tags", "--depth=1", "fork", UPSTREAM, BASE)
for ref in [EXT, UPSTREAM, BASE]:
    if rev(f"{ref}^{{commit}}") != ref:
        raise RuntimeError(f"Pinned commit missing {ref}")

raw = git("merge-tree", "--write-tree", f"--merge-base={BASE}", "-X", "ours", EXT, UPSTREAM, check=False)
(ARTIFACT / "merge-tree.txt").write_text(raw.stdout)
head = raw.stdout.splitlines()
if not head or not re.fullmatch(r"[0-9a-f]{40,64}", head[0]):
    raise RuntimeError("Unable to obtain merge tree object, inspect artifact.")
merged_tree = head[0]
if git("cat-file", "-t", merged_tree).stdout.strip() != "tree":
    raise RuntimeError("Merge output is not a tree.")

stage_re = re.compile(r"^[0-7]{6} [0-9a-f]{40,64} [123]\t(.+)$")
conflicts = sorted({m.group(1) for line in head if (m := stage_re.match(line))})
(ARTIFACT / "ours-unresolved-conflicts.txt").write_text("\n".join(conflicts) + "\n")
# A scratch commit with only this integration branch as parent: no fabricated upstream Git ancestry.
scratch = git(
    "-c", "core.hooksPath=/dev/null", "commit-tree", merged_tree, "-p", ORIGINAL,
    "-m", "WIP synthetic merge snapshot (unreviewed, do not ship)"
).stdout.strip()
git("switch", "--detach", scratch)

for path in conflicts:
    if path.startswith("/") or path.startswith("../") or "\x00" in path:
        raise RuntimeError("Unsafe conflict path.")
    restore(EXT, path)
for path in PRESERVE:
    restore(EXT, path)
for path in ADDITIONAL_PRESERVE_FROM_LAUNCHER:
    restore(ORIGINAL, path)

# The privileged one-shot workflow does not persist in the imported candidate.
write_workflow = ".github/workflows/experimental-upstream-merge-import.yml"
git("rm", "-f", "--ignore-unmatch", "--", write_workflow)

# Guards intentionally fail closed. Do not execute fetched application code with write token.
for path in PRESERVE:
    # Compare every tracked path under the protected prefix, including new files.
    old = set(git("ls-tree", "-r", "--name-only", EXT, "--", path).stdout.splitlines())
    new = set(git("ls-files", "--", path).stdout.splitlines())
    if old != new:
        raise RuntimeError(f"Protected path set changed: {path}: added {sorted(new-old)[:8]}, removed {sorted(old-new)[:8]}")
    for file in old:
        before = tree_file(EXT, file)
        after = git("hash-object", "--", file).stdout.strip()
        if before != after:
            raise RuntimeError(f"Protected blob changed: {file}")
for path, marker in FORBIDDEN_UPDATER_MARKERS:
    if marker in Path(path).read_text():
        raise RuntimeError(f"Forbidden active updater marker in {path}: {marker}")

for extra_workflow in [
    ".github/workflows/release.yml",
    ".github/workflows/publish.yml",
]:
    if Path(extra_workflow).exists() and tree_file(EXT, extra_workflow) is None:
        raise RuntimeError(f"Unreviewed upstream publishing workflow: {extra_workflow}")

# Capture file-level coverage, NOT a proof of behavioral parity.
extended_changed = git("diff", "--name-only", EXT).stdout.splitlines()
upstream_remaining = git("diff", "--name-only", UPSTREAM).stdout.splitlines()
(ARTIFACT / "extended-candidate-diff.txt").write_text("\n".join(extended_changed) + "\n")
(ARTIFACT / "upstream-candidate-diff.txt").write_text("\n".join(upstream_remaining) + "\n")

summary = {
    "status": "WIP source reconciliation; NOT feature parity; NOT security cleared",
    "extended_main": EXT,
    "fork_upstream_main": UPSTREAM,
    "synthetic_ancestor": BASE,
    "new_branch": BRANCH,
    "source_merge_tree": merged_tree,
    "merge_tree_exit_status": raw.returncode,
    "unresolved_merge_paths_favored_extended": len(conflicts),
    "unresolved_merge_paths": conflicts,
    "protected_paths": PRESERVE,
    "paths_modified_from_extended_main": len(extended_changed),
    "paths_still_different_from_fork_main": len(upstream_remaining),
    "acceptance": {
        "frontend_build": "NOT_RUN",
        "native_rust": "NOT_RUN",
        "mobile_relay": "NOT_RUN",
        "security": "BLOCKED_BY_REVIEW",
        "end_to_end": "NOT_RUN",
    },
}
(ARTIFACT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

report = f"""# Initial synthetic main-to-main integration candidate — not approved

Extended main: `{EXT}`  
Fork main: `{UPSTREAM}`  
Original content ancestor: `{BASE}`

Source merge tree `{merged_tree}`. For the first automated code import, conflicting content favors Extended. **This does not prove the new upstream behavior is present in conflicting functions.**

- Paths modified from Extended main: **{len(extended_changed)}**
- Paths still different from fork main: **{len(upstream_remaining)}** (includes legitimate hardening, branding and exclusive features)
- Paths with unresolved Git conflicts explicitly retained from Extended: **{len(conflicts)}**.
- Protected paths were restored byte-for-byte from Extended main. Upstream-only workflow files were not imported.
- Full acceptance, functional parity and security validation: **NOT EXECUTED**.

The list of unresolved paths is in `docs/integration/main-to-main/unresolved-paths.txt`. A full assessment artifact is attached to the GitHub Actions importer run. The report is **a worklist**, not a resolution certificate.

## Next gates

1. Audit each auto-merged source path touching network access, credentials, remote execution, providers, agent tool grants, Tauri IPC, storage, cloud connections and mobile push.
2. Reconcile all retained Extended conflict paths against upstream source and tests. Keep both sets of user-facing features or document an explicit verified incompatibility.
3. Reconcile and lock new dependency requirements with no updater, unattended installer, untrusted `npx` bootstrap or non-consensual downloads.
4. Verify tests/builds under a **separate read-only CI token**, then execute native/physical remote safety checks.
5. Never merge to main or publish this candidate solely based on Git's merge result.
"""
Path("docs/integration/main-to-main").mkdir(parents=True, exist_ok=True)
Path("docs/integration/main-to-main/CANDIDATE.md").write_text(report)
Path("docs/integration/main-to-main/unresolved-paths.txt").write_text(
    "\n".join(conflicts) + "\n"
)

git("add", "-A")
if not git("diff", "--cached", "--quiet", check=False).returncode:
    raise RuntimeError("No source changes to stage; abort unexpected empty integration.")
git("config", "user.name", "github-actions[bot]")
git("config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com")
git("-c", "core.hooksPath=/dev/null", "commit", "-m",
    "feat(integration): stage main-to-main upstream source with Extended security precedence")
candidate = rev("HEAD")
# Avoid pushing if another actor already advanced the isolated branch.
remote = git("ls-remote", "origin", f"refs/heads/{BRANCH}").stdout.split()[0]
if remote != ORIGINAL:
    raise RuntimeError(f"Branch changed after assessment; expected {ORIGINAL}, got {remote}.")
pushed = git("push", "origin", f"{candidate}:refs/heads/{BRANCH}", check=False)
if pushed.returncode:
    raise RuntimeError(f"Push was rejected, leaving remote unchanged: {pushed.stdout[:1800]}")
summary["candidate_commit"] = candidate
summary["status"] = "PUSHED_AS_WIP_NOT_VERIFIED"
(ARTIFACT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps({key: value for key, value in summary.items() if key != "unresolved_merge_paths"}, indent=2))
