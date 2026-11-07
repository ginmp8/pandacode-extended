# Extended 0.12.0 — known issues and acceptance gaps

**Status:** Initial register inherited from [0.11.0](../0.11.0/known-issues.md). An unverified scenario is **not** a confirmed bug. Do not silently declare prior gaps fixed or regressions present.

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

## Baseline provenance

- [0.11.0 known issues](../0.11.0/known-issues.md)
- [0.11.0 release readiness](../0.11.0/release-checklist.md)
- [0.11.0 native/application release identity](../releasing.md)

As new evidence arrives, distinguish **observed failures**, **known inherited limitations**, **planned tests** and **validated fixes**. Do not disable upstream functionality in lieu of a justified correction.
