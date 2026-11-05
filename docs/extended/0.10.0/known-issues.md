# PandaCode Extended 0.10.0 — unresolved verification register

**Status:** Candidate evidence gaps, **not** confirmation that each item is a product defect.
**Opening reference:** [Extended `e8122223`](https://github.com/ginmp8/pandacode-extended/commit/e81222232314844d8fc52f65cb8e2dd37c5da2c8); upstream [source `fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852) (0.3.97).

| Area | Classification at cycle opening | Required verification for 0.10.0 |
| --- | --- | --- |
| Shared upstream/Extended differences | **UNVERIFIED:** the previous 0.9.0 source inventory reported 65 divergent shared paths and zero missing upstream paths at its specific SHA; no fresh 0.10.0 full-tree comparison yet | Recompute complete inventory at final source/destination SHAs and justify all retained product hunks |
| Settings and remote capabilities | **UNVERIFIED:** native On/Off behaviors for Connect, bots, MCP, direct routes, cloud/VM, credential transfer and updates | Exercise enabled, disabled, manual, automatic, restart, existing-process and approval paths |
| Desktop, model and UI behavior | **UNVERIFIED:** upstream 0.3.95–0.3.97 conversation folding, window restoration, IME, macOS drag/drop, Cline MCP and model configuration in installed apps | Native platform checks, provider compatibility and focused regressions |
| Mobile and simulators | **UNVERIFIED:** iOS transcript fix, Linux Android emulator availability/boot and actual device behavior | Exercise relevant devices and simulator startup |
| Upgrade/data compatibility | **UNVERIFIED:** existing SQLite data and real installer upgrades were not certified by historical CI | Restore a representative prior database, test migration/upgrade and recovery |
| Release identity and publication | **BLOCKED:** source Tauri version and `releases/{version}.md` are used by the current release workflow, not the independent Extended label | Implement/validate a distinct 0.10.0 distribution identity and release-note mapping without breaking source-version parity |
| Dependencies and security | **PENDING:** previous-cycle npm/RustSec success does not apply to the final 0.10.0 SHA | Re-run audits and review actual dependency/override deltas |

No upstream capability should be removed on the basis of unverified concerns. Confirm actual defects with source evidence and focused regression tests, and preserve Extended's existing security/permission controls.

Reference: [0.9.0 historical register](../0.9.0/known-issues.md), [0.10.0 dependency evidence](dependency-security.md), and [release checklist](release-checklist.md).
