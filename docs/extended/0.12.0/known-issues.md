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
| ZCode participation in agent teams (inherited upstream 0.3.102 limitation) | ZCode supports single-agent tasks; automatic teams exclude it, and a team with completed worker activity cannot be switched to ZCode in the same task | Verify the imported model-picker and team-catalog regression tests and test single-agent/team transitions in native app; do not treat the upstream harness limitation as an Extended regression |

## Baseline provenance

- [0.11.0 known issues](../0.11.0/known-issues.md)
- [0.11.0 release readiness](../0.11.0/release-checklist.md)
- [0.11.0 native/application release identity](../releasing.md)

As new evidence arrives, distinguish **observed failures**, **known inherited limitations**, **planned tests** and **validated fixes**. Do not disable upstream functionality in lieu of a justified correction.

## PandaCode 0.3.103 integration

Council's scored review/fix rounds, composer panel, controls and stale-score tracking were copied from the fork's current HEAD, with original tests. **Known upstream limitations:** paid-model review/fix sessions have not been validated; external file edits and shell/script changes not reported through file tracking may not invalidate a score. Native Settings On/Off interactions, cancellation/restart, resume, Connect/MCP/cloud permissions and real platform behavior are **unverified**, not confirmed regressions. The 80 previously customized shared source files have not been exhaustively accepted hunk by hunk.

## PandaCode 0.3.104 acceptance gaps

- **Unverified, not confirmed defects:** desktop panda startup/replay, star-button external link behavior, keyboard access, reduced motion, sidebar sizing and mobile shared-source build behavior have only imported source tests and code review, not independent native-app validation.
- **Static parity:** at the [integrated source SHA](https://github.com/ginmp8/pandacode/commit/d5d51769ccac15145e67c071ea06a5502c4cadcb) all 1,561 tracked upstream blobs have corresponding paths; the 80 existing Extended customized shared files still lack exhaustive hunk-by-hunk and real Settings On/Off acceptance. This is not a confirmed loss of functionality.
- **CI:** code-commit [push run #42503275362](https://github.com/ginmp8/pandacode-extended/actions/runs/42503275362) and [PR run #42503280072](https://github.com/ginmp8/pandacode-extended/actions/runs/42503280072) were still running when initially checked. Validate final documentation SHA independently before calling the candidate build green.

## PandaCode 0.3.105 inherited-account acceptance (2026-11-08)

- **Observed code incorporation, not a confirmed bug:** all 38 changed paths from [source `f28c4df0`](https://github.com/ginmp8/pandacode/commit/f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665) were reconciled in [Extended `36228b94`](https://github.com/ginmp8/pandacode-extended/commit/36228b949aaf8648109e2f8f8da441a8735cee41). No upstream files were omitted from the complete static tree. Eight files required local reconciliation, retaining prior Extended behavior.
- **Unverified runtime scenarios:** actual Claude/Codex credentials across distinct config folders and subscriptions, token precedence, identity refresh while requests are in flight, opt-in account pooling, handoff/resume, compact expandable account-switch notices and /btw isolation across task changes; repeat with existing Extended Settings On/Off and real authorized provider/native builds.
- **Evidence boundary:** static tree parity and imported tests cannot prove runtime behavior. No local tests executed; CI was pending on the exact code commit at first observation. The 80 shared-file differences still require complete code-level justification and functional acceptance.


## Upstream 0.3.106 acceptance status — 2026-11-09

**No new confirmed Extended defect is asserted by this integration.** The source introduces transcript/final answer following, accessibility-aware themed headers, RTL prose, non-English keyboard shortcuts, recovery from inspector task-load failures, and task tools for every supported harness. Their source code and regression tests have been staged, but native/Playwright behavior has not been independently exercised with Extended controls. The 80 source-divergent Extended shared paths still need full hunk-level semantic classification. Missing scanner coverage is now explicitly a failed assurance step rather than an apparent success; this does not substitute for an SBOM or actual security scan.

## Exact-SHA CI security finding — 2026-11-09

At [Extended integration `34d8fb4c`](https://github.com/ginmp8/pandacode-extended/commit/34d8fb4caa985f2ceb4815945dc58ed3042c8353) (upstream [`0bfd5738`](https://github.com/ginmp8/pandacode/commit/0bfd57383621d5aa001d3871018920855cabec39), app **0.3.106**), [push CI #42644925066](https://github.com/ginmp8/pandacode-extended/actions/runs/42644925066) and [PR CI #42644937837](https://github.com/ginmp8/pandacode-extended/actions/runs/42644937837) both **failed**: five of six jobs succeeded in each, but **RustSec audit failed**. The lockfile contains `rusqlite 0.38.0` and `libsqlite3-sys 0.36.0`, whose bundled SQLite is affected by [RUSTSEC-2026-0459 / CVE-2026-119375](https://github.com/RustSec/advisory-db/blob/main/crates/libsqlite3-sys/RUSTSEC-2026-0459.md) (CVSS 6.8). The advisory identifies patched `libsqlite3-sys >=0.39.0`, released with `rusqlite 0.41`; the application explicitly enables `rusqlite`'s `bundled` feature. Its exposure involves opening a crafted local database plus WAL file; the failed audit does not independently establish exploitability in a deployed installation.

**Unresolved, release-blocking dependency remediation:** upgrade the compatible `rusqlite`/SQLite dependency graph, regenerate and commit `src-tauri/Cargo.lock` using Cargo (not hand-edited), verify the Rust toolchain floor (`rusqlite 0.41` requires Rust 1.89), check any affected SQLite API/storage and migration paths, and rerun `cargo check --locked`, relevant Rust/SQLite regression tests, and `cargo audit --file src-tauri/Cargo.lock` before treating the finding as fixed. The local sandbox has no Cargo and cannot resolve GitHub DNS, so no validated lockfile regeneration or Rust test was possible. No vulnerability suppression, CI bypass, or dependency-version-only edit was applied. The source and destination Git trees still have **zero missing upstream paths**, but this does not prove functional equivalence of the 80 customized shared files or validate Settings/Connect/MCP/cloud/VM native behavior. Planned Extended distribution **0.12.0** remains unapproved; checks on any documentation-only follow-up SHA are separate.
