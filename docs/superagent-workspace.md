# SuperAgent Teams and Demands workspace (issue #148, partial)

The existing **Bots → Orchestrations** workspace offers native-backed, persistent views
for reusable Teams and their Demands/Runs. One configured Team is one SuperAgent;
each independent Demand is a durable Ticket bound to exactly one Team.

## Direct read-only assignment to a configured Worker (partial S2)

For a Team with two or more configured Workers and **no Council Reviewers**, the
trusted local operator may now explicitly enable the Team, open **Demands / Runs**,
load its native Worker roster, select one Worker and choose **Worker directly**.
A fresh native preflight shows the exact selected bot, project, policy revision
and zero tool/side-effect authority. Submission independently rechecks the
selected roster member, immutable project scope, current policy and live checkout
occupancy before atomically creating the Ticket, task-scoped Mandate, Assignment,
native bot conversation and audit. The original role/profile snapshot includes
the entire configured Team, while the Ticket pins the one explicitly selected
Worker. Request-key replays cannot switch Workers or deliver another packet;
busy selected Workers are denied rather than silently queued. Stopping revokes
the same native Mandate and rejects late callbacks. Other ordinary bot chats
are not modified.

**This does not implement autonomous multi-Worker scheduling**, implicit worker
selection, Council reviewer execution, delegated subagents, scoped write tools,
or automatic retry. Coordinated dispatch for multi-Worker Teams and any Team with
configured Council Reviewers remain unsupported. The legacy single-Worker
admission path does not silently select the first member of a multi-Worker Team.
Native tests cover selection, prohibited identities, task profile persistence,
no ambient tool grants, idempotency and revocation; a focused Playwright test
checks that the UI supplies an explicit Worker ID. These tests require a passing
exact-HEAD CI run before the slice is considered verified.

## Native-backed surfaces

The global Demands workspace can filter by the exact native project ID already associated with existing Teams. The filter runs in SQLite **before** cursor pagination against each Ticket's immutable admitted project, not the Team's current name or a client-side page. It does not infer a GitHub repository, read private worker evidence or grant access. If a historical project is no longer present in any Team, the operator can still inspect its Ticket by ID through existing native recovery endpoints; the drop-down deliberately does not fabricate a stale Team/project mapping.


- **Duplicate Team:** the native `superagent_duplicate` command checks the source
  Team's exact policy revision and clones its ordered Worker/Reviewer roles
  within one SQLite transaction. A stale source, invalid roster or paused bot
  aborts without leaving a partial Team. The new Team is disabled and never
  copies Tickets, Mandates, source subscriptions, Council results or grants.
  The renderer does not attempt to assemble authority from separate reads.

- **Teams:** saved policy revisions, coordinator/worker bot identity and native task counters.
  The read-only effective policy inspector shows each member's configured provider,
  model, mode and reasoning effort alongside Skill Library status; these are
  configuration observations, not provider eligibility, grants or proof of execution.
  Native roster reads reject non-contiguous Worker/Reviewer positions and role
  aliases to the Coordinator or primary Worker rather than silently remapping authority.
  The directory shows native correlated member counts (one Coordinator,
  configured Workers and Reviewers) without inflating Ticket totals when a
  Team has multiple roles. Search and Off/Manual status filters operate only
  on the native-loaded Team directory and do not change the selected Team,
  stored policies, permissions or execution. Older fixtures without counts
  are marked unavailable rather than inferred.
  A separate native **needs attention** counter includes blocked, failed,
  interrupted and inconclusive durable Tickets, not just blocked; it is
  computed across the entire Team in SQLite (not from loaded page rows).
  Council-pending and human-approval states remain separate, and no status
  count implies a verified review or task completion.
