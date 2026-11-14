# Extended SuperAgent orchestration

> **Integration status:** PR #154 extends the previous #153 reconciliation
> with a draft native Demand scheduler, immutable WorkItem dependency graph,
> existing-Team Worker selection, isolated child Tickets and a user-confirmed
> exact-file write capability for eligible Codex Workers. Once each native child
> Ticket reaches its real completion gate, the scheduler unlocks dependent work
> and can close the root Demand without an extra continuation prompt. This
> The candidate now includes an explicit, bounded **read-only GitHub Auto**
> pipeline and native Worker-created child/grandchild requests behind separate
> local grants and provider/tool checks. Neither path authorizes unrestricted
> shell/MCP/file/GitHub effects. The implementation does **not** satisfy all
> #131/#148 acceptance criteria or substitute for exact-SHA runtime evidence.
> Unix scoped file writes remain denied until a verified native adapter exists.
> No draft source or unexecuted test is proof of issue completion.
>
> **Integration branch:** `feat/issue-148-unified-150-151-v2`; see
> [the Demand scheduler contract](superagent-demand-scheduler.md).
> Earlier source-PR descriptions, worker-reported checks and historical
> review packets are not independent proof for the integration HEAD.

**Authoritative specification:** [Issue #148](https://github.com/ginmp8/pandacode-extended/issues/148), extending [Issue #131](https://github.com/ginmp8/pandacode-extended/issues/131). Addenda: [EGM-01–07](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817527256), [DD-01–14](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817744113), [SA-01–09](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817864073), [NS-01–12](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817979555).

**Integration target:** extended/development

**Working source baseline:** f9efb3a1c7558dcf7d5688bb1687cfff9253f967

**Current integration branch:** feat/issue-148-unified-150-151-v2

## Purpose and ownership

Implement durable native SuperAgent orchestration using existing bots, gateway, queue, provider/harness enforcement, Skills, Architect/Council, GitHub and headless runtime. Do not create another bot registry or reviewer scorer. Direct chats, shared channels, permissions and existing manifests remain backward-compatible. Autonomy is Off by default; each source and side effect requires explicit, versioned, native authority.

The standalone design packet and source map are [here](extended-superagent-design-review.md) and [here](extended-superagent-source-map.md). S0 reviews are not Ticket approvals and do not grant authority.

## Delivery sequence

| Stage | Required behavior |
| --- | --- |
| S0 — Design/source baseline | Freeze goals, glossary, roles, constraints, source map, migration/rollback, journeys and acceptance IDs. Complete four isolated reviews with context disclosure, deduplicated findings, dispositions and honest not_run states. |
| S1 — Native authority/leases | Durable Demand/Mandate/Run, manual admission, exact scoped writes, project/resource leases, correlated callbacks, stop/revoke and settlement. Unsupported OS/harness remains disabled. |
| S2 — Teams/Demands/Runs | Versioned Teams; Coordinator, multiple Workers, optional native Reviewers; role policy/Skills/budget snapshots; work graph/scheduler; compatible manifest and UX-01–24. |
| S3 — Council | Bind Team, Ticket, WorkItem/Run, assignment, artifact, cycle/round and reviewer policy; require a fresh native verdict or attributed human exception. |
| S4 — Sources/Auto | Manual authenticated local-gh Observe for one exact linked thread, Off/Observe, allowlists, per-thread cursor/dedup and privacy disclosure. A draft bounded local-GitHub polling/Auto intake exists behind separate exact-source grants; no ambient write privileges, default Off and provider validation remain required. |
| S5 — Recovery/effects/headless/subagents | Restart/checkpoints, CAS effect claims/read-back, stop races, budgets/watchdogs, serve parity and policy-bound nested-child lineage. |
| S6 — Exact-SHA verification | Trace AC-01–31, UX-01–24, EGM-01–07, DD-01–14, SA-01–09 and NS-01–12 to tests, exact SHA, CI, screenshots and residual risk. |

## Autonomous implementation gates

1. Re-read #148, #131, relevant addenda and current HEAD before each broad stage. Keep one writer per overlapping scope; use Router with profile auto.
2. Implement native behavior before UI claims. Verify focused Rust, TypeScript, Playwright and applicable platform targets. Never mark an unavailable platform supported.
3. Begin file tracking before every edit batch. Open prose files in the sidebar after writing.
4. Preserve history and linear branch. Never force-push, merge, release or close issues. Commit and push only after all stages and exact-SHA checks pass.
5. Any new automatic network request must be disclosed in README and website privacy page before enabling it.
6. Missing reviewers/checks are not_run, never pass. Worker, child or design-review output cannot finalize a Ticket.

## Verification ledger

| Checkpoint | Status |
| --- | --- |
| #148/#131 and EGM/DD/SA/NS | Mapped; S0 independent review pending |
| S0 source map | Updated for #148 baseline and candidate boundaries |
| S1 scoped writes | Integrated native exact-path grants and provider ToolHost; Windows-supported/Unix-denied boundaries; integration-head tests and effect replay gates pending |
| S2 UX-P1 persistent workspace | Implemented and browser-verified; preserve non-modal route |
| S2 multi-role Teams/work graph | Partial: a bounded persisted native WorkItem DAG now admits eligible Team Workers through separate child Tickets and native checkout leases; real multi-checkout parallelism, automated quality gates and broader scheduler recovery remain outstanding. |
| S3 task-bound Council | Integrated candidate native independent reviewer start, exact Ticket/artifact correlation and completion callbacks; native acceptance tests and final integration SHA still unverified |
| S4 manual Observe / Auto | Partial: native 24-hour, exact-repository/actor/event operator grant, authenticated bounded issue-opened polling, read-only Ticket intake and revoke; full action-class policy, controlled external write effects and disposable-repository evidence remain outstanding |
| S5 recovery/headless/subagents | Partial: native Worker-created descendants, depth/count limits, immutable lineage, Council parent gate, root Stop cascade and shared desktop/headless scheduler; full restart/fault-injection, cross-process oracles, nested provider compatibility and headless write admission remain unverified. |
| S6 exact-SHA evidence | In progress on PR #154; source-level implementation is not compiled/verified for the final integration HEAD |
| Live GitHub smoke | not_run until a disposable authorized repository is available |

## Existing safe import and thread behavior

Companion-manifest import preflights every referenced bot, writes a verified
configuration-only snapshot before mutation, and imports presets disabled.
Snapshots exclude Tickets, Mandates, sessions, credentials, memory and approvals.
Legacy bot manifest and Git sync remain separate; importing bots or channels
never enables an orchestration. Restore requires a new preview and confirmation.

Ticket templates only fill names; both existing bot IDs must be selected
explicitly. They do not create profiles, enable a policy, grant tools or start work.

## Team Builder and Ticket role evidence

Team Builder creates a disabled manual Team with one Coordinator, 1–16 ordered
Workers and up to 8 ordered Reviewers. It can update a Team only while disabled
and before its first Ticket; the native command remains the final revision and
eligibility check. `workerBotId` is retained as the first ordered Worker for
compatibility. Every changed Team edit creates a new immutable Team version.

Ticket evidence reads `teamSnapshot` from that Ticket's native snapshot response
and exposes its provenance, ordered roles and the exact returned payload. The UI
does not substitute the current Team if a saved snapshot is absent. `legacy-current`
means the mutable Team row observed during upgrade; it may not represent the
historical admission state. `legacy-unknown` preserves only older saved identity
evidence; synthetic placeholders and missing profile data are labeled as
unknown, not reconstructed. Reviewer membership is descriptive only until the
S3 native Council adapter is bound to a Ticket. The multi-role editor does not
claim parallel worker scheduling or reviewer execution; the S2 work graph and
scheduler are still incomplete.

The trusted local Ticket detail can link an exact issue:NUMBER or pull:NUMBER to
a same-orchestration Off/Observe source. SQLite v42 stores up to 16 references,
rejects foreign tasks/conflicting owners and preserves links across restart.
This is manual context metadata, not authenticated delivery, Auto or write authority.

## Manual GitHub Observe intake

An event source starts Off with an exact repository, actor and event class. The
user can switch it to Observe, then click **Fetch once** beside an already-linked
issue or pull request in Ticket evidence. The click names the exact repository
and thread and performs one read: `gh auth status`, followed by one bounded
GraphQL request to `api.github.com/graphql` through the user's local `gh` CLI.
It uses the permissions already granted to that CLI and asks for no new GitHub
access. There is no polling, subscription or request on app launch or page load.

This slice supports exactly these six event classes:

- `issues.opened`: issue database ID and author login.
- `issues.edited`: the latest issue `UserContentEdit` node ID, editor login and
  `editedAt`. The node ID is hashed for the receipt key; the timestamp in
  milliseconds is used as the thread cursor. No title, body or edit diff is read.
- `issue_comment.created`: latest comment database ID and author login.
- `pull_request.opened`: pull request database ID and author login.
- `pull_request_review.submitted`: latest review database ID and author login.
  It also reads `submittedAt` only to reject pending drafts; the timestamp is
  not retained.
- `check_run.completed`: latest check run in the latest check suite on the exact
  pull request's latest commit, only when GraphQL reports `COMPLETED` and a
  completion timestamp. The check run database ID is its receipt/cursor value;
  the owning GitHub App slug is its actor allowlist identity.

Every other GitHub class or action is unsupported. This includes other
`issues.*`, `issue_comment.*`, `pull_request.*`, `pull_request_review.*` and
`check_run.*` actions, plus all `check_suite.*`, `workflow_run.*`, commit status,
push, release and other webhook classes. This is a manual snapshot of the
latest matching metadata, not a subscription or a complete event history. It
does not request issue or PR titles/bodies, comment or review text, edit diffs,
check names/conclusions/output or model text. A missing CLI, failed
`gh auth status`, API or GraphQL error, truncated response, missing actor/ID or timestamp,
unfinished check run, or unexpected repository/thread fails closed.

The native side verifies the source is still Observe, the exact thread is linked
to that source's orchestration, and the repository/event/actor match the saved
allowlists. It rechecks the source revision and thread link after the request;
turning the source Off or changing its revision during the request prevents a
receipt from being recorded. Event data is untrusted metadata and cannot start
or launch a Ticket, change policy, grant tools or enter a worker prompt.

The existing SQLite decision table stores the source, stable event ID, returned
repository, actor, event class, exact thread key, event-specific sequence,
decision, source revision and local record time. Check run sequence values are
GitHub database IDs; issue edit sequence values are edit timestamps in
milliseconds. No GitHub text or response body is retained. Receipts are capped
at 10,000 per source and remain in local app data until that data is removed.
Replays return the original decision; each linked thread has its own monotonic
cursor, gaps are accepted, and older out-of-order values are recorded as
ignored. Turning a source Off blocks the next fetch before even
`gh auth status`, so Off sends no request.
