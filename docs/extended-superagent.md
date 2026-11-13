# Extended SuperAgent orchestration

> **Implementation status:** Incomplete. The persistent Orchestrations workspace, manual read-only Ticket flow, scoped-write candidate, exact-effect candidate and source-policy groundwork exist in the worktree. S1 writes pass focused Windows checks but are disabled on Unix; one SuperAgent effects restart test fails. Full multi-role Teams, task-bound Council, authenticated intake/Auto, durable recovery/headless parity, nested subagents and S6 exact-SHA evidence remain outstanding. No overall acceptance is claimed.
> **Requested push branch:** feat/issue-148-pe. No commit or push has been made.

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
| S4 — Sources/Auto | Authenticated local-gh intake, Off/Observe/Auto, allowlists, cursor/dedup, source-to-Team routing, revocation and privacy disclosure. Auto stays unavailable until S1/S5 gates pass. |
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
| S1 scoped writes | Partial: focused Windows checks pass; Unix disabled; one unrelated effects test fails |
| S2 UX-P1 persistent workspace | Implemented and browser-verified; preserve non-modal route |
| S2 multi-role Teams/work graph | Incomplete |
| S3 task-bound Council | Incomplete; adapter is advisory |
| S4 authenticated intake/Auto | Incomplete and fail-closed |
| S5 recovery/headless/subagents | Incomplete; Rust validation outstanding |
| S6 exact-SHA evidence | Not started |
| Live GitHub smoke | not_run until a disposable authorized repository is available |

## Existing safe import and thread behavior

Companion-manifest import preflights every referenced bot, writes a verified
configuration-only snapshot before mutation, and imports presets disabled.
Snapshots exclude Tickets, Mandates, sessions, credentials, memory and approvals.
Legacy bot manifest and Git sync remain separate; importing bots or channels
never enables an orchestration. Restore requires a new preview and confirmation.

Ticket templates only fill names; both existing bot IDs must be selected
explicitly. They do not create profiles, enable a policy, grant tools or start work.

The trusted local Ticket detail can link an exact issue:NUMBER or pull:NUMBER to
a same-orchestration Off/Observe source. SQLite v42 stores up to 16 references,
rejects foreign tasks/conflicting owners and preserves links across restart.
This is manual context metadata, not authenticated delivery, Auto or write authority.
