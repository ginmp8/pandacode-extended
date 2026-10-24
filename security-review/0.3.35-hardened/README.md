# PandaCode 0.3.35 hardened — Backport and Security Review

**Review date:** 2026-10-24
**Product:** `0.3.35-hardened.1` / source package `pandacode-0.3.35-hardened.zip`
**Evidence status:** `STATIC_BACKPORT_VERIFIED` for the backport; **`review_status=blocked-critical-evidence`** for operational release because runtime behavior and authorization were not proven.
**Base:** `pandacode-hardened-0.3.34.1.zip` — SHA-256 `094d2e05af75c488c346205dacf08fae6e898ef04f9221d2701e701636819103`.
**Comparison:** `pandacode-0.3.35.zip` — SHA-256 `fc668a66394a3390f78b44a2cd9187b027f383ad1492ddf9653dd00c3f2b1cdd`.

## Scope Decision

PandaCode 0.3.34.1 hardened already has a **native menu** with tasks and agent usage. The focused innovation documented in upstream 0.3.35 is the **visual tray/menu bar panel**. It was adopted as a limited backport; **there is no general functional equivalence with original 0.3.35**.

**Implemented in source:**

- Panel under the icon with `running`, `waiting`, and completed tasks (including success/failure distinction), agent/project association, task access, arrow-key and Enter navigation.
- Visual plan-limit bar with percentage, 80%/95% state, reset time, stale-cache information, and refresh only when opened or explicitly requested by the user.
- Dedicated `tray` webview with minimal event capability; Rust IPC has an explicit list of seven `tray_*` commands, separate from workspace/launcher commands. Navigation is allowed only inside the app with `view=tray`.
- Additional adaptation safeguards: at most 1,024 unseen completed tasks; 32 provider queries per panel refresh; one usage-query round at a time; reject non-finite CSS percentages; retain upstream panel-height limit/validation.
- Upstream preview mock and Playwright panel test, but no navigation tests were run in the current sandbox.

**Intentionally excluded:**

- Cloud agents (`cloud_agents`), remote-machine tool setup, code/token/credential upload, and automatic sync. These would change trust boundaries, require new authorization/telemetry, and need OS/network testing.
- New cloud/VM providers, deploy scripts, plaintext macOS secret storage, expanded permissions, and remote installation flows. Hardened version continues to use the preexisting protected key store.
- App/agent updater, process/restart permissions, and remote `npx` installer. The prior hardened manual-update policy remains unchanged.
- Unneeded dependency changes, release scripts, upstream locks, broad changes to bots, HTTP/relay, Cloudflare, Connect, workspace files, or MCP tools.

**Important:** inherited code already has security controls and compensating measures that are not revalidated merely because their bytes remain identical. Earlier records (`security-review/0.3.34.1/`) retain their pending-execution caveats.

## Evidence Collected

| Check | Result | Limit |
|---|---|---|
| ZIP inventory, paths, links, duplicates | Passed: both inputs safe for controlled extraction (1,141 and 934 entries) | Does not attest to absence of malicious code |
| Frozen baseline with hashes for both ZIPs, 21 critical files, and fixed scope | Recorded in `frozen-evaluation.json` | Hash proves identity, not behavior |
| Static regression of protected controls, IPC commands, versions, and lockfiles | Passed, `static-verification.json`; reproducible `verify-source.py` | Syntax/structure inspection, no runtime |
| TypeScript parser on `src/tray-popover.ts`, `src/entry.ts`, `src/preview.ts`, `tests/tray-popover.spec.ts` | **4/4 syntax-clean** | Parser, not full typechecker |
| `node --check` on preserved npx installer | Passed | Installer not executed |
| `tsc --noEmit` on baseline/candidate | **Blocked by missing dependencies:** 62 baseline diagnostics, 63 candidate. Only new diagnostic: declared `@tauri-apps/api/event` module unavailable in environment. | Neither approved compilation nor a candidate-specific failure |
| Verifier negative control (byte-level tamper in disposable copy of critical file) | Failed as expected on scope/SHA-256; `tamper-probe.json` | Shows checker sensitivity, not runtime security |
| `cargo check --locked`, `cargo test --locked` | Not run: Cargo/Rust unavailable | Rust syntax/compilation unverified |
| Node suite, Playwright, Tauri app | Not run: dependencies/browser unavailable and full isolation for untrusted code unverified | Real UX, tray events, native integration unverified |
| Current SCA, installation, signing, Windows/macOS/Linux | Not run | Does not certify product or distribution security |

Network namespace separation and read-only mount for source were observed; the available sandbox failed to guarantee **the entire filesystem root was read-only**. Therefore, project-controlled scripts and app tests with potential host access were not run. This is a validation limitation, not a product failure.

## Risk and Governance Review (SGR-2.0)

