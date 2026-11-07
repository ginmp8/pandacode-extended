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
