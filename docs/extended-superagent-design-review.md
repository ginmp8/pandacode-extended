# SuperAgent #148 — frozen design and review packet

**State:** S0 design review draft. Four independent reviews are required before broad implementation. A narrow S1 write-grant slice was already in flight; this packet neither approves that code nor grants a Ticket.

**Repository:** ginmp8/pandacode-extended

**Source baseline:** f9efb3a1c7558dcf7d5688bb1687cfff9253f967, the limited #146 MVP SHA identified by #148. The worktree contains uncommitted implementation candidates; S6 must validate the final committed SHA.

**Requested push branch:** feat/issue-148-pe

## 1. Objective and boundaries

Implement the complete #148 SuperAgent contract: a reusable native Team executes an authorized Demand through bounded planning, dependency-aware WorkItems, multiple role-based Runs, tests/fixes, optional native review, recovery and delivery. The product uses existing bots, provider/harness support, Skills, gateway, native Council, schedules and GitHub integration.

No second bot registry, alternate reviewer scorer, hidden network, telemetry, credential export, automatic merge/release, or unrestricted model authority. Default source mode and Team autonomy are Off. A plan, design review, worker result, child result or unverified Council observation never grants authority or finalizes a Ticket.

### Goals

- One durable Demand lifecycle shared by manually created work and explicitly authorized GitHub/schedule sources.
- Reusable, versioned Teams with one Coordinator, one or more Workers and optional Reviewers; per-role existing bot, harness/model, Skills, capability, budget and fallback snapshots.
- Dependency-aware work graph and scheduler with safe multi-demand concurrency, project/resource leases and isolated worktrees where parallel writes are allowed.
- Exact, native scoped writes and side effects with durable grants, stop/revoke checks, reconciliation and honest uncertain states.
- Persistent Teams, Demands, Runs and Review workspace with truthful graph/list, effective policy, evidence and usage.
- Desktop and serve share lifecycle, admission, authorization, recovery and effect semantics.
- Worker-created nested Subagents only when the selected harness reports an enforceable child lifecycle and Team policy permits it.

### Non-goals

- Creating or copying global bot profiles as part of a Team.
- Converting a bot permission mode or prompt into native authority.
- Treating Teams like Shared Channels or changing ordinary bot conversations.
- Allowing Council or a child to mint a Ticket grant, change Team policy, self-review, accept a Ticket or issue another external effect.
- Reporting estimated or missing usage as exact spend.
- Automatic network activity for an Off source; any new automatic request must be added to the README privacy table and website privacy page before enabling it.

## 2. Glossary and identity model

| Term | Meaning |
| --- | --- |
| Team / SuperAgent | Reusable, versioned policy and ordered role bindings. It is configuration, not a running task. |
| Demand / Ticket | Durable user or authorized-source request, with immutable objective/source/actor and a pinned Team/policy snapshot. |
| WorkItem | One node in a Ticket's dependency DAG. It has an objective, evidence contract, dependencies and state. |
| Run / Assignment | One role-bound attempt for one WorkItem, with exact bot/session/artifact/resource/budget identity. Retries create new attempts. |
| Mandate | Immutable native authority snapshot for the Ticket. It is distinct from an exact-path write grant and each external-effect grant. |
| Reviewer | A configured role admitted through native reviewer security/Council. A prompt or model-generated review is not an authoritative verdict. |
| Subagent | Provider-created descendant of a Worker Run. Its ancestry, depth, count, concurrency and budget are native records; its result is evidence for its parent only. |
| Source | Manual, authenticated GitHub or scheduled origin bound to one Team, allowlist, mode, cursor and immutable admission mandate. |

Ticket, WorkItem, Run, Team version, source delivery, Council cycle/round/artifact and effect receipt have separate stable IDs. Bot and session IDs are references plus snapshots, never Ticket ownership.

## 3. Roles and policy

- Coordinator: exactly one existing bot. Produces a bounded plan and WorkItem DAG. Cannot grant tools, select unconfigured bots, widen mandate or declare completion.
- Workers: one or more configured existing bots. Assignments take only ready WorkItems, obey native tool gates and exact resource leases, and return evidence.
- Reviewers: zero or more configured roles. Admission uses existing reviewer-security/Council bindings. A distinct fresh review runs only after the latest artifact is frozen. A Worker, Coordinator, descendant or design reviewer cannot review its own work into approval.
- Subagents: optional children of a Worker Run. Default depth is zero. A Team may set a finite depth/count/concurrency/time/token budget; maximum supported depth is two (worker → child → grandchild). Unsupported providers show unavailable and cannot simulate a child.
- For each role, snapshot bot ID, effective harness/provider/model, Skills/MCP/capabilities, fallback order, runtime switches, budget and source version. Existing bot configuration remains the single source of truth; changing a profile does not rewrite an admitted Run's snapshot.
- Economy/Performance selects bounded routing/fallback strategy. Usage is attached to the actual Run and categorized as actual, estimated, unpriced or unknown.

