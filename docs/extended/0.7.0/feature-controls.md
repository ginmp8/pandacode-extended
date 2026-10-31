# PandaCode Extended 0.7.0 — Feature controls

**Settings → Feature controls** stores opt-in switches in the app's secure store. Newly
installed builds default to off for background agent update checks, credential transfer
to cloud agents, agent-driven cloud/VM management, and external MCP servers in
PandaCode-launched Claude and Codex. The global bot pause defaults to off. An existing
user's other settings (including individual bots and connected cloud accounts) are not
reset. The controls work independently of the application's own update mode and the
master **PandaCode Connect** switch.

- **Check agent updates automatically:** off suppresses the initial eight-second
  check and subsequent six-hour checks; **Check updates** still works manually.
  Updating an installed CLI remains an explicit action.
- **Transfer local credentials to cloud agents:** off refuses cloud-agent starts that
  require copying this computer's login files, API keys and GitHub token. It does not
  disconnect the provider, stop a machine, or revoke credentials already copied during
  an earlier run. Turn on before a new cloud run if that flow is required.
- **Allow agent VM and cloud management:** off leaves read-only machine inventory and
  screenshots available, but removes agent-facing VM mutation and cloud deployment
  tools from newly started runs. Previously registered callbacks also check the flag
  at execution time. Manual VM/cloud panels are independent.
- **Allow external MCP servers in Claude and Codex:** off requests an isolated
  MCP configuration for newly launched Claude and Codex processes, while retaining
  PandaCode's authenticated local tool bridge. **Codex effective isolation is
  not yet verified**; see [KI-01](known-issues.md#ext-070-ki-01--codex-mcp-isolation-may-not-be-enforced).
  Other harnesses maintain their native MCP configurations; this is **not** a
  system-wide MCP kill switch. Already-running agent processes retain their
  launch-time policy until restarted ([KI-05](known-issues.md#ext-070-ki-05--running-agents-retain-launch-time-mcp-configuration)).
- **Pause all bots and external channels:** on stops running bot threads, closes
  supported messaging listeners and prevents new gateway messages and bot schedules.
  Due schedules are retained for the next unpaused scheduler tick. Individual bot
  pause/channel preferences are unchanged, and re-enabling only starts channels that
  were independently enabled.
- **Use direct LAN / Tailscale routes:** on (the existing Connect default) allows
  authenticated direct connections when Connect itself is on. Off selects the relay
  and drops existing direct routes by restarting the Connect socket. This setting
  does not independently enable Connect.

Disable actions are enforced in Rust as well as reflected in the Settings UI; the
frontend switch alone is not treated as an authorization boundary.

This is an **Extended-owned** feature. The upstream PandaCode Settings documentation does not specify these opt-in defaults or guarantee Codex MCP isolation. Release-specific dependency security and status are tracked separately in [Extended 0.7.0](README.md).
