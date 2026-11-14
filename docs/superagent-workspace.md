# SuperAgent Teams and Demands workspace (issue #148, partial)

The existing **Bots → Orchestrations** workspace offers native-backed, persistent views
for reusable Teams and their Demands/Runs. One configured Team is one SuperAgent;
each independent Demand is a durable Ticket bound to exactly one Team.

## Native-backed surfaces

- **Teams:** saved policy revisions, coordinator/worker bot identity and native task counters.
  The directory shows native correlated member counts (one Coordinator,
  configured Workers and Reviewers) without inflating Ticket totals when a
  Team has multiple roles. Search and Off/Manual status filters operate only
  on the native-loaded Team directory and do not change the selected Team,
  stored policies, permissions or execution. Older fixtures without counts
  are marked unavailable rather than inferred.
- **Demands/Runs:** a read-only native Ticket index with stable 200-row
  cursor pages and an explicit **All Teams / specific Team** filter. The Team
  filter is enforced by the native SQL query **before pagination**, so sparse
  Teams are not omitted merely because other Teams have newer Tickets.
  Changing Team resets the pagination cursor and loaded Board/List rows;
  refresh queries current native state. The board buckets Working, Awaiting
  decision, Needs attention and Finished or stopped. All column counts represent
  **only loaded pages for the selected Team scope**, not complete totals.
  Unknown or malformed Team IDs fail closed instead of showing false empty results.
- **Review inbox:** a separate native, bounded cross-Team query shows actual
  `waiting_approval` (human decision) and `waiting_review` (native Council
  pending, **not** a verified pass) Tickets, even when they fall outside the
  general Demand index's most recent 200 rows. Open Demand resolves the
  original Team before offering native task controls.
- **Open demand:** resolves the original Team, fetches the exact Ticket
  when outside the Team's latest 50, verifies its Team ownership, then offers
  the established native Ticket controls.
- **Native Assignment inspector:** the Ticket snapshot includes transactional
  native Assignment insights: current linkage, recorded result presence, revocation,
  bounded attempt budget and timestamps. No provider result content is displayed.
  A persisted running assignment is **not proof of live provider execution**.

## Non-authority and recovery

Read-only views never issue Mandates, spawn sessions, infer task approval,
start GitHub Auto, or authorize tools. Source Observe events do not create
Demands. Explicit Ticket actions may invoke the shared native admission,
scoped-write grant or authorized issue-comment dispatcher, but inspection and
model output alone do not authorize either operation. Coordinator and Council
remain read-only; Worker writes require an exact native grant and a supported
provider/platform (currently Windows only). Native issue-comment effects
require an exact, idempotent, versioned grant with uncertain-state read-back.
Multi-worker autonomous scheduling, GitHub Auto and nested subagents remain
unsupported.
Ticket selection is discarded on Team change and when a native reload finds
the Assignment missing. Board uses ordinary keyboard-focusable native buttons
and the same theme tokens for desktop, narrow, light and dark views.

## Executable evidence and limitations

- Rust: native_ticket_snapshot_inspects_attempt_budget_and_revocation_without_grants.
- Playwright: native-backed attempt inspector shows recorded sessions and run
  budgets without granting effects.
- Playwright: cross-Team Demands index checks Board/List presentation while
  opening only the immutable Ticket's original Team.
- Rust: native_team_filtered_demand_pages_are_complete_isolated_and_restart_safe
  checks 205-Ticket Team pagination, other-Team isolation, invalid filters and restart reads.
- Playwright: native Team filter pages independently and resets pagination when changing Teams;
  its calls retain the exact selected native Team and page cursor without admitting work.

These tests must pass at the exact current SHA. This partial feature does not
close #131 AC-01..31 or #148 UX/SA/DD/NS/EGM acceptance. The unified branch
adds native task-bound Council and guarded Worker write paths, but needs
independent exact-SHA acceptance and platform validation. Source Auto, multi-
worker autonomous scheduling, complete headless parity and nested subagents
remain out of scope for this integration checkpoint. See
[the unified integration contract](superagent-unified-integration.md).


