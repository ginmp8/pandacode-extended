# PandaCode 0.3.35-hardened.2 — Implementation and Verification

Date: 2026-10-24. Artifact: source code, not a signed native installer.

## Result

Implemented low-impact local improvements inspired by Paseo usage patterns: advisor and limited committee profiles; preflight checks; dependency/blocker inspector; attention center based on loaded state; handoff with preview, limits, and confirmation. The original executor remains the sole owner of agents and tasks.

**Delivery status:** implementation and focused checks passed; **native operational approval remains inconclusive/blocked by missing evidence**. No claim is made of comprehensive security, full parity with upstream Panda, Paseo equivalence, or CPU/RAM savings.

## Exact Base and Version

- Input: `pandacode-hardened-feature-hardening.zip`.
- SHA-256: `3e436d75508f2ee91f12dbca160faa56ca32f812aada5449f65e40ed7748171f`.
- Input-declared version: `0.3.35-hardened.1`; new review: `0.3.35-hardened.2`.
- The `0.35.0` reference was not used to renumber the product lineage.
- All 1,060 original files were preserved in the immutable base. Selective-backport history remains in `HARDENED-REVISION.json`.

## Minimal Change

Only **one original code file**, `src/main.ts`, received functional integration. Four runtime files were added under `src/hardened/`, totaling 35,894 source bytes, excluding the small adapter in `main.ts`. This is not a measurement of final bundle, binary, memory, or CPU.

Other changed original files: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json` (version only); `HARDENED-REVISION.json` (lineage/scope); `README.md` (review paragraph). Tests, optional development tools, documentation, patch, and evidence were added.

**Zero** new runtime dependencies, resident services, ports, IPC commands, database migrations, approval paths, or execution mechanisms. All Rust implementation files, preexisting tests, capabilities, CSP, and dependency graphs remained identical. Tauri configuration differs only in version.

The functional patch passed `git apply --check` against `src/main.ts` from the original PandaCode 0.3.35 ZIP. This is textual evidence; it is not a build or behavioral compatibility proof and does not guarantee future merges.

## Features and Boundaries

| Feature | Implemented behavior | Deliberate limit |
|---|---|---|
| Advisor | Profile with one reviewer and explicit criterion | Preparation restricted to Codex plan; manual send |
| Committee | Two reviewers, two assignments, two coordinator turns | No debate to convergence or unlimited repetition |
| Preflight | Re-reads task, history, and team; validates model, mode, and budget | Does not replace native authorization at execution time |
| Inspector | Local dependencies, scopes, states, and reasons | Native scheduler remains authoritative; observation limits are shown |
| Attention | Loaded pending items, failures, and blockers; deduplicated navigation | Does not approve, respond, retry, or resume |
| Handoff | Selected fields, optional checkpoints, preview, and explicit copy | Best-effort secret redaction; no full transcript |

Saving a profile uses the existing library without applying the team or changing defaults. Later model-picker selection retains Panda's preexisting default behavior. Inserting a prompt only fills an empty composer: it does not call `send_message`, `architect_start`, `queue_add`, `architect_configure`, or approval commands. Existing attachments/drafts cause refusal, not replacement.

Refresh queries were separated from `reloadSessions`: the original helper also schedules automatic resumes. The new panel does not call it. Closing the panel removes handlers and blocks late insertions. Names, scopes, and messages are rendered as text.

## Checks Run

| Check | Baseline | Candidate | Interpretation |
|---|---:|---:|---|
| Full Node suite, same final commands/environment | 134 passed / 4 failed | 162 passed / 4 failed | 28 new cases passed; same four missing prerequisites |
| Focused original cases | 37/37 | Preserved | No original assertions/fixtures changed |
| Real panel browser tests with synthetic IPC | Not present | 15/15 | DOM, actions, keyboard, disposal, privacy, and layout |
| Full TypeScript, tool 5.8.3 | 63 diagnostics | Same 63 byte-for-byte | Dependencies missing; full compile not approved |
| Syntax analysis of five selected modules | — | Zero errors | Not full typecheck or Vite build |
| Secret scanner, three new TS files and `main.ts` | — | Zero candidates; JSON validated | Bounded scope; not full/historical audit |
| Diff/dependency invariants | — | Passed | Backend, controls, and original tests preserved |
| Patch on original 0.3.35 | — | Textually passed | Upstream not executed |

The four shared failures are three bundle tests requiring `dist/index.html` and one test file unable to import `material-icon-theme`. No test was disabled or relaxed to obtain these results. Dependencies were not installed to hide blockers.

Available TypeScript is 5.8.3 while the project declares ^5.9.3. Differential results do not replace checking with the declared toolchain. No Vite build, complete original Playwright suite, `test:release`, Rust/Tauri build, real provider test, or benchmark was run. `cargo`/`rustc` were unavailable.

New tests were written and observed failing before the corresponding implementation. Additional regressions caught and fixed stale handoff confirmation, request changes during async wait, and exposure of a truncated key block. The navigation fixture fix only omitted an optional `undefined` field that Python transport converted to `None`; no approval criterion was relaxed.

## Isolation and Meaning of Evidence

Node tests and typecheck ran in a Linux namespace with external networking unavailable, loopback isolated for local MCP tests, clean environment, read-only source/toolchain, temporary writable area, and CPU/memory/process/file/time limits. `containment.json` records executed probes. No real credentials or model APIs were accessed.

The browser ran the real new modules in in-memory HTML, with network requests aborted and IPC replaced at the boundary. Chromium required `--no-sandbox` in the disposable container; this is explicit in the test tool and does not change the product. These scenarios do not prove WebView/Tauri behavior, a real provider, or native process isolation.

## Bounded Security Review

- No code/credential transport, remote plugin, or IPC permission was added.
- Worker IDs are checked against the parent task before navigation; no new approval handler exists.
- Preflight requires Codex plan and a fresh context. `readOnly: true` in assignment remains advisory; the real boundary depends on the existing provider/native executor. Effectiveness was not demonstrated with real write attempts in this session.
- Handoff limits fields/size, collects no credentials, and requires confirmation of current text. Redaction does not cover every secret format; external software may persist clipboard contents.
- No current CVE audit, full secret audit, independent security review, or proof across all platforms was performed. Earlier reports remain historical.

**Assurance gate:** `blocked-critical-evidence` for native operational approval because build and validation of real boundaries are missing. This describes an evidence gap, not a confirmed vulnerability.

## Reproduction and Maintenance

`docs/HARDENED-REVIEW-TOOLS.md` documents usage, local commands, optional tools, and rollback without deleting data. `patches/paseo-review-tools.patch` isolates the functional extension. `source-manifest.json` pins delivered bytes outside this evidence folder; `delta-receipt.json` identifies the base and changes. The external checksum identifies the full ZIP.

With declared dependencies/toolchains, frontend build, original tests, native tests, and real advisor/committee scenarios—including write denial—still need to pass. A clean textual merge alone is not a release criterion.
