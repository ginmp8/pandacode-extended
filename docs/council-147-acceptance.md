# Council #147: architecture and acceptance evidence

## Evidence snapshot

This ledger applies the requirements in [issue #147](https://github.com/ginmp8/pandacode-extended/issues/147), read at its 2026-11-12 update, to [PR #152](https://github.com/ginmp8/pandacode-extended/pull/152), head 5edec40ce913dc950dd50f0799e3aa1d63dc17a2 (base extended/development). The PR is open. This is a candidate evidence snapshot, not issue closure, merge approval, or release evidence.

The [CI run for that SHA](https://github.com/ginmp8/pandacode-extended/actions/runs/43349227556) completed with conclusion **failure**. The Rust library test command passed with 2,899 passed, 0 failed and 97 ignored. The typecheck, npm unit/release tests and website build job also passed. Clippy failed in the Linux job and the Linux ARM64, macOS x86-64/ARM64 and Windows x86-64/ARM64 jobs. The Linux log reports 15 library errors and 23 library-test errors under warnings-as-errors; one visible diagnostic is the unused maximum_automatic_seats and minimum_root_seats methods in council.rs.

The shared checkout now contains follow-up edits that are not part of `5edec40ce913dc950dd50f0799e3aa1d63dc17a2` or this Actions run, including a restored legacy Shared Channel reviewer selector. This ledger intentionally stays at the pinned SHA: later working-tree source or tests do not change any status here.

Relevant Playwright failures in that run:

- Council selected-bot shard 1 failed at tests/council.spec.ts:48 waiting for “Apply composition”, and at :192 waiting for “Findings and dispositions · 2”.
- Council selected-bot shard 2 failed at tests/council.spec.ts:685 (panel layout assertion), :839 (keyboard test raised “Cannot read properties of null (reading 'dataset')”) and :882 (narrow viewport assertion). Shard 3 passed.
- Bot manifest and collaboration UI failed at tests/bot-collaboration.spec.ts:4 and :83 waiting for the prior selected-mode input; 13 tests in that job passed.

No implementation tests were run locally for this documentation update. The AC statuses below refer only to the exact-SHA Actions evidence above. Each row names the exact pinned test path and line(s); a cited partial test does not prove the whole criterion. “passed” is limited to ACs with direct passing tests for the stated outcome; “failed” records a relevant failing check; “blocked” means the needed runtime evidence was unavailable; “not_run” means the complete criterion was not established. Where no direct test exists for the full criterion, that gap is stated explicitly. A passing partial test does not promote a broader AC to passed.

## Architecture note

### Reviewer modes, strategy and criteria are separate

The native Council remains the only engine. Its optional, task-scoped schema-v1 composition combines three reviewer sources: A = root Architect-selected automatic reviewers, D = selected persistent bots reviewing directly, and N = selected bots coordinating their own child Councils. The mode is derived from those sources; A/D/N all off is invalid.

| Mode | A | D | N | Composition |
| --- | :---: | :---: | :---: | --- |
| M1 Architect | Yes | No | No | Original automatic Council |
| M2 Bots | No | Yes | No | #126 selected direct reviewers |
| M3 Bots + Architect | Yes | Yes | No | Direct bots plus automatic root seats |
| M4 Bots with own Architect | No | No | Yes | Nested child Council per selected bot |
| M5 Bots with own Architect + Architect | Yes | No | Yes | Child Councils plus automatic root seats |
| M6 Mixed bots | No | Yes | Yes | Direct and nested selected bots |
| M7 Mixed bots + Architect | Yes | Yes | Yes | All three sources |

Each selected bot is assigned to one branch once. An executing bot cannot review its own task. Root quorum is 2–8 distinct seats, and the combined allocation cannot exceed eight. A nested child counts as one root seat. Each child has 2–8 distinct leaf reviewers. Repeated provider/model identity is reduced independence to disclose, not another independent model.

| Strategy | Contract |
| --- | --- |
| S1 Parallel (default) | Dispatch enabled sources together within finite limits; every required seat must pass. |
| S2 Escalated / on-demand | Begin from enabled sources only. Below-target score, critical/high finding, missing required evidence, same-criterion conflict or user force triggers required fresh review. For N, a bot can be Always nested or expand its same seat On trigger. |
| S3 Two-phase | Phase 2 uses fresh, independent read-only challengers from enabled sources against the frozen phase-1 artifact and findings. Both phases must pass; unavailable independent challengers cannot pass. |

The criteria source is a third independent choice: Architect-defined, User-defined or Hybrid. User criteria remain immutable required anchors; Hybrid may add only nonduplicative Architect lenses with provenance. Required criterion evidence and score are checked individually, and a per-criterion target cannot lower a stricter gate. The gate uses the minimum passing seat/criterion, not an average or vote. A child’s composite is the minimum eligible fresh child score; missing or unverifiable child evidence has no pass-equivalent score.

### Child lifecycle and evidence boundary

One task/delivery and artifact revision → root Architect/Council freezes its config and reviewer bindings → each N seat owns a separate child Architect/Council session → the child dispatches 2–8 independent read-only leaf reviewers → native code validates child scores, required evidence, policy and artifact revision → a typed child receipt settles exactly one parent seat → the root decides only after every required seat settles → one terminal task/delivery result and any separately authorized follow-up.

Child start, worker-turn end, prompt claims or partial scores do not settle the parent. Child bindings retain root cycle, parent seat, child cycle, artifact revision and policy provenance. The hierarchy limit is two levels total (root plus child); nested leaves cannot create grandchildren. Child work is not the selected bot’s open chat and cannot alter the bot profile, credentials, memories or unrelated tasks. Reviewers are native read-only by default. A narrowly scoped file action requires the user’s explicit task-bound grant; shell, checkout writes and broader tools are not implied.

The Shared Channel plan uses maxDepth 2, maxRootSeats 8, at most 64 concurrent leaves, at most 128 leaf reviews per round, a 24-hour timeout and a 1,000,000-micro estimated-cost cap. A cost cap is not satisfied by unknown cost. Findings, dispositions, child/review receipts and usage are projected for the current root cycle only; stale cycle or artifact evidence cannot satisfy a new review. Unknown and estimated usage remains labelled as such.

### Recovery, migration and rollback

- Migration is additive: composition is optional, schema version 1. Absence preserves the legacy M1/M2 configuration path. The passing native test legacy_m1_and_m2_json_keep_the_original_defaults_and_serialization covers that serialization compatibility; it does not prove every historical task migration.
- The PR #152 changed-file list contains no storage schema/migration file. The observed compatibility is Council config serialization, not evidence of a database downgrade or every historical task migration.
- Native recovery code links each child to its root and supports root-owned pause, resume and cancel with idempotent cleanup. Tests include nested_child_tracks_root_pause_resume_and_cancel_with_idempotent_recovery and nested_escalation_decision_recovers_same_seat_phase_trigger_and_child_link. They do not cover every restart, deletion, provider-loss, callback-pending and rollback state in AC-23/24.
- Safe rollback guidance, inferred from the optional config and root-owned child lifecycle: stop admitting new composed tasks; pause or settle/cancel active children through their root; preserve child links and receipts until terminal work/callbacks settle; then omit composition for new tasks to return to M1/M2. This sequence was not tested as a deployment rollback. Do not infer that dropping or rewriting active child records is safe.

## AC-01–40 ledger

Normative wording is in issue #147’s acceptance table. The references below point to source/tests pinned to the PR head and the exact Actions run.

| AC | Status | Evidence at 5edec40ce913dc950dd50f0799e3aa1d63dc17a2 | Exact pinned test path(s):line(s) |
| --- | --- | --- | --- |
| AC-01 | failed | M1 regression UI checks failed; this does not establish the remaining routing/scoring paths. | `tests/council.spec.ts:685,839,882` |
| AC-02 | failed | Selected-bot compatibility UI checks timed out waiting for the prior selected-mode input. Native M2 behavior is not inferred from the UI failure; the pinned source has no legacy Auto/Selected radios. | `tests/bot-collaboration.spec.ts:4,83`; `src/bot-shared-channels.ts:253–268` |
| AC-03 | not_run | The native 7×3 dispatch test includes M3 but stops before reviewer verdicts and the final gate. | `src-tauri/src/architect/tests.rs:2070` |
| AC-04 | not_run | The native dispatch test creates M4 child sessions and leaf seats; it does not complete a child verdict or settle its composite parent seat. | `src-tauri/src/architect/tests.rs:2070` |
| AC-05 | not_run | The M5 root/child dispatch case is present in the native matrix; waiting for both branches to reach a terminal verdict is not asserted there. | `src-tauri/src/architect/tests.rs:2070` |
| AC-06 | not_run | The M6 source allocation is present in the native matrix; direct/nested terminal aggregation is not run. | `src-tauri/src/architect/tests.rs:2070` |
| AC-07 | not_run | The M7 source allocation is present in the native matrix; all-source approval and result hierarchy are not run to terminal state. | `src-tauri/src/architect/tests.rs:2070` |
| AC-08 | not_run | Composition rejection and self-review/unsupported-capability cases pass, but exhaustive invalid inputs and no-side-effect atomicity are not established. | `src-tauri/src/architect/council_tests.rs:2192,2211,2251` |
| AC-09 | not_run | The no-grandchild and read-only/profile tests cover parts of the contract; active-chat isolation and every unauthorized-worker path are not exercised together. | `src-tauri/src/architect/council_tests.rs:636,1222`; `src-tauri/src/architect/tests.rs:5529` |
| AC-10 | not_run | Typed child-receipt validation passes; an integrated child-to-one-root-seat completion/gate test is not established. | `src-tauri/src/architect/council_tests.rs:2335` |
| AC-11 | not_run | Frozen criteria and required-evidence cases pass, but the full three-source, multi-round anchor/provenance/coverage contract is not established. | `src-tauri/src/architect/tests.rs:248`; `src-tauri/src/architect/council_tests.rs:2282`; `tests/council.spec.ts:115` |
| AC-12 | passed | Native gates use the lowest score, reject missing criterion coverage/evidence and reject stale evidence; these tests ran in the passing Rust library job. | `src-tauri/src/architect/council_tests.rs:993,1966,2316` |
| AC-13 | not_run | The native matrix dispatches S1 branches, but deterministic terminal consolidation after a required branch failure is not established. | `src-tauri/src/architect/tests.rs:2070` |
| AC-14 | passed | Native trigger tests cover the listed S2 triggers and a complete evidenced no-trigger result; fresh no-N and user-forced escalation cases also ran. | `src-tauri/src/architect/council_tests.rs:1569,1953`; `src-tauri/src/architect/tests.rs:394,480,553,785` |
| AC-15 | not_run | The phase gate requires two correlated rounds and distinct challengers, but phase-1 report preservation and the full phase-2 lifecycle are not established. | `src-tauri/src/architect/council_tests.rs:1933` |
| AC-16 | not_run | Deadline, cost and leaf-budget tests ran for strategies; retry exhaustion and all fan-out recovery paths are not established. | `src-tauri/src/architect/tests.rs:637,2244` |
| AC-17 | not_run | A stale round/criterion receipt is rejected in native gate tests; the full parent/child/phase correction lifecycle is not established. | `src-tauri/src/architect/council_tests.rs:1001,1966` |
| AC-18 | not_run | Limit and missing-evidence cases are covered, but all named non-pass states and explicit user exceptions are not proven by one complete terminal-state suite. | `src-tauri/src/architect/council_tests.rs:1015,1148,1966` |
| AC-19 | not_run | The task adapter asserts one immutable receipt; exactly-once Shared Channel callback timing after every root seat settles is not established. | `src-tauri/src/superagent/council_adapter.rs:645` |
| AC-20 | blocked | No direct live supported-harness adapter test exists in this CI run. Native policy tests are not proof of behavior on each real harness adapter. | Partial native policy tests: `src-tauri/src/architect/council_tests.rs:636,748`; no direct live-adapter test. |
| AC-21 | not_run | UI skill receipt display and native skill/policy tests exist, but supply, invocation, native check execution and verified evidence are not established end to end. | `tests/council.spec.ts:290`; `src-tauri/src/architect/council_tests.rs:41,95,453` |
| AC-22 | not_run | No direct same-bot concurrent-task isolation test exists in this CI run. | No direct test at this SHA. |
| AC-23 | not_run | Decision/link recovery and allocation rollback tests pass; restart is not exercised across each child/root/phase/callback-pending state. | `src-tauri/src/architect/tests.rs:5,115` |
| AC-24 | not_run | Root-owned child pause/resume/cancel passes; delete, revoke, provider loss, timeout, stop and stale-roster cases are not all exercised together. | `src-tauri/src/architect/tests.rs:870,5564` |
| AC-25 | not_run | A native test covers the opt-in post-accept GitHub action contract; exact-SHA idempotent issue creation after root approval is not established. | `src-tauri/src/architect/council_tests.rs:10` |
| AC-26 | not_run | Per-delivery and single-message submission tests pass; two recipients and two messages to the same bot are not verified together. | `tests/bot-collaboration.spec.ts:83`; `tests/bots-channels.spec.ts:126,201` |
| AC-27 | failed | The all-mode UI test timed out waiting for the composition editor’s Apply composition button in the exact-SHA run. | `tests/council.spec.ts:48` |
| AC-28 | failed | Keyboard containment and narrow viewport UI checks failed; related Shared Channel layout checks also failed. | `tests/council.spec.ts:115,839,882`; `tests/bot-collaboration.spec.ts:4` |
| AC-29 | not_run | No direct full Council-off/chat/Team/Skills/Channels regression matrix test exists; the checked tests cover only individual paths. | Partial cases: `tests/bot-collaboration.spec.ts:194`; `tests/bots-channels.spec.ts:14,201` |
| AC-30 | failed | The native runtime test dispatches all 21 pairs, but the all-mode UI test failed and terminal evidence/callback assertions are absent from that matrix. | `src-tauri/src/architect/tests.rs:2070`; failed UI `tests/council.spec.ts:48` |
| AC-31 | passed | Task-consumer tests accept one correlated current native approval, reject replay/forged status and retain Council-off behavior; the Rust library job passed. | `src-tauri/src/superagent/council_adapter.rs:632,645,687,697` |
| AC-32 | not_run | No direct test exists for the complete documentation, publication-at-tested-head and CI-closure criterion. | No direct test at this SHA; the artifacts are this ledger and `docs/shared-channel-council.md`. |
| AC-33 | not_run | Typed finding validation and distinct content digests are tested, but the complete two-finding failed-review output contract is not established end to end. | `src-tauri/src/architect/council_tests.rs:2335,2400` |
| AC-34 | not_run | Stale evidence and model self-claims are covered, but corrected-revision history and verified finding closure are not established end to end. | `src-tauri/src/architect/tests.rs:2300`; `src-tauri/src/architect/council_tests.rs:1001,2400` |
| AC-35 | passed | The task adapter produces one correlated terminal receipt and authorized evidence without #148, asserts replay stability and denies a revised ticket artifact; the Rust library job passed. | `src-tauri/src/superagent/council_adapter.rs:645` |
| AC-36 | not_run | Changed caller/content replay and forged status tests pass; wrong assignment/digest/policy, foreign evidence, restart replay and unauthorized writes are not all exercised. | `src-tauri/src/superagent/council_adapter.rs:413,687,697` |
| AC-37 | not_run | Unknown measurement provenance is covered, but root/child aggregation, elapsed/queue time and no-double-counting are not established. | Partial contract test: `src-tauri/src/architect/council_tests.rs:2335`; no direct aggregation test at this SHA. |
| AC-38 | not_run | Budget tests cover all strategies, but equivalent mandatory gates across Economy/Performance and retry/exhaustion are not established. | Partial tests: `src-tauri/src/architect/tests.rs:637,2244`; no direct Economy/Performance equivalence test. |
| AC-39 | failed | The current-cycle inspector test timed out waiting for its two finding/disposition rows; a separate nested inspector fixture does not establish the full direct/channel/embedded contract. | `tests/council.spec.ts:150,192`; `tests/architect.spec.ts:971` |
| AC-40 | failed | The exact-SHA Actions run is failed; no direct test establishes the complete compatible 7×3 AC-01–40 closure. | [Actions run 43349227556](https://github.com/ginmp8/pandacode-extended/actions/runs/43349227556), pinned to `5edec40ce913dc950dd50f0799e3aa1d63dc17a2`; no full-closure test. |

### Pinned source and run links

- [Issue #147 specification](https://github.com/ginmp8/pandacode-extended/issues/147) · [PR #152 and changed files](https://github.com/ginmp8/pandacode-extended/pull/152/files) · [CI run 43349227556](https://github.com/ginmp8/pandacode-extended/actions/runs/43349227556)
- [Native Council schema, gates, criteria, findings and usage](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/src-tauri/src/architect/council.rs) · [Native Architect child lifecycle and ledger projection](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/src-tauri/src/architect.rs) · [reviewer security](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/src-tauri/src/architect/reviewer_security.rs)
- [Native 21-pair dispatch test](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/src-tauri/src/architect/tests.rs#L2070) · [Council trigger, phase, criteria and receipt tests](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/src-tauri/src/architect/council_tests.rs#L1569) · [task Council adapter tests](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/src-tauri/src/superagent/council_adapter.rs#L632)
- [Council UI tests](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/tests/council.spec.ts#L48) · [Shared Channel UI tests](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/tests/bots-channels.spec.ts#L14) · [Shared Channel compatibility failures](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/tests/bot-collaboration.spec.ts#L4)
- [PR-head issue-147 engineering notes](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/docs/architect-workers.md#schema-v1-composition) · [Shared Channel guide](shared-channel-council.md)
