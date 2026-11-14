# SuperAgent GitHub Observe event coverage

The operator-triggered GitHub Events adapter is **read-only** and uses the
existing signed-in `gh api user` identity plus repository-scoped GitHub REST
Events pages. Configured Team sources default to **Off**. Observe only records
bounded, deduplicated decision metadata for the exact repository, actor, kind,
and issue/PR subject. It does not admit a Ticket, launch a Worker, request a
native Council verdict, post a comment or approve a review.

Supported GitHub REST Events actions include issue opening/editing, issue/PR
comments, PR opening/closing/reopening, review creation/update/dismissal and
PR diff review-comment creation. For backward compatibility the internal source
kind `pull_request_review.submitted` represents a GitHub REST
`PullRequestReviewEvent` with `payload.action=created`. GitHub webhooks use
a different review action name (`submitted`); webhook payloads are **not**
accepted by this operator-invoked Events adapter. Each review/comment is only
recorded against the typed `pull:<number>` subject. It is never a native
SuperAgent Council pass.

`check_run.completed` remains **unavailable** in Observe because the GitHub
Events feed does not provide a verified Check Runs stream. PR-to-issue linking
and CI status require separate source-backed adapters; PR titles/bodies and
review comments must not be interpreted as authenticated linkage or authority.
Cursor/receipt persistence survives restart, but the finite Events retention
window is not a gap-free subscription. If pagination cannot establish the
previous boundary, polling fails without advancing the cursor.

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

## Explicit authenticated GitHub Auto (schema v57)

GitHub Auto is configured on an existing **Observe** event source and its
explicitly enabled, single-Worker Team. An event source continues to use its
original Off/Observe authorization and receipt cursor; the separate native
\`sa_auto_source_policies\` sidecar stores Auto consent, its revision and
bounded hourly/daily intake quotas. Only authenticated \`issues.opened\` and
\`pull_request.opened\` events from the exact configured repository and actor
are eligible. The signed-in operator's own events are ignored.

The **Sources → Configure GitHub Auto** inspector exposes three native modes:
**Off** (no new intent), **Staging** (authenticated receipts recorded without a
Worker), and **Active** (a verified event may admit one read-only Worker using
the existing native Team/Ticket mechanism). Changing modes requires explicit
confirmation and a matching consent revision. The source must remain Observe;
turning its source Off or changing its revision revokes outstanding
source-linked Auto mandates and quarantines undecided intake receipts.

A bounded host monitor (desktop or headless) polls one eligible source per
cycle through the existing authenticated GitHub CLI adapter, at most three
pages per poll. It does **not** claim gap-free webhook/subscription delivery.
A durable composite \`(source_id,delivery_id)\` key, native policy revisions,
activation time, quota checks and a SQLite authorization trigger prevent
duplicate or pre-consent events from becoming new tasks. The provider dispatch
is guarded by the same native bot/project lease and provider session policy
as manual tasks. Source text is data; no GitHub issue, PR, comment, reviewer or
LLM output may create a tool grant, change the Team roster, authorize external
writes or accept evidence.

An uncertain provider handoff is **never retried automatically**. The outbox
records the original Ticket and an explicit uncertain/quarantined state;
restart quarantines in-flight claims and native assignment recovery revokes
unsettled authority. Manual or native Council evidence acceptance remains
required after worker completion. This feature is a bounded consented
read-only intake path, **not** a fully autonomous code-writing/review/release
pipeline or a guarantee that every GitHub event will be delivered. Follow
the original #148 task authorization, effects and provider safety gates for
any future writable Auto capabilities.

## Atomic authenticated Auto admission

GitHub Auto is a separate, explicitly consented source of native work. It
cannot claim the identity of a direct local-user instruction. A staged event
must have an exact durable authenticated receipt, active source policy and
matching source/consent revisions before a Worker can start.

For an authenticated Auto Worker, the Ticket actor and mandate actor are both
`github-auto`. Native admission atomically persists the Ticket, immutable Team
snapshot, scoped read-only mandate, Worker assignment and the receipt-to-Ticket
link in one SQLite transaction. If any source, consent or Ticket revision
changes, no partial Ticket/lease/session may be admitted. The legacy Ticket
`source` column is retained as its compatibility projection (`manual`
CHECK); **the authoritative Auto provenance** is the distinct actor and the
one-to-one record in `sa_auto_intakes`. It is never inferred from a text
prompt or sender name.

Provider/tool admission rechecks source consent and the exact linked receipt,
including after revocation, restart, callback replay and attempted cross-source
request-key reuse. A Ticket claiming `github-auto` with a missing link has
**no authority** and cannot fall back to manual credentials or grants. An
ordinary manual Ticket stays independent of GitHub Auto source activation.
An in-flight provider whose consent is revoked is blocked or interrupted,
never promoted to a successful completion by a stale callback.

## Source-origin inspection and retry authority

The Ticket Inspector displays an authenticated Auto event kind, subject
reference, receipt state and a live indication of whether the original
source's consent still authorizes it. It retains historical attribution even
when consent is revoked, a receipt is quarantined, or the source is removed
from the active poll set. A missing receipt is shown as **missing-receipt**,
not as a local-user request.

One-shot headless Ticket inspection exposes only the coarse authenticated
origin kind, event class, receipt state and current authorization boolean;
the repository, subject reference and delivery ID remain redacted. Native
manual retry accepts only explicitly local-user Tickets. Previously admitted
GitHub Auto work cannot obtain a second session through manual or headless
retry, regardless of whether its source was revoked. A future retry must
create a fresh, consented, independently authenticated native demand.

## Finalization checks for external-origin work

A native human acceptance or verified Council pass is a state-changing
finalization. Both paths must recheck the authenticated Auto receipt and its
current source consent at the moment of the state transition. Any revoked,
orphaned or mismatched source blocks completion even if a previous provider
result exists. Required native work-plan items must also carry independent,
verified evidence receipts before either Council or Council-Off finalization.
These checks are performed under the existing cross-process dispatch fence;
already finalized historical receipts remain immutable.