## 4. Lifecycle and safety invariants

Admission binds source actor, Team version, objective, project, roles, policy, tool capabilities, review policy, budget, dependencies and expected side effects. Native code validates the snapshot and reserves leases before launch.

Ticket flow: queued → planning → ready → running → waiting_review or waiting_approval → completed; terminal alternatives include blocked, failed, stopped, interrupted and inconclusive. WorkItems and Runs have their own states. UI labels come from durable state, never a guessed percentage.

All mutations use expected revisions and idempotency keys. Stop/revoke is durable before providers are stopped; late callbacks cannot write or settle. Restart either resumes from a verified checkpoint with the same snapshot or marks work interrupted. Unknown external outcome remains uncertain until authoritative reconciliation. Auto is withheld until these gates pass.

Parallel writes require separate worktrees or non-overlapping exact path grants; a same-checkout UI concurrency setting is insufficient. Native policy denies unsupported OS/harness combinations. Existing S1 candidate leaves Unix worker writes disabled while ancestor-pinning remains unproven.

## 5. Source map and reuse decisions

| Existing seam | Reuse / required extension |
| --- | --- |
| src-tauri/src/bots.rs and bots/gateway.rs | Existing bot identity, thread session, Skills and queued delivery. Add role/run correlation and cross-session/resource leases. |
| src-tauri/src/superagent.rs and storage.rs | Native Ticket/mandate/assignment/audit and additive migrations. Add Team-version snapshots, WorkItems, scheduler/lineage and run history. |
| src-tauri/src/providers.rs and providers/host_tools.rs | Harness execution and native ToolHost. Built-in shell/file tools remain read-only for scoped workers; only exact app-mediated grants write. |
| src-tauri/src/workspace.rs and superagent/write_grants.rs | Exact path validation and atomic replacement. Windows candidate is tested; Unix writes fail closed. Keep unsupported targets disabled until a secure primitive is proven. |
| src-tauri/src/architect/reviewer_security.rs, architect/council.rs and superagent/council_adapter.rs | Native reviewer binding, freshness, rounds and verdict. Add exact Ticket/run/artifact/policy correlation and one replay-safe terminal transition. |
| src-tauri/src/superagent/events.rs, github.rs and schedules.rs | Source policy and local authenticated gh polling. Add trusted delivery receipt/cursor, bounded overlap and source-to-Team route. Off performs no calls; source text is untrusted. |
| src-tauri/src/superagent/effects.rs and headless.rs | Exact grant and receipt foundation. Add durable claim/settlement/read-back/recovery and a shared desktop/serve semantic path; never blind replay. |
| src-tauri/src/superagent/manifest.rs | Preserve v1 as disabled single-worker mapping; add v2 role bindings and dependency preflight. Import no runtime authority or secrets. |
| src/main.ts, src/superagent-ui.ts, src/superagent.css, src/preview-superagent.ts | Keep existing persistent in-shell Orchestrations route; add Teams/Demands/Runs/Review workspace and real preview commands. |
| src-tauri/src/usage_ledger.rs and task_usage.rs | Reuse local usage evidence. Report unknown where a harness lacks reliable attribution. |
| Provider/harness child-agent adapters | Add native lineage and finite policy only for observable and controllable child lifecycle; otherwise reject delegation. |

Detailed current source map: [extended-superagent-source-map.md](extended-superagent-source-map.md).

## 6. Transport decision for S4

Use the user's authenticated gh CLI as the only GitHub credential boundary. Poll only explicitly enabled configured sources, on a bounded schedule, using fixed repository/event allowlists and a durable resource-ID/update-time cursor with overlap and deduplication. Do not accept caller-supplied event envelopes as authenticated. Do not build an app-owned webhook receiver or persist tokens. Off performs no network calls; Observe records metadata and never launches; Auto may create/update a Demand only after S1 admission and S5 recovery/effect gates pass. Issue/comment text is hostile input, never policy or instructions.

Before enabling polling, disclose endpoint, permissions, cadence, payload and local retention in README and the website privacy page. If GitHub CLI cannot provide authoritative cursor/read-back for an event type, keep that event type unavailable instead of inventing a cursor.

## 7. Migration and rollback

Use additive forward migrations with foreign keys and replay tests. Preserve current single Coordinator/Worker configurations as a v1 Team version. New Team edits create a new version; admitted Tickets keep their original role and policy snapshots.

