# Extended SuperAgent orchestration

> **Implementation status:** Partial native implementation. S0 source mapping,
> S1 manual read-only Ticket/Mandate/Assignment dispatch and audit, and part of
> S2 presets/UI exist. S1 task-scoped writable authority, full S2 integration,
> and S3–S6 remain incomplete. No overall acceptance is claimed.
> All development remains on `feat/issue-131-superagent`.

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

## Source baseline and dependency map

The implementation is anchored to the [S0 source audit](extended-superagent-source-map.md).
That audit pins the exact branch SHA, entrypoints, native authorization and
callback paths, durable-state gaps, test owners and integration gates. It
intentionally distinguishes **source inspection** from **executed acceptance**.

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
| S0 source map | Source-authored; baseline CI tracked separately | `docs/extended-superagent-source-map.md` |
| S1 native read-only manual task | Partially implemented; exact-HEAD CI required | `src-tauri/src/superagent.rs`; native `superagent_snapshot` |
| S2 presets and task inspector | Partially implemented; browser tests cannot prove real provider execution | `src/superagent-ui.ts`; `tests/superagent.spec.ts` |
| S3–S5 full integration | Incomplete | See issue #131 |
| S6 AC-01–AC-31 automated evidence | Not yet complete | See issue #131 |
| Authenticated/manual provider scenarios | Not run — user-deferred | Not a code-stage gate |

**Note:** This file is the initial draft-PR scope declaration, not a substitute
for the feature implementation, tests, or later source-backed documentation.


### Native companion-manifest recovery snapshots

A confirmed desktop companion-manifest import preflights every referenced bot.
If it will create presets, the native importer first writes and verifies a
configuration-only snapshot to the existing private `bot-backups` directory
as `orchestrations-*.json`, reusing the legacy backup's exclusive atomic
writer. The command returns `backupPath` for the verified snapshot.
An unchanged reimport does not create duplicate presets or backup files.
Backup creation/verification failure causes no orchestration mutation.

Snapshots contain no Ticket, Mandate, session, credential, memory, or approval
state. Restoration requires a new explicit preview and confirmation, with
imported presets disabled by default. Legacy bot manifest and Git sync remain
separate: importing bots or shared channels never implicitly enables
orchestrations.

### Safe editable examples

The creation form provides **Software Development** and **Issue Resolution** name templates.
These are configuration-only suggestions: they neither choose existing bots nor
create profiles, enable a policy, grant tools or start a task. Both bot IDs
must be chosen explicitly, and the created preset is disabled.

### Explicit GitHub thread references (partial S4)

The trusted local Bots → Orchestrations Ticket detail can now link an exact `issue:NUMBER` or `pull:NUMBER` to a configured Off/Observe GitHub source from the same orchestration. Native SQLite v42 stores at most 16 source-thread references per Ticket, rejects foreign tasks and conflicting owners, preserves links across restart, and does not modify task authorization or launch a provider. This is **manual context metadata only**, not authenticated GitHub event delivery, subscription notifications, Auto policy, or authorization for write tools. Those integration steps remain pending.

## Checkout identity and native admission (S1)

SQLite v47 refreshes the native SuperAgent assignment lease triggers. Local
projects with the **same stored checkout path** share an exclusive running
assignment lease even when project IDs and selected bots differ. Manual
preflight, ordinary-session conflict checks and provider admission use
matching exact-path protection. A rejected admission creates no tool grant.

New Demands pin each configured Team member's project checkout path in the
immutable admission profile. Reassigning that path under the same project ID
denies provider admission and prevents late callbacks from becoming approvable
evidence. Existing read-only task profiles without this field remain readable.

This protection compares stored checkout path strings. It does **not** claim
equivalence for symlink targets, relative aliases or case-folded paths.
Writable tools and remote effects still require separate task-scoped native
tool/provider grants; this change does not enable them.
