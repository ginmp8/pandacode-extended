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
