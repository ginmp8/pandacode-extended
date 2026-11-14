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


## Review-time authority enforcement

Composed reviewers now require their persisted native authority at the execution
boundary, while reading frozen task evidence, recording/verifying findings,
recovering selected-bot reviewer seats, and projecting nested-child review evidence.
Missing bindings on composed workers fail closed at the tool-policy boundary.
Legacy non-composed read-only sessions retain their historical fallback.
A native regression covers admission before/after issuance, modification,
revocation, and legacy compatibility.

Code and tests are published, but actual Rust, browser, and live provider
validation remain not_run at this integration SHA.


## Mandatory child budget preflight and all-mode grant coverage

Composition validation now rejects a leaf-review budget that cannot admit
even the mandatory Always-nested child reviewers, including the fresh second
phase required by the TwoPhase strategy. OnTrigger branches remain conditional
and may be admitted with a smaller initial budget; a later trigger is subject
to the existing runtime hard cap.

The existing native 7-mode x 3-strategy dispatch test now additionally
requires persisted native authority records for every allocated root reviewer
and child leaf. This expands real dispatch regression coverage; it does **not**
yet execute or prove 21 end-to-end terminal accept/reject scenarios.


## Standalone non-Channel native task client (AC-35 / AC-36)

A new architect/native_council_task.rs native API now offers task-scoped
admit/inspect/record_terminal/read_terminal/read_evidence without depending
on SuperAgent issue #148 or creating a second Council. A new request
pins caller identity, project, assignment, root cycle, objective, policy,
local session scope, and caller-supplied target digest before reviewer dispatch.
Requests are idempotent and cannot be rebound silently. Terminal results
are gated on completed current native Council approval, native correlated
evidence, reviewers starting after the task was admitted, and one-winner
content compare-and-swap storage. Evidence reads recheck the same authority
and current evidence.

The artifact digest is a correlation key provided by a trusted native caller,
**not** cryptographic proof of the bytes the model reviewed. This API is
native-only; it intentionally provides no model-callable permission or
outbound callback. A negative native test covers idempotency, mismatched
caller/project/request/digest, absent approval, unauthorized evidence reads,
and stale objective/policy. A terminal happy-path test and external native
consumer wiring remain unverified.


## Native task exclusive root-cycle ownership

A Council root cycle now admits exactly one non-Channel native task assignment:
its SQLite insertion atomically excludes other task records for the same
cycle and rejects mismatched callers or artifact identities. Request storage
keys contain the cycle so old request records survive later task cycles
without granting reusable approval authority. This adds negative coverage
for multiple assignments/callers on one Council review.


## Native task recovery regression

A disposable SQLite test covers replaying an identical request after process
restart, refusing a terminal receipt without a real Council pass, retaining
old admission history, admitting a fresh objective with the same assignment
name under a new cycle, and rejecting the old request ID on that new cycle.


## Frontend alignment for blocked native Council verdicts

TypeScript CouncilState now includes blocked, failed, stopped, inconclusive and
stale, and the view renders each explicitly without ever suggesting passage.
The composer/tray can show an alert even when an invalid composition is
blocked before its first round. Native notices are escaped before display.
Frontend preflight now mirrors the mandatory nested-leaf budget rule instead
of deferring its error until backend submission. Node tests cover all new
states and the fixed versus OnTrigger budget boundary.


## November 14, 2026: latest #149/#152 source review (incremental)

- Reviewed **PR #149** from `664c530152693f403199496a450ce17df550cf74` to `dba7c7ff1ee3ab3484aebcc53c6a3a374be589dc` (22 new, linear source commits; 12 changed paths). **PR #152** remains at `48a8076ddac65ddd5ae790cce93dc2d8ad73ea0e` with no new source changes.
- Ported a source-backed security invariant into the authoritative #152-derived Council engine: distinct scored root worker IDs and native session IDs are now required in every completed round, including legacy rounds; two-phase challengers additionally cannot reuse first-phase worker/session identities. Added negative Rust regressions for both conditions.
- Existing PR #155 native composition, task-isolated child Council, immutable child receipt, Shared Channel composition UI and current terminal-evidence code remain authoritative. The #149 `child_council.rs` and `council-inspector.ts` implementations are **not** copied into parallel, competing runtime/UI pipelines.
- **Not a full semantic source sync:** the remaining #149-only child lifecycle, shared consumed root/child admission budget, child-only terminal/restart rotation and inspector drill-down differences still require path-by-path behavior comparison and exact-HEAD native/UI tests. Source patches overlap heavily with PR #155's independently evolved architecture; raw cherry-picks would replace working contracts.
- No source or base ref is mutated here. The original #149/#152 commit SHAs are evidence pointers, not integrated-commit ancestry. Exact-HEAD Rust/Node/Playwright tests and full #147 acceptance remain unverified; keep this PR in draft.

## November 14: combined root/child admission cap

- Adapted PR #149's cumulative 1,024-attempt model to the single #152-derived native Council: root reviewer admission and all linked child Council leaf admissions share one cycle-wide ceiling, across all rounds and retries.
- Preserved the existing configured `maxLeafReviewsPerRound` limit and legacy M1/M2 behavior. The cumulative check runs before new root **and** child reviewer creation inside the native dispatch transaction; duplicates or changed child-cycle identities fail closed.
- Added native regression cases for abandoned and failed child leaves, multiple rounds, in-flight batch accounting, old-cycle exclusion, and legacy compatibility.
- This change does not copy PR #149's parallel child engine or claim end-to-end integration acceptance. Rust and UI tests remain unexecuted from the connector environment.