### GitHub Issue lifecycle observation

Sources can explicitly observe `issues.closed` and `issues.reopened` from
GitHub's REST Events `IssuesEvent`, alongside existing issue-open, comment
and PR/review/Check Run classes. The configured repository and exact actor,
signed-in operator identity and native Observe revision are verified before a
decision receipt is stored. Closing or reopening an issue changes no Ticket
state, starts no Demand and grants no Worker privilege. Event body content is
not persisted or interpreted as a task instruction. Replays are idempotent
across application restart. This extends observation only; Auto remains
unavailable.

### Authenticated Observe: self-event loops and bounded pagination

The operator-issued **Poll authenticated GitHub** command resolves the current
account with native `gh api user`, then reads up to three chronological
`repos/{owner}/{repo}/events` pages (100 events each). It validates ordered
numeric event IDs and a persisted-cursor or source-activation boundary before
committing any decision. An incomplete or changing feed, failed request or
invalid identity is an error: the previous cursor is not silently advanced.
A source can still miss events that GitHub no longer retains, so this is **not**
a guaranteed subscription or permission to enable Auto.

An event authored by the authenticated operator is durably recorded as
`ignored_filter` even when its configured actor matches the source; such
self-authored events are never `observed_permitted`. The UI reports new
self-authored denials separately. Only bounded metadata is stored; issue and
comment bodies are not included in decision receipts. Off performs no network
polling, and linked source events remain context only, not task grants or tool
instructions. This intentionally reuses the existing policy-denial receipt
schema without a destructive migration.


### Native execution graph (bounded ownership view)

In the Demand inspector, **Inspect graph** reads `superagent_graph`
from the same native Ticket/Assignment storage and displays a revision-bound
Ticket root, its latest 50 real assignment attempts and any explicitly
pending human or Council decision state. Edges mean **Ticket owns attempt**
or **Ticket awaits decision**, not proof of agent-to-agent delegation or
Council approval. The response omits objectives, authorization tokens,
worker results and private transcripts. The graph is loaded on demand,
fails closed when the Ticket revision has changed, and creates no work or
grants. Keyboard users and screen readers receive a semantic ordered
list; a non-SVG layout keeps narrow and high-contrast modes legible.


## Effective Team policy and Skills inspection (partial S2 / AC-10)

From **Configure Team roles**, load the existing native roster and select **Inspect
effective policy and Skills**. The main-window-only `superagent_effective_team`
command reads the Team's policy revision and actual existing bot profiles
(Coordinator, ordered Workers and optional Reviewers), including the bot's
configured harness/provider, model, mode, project and paused state.

The configured-role editor preserves the operator's explicit order for additional
Workers with keyboard-focusable Up/Down controls; the revision-guarded native
roster commit stores that order and reloads it from SQLite. This is configuration
only, not task delegation or permission to execute multiple Workers.

The command inspects the **existing PandaCode Skills Library** and each bot's
assigned skill IDs without importing, installing, activating, executing or granting
anything. It reports library presence and native package-scan safety state
(`clean`, `findings_acknowledged` or `review_required`) separately. Missing
assigned IDs remain visible. A native, per-member local configuration check
reports paused bots, missing assigned Skills and unresolved Skill safety findings
with actionable remediation in the existing Bots / Skills Manager screens.
It also detects Workers whose project changed after Team configuration; a
Coordinator may intentionally have a separate project. An out-of-scope Worker
is a configuration problem, never a grant or a reason to substitute a bot.
These checks do not infer a live provider capability, do not change any stored
bot or Team policy, and never grant execution rights. **Harness eligibility and actual invocation/evidence
are explicitly `not_verified`** until a task-bound provider and evidence contract
can establish them; a known package is not a verified reviewer skill.

