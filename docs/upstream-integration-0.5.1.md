# PandaCode Extended 0.5.1 — incremental upstream 0.3.48 review

## Frozen source identities

- Extended base `main`: `430909614339c58ac4a5beea4baa2707f2af27da`
- Upstream previous tag `v0.3.47`: `4187553cb48db96657609948f6b0904556845d45`
- Upstream target tag `v0.3.48`: `9cc1a9a58d259f7316e4f3ad621e8ca120358d89`
- Original release-note Git blob: `639bd02d0f5513aad54bd253c6f496676b824962` at `releases/0.3.48.md`.
- Issue: [#30](https://github.com/ginmp8/pandacode-extended/issues/30).
- Original source comparison: [0.3.47...0.3.48](https://github.com/aicodeking/pandacode/compare/v0.3.47...v0.3.48).
- Announcement: [v0.3.48](https://github.com/aicodeking/pandacode/releases/tag/v0.3.48) on 2026-10-26.

The original delta modifies 10 paths: the only production UI change is `src/styles.css`; upstream also adds `tests/provider-handoff.spec.ts`, updates Playwright registration, publishes the release note and date, and bumps upstream package/Cargo/Tauri versions. No new provider, remote transport, credential flow or IPC endpoint is introduced in this tagged delta.

## Feature coverage and tests

| Change | Extended path | Port status |
| --- | --- | --- |
| Center the `Conversation carried forward` row | `src/styles.css` | Implemented; remove fixed 34px left offset; centered summary, truncation and max-width detail |
| Browser geometry at 1440px and 760px | `tests/provider-handoff.spec.ts`, `playwright.config.ts` | Added matching upstream geometry assertions |
| Upstream release history | `releases/0.3.48.md`, `website/src/data/release-dates.json` | Mirrored original note, separate Extended date |
| Extended technical version | npm desktop/mobile, Cargo/Tauri, Expo manifests and locks | `0.5.1`, not `0.3.48` |
| Display identity | `EXTENDED-RELEASE.json` | `0.5.1+pc.0.3.48`; original source ancestry remains `0.3.28` |
| CI throughput | `.github/workflows/ci.yml` | Measured experiment: browser matrix shards; repeated smoke tests retained |

### Security exclusions and preserved behavior

Do not copy upstream installation/update settings or release automation. Preserve the manually reviewed distribution policy, disabled updater, CSP, exact IPC authorization, project/terminal protections, secure secret stores, remote device trust and approved-only elevated permissions. Retain Extended task and GitHub integration features, its historical test suite and its own release identity. This source-only integration does **not** claim a complete upstream rebase, runtime equivalence on every platform or native installer validation.

### Provenance and validation boundary

The existing `integration_provenance` document and archived `0.3.42–0.3.47` comparison/hash describe the previous reviewed integration. They are **not** an archive of upstream 0.3.48 and must not be renamed or fabricated. The new `incremental_upstream_review` separately pins the exact old/new upstream tag commits, this Extended base and the actual Git blob of the original 0.3.48 note. Release tests validate those identities. The existing historical reviews and upstream note digests remain unchanged.

### CI performance experiment

Baseline [run 40268273421](https://github.com/ginmp8/pandacode-extended/actions/runs/40268273421): desktop browser 467 s (155 s twice-run smoke + 285 s integration); historical browser 495 s (464 s test), Rust 411 s (80 s dependencies + 184 s tests + 92 s Clippy), web 39 s, mobile 45 s, relay 12 s. Wall-clock critical path ~495 s. New CI shards the desktop integration list into 2 jobs and the historical list into 3 jobs, each with 4 Playwright workers; dedicated chat smoke retains `--repeat-each=2 --workers=1`. All original spec lists remain in the workflow, with new responsive handoff coverage; diagnostic artifact names are shard-specific. Rust is intentionally not split yet, because its native setup and compilation would be duplicated.

**Hypothesis, not verified improvement:** browser shards may approach ~2-minute test steps, but setup, runner limits and test contention may dominate. Compare actual job timings and failures from the new PR CI to this frozen baseline; tune workers and sharding only from evidence. Never reduce tests or assertions just to meet a timing target.
