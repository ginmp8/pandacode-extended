# Extended SuperAgent orchestration

> **Implementation status:** Incomplete. At HEAD `bce1a51e` the persistent Orchestrations workspace, manual read-only Ticket flow, scoped-write candidate, exact-effect candidate and source-policy groundwork exist. Team Builder creates and edits ordered worker/reviewer membership; an edit now appends a new immutable Team version and policy revision for future Tickets, while active Tickets keep their admitted snapshot, mandate, assignment and write grants. Required Council is selectable in Team Builder (two to eight distinct reviewers) and manifest v2 carries `councilRequired` without downgrading; the latest green CI run (`43648593574`, 19/19 jobs) covers that commit `7747c538`, while HEAD run `43654608546` had no conclusion when this was recorded. For a Ticket pinned to `council_required`, the native adapter binds the finished worker artifact (artifact digest, ticket revision, workspace identity, policy revision) and dispatches exact task-bound native reviewers; finalization requires a fresh verified native Council verdict or an attributed, reasoned human exception. Manual acceptance remains available when the policy is not `council_required`; the attributed exception exists natively but is not exposed in the desktop UI or headless CLI. Multi-worker scheduling and the work-item graph remain incomplete. Windows scoped writes were repaired locally (39/39 focused scoped-write/exact-grant tests) and the effect-stop race was serialized (11/11), but Unix writes remain disabled and HEAD has no green CI. S4's manual one-shot Observe slice fetches bounded metadata for an exact linked thread across six event classes; Auto, durable recovery/headless parity, nested subagents, EGM-04 durable findings and EGM-07 Plan/Readiness remain outstanding. No overall acceptance is claimed.
> **Requested push branch:** feat/issue-148-pe. HEAD `bce1a51eb23ccba97cd4b3953d36009acf12298f` (`bce1a51e`) is pushed on `origin/feat/issue-148-pe`. The latest green CI is run `43648593574` at `7747c5380aeb83718ce15715b6a0f3ba32ab496c` (19/19 jobs); HEAD's run `43654608546` had no conclusion when this was recorded, so no green-CI claim is made for HEAD. Local Windows checks passed: native clippy and the manifest round-trip test for required Council; the focused native tests for versioned Team edits; the scoped-write/exact-grant suite (39/39); and the effect-stop test (11/11). The Council Team Builder UI tests were CI-tested at `7747c538`, while the future-only Team edit UI test was not run locally and is not CI-tested.

**Authoritative specification:** [Issue #148](https://github.com/ginmp8/pandacode-extended/issues/148), extending [Issue #131](https://github.com/ginmp8/pandacode-extended/issues/131). Addenda: [EGM-01–07](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817527256), [DD-01–14](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817744113), [SA-01–09](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817864073), [NS-01–12](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817979555).

**Integration target:** extended/development

**Working source baseline:** f9efb3a1c7558dcf7d5688bb1687cfff9253f967

**Requested remote branch:** feat/issue-148-pe

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
| S4 — Sources/Auto | Manual authenticated local-gh Observe for one exact linked thread, Off/Observe, allowlists, per-thread cursor/dedup and privacy disclosure. Polling, source-to-Team routing and Auto remain unavailable until S1/S5 gates pass. |
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
| S1 scoped writes | Partial: Windows scoped writes pass 39/39 focused scoped-write/exact-grant tests and the effect-stop race passed 11/11 local runs; Unix writes remain disabled; HEAD CI has no conclusion, so this is local Windows evidence only. |
| S2 UX-P1 persistent workspace | Implemented; covered by the green `7747c538` browser run; preserve non-modal route |
| S2 multi-role Teams/work graph | Partial: Team Builder supports ordered Workers and Reviewers and appends a new immutable Team version for future Tickets; active Tickets keep their saved snapshot, mandate, assignment and grants. Required-Council reviewer dispatch is native; multi-worker scheduling and the full work graph remain incomplete. |
| S3 task-bound Council | Partial: required Council is selectable in Team Builder and carried by manifest v2 without downgrade; required-Council admission freezes the reviewer policy, the worker result binds the exact artifact/workspace/ticket/policy identity, and native reviewers are dispatched and finalized only from a revalidated native verdict. An attributed human exception requires the same current binding and a recorded reason but has no desktop or headless surface. Green run `43648593574` at `7747c538` covers the named required-Council UI/native tests; HEAD `bce1a51e` is not CI-tested. A WorkItem/Run binding and a real required-Council browser journey remain absent. |
| S4 manual Observe / Auto | One-shot linked-thread metadata intake supports exactly six event classes and has focused native/UI tests; those tests are covered by the green `7747c538` run, not by a run at HEAD. Auto and background intake remain unavailable. |
| S5 recovery/headless/subagents | Incomplete: checkpoint resume, general effects, headless lifecycle parity and nested subagents remain absent. The effect-stop race is locally tested (11/11), not CI-tested at HEAD. |
| S6 exact-SHA evidence | Partial: the acceptance ledger now records a checkpoint delta with green `7747c538` evidence and local Windows checks; HEAD CI has no conclusion and no final-SHA proof or screenshots exist. |
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

Team Builder creates a manual Team with one Coordinator, 1–16 ordered Workers
and up to 8 ordered Reviewers. An edit appends a new immutable Team version and
policy revision for future Tickets, including when the Team is enabled or already
has Ticket history; the native command remains the final revision and eligibility
check and rejects a stale revision. Active Tickets keep the role/profile snapshot,
mandate, assignment and write grants they were admitted under. `workerBotId` is
retained as the first ordered Worker for compatibility. A no-op edit returns the
current Team without a new version. Required Council is a per-Team choice that
validates two to eight distinct reviewers; manifest v2 carries `councilRequired`
and normalizes a legacy `councilEnabled` gate to required so import cannot
silently downgrade it.

Ticket evidence reads `teamSnapshot` from that Ticket's native snapshot response
and exposes its provenance, ordered roles and the exact returned payload. The UI
does not substitute the current Team if a saved snapshot is absent. `legacy-current`
means the mutable Team row observed during upgrade; it may not represent the
historical admission state. `legacy-unknown` preserves only older saved identity
evidence; synthetic placeholders and missing profile data are labeled as
unknown, not reconstructed. Reviewer membership becomes operative for a Ticket
whose policy is `council_required`: admission freezes the exact reviewer roster
and policy, and the finished worker artifact is bound and dispatched to those
native reviewers. Council-off Tickets keep manual acceptance, and a complete
below-target required round can be accepted only through the attributed,
reasoned native command; that command is not exposed in the current desktop UI
or headless CLI. The multi-role editor does not claim parallel worker
scheduling; the S2 work graph and scheduler remain incomplete.

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
