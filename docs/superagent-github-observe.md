# SuperAgent GitHub Observe event coverage

The operator-triggered GitHub Events adapter is **read-only** and uses the
existing signed-in `gh api user` identity plus repository-scoped GitHub REST
Events pages. Configured Team sources default to **Off**. Observe only records
bounded, deduplicated decision metadata for the exact repository, actor, kind,
and issue/PR subject. It does not admit a Ticket, launch a Worker, request a
native Council verdict, post a comment or approve a review.

Supported GitHub REST Events actions include issue opening/editing/closing/
reopening, issue and PR assignment/unassignment and label changes, PR
opening/closing/reopening/merging, issue/PR comment creation, review
creation/update/dismissal and PR diff review-comment creation. The additional
triage actions record only exact event-kind, actor and typed thread metadata:
label text, assignee details and event payloads are never persisted. A merged
PR observation is not proof of SuperAgent delivery or authorization. A
`PullRequestEvent` closure with an explicit boolean `pull_request.merged=true`
is normalized to the existing `pull_request.merged` source kind; an ordinary
closure remains `pull_request.closed`. A contradictory `merged=false` with
the `merged` action is not accepted as proof of a merge. These events remain
read-only, deduplicated observations, including after restart. For backward
compatibility the internal source
kind `pull_request_review.submitted` represents a GitHub REST
`PullRequestReviewEvent` with `payload.action=created`. GitHub webhooks use
a different review action name (`submitted`); webhook payloads are **not**
accepted by this operator-invoked Events adapter. Each review/comment is only
recorded against the typed `pull:<number>` subject. It is never a native
SuperAgent Council pass.

For PR lifecycle/review events, a GitHub Events payload that includes both
`payload.number` and `payload.pull_request.number` must agree. If the PR
payload includes a base repository ID, it must match the independently
attested event repository ID. Conflicting metadata aborts the entire Observe
window without inserting a receipt, advancing the cursor, or linking new
Ticket context; a correct replay remains idempotent across restart.

`check_run.completed` remains **unavailable** in Observe because the GitHub
Events feed does not provide a verified Check Runs stream. PR-to-issue linking
and CI status require separate source-backed adapters; PR titles/bodies and
review comments must not be interpreted as authenticated linkage or authority.
Cursor/receipt persistence survives restart, but the finite Events retention
window is not a gap-free subscription. After the first poll, each bounded
window must contain the **exact persisted cursor event ID**. Finding only an
older event or a timestamp before activation cannot justify skipping the
missing interval. If the original cursor is outside GitHub's finite retention,
polling fails without advancing the cursor or writing any new decision;
the operator must investigate/reconcile the source history explicitly.
No inferred task, replay or Auto write is attempted.

Reference: https://docs.github.com/en/rest/using-the-rest-api/github-event-types

## Explicit Ticket thread correlation across sources

An operator can link one exact `issue:<number>` or `pull:<number>` to a native
Ticket. That link is **context only**, not evidence of a GitHub authorization or
a task launch. The Ticket's linked activity now combines previously verified
Observe receipts for that same Team, repository and typed thread across its
configured issue-comment, PR-review and Check Run source classes. The native
receipt timeline is sorted by observation time: GitHub Events IDs and local
Checks counters must not be compared as a shared upstream sequence. A source
in another Team/repository or an unrelated issue/PR never contributes.
Multiple event-class links to one thread do not duplicate receipts, and the
same Team/repository/thread cannot be linked to two different Tickets. These
relationships remain durable after restart. The operator must still explicitly
link the relevant PR thread: no title/body, issue reference or review comment
can infer the issue-to-PR relationship, and no Council/CI approval is implied.

Completed Check Runs use a separate authenticated Checks API lookup, not the
Events feed. Their result is immutable observed metadata, never a verified
SuperAgent acceptance gate or permission to execute a Worker.

## Operator revocation of explicit thread context

In the native Ticket inspector the local operator can choose **Unlink** next
to an exact `issue:<number>` or `pull:<number>` association and confirm its
removal. The main-window-only native command verifies the Ticket, source,
Team and precise typed key, then atomically deletes only that association and
records `source_thread_unlinked` in the Ticket's durable audit. A wrong
Ticket/source/key or a repeated unlink fails rather than affecting another
Ticket. Revocation is permitted even for terminal Tickets to correct a link,
but creating new links to terminal Tickets remains prohibited.

