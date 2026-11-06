# PandaCode Extended 0.10.0 — feature controls

**Status:** Inherited control contract; no new 0.10.0 behavior or native On/Off verification claimed.
**Opening reference:** [Extended `e8122223`](https://github.com/ginmp8/pandacode-extended/commit/e81222232314844d8fc52f65cb8e2dd37c5da2c8); upstream [source `fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852) (source application 0.3.97).

These existing Extended preferences restrict only the relevant automatic or remote operation when disabled. They must **not** remove the upstream feature, bypass required user approval when enabled, or silently change established defaults.

| Existing control | Required enabled or manual behavior | Required disabled behavior |
| --- | --- | --- |
| Automatic app/agent updates | Explicitly enabled checks function; manual checks remain usable | Only configured automatic checks are suppressed |
| Connect and direct routes | Approved/authenticated relay and separately allowed LAN/Tailscale routes remain usable | Only the disabled connection route is blocked, including after restart |
| Global bot pause | Configured bots and schedules can run when unpaused; pause is optional | Deferred bot work stops without destroying configuration |
| External MCP exposure | Allowed, newly launched harnesses retain permitted MCP tools | Disallowed external MCP access is suppressed in the relevant process scope |
| Cloud-agent credential transfer | Explicitly permitted transfers remain subject to authentication and approval | No new disallowed automatic credential transfer |
| Agent-managed cloud and VM actions | Authorized operations remain available when enabled | Restricted agent mutations are blocked without removing safe manual/read-only paths |

**Acceptance still required for the final 0.10.0 SHA:** actual enabled/disabled and manual/automatic paths, process restart, existing-agent behavior, bot pause/resume, real two-device Connect, Cline MCP, cloud/VM operations, and credential transfer. Source presence and green CI do not certify these runtime scenarios.

See the [previous 0.9.0 evidence](../0.9.0/feature-controls.md), [parity contract](../upstream/parity-contract.md), and [0.10.0 checklist](release-checklist.md). Any future control changes must be documented with a real code diff, tested behavior, and a narrow rationale.

## 2026-11-05 upstream documentation-only sync

The pinned upstream [`78f77a0`](https://github.com/ginmp8/pandacode/commit/78f77a0becee2029f4b89ef60fca0c97e48251e2) changes documentation/screenshots and screenshot capture scripts, not production control paths. It does not establish any new Settings behavior. Existing On/Off, restart, process lifetime and remote integration acceptance requirements above remain open until executed on the final Extended commit.

## Upstream 0.3.98 Settings coexistence — 2026-11-05

The newly inherited **Recommended** section displays suggested configuration state and navigation only; it must not toggle or bypass Extended policy controls. **Feature controls** remains a distinct Extended section alongside the new upstream section. The upstream opt-in migration for the running-agent widget is independent from Connect, MCP, bots, cloud/VM, credentials and update-policy gates. Its default must not silently change those controls.

The reconciled `src/settings-sections.ts`, `src/settings.ts` and affected tests retain both sections. Static source-merging checks are not proof of native On/Off behavior, persistence, restart semantics or upstream capability when enabled; perform those tests on the final exact SHA.