Manifest v2 import is previewed, dependency-checked, transactional and disabled by default. It imports no Ticket, Run, session, approval, grant, cursor secret or effect receipt.

Rollback first disables Auto/admission, durably revokes grants and stops sessions, then leaves audit and effect history intact. Do not delete or overwrite Ticket history. Configuration-only restore requires a new preview and confirmation and cannot restore runtime authority.

## 8. User journeys

1. Manual demand: user chooses an existing Team and objective; native preflight shows roles, effective model/harness/Skills, project, budget, missing capabilities and write/effect gates. User confirms a bounded mandate. Coordinator proposes a graph; native validates it; scheduler starts ready Workers under leases. Real test/fix evidence and optional fresh Council review appear in the Run inspector. User stops, retries or accepts only with a current revision and required review.
2. GitHub source: user configures exact repo/actors/events and Team, then selects Off, Observe or explicitly authorized Auto. Polling/dedup is native and durable. Off creates no network request; Observe never launches. Auto creates or updates one idempotent Demand with immutable source/team/mandate snapshots. Untrusted body text cannot alter tools or policy.
3. Recovery: crash before an effect leaves a recoverable claim; crash after possible remote commit requires read-back. A unique verified remote match settles applied; conflict or unavailable read-back stays uncertain. Restart and serve expose the same state and controls. Stop revokes before provider shutdown; late results remain denied.
4. Nested delegation: an eligible Worker requests child work within finite depth/count/concurrency/time/token limits. Native lineage ties child to parent Run; stopping/revoking parent stops descendants. Child output can satisfy a WorkItem evidence contract but cannot approve or finalize the Ticket.

## 9. Acceptance map

- #131 REQ-01–08 and AC-01–31 remain normative baseline.
- #148 stages S0–S6 plus S2-UX UX-01–24 are normative; UX-01 modal removal is already present in this checkout and must not regress.
- EGM-01–07 governs the standalone packet, isolated review, findings, non-authoritative design review and exact-SHA drift.
- DD-01–14 governs unified authorized intake, autonomous one-shot lifecycle, scheduler, recovery, workspace views, immutable policy and concurrency.
- SA-01–09 governs reusable Teams, role-specific harness/Skills, routing mode, budgets and truthful cost.
- NS-01–12 governs worker-created lineage, finite nested policy, native stop/restart and child authority denial.
- Dependency issues #123, #126 and #129 remain separate; reuse their existing manifest, reviewer-security and Skills contracts rather than widening their ownership.
- S6 must map each ID to source, named executable test, command, exact SHA, CI run, screenshot where applicable, result and residual risk.

## 10. Independent review protocol (EGM-01–07)

Four read-only reviewers receive this frozen packet and only the verified reading list below, each in a new isolated worker context:

1. Comprehension: find ambiguous terms, missing normative IDs and user journeys that cannot be implemented from this packet.
2. Simplicity/reuse: challenge duplicate stores, APIs, roles or mechanisms; require existing gateway/Council/Skills/usage components to be reused.
3. Design risks: test security boundaries, races, migration/rollback, source trust, revocation, budgets, event injection and false UI states.
4. Implementation readiness: sequence dependencies, identify file/test owners, and flag choices that still block bounded implementation.

Verified reading list: this packet; docs/extended-superagent-source-map.md; docs/extended-superagent.md; GitHub #148 and #131; comments 6817527256, 6817744113, 6817864073 and 6817979555. Reviewers must list what they actually read and separately identify injected repository/project instructions, by filename/source. Do not quote hidden host instructions. Record findings as requirement ID, severity (blocker/major/minor/info), source, evidence, dedup key, recommendation and accepted/rejected/deferred disposition with reason. A timeout, refusal or missing review is not_run, never “no findings.”

Design review does not issue a Ticket grant or final approval, cannot self-review implementation, and is not a gate for unrelated safe work. Freeze S0 only after four dispositions are recorded. Broad S1–S5 implementation uses one active writer per overlapping scope and never weakens a gate to make a test pass.

## Review register

| Lens | Worker | Context/read-list disclosure | Findings and disposition | State |
| --- | --- | --- | --- | --- |
| Comprehension | Not dispatched | No reviewer context or read performed | No findings claimed; review unavailable | not_run — Router and dispatch returned “Team is no longer active” |
| Simplicity and reuse | Not dispatched | No reviewer context or read performed | No findings claimed; review unavailable | not_run — Router and dispatch returned “Team is no longer active” |
| Design risks | Not dispatched | No reviewer context or read performed | No findings claimed; review unavailable | not_run — Router and dispatch returned “Team is no longer active” |
| Implementation readiness | Not dispatched | No reviewer context or read performed | No findings claimed; review unavailable | not_run — Router and dispatch returned “Team is no longer active” |
