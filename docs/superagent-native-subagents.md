# Native bounded Worker-created subagents — issue #148

**Draft implementation, not a claim that NS-01..12 are accepted.**

## Authority and trust boundaries

One configured Team remains the SuperAgent. The Coordinator and existing
Worker/Reviewer roster never change when a Worker requests a temporary child.
Only an existing, pinned Team Worker holding a native active Assignment inside
an originally authorized Demand may request a descendant. A trusted local
user must first confirm `superagent_enable_subagents` with exact root Ticket,
policy revision, Team version and limits:

- Depth 1–3, at most 1–4 children per parent, 1–24 descendants in total
- No self- or ancestor-bot delegation; no new bot profile or saved Team role
- No imported/bot-supplied/grandchild authority to increase these limits
- No inherited file/shell/MCP/GitHub/effect permissions or Council reviewer power
- The root Stop/revocation applies to every native child and grandchild

The tool `pandacode_superagent_spawn_child` is published to an eligible native
Codex worker only through the existing native provider read-only policy and
an exact restricted ToolHost catalog. Its parent and root Ticket IDs, eligible
existing Worker IDs, bounded objective and stable key are checked again at the
native effect boundary. The tool produces a **pending** immutable lineage
record, not a run grant or immediate successful delivery.

## State, scheduling and inspection

The native `sa_subagent_policies` and `sa_subagent_nodes` tables record root,
parent Ticket, parent Assignment, immutable worker identity, depth, request
digest, child Ticket and state. Unique keys and native mutation guards make
duplicate requests idempotent and conflicting requests fail closed.

Once the parent Worker releases its native checkout Assignment, the scheduler
may admit an eligible read-only child Ticket in that same root Team, preserving
original project/role snapshots and resource leases. Every descendant must
pass its own native evidence/review gate. A parent Worker may not be accepted
while it has required unsettled children; native Council finalization also
checks that condition. Root completion waits for every WorkItem and descendant
to be truly completed, and unresolved external effects block finalization.

The dedicated Orchestrations Ticket inspector can load the saved child/grandchild
tree and the native subagent policy. The native scheduler resumes its read-only
reconciliation in desktop and `pandacode serve`.

## Unsupported/unverified operations

Full child-cost accounting, cross-provider subagent execution, detached child
worktrees, simultaneous writes to one checkout, dynamic reassignment and
all process-level crash/replay/Council acceptance scenarios are **not proven**.
The current child ToolHost is Codex-specific; no provider automatically gains
extra commands. This module is not permission for unattended GitHub publishing,
merge, deployment or secrets. User-operated Desktop/provider proof is separate
from executable automated CI. Execute NS-01..12 and the original AC-01..31
against the **exact final commit** before declaring issue completion.

No source Team/PR branch is rewritten by this integration.