| Item | Classification | Static evidence | Risk and decision |
|---|---|---|---|
| R01 — New panel IPC | `needs-verification` / high / medium confidence | `lib.rs` routes `tray` before `main`; `capabilities/tray.json` enables events only; `tray.rs` restricts navigation. | Added surface appears limited; runtime-test that tray window cannot invoke workspace/CLI/file calls. Do not claim isolation until tested. |
| R02 — Title/percentage rendering | `needs-verification` / medium / medium confidence | `tray-popover.ts` escapes dynamic data, uses local brand URLs and finite percentages. | Test malformed HTML/attributes and extreme values at runtime with real CSP. No direct unescaped sink found for new fields. |
| R03 — Usage queries | `needs-verification` / low / high code confidence | Uses existing `query_usage`; triggered on panel open or user refresh; concurrent-request limiter. | Queries may depend on CLI/provider and do not guarantee no traffic; document/test consent, caching, and limits. |
| R04 — Upstream cloud agents | `not-applicable` / informational / high confidence | Transport/sync/credential files were not imported; `mod cloud_agents` absent. | Avoids a new code/credential transfer flow; not an audit of existing cloud integrations. |
| R05 — Historical F01–F08 hardening | `needs-verification` / high (validation gate) / high confidence that tests are absent | Hashes match across key boundaries: bots, MCP/deploy, Cloudflare, installation, HTTP relay. | No textual regression in protected files, but 0.3.34.1 runtime/test gaps remain open. |
| R06 — Supply chain/updaters | `governance-risk` / medium / high confidence in evidence gap | Pinned dependencies/versions preserved; no new updater/installer enabled. | No current SCA, signed binary, or installer tests; insufficient for broad operational distribution approval. |

No credential authentication, agent-tool execution, package installation, cloud-provider access, file upload to APIs, or invasive tests were attempted. Static string checks on **new code files** found no common secret patterns. This does **not** mean the entire distribution and its history are free of secrets.

## Authority Matrix for the New Webview

| Action from `tray` webview | Scope | Authorization source | Effect/failure behavior |
|---|---|---|---|
| `tray_snapshot` | Read visible tasks, project names, cached percentages | Local `tray` window in dedicated capability and Rust allowlist | Storage error returns failure; does not execute tasks |
| `tray_refresh_usage` | Query limits for up to 32 configured CLI/provider IDs | User clicks Refresh or opens panel | No installation bypass; querying an existing account must be validated at runtime |
| `tray_open`, `tray_show_app`, `tray_hide`, `tray_resize` | Local navigation/presentation | Clicks/keyboard in restricted window | Handler does not execute CLI/workspace/deploy |
| `tray_quit` | Local process exit | UI or native-menu Quit | Does not roll back preexisting remote tasks; existing product limitation |
| General commands (files, secrets, agent tools, bots, MCP, deploy) | **Not permitted to `tray` webview** | Enforced by source-code label routing | Effective denial requires native IPC test |

The new panel has no workspace permissions in `capabilities/default.json` and creates no remote execution. Command separation in `lib.rs` and explicit refresh queries must be checked in the real application, not by inspection alone.

## Remaining Criteria Before Operational Use

1. In an isolated environment with Rust and dependencies pre-provisioned, run `cargo check --locked --offline` and `cargo test --locked --offline` in `src-tauri/`; verify `tray_position_centers_under_icon_and_clamps` and historical security regressions.
2. With `node_modules` generated from the lockfile without unapproved install scripts, run `./node_modules/.bin/tsc --noEmit`, Node suite, and `tests/tray-popover.spec.ts` using an approved local browser.
3. Test all three platforms: left/right click, transparent-window open/close/focus, layout, keyboard, task completion/failure, refresh, navigation URLs, network/provider errors, and app close.
4. Attempt high-privilege commands (run, files, bot, MCP, cloud, terminal) from `tray` webview and prove denial; verify `tray_snapshot` returns only expected data and other webviews do not inherit panel permissions.
5. Check integrations/secrets and CVEs against actually pinned versions, cancellation, HTTP/body limits, bot approvals, and package origin; address risks still open in 0.3.34.1.
6. Verify final binary, installer reputation/signature, and integrity on target machines. Then conduct independent review tied to ZIP and test hashes.

**Verdict:** source backport delivered and **approved on static criteria only**; `review_status=blocked-critical-evidence` for operational execution/distribution because dynamic evidence is absent. This is not a certificate of vulnerability-free status or complete upstream compatibility.

## Audit Artifacts

- `changes.json` — exact changed/added file list; `changes.patch` — contextual backport diff, excluding recursive evidence tree.
- `frozen-evaluation.json` — scope, inputs, and protected hashes frozen before implementation.
- `verify-source.py` — textual/hash invariant checker; `static-verification.json` — execution receipt.
- `source-manifest.json` — SHA-256 for each ZIP file except the manifest itself.

These files document a source candidate. Old logs in the tree do not turn historical tests into current tests.
