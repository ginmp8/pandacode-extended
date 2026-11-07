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

## 0.12.0 Extended feature gaps and known limits

| Feature | Current source evidence | Pending native evidence / limitation |
| --- | --- | --- |
| [#121 — History preferences](../../extended-bot-context.md) | Per-bot defaults, member-scoped per-channel overrides and focused tests are present on `extended/development`. | Validate migration, retained preferences, restart, UI precedence, membership removal, archived channels, queued work and native context. **History exclusion is not complete context or memory isolation.** |
| [#122 — Cross-channel collaboration](../../extended-bot-collaboration.md) | Bot post/request tools, attributed timelines, tracking and recovery logic exist in source. | Validate actual multi-channel interactions, no accidental work or broadcast, idempotency, stop/restart, loop limits and private direct requests. **Callbacks are at-most-once attempts:** interruption between claim and persistence may leave a callback incomplete without rerunning recipients. |
| [#123 — Portable manifests](../../extended-bot-manifests.md) | Desktop/headless adapters, pre-import backup gate and SHA-pinned GitHub reads exist in source. | Verify cross-machine round-trips, partial import, backup failure, safe external-channel settings, source integrity and access. **No automatic Git polling**; configuration backups cannot restore credentials, messages, workspaces, memories, pairing or live work. |

No new confirmed defect is inferred from these source or acceptance limitations. See the [0.12.0 checklist](release-checklist.md) for the required evidence.

## Baseline provenance

- [0.11.0 known issues](../0.11.0/known-issues.md)
- [0.11.0 release readiness](../0.11.0/release-checklist.md)
- [0.11.0 native/application release identity](../releasing.md)

As new evidence arrives, distinguish **observed failures**, **known inherited limitations**, **planned tests** and **validated fixes**. Do not disable upstream functionality in lieu of a justified correction.
