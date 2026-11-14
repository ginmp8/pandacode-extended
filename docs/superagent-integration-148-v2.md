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
- Retain the unified schema version 52 and its additive migrations instead of applying a conflicting schema-version downgrade.
- Incorporate PR #151's newer Playwright selector and preserve its app-mediated write grants and external-effect reconciliation.

## Verification boundaries

Source-level integration is not executable proof. Before review/merge, run exact-SHA Rust compilation, focused native tests/Clippy, TypeScript checks, Playwright, SQLite upgrade/restart tests, headless command contract tests, Council and scoped write/revocation oracles, GitHub reconciliation tests, and supported-platform CI. No claims of passed checks are made here.

GitHub Auto, full dependency-aware autonomous scheduling, complete multiworker execution, nested subagents and the entire #131/#148 acceptance ledger remain outstanding. Keep the PR in Draft until those gates are addressed.
