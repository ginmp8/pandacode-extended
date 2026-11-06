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

**Earlier documentation-only import (2026-11-05):** [`78f77a0becee2029f4b89ef60fca0c97e48251e2`](https://github.com/ginmp8/pandacode/commit/78f77a0becee2029f4b89ef60fca0c97e48251e2) still declares source application version 0.3.97. Its new README, screenshots and screenshot-capture scripts are inherited upstream documentation tooling, not new Extended product features. See the [0.10.0 release candidate notes](../../../releases/extended/0.10.0.md) for inventory and verification limitations.

## Current integrated upstream source — 2026-11-05

- [`0461e02b85c95f8a52e927382c5069d0799dfed8`](https://github.com/ginmp8/pandacode/commit/0461e02b85c95f8a52e927382c5069d0799dfed8) is the source candidate declaring application **0.3.98**, replacing the older 0.3.97 source baseline for continuing parity work.
- [Original upstream 0.3.98 notes](../../../releases/0.3.98.md) and [Extended 0.10.0 draft notes](../../../releases/extended/0.10.0.md) distinguish inherited behavior from Extended-specific remediations.
- The source update incorporates Recommended Settings and UI/drag/reply changes while keeping the Extended Feature controls section available. Full native acceptance and independent 0.10.0 distribution approval remain open.

## Verified source and CI checkpoint — 2026-11-05

- **Source:** [`0461e02b`](https://github.com/ginmp8/pandacode/commit/0461e02b85c95f8a52e927382c5069d0799dfed8) (**0.3.98**); **integrated code:** [`1d549e2b`](https://github.com/ginmp8/pandacode-extended/commit/1d549e2b01357d813e72ead373565de199481520). Recursive trees (`truncated=false`) verify **1,512 upstream files: 1,447 exact, 65 shared differences, none missing; 40 Extended-only files**, with no file-mode mismatches. The 65 retained differences still require complete hunk and enabled/disabled behavior verification.
- **Exact-code-SHA CI:** [Push #42101783450](https://github.com/ginmp8/pandacode-extended/actions/runs/42101783450) and [PR #42101794523](https://github.com/ginmp8/pandacode-extended/actions/runs/42101794523) completed successfully, **6/6 jobs each** (TypeScript/unit/website, Rust/Linux, RustSec, npm, mobile/Expo, macOS ONNX). This does **not** prove installed-app functionality or count as CI for subsequent documentation-only commits.
- **Outstanding:** release-script and native acceptance; end-to-end Settings On/Off, Connect/MCP/cloud/VM/credentials; actual upgrades, release identity and approval. No tag, installer, publication, or release approval follows from this checkpoint.

## Upstream 0.3.99 integration checkpoint — 2026-11-06

The source [`6b94bf1d`](https://github.com/ginmp8/pandacode/commit/6b94bf1d8b579d08834578f6e24684ea0d3fccbd), tree `befc2c75f17255b820322e23e9e504b2a0c53895`, declares application **0.3.99**. The upstream delta includes **14 changed paths**: 9 copied exactly (including original `releases/0.3.99.md`) and 5 reconciled with existing Extended changes. Expected post-commit static inventory: **1,513 upstream blobs; 1,446 exact, 67 divergent, 0 missing, 40 Extended-only**. Confirm these figures by a full recursive Git-tree readback after publishing. Divergent hunks and native On/Off functionality are **not** completely certified.

[Upstream 0.3.99 notes](../../../releases/0.3.99.md) cover combined team-lead changes and limitations; the [Extended 0.10.0 candidate record](../../../releases/extended/0.10.0.md) distinguishes inherited changes from Extended-only corrections. No native acceptance or release approval is implied.

## Verified 0.3.99 code, static inventory and CI checkpoint — 2026-11-06

- **Immutable source:** [`ginmp8/pandacode:main` at `6b94bf1d`](https://github.com/ginmp8/pandacode/commit/6b94bf1d8b579d08834578f6e24684ea0d3fccbd), tree `befc2c75f17255b820322e23e9e504b2a0c53895`, source application version **0.3.99**. The source branch was rechecked before this documentation update.
- **Integrated code:** [`ginmp8/pandacode-extended:base/development` at `5f28c225`](https://github.com/ginmp8/pandacode-extended/commit/5f28c225dde0f3209be3b1503102b695504c1bed), tree `4172f241e597d1171e34f538a702a7c571199a4f`. Complete recursive Git trees reported `truncated=false`: **1,513 upstream files, 1,444 exact blob/mode matches, 69 shared-path differences, zero missing upstream paths, 40 Extended-only files, zero mode mismatches**. The increase from 67 to 69 shared differences is associated with the separately committed bot-queue UI correction and tests, not a new upstream release. Shared differences have not all been certified hunk by hunk; static presence does not prove full functional parity.
- **Exact-code-SHA CI:** [push run 42167094597](https://github.com/ginmp8/pandacode-extended/actions/runs/42167094597) and [PR run 42167100638](https://github.com/ginmp8/pandacode-extended/actions/runs/42167100638) completed successfully, **6/6 jobs each**: TypeScript/unit/website, Linux Rust check/tests, RustSec, npm dependency audit, Expo/mobile and macOS ONNX parity.
- **Build preparation, not publication:** [Release workflow 42170453607](https://github.com/ginmp8/pandacode-extended/actions/runs/42170453607) completed successfully on the same code SHA, with **8 successful jobs** (prepare, six Windows/Linux/macOS x64/ARM64 builds, SBOM) and **two skipped jobs** (`publish`, `npm`). The macOS Intel packaging job passed on rerun. No release, tag or npm publication follows from this verification.
- **Remaining limitations:** local Git access failed with `Could not resolve host: github.com` (exit 128), preventing local test execution. Installed-app Settings On/Off, manual/automatic transitions, bot queue and permissions, Connect, external MCP, cloud/VM, credential approval, real upgrades, native platform acceptance, and independent Extended distribution identity/approval remain unverified. The Rust `zip` remediation remains pinned at 9.0.5 in the Extended manifest and lockfile. CI and build results above apply **only to code SHA `5f28c225`**, not to any later documentation-only commit or future 0.10.0 candidate.
