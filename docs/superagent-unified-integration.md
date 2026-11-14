# SuperAgent #148 — PR #150 / PR #151 Integration

**State:** draft semantic reconciliation. This document describes the integration candidate, not verified acceptance or a release.

## Provenance and ownership

- Common base: `f9efb3a1c7558dcf7d5688bb1687cfff9253f967` (`extended/development` at branch creation).
- Workspace/Observe source: PR #150 at `9072d84e0111f0afe58fe6d7598c1e09098d3202` (updated; original integration pin: `81a08dd08554b7d0e6a4096e5f88b21dd8b06223`).
- Native scoped-execution/Council source: PR #151 at `892fa783696489cb12d4924c29d40ce1eadb692a` (updated; original integration pin: `a5c92aac8a8ea5fe95be878c98a9e0750c945a0f`).
- Integration branch: `feat/issue-148-superagent-unified`; PR #153.
- Both source branches are read-only and retain their complete independent work.
- Each integration commit has one parent and contains the resulting tree; commits from neither PR are squashed or replayed into their original branch.

## Canonical native contracts

| Concern | Canonical owner | Integration decision |
| --- | --- | --- |
| Team, ordered Workers/Reviewers and admitted Ticket policy | `superagent.rs`, native Team versions | Use PR #151's versioned immutable Team/Ticket snapshots as the sole source of truth. PR #150's roster API only projects/updates that model; `sa_team_roles` remains an inert compatibility sidecar. |
| Worker write authorization | `superagent/write_grants.rs` and provider `ToolHost` | Preserve PR #151's exact-path, assignment/session-bound, revocable write grants and platform fail-closed checks; PR #150's UI may request but never create a grant itself. |
| Council review | Native Architect/Council and `superagent/council_adapter.rs` | Preserve verified task/assignment/artifact binding and native independent reviewer callbacks. Observational Council state alone never authorizes completion. |
| GitHub input | `superagent/events.rs`, existing `gh` adapter | Retain PR #150's explicitly operator-triggered authenticated Events/Checks Observe and PR #151's exact manual thread fetch. Neither implies an authenticated always-on subscription or Auto admission. |
| External effect | `superagent/effects.rs` | Preserve PR #151's native issue-comment grants, idempotency, read-back and cross-process fence. Expose PR #150's receipt history as read-only projection of the same authoritative table. |
| SQLite | `storage.rs` | PR #151 migrations v44–49 remain authoritative; v50 adds #150's roster/check-outcome tables; v51–52 add #151's Council workspace and immutable review snapshot identities. Previously used v44/v45 and v50/v51 variants are detected by actual table/column presence before additive upgrades; no worker write grant is issued during migration. |
| Experience | `src/superagent-ui.ts` and `src/main.ts` | Preserve PR #150's dedicated Teams/Demands/Review/Sources workspace and inspections; surface PR #151's explicit scoped-write and issue-comment UX under native Ticket authority. |
| Headless | `headless.rs` | Preserve PR #150's operational read/Observe/Team commands and PR #151's confirmed issue-comment command; no extra unauthenticated network control surface. |

### Prior version 44/45 collision

PR #150 used schema version 44 for `sa_team_roles` and version 45 for
`sa_check_run_results`. PR #151 independently used these version numbers
for exact worker grants and checkout-root pinning. `user_version` therefore
does **not** identify which PR's tables exist. The unified migration inspects
the actual roster table when upgrading either version, creates the missing
worker-grant schema with conservative revoked/default permissions, and then
continues the v46–v52 migrations. Any legacy worker/reviewer sidecar members
become future-facing versioned Team membership; existing Ticket evidence and
its recorded original version are never rewritten from the mutable Team.

This migration path must be exercised on copies of both legacy schemas before
approval. Until its cross-version CI is executed on the exact integration SHA,
the upgrade contract is **unverified**, not accepted.

### Updated upstream capabilities

- **#150 headless roster:** `--inspect-team-roster <team-id>` projects the
  canonical versioned Team and existing bot profiles from read-only storage.
  No provider session or Council grant is issued.
- **#150 portable manifest v3:** export records ordered, credential-free
  bot profile expectations (provider, model, mode, assigned Skill IDs), and
  import checks *every role* against existing local bots before any backup or
  mutation. V1 and V2 imports remain accepted. Source v3 `enabled` metadata
  is accepted for compatibility but never restores execution authorization.
- **#151 Council review snapshot:** a native authorized worker's checkout is
  frozen and hashed before independent reviewers run; the exact snapshot,
  workspace digest, Ticket, assignment, artifact and Council cycle are
  validated together before finalization. An unavailable, changed or unverified
  snapshot fails closed.
- **Collision recovery:** separate app versions already used migration
  numbers 50 and 51 differently. v51/v52 update checks existing columns,
  and independently creates the combined v50 metadata tables if absent,
  rather than relying on `PRAGMA user_version` alone.

## Safety invariants

- Neither a model prompt nor an imported manifest issues or widens an authorization grant.
- Coordinator and Council reviewer writes remain denied; only a currently authorized Worker may use a supported exact-path ToolHost.
- GitHub Off remains no-op. Observe captures bounded authenticated metadata without automatically launching tasks, posting comments or approving work.
- External effects require a current, versioned mandate and exact user confirmation. Uncertain results are reconciled before any new write; never treat remote failure/timeout as proof that an action did not occur.
- A mutable Team edit changes future admissions only. Existing Ticket snapshots and original evidence remain immutable.
- No task reaches a successful final state from model output alone, a stale Council round or a queued notification.

## Validation and known gaps

The initial integration is **not ready to merge** until final-HEAD Rust build/tests/Clippy, TypeScript build, Playwright and platform jobs are executed. Regression gates must cover legacy bots/channels, SQLite upgrades, source deduplication, malformed/unauthorized events, Council binding, Team-history edits, Stop/revocation races, exact write paths, and uncertain issue-comment recovery.

The overarching #148 contract still includes a dependency-aware autonomous scheduler, concurrent independent Workers, fully authenticated GitHub Auto, headless lifecycle parity and nested Worker-created subagents. File presence, generated documentation, and successful legacy MVP tests do not prove those features delivered. Preserve explicit unsupported/blocked states until they have native implementation and executed evidence.

## Exact-SHA verification record

Validation on the reconciled updated-source commit is **pending exact-SHA CI**. Previous CI does not transfer. CI results, passed tests, failing jobs and the tested commit SHA must be recorded on the PR before promoting it out of draft. Existing PR #150/#151 CI results do not transfer to this integrated tree.