- **Demands/Runs:** a read-only native Ticket index with stable 200-row
  cursor pages and an explicit **All Teams / specific Team** filter. The Team
  filter is enforced by the native SQL query **before pagination**, so sparse
  Teams are not omitted merely because other Teams have newer Tickets.
  Changing Team resets the pagination cursor and loaded Board/List rows;
  refresh queries current native state. The board buckets Working, Awaiting
  decision, Needs attention and Finished or stopped. All column counts represent
  **only loaded pages for the selected Team scope**, not complete totals.
  Unknown or malformed Team IDs fail closed instead of showing false empty results.
  Search objectives submits an explicit native SQLite substring query (ASCII
  case-insensitive and literal, including percent/underscore) before pagination;
  results do not depend on which pages were previously loaded. Search is
  restricted to the Ticket objective and does not expose request keys, actor,
  provider messages, private evidence or session IDs. The form accepts Enter
  or its Search button; an empty search restores the unfiltered native index.
  Searches are bounded to 128 UTF-8 bytes and cannot authorize an action.
- **Review inbox:** a separate native, bounded cross-Team query shows actual
  `waiting_approval` (human decision) and `waiting_review` (native Council
  pending, **not** a verified pass) Tickets, even when they fall outside the
  general Demand index's most recent 200 rows. Open Demand resolves the
  original Team before offering native task controls.
- **Open demand:** resolves the original Team, fetches the exact Ticket
  when outside the Team's latest 50, verifies its Team ownership, then offers
  the established native Ticket controls.
- **Admitted Team configuration:** the native Ticket snapshot compares its original
  Mandate policy revision with the current Team policy inside one read transaction,
  while retaining the frozen original role identities, models and assigned Skill IDs.
  The Demand inspector displays both revisions and the original configuration,
  not current bot names or local checkout paths. Drift never changes admission
  authority, authorizes a tool or establishes actual Skill invocation. Legacy
  Tickets without a recorded profile explicitly show unavailable provenance.
- **Native Assignment inspector:** the Ticket snapshot includes transactional
  native Assignment insights: current linkage, recorded result presence, revocation,
  bounded attempt budget and timestamps. No provider result content is displayed.
  A persisted running assignment is **not proof of live provider execution**.

## Non-authority and recovery

The views never issue Mandates, spawn sessions, infer task approval, start source
Auto, execute GitHub effects or authorize tools. Source events do not create
Demands. Coordinator and Council remain read-only; existing task admission must
repeat native guards. Automatic multi-Worker delegation, scoped writes, recursive subagents and production
external-effect dispatch remain unavailable. Explicit operator-selected direct
read-only Worker admission has the limited native boundary described above.
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
- Rust: native_demand_search_is_pre_paginated_scoped_literal_and_restart_safe
  validates >200 newer nonmatches, literal wildcard and SQL-injection text,
  native Team/state intersection, cursor stability, invalid input and restart.
- Playwright: native objective search invokes the indexed native Ticket projection,
  supports Enter/clear, retains Board/List parity and issues no write commands.

These tests must pass at the exact current SHA. This partial feature does not
close #131 AC-01..31 or #148 UX/SA/DD/NS/EGM acceptance; full native Council,
source Auto, safely scoped worker writes, headless parity and recovery remain.


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

### Portable bot-profile drift preflight (schema v4, partial S2)

The native companion export now includes schema version 4 and an **ordered,
configuration-only bot profile** for every Coordinator, Worker and Reviewer:
portable bot display name, configured provider/harness, model, bot mode,
reasoning effort and assigned Skill IDs. It does **not** export provider credentials, local bot IDs,
task grants, objectives, session history or Skill contents. Legacy imports remain accepted under their original
conservative policy.

A v4 preview/confirmed import requires the target installation to already
contain an unambiguous existing bot for each role with the same configuration.
Missing/changed model, harness, mode, reasoning effort or Skill assignment
rejects the entire preflight **before a private backup or any SQLite mutation**.
Earlier v1/v2/v3 manifests remain importable under their original conservative
checks; v3 did not pin reasoning effort. Merely changing a v4 document's
schema version to v3 cannot bypass that check while retaining v4 fields.
A destination may use different local bot IDs. Successful imports remain **disabled and Off** and do not
install Skills, change existing bots, issue a grant, or prove provider
availability/Skill invocation. Native execution capability still requires
independent task-scoped admission checks.


### Immutable admitted Team profiles (schema v46; partial DD-13 / S2)