## November 14: completed-cycle receipt isolation

- Adapted PR #149's new-cycle rotation for completed composed Councils to PR #155's native objective-start path. Capture the prior completed state before swapping the objective, then advance the root cycle key monotonically (even for same-tick requests).
- An old blocked or now-stale terminal verdict cannot keep a newly submitted objective in the previous receipt namespace. Incomplete/paused Council continuation retains its existing behavior.
- Added a Rust regression for a completed composed task with an old non-passing review, new objective, and exclusion of prior-cycle reviewer seats.
- Native tests are not run in this connector-only environment; no full issue #147 terminal acceptance is claimed.

## November 14: authenticated current-cycle child receipt roster

- Adapted PR #149's complete-composite-roster defense to PR #155's existing native terminal receipt verification.
- Before root approval, every current-cycle child receipt must validate and correlate to a real native nested root reviewer seat. Duplicate receipts for one parent or records for unknown parent identities fail closed; old-cycle receipts remain audit history.
- Added negative Rust tests for duplicate and phantom child receipts and historical-cycle isolation. This is additive to PR #155's existing child evidence, frozen policy, terminal CAS and task-correlation verification.
- This does not introduce a duplicate child Council engine; exact-HEAD Rust tests remain unrun in this environment.

## November 14: prohibit child-leaf harness failover

- Reconciled PR #149's task-owned leaf failover restriction with PR #155's own nested Council runtime. A failed child reviewer keeps its frozen provider, model, native session and tool-policy boundary; a new independent review requires a new authorized seat.
- Existing root/native worker fallback and the selected-bot freeze policy are unchanged.
- Added a Rust regression using a nested Council fixture with two configured harnesses. Exact-HEAD Rust validation is delegated to CI; this change has not been locally compiled.

## November 14: validate child callbacks against the active root

- Adapted PR #149's final native leaf-callback authority recheck into the existing composed Council runtime. A task-owned child callback now requires its stored child link, current root authority, active root status and the original non-abandoned parent seat before it can mutate child results; ordinary root and legacy callbacks are unchanged.
- Added a regression that simulates a revoked/paused root with an in-flight old-generation child callback and verifies that no child reviewer result or revision is credited.
- This is an additional native guard, not a substitute for the existing immutable child terminal receipt or the root final approval gate.

- Callback authorization uses the existing validated child-to-parent decision link rather than requiring the parent reviewer's session ID to equal the child coordinator's session ID; OnTrigger escalation legitimately allocates a distinct child session. The 7×3 native dispatch test now asserts child callback authorization for configured nested modes.

## November 14: reject ambiguous child ownership records

- Reconciled PR #149's unique child-parent binding invariant with PR #155's existing immutable child links. The native child lookup now examines up to two matching records and fails closed on duplicate ownership rather than selecting an arbitrary first row.
- Added a negative Rust regression using two stored native links for the same child session. No new Council storage format or subsystem was introduced.


## Native child reviewer inspection (AC-10/11/39)

The unified Council projects read-only leaf rows (`councilChildLeaves`) only after validating the root-child link and each native reviewer binding. Each row includes the root/child cycle, round, parent seat, reviewer identity, attempt, status and reported score. Failed, abandoned or unscored workers remain `Unknown` rather than fabricating approval.

The existing native `councilCriterionOutcomes` projection is now typed and consumed by the shared inspector. UI filtering retains only the active root and correlated child cycles, displays actual child scores and criterion evidence when available, and retains a **pending native terminal** state when leaf reviews exist but no composite receipt has been issued. A reported leaf score never becomes another root vote.

New Rust, Node and Playwright tests cover reported-score limits, stale-cycle exclusion, criterion evidence and pending-versus-passed UI. Exact-HEAD Rust/browser test execution is pending; this is implementation evidence, not a passed acceptance receipt.


## Measured and estimated Council usage aggregation (AC-37/38)

The shared inspector now computes a per-root-cycle rollup from native-validated usage rows, keyed by `(rootCycleId, childCycleId, attemptId)`. Identical attempts are counted once even if their persisted usage is projected more than once; conflicting duplicates make dependent totals Unknown instead of arbitrarily selecting a value.

Token, request and reviewer-duration sums preserve `measured`, `provider_estimated`, `calculated_estimated` and `unknown` provenance. An unknown component keeps its aggregate unknown, without treating absent measurements as zero. Reviewer-duration sums are explicitly not root wall-clock duration because reviewers overlap.

Cost estimates are summed only when every attempt has bounded priced provenance in a single currency. Missing pricing or mixed currencies produce an Unknown aggregate. The UI identifies pinned source/version/calculation timestamp and does not present the estimate as a bill. Budget-consumption snapshots are not summed as if they were independent leaf attempts.

Added Node cases for measured, missing, cross-currency and conflicting-duplicate records, plus a Playwright inspector aggregate assertion. No provider billing data is inferred or generated.
