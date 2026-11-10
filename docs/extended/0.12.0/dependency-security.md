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
