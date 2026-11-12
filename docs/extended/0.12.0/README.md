# PandaCode Extended 0.12.0 — incremental development cycle

## 2026-11-12 — fork 0.3.114 imported

The current source is [`650537ae449d805656f531fc6fdb5dec09eceb80`](https://github.com/ginmp8/pandacode/commit/650537ae449d805656f531fc6fdb5dec09eceb80) (tree `e6a217fa48157f11f9bc03739d7995b0c8c4cd74`). The 41-path source delta since `9aa88776` imports upstream transcript windowing and Linux audio fallback repairs, plus original 0.3.113/0.3.114 release notes. 33 changed files are byte-exact, eight are reconciled with Extended controls, dependency fixes and CI. The Extended release remains **0.12.0 DRAFT**. This static import is not native Settings On/Off or external-integration acceptance; consult the [checklist](release-checklist.md) and [known issues](known-issues.md).

## 2026-11-12 — JACK/OSS fallback and no-empty-smoke CI import

The current unreleased [fork `9aa88776`](https://github.com/ginmp8/pandacode/commit/9aa88776f07c2274134e4a952ffcd7da71e70e50) (Git tree `bd360c6d9d3d6f8bd209a8408914c4e1b80f85ef`, application **0.3.112**) advances two source commits since `eb78ef90`. Five upstream paths changed: Linux JACK/OSS dictation-versus-microphone-probe documentation, the original `releases/0.3.112.md`, a test-only recording helper in `voice/capture.rs`, real-audio fallback and latency assertions, and release-profile CI. Four paths are imported byte/mode-exact; `.github/workflows/ci.yml` is reconciled to require one passing ignored release-audio smoke test, install ALSA plugins for JACK fallback, and cancel only superseded PR runs. Existing Extended security checks, Expo, Windows ARM64, APT recovery and macOS timeouts remain. The Extended CI test now validates the new smoke contract. Source application manifests remain 0.3.112.

Previous Extended `c2c3789c` [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43182030651) passed 11/11 jobs; [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43182038440) had 10/11 completed successfully and macOS ARM64 in progress at inspection. The new publication SHA needs independent CI; native Settings On/Off, real devices, cloud/VM, MCP and Connect integrations are not proven by static inventory. Planned Extended **0.12.0 remains DRAFT**.


**Status (opened 2026-11-07):** Planned documentation and integration cycle, not a released, approved, tagged or distributed build.

Extended 0.12.0 starts from the completed [0.11.0 source baseline](../0.11.0/README.md). The opening `base/development` commit has `base/hardened` `642b25777b5a3d1600107ff4b9b23604ea90e8a2` as its sole parent. The 0.11.0 `main` has an equivalent Git tree but different rebase commit history. This new cycle introduces documentation scaffolding, **not an upstream source-version bump**.

## 2026-11-12 — inherited microphone smoke-test update

The unreleased [fork `9e5d0988`](https://github.com/ginmp8/pandacode/commit/9e5d0988970785a9302938ecdc8d8848b7d79a7f), Git tree `f82fa609b8f4e74d322729f7c189e783d98e4cad`, updates only `src-tauri/src/voice/tests/real_audio.rs`. Recording-cap assertions now measure meter callback duration, account for unpaced Linux null-ALSA capture and report audio and wall-clock durations. Imported byte/mode-exact; application version remains **0.3.111**. No Settings, production feature, dependency manifest, or Extended control changed. The complete candidate tree has 1,890 upstream paths, 1,948 Extended paths, 1,809 source-identical shared, 81 customized shared and 58 Extended-only, with zero missing paths or type/mode differences. These counts are static, not native behavioral acceptance.

Previous destination `e1cd7e75` [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43165784610) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43165792686) each passed 10/11 jobs. The Node/release identity test found an obsolete `0.3.111` original-release-note SHA in Extended's `source-baseline.json`: pinned `ce584235` versus actual upstream and Extended blob `357b3614`. The baseline pin was updated; neither the test nor original upstream notes were weakened or altered. Exact new-SHA CI, native microphone, Settings On/Off and external integrations remain unverified.

## 2026-11-12 upstream integration

The unreleased fork [`8e5ab59c`](https://github.com/ginmp8/pandacode/commit/8e5ab59cdd22344a08e199dd94e6674af71ac7bd), tree `4fccf8f62aad4a59e3a03005db469819394713f4`, keeps application **0.3.111** unchanged and modifies two source paths. `releases/0.3.111.md` is mirrored exactly; CI is reconciled for the fully qualified ignored release-profile audio test with `--exact` and APT fetch retries `Acquire::Retries=3`, retaining Extended's audits, mobile checks, ARM64 recovery and macOS timeout. The Extended regression test now checks the real Linux/macOS capture requirement and both exact test commands. No upstream product capability or dependency version changed.

Nontruncated candidate inventory: 1,890 upstream files, 1,948 Extended files, 1,809 source-exact shared, 81 customized shared, 58 Extended-only, none missing and no mode/type differences. The 81 differences require separate hunk-level and runtime acceptance. Previous Extended `e059e9a08623` [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43159765950) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43159773384) each reported a failing Node web-test job (526/527 passed): its stale assertion required macOS-only capture despite Linux/macOS now being required. Corrected without weakening coverage. Other jobs were running at inspection; new-SHA CI and real Settings On/Off, microphone and integration acceptance remain pending.

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

## Fork HEAD 66a7df2f: Linux Voice compatibility and strict macOS CI — 2026-11-11

[Upstream `66a7df2f`](https://github.com/ginmp8/pandacode/commit/66a7df2f58ed529056f549fd48b83790153f613d) (tree `caad53d4fd02fb9b1cdf408ed94cdbfd2e354694`) modifies seven source paths without changing the native application version (**0.3.109**). Six paths are imported byte/mode-exact, including the new `src-tauri/src/voice/tests/alsa_names.rs`; the CI workflow is reconciled to require macOS real-audio capture with `PANDACODE_REQUIRE_REAL_MIC=1` while preserving Extended's 70-minute audio step, 120-minute platform job and push-CI policy. The source now resolves cpal 0.16 ALSA saved card names to compatible cpal 0.17 input PCM identifiers and tests fallback, capture-device ordering and duplicate names. A Windows startup smoke fixture's process observation window is increased to 10 seconds without weakening its rejection assertions. Upstream release notes at `releases/0.3.109.md` remain copied exactly, even though their Linux limitation wording predates the follow-up code fix.

Before integration, the full source snapshot had **1,699 files**, the Extended tree **1,755**, with **one missing** upstream ALSA test. Expected after publication: **1,699 source / 1,756 Extended files; 1,618 exact shared, 81 customized shared, 57 Extended-only, no missing files and no mode/type differences**; the exact published tree must be checked separately. Extended's planned distribution remains **0.12.0 DRAFT**. The earlier Extended [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42969586203) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42969593436) were successful on `70d18910` only, not the new integration commit. No local Node/Rust tests or native Linux/real macOS microphone tests have been run for this import. Full semantic acceptance of 81 historical shared divergences, Settings On/Off, Connect/MCP, bots, cloud/VM, credentials, migrations and installers remains open.

## Fork main 0.3.111 integration candidate — 2026-11-11

Fork `ginmp8/pandacode:main` at [`a403c3d9`](https://github.com/ginmp8/pandacode/commit/a403c3d9bfd67ad4bb70eff3f1d2d4befebfa40e), source tree `79a4d1aa2c8dc3537f98f0f5d6cd166837cf4565`, application version **0.3.111**, contains six source commits after `66a7df2f`. The integration candidate imports **293 source-exact new or changed paths** and semantically reconciles **15 concurrently modified paths** to retain Extended controls, dependency remediations and compatibility. Frozen recursive inventory: **1,890 upstream paths, 1,947 Extended paths; 1,809 exact common, 81 divergent common, 57 Extended-only, zero missing upstream, zero mode/type differences**. Native On/Off, remote/Connect, bot, MCP, cloud/VM and credential scenarios have not been independently accepted; GitHub CI is unverified for the integration candidate until its final SHA is published and checks complete.

New inherited functionality includes phone app navigation, screens, attachment pickers, background Connect reconnect and RPC, mobile administration and tasks, new Rust remote methods, and queuing/voice fixes. These are **upstream inheritance**, not new Extended-only features. The source release contains `releases/0.3.111.md`; no `releases/0.3.110.md` is present in the frozen source tree, and none was fabricated. No build/install release, tag or npm publication is approved.


## Fork post-release source update — 2026-11-11

Fork main [`961b43fa`](https://github.com/ginmp8/pandacode/commit/961b43fa88ad1d24eb083bc10779cb7034c4a0fd) (Git tree `3506e38d545f11c57ebb3f2ebe3acdec449606bc`, native app **0.3.111**) adds one unreleased source commit after `a403c3d9`. Direct Extended integration [`5beff6d8`](https://github.com/ginmp8/pandacode-extended/commit/5beff6d8737e952341023bfdf10aaeb38697fcae) is a single-parent child of `7bd8ac1f`: **seven upstream-modified paths**, six copied with exact source blob SHA and mode, and `.github/workflows/ci.yml` reconciled to add Windows arm64 startup-smoke coverage while retaining Extended's 70-minute macOS audio-test and 120-minute matrix-job timeouts. Complete nontruncated inventory at the integration tree: **1,890 upstream / 1,947 Extended blobs; 1,809 exact shared, 81 divergent shared, 57 Extended-only, zero missing upstream paths and zero mode/type differences**.

The added source behavior is Linux fallback to the default microphone on saved-device open failure, a Rust regression test, Windows ARM64 startup-smoke CI and its bounded PID retry. Upstream release notes are byte-identical in Extended; version **0.3.111** remains separate from planned Extended **0.12.0 DRAFT**. Exact new-SHA CI, native capture and configuration/remote acceptance remain unverified at this documentation checkpoint.

## Fork HEAD 106b3852 integration — 2026-11-11

- **Source:** [`106b3852fcfc737922189cb3ef64b98b6b727c69`](https://github.com/ginmp8/pandacode/commit/106b3852fcfc737922189cb3ef64b98b6b727c69), tree `155ab5c52bf39cfa6b0df48ede563e078297055a`, app **0.3.111** unchanged.
- **Inherited fix:** Linux now reports saved ALSA card/raw-ID open failures rather than silently switching input; only saved JACK/OSS can fall back during dictation. The microphone probe never falls back. Source Rust regression tests, original release notes and Voice documentation are mirrored byte-exact.
- **CI reconciliation:** Extended retains manual publishing, dependency/mobile/RustSec checks, APT recovery, Windows ARM64 startup smoke and the longer 70-minute macOS audio test; it also gains upstream Windows x64/Linux ARM64 release-profile audio smoke and both 10-minute APT step limits. A focused Extended workflow regression test is added.
- **Candidate static inventory:** 1,890 source and 1,948 Extended blobs; 1,809 shared identical, 81 shared customized, 58 Extended-only, zero missing or type/mode mismatches. Six upstream changed paths: five source-exact, one adapted workflow.
- **Validation boundary:** CI for the final published SHA, real microphones on target platforms, and functional Settings On/Off, Connect/MCP, bots, cloud/VM, credentials and semantic acceptance of the 81 shared differences are not yet established. The Extended **0.12.0** cycle remains **DRAFT**, not released.

## Code-SHA CI completed and shared-difference inspection — 2026-11-11

Source remains [`106b3852`](https://github.com/ginmp8/pandacode/commit/106b3852fcfc737922189cb3ef64b98b6b727c69) (tree `155ab5c52bf39cfa6b0df48ede563e078297055a`; application **0.3.111**). The integration code SHA [`d3a59446`](https://github.com/ginmp8/pandacode-extended/commit/d3a59446bd9c208b89e8d999f3d001d5be2f6955) completed both [push CI #43075335735](https://github.com/ginmp8/pandacode-extended/actions/runs/43075335735) and [PR CI #43075350618](https://github.com/ginmp8/pandacode-extended/actions/runs/43075350618) **successfully, 11/11 jobs each**, including npm audits, Node/TypeScript/website, mobile/Expo, Rust tests, Clippy (Linux ARM64, macOS x64/ARM64, Windows x64/ARM64), RustSec and macOS ONNX.

Both recursive Git inventories were fetched without truncation: **1,890 upstream files / 1,948 Extended files; 1,809 identical shared, 81 divergent shared, 58 Extended-only, zero missing source paths or mode/type mismatches**. All **81 shared divergences were inspected using added/source-only changed-line sets** against the exact frozen source/destination blobs. The apparent differences concern existing Extended controls, bot context, integration glue, CI/release identity, dependency hardening, schema compatibility and regression tests. This is **not** contextual hunk-by-hunk validation of every caller or native On/Off behavior; no complete functional-parity claim is made.

No fork code or dependency manifest changed in this CI-evidence follow-up. Local Git checkout remains unavailable because GitHub DNS does not resolve in the execution container, so native/local tests and real microphone/Connect/MCP/cloud/VM/credential/installer acceptance were **not** performed. A documentation-only follow-up commit has its own independent CI state and must not inherit the success of `d3a59446`. The planned **Extended 0.12.0 remains DRAFT**, with no release, tag or distribution approval.

## Upstream 0.3.111 documentation correction — 2026-11-12

Fork [`a2ba4b43`](https://github.com/ginmp8/pandacode/commit/a2ba4b430951ee2995bdfda140d4f516cb493a78) updates only original `releases/0.3.111.md`; application **0.3.111** is unchanged. Its corrected phone access, one-hour retry window, 5 MB photo limit and Linux ALSA/JACK/OSS caveats are mirrored exactly, not new Extended functionality. The recursive source/destination comparison after the copy yields **1,890 / 1,948 files, 1,809 exact shared, 81 Extended-customized shared, 58 Extended-only, zero missing/type/mode mismatches**. Previous Extended SHA `1c9fcc3e` passed [push](https://github.com/ginmp8/pandacode-extended/actions/runs/43093250270) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43093260565) CI (11 jobs each); the new SHA requires independent checks. Settings On/Off and native feature parity remain unverified. Planned **Extended 0.12.0 remains DRAFT**.

## Source 802fa847 integration (2026-11-12)

Two additional unreleased upstream commits since `a2ba4b43` alter three source paths without changing native version **0.3.111**. [Fork `802fa847`](https://github.com/ginmp8/pandacode/commit/802fa847f6890d729e98bfe4f2f44190cf045861) extends Linux x64 and ARM64 optimized audio CI using a null ALSA input, documents saved-microphone fallback rules and adds a historical Linux test qualification to the original 0.3.111 release notes. Both documentation files are source-exact; the workflow retains Extended audits, APT remediation and longer macOS smoke.

The complete source/target tree inventories show no missing upstream paths or mode mismatches. Pre-integration counts are 1,890 upstream / 1,948 Extended, 1,807 exact shared, 83 shared divergent and 58 Extended-only; two source-exact updates reduce the shared divergences to the prior **81**. Prior `2fdacaa7` CI is green [on push](https://github.com/ginmp8/pandacode-extended/actions/runs/43152555072) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43152561038), 11/11 each. Final exact-SHA CI, real microphone and On/Off/Connect/MCP/cloud native acceptance are separate; **0.12.0 remains DRAFT**.

## 2026-11-12 — PandaCode 0.3.112 source checkpoint

Pinned fork [`eb78ef90`](https://github.com/ginmp8/pandacode/commit/eb78ef90df6809219349e23fadf2e659ba00da2c), tree `9440057b8012ac1ccd9bbf5a7b5656d488e889c1`. Three new source commits update 12 tracked paths: seven source-exact paths, including two additions (`quit_guard.rs`, original release notes), and five adapted paths preserving Extended npm/Cargo dependency hardening and native runtime changes. Native application version is `0.3.112`; distribution documentation is **Extended 0.12.0 DRAFT**. Inherited quit confirmation asks only for local running agents and bypasses cloud-only, headless, already-confirmed/shutting-down and updater restart cases. Original macOS notarization notes and tests are retained. Previous Extended SHA `17b8d4fa` passed 11/11 jobs on each [push](https://github.com/ginmp8/pandacode-extended/actions/runs/43173312668) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43173319356) CI. New commit CI, native dialog, all customized-source semantic differences and Settings On/Off/integrations require separate validation.
