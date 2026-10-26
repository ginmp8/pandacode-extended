# PandaCode Extended 0.6.0: incremental integration of original 0.3.48

Issue: #28. Extended source baseline: `main` at `430909614339c58ac4a5beea4baa2707f2af27da`.
Original source baseline `v0.3.47`: `4187553cb48db96657609948f6b0904556845d45`.
Original target `v0.3.48`: `9cc1a9a58d259f7316e4f3ad621e8ca120358d89`.
Original release note Git blob: `639bd02d0f5513aad54bd253c6f496676b824962`.
See [official release](https://github.com/aicodeking/pandacode/releases/tag/v0.3.48) and [compare](https://github.com/aicodeking/pandacode/compare/v0.3.47...v0.3.48).

## Feature mapping

| Change in 0.3.48 | Extended 0.6.0 status | Source |
| --- | --- | --- |
| Align handoff row with conversation column | Included | `src/styles.css` |
| Center disclosure summary and expanded text | Included | `src/styles.css` |
| Bound label overflow on narrow threads | Included | `src/styles.css` |
| Test geometry at 1440 and 760 pixels | Included | `tests/provider-handoff.spec.ts`, Playwright/CI registration |
| Original upstream release note and publish date | Included, kept as upstream source | `releases/0.3.48.md`, `website/src/data/release-dates.json` |
| Original `0.3.48` package/Cargo/Tauri versions | Adapted to independently versioned Extended `0.6.0` | Product manifests, Cargo and Expo |
| Original installer/updater instructions | Excluded by Extended security policy | Existing updater and installer restrictions remain unchanged |

## Example and behavioral contract

```css
.provider-handoff summary {
  display:flex;
  align-items:center;
  justify-content:center;
  width:fit-content;
  max-width:100%;
  margin:0 auto;
}
.provider-handoff summary > span {
  min-width:0;
  overflow:hidden;
  text-overflow:ellipsis;
  white-space:nowrap;
}
.provider-handoff p { max-width:70ch; margin:8px auto; }
```

The disclosure stays a real `<details>` element and the summary remains keyboard accessible. Provider logos, historical provider identity and escaped handoff content must not change. Test geometry independently from screenshot appearance.

## Security, historical identity and exclusions

Keep Extended-only review/coordination, Browser QA, workflows, connected-account isolation, remote device trust, replay prevention, secret storage, default-off remote Full access, explicit approvals, disabled automatic updates and blocked implicit installers. Do not import upstream native settings or new packages. Preserve `HARDENED-REVISION.json`, prior integration provenance and historical notes. Never claim complete code rebase or full behavioral parity.

This increment is bounded to the original 0.3.48 release delta. The complete historical 0.5.0 integration provenance remains in [its original record](upstream-integration-0.5.0.md); no reference ZIP was provided or SHA-256 invented for this incremental review. Full native builds, physical-device Connect, other operating systems and paid provider flows require their own evidence before claiming equivalence.

## Acceptance

Run `npm run build`, `npm test`, `npm run test:release`, `npm run site:build`, the Playwright handoff tests plus existing suites, and CI Rust/Connect jobs. The version/release gate must verify this review's commits, note, continuity and source identities. A passing CI is not proof of untested platforms or upstream full-tree parity.
