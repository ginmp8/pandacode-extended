# Extended 0.12.0 — dependency and security tracking

**Status:** Opening baseline inherited from [0.11.0](../0.11.0/dependency-security.md); no new remediation is claimed for the 0.12.0 cycle.

Preserve the existing security remediations and compatible overrides, including the previously documented Rust `zip` 9.0.5 update (first applied in the 0.10.0 cycle). Do not describe inherited patches as new security fixes. The fork's native application version is 0.3.101 at opening; this is independent of Extended 0.12.0 documentation.

## Required checks for every imported upstream change

- [ ] Compare npm, Cargo, mobile/Expo and other lockfiles and manifests with the exact upstream commit, preserving remediations unless replaced by a verified safe alternative.
- [ ] Run RustSec, npm audit, TypeScript/website, Rust check/tests/Clippy, mobile/Expo and relevant native build checks at the new exact commit.
- [ ] Check updates to dependency overrides, MSRV/toolchain requirements, supported architectures and lockfile consistency.
- [ ] Trace each confirmed advisory to affected and fixed versions, applicability and mitigation evidence; do not claim exploitability without support.
- [ ] Verify that Extended tag/release identity remains separate from the upstream native installer version and that no npm publishing job was introduced.
- [ ] Track installation, update/rollback, signing, SBOM and artifact provenance separately from CI unit-test success.

