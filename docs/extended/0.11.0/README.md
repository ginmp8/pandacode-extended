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

## First imported upstream update — 2026-11-06

- Source: [`ginmp8/pandacode:main` at `e0b65e51`](https://github.com/ginmp8/pandacode/commit/e0b65e51c4b6bc9c67982eaed08f7f13985fa01a), upstream app **0.3.100**.
- Exact code integration: [`ginmp8/pandacode-extended:base/development` at `a28ce807`](https://github.com/ginmp8/pandacode-extended/commit/a28ce807a9bcc3a8583440ff6035deb1fd0854da); **7 added files** and **18 modified files**, including upstream release notes, account-pool runtime, connection UI, integration tests and six application-version fields.
- The source's account pools and CLI sign-in are **inherited features**. No new Extended-only control or dependency remediation is claimed. Existing Extended controls remain present; end-to-end validation on actual accounts and both On/Off states is not established by static copying.
- [Original 0.3.100 release notes](../../../releases/0.3.100.md) and [Extended 0.11.0 draft](../../../releases/extended/0.11.0.md).

The whole-tree comparison after code integration found no missing source paths. The remaining shared changes must still be classified by hunk and exercised functionally; CI and native validation remain independent acceptance criteria.

## Next inherited upstream integration — PandaCode 0.3.101

- [Source `008017de`](https://github.com/ginmp8/pandacode/commit/008017de3316aea56af9294d4d3f8556b52174bb) → [Extended code integration `d7cf1546`](https://github.com/ginmp8/pandacode-extended/commit/d7cf15467bf8f25563db253e7b35a1efeb2e5b5e).
- 3 source-exact files added and 24 existing paths patched; upstream Providers account cards and Usage panel now share account-status UI, identity metadata and usage-meter behavior. See [original upstream notes](../../../releases/0.3.101.md).
- The nontruncated recursive tree comparison found 0 missing source paths and 80 shared divergences. Existing Extended-only paths and feature-control modules remain; native functional parity and release approval are still pending.
