# Settings

Settings opens on **General**, with grouped rows for text size, compact conversations, and wallpapers. Wallpaper customization includes separate live visibility and blur controls, the seven scene sets, and a fixed favorite. Preferences save automatically.

**Agents** contains the default agent, tools and integrations, and compact installation status rows. Expand a provider to see its version and executable path. Refresh checks installations without resetting the selected tab, default agent control, or expanded provider details.

**Agents → Project checks after turns** runs each task's checkout through its own fast check once a turn ends cleanly: the npm `test` script when package.json declares one, otherwise `cargo check` for a Rust checkout, with a two-minute limit. These are the project's own test or check commands, run with your own environment, so they can do anything running them in a terminal would. Tasks in plan mode or read-only are skipped, and a result that arrives after the task started running again, gained queued messages, had you send a message, or had the setting turned off is dropped. A check that cannot start (say, npm is missing) or finish in time is skipped, not reported as a failure, and nothing runs when the project declares no check. When the check fails and the setting is on, one fix-it message with the bounded failure output joins the task's message queue — visible and editable like any queued message — and the idle task starts on it. A repair turn's own failure waits for the user instead of looping; sending a message yourself restarts the repair budget. Teams, bot conversations and tasks with queued messages are not checked. The setting is off by default and kept per window.

**General → Quick launcher** enables the global overlay and changes its shortcut. The default is Command + Shift + Space on macOS (Control + Shift + Space elsewhere). Record a key combination or type one, then Save. PandaCode must remain running; clicking another app hides the overlay without stopping agents. Escape returns from a conversation to the active list, then hides the overlay on the next press. The lightweight overlay shows only currently active agents: running tasks, agents waiting for approval, coordinating architects, their workers, and provider subagents. Selecting an entry opens its streaming conversation inside the overlay, including worker and subagent responses. The back arrow returns to the list. Only the explicit **Open in app** button brings the full workspace forward; a conversation remains readable if its agent finishes while you are viewing it. Completed, idle, and stopped runs disappear automatically. The model button beside the project opens the same searchable harness/model picker as the workspace, with provider logos and discovered model names. An explicit overlay selection persists across restarts and workspace task changes. Until one is chosen, new work follows the workspace’s last harness/model/effort; permissions and project context continue to follow the workspace. Saved team workers are preserved, while an explicit overlay selection sets the architect for the new task only. Its project selector can change the launch project. Shortcut conflicts leave the previous binding intact.

**Agents → Provider connections → Set up** configures GLM Coding Plan and optional executable paths for OpenCode or Antigravity. GLM requires Claude Code plus a Z.ai Coding Plan key; the saved key lives in PandaCode's 0600 `secrets.json` on macOS, the Secret Service on Linux and Credential Manager on Windows; you can also set `ZAI_API_KEY` in the application environment. OpenCode uses its own CLI login. Antigravity requires Google's official ACP server and its sign-in; the interactive `agy` executable is not interchangeable. See [provider details](providers.md).

The **Claude Code** card reports inherited credential sources and checks the saved
Claude login. **Prefer saved Claude login** removes inherited API credentials and
provider routing overrides only from new Claude processes started by PandaCode.
Choose **Use current environment** to restore the default. Claude settings and
organization policies can still affect authentication; see
[Claude login details](providers.md#claude-login-and-environment-credentials).

The two tabs support arrow keys, Home, and End. The layout uses rem-based dimensions and stacks controls in narrow windows, including at 140–160% text size. System colors and reduced-motion preferences apply to both tabs.

The Settings module and styles load when opened. General uses local state immediately; provider discovery starts only when Agents is opened or refreshed. A delayed discovery response cannot replace a different dialog or discard wallpaper edits. Closing or replacing a settings view removes its event handlers and contents.

## Verification

The update passed 23 Node checks, the production build, 55 Chrome browser cases, and 41 WebKit cases. These cover blur dragging and persistence, wallpaper schedules, tab keyboard navigation, delayed discovery, default-agent menus, and layout at enlarged text sizes. One existing WebKit inspector-resize timing case passed when rerun in isolation. General, Agents, and wallpaper controls were visually checked in dark and light themes and in a narrow window, with no page errors.

## Feature controls

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
  not yet verified**; see [KI-01](known-issues-extended-0.7.0.md#ext-070-ki-01--codex-mcp-isolation-may-not-be-enforced).
  Other harnesses maintain their native MCP configurations; this is **not** a
  system-wide MCP kill switch. Already-running agent processes retain their
  launch-time policy until restarted ([KI-05](known-issues-extended-0.7.0.md#ext-070-ki-05--running-agents-retain-launch-time-mcp-configuration)).
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
