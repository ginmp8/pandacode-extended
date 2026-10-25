# PandaCode Extended versioning and release notes

## Goals and identity

PandaCode Extended is an independently versioned derivative of [PandaCode](https://github.com/aicodeking/pandacode). Its canonical repository URL is https://github.com/ginmp8/pandacode-extended (pending the rename of the currently hosted `ginmp8/pandacode-hardened` repository). Its application version is separate from the original project's version. The original upstream releases are *references*, not evidence that all their features or commits have been merged into Extended.

**Product SemVer**: `MAJOR.MINOR.PATCH` (e.g. `0.4.0`). Npm, Cargo, Tauri, Cargo.lock, source release directory, updater/version comparisons (if separately authorized later), and the Git tag (`v0.4.0`) use this three-component core. Git tags and published artifacts are immutable: never rewrite a released version with different bytes.

**Display/distribution identity**: `<extended>+pc.<tracked-upstream>` (e.g. `0.4.0+pc.0.3.42`). `pc` refers to the latest *reviewed/tracked original PandaCode version*, not a full source rebase, binary compatibility guarantee, or exact dependency version. SemVer ignores build metadata after `+` when comparing precedence: updating `pc` alone must **never** create a distinct release under an unchanged Extended core. While the first `0.4.0` release is still unpublished, corrections on `release/0.4.0` keep the same core version. Only after a release is actually published should a later, distinct release receive a separately approved new core version.

**Source provenance**: the historical initial upstream source ancestry is independently recorded (currently `0.3.28`). Selected patches and upstream changes tracked through `0.3.42` have not established complete `0.3.42` source parity. Keep provenance truthful and security evidence immutable. `HARDENED-REVISION.json` remains a historical review record, not the current product version.

`EXTENDED-RELEASE.json` is the current source metadata contract. The product-version script updates its Extended version, display identifier and current release-note path. Updating the tracked upstream reference requires a separate vetted integration/decision, explicit metadata update and validation; it is **not** inferred from an upstream announcement.

## Current release candidate

| Status | Technical version | Display identifier |
| --- | --- | --- |
| First Extended release — not yet published | `0.4.0` | `0.4.0+pc.0.3.42` |

**Until this release is actually published**, all corrections, adjustments and review fixes on `release/0.4.0` remain part of **the same version `0.4.0`**. Neither a new tag nor a new release version is created for each corrective commit. The planned tag is `v0.4.0`, and it must not be created before publication is explicitly authorized.

**After publication**, version selection for a distinct future release must be decided from its actual changes and consumer compatibility. Corrections normally use PATCH, compatible additions use MINOR, and incompatible changes require a migration assessment. This document deliberately does not preassign numbers to hypothetical releases.

## Source history layout and dates

```text
releases/
  0.3.42.md                    # Original PandaCode note, unchanged
  extended/
    0.3.42-hardened.md         # Legacy Extended note, retained
    0.4.0.md                   # New Extended source note
```

- Original PandaCode notes stay at `releases/<upstream-version>.md` and keep existing `website/src/data/release-dates.json` keys such as `0.3.42`.
- All local/Extended notes, including historical `hardened` files, live at `releases/extended/<extended-version>.md`; date keys are **`extended/<extended-version>`** (e.g. `extended/0.4.0`). This avoids future collisions if the original and Extended both use `0.4.0`.
- The changelog identifies a note's source **from its directory**, not its version number or heading. The display version is taken from the current metadata for the active Extended release.
- Keep historical note content, attribution and original dates for documented releases. Missing historical dates remain unknown; never fabricate them to pass a test. Unpublished intermediate candidates must not appear in release history. The Extended release line starts with `0.4.0`.
- Moving notes changes raw GitHub paths. The maintained links are corrected, but external links to the old paths are not automatically redirected by GitHub; use the mapping above to find the new locations.
- Git tags and GitHub Releases in this repository, when explicitly created, represent **Extended**, not the mirrored upstream note.

## Procedure (no auto-publication)

1. Decide the next Extended SemVer from actual product/consumer changes, and review integration provenance separately from version tracking.
2. On a disposable checkout run `npm run release:version -- X.Y.Z`; this updates npm, Tauri, Cargo, lockfiles, `EXTENDED-RELEASE.json`, and the namespaced website release date.
3. Write the user-facing `releases/extended/X.Y.Z.md`, with highlights, risks, upstream reference, evidence limitations and migration instructions where needed. Do not claim tests, signatures or source parity not actually verified.
4. Run `npm run build`, `npm test`, `npm run test:release`, `npm run site:build`, and supported native/test checks. Keep the PR unmerged if required checks fail or remain unknown.
5. Only when explicitly authorized to ship: tag `vX.Y.Z`, run the local-only reviewed build process and document SHA-256/source identity. No automatic GitHub Release, updater feed or npm publication.

## Active bundle validation

The Extended desktop release builder rejects a version that does not have exactly three numeric SemVer components. It requires a matching `PandaCode Extended` bundle name and rejects original-PandaCode bundle names, stale versions and mismatched macOS bundle versions. The `+pc.<upstream>` suffix is display-only and must not be inserted into npm/Tauri/Cargo versions, native bundle filenames or release tags. Historical four-component/hardened filenames remain readable for archive and provenance purposes only; they are not valid inputs to the active Extended build.

**Compatibility guardrails:** keep bundle ID `ai.pandacode.desktop`, local data/credential paths, `pandacode` package/CLI/tool identifiers, previous license attribution and current updater/permission restrictions unless separately reviewed. Changing these would require its own migration and validation.