At the point of each native manual/headless Demand admission, the SQLite
transaction now captures its Team policy revision and a bounded, configuration-only
profile for the selected Coordinator, ordered Workers and optional Reviewers:
existing bot identity, project, provider/harness, model, mode, reasoning effort
and assigned Skill IDs. It saves the profile atomically with the Ticket, Mandate,
Assignment and admission audit. The task inspector returns the historical
`admittedProfile` even when bot defaults subsequently change. Existing pre-v46
Tickets report no profile rather than fabricated historical values.

A **new worker session** initiated by retry or after a coordinated plan must
match the captured worker profile. If the bot changes, native admission stops
without dispatching a more privileged or different model; the operator must
create a new explicitly authorized Demand. Already admitted sessions retain
their pinned session profiles, and ordinary chats are unaffected. This snapshot
is **not** a tool grant, a verified Skill invocation, a working multi-worker
scheduler, or a substitute for native Council/effect-time authorization.

Rust regressions cover replay, changed bot profile denial, fresh-Demand profiles,
restart persistence and schema-v45 upgrade preserving historical Tickets.


## Atomic multi-role Team creation (partial UX-P2)

The native `superagent_create` IPC accepts an optional ordered `roster` with `workerBotIds` and `reviewerBotIds`. It also accepts optional `maxWorkerAttempts` (1–4); a provided cap is saved transactionally with the disabled Team and its roles, while omission preserves the legacy default of two. Example templates suggest a bounded cap but never select bots or enable retries, permissions or work. Existing callers without this argument retain the one-Worker contract. With a roster, the Team and all role rows are created in one SQLite transaction. Duplicate, paused, missing, over-limit and cross-project Worker roles fail without leaving a partially created Team. All roles reuse existing bot IDs. Teams remain disabled/Off; multi-member execution remains unavailable pending native scoped admission. The workspace exposes these optional members before Save and disables unsupported execution. Rust rollback/restart and Playwright creation regressions are authored, but their execution requires verification on the published SHA.


### Native Ticket activity paging (desktop)

The Ticket inspector begins with a bounded latest-50 native audit snapshot. The user can
explicitly load older pages through `superagent_activity_page`, with an exclusive audit-ID
cursor, a 1–100 item limit, Ticket identity checks and descending durable event order.
All/Compact/JSON views and the text filter operate on the loaded native events only;
there is no inferred history or implicit provider action. The main-window IPC guard
protects full private audit details. Headless activity inspection remains separately
redacted and read-only. Missing or invalid Tickets and cursors fail closed.

Validation: `native_desktop_activity_pages_are_ticket_scoped_strict_and_restart_durable`
(Rust) and `desktop native Ticket activity pages are keyset-bound` (Playwright).
These tests do not constitute proof of native Council or tool authorization.

## Versioned Team purpose and strategy (v50, partial UX-P2)

The native Team roster now stores a bounded purpose (up to 400 characters)
and one of `balanced`, `economy`, `performance`, `security_critical`
or `custom`. The disabled Team's **Save Team purpose and strategy** action
uses the same main-window authorization, policy-revision compare-and-swap,
unsettled-Demand prohibition and transaction boundary as the Worker attempt
budget. Invalid or stale edits fail before changing the saved intent.

An admitted Demand freezes the effective Team purpose and strategy in a
revision-bound `sa_ticket_strategies` record **in the same transaction** as
its Ticket, role profile, Mandate and first Assignment. Subsequent Team edits
do not rewrite that snapshot. Pre-v50 Tickets deliberately show unavailable
strategy provenance; their authorization and provider behavior remain unchanged.
Duplicating a Team includes the strategy, but never copies runtime state.

Portable SuperAgent manifest **v6** carries both fields, validates them before
import, rejects conflicts with existing Teams and imports new Teams disabled.
Versions 1–5 remain importable with empty purpose and balanced strategy.
No strategy value grants tools, changes existing bot models, selects providers,
infers costs, initiates GitHub work, supplies a Council verdict or launches a
multi-worker runtime. An actual optimization scheduler remains out of scope for
this validated configuration slice.

Evidence targets: `team_strategy_is_versioned_portable_and_pinned_per_demand_after_restart`,
`portable_v6_strategy_roundtrip_rejects_conflicting_and_legacy_metadata`
(Rust), and `Team strategy changes through native CAS and persists after
refresh` (Playwright). Execution results must be checked on the **exact
published commit**, not inferred from these test declarations.
