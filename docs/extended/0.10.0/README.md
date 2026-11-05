# PandaCode Extended 0.10.0 — development documentation

**Status:** Planned documentation cycle; not a released installer or approved publication.
**Cycle-opening Extended baseline:** [Extended `e8122223`](https://github.com/ginmp8/pandacode-extended/commit/e81222232314844d8fc52f65cb8e2dd37c5da2c8) on `base/development`.
**Pinned upstream at opening:** [source `fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852), PandaCode source/application **0.3.97**. Source versions and new upstream commits are tracked independently of the Extended cycle.

- [0.10.0 draft release notes](../../../releases/extended/0.10.0.md)
- [0.10.0 feature controls](feature-controls.md)
- [0.10.0 dependency/security evidence](dependency-security.md)
- [0.10.0 unresolved verification register](known-issues.md)
- [0.10.0 release-readiness checklist](release-checklist.md)
- [Upstream functional parity contract](../upstream/parity-contract.md)
- [Current pinned upstream source manifest](../upstream/source-baseline.json)
- [Previous Extended 0.9.0 release documentation](../0.9.0/README.md)
- [Previous Extended 0.9.0 notes](../../../releases/extended/0.9.0.md)

The 0.10.0 files start with the **previous-cycle evidence as historical context**, not as fresh test results. A later upstream change belongs in 0.10.0 only after source integration and recorded evidence. Do not rewrite upstream notes in `releases/*.md` or change upstream package/Cargo/Tauri application version fields to the Extended distribution label.

**Latest integrated-source candidate (2026-11-05):** [`78f77a0becee2029f4b89ef60fca0c97e48251e2`](https://github.com/ginmp8/pandacode/commit/78f77a0becee2029f4b89ef60fca0c97e48251e2) still declares source application version 0.3.97. Its new README, screenshots and screenshot-capture scripts are inherited upstream documentation tooling, not new Extended product features. See the [0.10.0 release candidate notes](../../../releases/extended/0.10.0.md) for inventory and verification limitations.

## Current integrated upstream source — 2026-11-05

- [`0461e02b85c95f8a52e927382c5069d0799dfed8`](https://github.com/ginmp8/pandacode/commit/0461e02b85c95f8a52e927382c5069d0799dfed8) is the source candidate declaring application **0.3.98**, replacing the older 0.3.97 source baseline for continuing parity work.
- [Original upstream 0.3.98 notes](../../../releases/0.3.98.md) and [Extended 0.10.0 draft notes](../../../releases/extended/0.10.0.md) distinguish inherited behavior from Extended-specific remediations.
- The source update incorporates Recommended Settings and UI/drag/reply changes while keeping the Extended Feature controls section available. Full native acceptance and independent 0.10.0 distribution approval remain open.
