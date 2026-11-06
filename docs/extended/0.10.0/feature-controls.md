# PandaCode Extended 0.10.0 — feature controls

**Status:** Existing Feature controls contract retained. The Extended-only bot-chat permission correction below is committed; native application and end-to-end On/Off verification are pending.
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

## Bot chat permissions (Extended upstream compatibility patch — 0.3.98)

The normal task permission picker originally wrote only `sessions.mode`. The upstream bot gateway calls `sync_thread_profile` before delivery and restores `bots.mode`, silently discarding that choice. In Extended, the permission picker **in the bot's own desktop conversation** now calls a dedicated, main-window-guarded `bot_set_mode` operation; it writes **only** `bots.mode` and uses the existing upstream profile synchronization. The UI confirms the change when enabling **Full access**.

This is a **bot-wide** setting, not an isolated per-chat grant: it affects the bot's desktop conversation, connected channels, shared-channel/bot-to-bot work and scheduled runs. Existing active turns keep their launch permissions; new turns use the selected mode. Ordinary task permission changes continue to use `update_session`. No provider permission bridge, feature-control switch, SQLite schema, or upstream default is changed.

**Forward-parity rule:** on each upstream merge, check whether PandaCode fixed chat-vs-profile permission changes. Keep the smallest working correction or remove this Extended patch if upstream now provides equivalent behavior. Validate full/ask/plan transitions, canceling the Full access confirmation, persistence after a bot message/reopening, external-channel propagation, and an active-turn handoff. Never preserve two independently writable bot permission policies unless upstream makes the transport boundary explicit. Relevant regression tests: `src-tauri/src/bots/tests.rs` and `tests/bots.spec.ts`.

## Upstream 0.3.99 integration checkpoint — 2026-11-06

Source [`6b94bf1d`](https://github.com/ginmp8/pandacode/commit/6b94bf1d8b579d08834578f6e24684ea0d3fccbd) changes team-lead changed-files presentation and does not intentionally change the existing Extended Settings policy or bot-chat permission patch. Connect, direct routes, global bot pause, external MCP, updates, cloud/VM and credential-transfer switches retain their On/Off contracts. Native Settings On/Off, persistence, bot resume and remote integration are not verified by static import.
