#!/usr/bin/env python3
"""One-time source-only three-way reconcile for pinned PandaCode fork main.

This script executes only git diff/show/merge-file commands and writes candidate
source paths plus a JSON audit. It does not run upstream scripts or install code.
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
EXTENDED = "f0105e7db85a0d7bc2e70726716fcfa939fac182"
FORK = "0336a2fb5e3abaea35c095fcc63805652f49ea98"
ANCHOR = "15541a51f3f5d8ee32d91349f9d5035a53c4cd19"
OUT = ROOT / "docs/upstream-main-merge-status.json"

PROTECTED_PREFIXES = (
    "src-tauri/", "mobile/src/connect/", "relay/src/",
    "src/remote-", "src/preview-", "src/bots", "src/cloud-",
    "src/provider-", "src/agent-updates", "src/bridge",
)
PROTECTED_EXACT = {
    ".github/workflows/ci.yml", "AGENTS.md", "README.md",
    "package.json", "package-lock.json", "mobile/package-lock.json",
    "mobile/package.json", "playwright.config.ts", "playwright.webkit.config.ts",
    "vite.config.ts", "src/main.ts", "src/settings.ts", "src/bots.ts",
    "src/preview.ts", "src/skills-library.ts", "src/remote-settings.ts",
    "src/task-board.ts", "src/styles.css", "src/sidebar-tree.css",
    "src/model-picker.ts", "src/approval-modes.ts",
}
ALLOWED_ROOTS = ("src/", "mobile/src/", "mobile/tests/", "relay/test/",
                 "tests/", "website/src/", "docs/")


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(args, cwd=ROOT, check=check, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE)


def names(left: str, right: str) -> set[str]:
    return {part.decode("utf-8") for part in
            run("git", "diff", "--name-only", "-z", left, right).stdout.split(b"\0")
            if part}


def blob(ref: str, path: str) -> bytes:
    return run("git", "show", f"{ref}:{path}").stdout


def main() -> None:
    for ref in (EXTENDED, FORK, ANCHOR):
        value = run("git", "cat-file", "-t", ref).stdout.strip()
        if value != b"commit":
            raise RuntimeError(f"{ref} is not a commit")
    both = sorted(names(ANCHOR, EXTENDED) & names(ANCHOR, FORK))
    # Exclude files that were added or deleted on either side.
    both = [path for path in both if
            run("git", "cat-file", "-e", f"{EXTENDED}:{path}", check=False).returncode == 0
            and run("git", "cat-file", "-e", f"{FORK}:{path}", check=False).returncode == 0
            and run("git", "cat-file", "-e", f"{ANCHOR}:{path}", check=False).returncode == 0]
    outcome: dict[str, list[str]] = {
        "merged": [], "identical": [], "protected": [],
        "conflicting": [], "non_text": [], "unsupported": []
    }
    for path in both:
        dest = (ROOT / path).resolve()
        if not dest.is_relative_to(ROOT) or not dest.is_file() or dest.is_symlink():
            outcome["unsupported"].append(path)
            continue
        if path in PROTECTED_EXACT or path.startswith(PROTECTED_PREFIXES):
            outcome["protected"].append(path)
            continue
        if not path.startswith(ALLOWED_ROOTS):
            outcome["unsupported"].append(path)
            continue
        original = dest.read_bytes()
        base = blob(ANCHOR, path)
        upstream = blob(FORK, path)
        if b"\0" in (base + original + upstream):
            outcome["non_text"].append(path)
            continue
        if original == upstream:
            outcome["identical"].append(path)
            continue
        tmp = []
        try:
            for name, data in (("current", original), ("base", base), ("upstream", upstream)):
                handle = tempfile.NamedTemporaryFile(prefix=f"fork-{name}-",
                                                     delete=False)
                with handle:
                    handle.write(data)
                tmp.append(handle.name)
            result = run("git", "merge-file", "-p", "--diff3", tmp[0], tmp[1],
                         tmp[2], check=False)
        finally:
            for file in tmp:
                pathlib.Path(file).unlink(missing_ok=True)
        if result.returncode != 0 or b"<<<<<<<" in result.stdout:
            outcome["conflicting"].append(path)
            continue
        if result.stdout == original:
            outcome["identical"].append(path)
            continue
        mode = dest.stat().st_mode & 0o777
        with tempfile.NamedTemporaryFile(dir=dest.parent, prefix=".fork-merge-",
                                         delete=False) as handle:
            handle.write(result.stdout)
            work = pathlib.Path(handle.name)
        try:
            work.chmod(mode)
            os.replace(work, dest)
        finally:
            work.unlink(missing_ok=True)
        outcome["merged"].append(path)
    result = {
        "source": f"ginmp8/pandacode@{FORK}",
        "target": f"ginmp8/pandacode-extended@{EXTENDED}",
        "anchor_not_git_ancestor": ANCHOR,
        "automated_review_is_not_security_clearance": True,
        "protected_contracts_require_manual_review": True,
        "counts": {k: len(v) for k, v in outcome.items()},
        "paths": outcome,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print("AUTO_MERGE_COUNTS " + json.dumps(result["counts"], sort_keys=True))
    print("CANDIDATE_SHA256 " + hashlib.sha256(OUT.read_bytes()).hexdigest())
    for kind in ("conflicting", "protected", "unsupported"):
        print(f"{kind.upper()}: " + ", ".join(outcome[kind]))


if __name__ == "__main__":
    main()
