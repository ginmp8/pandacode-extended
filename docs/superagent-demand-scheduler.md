# Native SuperAgent Demand scheduler — issue #148

**Status:** partial production implementation, not a claim of full issue completion.

## Intent and sources

- A reusable Team is configured from existing PandaCode bots. An operator submits
  one Demand to its Coordinator; its original Ticket, policy revision, project,
  Team-version snapshot and native mandate remain the source of authority.
- The Coordinator is asked for a single strict JSON object with `workItems`,
  whose objects have `key`, `objective`, `workerBotId`, `dependsOn` and
  optional `requiresWrite`. This is **untrusted plan data**, not a tool grant,
  Shell access, Council approval or authenticated GitHub event.
- The operator can also create a plan explicitly through
  `superagent_plan_demand`. A DAG is bounded to 32 items, rejects missing
  dependencies, duplicates, cycles, oversized objectives and Workers outside the
  original immutable Team. Each plan has a SHA-256 payload digest.
- Each eligible item becomes its own native child Ticket/Mandate/Assignment,
  separate session and audit trail using the existing bot gateway. Dependencies
  become ready **only after a child Ticket is natively completed**, not after
  a model turn. The existing provider/checkout lease prevents competing writes
  to the same project. Busy Workers stay pending rather than acquire the
  authority of another Worker. Independent safe items keep progressing.

## File-write authority

- `requiresWrite: true` merely requests a capability. The item is **not ready**
  until a local user invokes `superagent_authorize_work_item_write` and approves
  exact normalized project-relative paths in PandaCode's native dialog.
- The approved scope is stored immutably in `sa_work_item_write_scopes`,
  bound to the root Ticket revision. Only eligible existing Codex Workers on
  platforms supporting native scoped-write path pinning can receive it.
- The child grant is inserted in the **same transaction** as its native
  Assignment and session admission. Every write goes through
  `pandacode_superagent_write_file`, the existing bounded ToolHost with
  effect-time grant checks, file scope, receipt and revocation.
- Other filesystem, shell, MCP, GitHub and review effects remain denied.
  The original Coordinator and native Council reviewers remain read-only.
  An imported Team or a model's requested paths cannot create a grant.

## Completion, interruption and recovery

- Native bot completion invokes the scheduler. It records only stored statuses
  and can start new independent eligible WorkItems without user "continue"
  messages. A child Ticket's `completed` state is the predecessor proof;
  `waiting_review`, `waiting_approval`, `blocked` and `failed` do not pass.
- When **all** children are truly completed and no external effect remains
  pending/uncertain, the root Demand can transition atomically to completed
  with native audit, as part of its original operator-authorized one-shot run.
- Root Stop revokes all child mandates, assignments and pending effects within
  the original SQLite transaction; native provider admission checks the
  original root's authority. Active child processes are cancelled afterward.
- Unknown or interrupted effects remain unresolved; the scheduler never
  infers exactly-once network delivery, task approval, or a successful test
  from model text alone.

## Current limits and verification

The incomplete items still include advanced provider-local test execution and
quality-backed autonomous acceptance without per-child human decisions, full
multi-checkout concurrent writes, continuous headless admission after restart,
GitHub Auto and native Worker-created recursive subagents. Cross-platform
compilation, real authorization/denial oracles, migration/restart fixtures,
Council callbacks, end-to-end UI/keyboard proof and the full AC/UX/DD/SA/NS/EGM
ledger must be executed on the final exact SHA. Do not merge this draft based
only on source presence or a partial green CI matrix.
