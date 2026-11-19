# SuperAgent #148 — second-generation source reconciliation

## Inputs and ownership

This draft integrates later commits from PRs #150 and #151 into a new branch based on the existing semantic integration in PR #153. The original PR branches and PR #153 are not modified.

- Earlier unified commit: `8679585028b92185679b51a4700779e87ff79950`.
- PR #150 previously integrated: `9072d84e0111f0afe58fe6d7598c1e09098d3202`; latest source: `6580fc4d25d403b4feac20b674aa1ff4c9697327` (15 new commits).
- PR #151 previously integrated: `892fa783696489cb12d4924c29d40ce1eadb692a`; latest source: `3f9ef83ebbfceae9c3df52418044ef8c17714e1f` (one new commit).
- Integration branch: `feat/issue-148-unified-150-151-v2`; base PR target: `extended/development`.

## Semantic decisions

- Preserve the existing unified native Team/Ticket, Council, scoped tool-write, GitHub effect and receipt contracts.
- Integrate additional authenticated Observe metadata, source health, event identity protection, linked issue/PR context, headless review/recovery inspection, worker role ordering and configuration preflight.
- Use one immutable, digest-validated `sa_ticket_team_snapshots` record as the admitted Team-profile authority. The separate experimental `sa_ticket_profiles` v46 table from PR #150 is deliberately not duplicated. Expose a read-only `admittedProfile` projection and reject stale profiles on new worker attempts.
- Preserve the original unified v52 history and apply only new additive migrations for Demand plans (v53), immutable scoped WorkItems (v54), authenticated Auto intake (v55) and bounded descendants (v56).
- Incorporate PR #151's newer Playwright selector and preserve its app-mediated write grants and external-effect reconciliation.

## Verification boundaries

Source-level integration is not executable proof. Before review/merge, run exact-SHA Rust compilation, focused native tests/Clippy, TypeScript checks, Playwright, SQLite upgrade/restart tests, headless command contract tests, Council and scoped write/revocation oracles, GitHub reconciliation tests, and supported-platform CI. No claims of passed checks are made here.

GitHub Auto and nested subagent code now exist behind explicit bounded grants; their comprehensive runtime proof, autonomous full engineering-test/review/delivery loop, full multi-checkout parallelism and the entire #131/#148 acceptance ledger remain outstanding. Keep the PR in Draft until those gates are addressed.


## Read-only source reconciliation checkpoint — 2026-11-15

Target: PR #154, `feat/issue-148-unified-150-151-v2`, pre-reconciliation
commit `6fdae591d0c4789c1680bdd42a17f30ffc64a34f`. The original PR source heads were frozen at:

- PR #150: `1db91cd9e70a1f651f0ef26d311bcd7eb5a741fc` (30 commits since the earlier reviewed origin).
- PR #151: `311a9e7fbf8d8551e4e9493b83af4aa806a2886c` (24 commits since the earlier reviewed origin).
- PR #153: `72490322091648187a991276e4bbb2b650c65bc6` (8 commits since the earlier reviewed origin).

**Ported in this bounded checkpoint:** from #150, fail-closed rejection of blank
mandate issuer IDs in native read-only policy and watchdog; from #151,
Windows scoped-write DACL access/handle ordering plus versioned Team snapshot
and grant regression adjustments; from #153, portable Team schema v5 with
bounded strategy round-trip, preview validation and browser expectation.
The source-pinned #131 baseline and #151 historical acceptance ledger are
included as source evidence, not as runtime evidence for PR #154.

**Unresolved semantic conflicts:** #150 authenticated GitHub poll signature,
headless paging, source receipts and roster-preflight work; #151 native Council,
WorkItem and Team UI flows; #153 one-shot coordinated Worker, Auto provenance,
strategy UI and native gateway contracts. These must be reconciled with the
unified scheduler, checkout leases, migration lineage and security fences
without replacing them with older source implementations. This checkpoint is
not feature parity with any source branch or #148 acceptance completion.

**Verification boundary:** exact source patches for the selected hunks were
matched against the target without a source or base ref mutation. No native
Rust/Windows, TypeScript or Playwright test run was available in this environment;
CI for the new exact head is a separate gate. No claim of passing tests,
merge readiness, or provider-backed execution is made here.
