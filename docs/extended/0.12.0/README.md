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
