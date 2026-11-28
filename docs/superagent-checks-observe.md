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

This explicit Check Run lookup does not create a webhook, inferred Demand,
provider run, Council review, grant, GitHub write or Auto mode. Periodic
linked PR observation is described below. Ordinary bot and channel traffic
is unaffected when SuperAgent is Off.

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
pending count and previously unseen pre-activation count, then reloads native state. Load linked source decisions
to inspect individual recorded outcomes. These are observations, not task
acceptance, Council approval, GitHub writes or Auto execution.

The linked PR lookup requests `filter=all` rather than GitHub's default latest-only
filter so rerun identities are not silently omitted from the bounded response.
The entire returned window must fit the existing 100-run limit; this is not a
gap-free subscription or a claim about runs outside GitHub's API window. Valid
unseen completed runs predating the current Observe activation are counted
separately, not recorded, and cannot pin a source or advance its cursor. Already
recorded IDs still use immutable replay validation. The individual Check Run
command still rejects unseen pre-activation results. A malformed or conflicting
identity remains a batch failure, including for historical-only windows.

## Additional completed outcomes (v62)

GitHub-generated `stale` and `startup_failure` conclusions are preserved as
observed, non-authorizing outcomes rather than rejecting an otherwise valid
linked PR window. The v62 migration transaction copies every existing sidecar
row into a widened CHECK constraint, replaces the sidecar, and retains the
same primary and foreign keys. Legacy unknown receipts remain unknown.
Unknown conclusions still fail closed; replay cannot rewrite an outcome.
Keep a pre-upgrade database backup; older application versions cannot open
a v62 database, so downgrade requires restoring the matching backup.

Native regression:
`superagent::events::tests::check_run_v62_upgrade_preserves_receipts_and_accepts_native_failure_outcomes`.

## Periodic linked PR observation

An enabled Team with a Check Run Observe source and an explicitly linked PR
is eligible for periodic read-only Checks observation in desktop and live
serve. The existing Observe scheduler rotates over the native source/PR
link keys, selecting at most one link per interval independently of its
GitHub Events batch. It does not scan arbitrary repositories or discover
unlinked PRs. Failed links advance the in-memory rotation; after restart
selection starts from durable links and immutable receipts deduplicate
previous observations. This is a bounded best-effort read, not a gap-free
subscription or a guarantee about PRs outside the current head window.

The periodic path freezes the current enabled Team policy revision along
with the source and Ticket/link fence before calling the authenticated
adapter. Team disable/re-enable, source revocation, link changes or Ticket
revision changes during the request prevent receipt admission. Off sources,
disabled Teams and unlinked PRs do not initiate a periodic Checks request.
Explicit operator commands retain their existing behavior on disabled Teams.
No native grants, assignments, Council decisions or GitHub writes are issued.

Native regression:
`superagent::events::tests::background_linked_pr_checks_rotate_and_fence_team_activation`.
The regression covers selection, Team activation ABA, source revocation and
duplicate durable receipts after restart; real authenticated transport
verification remains separate.
