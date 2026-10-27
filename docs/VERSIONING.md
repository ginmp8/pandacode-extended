# PandaCode Extended versioning and release notes

## Goals and identity

PandaCode Extended is an independently versioned derivative of [PandaCode](https://github.com/aicodeking/pandacode). Its canonical repository is https://github.com/ginmp8/pandacode-extended. Its application version is separate from the original project's version. Original upstream releases are references; implemented features are tracked separately in the current integration coverage document.

**Product SemVer**: `MAJOR.MINOR.PATCH` (e.g. `0.5.0`). Npm, Cargo, Tauri, Cargo.lock, the mobile package/lock and Expo manifest, source release directory, updater/version comparisons (if separately authorized later), and the Git tag (`v0.5.0`) use this three-component core. Git tags and published artifacts are immutable: never rewrite a released version with different bytes.

**Display/distribution identity**: `<extended>+pc.<tracked-upstream>` (e.g. `0.5.1+pc.0.3.48`). `pc` refers to the latest reviewed/tracked original PandaCode version; it does not claim a complete source rebase, binary compatibility or an exact dependency version. SemVer ignores build metadata after `+` when comparing precedence: updating `pc` alone must never create a distinct release under an unchanged Extended core. Fixes to an unpublished candidate keep its approved core version. A distinct release requires a separately approved core version.

**Source provenance**: the historical initial upstream source ancestry is independently recorded as `0.3.28`. Selected later ports do not rewrite that ancestry. The previous candidate combined the `0.3.42` through `0.3.47` integration with a [supplemental `0.3.28` through `0.3.42` review](upstream-parity-0.3.28-0.3.42.md) and preserves intentional Extended differences; full upstream parity remains unclaimed. `HARDENED-REVISION.json` is the immutable historical review record for the earlier integration through `0.3.42`, not the current product version.

`EXTENDED-RELEASE.json` is the current source metadata contract. The product-version script updates its Extended version, display identifier and current release-note path. A product-only patch retains the existing integration coverage document and source identities. Updating the tracked upstream reference requires a reviewed integration, an explicit `reviewed_upstream_range`, immutable source identities in `integration_provenance`, and a coverage document. The release validator checks those separately from the historical review's pinned digest; advancing the active reference must not edit old evidence to match. An upstream announcement alone does not update this metadata.

The `historical_upstream_review` supplement records the separate feature-branch baseline, immutable original tag commits, coverage document and hashed release-evidence manifest for issue #24. It retains the newer integration's provenance and keeps the unpublished candidate at `0.5.0`. Source-only notes do not establish a published release date. The validator checks all 15 original source-note digests in the historical range, including the unpublished 0.3.36 note, without rewriting the legacy review record.

The `early_upstream_review` supplement records [issue #26](https://github.com/ginmp8/pandacode-extended/issues/26), the current feature-branch baseline, the exact original `0.3.27` snapshot and the [review before 0.3.28](upstream-parity-before-0.3.28.md). Its manifest verifies 77 additional source-note digests: 49 source-only notes `0.2.0–0.2.48` and 28 published releases `0.3.0–0.3.27`. The current public inventory does not establish original `0.2.*` tags or publication dates; the snapshot identity must not be presented as their individual release commits. This supplement retains both existing reviews, the tracked `0.3.47` reference and the `0.3.28` ancestry. The combined validator covers 97 distinct original notes without requiring invented dates for source-only evidence.

## Current release candidate

| Status | Technical version | Display identifier |
| --- | --- | --- |
| [Published Extended release](https://github.com/ginmp8/pandacode-extended/releases/tag/v0.4.0), 2026-10-25 | `0.4.0` | `0.4.0+pc.0.3.42` |
| Previously prepared Extended candidate, retained | `0.5.0` | `0.5.0+pc.0.3.47` |
| Previous upstream 0.3.48 integration candidate, not published | `0.5.1` | `0.5.1+pc.0.3.48` |
| Previous three-fix candidate, not published | `0.5.2` | `0.5.2+pc.0.3.48` |
| Current upstream integration candidate (not shipped or fully verified) | `0.6.0` | `0.6.0+pc.0.3.50` |

The historical 0.5.2 candidate addresses dark-theme native select menus, manual Board stage management, and bot-owned deleted conversation cleanup (issues #32, #33, and #34). It preserves the tracked 0.3.48 upstream reference and all earlier source evidence; this candidate is unpublished and untagged.

The previous candidate used `0.5.0` for the upstream feature integration tracked in [issue #22](https://github.com/ginmp8/pandacode-extended/issues/22) and its historical coverage supplements in [issue #24](https://github.com/ginmp8/pandacode-extended/issues/24) and [issue #26](https://github.com/ginmp8/pandacode-extended/issues/26). The original corrections remained part of `0.5.0`; that historical candidate stayed at `0.5.1`. The current unpublished candidate must remain at `0.6.0`; individual corrective commits do not create patch releases. Keep the existing `0.4.0` release, tag, note and date unchanged. No tag for an unpublished candidate is created without explicit publication authorization. Incremental source evidence is in [0.5.1 coverage](upstream-integration-0.5.1.md), and the prior 0.5.0 coverage is retained.

**After publication**, version selection for a distinct future release must be decided from its actual changes and consumer compatibility. Corrections normally use PATCH, compatible additions use MINOR, and incompatible changes require a migration assessment. This document deliberately does not preassign numbers to hypothetical releases.

## Source history layout and dates

```text
releases/
  0.3.42.md                    # Original PandaCode note, unchanged
  extended/
    0.3.42-hardened.md         # Legacy Extended note, retained
    0.4.0.md                   # Published Extended history, retained
    0.5.0.md                   # Historical Extended candidate note
    0.6.0.md                   # Current Extended candidate note
```

- Original PandaCode notes stay at `releases/<upstream-version>.md` and keep existing `website/src/data/release-dates.json` keys such as `0.3.42`.
- All local/Extended notes, including historical `hardened` files, live at `releases/extended/<extended-version>.md`; date keys are **`extended/<extended-version>`** (e.g. `extended/0.5.0`). This avoids collisions when the original and Extended use the same version number.
- The changelog identifies a note's source **from its directory**, not its version number or heading. The display version is taken from the current metadata for the active Extended release.
- Keep historical note content, attribution and original dates for documented releases. Missing historical dates remain unknown; never fabricate them to pass a test. Only the explicitly requested current candidate may have a source note before publication; abandoned intermediate version aliases must not appear as releases. A source-note date records preparation and does not certify binary publication. The Extended release line starts with `0.4.0`.
- Moving notes changes raw GitHub paths. The maintained links are corrected, but external links to the old paths are not automatically redirected by GitHub; use the mapping above to find the new locations.
- Git tags and GitHub Releases in this repository, when explicitly created, represent **Extended**, not the mirrored upstream note.

## Procedure (no auto-publication)

1. Decide the next Extended SemVer from actual product/consumer changes. For an upstream integration, record the reviewed release range, source commits, reference-archive digest and coverage document separately from historical ancestry.
2. On a disposable checkout run `npm run release:version -- X.Y.Z`; this updates desktop and mobile npm manifests/locks, Tauri, Cargo, the Expo version, `EXTENDED-RELEASE.json`, and the namespaced website release date. Mobile platform build counters remain an explicit packaging decision; this command does not enable Expo updates or configure publication.
3. Write the user-facing `releases/extended/X.Y.Z.md`, with highlights, risks, upstream reference, evidence limitations and migration instructions where needed. Do not claim tests, signatures or source parity not actually verified.
4. Run `npm run build`, `npm test`, `npm run test:release`, `npm run site:build`, and supported native/test checks. Keep the PR unmerged if required checks fail or remain unknown.
5. Only when explicitly authorized to ship: tag `vX.Y.Z`, run the local-only reviewed build process and document SHA-256/source identity. No automatic GitHub Release, updater feed or npm publication.

## Active bundle validation

The Extended desktop release builder rejects a version that does not have exactly three numeric SemVer components. It requires a matching `PandaCode Extended` bundle name and rejects original-PandaCode bundle names, stale versions and mismatched macOS bundle versions. The `+pc.<upstream>` suffix is display-only and must not be inserted into npm/Tauri/Cargo versions, native bundle filenames or release tags. Historical four-component/hardened filenames remain readable for archive and provenance purposes only; they are not valid inputs to the active Extended build.

**Compatibility guardrails:** keep bundle ID `ai.pandacode.desktop`, local data/credential paths, `pandacode` package/CLI/tool identifiers, previous license attribution and current updater/permission restrictions unless separately reviewed. Changing these would require its own migration and validation.

## 0.6.0 candidate source lineage

Issue #38 tracks 0.3.49 Connect and issue #39 tracks 0.3.50 traces. The single review branch and PR #40 (ready for review, not merged) use source snapshots pinned to upstream commits `e71acdda9eb5d488122081c4dbc49ee1a5b4f9e2` and `15541a51f3f5d8ee32d91349f9d5035a53c4cd19`. Historical 0.3.48 coverage remains immutable; new source identities and unresolved verification risks are recorded in [the 0.6.0 assessment](upstream-integration-0.6.0.md). No tag, artifact, updater or published release is authorized. The owner authorized Ready for review as an operational review state. That authorization does not waive implementation, security, upstream-parity or physical-device gates, and does not authorize merge or release. Corrective commits for #41/#42 retain `0.6.0`.
