# Extended 0.12.0 — known issues and acceptance gaps

**Status:** The initial register was inherited from [0.11.0](../0.11.0/known-issues.md); 0.12.0 Extended-only acceptance gaps are now recorded below. Unverified scenarios are **not** confirmed defects.

| Area | Opening classification | Evidence needed before 0.12.0 release |
| --- | --- | --- |
| Full upstream functional parity | Static/source checks were recorded in 0.11.0; remaining shared divergence and live behavior are unverified | Fresh complete tree, file modes/binaries, hunk-level explanations and Settings On/Off tests |
| Bot and channel context | Fresh-context and shared-channel preferences inherited from 0.11.0 | Real delegation, fresh-context, channel history, restart, queue and permissions regression |
| Account pools and sign-in | Imported upstream features, native multi-account validation not documented as complete | Live authorized Claude/Codex accounts, usage/limit transitions, restart, queue preservation and provider UI |
| Mobile redesign | Source inherited in 0.11.0; real device behavior not verified | iOS/Android navigation, authentication, notifications, badge and TestFlight usage |
| Connect, MCP, cloud/VM, credentials | No full native acceptance recorded | Test both settings states, approvals, remote authorization and recovery |
| Existing installations / data migrations | Acceptance pending | Representative SQLite upgrades, backup/recovery, rollback and native platform installation |
| Independent Extended packaging | Tag/version workflow patched in 0.11.0, but runtime auto-update behavior has limitations | Real installers, updater manifest, signing and differing Extended tag vs upstream app-version behavior |
| Confirmed 0.12.0-specific defects | None at cycle opening | Reproduction, impacted SHA, fix and regression evidence |
| ZCode participation in agent teams (inherited upstream 0.3.102 limitation) | ZCode supports single-agent tasks; automatic teams exclude it, and a team with completed worker activity cannot be switched to ZCode in the same task | Verify the imported model-picker and team-catalog regression tests and test single-agent/team transitions in native app; do not treat the upstream harness limitation as an Extended regression |

## 0.12.0 Extended feature gaps and known limits

| Feature | Current source evidence | Pending native evidence / limitation |
| --- | --- | --- |
| [#121 — History preferences](../../extended-bot-context.md) | Per-bot defaults, member-scoped per-channel overrides and focused tests are present on `extended/development`. | Validate migration, retained preferences, restart, UI precedence, membership removal, archived channels, queued work and native context. **History exclusion is not complete context or memory isolation.** |
| [#122 — Cross-channel collaboration](../../extended-bot-collaboration.md) | Bot post/request tools, attributed timelines, tracking and recovery logic exist in source. | Validate actual multi-channel interactions, no accidental work or broadcast, idempotency, stop/restart, loop limits and private direct requests. **Recovery uses native admission checks:** interrupted callback claims are retried only when no matching queued/recorded callback exists. The recipient's completed task is not rerun to recover its callback; exactly-once delivery across uncertain native launches remains unverified. |
| [#123 — Portable manifests](../../extended-bot-manifests.md) | Desktop/headless adapters, pre-import backup gate and SHA-pinned GitHub reads exist in source. | Verify cross-machine round-trips, partial import, backup failure, safe external-channel settings, source integrity and access. Pre-update validation rejects invalid Extended settings; source-content and local-state checks avoid redundant Git applies. Playwright-focused UI testing is added to CI, but native end-to-end restoration remains unverified. **No automatic Git polling**; configuration backups cannot restore credentials, messages, workspaces, memories, pairing or live work. |

Code-level fixes were added for audit findings affecting callback recovery and manifest prevalidation/sync; unresolved native acceptance scenarios are not silently marked complete. See the [0.12.0 checklist](release-checklist.md) for the required evidence.

## Baseline provenance

- [0.11.0 known issues](../0.11.0/known-issues.md)
- [0.11.0 release readiness](../0.11.0/release-checklist.md)
- [0.11.0 native/application release identity](../releasing.md)

As new evidence arrives, distinguish **observed failures**, **known inherited limitations**, **planned tests** and **validated fixes**. Do not disable upstream functionality in lieu of a justified correction.

## PandaCode 0.3.103 integration

Council's scored review/fix rounds, composer panel, controls and stale-score tracking were copied from the fork's current HEAD, with original tests. **Known upstream limitations:** paid-model review/fix sessions have not been validated; external file edits and shell/script changes not reported through file tracking may not invalidate a score. Native Settings On/Off interactions, cancellation/restart, resume, Connect/MCP/cloud permissions and real platform behavior are **unverified**, not confirmed regressions. The 80 previously customized shared source files have not been exhaustively accepted hunk by hunk.
