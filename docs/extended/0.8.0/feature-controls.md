# PandaCode Extended 0.8.0 — inherited feature controls

**Status:** inherited from the previous Extended candidate; this document makes no claim of newly verified 0.8.0 runtime behavior.

PandaCode Extended must preserve upstream functionality and expose the existing user-configurable safety boundaries. A switch set to **Off** restricts only the operation named by that switch; **On** retains the corresponding upstream capability subject to normal permissions and platform requirements.

| Control | Existing behavior to retain | Important scope |
| --- | --- | --- |
| Automatic agent/app update checks | Configurable background checks; manual checks remain available | Not a blanket disable of updates |
| PandaCode Connect and direct LAN/Tailscale routes | Connect is separate from its optional authenticated direct route | Disabling direct routes should not implicitly disable permitted relay access |
| Cloud credential transfer | Opt-in for copying local credentials to new cloud agents | Turning Off does not revoke previously copied credentials |
| Agent-driven VM/cloud management | Restrict agent-initiated mutations while preserving manual/read-only paths | No upstream provider removal |
| External MCP servers in Claude/Codex | Configurable for **newly launched** supported processes | Effective Codex isolation still requires integration verification |
| Global bot pause | When enabled, pause supported channels/bots and defer due work without deleting configuration | Disabling pause restores only otherwise-enabled integrations |

The detailed source behavior and historical evidence are in [0.7.0 feature controls](../0.7.0/feature-controls.md). The governing [upstream parity contract](../upstream/parity-contract.md) supersedes any shorthand here.

## Additional inherited 0.3.91 flows

The upstream cloud Settings screen now sorts connected providers first and adjusts the environment help/checkbox layout. These presentation changes must coexist with Extended's credential-transfer and cloud/VM-management opt-in controls; no control is intentionally relaxed. Confirm both allowed (On) and disallowed (Off) paths, including manual read-only flows, after this source integration.

## Required 0.8.0 verification

- [ ] Verify every control On and Off on the final candidate SHA, including Rust enforcement, UI state, manual-path availability and failure/restart behavior.
- [ ] Verify no credential exposure and no loss of legitimately enabled cloud, remote, MCP or bot capabilities.
- [ ] Check existing connections and running processes for intentionally non-retroactive policy effects; document the user-facing consequences.
- [ ] Reconcile any changes from a future upstream HEAD rather than overwriting Extended controls with raw upstream files.

These are **planned checks**, not executed 0.8.0 acceptance results.

## Source 0.3.92 Settings integration

The redesigned upstream Settings navigation moves page definitions into `src/settings-sections.ts`. Extended retains a separate **Feature controls** section wired to `mountFeatureControls` and keeps the Connect master switch while adopting the new upstream remote Settings presentation. The controls are intended to restrict only their named operations; upstream manual actions remain available where permitted. The final candidate requires browser/native On/Off and restart verification; source inspection alone is not a runtime acceptance result.

## Source 0.3.93 integration

Codex now supports process-local thread-limit handling that respects an explicit user limit. Extended's existing `external_mcp_servers` switch must continue applying separately to newly launched Codex processes. Both command arguments are preserved together; verification must exercise MCP On and Off, an explicit Codex thread limit, and the no-limit default without editing user configuration. Team resume, remote worker pagination, and bot scheduling must continue honoring the existing Extended permissions and controls.

This integration is based on source inspection and a nonoverlapping three-way merge; live control-toggle acceptance remains pending.

## Source 0.3.94 control compatibility

This upstream update primarily affects worker progress, live turn cards, and their tests. Extended opt-in switches remain independently configured. Confirm the changed worker UI with bots paused/resumed and Connect On/Off; no native toggle acceptance is claimed by this source copy.
