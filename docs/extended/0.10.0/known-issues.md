# PandaCode Extended 0.10.0 — unresolved verification register

**Status:** Candidate evidence gaps, **not** confirmation that each item is a product defect.
**Opening reference:** [Extended `e8122223`](https://github.com/ginmp8/pandacode-extended/commit/e81222232314844d8fc52f65cb8e2dd37c5da2c8); upstream [source `fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852) (0.3.97).

| Area | Classification at cycle opening | Required verification for 0.10.0 |
| --- | --- | --- |
| Shared upstream/Extended differences | **PARTIALLY VERIFIED (2026-11-05):** complete, nontruncated Git trees at upstream `fc32c3f585db6a72fee967e8d91f2bb50fe67852` and Extended `abe19dbde19bb2f44b3d61bdf8668941425725e0` show 1,501 upstream blobs: 1,436 exact, 65 shared-path differences, zero missing; 40 Extended-only blobs. Per-hunk and runtime parity are not certified | Recompute complete inventory at final source/destination SHAs and justify all retained product hunks |
| Settings and remote capabilities | **UNVERIFIED:** native On/Off behaviors for Connect, bots, MCP, direct routes, cloud/VM, credential transfer and updates | Exercise enabled, disabled, manual, automatic, restart, existing-process and approval paths |
| Desktop, model and UI behavior | **UNVERIFIED:** upstream 0.3.95–0.3.97 conversation folding, window restoration, IME, macOS drag/drop, Cline MCP and model configuration in installed apps | Native platform checks, provider compatibility and focused regressions |
| Mobile and simulators | **UNVERIFIED:** iOS transcript fix, Linux Android emulator availability/boot and actual device behavior | Exercise relevant devices and simulator startup |
| Upgrade/data compatibility | **UNVERIFIED:** existing SQLite data and real installer upgrades were not certified by historical CI | Restore a representative prior database, test migration/upgrade and recovery |
| Release identity and publication | **BLOCKED:** source Tauri version and `releases/{version}.md` are used by the current release workflow, not the independent Extended label | Implement/validate a distinct 0.10.0 distribution identity and release-note mapping without breaking source-version parity |
| Dependencies and security | **PENDING:** previous-cycle npm/RustSec success does not apply to the final 0.10.0 SHA | Re-run audits and review actual dependency/override deltas |
| Rust ZIP advisory GHSA-m665-v7pm-7grj | **TARGETED PATCH, CI PASS AT `dcef242c`:** The direct `zip` pin and Cargo.lock entry were changed from 9.0.4 to 9.0.5 (minimum patched release). The VSIX import path uses `ZipArchive`, not a confirmed streaming extraction path; no full-app exploit claim | Exact-SHA [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/41969392506) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/41969398025) passed all 12 jobs; focused VSIX and installed-platform acceptance remain unverified; monitor `main` separately |

No upstream capability should be removed on the basis of unverified concerns. Confirm actual defects with source evidence and focused regression tests, and preserve Extended's existing security/permission controls.

Reference: [0.9.0 historical register](../0.9.0/known-issues.md), [0.10.0 dependency evidence](dependency-security.md), and [release checklist](release-checklist.md).

## 2026-11-05 screenshot-documentation verification gap

[Upstream `78f77a0`](https://github.com/ginmp8/pandacode/commit/78f77a0becee2029f4b89ef60fca0c97e48251e2) updates the screenshot capture suite and visual evidence. The source changes were imported by exact blob identity, but the interactive Playwright screenshot recapture and native On/Off integrations were not executed in the restricted local environment. This is an **unverified test gap**, not a demonstrated product defect or a justification for disabling any upstream feature.
