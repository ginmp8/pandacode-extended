# SuperAgent Teams and Demands workspace (issue #148, partial)

The existing **Bots → Orchestrations** workspace offers native-backed, persistent views
for reusable Teams and their Demands/Runs. One configured Team is one SuperAgent;
each independent Demand is a durable Ticket bound to exactly one Team.

## Native-backed surfaces

- **Teams:** saved policy revisions, coordinator/worker bot identity and native task counters.
- **Demands/Runs:** a read-only cross-Team index limited to the 200 most recently
  updated native Tickets. Board and List show the same loaded records. The board
  buckets Working, Awaiting decision, Needs attention and Finished or stopped.
  All column counts represent **only the loaded page**, not complete totals.
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

The views never issue Mandates, spawn sessions, infer task approval, start source
Auto, execute GitHub effects or authorize tools. Source events do not create
Demands. Coordinator and Council remain read-only; existing task admission must
repeat native guards. Unsupported multi-worker execution, scoped writes,
recursive subagents and production external-effect dispatch remain unavailable.
Ticket selection is discarded on Team change and when a native reload finds
the Assignment missing. Board uses ordinary keyboard-focusable native buttons
and the same theme tokens for desktop, narrow, light and dark views.

## Executable evidence and limitations

- Rust: native_ticket_snapshot_inspects_attempt_budget_and_revocation_without_grants.
- Playwright: native-backed attempt inspector shows recorded sessions and run
  budgets without granting effects.
- Playwright: cross-Team Demands index checks Board/List presentation while
  opening only the immutable Ticket's original Team.

These tests must pass at the exact current SHA. This partial feature does not
close #131 AC-01..31 or #148 UX/SA/DD/NS/EGM acceptance; full native Council,
source Auto, safely scoped worker writes, headless parity and recovery remain.


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
