# PandaCode 0.3.35.4 / hardened.4 — bounded improvement & installation-security assessment

**Date:** 2026-10-24. **Base:** `pandacode-hardened-0.3.35.3.zip`, SHA-256 `a973fc6f7d4f045e3861146ec2ad073cec34bc798b81dfa57c4cd1f0439fb041`. **Output:** full source archive, not a signed Windows installer. **Primary improvement mode:** feature; secondary: security gate, regression hardening. **Decision:** keep the bounded manual-profile improvements as a source candidate. **Native isolated review: blocked** until separately proven. **Installation verdict: inconclusive** pending native/OS checks; do not treat source tests as install safety.

## Actual change

- In **Review & coordination → Review presets**, Coordinator provider/model and one or two reviewer provider/model selectors read from installed provider/model catalogs. Advisor and Committee remain bounded to one/two workers, two lead turns, one assignment per worker, with `rosterOnly` and no automatic configuration. Existing Codex/manual profiles remain possible. A selected provider's model is not silently substituted.
- Explicit **Isolated review — native security gate** in the same panel. It queries the new **read-only** `review_readiness` command in `src-tauri/src/review.rs`. The response is always `ready=false`, `status=containment_unproven`, `provenProviders=[]`. No new CLI, MCP server, new network listener or checkout modification occurs. The interface has **no isolated start button**. This is deliberately **not** a functioning isolated Review & Coordination execution engine.
- Manual review text explicitly warns **NOT isolated**; the original Send action remains the only dispatch, with existing task/provider approvals. Model-facing `readOnly`/plan instructions are not represented as an OS jail.
- Immediately before inserting the manual prompt into the existing composer, `src/main.ts` independently re-reads task, team and message history. A task/provider/model mismatch, stale team, prior conversation, missing model, active task or attachments is rejected. The non-atomic gap between composer insertion and subsequent manual Send remains an ordinary app behavior, not a security proof.
- Offline browser fixture compiler now includes the workflow module graph. Existing test files were not weakened or removed; 4 Node tests and 3 dedicated browser cases cover role validation and UI integration.

No dependency version, application updater, signing process, database schema, selected remote host feature, OS permission model, provider protocol or browser tool was changed. No Paseo runtime, daemon or source was imported. Base is still a selectively backported upstream 0.3.28 fork, **not** feature parity with upstream 0.3.35 or Paseo.

## Changed source surface

Existing: `src/hardened/review-model.ts`, `src/hardened/review-panel.ts`, `src/hardened/review-tools.css`, `src/main.ts`, `src-tauri/src/lib.rs`, and fixture `scripts/hardened-review/compile-ui.mjs`. New: `src-tauri/src/review.rs`, `tests/hardened-review-0354.test.mjs`, `scripts/hardened-review/browser-0354.py`. Metadata, README, documentation, release notes, evidence and the feature-only patch were also updated/added. `git apply --check` passed against the **exact** 0.3.35.3 extracted base; it is a textual check only. Every previous source file remains in the package; input archive is preserved unchanged.

## Executed verification

| Gate | Evidence | Outcome |
|---|---|---|
| Source intake | SHA-256 above; ZIP metadata examined, bounded extraction; no path traversal, symlink or case collision in the base. | PASS for intake, **not** install verification |
| Isolated test preflight | Linux user/network namespaces and Node read permissions used; attempts to read `/etc/passwd`, write outside the allowed scratch, or start a child process returned `ERR_ACCESS_DENIED`; no external network access to tests. | PASS for these bounded controls; not the PandaCode native CLI jail |
| Existing frontend review tests | `tests/hardened-review.test.mjs`, Node 22.16, `--experimental-strip-types --permission`, network/user/mount/PID namespace, source-read only. | **23/23 PASS** |
| New model/security tests | `tests/hardened-review-0354.test.mjs` under same restriction. | **4/4 PASS** |
| TypeScript full project | Trusted `tsc --noEmit --pretty false` on exact baseline and candidate. | Both fail identically, **63 diagnostics each**, due to unavailable dependency modules and related type fallout. **No new diagnostic**; **not a full build** |
| Scoped TypeScript syntax transpilation | Trusted TS 5.8.3 transformer, 8 reviewed modules; no package hooks or project build. | **0 syntax errors**, not type/runtime proof |
| Existing Chromium UI fixture | `browser-checks.py` (13 tests), locally installed Chromium, synthetic native IPC, Playwright in non-root network-isolated Linux user namespace, no `--no-sandbox`. | **13/13 PASS** |
| Extended Chromium regression fixture | `browser-regressions.py` (original 13 plus 2 additional tests). | **15/15 PASS** |
| New Chromium multi-provider/gate fixture | `browser-0354.py` (original 13 plus 3 new tests). | **16/16 PASS** |
| Patch textual reapplication | `git apply --check patches/review-multi-harness-hardened.4.patch` against exact hardened.3 source. | PASS; no future merge guarantee |

