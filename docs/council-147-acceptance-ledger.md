# Council #147 — acceptance evidence ledger

> Evidence status: **IMPLEMENTATION UNVERIFIED**. No row below claims a passing validation, a terminal business acceptance or release readiness. This ledger is source traceability, not a substitute for execution.

## Frozen integration context

- Target repository: `ginmp8/pandacode-extended`. Target PR: #155 (`feat/issue-147-council-unified` -> `extended/development`, draft).
- Review baseline before this ledger: `8eaababef579d41d857ad1c08e187aa4a1dcafd2`. Subsequent changes require a fresh exact-SHA verification; do not infer the current HEAD from this document.
- Read-only source PR #149: `fcf6a0f00d126a7e1e9d5d78547864d6a65a78ec` (native child council variant).
- Read-only source PR #152: `48a8076ddac65ddd5ae790cce93dc2d8ad73ea0e` (task-isolated Council composition).
- All code changes belong exclusively to the PR #155 head. No merges, force updates or changes to source branches are authorized.

## Per-requirement source coverage (implementation vs. proof)

| AC | Required behavior | Source and test entrypoints | Evidence status |
|---|---|---|---|
| AC-01 | Original M1 and selected M2 remain available | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect/council_tests.rs` | Code + tests present; exact-HEAD runtime unverified |
| AC-02 | Three reviewer sources and seven valid combinations | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect/council_tests.rs` | Code + tests present; exact-HEAD runtime unverified |
| AC-03 | M3 automatic plus direct selected bots | `src-tauri/src/architect/bot_reviewers.rs`; `src-tauri/src/architect/tests.rs` | Native dispatch tests present; provider smoke missing |
| AC-04 | M4 task-owned child Architects and native leaf reviewers | `src-tauri/src/architect.rs`; `src-tauri/src/architect/tests.rs` | Native dispatch tests present; provider smoke missing |
| AC-05 | M5 independent automatic and nested child quotas | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect/tests.rs` | Source-aware dispatch present; terminal runtime unverified |
| AC-06 | M6 direct and nested bots with no phantom root votes | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect/tests.rs` | Native dispatch tests present; terminal runtime unverified |
| AC-07 | M7 all three reviewer sources | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect/tests.rs` | Native dispatch tests present; provider smoke missing |
| AC-08 | Two through eight independent root seats and no duplicate bots | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect/council_tests.rs` | Native bounds present; exact-HEAD tests unrun |
| AC-09 | Child scopes/read-only guard and no grandchildren | `src-tauri/src/architect.rs`; `src-tauri/src/architect/reviewer_security.rs` | Read-only policy present; supported-harness smoke pending |
| AC-10 | Native nested lifecycle and one parent seat per child | `src-tauri/src/architect.rs`; `src-tauri/src/architect/tests.rs` | Isolated child lifecycle present; terminal evidence integration pending |
| AC-11 | Architect/User/Hybrid criteria sources and provenance | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect.rs` | Native validation present; UI/browser exact-HEAD unrun |
| AC-12 | Required scores/evidence and minimum-based gate | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect/council_tests.rs` | 21-mode/strategy negative fixture present; execution unrun |
| AC-13 | S1 parallel/concurrency limits | `src-tauri/src/architect.rs`; `src-tauri/src/architect/tests.rs` | Native capacity paths present; concurrent provider smoke pending |
| AC-14 | S2 escalation and deterministic trigger evidence | `src-tauri/src/architect.rs`; `src-tauri/src/architect/council_tests.rs` | Native S2 decisions and tests present; provider smoke pending |
| AC-15 | S3 independent second-phase challenge | `src-tauri/src/architect/council.rs`; `src-tauri/src/architect/council_tests.rs` | Phase1-only denial and distinct sessions tested in source; execution unrun |
| AC-16 | Resource ceilings and replay/attempt budget | `src-tauri/src/architect.rs`; `src-tauri/src/architect/tests.rs` | Shared root/child quotas present; concurrent stress pending |
| AC-17 | Artifact revision staleness/re-review | `src-tauri/src/architect.rs`; `src-tauri/src/architect/council_tests.rs` | Native revision checks present; end-to-end invalidation pending |
| AC-18 | Native root terminal gate and typed child receipts | `src-tauri/src/architect.rs`; `src-tauri/src/architect/native_council_task.rs` | Native verified proof required; positive terminal smoke pending |
| AC-19 | One terminal delivery callback, idempotent recovery | `src-tauri/src/bots/shared_channels.rs`; `src-tauri/src/bots/tests.rs` | CAS/duplicate callback tests present; live race pending |
| AC-20 | Supported harnesses preserve read-only authority | `src-tauri/src/architect/reviewer_security.rs`; `src-tauri/src/architect/bot_reviewers.rs` | Contract paths present; per-harness smoke pending |
| AC-21 | Task-scoped skill pins/provenance/native check evidence | `src-tauri/src/architect/reviewer_security.rs`; `src-tauri/src/bots/tests.rs` | Native checks and negative tests present; live Skills smoke pending |
| AC-22 | Independent tasks for concurrent same-bot deliveries | `src-tauri/src/bots/shared_channels.rs`; `src-tauri/src/bots/tests.rs` | Two-concurrent-message test present; runtime race pending |
| AC-23 | Durable nested child restart and resume | `src-tauri/src/architect.rs`; `src-tauri/src/architect/tests.rs` | Recovery and idempotency tests present; crash-injection pending |
| AC-24 | Failure, cancellation and timeout fail closed | `src-tauri/src/architect.rs`; `src-tauri/src/architect/tests.rs` | Lifecycle tests present; provider-failure smoke pending |
| AC-25 | Optional post-acceptance GitHub issue authorized once | `src-tauri/src/bots/shared_channels.rs`; `src-tauri/src/bots/tests.rs` | One-claim tests present; external integration unrun |
| AC-26 | Shared Channel delivery-scoped Council isolation | `src-tauri/src/bots/shared_channels.rs`; `src-tauri/src/bots/tests.rs` | Delivery and native terminal tests present; browser runtime pending |
| AC-27 | One Council modal and task-scoped composition UI | `src/council-panel.ts`; `src/bot-shared-channels.ts`; `tests/council.spec.ts` | UI code/tests present; visual/browser acceptance pending |
| AC-28 | Keyboard, accessibility and responsive inspector | `src/council-panel.ts`; `tests/council.spec.ts` | Playwright assertions present; WebKit/real viewport unrun |
| AC-29 | Legacy M1/M2 and existing runtime regression | `src-tauri/src/architect/council_tests.rs`; `src-tauri/src/bots/tests.rs` | Legacy tests present; all-platform suite pending |
| AC-30 | All seven modes x three execution strategies | `src-tauri/src/architect/tests.rs`; `src-tauri/src/architect/council_tests.rs` | 21 native dispatch/status fixtures and denial matrix present; exact-HEAD unrun |
| AC-31 | Cost, fallback and progress evidence transparency | `src/council.ts`; `src/council-panel.ts`; `src-tauri/src/architect.rs` | Summary/code present; cost under unknown data testing pending |
| AC-32 | Docs with source/branch, concept screenshots and decisions | `docs/council-147-integration.md`; `docs/architect-workers.md` | Docs present; live screenshots and final verification pending |
| AC-33 | Multiple typed findings with titles, explanations, verified refs | `src-tauri/src/architect.rs`; `src-tauri/src/architect/council.rs` | Immutable finding code and transactional writes present; real-file integration pending |
| AC-34 | Remediation/waiver dispositions revision- and actor-bound | `src-tauri/src/architect.rs`; `src-tauri/src/architect/council_tests.rs` | Native authorization present; positive human workflow smoke pending |
| AC-35 | Standalone task/delivery Council request and terminal client | `src-tauri/src/architect/native_council_task.rs`; `src-tauri/src/architect/tests.rs` | Public native API and negative tests present; positive end-to-end receipt pending |
| AC-36 | Wrong-caller/request/artifact and duplicate-write denial | `src-tauri/src/architect/native_council_task.rs`; `src-tauri/src/architect/tests.rs` | Native request/CAS tests present; race/fault-injection pending |
| AC-37 | Measured/estimated/unknown per-attempt usage | `src/council.ts`; `src-tauri/src/architect.rs`; `tests/council.test.mjs` | Native records/UI rollup present; exact-HEAD tests unrun |
| AC-38 | Bounded strategy comparison and no invented bills | `src/council.ts`; `src/council-panel.ts`; `src-tauri/src/architect.rs` | Budget/cost guards present; provider-priced smoke pending |
| AC-39 | Shared root/child inspector and evidence drill-down | `src/council-panel.ts`; `src/council.ts`; `tests/council.spec.ts` | Native child leaf/criterion projection present; real browser check pending |
| AC-40 | Reproducible exact-HEAD acceptance evidence and release gate | `docs/council-147-acceptance-ledger.md`; `.github/workflows/ci.yml` | Open gate: exact-HEAD CI, live harness and visual evidence not obtained |

