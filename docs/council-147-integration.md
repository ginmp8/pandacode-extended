# Council #147: two-PR integration ledger

## Frozen sources and branch policy

- Canonical implementation: [PR #152](https://github.com/ginmp8/pandacode-extended/pull/152), commit `455774df73fad829defd5765c89f91a601cd6297`.
- Additional hardening: [PR #149](https://github.com/ginmp8/pandacode-extended/pull/149), commit `664c530152693f403199496a450ce17df550cf74`.
- New destination: `feat/issue-147-council-unified`; target `extended/development`, base `f9efb3a1c7558dcf7d5688bb1687cfff9253f967`.
- Every integration commit has one parent and follows Conventional Commits; the source branches are never modified.
- **Draft only**: source integration and issue acceptance are not interchangeable.

## Canonical native implementation

Retain PR #152's typed `CouncilComposition`, native isolated child Architects, root/child receipts, strategy, criteria, resource usage, evidence, task adapter, UI and Shared Channel behavior as the single runtime Council. PR #149 uses a different `CouncilConfig` schema and alternative modules for reservations, request/receipt, criteria editor and inspector. Directly combining these would create a competing engine and incorrect task authority.

## Ported native hardening from #149

1. Finite root worker concurrency: for composed Councils with `maxParallel = 0`, cap new root admission at eight simultaneous workers. Lead capacity disclosures and queue reasons use the same effective limit. Preserve original M1/M2 unlimited behavior and explicit child-leaf concurrency.
2. Bounded actual root attempts: count all Council worker admissions in the current root cycle, including failed/abandoned workers, and fail closed after 1,024 attempts. The check runs before reviewer binding and is independent of worker keys; it does not change M1/M2 or the existing per-round child limit.
3. Native Rust regression tests cover the boundary and compatibility behavior.

## Remaining semantic-reconciliation debt

The following PR #149 source-specific implementations were **not** imported as parallel, unverified components: `architect/child_council.rs` (reservation-only), `architect/council_requests.rs` (alternate task requests and receipts), `council-criteria-editor.ts` and `council-inspector.ts` (different UI/type contract), plus tests that depend on their conflicting types. Evaluate their distinct security, restart and inspection semantics against the canonical #152 path, then port missing behavior and native negative tests without duplicating runtime authority.

Other unsatisfied #147 criteria remain explicit: 7 x 3 real terminal native scenarios; every AC-01..40 with exact-head evidence; stale revision and callback isolation; child restart/recovery; supported-harness native tool policy; provider interaction and accessible channel/embedded inspector regression. No implementation snapshot, earlier CI result or document ledger proves closure.

## Verification and release gate

Run `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib`, `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, `npm run build` and focused Playwright Council/Channel tests against the new exact commit. This connector-only integration has **not run** local Rust, Node, browser, Desktop or provider tests. Keep the PR a draft until all material divergences, missing acceptance cases and regressions are resolved.

## Source-sync checkpoint (2026-11-13)

- PR #149 remains at `664c530152693f403199496a450ce17df550cf74` (no new commits since the initial integration).
- PR #152 advanced from `455774df73fad829defd5765c89f91a601cd6297` to `48a8076ddac65ddd5ae790cce93dc2d8ad73ea0e` (one new commit).
- Reconciled source updates: Council finding disposition cycle/artifact verification, native human waiver, task-bound evidence checks, Channel delivery correlation and native/browser regressions; tool catalog v15; Astro 7.4.12 and lockfile; source `.gitignore`.
- Retained the integration's existing root attempt and concurrency limits without introducing #149's competing Council engines.
- The source `.gitignore` contains hundreds of generated test-artifact paths; source parity is preserved, but cleanup is separate from this checkpoint.
- The original #152 acceptance ledger is pinned to an older SHA. Full #147 AC-01..40 and exact-HEAD test verification remain **pending/not_run**.


## Composed-root native authority hardening (new integration batch)

The integration now requires every active root reviewer in a passed composed Council,
including read-only automatic reviewers, to have an exact persisted native grant.
The same check applies before the root `finish_team` transition, while verifying
a terminal task/channel verdict, and while validating persisted per-seat receipts.
Missing, altered, or revoked grants fail closed; absence of the additive
`composition` contract retains the original M1/M2 read-only compatibility path.

A native negative regression covers missing first/second grants, the legacy
fallback, modified immutable identity, and grant revocation. This adapts the
security intent of PR #149's newer M3-only fix without introducing #149's
competing Council engine. No runnable test or CI result is claimed here.


## Display and receipt identity consistency

Composed Council native reads and stored UI snapshots now show a `blocked`
state when a scored root reviewer lacks its persisted grant, rather than
presenting an unauthorizable `passed` label. The finish gate compares the
unmodified Council score/evidence verdict before applying this authorization
notice, so a blocked display does not inadvertently allow completion.

The root terminal gate also rejects multiple native review or child receipts
for the same current seat identity, rather than choosing the first match.
A regression test covers zero, one, duplicate and replayed receipt matches.
Automated execution remains outstanding for this connector-authored batch.
