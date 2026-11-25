# SuperAgent authenticated Checks API observation

`Bots → Orchestrations → Sources` can create an explicit `check_run.completed`
source for a single canonical repository and GitHub App slug. The source starts
**Off**, and enabling **Observe** requires a local user action and current
policy revision. A local operator then enters a positive Check Run ID and
selects **Observe authenticated Check Run**.

The native main-window command `superagent_source_observe_check_run` checks
policy *before* making a GitHub request, obtains the signed-in operator from
`gh api user`, and performs one read-only
`gh api repos/{owner}/{repo}/check-runs/{check_run_id}` request. A response
must identify that exact completed run, a supported conclusion, its exact
GitHub App slug and **exactly one** PR with the same 40-hex-character head SHA
whose API-provided `base.repo.url` identifies the exact configured repository.
A missing or foreign base repository rejects correlation even when the PR number
and head SHA match. The PR's numeric `base.repo.id` must also match the
repository ID independently obtained by the authenticated `gh api repos/...`
lookup. A matching owner/name or REST URL alone is insufficient because
repository names can be reused after deletion. Missing, malformed or mismatched
IDs fail the entire observation atomically, without advancing the source cursor
or persisting a source pin or outcome.
Missing PR associations (including some fork checks), multiple matching PRs,
a changed PR head, an unmatched App, a Check Run before Observe activation,
a stale policy revision or an invalid response all fail **without recording
an event or advancing a cursor**.

The GitHub Checks API `pull_requests` list identifies matching open PR heads,
**not necessarily the PR that triggered the check**. The receipt is therefore
context for an explicitly linked `pull:<number>` thread, not evidence of
delivery ownership, a passing CI gate, a completed task or source authority.
No title, full output, annotations, issue text or private GitHub content is
persisted. Stored receipts are bounded and deduplicated by the immutable
Check Run ID. The per-source `lastSequence` for Checks is a **local observation
counter** (unlike the GitHub Events feed cursor), permitting out-of-order
explicit lookups without inventing gap-free subscription guarantees.
Replays return the original receipt; changed identity/head/PR correlation
is rejected rather than silently reassigned. The decision and counter are
committed together in SQLite.

This capability does not create a webhook, background poller, inferred
Demand, provider run, Council review, grant, GitHub write or Auto mode.
Ordinary bot and channel traffic is unaffected when SuperAgent is Off.

Verification: native tests
`superagent::events::tests::check_run_observe_requires_explicit_enable_and_never_admits_work`
and `superagent::events::tests::check_run_correlation_denies_ambiguous_missing_stale_or_changed_payloads`
(including missing, foreign and lookalike PR base-repository URLs).
The unit tests use API-shaped fixtures; a real signed-in GitHub CLI and
private repository permissions remain separate operational verification.


## Durable, non-authorizing Check Run outcome (v45)

For newly observed completed Check Runs, an additive SQLite sidecar persists
the authenticated `conclusion`, exact `head_sha`, and `completed_at`
with the original decision in the **same transaction**. Source decisions and
explicitly linked Run activity display **Observed Check**, not a passing task
gate or Council verdict. Native queries exclude full output, annotations,
workflow payloads and private text.

Replays changing conclusion, SHA or completion time are rejected without
advancing the cursor or changing the receipt. Existing v41-v44 decisions
remain intact after v45 migration; outcomes absent at their original
observation remain explicitly unknown, never inferred or backfilled from a
later API response. A sidecar write failure rolls back the complete receipt
and cursor. This schema change is additive; operators should retain a
pre-upgrade backup, as older builds cannot open a v45 database.

## Replay after disabling and re-enabling Observe

A Check Run attested during an earlier Observe policy revision remains an
immutable duplicate if the operator later disables and re-enables that source.
Replaying it does not increase the local observation counter, rewrite its
recorded conclusion, or create a task. A Check Run **not previously recorded**
whose completion predates the current Observe activation is still denied,
even if the API returns it after reactivation. Off still makes no authenticated
requests. This behavior is verified by a native SQLite restart regression,
not by a real GitHub provider run.

## Observe a linked PR from Desktop or headless

From the Demand Inspector, load the Team's Sources and explicitly link the
exact PR thread to an enabled Check Run Observe source. **Observe linked PR
Checks** appears only on that source/thread association. It uses the same
native authenticated adapter as the existing headless linked-PR Checks
operation. The native command freezes the Ticket revision, source revision
and link generation before network IO, verifies a bounded PR-head window,
and rechecks context in the receipt transaction. No API payload comes from
the Webview. A replacement App with the same slug cannot contribute a
pending-only count after a numeric App identity is pinned; an empty or
pending-only window never creates a new pin. A replaced or moved PR head
aborts the observation. The UI reports the head, new receipts, duplicates
and pending count, then reloads native state. Load linked source decisions
to inspect individual recorded outcomes. These are observations, not task
acceptance, Council approval, GitHub writes or Auto execution.
