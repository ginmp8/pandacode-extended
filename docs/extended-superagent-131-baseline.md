# SuperAgent #131 — normative baseline appendix

**Status:** Source-pinned verbatim appendix for the frozen SuperAgent design packet. This file adds no requirement and changes no #131 wording.

## Source pin

| Field | Value |
| --- | --- |
| Issue | [#131 — feat(extended): native SuperAgent orchestration with existing bots and durable tasks](https://github.com/ginmp8/pandacode-extended/issues/131) |
| Repository | `ginmp8/pandacode-extended` |
| Issue node ID | `I_kwDOVUqyy88AAAABdk49dA` |
| Issue state at fetch | `open`; author `ginmp8` |
| Body version | `created_at` `2026-11-09T04:14:20Z`; `updated_at` `2026-11-11T06:09:40Z` |
| Body ID and URL | GitHub exposes no separate body ID. The normative body is the `body` field of issue node `I_kwDOVUqyy88AAAABdk49dA`; canonical URL: https://github.com/ginmp8/pandacode-extended/issues/131 |
| Comment IDs and URLs | None. GraphQL `comments(first: 100) { totalCount }` returned `0`; REST `GET /repos/ginmp8/pandacode-extended/issues/131/comments --paginate` returned no comments. No comment ID or URL exists. |
| Fetch time | `2026-11-14T19:47:27Z` |
| Fetch command | `gh issue view 131 --repo ginmp8/pandacode-extended --json number,title,url,body,comments` |

**Normative source:** the issue body only. The issue had no comments at fetch time, so no comment can be a normative source for this appendix.

## Coverage and fidelity

- Requirements found verbatim: **8 of 8**. Acceptance criteria found verbatim: **31 of 31**.
- Missing IDs: **none**. No `MISSING FROM SOURCE` row is required.
- Normative comments: **0**.
- Reproduced source sections: the eight `### REQ-xx` sections; the 31-row acceptance matrix; the six branch-alignment acceptance expansions the source marks "same AC IDs"; and the six branch-specific constraints that the source's implementation rule declares part of the requirement set.
- The source acceptance table is split by a blank line after `AC-25`. This appendix merges the two fragments into one three-column table. No row wording, ID, order or cell content changed.
- Quoted blocks add only a `> ` prefix for visual delimitation. No other Markdown normalization is applied: emphasis, code spans, arrows, em dashes and link targets are the source's.
- The JSON block inside the REQ-04 section is the source's "Illustration only — proposed, not an existing supported manifest". It is reproduced because it is inside the requirement section; it is not a schema or a normative requirement.

## Branch-specific constraints declared part of the requirement set

The source's implementation rule states that the six constraints below are part of the requirements and the acceptance matrix. The full source subsection is reproduced, including its dated heading and the implementation rule.

> ### Current `extended/development` alignment — 2026-11-11
>
> Rechecked the **actual target branch**, not `main`, `base/development` or an unmerged feature branch: [`4cf491afda7a`](https://github.com/ginmp8/pandacode-extended/commit/4cf491afda7aa68a1ab6893e418783459fdbbafd). This is a read-only source/contract inspection, **not** a new claim of executable SuperAgent or end-to-end Council acceptance.
>
> 1. **Gateway admission is per session, not per bot.** [`bots/gateway.rs` lines 178–222](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/bots/gateway.rs#L178-L222) checks `agents.contains_key(session_id)` and the Architect state of that same session. Independent SuperAgent sessions for one bot can bypass that busy check. SuperAgent must own durable **cross-session per-bot admission/leases**, configured concurrency and resource locks, while continuing through the existing gateway/queue. Do not call this an already-implemented per-bot semaphore.
> 2. **Native bot/session records are not durable Ticket storage.** [`bots.rs` lines 64–69 and 113–116](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/bots.rs#L64-L116) uses `bot_threads` and `bot_tasks` rows attached to sessions and bots with cascade deletion; [bot reset/delete](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/bots.rs#L705-L737) removes owned sessions/records. SuperAgent's Ticket/Mandate/audit must survive removal or reset of the original bot/session, while future assignments become unavailable/revoked; no cascading historical deletion.
> 3. **Completion and notification require a separate gate.** [`gateway::run_finished` lines 355–436](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/bots/gateway.rs#L355-L436) delivers normal non-desktop or schedule results and drains the session queue at turn boundaries. A new orchestration assignment must prevent ordinary bot replies/notify-target changes from becoming a premature or duplicate user-visible final result before native evidence and optional Council approval. Preserve existing non-orchestration behavior.
> 4. **Manifest safety is not all-or-nothing import.** [`extended_bot_manifest::apply_with_backup`](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/extended_bot_manifest.rs#L128-L140) verifies and backs up before invoking [`headless_bots::apply`](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/headless_bots.rs#L20-L70), which reports per-bot/channel failures and retains earlier successful entries. Keep legacy behavior compatible, but make orchestration dependency preflight and orchestration-state commit atomic, with precise partial-import reporting for mixed legacy documents. Existing [manifest validation](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/extended_bot_manifest.rs#L39-L52) requires a `bots` array and enforces a size limit; explicitly define whether orchestration-only files use a separate path.
> 5. **Council exists in code, but S7 acceptance remains blocked.** [`docs/council-acceptance-s7.md` lines 27–56](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/docs/council-acceptance-s7.md#L27-L56) records unfinished executable integration/adapter evidence. S3 may implement its thin task-to-Council adapter against present native contracts, but must **not** claim dependency readiness or pass if required exercised native evidence is still missing. Deferring owner-run interactive checks does not convert blocked **automated** acceptance into passed; however, pending manual/user-run S7 checks must not block implementation, stage progression, or the next automated integration step.
> 6. **Desktop and headless entrypoints differ.** [`extended_bot_manifest::check_main`](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/extended_bot_manifest.rs#L183-L203) requires the main Webview window for desktop import; [`headless.rs` start services](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/headless.rs#L168-L251) and [headless import](https://github.com/ginmp8/pandacode-extended/blob/4cf491afda7aa68a1ab6893e418783459fdbbafd/src-tauri/src/headless_bots.rs#L558-L610) follow separate paths. Orchestration administration, approvals, stop/recovery and subscriptions need explicit authorized headless contracts, never a bypass of desktop checks or an unauthenticated new endpoint.
>
> **Implementation rule:** treat these six branch-specific constraints as part of REQ-01–REQ-08 and the acceptance matrix below. Existing ordinary Bots, direct/channel collaboration, native Council, imports and headless commands must continue to work unchanged when orchestration is disabled.

## Requirements (verbatim)

The eight source headings are reproduced as headings; the body under each heading is the exact source body in a blockquote.

### REQ-01 — Modular boundary and source ownership

> 1. Add a compact **Extended-owned** orchestration module. Its public operations should be semantic (create/configure orchestration, admit task, assign, receive result, pause/stop/resume, evaluate event, list state), not expose low-level SQL or force callers to orchestrate internal steps.
> 2. Reuse the **existing** bot runner, queues, session/worktree storage, approval pipeline, collaboration transport, GitHub integration and native Council. No second bot registry, agent runtime, event bus, skill library, generic plugin host or independent service.
> 3. Coordinator is an **existing bot ID** in a task-scoped coordination role. The native backend remains authoritative for policies, state transitions and completion. Prevent coordinator shell/file writes and unapproved external tool calls through **effective tool/provider enforcement**, not merely a system prompt. A harness unable to enforce the required scope must be blocked.
> 4. Keep upstream/Architect defaults unchanged. Make all new storage additive and migrations idempotent. Verify actual ownership and upgrade patterns before creating tables.
> 5. Avoid conflating model selection (Panda Router) with task coordination. A task may use any supported harness, including DeepSeek, Codex or another existing provider.

### REQ-02 — Durable Task (Ticket), Mandate, Assignment and lifecycle

> Persist at minimum:
> - **Orchestration config:** ID, name, enabled state, selected local coordinator/worker IDs, project scope, policy revision, limits and origin.
> - **Task/Ticket:** stable unique ID independent of bot, conversation, provider session or PR; immutable original objective/actor/source; scope; creation and update time; state; last checkpoint; artifact/SHA; latest decision and active assignment.
> - **Mandate:** explicit vs inferred request, user/policy authorization, version, allowed repo/project/tools/side effects, lifetime and stop/revocation. An event or model response cannot increase authority.
> - **Assignment:** attempt ID, task ID, worker bot ID, actual session ID, context/checkout identity, tool restrictions, start/end, result/evidence, timeout and retry counter.
> - **Event/Subscription:** source event identity, source thread/PR/issue, correlation to task, cursor/replay state and allowed recipients.
> - **External-action receipt:** stable idempotency identity, arguments/content digest, pending/applied/uncertain result, retries and any reconciliation evidence. Reuse existing receipts/queue; add a small transactional outbox only for proven gaps.
>
> **Lifetime/foreign-key invariant:** existing `bot_tasks`/`bot_threads` are **not** the durable SuperAgent Ticket source of truth. Preserve original objective, mandate, assignments and audit across bot reset/deletion, session deletion, channel archival and provider replacement, while making affected future work unavailable. Avoid `ON DELETE CASCADE` from ephemeral bot/session records into durable Ticket/evidence history; test orphan and revocation handling.
>
> A user task's normal path is `queued → running → (waiting_approval | waiting_review | blocked) → completed`. `failed`, `stopped`, `interrupted` and `inconclusive` must be distinct where applicable. Define allowed/forbidden transitions, terminal state invariants, state version checks and restart semantics. **A finished model turn is not a finished task.**
>
> Only evidence and **native gates** can mark completion. For Council-enabled tasks, `passed` or explicitly attributed human `accepted` is required. `limit`, `failed`, `stopped` and `inconclusive` are **not** passing verdicts.

### REQ-03 — Existing bots as isolated, bounded executors

> 1. Select coordinator/workers from **existing bot IDs**; preserve all global profile information, selected skills, harness/model, memory, schedules, channels, credentials and unrelated conversations.
> 2. Start a **separately correlated task/session** per independent work item, using native session/fresh-context mechanisms when they meet the contract. Store `task_id → assignment_id → bot_id/session_id`; do not put two unrelated tasks in one mutable live conversation.
> 3. Reuse the bot gateway and persistent queue, including existing **per-session** busy handling/queue, pause/stop, permission prompts and callbacks; add a durable **orchestration-owned per-bot admission/concurrency and resource-lease guard across separate sessions**. Existing direct/channel three-hop bounds, self-target rejection, explicit-recipient and channel-membership rules must also apply to SuperAgent dispatch and replay.
> **Dispatch/return invariant:** record a native orchestration assignment identity and origin independently of bot thread/channel naming. Ordinary bot sessions currently have per-session queues, normal per-turn replies and proactive notification targets; SuperAgent must not hijack the default desktop conversation, create an unapproved external channel, alter a bot's global notify destination or emit a task-final result through `gateway::run_finished` before its own evidence/Council gate. Maintain original bot/collaboration routes (including hop/cycle limits) across orchestrated delegation.
>
> 4. Parallel tasks with writes need independent worktrees or explicit exclusive file/write ownership. No concurrent writes to one unprotected checkout or silent last-writer-wins integration.
> 5. Each delegation packet includes scoped objective, authorized artifact refs, relevant context only, allowed/forbidden tools, expected evidence, time/attempt budget, source thread, task ID and route. Never send private unrelated transcripts, hidden reasoning or secrets.
> 6. A model may recommend a worker or repair, **not** add permissions, auto-enable skills, raise budgets, weaken reviews, rewrite goals or choose an unapproved fallback. Replace unavailable workers only through an authorized choice with equal/narrower authority.

### REQ-04 — Presets and backward-compatible configuration

> 1. Add **Bots → Orchestrations** with named orchestrations referencing existing bots. Do **not** create duplicate “SuperAgent bots.” Supported config: coordinator; worker set; optional reviewer set; project/repo scope; concurrency/retry/budget caps; sources; autonomy `Off / Observe / Auto`; optional native Council.
> 2. Extend the **existing** declarative bot-manifest import/export/backup/Git-sync implementation (#123) with a **versioned optional orchestration section or compatible companion manifest**. Make a single documented canonical schema; reuse existing manifest, backup and validation boundaries **without assuming the legacy per-bot/channel importer is transactional**; make orchestration dependency preflight and orchestration-state commits atomic, and report mixed-import partial success safely. Existing `bots` / `sharedChannels` imports without orchestration fields must work unchanged.
> 3. In exported presets use **portable bot names/identities** (local IDs vary between machines); on import resolve to canonical existing/new bot IDs. Reject duplicate names, ambiguous references, missing bots, invalid policies and unsupported harnesses. Use local bot IDs for all runtime authorization and relationships.
> 4. Import must preview changes, require explicit approval and never automatically enable an imported orchestration, run a task, install/activate skills or edit a bot's global profile. Reimporting unchanged data is idempotent, not duplicate configuration or schedules. Editing a preset must not mutate already admitted mandates.
> 5. Export **configuration only**, no live task state, conversations, saved memories, GitHub content, credentials, provider tokens, pending approvals or private data. Preserve legacy bot import/export on desktop and headless.
> 6. Provide example presets **Software Development** and **Issue Resolution** with editable bot/provider names, an optional Council and **manual-only safe defaults**.
>
> **Illustration only — proposed, not an existing supported manifest:**
>
> ```json
> {
>   "orchestrations": [{
>     "name": "Software Development",
>     "enabled": false,
>     "coordinatorBot": "Router",
>     "workerBots": ["Builder", "Debugger"],
>     "reviewerBots": ["Quality", "Security"],
>     "autonomy": "off",
>     "maxConcurrentTasks": 2,
>     "maxAttemptsPerAssignment": 2,
>     "council": {"enabled": false}
>   }]
> }
> ```
>
> The actual schema must follow existing manifest conventions; do not introduce competing Council data types.

### REQ-05 — Event intake, subscriptions, policies and proactivity

> **Sources:** manual/UI, authorized bot/channel requests and schedules first; subsequently **explicitly enabled** GitHub issue, comment, PR and CI/check events for selected repositories. Reuse current GitHub authentication/CLI/integration; choose and document a supported polling/notification mechanism after inspection, not a fictitious webhook. No new mandatory cloud service.
>
> 1. Normalize input as an immutable **Trigger**: source, event key/type, actor/provider identity, repository/project, source thread, timestamp, bounded content reference, authorization context and deduplication identity. Persist cursor/progress and handle duplicate, delayed, reordered or partially fetched events.
> 2. Deterministically filter origin/repo/actor allowlists and self-generated events before model-assisted evaluation. External text is **data, not authority**.
> 3. A **Subscription** connects one or more issue/PR/source threads to an existing task. A later relevant comment or CI update becomes **context** for that task, not permission for a new task, broader scope, push or merge. Cross-thread correlation informs context; it does not grant authority.
> 4. Three separately enforced modes, starting **Off**:
>    - **Off:** no inferred work; explicit, authorized manual tasks remain supported.
>    - **Observe:** record would-accept/ignore decisions and evidence **without** launching inferred workers, writing files, posting externally or approving anything.
>    - **Auto:** admit only inferred work matching **explicit user-managed, versioned rules** for an authorized source, actor, project/repository, task/action class and budget. Model recommendations never write or elevate rules.
> 5. Revalidate active policy before starting inferred work, resuming after policy changes, and committing consequential effects. Treat bot actions, new comments and approvals as different trust levels. Stop processing on revoked permission.
> 6. Enforce finite task/concurrency, per-bot admission, attempts, daily auto-action, context/tool-token and notification budgets. An autonomous mode may create a **draft** within a permitted workspace; it must still honor existing permissions for publishing PRs, writing comments, pushing, merging, releasing, deleting or accessing secrets. No unrestricted full-access default.

### REQ-06 — Reuse native Council and Skills with explicit dependencies

> - **Do not create a new Council** or bot-reviewed prompt imitation. [#126](https://github.com/ginmp8/pandacode-extended/issues/126) retains ownership of native reviewer seats and per-delivery channel review; integrate through its existing task/cycle/revision contract when available.
> - Council is **optional per orchestration task** and must remain independent of the worker's global bot settings. Preserve independent seats, read-only enforcement, fresh rounds, stale-artifact invalidation, minimum-score gate, explicit exceptional human acceptance and native status semantics.
> - If #126's required integration is unavailable, mark Council-capability **blocked** for that configuration; do not silently skip it, substitute a new engine or claim passed review. This dependency does **not** block the manual non-Council MVP.
> - [#129 / merged PR #130](https://github.com/ginmp8/pandacode-extended/pull/130) owns the central Skills Library, import/sync, trust and activation. SuperAgent MUST reuse that library and existing bot skill references; it MUST NOT clone, install, activate, update or change global activation as a side effect. Distinguish **library installed**, **assigned to a bot**, **eligible/provisioned in this task/harness**, **actually invoked** and **evidence verified**. The Council #126 review adapter owns isolated task-scoped delivery of a reviewer's explicitly assigned skills, including when they are not globally activated, subject to trust, dependencies and tool restrictions. Ordinary worker tasks continue to follow the existing bot gateway's own supported skill provisioning semantics. Missing mandatory evidence blocks completion; missing optional skills must be visible.
> - Skills, native Council and coordinator bot instructions cannot increase tool permissions or bypass approvals.

### REQ-07 — Persistence, recovery, reliability and headless parity

> - Persist state and intent before side effects. Reuse native queues, action receipts, provider/session identity and existing restart/reconcile mechanisms; do not duplicate stores unnecessarily.
> - Differentiate **request accepted**, **worker started**, **turn ended**, **evidence verified**, **Council approved**, **task completed** and **reply delivered**. A failure/late callback must not change a stopped or outdated task into completed.
> - Apply stable idempotency keys; identical retry returns existing effect, reused key with different arguments fails. If an external push/comment/PR outcome is uncertain, **inspect/reconcile before retry**; never blindly issue another write. No unsupported “exactly once” guarantee.
> - Resume from stored checkpoints after restart; do not rerun completed steps just because the LLM context is empty. Handle provider downtime/quota, run timeout, host sleep, lost checkout, paused/deleted bot, revoked policy, missing channel, interrupted callback and budget exhaustion with **truthful** visible states.
> - Stop/cancel must prevent new assignments, scheduled auto-input and stale callbacks from resurrecting a task. Retrying after an uncertain side effect must require reconciliation or human action.
> - Add bounded watchdog detection for silent/no-progress or repeated-work loops, with finite recovery budget; do not create a self-repair loop that runs indefinitely.
> - Support both desktop and `pandacode serve`. State-changing timers/startup/run-end work must have native headless equivalents; do not depend on a desktop JavaScript timer. Provide an explicit **authorized headless task/orchestration management boundary**, rather than reusing desktop commands that require a main Webview window or creating a new unauthenticated control endpoint. Unsupported UI-only commands must fail visibly without escalating to full access.
> - Plan idempotent additive SQLite upgrade, older-schema compatibility, backup and documented rollback. No destructive migration or silent data loss.

### REQ-08 — UI, controls and observability

> Provide **Bots → Orchestrations** as a **global view inside Bots**, not another tab in an individual bot's settings, using PandaCode design-system tokens (spacing, typography, margins, borders, icons, focus, themes and responsive layout):
>
> 1. **Orchestrations list:** name, enabled state, coordinator, workers, autonomy mode, running/blocked/task counts, last activity, loading/error/empty states.
> 2. **Create/Edit:** select existing bots by ID, inspect model/harness availability and activated skills; set task limits, project/source policies, optional native Council and autonomy mode. Show precise setup errors; don't permit unknown/duplicate/paused/unsupported choices.
> 3. **Task detail:** task ID, source/origin, mandate/policy revision, associated sessions/commits/PRs, worker assignments, queue position, last verified checkpoint, Council status, human attention, actions, error and evidence.
> 4. **Actions:** explicit approve, reject, retry, resume, pause, stop and view evidence with native state/permission validation. A button must act only within its allowed state; no success UI before backend receipt.
> 5. **Event timeline:** source events, accepted/ignored/deduplicated decisions, assignment attempts, policy changes, source-thread subscriptions and latest delivery results, without secrets or unrelated transcripts.
> 6. **Clear states:** `Observe` is not execution; `accepted by human` is not `passed`; running is not completed. Only notify the user when attention/results matter; respect existing notification settings and avoid repetitive noise.
> 7. **Accessibility:** focus and keyboard operation, status announcements, error feedback, loading indicators, accessible labels and light/dark/compact-window acceptance. Reuse existing Bot/Channel/Team UI patterns rather than copy their full settings.

## Acceptance criteria (verbatim)

The source table's columns are `ID`, `How to test` and `Must be observed`. The rows below are exact source rows, including the bold `AC-xx` labels.

| ID | How to test | Must be observed |
| --- | --- | --- |
| **AC-01 Legacy behavior** | With feature disabled run normal bot chats, direct/channel delegation, original Architect/Council, queue, skills and existing import/export regressions. | No regression, no new top-level menu, no automatic tasks or changed bot permissions. |
| **AC-02 Manual task** | Create a safe orchestration with existing Router/Builder and submit one manual request to disposable project. | One task ID, correct mandate, one correlated bot assignment/session and truthful final result. |
| **AC-03 Parallel/queued work** | Submit two unrelated tasks to same busy worker and to independent workers. | Independent contexts, bounded queue/concurrency, no interleaved transcript/state. |
| **AC-04 Bot identity** | Snapshot profiles, memories, rules, skills, schedules, channels and normal chats before/after running and removing an orchestration. | All unrelated bot state unchanged. |
| **AC-05 Coordinator tool boundary** | Set coordinator's ordinary profile to full; attempt shell/file writes and external side effects via an orchestration. | Native/provider restrictions actually deny them; unsupported enforcement blocks setup. |
| **AC-06 Route/cycle limits** | Attempt self-delegation, nonmember recipient, repeated visited bot, ambiguous target and a fourth mixed direct/channel hop. | Existing #122 safeguards still reject every invalid route and cannot be reset by callbacks. |
| **AC-07 Workspace conflict** | Run two workers writing overlapping files of same repository. | Distinct worktrees or serialized write owner; no silent lost edits. |
| **AC-08 Unavailable/paused bot** | Dispatch to busy, paused, removed or unsupported bot; attempt unauthorized fallback. | Visible queue/blocked state; no lost work or silent privilege expansion. |
| **AC-09 JSON migration/round trip** | Export bots + orchestrations, import on fresh machine/database with different local IDs, reimport and remove one referenced bot. | Resolved references, preserved settings, no duplicates, missing dependency error, no secrets/live work exported. |
| **AC-10 Skill references** | Missing, disabled, incompatible and enabled skill IDs across two workers. | No automatic installation/activation; truthful available/missing/actually-invoked reporting. |
| **AC-11 Council pass/fail** | Once #126 is integrated, run two distinct reviewer bots; induce low score, repair/new round, pass and stale artifact revision. | Existing native gate controls task; stale/failed review cannot complete it. If unavailable, blocked not passed. |
| **AC-12 Human acceptance** | Reach review limit; test with and without explicit attributed human acceptance/reason. | `accepted` separate from `passed`; limit alone is not success. |
| **AC-13 Off mode** | Enable GitHub source but leave autonomy Off; generate issue/comment/CI events. | No inferred worker starts or external effects; explicit requests still work. |
| **AC-14 Observe mode** | Feed permitted, denied, duplicate and adversarial GitHub events while in Observe. | Only would-accept/ignore decisions saved; **zero** inferred launches, writes, posts or approvals. |
| **AC-15 Auto policy** | Authorize one disposable repo/actor/action class; test matching and nonmatching event, then revoke rule. | Only in-scope work admitted, policy version recorded and revocation respected. |
| **AC-16 Dedup/reordering** | Replay identical issue/comment/CI event, then out-of-order events; simulate cursor restart and GitHub rate-limit/API error. | One logical event/action, preserved progress, explicit unresolved failures and no missing/duplicate task. |
| **AC-17 Thread subscription** | Start on issue, associate PR, restart, add relevant PR review/CI and an unrelated comment. | Same task receives only related context; no authority increase or unintended new task. |
| **AC-18 Prompt injection/privacy** | Malicious issue/comment requests secrets, policy change, auto-merge; test excluded channel history and private DM. | No unauthorized command, credential disclosure, approval, repo write or cross-channel transcript leak. |
| **AC-19 Crash/restart** | Kill process before dispatch, during work, before callback and after result; repeat desktop/serve. | Durable, truthful reconciliation and no duplicate task, worker or notification. |
| **AC-20 Uncertain remote effect** | Simulate timeout after external GitHub action succeeds but before local receipt; retry same/different payload key. | Reconcile or block; no blind duplicate action; mismatched payload rejected. |
| **AC-21 Stop/late callback** | Stop or cancel during worker run; deliver delayed completion and retry. | No resurrection, late success or post-stop new effects without explicit authorization. |
| **AC-22 Watchdog/budgets** | Induce no-output, repeated identical handoffs, quota/provider failure and exhausted retry/task budget. | Finite recovery, actionable blocked/failed status and retained checkpoint. |
| **AC-23 Headless parity** | Execute identical manual task, approval, queue, interruption and restart in desktop and `pandacode serve`. | Equivalent state and policies; unsupported UI-only operations fail safely. |
| **AC-24 Accessibility/UI** | Keyboard, screen reader, light/dark and compact window; loading, empty, error, running and approval states. | Correct focus, readable hierarchy, explicit feedback and no unintended submission. |
| **AC-25 Database/backward compatibility** | Upgrade copy of existing app-data, import old bot JSON, test repeated upgrade and documented backup rollback. | No data loss, schema idempotent, old config still imports, rollback path demonstrated/documented. |
| **AC-26 Council-independent MVP** | With Council disabled, finish a manual SuperAgent task; separately enable a Council-required policy while the #126 adapter is absent/unavailable. | Manual path remains functional, no implicit reviewer; required unavailable Council yields explicit blocked/waiting status with no passed/completed flag. |
| **AC-27 Correct task→Council correlation** | Create two concurrent tasks using the same reviewer bots; inject duplicate, reordered, or another task's review result/callback. | Each outcome is applied only to the matching task/assignment/Council cycle/artifact revision; exactly one valid state transition and no cross-task evidence or final reply. |
| **AC-28 Revision invalidation** | Start a review, then correct the reviewed artifact, change its SHA/digest and replay an earlier passing outcome. | Old result stays in audit history but is stale; a new authorized round/review is required; no premature task completion. |
| **AC-29 Non-channel and channel routes** | Run a Council-enabled ordinary SuperAgent task with no Shared Channel; then run a task explicitly originated from an opted-in Shared Channel delivery. | Both use native Council; non-channel path creates no synthetic channel; channel path enforces #126 consent/membership/history and one final correlated reply. |
| **AC-30 Reviewer skills and tool authority** | Choose reviewers with eligible bot-assigned skills including a library-installed but not globally activated skill, then test an untrusted/missing mandatory skill and attempted reviewer write/MCP escalation. | Resolution/skill pinning belongs to #126; no global activation, silent truncation or implicit new tool permission; unavailable mandatory evidence blocks and actual read-only enforcement is verified. |
| **AC-31 Council lifecycle/failover/recovery** | Trigger review limit, failing score, explicit human acceptance, reviewer/provider failure, queue drain, restart, stop and delayed callback; replay an uncertain external effect. | Native Council statuses remain distinct; only genuine `passed` or authenticated attributed `accepted` with required checks allow progress; stopped/stale tasks never resurrect and external writes are not duplicated. |

## Branch-alignment acceptance expansions (same AC IDs)

The source labels these expansions "same AC IDs". They refine the rows above and create no new ID.

> - **AC-03 / AC-07:** Admit two separate SuperAgent sessions belonging to the same bot under a configured per-bot limit and overlapping checkout resources. Assert the native cross-session gate enqueues/serializes or safely leases them, and release/restart cannot double-admit. Existing same-session message queue must still behave identically.
> - **AC-04 / AC-19 / AC-25:** Reset or delete a selected bot, delete its old conversation/session, archive a source channel and restart. Immutable Ticket/mandate/assignment/history and idempotency receipts remain queryable; further unauthorized assignments are blocked. No cascading audit loss or orphaned active lease.
> - **AC-09:** Reconcile a legacy mixed import where one bot fails after another succeeds, with an orchestration referring to the failed bot. Existing successful legacy entries remain as documented, but no incomplete/unapproved orchestration is committed or enabled. Exercise duplicate, missing, renamed and ambiguous bot references; preserve manifest size and schema rules. Define/test orchestration-only import separately if supported.
> - **AC-19 / AC-23 / AC-29:** Prove that `gateway::run_finished`, queue drain, scheduler, external channel reply and notification target cannot publish a SuperAgent final result before the matching native task evidence/Council gate. Assert exactly one logical final reply after approval, and ordinary bot communications remain unchanged.
> - **AC-23 / AC-25:** Exercise authorized headless create/list/admit/approve/stop/recover and rejected unauthorized requests without a desktop Webview. Check durable state and identical native policy in desktop/headless; do not treat a UI-only mocked command as headless proof.
> - **AC-11 / AC-27 / AC-31:** Before declaring Council-enabled S3/S6 complete, cite an executed task-scoped native adapter/cycle/verdict correlation and the relevant outstanding #126 S7 evidence. Source presence or deferred interactive testing is not an acceptance pass; report unavailable real-provider coverage as `blocked`/`not_run` **for the evidence item, not as a reason to pause S3–S6 development**. Native contract implementation plus applicable automated tests, rather than user-operated #126 S7 tests, controls progression.

## How #148 uses this baseline

[Issue #148](https://github.com/ginmp8/pandacode-extended/issues/148) extends this baseline; it does not replace, renumber or rewrite the #131 criteria above.

- **S0–S6** are the implementation, source-map and exact-SHA evidence stages. Stage completion can add obligations, but a stage is not an acceptance-criterion pass; each criterion still passes or fails only against the text above plus the source's declared branch-alignment expansions.
- **S2-UX UX-01–24** is the normative UX addendum under the existing UI and accessibility requirements. It can make the observable UI stricter or more specific; it cannot turn an unmet #131 criterion into a pass.
- **EGM-01–07** ([addendum](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817527256)), **DD-01–14** ([addendum](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817744113)), **SA-01–09** ([addendum](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817864073)) and **NS-01–12** ([addendum](https://github.com/ginmp8/pandacode-extended/issues/148#issuecomment-6817979555)) are the design-review, Demand/lifecycle, Team/routing/cost and nested-subagent addenda. They are additive constraints, not replacements. If an addendum appears to conflict with the #131 text, the #131 text in this appendix controls the acceptance meaning and the conflict must be reconciled explicitly rather than resolved by paraphrase.
- **Evidence discipline:** a stage completion, a design-review disposition or a worker self-report is not acceptance evidence. The #131 evidence record still applies to each criterion: test ID, exact SHA, OS/runtime, harness/provider, test data, procedure, expected/actual result, evidence link and truthful `passed`/`failed`/`blocked`/`not_run` status. Optional manual/user-deferred checks remain `not_run`, never `passed`.
- **Packet link:** link this appendix from §9 "Acceptance map" of [`extended-superagent-design-review.md`](extended-superagent-design-review.md) at the bullet that names the #131 normative baseline, so the packet carries the text rather than only the IDs.

## Verification record for this appendix

- Source fetch: `gh issue view 131 --repo ginmp8/pandacode-extended --json number,title,url,body,comments` at `2026-11-14T19:47:27Z`; issue body `60,593` characters / 369 source lines; issue `updated_at` `2026-11-11T06:09:40Z`.
- Comment check: GraphQL `comments(first: 100) { totalCount }` returned `0`; REST `GET /repos/ginmp8/pandacode-extended/issues/131/comments --paginate` returned no comments.
- Text comparison: a local PowerShell check re-read the same body and confirmed 8/8 REQ bodies, 31/31 AC rows, 6/6 branch-alignment expansion bullets and the complete alignment subsection are byte-identical to the source after removing the added `> ` quote prefix. Each `REQ-01`–`REQ-08` heading and each `AC-01`–`AC-31` row appears exactly once as a canonical entry.
- Markdown hygiene: `git diff --check` passed (exit 0), including an intent-to-add check of this new file; the file has no CR characters, trailing whitespace, remaining source placeholders or `MISSING FROM SOURCE` rows. `git status --porcelain` showed only `?? docs/extended-superagent-131-baseline.md`.
- Not run: no Rust, Node, npm or Playwright check was run; this is a documentation-only addition.
