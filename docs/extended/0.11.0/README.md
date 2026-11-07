# PandaCode Extended 0.11.0 — incremental development cycle

**Status:** Open for incremental changes; not released or approved.

Extended `0.11.0` is the planned release-documentation cycle following [0.10.0](../0.10.0/README.md). The [release notes](../../../releases/extended/0.11.0.md) start as a draft, and actual scope will grow with verified PandaCode fork `main` changes and Extended-only corrections.

## Operating rules

1. Read the fork `ginmp8/pandacode:main` at its current full SHA for **every** import; include unreleased commits, all files, assets, tests, scripts, manifests, releases and relevant binary objects.
2. Apply real source deltas to **`ginmp8/pandacode-extended:base/development`** only. Preserve optional Extended controls, security fixes, data compatibility, and additive Extended features. Do not copy `main` back onto `base/hardened`.
3. Record each verified change with source/target commit SHAs and validation outcomes; distinguish upstream-inherited changes from Extended-specific changes. Never relabel earlier-cycle behavior as newly developed in 0.11.0.
4. Keep the open `base/development` → `base/hardened` release PR generic and incremental. PR review/merge remains separate from the scheduled import routine.
5. Keep upstream application-version fields synchronized to the fork; never overwrite them with Extended distribution `0.11.0`.

## Current records

- [Draft release notes](../../../releases/extended/0.11.0.md)
- [Feature controls](feature-controls.md)
- [Dependency/security evidence](dependency-security.md)
- [Known issues and unverified risks](known-issues.md)
- [Readiness checklist](release-checklist.md)
- [Full upstream parity contract](../upstream/parity-contract.md)
- [Previous 0.10.0 notes](../../../releases/extended/0.10.0.md)

Initial Extended reference: `3eda289623850405893455e952ace45b6e0a4d89`; upstream source reference at opening: `6b94bf1d8b579d08834578f6e24684ea0d3fccbd` (application 0.3.99). These are **historical baselines**, not pins for future sync.