## Mandatory independent verification before closing

1. Freeze the exact remote PR HEAD; validate its parent chain and all source branch refs, then run `cargo check`, focused Architect/Council Rust tests and `cargo clippy --all-targets` with the repository toolchain.
2. Run `npx tsc --noEmit`, `npm test`, and the focused Council/Shared Channel Playwright tests in a browser that serves the correct worktree. Inspect Chrome and WebKit/reduced-motion/keyboard states.
3. Execute **native** 7 modes x 3 strategies with real task/delivery identities, including phase 2, child timeouts, cancellation, provider failures, changed artifacts, repeated native callbacks, task restart and replayed receipts. Rust synthetic fixtures are not provider/harness acceptance.
4. Validate a full **positive** standalone task-client terminal (`native_council_task::admit` -> independent read-only reviews -> verified persisted pass -> `record_terminal` -> `read_terminal`/`read_evidence`) as well as wrong-tenant/project, wrong caller, altered policy, repeated/CAS conflicts, and crash recovery.
5. Verify at least two distinct evidence-backed findings under the same immutable criterion using real files; replay the submission and inject an error to prove rollback of both root and child SQLite writes.
6. Execute real selected-bot/native harness smoke across the installed provider matrix; verify optional vs. required Skills pins, read-only session tools, no grandchild authorizations, and no leaked bot-chat context.
7. Capture dated UI screenshots from the **running candidate** for direct chat and Shared Channels; verify all seven modal mode combinations, criterion provenance, root/child hierarchy, pending terminal state and score/evidence drilldown.
8. Correlate the latest GitHub Actions run to the exact HEAD, record failed/skipped/cancelled checks and their actual fingerprints, and close critical findings before claiming acceptance. CI running does not block further safe development.

## Status/decision contract

Source text, diagrams, static tests or unexecuted test source alone are **not proof**. `Passed` requires the existing native current-cycle grant, reviewer and child terminal receipts, independent criterion evidence and a completed root task. `Accepted by user` requires the separately attributed revision-bound user action. `Limit`, `Inconclusive`, `Stopped` and `Failed` are **not passing**. An unverified test is **not** relabeled passed, even if its source appears complete.

Latest integration decisions and commit-level deltas are recorded in [`docs/council-147-integration.md`](council-147-integration.md).
