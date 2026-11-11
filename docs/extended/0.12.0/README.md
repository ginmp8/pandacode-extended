# PandaCode Extended 0.12.0 — incremental development cycle

**Status (opened 2026-11-07):** Planned documentation and integration cycle, not a released, approved, tagged or distributed build.

Extended 0.12.0 starts from the completed [0.11.0 source baseline](../0.11.0/README.md). The opening `base/development` commit has `base/hardened` `642b25777b5a3d1600107ff4b9b23604ea90e8a2` as its sole parent. The 0.11.0 `main` has an equivalent Git tree but different rebase commit history. This new cycle introduces documentation scaffolding, **not an upstream source-version bump**.

## Operating rules

1. For every upstream import, read the **current** `ginmp8/pandacode:main` HEAD, including unreleased commits; assess its complete tracked source tree, tests, assets, scripts, releases, binary objects, modes and manifest versions.
2. Apply upstream integrations **only to `ginmp8/pandacode-extended:base/development`**; keep all upstream capabilities present and functional when explicitly enabled and authorized.
3. Preserve Extended-only Settings controls, native compatibility fixes, added features and verified security remediations; classify each customized shared-file hunk and validate both control states.
4. Log each imported fork SHA, Extended integration SHA, path-level exceptions and exact-SHA tests. Do not count 0.11.0 functionality again as new 0.12.0 functionality.
5. Keep Extended release documentation `0.12.0` separate from the fork's native version (0.3.101 at cycle opening). Do not edit upstream manifests to force a new Extended version.
6. Promote releases using `base/development` → `base/hardened` → `main` with **rebase-only** integrations; no merge/squash commits and no reverse sync from `main` into `base/hardened`. A separate linear replay branch may be used to promote an identical tree to `main` if historical rebase SHAs prevent a direct PR.
7. Never publish tags, GitHub Releases or installers as part of routine upstream synchronization or documentation changes.

## Current records

- [Draft 0.12.0 release notes](../../../releases/extended/0.12.0.md)
- [Feature-control continuity](feature-controls.md)
- [Dependency and security tracking](dependency-security.md)
- [Known issues / acceptance gaps](known-issues.md)
- [Release-readiness checklist](release-checklist.md)
- [Upstream functional parity contract](../upstream/parity-contract.md)
- [Prior 0.11.0 release notes](../../../releases/extended/0.11.0.md)
- [Extended release-tag and publication rules](../releasing.md)

## Opening provenance

