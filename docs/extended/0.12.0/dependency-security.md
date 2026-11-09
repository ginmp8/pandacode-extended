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
