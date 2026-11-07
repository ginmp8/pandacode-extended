# PandaCode Extended 0.12.0 — incremental development cycle

**Status (opened 2026-11-07):** Source changes for three Extended-only features are integrated on `extended/development` in [draft PR #120](https://github.com/ginmp8/pandacode-extended/pull/120). Native acceptance, release approval, tagging, installers and distribution remain pending.

Extended 0.12.0 starts from the completed [0.11.0 source baseline](../0.11.0/README.md). The opening `base/development` commit has `base/hardened` `642b25777b5a3d1600107ff4b9b23604ea90e8a2` as its sole parent. The 0.11.0 `main` has an equivalent Git tree but different rebase commit history. The cycle opened with documentation scaffolding, **not an upstream source-version bump**. Extended-specific development for issues #121–#123 followed on `extended/development`; `base/development` remains the upstream synchronization branch.

## Operating rules

1. For every upstream import, read the **current** `ginmp8/pandacode:main` HEAD, including unreleased commits; assess its complete tracked source tree, tests, assets, scripts, releases, binary objects, modes and manifest versions.
2. Apply upstream integrations **only to `ginmp8/pandacode-extended:base/development`**; keep all upstream capabilities present and functional when explicitly enabled and authorized.
3. Preserve Extended-only Settings controls, native compatibility fixes, added features and verified security remediations; classify each customized shared-file hunk and validate both control states.
4. Log each imported fork SHA, Extended integration SHA, path-level exceptions and exact-SHA tests. Do not count 0.11.0 functionality again as new 0.12.0 functionality.
5. Keep Extended release documentation `0.12.0` separate from the fork's native version (0.3.101 at cycle opening). Do not edit upstream manifests to force a new Extended version.
6. Promote releases using `base/development` → `base/hardened` → `main` with **rebase-only** integrations; no merge/squash commits and no reverse sync from `main` into `base/hardened`. A separate linear replay branch may be used to promote an identical tree to `main` if historical rebase SHAs prevent a direct PR.
7. Never publish tags, GitHub Releases or installers as part of routine upstream synchronization or documentation changes.

## Extended-only source changes

| Issue | Feature and user workflow | Reference |
| --- | --- | --- |
| [#121](https://github.com/ginmp8/pandacode-extended/issues/121) | **Per-channel history.** In **Bot Settings → Shared Channels**, set **Include channel history by default** and choose **Use default**, **Include** or **Exclude** for each joined channel. Overrides affect newly composed assignments only, not already queued work, stored posts, harness history or memories. | [Context and history policy](../../extended-bot-context.md) |
| [#122](https://github.com/ginmp8/pandacode-extended/issues/122) | **Explicit bot collaboration.** Bot-owned tools post informational updates or request work from one/two specified members of one local Shared Channel. Requests are attributed, correlated and bounded; ordinary direct bot messages remain private to their thread. Membership never triggers a broadcast. | [Tools, routing, retries and recovery](../../extended-bot-collaboration.md) |
| [#123](https://github.com/ginmp8/pandacode-extended/issues/123) | **Bot manifests and Git sources.** Use **Bots → + → Import / Sync bots…** to import local JSON or configure and manually sync GitHub sources; **Export bots…** exports selected bot configurations. Headless import/export and one-shot `pandacode serve --sync-bots` share the importer. | [Manifest format, backups and sync](../../extended-bot-manifests.md) |

These features are Extended-owned and are **not** inherited upstream capabilities. History controls do not guarantee strict context isolation; communication tools do not grant additional permissions. Manifest backups preserve configuration only, excluding credentials, conversations and runtime history; GitHub sync never pushes or polls automatically.

The release [checklist](release-checklist.md) and [known-issues register](known-issues.md) keep native, restart, migration and cross-machine acceptance explicitly pending. PR #120 integrates `extended/development` toward `main` after review; this does not authorize merging or alter upstream synchronization and release-promotion rules.

## Current records

- [Draft 0.12.0 release notes](../../../releases/extended/0.12.0.md)
- [#121 — Bot context and Shared Channel history](../../extended-bot-context.md)
- [#122 — Explicit local channel collaboration](../../extended-bot-collaboration.md)
- [#123 — Bot manifests and Git synchronization](../../extended-bot-manifests.md)
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