These browser runs reuse the same 13 base cases, so they represent **18 distinct browser cases**, not 44 independent ones. Browser tests use in-memory HTML and mock providers; they do not execute Tauri, real providers, local HTTP integrations or any sandboxed review agent. Node test files were executed only with read-only source access and restricted environment; no uploaded PandaCode executable, Cargo build or installer was run.

**Not run / blocked:** native Rust compilation and native `review_readiness()` Rust unit test (cargo/rustc not installed); complete Vite frontend build and `npm run test:release` (declared npm dependencies unavailable); full original Node suite; original complete UI suite; native Windows, macOS and Linux installed-app/security checks; provider/CLI launch; real inference egress; actual OS sandbox escape trials for an agent process tree; native snapshot/MCP/patch/approval lifecycle. Hence no claim of production-ready isolated review, whole-repo security assurance, successful Windows install, or performance improvement.

## Trust-boundary review matrix (13 surfaces)

| ID | Coverage | Current observation / missing proof |
|---|---|---|
| S01 source/release identity | partial | Exact base/candidate source and ZIP inventory; no signed executable provenance |
| S02 build/dependency execution | partial | No changed dependencies, but Cargo/npm build steps and transitive dependencies not executed/audited in an isolated native build |
| S03 install/updates/persistence | partial | No updater change; installed native behavior untested |
| S04 agent prompt/tool control | partial | Manual role limits verified through models/fixtures; no native containment or model compliance proof |
| S05 content/egress | partial | Inference goes to user-selected provider on manual Send; arbitrary CLI egress not denied/proven |
| S06 credentials | partial | No new credential flow; inherited credentials from existing CLI host not audited end-to-end |
| S07 remote/relay/bots | not-run | Existing product areas untouched, not revalidated |
| S08 browser/webview/IPC | partial | New command in trusted main handler, not launcher/tray; fixture checks; installed WebView unavailable |
| S09 local MCP/API | partial | No dedicated review MCP or execution-scoped token; previous Architect MCP left unchanged |
| S10 jobs/state/lifecycle | partial | Existing scheduler retained; no executable review lifecycle or auto-fix state machine |
| S11 OS paths/network | partial | UI/runtime adapter reviewed; native child process isolation not proven, readiness reports blocked |
| S12 AV/OS install evidence | not-run | No Windows native binary, AV/process trace or isolation tests |
| S13 release/evidence | partial | Tests and source hashes recorded; no signed installer, native release or full deterministic build |

**Finding RC-01, `needs_validation`:** plan/read-only modes and kill-on-close process groups/job objects do not independently establish a jail. The UI explicitly disclaims isolation; the native readiness check reports `containment_unproven`. There is no proven unauthorized escape incident in the supplied evidence, but **isolated review must remain disabled** until filesystem/network/credential confinement and provider transport are proven on each target OS. No severity is assigned to an unvalidated exploit hypothesis.

**Residual risks:** manual Send invokes the regular user-selected provider and its own CLI and existing application privileges. Model compliance with read-only prompts, network destinations, credential inheritance, third-party provider data policies, native security controls and installed-app path are not established by these source/UI tests. No automatic patch apply, worktree merging, remote runtime, telemetry, updater or new dependency was introduced.

## Next mandatory gate for true isolated review

Implement and independently test the OS-specific sandbox around the selected provider process **and descendants**. Deny writes outside stable snapshot, unauthorized network/loopback, cross-provider credentials and foreign MCP tool capabilities, while preserving provider inference via explicitly authorized endpoints. Only after those tests pass may a separate `pandacode_review` MCP, user-confirmed execution start, safe snapshot, token/role lifecycle, proposal hashing and approval-bound patch application be implemented and exposed. Follow `docs/HARDENED-REVIEW-TOOLS-PLAN.md`; never fall back to prompt-based isolation. On unsupported OS/provider, fail closed.

**Release policy:** source candidate 0.3.35.4 (0.3.35-hardened.4), no upstream merge, Git commit, tag, deployment or binary packaging. Restore to exact original 0.3.35.3 ZIP if the candidate is rejected; this run changed only the isolated copy. Use the feature-only patch to review scope, not as a substitute for all historical security changes.
