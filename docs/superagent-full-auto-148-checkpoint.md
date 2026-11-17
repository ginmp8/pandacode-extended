# SuperAgent #148 Full Auto — implementation and verification checkpoint

> **Status: partial implementation. Not a full-auto acceptance receipt.** This
> checkpoint captures the first scoped slice on the dedicated branch. Read the
> live [issue #148](https://github.com/ginmp8/pandacode-extended/issues/148)
> and every normative comment on each future run; this document does not
> supersede the issue, the original #131 acceptance criteria, or source code.

## Ownership and snapshot

- Only writable branch: `feat/issue-148-full-auto`.
- Starting snapshot: parent [PR #154](https://github.com/ginmp8/pandacode-extended/pull/154),
  `f2547f6af31a0447ca8491418692931c024f5bfe`.
- Dedicated stacked [PR #157](https://github.com/ginmp8/pandacode-extended/pull/157)
  targets the PR #154 branch. Both origin and base branches remain read-only.
- Code inspected and updated on this branch through `ea410ee95acbbbb06d0191d5cd3a9489a9c376cf`.
  Later SHA changes invalidate final verification; reconcile the latest
  checked-out and published HEAD before assertions.
- Build environment for this checkpoint: GitHub connector writes; no
  executable Rust toolchain or complete repository available locally.
  Rust/Playwright tests below are **written, not run**, until CI records
  actual results on the exact current commit.

## User journey and coverage ledger

Coverage values: `inspected` = source evaluated, `functional_slice` =
production behavior and relevant test authored; neither means final
implementation or executed passing validation. Every row remains
`automated_validation: not_run` at this source-writing checkpoint.

| Scope / acceptance | Coverage | Implementation | Native/frontend evidence; remaining work |
| --- | --- | --- | --- |
| Manual + authorized GitHub Demand intake, de-duplication (DD-01, AC-13..18) | inspected | partial | Existing `auto/intake.rs`, `auto.rs` and Ticket correlation; end-to-end authenticated opt-in + real provider test missing |
| Coordinator planning, bounded WorkItem DAG, Worker selection (DD-03, AC-02..08) | functional_slice | partial | `scheduler.rs` now requests a strict `requiresWrite` boolean as untrusted intent; runtime planner/provider cases and recovery from invalid proposals are incomplete |
| Finite one-shot handoff, ready frontier, native child admission (DD-02, DD-04, DD-09) | inspected | partial | `scheduler/dispatch.rs` has native scheduler and dependency receipts, but full no-prompt engineering execution is absent |
| File edits and limited authority (DD-02, DD-05, DD-06, AC-02..08) | functional_slice | partial | New `superagent_authorize_demand_work_item_writes` authorizes the **same explicitly listed exact paths** for multiple pending write WorkItems in one native confirmation. Each child has its own immutable scope. The batch is all-or-nothing with audit; the initial Demand has no pre-approved source scope, so follow-up confirmation is **still required** |
| Native tests, failure analysis and bounded repair (DD-02, DD-04, DD-05, DD-09, AC-19..23) | inspected | partial | No verified generic native engineering test runner/retry/repair/receipt loop or genuine provider end-to-end acceptance yet |
| Independent task Council verdict/exception (DD-02, DD-06, DD-09, AC-11..12, AC-26..31) | inspected | partial | `council_adapter.rs` / native seat integration exists but the true full-auto review journey remains unverified |
| Source commit/PR and evidence-backed delivery (DD-05, DD-10) | inspected | partial | `delivery.rs` explicitly reports missing test/CI/publication attribution; generic safe autonomous deliverable path remains incomplete |
| Restart, stop, idempotency, effect reconciliation (DD-07, AC-19..23) | inspected | partial | Checkpoint/scope guards and effect receipts present; process-level restart, uncertain effects and interrupted batch authorization need executed tests |
| Reusable Team / Demand UX and truthful state (DD-08, DD-11..14, SA-01..09, UX-01..24) | functional_slice | partial | `superagent-ui.ts` exposes the native multi-WorkItem batch path, without pretending to have test/PR permissions. Genuine native UI journey, initial full-auto preflight and accessibility matrix missing |
| Worker-created bounded subagents (NS-01..12) | inspected | partial | Existing native ancestry, depth and Stop structures; real provider child/grandchild proof unavailable |
| Design review and acceptance drift (EGM-01..07, S0/S6, AC-01..31) | inspected | partial | This scoped checkpoint is not the full independent fresh-context review nor an exact-SHA completion ledger; original requirements remain mandatory |
| Genuine 3+ work-item full-auto scenario (DD-02, DD-05, DD-09) | inspected | not_started | Must verify real code edit, executable test, bounded correction, native review, deliverable receipt with **zero follow-up user prompts** and **zero per-worker clicks** in Economy and Performance policies where eligible |

## Changes in this source-writing batch

1. `scheduler.rs`: extracted a side-effect-free Coordinator prompt builder
   enabling untrusted `requiresWrite` requests without any file/tool
   grant. Existing native JSON validation, Team membership and grant
   issuance remain authoritative.
2. `scheduler.rs`: refactored exact-path scope insertion into a common
   transactional helper accepting one or multiple pending WorkItems. Item
   ID/key, original Demand and revision remain pinned. Any failing
   insertion or audit aborts the **entire** confirmation.
3. `scheduler.rs`: new native batch confirmation command performs a
   before-and-after-dialog preflight against the original Demand mandate,
   worker profile, Codex support, checkout and exact allowed paths, then
   calls the scheduler only after a successful atomic grant commit.
4. `lib.rs`: registers the native command. `superagent-ui.ts`: offers the
   batch path only for two or more pending unauthorized write WorkItems,
   and verifies the returned native projection before showing success.
5. Added a Rust regression for atomicity, stale revision, item-key mismatch,
   replay and immutable scope, and a Playwright scenario for native
   command routing, cancel/retry and no manual Worker dispatch.
   These **test sources have not yet been executed** in this checkpoint.

## Safety and operational notes

- A Coordinator model response is **untrusted plan data**, not a write grant.
- Batch authorization displays every included WorkItem and explicitly states
  that the **identical** approved path list is granted to **each** child.
  This is appropriate only when the operator truly intends that shared scope.
  For least privilege, use the existing narrower single-WorkItem route.
- No native shell, MCP, GitHub effect or Council approval is acquired through
  file scope. Unsupported platforms/providers must fail closed.
- Final completion cannot be inferred from completed model turns, UI labels,
  read-only Git snapshots, PR status or CI green alone.

## Next execution candidates (coverage first)

1. Replace post-plan manual authorization with a **single initial, finite
   Demand authorization** for specific allowed files, bound to a pinned
   Team/checkout/policy. A future coordinator plan may use only subsets
   of that original consent; never grant extra paths on model request.
   Test denial, source drift, revocation, native restart and replay.
2. Add an explicitly scoped, bounded native test-execution capability with
   attributed attempt receipts, no unrestricted shell and structured
   failure signals for scheduler-owned finite correction/retry.
3. Integrate review/policy gates and authorized engineering delivery
   effects, then implement truthful frontend preflight/Run inspector
   connected to the same durable backend state.
4. Execute the required real no-prompt multi-WorkItem scenario and complete
   the exact-SHA AC/DD/SA/NS/UX/EGM independent review.

## Anti-stall and stop rule

- Phase: **coverage**.
- Initial scoped failure registry: none (validation pending, no failed
  checks claimed).
- Ready next action: initial native Demand-level scope authorization and
  restricted propagation to child WorkItems.
- Stop automation only after independent source review confirms all
  backend/frontend full-auto acceptance, real required automated tests
  passed at the **current published HEAD**, and no blocking functional,
  authorization or recovery debt remains. Do not merge the PR.