This does not delete verified GitHub Observe receipts or mutate source
cursors, approvals, Mandates or the Ticket lifecycle. Other explicit links
for the **same Team/repository/typed thread** can still expose its receipt
metadata; remove each authorized association to stop correlation. No GitHub
network operation, worker launch, external effect or Auto authorization
occurs. The Inspector refreshes links and invalidates its cached observed
activity after the native transition.

## Explicit issue-to-PR context (operator assertion)

The Ticket Inspector can associate an exact `pull:123` with an already-linked `issue:42` on the same Team, Ticket and repository. This is a **local operator assertion**, not an authenticated GitHub issue-closing relationship. The native SQLite association and audit commit together, are idempotent on replay and roll back together on error. Other Teams, repositories and Tickets cannot inherit this relationship. The existing unlink action revokes one exact context reference while retaining decision receipts and audit. No provider, task admission, auto-execution, Council approval or external write is implied. Only separately authenticated Observe decisions are displayed as activity.

## Case-insensitive GitHub source identities

GitHub actor logins and App slugs are compared case-insensitively. Creating an
otherwise identical source with different actor capitalization reuses its
existing source ID, policy revision, receipt history and cursor instead of
splitting observation into duplicate streams. New actor filters are stored in
lowercase. Existing sources are not silently rewritten; if legacy rows already
contain multiple case-only aliases for the same Team, repository and event
kind, source creation fails with an explicit ambiguity until an operator
reconciles the configuration. This never enables a source, admits a Ticket,
or authorizes a GitHub effect.

## GitHub repository display-case stability

Configured source repositories remain exact canonical lowercase `owner/repo`
identities. Authenticated GitHub Events API responses can display the same
repository with different capitalization; the native feed validator and typed
event parser compare that identity case-insensitively without permitting
similar prefixes, extra path segments, foreign repositories or missing IDs.
The stored decision retains the configured canonical repository identity,
and replay of the same delivery with a case-only actor variation is
idempotent. Existing receipts and cursors are preserved across restarts.
No wider repository filter, new subscription, Auto execution or write
authorization is introduced.

Legacy source rows with mixed-case repository identities are compared with
ASCII case-insensitive equality when reusing a source or correlating an
operator-linked issue/PR thread. A case-only alias cannot associate the same
Team/repository/thread with a different Ticket, and cross-source authenticated
receipts are visible only through an explicit link to that Ticket. Existing
source IDs, cursors and audit records are not rewritten; ambiguous case-only
legacy duplicates still fail closed. Authenticated events for a legacy
mixed-case source are compared against its exact GitHub repository identity
and recorded under the canonical lowercase repository key, without losing
existing receipts or creating a Ticket. This remains Observe-only context,
never a GitHub grant or inferred demand.

## Per-window repository identity consistency

Every explicit live Observe poll now authenticates the operator through
`gh api user`, resolves `gh api repos/{owner}/{repo}`, and requires an exact
repository `full_name` (case-insensitive) and positive numeric `id`. Every
event in the subsequently fetched bounded Events window must contain that
same repository ID. A renamed, deleted/recreated, redirected, foreign or
inconsistent repository cannot reuse the previous display name and silently
advance the source cursor. The entire failed poll leaves decision receipts
and the cursor unchanged. An empty verified feed is valid.

The internal legacy-fixture parser still accepts all-ID-free test windows for
backward compatibility; the **live authenticated adapter** does not. This is
a read-only extra repository-metadata request after explicit Observe opt-in,
not a durable GitHub subscription or Auto authorization. Source configuration
still defaults Off, and no Ticket, Worker, Council or external write is created.


## Stable authenticated operator during Observe

The native Events and Checks API adapters recheck `gh api user` after
their bounded GitHub reads and before committing any Observe receipt. If the
signed-in GitHub account changed, is missing, or cannot be reauthenticated,
the observation fails closed: the source policy, cursor, receipts and linked
Ticket context remain unchanged. Case-only variations of the same login
are accepted. This is an additional read-only authentication check, not a
GitHub subscription, permission grant, Council verdict or Auto trigger.

