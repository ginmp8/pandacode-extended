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