| Evidence | State |
| --- | --- |
| Baseline | [0.11.0 dependency/security documentation](../0.11.0/dependency-security.md) |
| Prior source validation | [0.11.0 `main` CI #42392453324](https://github.com/ginmp8/pandacode-extended/actions/runs/42392453324), 6/6; **not** a future 0.12.0 test |
| New 0.12.0 fixes | None verified at cycle opening |
| Final 0.12.0 dependency and native validation | Pending |

Keep all future remediation notes source-linked, version-specific and reproducible.

## Upstream 0.3.102 integration — 2026-11-08

The [fork source `7134edf5`](https://github.com/ginmp8/pandacode/commit/7134edf501d31acc8bec28ff74be9d1dba171669) updates the upstream application's version metadata from 0.3.101 to **0.3.102** in root npm, Cargo package/lock and Tauri manifests; those fields were synchronized in Extended. The source change did not introduce dependency package version changes in the compared manifests. The existing Extended dependency overrides/remediations were preserved by applying only the source release-version hunks. No new security remediation is claimed. npm audit, RustSec, Rust/TypeScript builds and native packaging were not executed locally; CI for the integration commit must be checked by SHA before claiming it passed.

## PandaCode 0.3.103 integration

Upstream [`174f0c0b`](https://github.com/ginmp8/pandacode/commit/174f0c0b17ae1b036210cbc5dddf45f0701cd8b0) advanced the **native application version to 0.3.103** in npm root and lockfile, Cargo manifest and app lock entry, and Tauri configuration; Extended synchronizes these six fields without changing the planned distribution cycle 0.12.0. Existing Extended package/lock overrides and dependency remediations were retained through nonoverlapping three-way merges. The upstream feature introduced Council tests and implementation, not a separately verified dependency security fix.

No local `npm audit`, RustSec, Rust builds, mobile tests or installed-app checks ran in this session. Evaluate CI at the final published SHA and keep any native-platform, signed-updater, migration or approved cloud/credential tests separately pending.

## PandaCode 0.3.104 dependency/build reconciliation

The [source `d5d51769`](https://github.com/ginmp8/pandacode/commit/d5d51769ccac15145e67c071ea06a5502c4cadcb) changes six native application-version fields to **0.3.104**; the [Extended integration `6b6a39a8`](https://github.com/ginmp8/pandacode-extended/commit/6b6a39a82f508360572518519badb56e3df8313e) aligns them while preserving Extended npm/Cargo version overrides, including DOMPurify, KaTeX and Vite, and existing build/release protections. The upstream Vite `tsconfigRaw` change avoids requiring Expo's base tsconfig when pure mobile-brand modules are compiled for desktop. The `pandacode-run.sh` preview launcher is inherited unchanged and executable; no new auto-publication step was added.

The source-specific build regression and brand tests were copied. No additional remediation or advisory closure is claimed. The exact-SHA [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42503275362) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42503280072) were in progress when first observed; final code and documentation commit results, npm/RustSec audits and installed-app validation must be checked separately. Local build/audit commands were not run.

## PandaCode 0.3.105 dependency/version reconciliation

The pinned [fork `f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665`](https://github.com/ginmp8/pandacode/commit/f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665) advances the native application version from **0.3.104** to **0.3.105**, in `package.json`, both package-lock root version fields, the app Cargo package/lock entries and `tauri.conf.json`. The [Extended code integration `36228b949aaf8648109e2f8f8da441a8735cee41`](https://github.com/ginmp8/pandacode-extended/commit/36228b949aaf8648109e2f8f8da441a8735cee41) applied those exact version hunks to existing remediated manifests, preserving Extended dependency overrides and lockfile history. The source comparison did not introduce unrelated dependency-version changes in those manifest hunks; no new vulnerability fix or advisory resolution is claimed.

CI for the code SHA was [queued or running](https://github.com/ginmp8/pandacode-extended/actions/runs/42554247514) at the first check. Local `npm audit`, RustSec, dependency rebuild and installed-platform tests were not run; a passing CI job, if observed, would still be narrower than manual provider/credential and installer acceptance.


## Upstream 0.3.106 scanner and dependency continuity

The fork's application version **0.3.106** is carried in npm, Cargo and Tauri metadata. Existing Extended dependency remediations, overrides and lockfile-only modifications remain preserved; this source update did **not** introduce or claim a newly fixed dependency vulnerability. The inherited release workflow retries transient Syft/Grype download failures and returns an explicit failed coverage step when an SBOM coverage report is missing. The Extended manual-publication gate (no automatic public release or npm publish) remains unchanged. Run npm and RustSec audits and inspect the exact published commit's CI jobs before recording a passed audit.

## Exact-SHA CI security finding — 2026-11-09

At [Extended integration `34d8fb4c`](https://github.com/ginmp8/pandacode-extended/commit/34d8fb4caa985f2ceb4815945dc58ed3042c8353) (upstream [`0bfd5738`](https://github.com/ginmp8/pandacode/commit/0bfd57383621d5aa001d3871018920855cabec39), app **0.3.106**), [push CI #42644925066](https://github.com/ginmp8/pandacode-extended/actions/runs/42644925066) and [PR CI #42644937837](https://github.com/ginmp8/pandacode-extended/actions/runs/42644937837) both **failed**: five of six jobs succeeded in each, but **RustSec audit failed**. The lockfile contains `rusqlite 0.38.0` and `libsqlite3-sys 0.36.0`, whose bundled SQLite is affected by [RUSTSEC-2026-0459 / CVE-2026-119375](https://github.com/RustSec/advisory-db/blob/main/crates/libsqlite3-sys/RUSTSEC-2026-0459.md) (CVSS 6.8). The advisory identifies patched `libsqlite3-sys >=0.39.0`, released with `rusqlite 0.41`; the application explicitly enables `rusqlite`'s `bundled` feature. Its exposure involves opening a crafted local database plus WAL file; the failed audit does not independently establish exploitability in a deployed installation.

**Unresolved, release-blocking dependency remediation:** upgrade the compatible `rusqlite`/SQLite dependency graph, regenerate and commit `src-tauri/Cargo.lock` using Cargo (not hand-edited), verify the Rust toolchain floor (`rusqlite 0.41` requires Rust 1.89), check any affected SQLite API/storage and migration paths, and rerun `cargo check --locked`, relevant Rust/SQLite regression tests, and `cargo audit --file src-tauri/Cargo.lock` before treating the finding as fixed. The local sandbox has no Cargo and cannot resolve GitHub DNS, so no validated lockfile regeneration or Rust test was possible. No vulnerability suppression, CI bypass, or dependency-version-only edit was applied. The source and destination Git trees still have **zero missing upstream paths**, but this does not prove functional equivalence of the 80 customized shared files or validate Settings/Connect/MCP/cloud/VM native behavior. Planned Extended distribution **0.12.0** remains unapproved; checks on any documentation-only follow-up SHA are separate.

## SQLite/RustSec dependency candidate — 2026-11-09

- **Remediation candidate:** `rusqlite` was advanced to **0.41.1** with `bundled` still enabled. Its dependency graph now locks **libsqlite3-sys 0.39.1**, **hashlink 0.12.2**, and the updated `hashbrown 0.17.1` dependency entry. The 686 resolved package sections are identical to the lockfile graph from read-only `extended/development` at [`d0100f72`](https://github.com/ginmp8/pandacode-extended/commit/d0100f72c25e3ccff58c294d624505af8a216b48), except for the application package version, which stays at **0.3.106**. The lockfile entries were reused from that pre-existing CI-tested graph, not invented or suppressed.
- **Reference validation (different branch):** [CI #42651770546](https://github.com/ginmp8/pandacode-extended/actions/runs/42651770546) passed both RustSec and Linux Rust check/tests on the pinned read-only branch. The run failed an unrelated UI Playwright job. Those passing jobs support the dependency graph but **do not prove the new base/development SHA passes**.
- **Remaining gates:** no local Cargo/Rust runtime is available here. Run exact-new-SHA RustSec, locked Rust/Clippy tests and native SQLite storage/migration checks before declaring remediation complete. The upstream remains [`7cadfdbeb700`](https://github.com/ginmp8/pandacode/commit/7cadfdbeb7006573897a16e1d9c15ac0a59febad); planned Extended **0.12.0** remains draft, with no tag or release. No upstream path or feature was removed.

## Exact-SHA RustSec remediation validation — 2026-11-09

On Extended [`06cb9b0c0141`](https://github.com/ginmp8/pandacode-extended/commit/06cb9b0c0141e964d43d0c462cbd30e9fc44ec1c), [push CI #42680289120](https://github.com/ginmp8/pandacode-extended/actions/runs/42680289120) and [PR CI #42680300162](https://github.com/ginmp8/pandacode-extended/actions/runs/42680300162) each concluded **success (6/6)**. The committed `libsqlite3-sys 0.39.1` / `rusqlite 0.41.1` graph passed the RustSec audit in both runs; Linux `cargo test --locked --lib` and `cargo clippy --locked --all-targets -- -D warnings` also passed, as did the remaining npm, web, mobile, and ONNX jobs.

This resolves the **CI audit failure at this exact code SHA**, not the separate native installation and SQLite migration/rollback acceptance gates. No Cargo commands were executed locally, no advisory suppression was introduced, and the upstream application version remains `0.3.106`. Future commits require their own exact-SHA checks.

## Upstream 0.3.107 import dependency evidence

The application manifests were synchronized to **0.3.107** while retaining Extended's existing compatible dependency remediation: `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`, including their lockfile graph. The new inherited Skills manager invokes the upstream-pinned Skills CLI `skills@1.7.6`; this is an upstream feature description, not an independent security approval. npm/RustSec audits, Rust locked tests/Clippy, and live CLI installation remain pending for the new destination commit. No unrelated dependency downgrade or advisory suppression was introduced.

## Upstream 0.3.107 dependency CI follow-up — 2026-11-10

Both [push CI #42805588366](https://github.com/ginmp8/pandacode-extended/actions/runs/42805588366) and [PR CI #42805595388](https://github.com/ginmp8/pandacode-extended/actions/runs/42805595388) completed **successfully (6/6 jobs each)** on the reconciled code commit [`e4e5c0c51daa8a0149015bb26a00c8fb09bb2050`](https://github.com/ginmp8/pandacode-extended/commit/e4e5c0c51daa8a0149015bb26a00c8fb09bb2050). All reported jobs passed: Linux Rust check/tests, RustSec, Node/TypeScript/unit tests and website, npm dependency audit, Expo/mobile compatibility and macOS ONNX parity.

The RustSec and npm jobs passed with the Extended lockfiles at this exact code SHA; this replaces the earlier pending CI state for this commit only. Do not infer that live Skills CLI installations, native SQLite upgrade/rollback, Windows/Linux platform behavior or future commits have been validated. No new dependency remediation or security approval is claimed.

## Upstream 0.3.108 voice-dictation integration — 2026-11-10

- Source: [`ginmp8/pandacode:main` `952d0acbe02547af036ef6d7635ec415a472eb3b`](https://github.com/ginmp8/pandacode/commit/952d0acbe02547af036ef6d7635ec415a472eb3b), tree `202f77088372d261b263b806354d84ce094f25b8`, native app version **0.3.108**; includes commits after the source's release commit. The planned Extended distribution documentation remains **0.12.0 DRAFT**.
- Reconciled the 109-path source delta relative to `5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1`: 78 additions (including Rust/frontend audio input, model catalog, tests and notices), 17 exact source updates and 14 changes reconciled with Extended behavior. The inherited voice feature supports optional on-device speech models and configured transcription services, transcription into the composer, explicit recording/stop and tests. This is not an Extended-only invention.
- Preserved Extended Feature controls, release identity and dependency remediation, including `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`. Native microphone permissions, service credentials, model downloads, user-cancelled recording, offline transcription and Settings states need separate behavior checks.
- This section describes a source integration **candidate**, not evidence of a successful new-SHA CI run. Check the eventual exact commit SHA for Rust/Linux/Windows/macOS, RustSec, Node, npm, Expo, installer/platform and native feature-control acceptance. Historic successful CI applies only to the earlier SHAs.

The new voice pipeline introduces native `cpal`, `rubato`, `transcribe-cpp` and platform audio/CMake dependencies. The Extended Rust lockfile retains the prior remediated SQLite versions and must pass `cargo check --locked`, RustSec and all platform CI rows before any distribution approval.


## Upstream 0.3.108 post-version CI gate — 2026-11-10

The seven changed source paths at [`57f0aaf7cfc3`](https://github.com/ginmp8/pandacode/commit/57f0aaf7cfc35419d4a21b9823bbdf35dc73cfd1) do **not** change package or Cargo manifests/lockfiles. Existing Extended SQLite remediation (`rusqlite 0.41.1`, `libsqlite3-sys 0.39.1`) remains in place. Revalidate RustSec, locked Rust tests/Clippy and npm audit on the **exact new destination SHA**, and test native model downloads plus SQLite upgrade/rollback separately.

## Post-0.3.108 source follow-up — 2026-11-10

No npm/Cargo manifest or lockfile change; native app remains **0.3.108**, with Extended remediations `rusqlite 0.41.1` / `libsqlite3-sys 0.39.1` unchanged. Prior code SHA `f0c1cf82ff3261845eb2cd004088cc912cf4bf8f` passed both 11-job CI runs. Reverify RustSec, npm, and locked-build checks on the exact new destination commit; previous success is SHA-specific.

## Fork HEAD 979b7d4e integration — 2026-11-10

No package manifests or lockfiles changed in the six-path source delta. Existing Extended remediation (`rusqlite 0.41.1`, `libsqlite3-sys 0.39.1`) remains unchanged; no new vulnerability or remediation is asserted. Recheck RustSec, npm audit and locked Rust/Node builds on the new destination SHA; a previously cancelled ONNX/macOS push job does not demonstrate a dependency defect.

## Fork HEAD bc323966 integration — 2026-11-10

No npm/Cargo manifests or lockfiles changed in the 11-path upstream delta. Native application version remains **0.3.108** and Extended preserves `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`. The added Voice tests and CI gate changes do not claim a new vulnerability fix. Run RustSec, npm audits, Rust/Node locked checks and platform build verification against the **new destination SHA**; previous CI results cannot validate it.

## Mobile node-forge advisory remediation — 2026-11-10

The exact-SHA [push dependency-audit job](https://github.com/ginmp8/pandacode-extended/actions/runs/42920988289) on `c7c146067cfa6a176b2ee79a95f262b867ed175f` failed at `npm audit --prefix mobile --audit-level=low`: the mobile lockfile's **node-forge 1.4.2** is affected by [GHSA-6889-vf6x-f3g5](https://github.com/advisories/GHSA-6889-vf6x-f3g5), [GHSA-w4xr-hqv8-ch44](https://github.com/advisories/GHSA-w4xr-hqv8-ch44), [GHSA-6cv4-p8m7-4r25](https://github.com/advisories/GHSA-6cv4-p8m7-4r25), and [GHSA-j8rw-jp99-435v](https://github.com/advisories/GHSA-j8rw-jp99-435v). The GitHub-reviewed advisories specify **1.4.3** as the first fixed version. This is a confirmed dependency advisory and audit failure, not proof of a reachable application exploit.

The Extended mobile override and the one resolved `node_modules/node-forge` lock entry now target **1.4.3**, preserving Expo/CLI dependencies and other Extended overrides. The exact integrity (`sha512-P6wzfh6t70avmV8FrVEAd/36sOPJQ+hkaJVX1Laap9J/77m7M0eyozDvc8SwnDf4P7MP94gwbAAo0rg3YVXT+g==`) is sourced from the [npm registry's published metadata](https://registry.npmjs.org/node-forge/1.4.3). Local package install/audit was blocked by registry DNS. Confirm remediation with the **new exact-SHA CI**; keep installed-app and runtime acceptance separate.

## Verified node-forge remediation CI — 2026-11-10

The earlier mobile audit failure on `node-forge@1.4.2` was followed by the narrowly scoped lockfile and override update to **1.4.3** in [Extended commit `ab6c99787bf8ece8eae70b51eb3d071b91353e41`](https://github.com/ginmp8/pandacode-extended/commit/ab6c99787bf8ece8eae70b51eb3d071b91353e41). Both [push CI #42921416535](https://github.com/ginmp8/pandacode-extended/actions/runs/42921416535) and [PR CI #42921425457](https://github.com/ginmp8/pandacode-extended/actions/runs/42921425457) completed successfully with **11/11 jobs each** on that exact SHA, including the mobile dependency audit and Expo compatibility, root npm audit, RustSec and Linux Rust tests. This resolves the *observed audit/check failure* for the pinned commit, not an independently confirmed native-device security or compatibility test.

No new dependency version or advisory suppression was introduced by this documentation follow-up. Prior SQLite remediation (`rusqlite 0.41.1` / `libsqlite3-sys 0.39.1`) remains in the current lockfile. Later commits, installed-device behavior, SQLite migrations and all Extended control-state interactions require separate checks.

## Fork 0.3.109 audio dependency integration — 2026-11-11

The current source moves `cpal` from 0.16 to 0.17 (resolved to 0.17.3) to address a reported macOS release-build microphone enumeration crash and updates the CoreAudio/ALSA/Objective-C dependency closure in Cargo.lock. The Extended adaptation deliberately preserves previously remediated `rusqlite 0.41.1`, `libsqlite3-sys 0.39.1`, other Rust security constraints and JS/mobile `node-forge 1.4.3`. Compatibility of the composed lockfile, macOS optimized tests, npm audits and RustSec must be evaluated on the exact integration SHA; this note is not evidence of an independent security audit or installed-client acceptance.

## Fork application 0.3.111 and mobile package dependencies — 2026-11-11

The current source is [`a403c3d9`](https://github.com/ginmp8/pandacode/commit/a403c3d9bfd67ad4bb70eff3f1d2d4befebfa40e), application `0.3.111`. The candidate aligns npm, Cargo and Tauri application identity with this native version; the separate **Extended 0.12.0** distribution cycle remains DRAFT. Mobile adds `expo-document-picker ~58.0.4` and `expo-image-picker ~58.0.10` with merged lock records while preserving Extended's `deepmerge` patch. Root npm/Cargo locks retain Extended dependency remediations, including their security overrides; the upstream application-version-only changes in Cargo files are applied without downgrading those packages. These are static source/manifest checks, **not** evidence of npm audit, RustSec, installed-client compatibility, or a new vulnerability remediation.


## Mobile npm 10 clean-install failure after upstream 0.3.111 — 2026-11-11

At Extended [`d67a035d`](https://github.com/ginmp8/pandacode-extended/commit/d67a035d7a1a6e0635289b76ed1b07054263932f), the [push CI run](https://github.com/ginmp8/pandacode-extended/actions/runs/43028709840) and [PR CI run](https://github.com/ginmp8/pandacode-extended/actions/runs/43028720555) fail mobile `npm ci --prefix mobile --ignore-scripts` under npm 10.9.9. The error is `Invalid: lock file's brace-expansion@5.0.12 does not satisfy brace-expansion@5.0.13`. The Extended `minimatch` override is now scoped to the existing patched `brace-expansion 5.0.12` lock entry, preserving the `minimatch 11.0.3` remediation instead of downgrading or suppressing audit checks. A focused lockfile/override consistency regression was added. This is a correction candidate until `npm ci`, Expo, and the exact-new-SHA CI pass; installed-device functionality remains unverified.

## Fork HEAD `961b43fa` dependency continuity — 2026-11-11

The seven source-modified paths in [`961b43fa`](https://github.com/ginmp8/pandacode/commit/961b43fa88ad1d24eb083bc10779cb7034c4a0fd) include **no npm/Cargo dependency manifests or lockfiles** and no version-field changes; upstream native version remains **0.3.111**. The integration [`5beff6d8`](https://github.com/ginmp8/pandacode-extended/commit/5beff6d8737e952341023bfdf10aaeb38697fcae) retains the Extended dependency overrides, patched lockfiles, application identity and security remediations from the previous snapshot without attempting a downgrade. These facts reflect the seven-path source diff and exact tree inventory, **not** a fresh npm audit or RustSec result. Check the new commit's CI before claiming any automated audit success.

## Source 106b3852 dependency continuity — 2026-11-11

The six upstream changed paths do not include application manifests or lockfiles. The inherited native application version remains **0.3.111** in npm, Cargo and Tauri, separate from the planned Extended **0.12.0** distribution. Previously remediated dependencies, overrides, and Extended audit jobs remain unchanged; no newly fixed vulnerability is claimed. Audit and build results must be checked on the exact published commit rather than borrowed from prior commits.

## Source a2ba4b43 dependency continuity — 2026-11-12

The [single source commit](https://github.com/ginmp8/pandacode/commit/a2ba4b430951ee2995bdfda140d4f516cb493a78) edits only `releases/0.3.111.md`. npm/Cargo manifests, lockfiles, overrides, RustSec checks and installed dependency versions do not change; all six native app version fields remain **0.3.111**. No new vulnerability remediation, dependency downgrade or security-audit result is claimed. The new documentation-only destination SHA must be checked separately from previous successful CI.