- Hardened 0.11.0 baseline: [`642b25777b5a3d1600107ff4b9b23604ea90e8a2`](https://github.com/ginmp8/pandacode-extended/commit/642b25777b5a3d1600107ff4b9b23604ea90e8a2)
- 0.11.0 main promotion: [`e0e807dd265e0ff3ee4260585a7767caa6965b40`](https://github.com/ginmp8/pandacode-extended/commit/e0e807dd265e0ff3ee4260585a7767caa6965b40), same tree `8dc9dcd11ebbc2c18a8c604b4a9e6360e6af7aeb`
- Fork at cycle opening: [`13a7cb68583cf77ac6c4ec351abc3ebc020ff62a`](https://github.com/ginmp8/pandacode/commit/13a7cb68583cf77ac6c4ec351abc3ebc020ff62a), application `0.3.101` (historical reference only)
- Prior 0.11.0 source CI: [#42392453324](https://github.com/ginmp8/pandacode-extended/actions/runs/42392453324) succeeded 6/6; new 0.12.0 validation remains independent.

No new upstream functionality, live native acceptance or deployment is asserted at cycle opening.

## Latest inherited upstream integration (2026-11-08)

- Fork: [`7134edf5`](https://github.com/ginmp8/pandacode/commit/7134edf501d31acc8bec28ff74be9d1dba171669) — native app **0.3.102**, including upstream commits for ZCode model selection and release metadata.
- Extended source integration: [`7db3e2c4`](https://github.com/ginmp8/pandacode-extended/commit/7db3e2c4ae4c5b5dc48e2de50eed5980d247e8be) — 12 changed paths reconciled, original release notes preserved, no missing upstream paths by recursive tree inventory.
- ZCode remains a single-agent harness; automatic team catalogs omit its models. Selecting ZCode ends an unused team, and tasks with past worker activity must start a new task before switching to ZCode.
- **Validation boundary:** browser regression tests were imported but not executed locally; native platform and Extended control-state acceptance remains pending. CI is commit-specific.

## PandaCode 0.3.103 integration

Inherited Council scored reviews and fresh fix rounds were imported from [source `174f0c0b`](https://github.com/ginmp8/pandacode/commit/174f0c0b17ae1b036210cbc5dddf45f0701cd8b0) into [Extended `3f95db51`](https://github.com/ginmp8/pandacode-extended/commit/3f95db516e607cbe6fee5585e61d09ab2c118a71). All 41 upstream changed paths were reconciled; zero upstream paths are missing from the integrated recursive tree. Council is inherited upstream functionality, **not an Extended-only 0.12.0 feature**. The Extended distribution is still a draft; native app version **0.3.103** is separate.

The original [PandaCode 0.3.103 release notes](../../../releases/0.3.103.md) and imported Council tests describe intended behavior. Their presence is not evidence of executed local tests or real account/installer acceptance. Separate [control checks](feature-controls.md), [security tracking](dependency-security.md), [known limitations](known-issues.md) and the [release checklist](release-checklist.md) remain authoritative for acceptance gaps.

## PandaCode 0.3.104 integration

The fork's [complete 0.3.104 source tree at `d5d51769`](https://github.com/ginmp8/pandacode/commit/d5d51769ccac15145e67c071ea06a5502c4cadcb), including unreleased intermediate commits, was integrated into [Extended `6b6a39a8`](https://github.com/ginmp8/pandacode-extended/commit/6b6a39a82f508360572518519badb56e3df8313e). This introduces the **upstream-inherited** desktop panda introduction, sidebar brand replay, Settings star panda, responsive/reduced-motion behavior, shared-mobile-brand desktop Vite fix, preview script, tests and refreshed original screenshots.

All 36 changed upstream paths were reconciled; no source paths are missing. The source snapshot contains 1,561 tracked blobs; the candidate contains 1,618, including 57 Extended-only blobs and 80 customized shared paths. Original [upstream release notes](../../../releases/0.3.104.md) are copied unchanged. The application version is **0.3.104**, while this distribution-documentation cycle remains **0.12.0**. See [incremental release evidence](../../../releases/extended/0.12.0.md), [dependency tracking](dependency-security.md), [feature controls](feature-controls.md) and [release acceptance](release-checklist.md). Static parity and CI do not establish installed-app or Settings On/Off parity.

## PandaCode 0.3.105 source reconciliation

The fork's full [`main` source snapshot `f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665`](https://github.com/ginmp8/pandacode/commit/f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665), including four new files and 34 modified paths since 0.3.104, was incorporated in [Extended code commit `36228b949aaf8648109e2f8f8da441a8735cee41`](https://github.com/ginmp8/pandacode-extended/commit/36228b949aaf8648109e2f8f8da441a8735cee41). Its original [release notes](../../../releases/0.3.105.md) remain unchanged. The inherited account identity, account-pool/connection selection, collapsible account-switch notices and Settings responsiveness are **upstream** features. Six native app-version fields advance to **0.3.105**; this remains the **Extended 0.12.0** planned documentation cycle.

The file-level candidate has 1,565 upstream paths, no missing paths, unchanged modes and 80 pre-existing divergent shared files. The Extended-only adjustments, security/dependency corrections and /btw test isolation hook are preserved, but the 80 shared differences and actual native On/Off, provider credentials, Connect/MCP/cloud/VM and device behavior have not been exhaustively accepted. See [release evidence](../../../releases/extended/0.12.0.md), [feature controls](feature-controls.md), [dependency/security](dependency-security.md), [known gaps](known-issues.md) and [release checklist](release-checklist.md).

## PandaCode 0.3.106 source integration — 2026-11-09

The [fork source `0bfd5738`](https://github.com/ginmp8/pandacode/commit/0bfd57383621d5aa001d3871018920855cabec39) includes ten commits after 0.3.105, covering task tools for all supported harnesses and team leads, transcript streaming/following, RTL text, keyboard layouts, inspector failure recovery, accessible theme headers, and retrying release scanner downloads with explicit missing-coverage errors. The [original upstream 0.3.106 release notes](../../../releases/0.3.106.md) are copied unchanged. These are **inherited upstream fixes, not new Extended-exclusive features**.

All **34** changed source paths have been staged for source integration (26 source-exact and eight reconciled with existing Extended modifications). A complete nontruncated tree comparison identified **1,567 upstream** paths, **1,624 Extended** paths, **zero missing**, **80 shared differences** and **57 Extended-only** paths. The Extended 0.12.0 release candidate is **not approved or distributed**. Neither exact-publication-SHA CI nor native runtime acceptance was complete when this documentation was authored; inspect [the checklist](release-checklist.md) for current validation state.

## Post-0.3.106 upstream HEAD synchronization — 2026-11-09

Source [`7cadfdbeb7006573897a16e1d9c15ac0a59febad`](https://github.com/ginmp8/pandacode/commit/7cadfdbeb7006573897a16e1d9c15ac0a59febad) has one unreleased additional commit following the 0.3.106 release, updating only `scripts/release/sbom.test.mjs` to normalize CRLF workflow line endings and validate section boundaries. The test file is mirrored byte-exact; the upstream native app version remains **0.3.106** and planned Extended documentation remains **0.12.0 (draft)**. Complete nontruncated trees contain **1,567 source / 1,624 Extended files**, **1,487 identical shared**, **80 divergent shared**, **57 Extended-only** and **zero missing** source files. The existing RustSec finding and native/Settings acceptance are still unresolved.

## SQLite dependency remediation candidate — 2026-11-09

A bounded security remediation updates the bundled Rust SQLite graph to **rusqlite 0.41.1**, **libsqlite3-sys 0.39.1** and their compatible lockfile dependencies. Package records match the previously validated read-only `extended/development` graph, while application version **0.3.106**, planned Extended distribution **0.12.0**, and existing Settings/feature paths remain unchanged. [RustSec/locked Rust checks on that reference branch](https://github.com/ginmp8/pandacode-extended/actions/runs/42651770546) passed; exact-new-SHA CI and native SQLite/migration acceptance are still pending. See [dependency/security](dependency-security.md), [known issues](known-issues.md), and [release checklist](release-checklist.md).

## Exact-commit CI status — 2026-11-09

Both [push CI #42680289120](https://github.com/ginmp8/pandacode-extended/actions/runs/42680289120) and [PR CI #42680300162](https://github.com/ginmp8/pandacode-extended/actions/runs/42680300162) finished **successfully (6/6 jobs each)** on Extended code commit [`06cb9b0c0141`](https://github.com/ginmp8/pandacode-extended/commit/06cb9b0c0141e964d43d0c462cbd30e9fc44ec1c). RustSec, locked Linux Rust tests and Clippy, npm dependency audits, Node/release tests and website, Expo/mobile, and macOS ONNX checks passed. Earlier references to pending CI are historical snapshots, not the latest verified result for that code commit. Native SQLite migration and rollback, installed-platform acceptance, Settings On/Off integrations and the 80 shared-file semantic reviews remain open. The Extended `0.12.0` documentation remains an unapproved candidate.

## Upstream 0.3.107 Skills manager integration — 2026-11-10

- **Pinned fork HEAD:** [`5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1`](https://github.com/ginmp8/pandacode/commit/5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1), Git tree `df0d32d0b1f959f0ab9177515caba186c903c20d`; native application version **0.3.107**. The planned Extended distribution version remains **0.12.0**.
- Relative to the previous fork pin `7cadfdbeb7006573897a16e1d9c15ac0a59febad`, the source changes **92 tracked paths**: **53 additions, 37 modifications, and two deletions**. Reconciliation preserves **75 source-exact changed blobs**, **15 Extended-adapted shared paths**, and the two upstream deletions. Entire tree candidate: **1,618 upstream / 1,675 Extended files; 1,538 identical shared, 80 Extended-modified shared, 57 Extended-only, zero upstream missing**.
- **Inherited upstream feature, not Extended-exclusive:** Settings → Tools → Skills manager inventories installed agent skills, per-project/global disable switches, installs via the pinned Skills CLI, quarantine/restore, uninstall/Undo, and honors switches in slash commands and bots. Rules & library remains available as a separate page. Original `releases/0.3.107.md`, documentation, runtime modules, tests and fixtures were imported without rewriting upstream release history.
- **Preserved Extended behavior:** the separate Feature controls Settings page, optional Connect/MCP/bot/cloud/VM/credential gates, existing SQLite remediation `rusqlite 0.41.1` / `libsqlite3-sys 0.39.1`, and independent distribution identity. New Skills switches are inherited controls, not permission to remove other upstream capabilities.
- **Evidence limits:** untruncated pinned source/destination Git tree inventory and content-hash reconciliation; no local Git clone, Cargo, npm/Playwright or native desktop test execution in this environment. Exact-SHA GitHub CI and functional Settings On/Off, live Skills CLI, platform install/undo and upstream/Extended integration still require independent execution. No tag, release, installer distribution or deployment is approved.

## Exact-commit CI result for upstream 0.3.107 — 2026-11-10

Both [push CI #42805588366](https://github.com/ginmp8/pandacode-extended/actions/runs/42805588366) and [PR CI #42805595388](https://github.com/ginmp8/pandacode-extended/actions/runs/42805595388) completed **successfully (6/6 jobs each)** on the reconciled code commit [`e4e5c0c51daa8a0149015bb26a00c8fb09bb2050`](https://github.com/ginmp8/pandacode-extended/commit/e4e5c0c51daa8a0149015bb26a00c8fb09bb2050). All reported jobs passed: Linux Rust check/tests, RustSec, Node/TypeScript/unit tests and website, npm dependency audit, Expo/mobile compatibility and macOS ONNX parity.

The branch remained at this code SHA when checked. Exact matching of all upstream paths and successful CI are separate from native Skills installation, SQLite migrations, real Settings On/Off and full semantic parity of the 80 Extended-customized files. The planned 0.12.0 distribution is still unapproved; a subsequent docs-only commit has a separate CI identity.

## Upstream 0.3.108 voice-dictation integration — 2026-11-10

- Source: [`ginmp8/pandacode:main` `952d0acbe02547af036ef6d7635ec415a472eb3b`](https://github.com/ginmp8/pandacode/commit/952d0acbe02547af036ef6d7635ec415a472eb3b), tree `202f77088372d261b263b806354d84ce094f25b8`, native app version **0.3.108**; includes commits after the source's release commit. The planned Extended distribution documentation remains **0.12.0 DRAFT**.
- Reconciled the 109-path source delta relative to `5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1`: 78 additions (including Rust/frontend audio input, model catalog, tests and notices), 17 exact source updates and 14 changes reconciled with Extended behavior. The inherited voice feature supports optional on-device speech models and configured transcription services, transcription into the composer, explicit recording/stop and tests. This is not an Extended-only invention.
- Preserved Extended Feature controls, release identity and dependency remediation, including `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`. Native microphone permissions, service credentials, model downloads, user-cancelled recording, offline transcription and Settings states need separate behavior checks.
- This section describes a source integration **candidate**, not evidence of a successful new-SHA CI run. Check the eventual exact commit SHA for Rust/Linux/Windows/macOS, RustSec, Node, npm, Expo, installer/platform and native feature-control acceptance. Historic successful CI applies only to the earlier SHAs.


## Upstream 0.3.108 follow-up — 2026-11-10

The frozen fork source is [`57f0aaf7cfc3`](https://github.com/ginmp8/pandacode/commit/57f0aaf7cfc35419d4a21b9823bbdf35dc73cfd1), tree `6d7f582267471080a5752e243769a4684e1994a3`; it adds **two unreleased follow-up commits** after the prior `952d0acbe025` source pin. Native version remains **0.3.108** and the Extended planned release is still **0.12.0 DRAFT**. Seven upstream paths changed: **six copied byte/mode-exact**, including original release notes, documentation, install-note regression and a deterministic voice-model tamper test; the release workflow was **adapted** to enforce green exact-commit CI *only when manual Extended publication is requested*. Tag pushes remain artifact-only and no npm publishing is added. The complete upstream tree has **1,696 files** and Extended **1,753** (expected 1,616 shared exact, 80 previously Extended-customized shared, 57 Extended-only, no missing paths or mode differences). No local clone, npm, Cargo or native Settings On/Off test was executed; check exact final-SHA CI separately. This checkpoint does not certify behavioral parity or approve release.

## Post-0.3.108 source follow-up — 2026-11-10

Fork [source `8682acaf97dc`](https://github.com/ginmp8/pandacode/commit/8682acaf97dcd57217087d07deaa21e1dc6bb751) (tree `9405c4f98758a73fb559066bda62acd920fe75fd`) changes **four paths** after `57f0aaf7cfc35419d4a21b9823bbdf35dc73cfd1`: three source-exact documentation/release-note updates and one adapted release workflow. The inherited gate now accepts only an exact-SHA **push to main** CI run, parses run status/conclusion/URL independently, and distinguishes cancelled CI; Extended keeps manual publication only, artifact-only tag builds, independent release identity and no npm publishing. The application version stays **0.3.108** and planned Extended documentation stays **0.12.0 draft**. Previous Extended SHA `f0c1cf82ff3261845eb2cd004088cc912cf4bf8f` passed [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42851899273) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42852652908), **11/11 jobs each**. New exact-SHA CI, live release-gate, 80 historic divergent shared-file hunks, native Voice/Settings/Connect/MCP/bots/cloud/VM/credential, installer, SQLite migration and rollback acceptance remain unverified. No release approved.

## Fork HEAD 979b7d4e integration — 2026-11-10

Source [`ginmp8/pandacode:main` `979b7d4e3c07`](https://github.com/ginmp8/pandacode/commit/979b7d4e3c073e9bebf74f70bf80440a27564237) (tree `ba33a37daed7860854926617c90197d3182bd8ed`) includes **two commits after the previous pin `8682acaf97dc`**. Native application version stays **0.3.108**; planned Extended distribution stays **0.12.0 DRAFT**. The six modified upstream paths comprise five byte/mode-exact copies (`README.md`, `docs/releasing.md`, `docs/voice-input.md`, `releases/0.3.108.md`, and `website/src/pages/privacy.astro`) plus one reconciled Extended manual-publication workflow. The release workflow rejects an empty or malformed GitHub CI lookup with an explicit diagnostic, while tag builds remain artifact-only, publication requires manual opt-in, and npm publishing remains prohibited. A focused release-workflow regression assertion is added. No changes to native dependency or application-version manifests.

Expected verified post-integration inventory: **1,696 source / 1,753 Extended** paths; **1,616 exact shared, 80 divergent shared, 57 Extended-only, zero source paths missing**. This is structural, not behavioral parity. Check CI of the new exact destination SHA and native feature control paths separately; no release approved.

## Fork HEAD bc323966 integration — 2026-11-10

Frozen [fork main `bc323966a0e0`](https://github.com/ginmp8/pandacode/commit/bc323966a0e0f24b13dfd5732cc300f791a965fe) includes two commits after `979b7d4e`; the upstream app stays **0.3.108** and Extended distribution remains **0.12.0 DRAFT**. Eleven upstream paths changed: eight exact copies (Voice Rust implementation/tests, original upstream Voice/release docs) and three reconciled Extended paths (`ci.yml`, `release.yml`, `AGENTS.md`). Automatic microphone stops now correctly say **Transcribe recording**; local engine crashes offer a speech-service fallback, with upstream Rust regression tests. Main push CI is no longer auto-cancelled and a failed `gh run list` is explicitly rejected by the publishing gate. Extended retains manual-only GitHub Release publishing, tag-build artifacts only and no npm publish. A focused Extended release-gate test was extended. No version or dependency manifest changed.

Full path inventory: **1,696 upstream / 1,753 Extended** tracked files, no missing upstream paths; 80 historic shared-file divergences require separate semantic review, and the adapted `AGENTS.md` makes one further documented Extended difference. Exact-SHA CI and real Voice/Settings/Connect/MCP/bots/cloud/VM/credential/installer acceptance are not established by this structural import.

## Fork HEAD 9437b3a9 follow-up — 2026-11-10

Latest source: [`9437b3a92cc3`](https://github.com/ginmp8/pandacode/commit/9437b3a92cc3d41e899770f645f8a08449ce6bbf) (tree `5b8666849fcd7f3cbba918754efbfaedbc8fef41`, app version **0.3.108**), three commits after `bc323966`. Seven upstream paths changed: five copied byte/mode-exact; `ci.yml` and `release.yml` reconciled with Extended controls. Changes clarify Voice service privacy/server requirements and CI-run recovery; no new Extended product feature or new dependency update is claimed. The prior Extended [`4b8e2e10`](https://github.com/ginmp8/pandacode-extended/commit/4b8e2e1059de598064107d0d2f6282f98e9c5f9d) has successful [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42882104720) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42882114490); those results do not validate the new commit. Native Voice/Settings/Connect/MCP/bots/cloud/VM acceptance, installer behavior and historical divergent hunks remain unverified. **0.12.0 is still a draft.**

## Fork HEAD 64996a1a integration — 2026-11-11

**2026-11-11 — inherited PandaCode 0.3.109:** [fork `64996a1a`](https://github.com/ginmp8/pandacode/commit/64996a1a8b1d722eb10b0c4ec4b3c1ffef1fd197) (tree `7be2186e18047098aedbbd18ff0b4ea11d50b806`) includes the macOS release-build Voice crash fix via `cpal 0.17.3`, saved microphone-name compatibility, new release-profile real-audio tests, Council composer redraw synchronization, clarified privacy wording and original upstream release notes. The 17 changed paths comprise 12 byte-exact copies (including two new files) and five reconciled CI/dependency/version paths. Source app `0.3.109` and Extended planned distribution `0.12.0` remain separate; native acceptance and the eventual destination-SHA CI are not yet verified.

The release-profile macOS real-audio test is conditional on a compatible runner; even a successful test does not establish native device permissions, installed-client behavior, external speech services or the Extended On/Off permission matrix. No tag or package has been published.
