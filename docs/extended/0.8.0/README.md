# PandaCode Extended 0.8.0 — documentation index

**Status:** planned, unreleased Extended release candidate. The current documentation baseline is PandaCode source `0.3.90` from [`348add18a63906bd8928a7cef547eeaf91429fa7`](https://github.com/ginmp8/pandacode/commit/348add18a63906bd8928a7cef547eeaf91429fa7), initially integrated in Extended at [`f653c8641e23746a4038cfebf1a8a3df90c478e7`](https://github.com/ginmp8/pandacode-extended/commit/f653c8641e23746a4038cfebf1a8a3df90c478e7). This is not a claim that the Extended product version has been published, or that a binary is available.

**Latest targeted upstream parity:** PandaCode `0.3.94`, immutable fork commit [`5ab09c5a4da4a819b30b20771ef768866fa4ef56`](https://github.com/ginmp8/pandacode/commit/5ab09c5a4da4a819b30b20771ef768866fa4ef56). The Extended `0.8.0` candidate remains unreleased. The [original upstream 0.3.94 release notes](../../../releases/0.3.94.md) are mirrored byte-for-byte; earlier integration evidence remains historical.

- [Extended 0.8.0 release notes and feature scope](../../../releases/extended/0.8.0.md)
- [Release readiness and validation checklist](release-checklist.md)
- [Feature controls and enabled/disabled behavior](feature-controls.md)
- [Dependency and security evidence](dependency-security.md)
- [Carried-forward known issues and verification gaps](known-issues.md)
- [Mandatory upstream parity contract](../upstream/parity-contract.md)
- [Pinned upstream source identity](../upstream/source-baseline.json)
- [Previous 0.7.0 candidate documentation](../0.7.0/README.md)

**Version separation:** The PandaCode upstream application version comes from the source manifests. `0.8.0` is the separate **planned Extended release line**. Keep historical `0.7.0` evidence immutable unless correcting a verified error; do not silently re-label those results as 0.8.0 validation.

## Current synchronization evidence

- Code import: [Extended commit `083e7644`](https://github.com/ginmp8/pandacode-extended/commit/083e764485f08ea1b5cabbf3b3d18ea9b426e89c), mirrored from source `0.3.91` / `a9be2ced7bb594ad28a09fbd204cdad34770c750`.
- [Six-job CI #41498498966](https://github.com/ginmp8/pandacode-extended/actions/runs/41498498966): all `success` on the code SHA; native installers and full Settings On/Off acceptance remain unverified.
- Earlier PR #105 is merged, but covers an older `base/hardened` integration and is not evidence of a `0.3.91` release.

## Source candidate: PandaCode 0.3.92 (historical)

The fork introduces its redesigned UI and Settings navigation, live thread scenes, turn cards, tiled conversations, updated test coverage and associated static resources. Extended's configurable Feature controls page is retained as its own Settings section, and the Connect master switch remains an additional gate around upstream remote operations. These are inherited upstream features, not newly invented Extended 0.8.0 functionality. Native GPU, multi-window, Connect and controls On/Off acceptance is pending; CI must be tied to the final commit SHA.

## Verified 0.3.92 integration CI checkpoint — 2026-11-03

The exact Extended code commit [`80c438b47e06d1df330564dc8a4cb17318230310`](https://github.com/ginmp8/pandacode-extended/commit/80c438b47e06d1df330564dc8a4cb17318230310), based on [upstream source `28b835f8b8cb72dcda5506d4bcfff489ad9a98dc`](https://github.com/ginmp8/pandacode/commit/28b835f8b8cb72dcda5506d4bcfff489ad9a98dc), passed both [push CI #41692930559](https://github.com/ginmp8/pandacode-extended/actions/runs/41692930559) and [pull-request CI #41692943491](https://github.com/ginmp8/pandacode-extended/actions/runs/41692943491), six successful jobs per run. The checks cover TypeScript/unit/website, Linux Rust, Expo/mobile, npm dependency audit, RustSec and macOS ONNX. The status is tied to the **code SHA**, not this subsequent documentation commit or an approved Extended 0.8.0 release. Full native GUI/installer, settings On/Off and two-device Connect acceptance remain unverified; see the [release checklist](release-checklist.md).

## Latest source candidate: PandaCode 0.3.93

This import targets upstream commit [`e64505ea8ed66fc8ec8942da96451db56ff91455`](https://github.com/ginmp8/pandacode/commit/e64505ea8ed66fc8ec8942da96451db56ff91455) and preserves the existing Extended controls and dependency remediations. The upstream change adds reliable nested/background subagent reports, unlimited team-capacity operation, improved team state recovery, paged remote worker history, and Codex thread-limit handling that honors user configuration.

A complete source-tree comparison before publication found 1,495 source blobs, zero missing paths in the staged Extended tree, 62 shared files intentionally divergent from upstream, and 28 Extended-only files. Of the 90 changed paths since upstream 0.3.92, 77 use exact source blobs, 12 require independent three-way reconciliation with Extended changes, and one is the unchanged new upstream release note. These are static integration checks, not an assertion of functional equivalence or passing CI for the resulting commit.

Tests for resumed teams, subagents, remote pagination, Codex/MCP On/Off behavior, native installers and two-device Connect remain necessary. The CI verdict must be taken from the published Extended commit SHA, not a previous green run.

## Upstream 0.3.94 synchronization — 2026-11-04

- Pinned fork source: [PandaCode 0.3.94 `5ab09c5a`](https://github.com/ginmp8/pandacode/commit/5ab09c5a4da4a819b30b20771ef768866fa4ef56); original [upstream release notes](../../../releases/0.3.94.md).
- Extended code integration: [`2b1b4cae`](https://github.com/ginmp8/pandacode-extended/commit/2b1b4cae5bae3dd170e13db985c8ab1fb7779687), importing 14 source-changed paths (9 exact copies and 5 reconciled for Extended controls/dependency changes). No upstream source path is missing in the initial whole-tree comparison.
- [Push CI #41780208946](https://github.com/ginmp8/pandacode-extended/actions/runs/41780208946) and [PR CI #41780214009](https://github.com/ginmp8/pandacode-extended/actions/runs/41780214009) on the code integration commit were **cancelled**. On the subsequent [documentation commit `2b24c858`](https://github.com/ginmp8/pandacode-extended/commit/2b24c85867ff93372c4e54141c7c9fa4362dd536), [push CI #41780357920](https://github.com/ginmp8/pandacode-extended/actions/runs/41780357920) and [PR CI #41780361642](https://github.com/ginmp8/pandacode-extended/actions/runs/41780361642) both completed **success (6/6 jobs)**. Any later commit needs its own CI evidence.
- Source changes include expandable worker progress, live reply card height behavior, UI and regression tests. These are inherited upstream improvements, not new Extended-only features. Native control On/Off and real provider, cloud, bot and Connect integration remain unverified.
