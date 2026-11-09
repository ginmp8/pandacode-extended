# Council S1: native reviewer authority and isolation

## Compatibility

The existing Architect Council, its score parser, rounds, native automatic reviewer selection, saved WorkerProfile schema and ordinary bot/team workflows are unchanged in purpose. S1 adds a frozen per-seat reviewer binding, not a new Council engine or bot type. Legacy Council seats without the new field remain readable and receive the same restrictive runtime policy.

ReviewerBinding freezes a cycle ID, optional delivery ID, round, seat ID, optional reviewer bot ID, native artifact revision token, provider, model, lens and tool policy. Native artifact revision identifies the source state at dispatch; it is not a source hash, verified check or mandatory evidence gate. ReviewerState and ReviewerFailureCode are typed internal contracts for later integrations.

## Security boundary

The reviewer identity is resolved from admitted Architect state, never from a prompt or a bot profile. Every Council reviewer turn runs with plan/read-only permissions even if the parent session has Full access. This does not mutate the persisted bot profile or ordinary session mode.

- Codex receives the native read-only sandbox, network denied and no elevation. Its reviewer-specific CLI also disables native goals, multi-agent delegation, apps, plugins, remote plugins, browser and computer-use features. Feature flags alone do not establish isolation and must not override the fail-closed MCP-inventory preflight. Before starting a reviewer, the native CLI must successfully report an **empty** MCP inventory via `codex mcp list --json`; non-empty, disabled, unreadable, malformed or unsupported inventories and project-local MCP/plugin/app configuration fail closed. `-c mcp_servers={}` is only defense in depth: TOML merging does **not** remove inherited MCP servers.
- Claude, GLM and MiMo have defensive CLI flags for read-only tools but **must fail closed** as reviewer harnesses: managed hooks/settings or externally managed MCP servers can override session-level policies, so CLI filters alone are not verified isolation.
- No PandaCode host tools are registered for Council reviewers: bot profiles/memory, schedules, approvals, private messages, channels, settings, skills, Connections, deployments and team management tools cannot be called through the app's tool transport.
- A native permission escalation request is rejected without a user-overridable approval.
- Named Connections cannot inject reviewer-specific arguments, environments, MCP or secrets. Claude-family, ACP, other unsupported harnesses and cloud execution fail with read_only_unsupported rather than falling back to a writable execution context.

This is an implementation boundary, not proof that all CLI versions have no undeclared side effects. AC-03 and AC-18 require actual native-harness mutation attempts and filesystem/database comparisons, not just unit tests or serialized permission flags.

## Baseline and required verification

Source baseline: extended/development at d0100f72c25e3ccff58c294d624505af8a216b48. Before this implementation, GitHub CI showed eight passing checks and one failing Bot manifest and collaboration UI Playwright check.

Commands: cargo test --locked --manifest-path src-tauri/Cargo.toml --lib; cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings; npm ci; npm run build; npm test; and relevant Council/Architect/bots Playwright regressions in issue #126.

Desktop/harness proof remains required: configure a normally full-access bot reviewer, attempt actual shell, script, file, MCP, bot-profile/memory, schedule, approval, message, skill and Connection mutations, capture redacted before/after tracked and untracked filesystem hashes and SQLite states, exact CLI version/model and explicit permission denials. Unsupported harnesses are blocked/not tested. No simulated protocol test is a replacement for those experiments.

Do not check S1 complete until AC-03 and AC-18 have the required live evidence.

Reviewer support is provisionally limited to the Codex native read-only sandbox. This intentionally blocks native automatic Council reviews routed to unsupported models; it preserves the automatic selection algorithm, saved profiles and Council states without claiming unsafe reviewer execution is compatible. Enabling another harness requires a proven platform-level read-only boundary and versioned live tests, not just allowlist flags.

A native Codex MCP inventory preflight is a necessary restriction, not definitive proof that managed/plugin-provided tools cannot be introduced later. S1 stays unchecked until the effective tool inventory and unauthorized mutation attempts are verified on installed native harnesses, with before/after filesystem and persisted-state snapshots.