The UI rejects mismatched Team/policy revisions and clears the snapshot when
refreshing or switching Teams. This is a **read-only configuration inspection**,
not an admission, task-effective immutable policy snapshot, provider liveness
proof, positive permission grant, or completed S2-UX wizard. Multi-role execution
remains disabled until native scoped role admission is implemented.

### Editing reusable Teams after completed demands

Disabled Teams can rename themselves and change their configured additional Worker/Reviewer roster after all previous Tickets reach terminal states and every external-effect receipt is reconciled. These operations increment the native policy revision and retain every historical Ticket's admitted revision, original Worker, objective, audit and evidence. Primary Coordinator/Worker/project identity changes remain forbidden once a Team has Ticket history; create a separate Team to change those identities. The native SQL guards also reject active/reviewable Tickets, running assignments, pending/uncertain effects and stale revisions at the moment of mutation. The UI enables role editing for settled history, but native validation remains authoritative even if the visible list is incomplete. No such edit starts a Worker or grants access.

### Portable bot-profile drift preflight (schema v3, partial S2)

The native companion export now includes schema version 3 and an **ordered,
configuration-only bot profile** for every Coordinator, Worker and Reviewer:
portable bot display name, configured provider/harness, model, bot mode and
assigned Skill IDs. It does **not** export provider credentials, local bot IDs,
task grants, objectives, session history or Skill contents. Earlier v1/v2
imports remain accepted under their original conservative policy.

A v3 preview/confirmed import requires the target installation to already
contain an unambiguous existing bot for each role with the same configuration.
Missing/changed model, harness, mode or Skill assignment rejects the entire
preflight **before a private backup or any SQLite mutation**. A destination may
use different local bot IDs. Version downgrades cannot discard required v3
profile metadata. Successful imports remain **disabled and Off** and do not
install Skills, change existing bots, issue a grant, or prove provider
availability/Skill invocation. Native execution capability still requires
independent task-scoped admission checks.

### Immutable admitted Team configuration (unified native Team snapshots)

Each newly admitted native Ticket stores an immutable, revision-bound Team snapshot
in `sa_ticket_team_snapshots`, including the ordered Coordinator, Worker and
Reviewer roles and each member's effective bot configuration. The snapshot is
persisted in the Ticket admission transaction with a SHA-256 integrity digest;
Team version membership remains immutable even if the reusable Team later changes.
This is the single authoritative admitted profile in the unified branch. The
legacy `sa_ticket_profiles` sidecar proposed independently by PR #150 is not
created or read here because it would duplicate the authoritative snapshot.

Before a new worker attempt or task-scoped provider/tool action, native code
rechecks the relevant immutable role and effective configuration digest.
Changes to the selected bot's provider, model, mode, reasoning, Skills,
project, or other pinned configuration fail closed instead of silently granting
a different identity or tool capability. Historical Tickets with explicitly
unknown legacy profile provenance remain visible but cannot use that unknown
profile to authorize a fresh session. A stored configuration snapshot is never
itself a tool grant, a Council verdict, or evidence of Skill execution.

### Guided native Team creation (S2 UX, partial)

**Guided Team setup (five steps)** is available alongside the existing quick
Create form. It preserves local draft state through asynchronous native list
refreshes, allowing operators to configure the scenario, ordered existing
Coordinator/Workers/Reviewers, actual assigned bot Models/Skill IDs, optional
mandatory native Council, and a final native-policy preflight before Save.
All user-entered values are HTML-escaped, and the native `superagent_create`
command rechecks role distinctness, current paused state, project boundaries,
Council reviewer count, versioned Team snapshots and atomic persistence.

The wizard does **not** invent bot or Skill identities, install packages, infer
a harness capability from Skill assignment, issue a provider grant or execute
a model. The Team is always persisted Off; its separate native Manual enable
remains explicit. GitHub Auto, multi-Worker execution and nested subagent
admission are labelled unsupported until the corresponding native runtime
capabilities exist. The five-step UI is a configuration aid, not proof of
S2/S3/S4/S5 autonomous execution completion.

