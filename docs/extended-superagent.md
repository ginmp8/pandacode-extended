# Extended SuperAgent orchestration

> **Implementation status:** Not implemented at this checkpoint. This document
> establishes the source-of-truth scope for development in
> `feat/issue-131-superagent`; it is not acceptance evidence.

**Authoritative specification:** [Issue #131](https://github.com/ginmp8/pandacode-extended/issues/131)

**Integration target:** `extended/development`  
**Working branch:** `feat/issue-131-superagent`

## Purpose and ownership

Implement native, durable SuperAgent orchestrations using **existing bots** and
the application's current gateway, queue, storage, sessions, permissions,
Architect/Council, Skills, GitHub integration, and headless runtime. Do not create
another bot registry or independently reimplement Council authorization.

The UI belongs under **Bots → Orchestrations**. Existing direct chats, shared
channels, task execution, permissions, and import/export must remain
backward-compatible. Default autonomy is **Off**; proactive actions require
explicit, versioned, task-scoped authority and native enforcement.

## Delivery sequence

| Stage | Code and evidence required |
| --- | --- |
| S0 — Source baseline | Pin current branch and trace bot dispatch, callbacks, shared queue, sessions, permissions, storage, Architect/Council, manifests, headless mode, and GitHub. Document source references and baseline checks. |
| S1 — Manual native MVP | Implement durable Task, Mandate and Assignment identity, manual admission, correlated existing-bot execution, bounded authorization, stop and terminal settlement. Council remains Off by default. |
| S2 — Presets and UI | Add Bots → Orchestrations, preset CRUD, backups/import/export, status/progress and attention controls with regression coverage. |
| S3 — Council | Reuse the actual native Council cycle/revision/approval contract without making Shared Channels compulsory. Deny stale, unavailable, failed or unauthorized review. |
| S4 — Sources and policies | Add policy-driven GitHub/event/schedule subscriptions with Off/Observe/Auto, filtering, cursor deduplication and bounded authorizations. |
| S5 — Recovery/headless | Prove restart, idempotent external effects, concurrency, cancellation, budget/watchdog and desktop/serve consistency. |
| S6 — Verification and docs | Trace AC-01–AC-31 to implementation, executable tests, commit SHA, CI evidence, and honest gaps. Preserve optional manual cases as deferred. |

## Autonomous implementation gates

1. Before each stage, re-read issue #131 and the current branch HEAD. Use
   repository evidence to freeze affected contracts and build focused automated
   tests. Make only necessary changes on `feat/issue-131-superagent`.
2. Implement functional native behavior, then validate Rust, TypeScript and
   Playwright paths as applicable. Inspect required CI checks on the **exact**
   commit and repair failures without weakening or skipping tests.
3. Preserve one active stage and one Git writer. Continue immediately to the
   next verified stage when possible; otherwise retain code and precise status
   for the next scheduled invocation. Preserve a linear single-parent history.
4. Do **not** require owner-operated manual tests, Desktop access or provider
   credentials. Record unexecuted authenticated/live cases as
   `not_run — user-deferred`. This deferral never waives native security,
   permission, data-integrity or runnable automated requirements.
5. For S6, publish an AC-01–AC-31 ledger with source path, test ID,
   execution command, exact SHA, CI link, result and residual risk. Do not
   label unverified functionality complete.
6. Keep the pull request in **draft** until implementation and applicable
   automated acceptance are complete. Never automatically merge, release,
   approve or close the parent issue.

## Verification ledger

| Checkpoint | Status | Evidence |
| --- | --- | --- |
| Issue #131 specification and S0–S6 sequence | Recorded | GitHub issue #131 |
| S0 source map | Pending implementation | — |
| S1–S5 production functionality | Not started | — |
| S6 AC-01–AC-31 automated evidence | Not started | — |
| Authenticated/manual provider scenarios | Not run — user-deferred | Not a code-stage gate |

**Note:** This file is the initial draft-PR scope declaration, not a substitute
for the feature implementation, tests, or later source-backed documentation.
