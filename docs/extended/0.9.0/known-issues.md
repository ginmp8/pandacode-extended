# PandaCode Extended 0.9.0 — unresolved verification register

**Status:** evidence gaps and inherited concerns, not a claim that each is a confirmed product defect.

The [0.8.0 register](../0.8.0/known-issues.md) is historical input; its observations must be reproduced or resolved on the final 0.9.0 candidate rather than copied as confirmed live failures.

| Area | 0.9.0 evidence classification | Required next verification |
| --- | --- | --- |
| Upstream/Extended shared-path divergence | **UNRESOLVED:** 65 divergent shared blobs at source `5ab09c5a` vs Extended `e069ba4b`; zero upstream paths missing | Validate each relevant hunk and its callers/tests; classify all permitted differences and fix disproven parity |
| Connect, direct routes, bots, MCP, cloud/VM and credentials | **UNVERIFIED:** inherited controls exist in code/documentation, not native On/Off certification | Test allowed and blocked paths, restarts, approvals and existing processes |
| Subagents/teams and 0.3.94 response cards | **UNVERIFIED:** upstream 0.3.93/0.3.94 inherited behavior | Test nested completions, recovery, worker-card layout and remote resume in real clients |
| Compatibility and data upgrades | **UNVERIFIED:** green CI does not exercise representative existing user databases/installers | Test SQLite migrations, legacy settings and native upgrade paths |
| Dependency security | **CI-CHECKED, NOT FULLY CERTIFIED:** npm/RustSec jobs green at Extended `e069ba4b` | Review advisories/overrides and platform-specific integrations |

No deletion of upstream functionality or new safety default is justified by these gaps alone. Any confirmed defect must be addressed narrowly, with a regression test, while preserving upstream features. See the [parity contract](../upstream/parity-contract.md) and [release checklist](release-checklist.md).

## Updated evidence — 2026-11-04, upstream 0.3.97

Source [`fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852) and staged Extended tree `9f6f662464b94eb5b6798812d64f9c9c9eb713a6`: 1,501 upstream paths, 1,436 exact, 65 divergent shared, 0 missing, 34 Extended-only. Historical 0.3.94 rows above reflect an earlier comparison; this snapshot supersedes their counts. The 65 divergent paths still need full per-hunk certification; functional parity is **unverified**, not disproven. Prior CI success does not cover the new source changes. In particular, installed-window close/reopen, IME composition, macOS drag/drop, Cline MCP with Extended's gate, the Android emulator on Linux, credential sources, and iOS transcript rendering remain unverified locally or natively. Do not treat missing native test evidence as permission to remove inherited features.