### Native persisted Work Plans (DD-03/S2 planning slice)

A bounded, immutable dependency DAG can now be attached to a blocked,
revision-bound local-user Ticket. The schema stores up to 64 ordered work
items, 192 dependency edges and independently bounded acceptance criteria.
Native admission verifies the original read-only mandate, exact Ticket state
and revision, nonempty work item identity and acyclicity, then atomically
records the plan and audit digest. Replaying the same plan is idempotent; a
different plan cannot silently replace its evidence. A read-only inspector
exposes the initial ready frontier and explicit `executionGrantedByPlan:false`
and `evidenceVerified:false` signals.

This delivers **real persisted planning data**, not the outstanding
dependency-aware execution scheduler. Multi-worker dispatch, advancement
after actual verified work, retries, budgets and Council-backed completion
still require additional task-bound native runtime integration.

The **Ticket evidence → Work item dependency plan** panel now exposes explicit
native commands for registering and inspecting this immutable, revision-bound
DAG. The local JSON editor cannot alter Ticket identity or revision; those
are obtained from the current native Ticket snapshot. The UI escapes all
user-generated titles and acceptance text, displays the exact stored digest
and shows every item as *not executed / not verified*. No ready-frontier
entry grants a task, provider or external effect. The editable JSON draft
is isolated to its original inspected Ticket rather than silently reused
when switching Demands. Unsupported native plan inspection remains an
explicit error, not synthesized content.

### Native provider-session identity pinning (S1/S2, PR #150 follow-up)

Active SuperAgent sessions are checked against their immutable, versioned
`sa_ticket_team_snapshots` profile at the **provider/tool boundary** and again
before **callback settlement**. In addition to the current native Team/mandate
revision, actor, Bot and Council guards, the session's saved project, provider,
model, mode and reasoning effort must exactly match the admitted role's
SHA-256-verified configuration. An altered session cannot inherit another
harness or silently upgrade the result to an approvable Ticket.

The stored session profile stays unchanged if an operator edits a reusable Bot,
but those edits still cause the live Team-profile boundary to fail closed
until a freshly authorized Demand is created. This conservatively retains
existing revocation semantics rather than assuming changed Bot defaults are
safe to adopt. Legacy/unknown snapshots remain non-authoritative.

The native regression suite covers modified session fields, denied provider
access, denied final callbacks, duplicate callback handling and preservation of
session fields after later Bot edits. This is not a writable Work grant, not
proof of task completion, and not a substitute for verified Council/effects.

## Portable Teams and native objective search

Portable Team export now emits schema v5. Each role retains its
non-secret configured reasoning effort, provider, model, mode and Skills.
The exported versioned strategy also includes priority, bounded attempt
timeout/retries, requested parallelism, and optional token/cost and nested
delegation preferences. Legacy schema v1-v4 remains readable; v5 requires
one complete effective role profile and a validated strategy. Import rejects
ambiguous/missing bots, unsupported priorities, malformed budgets, or Council
requirements without eligible independent reviewers before any Team write or
backup. A successfully imported Team always starts disabled/Off, including
when its portable strategy requests unsupported parallel, spending or
delegation capabilities. Export never carries issued grants, live Tickets,
worker results, secrets, credentials or source subscriptions. The canonical
Team remains an atomic, immutable versioned native snapshot rather than a
parallel mutable roster.

The **All Demands / Runs** view supports explicit literal objective search.
Native Team and Ticket-state filters, objective matching, and newest-first
keyset pagination execute in SQLite before returning a page. Search input
is bounded to 128 bytes and does not grant execution, approval, or access
to private task transcripts. The headless counterpart returns only redacted
Ticket metadata, not the matching objective text.

## Native Team optimization policy

