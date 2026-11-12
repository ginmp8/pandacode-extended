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