## Pinned authenticated repository identity (schema v48)

Every explicit authenticated GitHub Events or Checks API observation resolves the repository's numeric GitHub ID through `gh api repos/<owner>/<repo>`. The first successful observation pins that ID to the source within the same SQLite transaction as decisions and cursor updates. Subsequent polls reject a changed ID, including a repository deleted and recreated under the same name, without altering existing receipts or advancing the cursor. Disabling and re-enabling Observe does not reset the pin.

Sources created before v48 preserve their historic receipts, but those historic records have **unknown repository-ID provenance**; migration does not fabricate an ID. Their first successful new authenticated observation establishes the pin prospectively. This remains **Observe-only**: no task admission, GitHub Auto, external effect or Council approval is authorized by these receipts. Operators can inspect the pinned `repositoryId` (or `null` before attestation) through source-integrity diagnostics. If the ID changes, the existing source must not silently rebind; investigate the repository identity and configure an independently authorized replacement source.

## Cross-source thread correlation and numeric repository identity

Existing links made before repository attestation do not permit a later source to bind the same Ticket to a different numeric repository ID. Every authenticated identity pin rechecks same-name repository links on that Ticket inside the receipt transaction. A conflict denies the pin, receipts and cursor advancement; explicitly reconcile the affected links before retrying. Independent Tickets and different repository names remain unaffected. No execution authority is granted.

An explicit Ticket issue/PR association and its source-linked activity compare
the independently pinned numeric GitHub repository IDs when available. Two
source classes with the same owner/name but different IDs never share receipts,
including after deletion and recreation of a repository. A pinned source does
not silently match a source whose repository identity is still unknown.
Existing legacy associations where **both** sources have unknown identities
remain readable as historical context, not authenticated proof of sameness.
For exclusive operator link ownership, different **known** numeric IDs may
hold the same issue/PR number on separate Tickets when a repository name was
reused. Matching IDs still conflict; a missing identity remains conservative
rather than permitting an ambiguous historical link.
Each source's own explicitly linked receipts remain visible without an
unrelated source's attestation. Cross-source activity and pagination accept
only receipts recorded at or after the observing source's repository
attestation. A later identity pin cannot retroactively authenticate older
name-only receipts; they remain inspectable under their original source or
an explicit link to that source. Operators can explicitly observe eligible
sources to establish prospective identity pins; importing, linking, or reading
a receipt never issues a grant, infers a GitHub task or starts Auto.

### Explicit desktop issue intake

From an enabled, supported Council-Off Team, open Sources, load an `issues.opened` or `issues.reopened` Observe source, and view its decisions. Select a currently permitted issue, an eligible configured Worker and an operator-written objective. Review and confirm the exact Team/source revisions. The desktop command uses the same atomic staging operation as `--queue-observed-issue`; it creates a blocked manual Ticket and source link, not a provider session or write grant. Use the existing Demand retry control for a later explicit read-only launch. A duplicate issue, stale source, identity mismatch or unsupported roster is rejected by native admission. An unresolved history gap at the current durable cursor also blocks new issue Demand staging until an explicit authenticated poll verifies that exact anchor. Existing request-key replays remain read-only, and manual intake without an Observe selection is unchanged. Observe polling itself never admits work.

### Overlapping authenticated Observe requests

Desktop, background subscriptions and headless polling share the native receipt transaction. If another request advances the source cursor while a GitHub Events window is in flight, the older window cannot advance beyond that new cursor unless it contains the exact newly committed event identity. A window that cannot establish continuity is rejected without recording receipts or advancing the watermark; fetch a fresh window using the current source cursor. Duplicate or older already-observed deliveries remain idempotent. This is an Observe-only safeguard, not Auto admission or permission to write to GitHub.

Background history-gap diagnostics also bind the Team activation revision at the atomic receipt write. A revoked or Off → On Team cannot be quarantined by a stale in-flight background failure, even if a separate headless connection changes the Team after preflight. Explicit local-operator Observe retains its existing independent recovery behavior. No diagnostic authorizes a task or GitHub write.