Each reusable Team has an immutable, versioned strategy snapshot. The **Teams**
workspace exposes **Resource strategy and native capability** for an existing
Team. A local operator can choose `balanced`, `cost`, `latency`,
`throughput`, or `quality` as a planning preference, a bounded worker
attempt limit, and a timeout of 1–30 minutes. The selected configuration is
pinned when a new Ticket is admitted; future Team changes never edit an
already-admitted Ticket's authorization or previous Team version.

The strategy can also retain requested concurrency (1–8), optional token and
estimated-cost ceilings, and an explicit future subagent-delegation preference.
These fields are **not** a grant: native parallel scheduling, trustworthy
token/price metering, and task-scoped nested subagents are not yet available,
so enabling a Team requiring them fails closed. UI and native inspection
identify each unsupported capability rather than claiming a real Economy or
Performance optimization result. Ordinary default manual Teams remain
compatible. The headless inspection path
`pandacode serve --inspect-team-strategy <team-id>` reads the saved policy and
support blockers without starting or recovering a provider.

Execution timeout checks use the Ticket's immutable versioned budget at
provider admission, result settlement, watchdog expiration, and external
effect attestation. The hard platform limit remains 30 minutes. Ticket
history, evidence, native Council requirements, write-scope restrictions,
and GitHub receipt reconciliation are independent mandatory gates.

## Evidence-bound Work Plan progression

A saved native Work Plan is a bounded, immutable DAG. Schema v56 adds
`sa_work_item_attempts` and `sa_work_item_approvals` sidecars; existing
plan definitions and historical Team/Ticket snapshots are not rewritten.

For an explicitly admitted native manual or headless retry, the same SQLite
transaction binds one eligible Work Plan item to the real Worker assignment.
The Worker receives its title and acceptance conditions as task context,
**not** as permission to expand its tool, file or external-effect scope.
A provider callback records the result's SHA-256 digest as **observed**
evidence, or a failed attempt. Rejected/expired/revoked assignments and
restart recovery leave the item failed or pending review, never verified.

The Ticket Inspector shows each item as planned, running, observed, failed
or verified. Its frontier advances only when predecessor items have
independent native verification receipts. For supported Council-Off,
read-only mandates, the local user can review an attributed Worker result,
check its original session/thread and approve that individual item. This
action does **not** approve the whole Ticket or automatically dispatch an
unverified next Worker. If more items remain, the Ticket returns to a
blocked/retryable state under the same native authorization; a new attempt
still passes checkout, session, policy and bounded retry guards. Once all
items are verified, final Ticket acceptance is a separate native action.

**Known limitation:** this is durable evidence-based DAG progression, not
yet an autonomous cross-item execution scheduler, independent automated
test oracle or nested-subagent runtime. Council-required item verification
requires a separate task-attributed Council adapter and currently fails
closed; provider output alone can never pass it. GitHub Auto admission,
parallel leased Workers, real cost metering and unattended final acceptance
remain independent implementation requirements.

## Provider-observed subagent hierarchy (native Read Inspector)

The native Ticket Inspector can read **actual** subagent events already persisted
by the providers' run tracker, without importing another bot registry or
fabricating runtime child sessions. The native child-tree command resolves
only child rows of the Ticket's persisted native Assignment sessions, preserves
parent-child IDs and the originating Worker bot, and derives counts for active,
waiting and nested descendants. It caps the displayed collection at 256 and
flags missing/cyclic ancestry, which never counts as accepted evidence.

The **Provider subagent hierarchy** list is an accessible alternative to an
execution graph. It deliberately distinguishes configured Team Workers, Council
Reviewers and provider-reported subagents. A provider claiming completed does
**not** create a new native Worker grant, prove its own independent permissions
or satisfy Council acceptance; those checks belong to the native Ticket and
finalization state machine.

This observation feature does **not** enable a new recursive spawn capability,
parent/child tool authority or autonomous child scheduling. Native child
admission must be implemented and verified before Team strategy can advertise
that ability. Historical provider subagent events are evidence, not a source
of writable credentials or permission escalation.
