# Council S1: native reviewer authority and isolation

## Compatibility

The existing Architect Council, its score parser, rounds, native automatic reviewer selection, saved WorkerProfile schema and ordinary bot/team workflows are unchanged in purpose. S1 adds a frozen per-seat reviewer binding, not a new Council engine or bot type. Legacy Council seats without the new field remain readable and receive the same restrictive runtime policy.

ReviewerBinding freezes a cycle ID, optional delivery ID, round, seat ID, optional reviewer bot ID, native artifact revision token, provider, model, lens and tool policy. Native artifact revision identifies the source state at dispatch; it is not a source hash, verified check or mandatory evidence gate. ReviewerState and ReviewerFailureCode are typed internal contracts for later integrations.

## Security boundary

The reviewer identity is resolved from admitted Architect state, never from a prompt or a bot profile. Every Council reviewer turn runs with plan/read-only permissions even if the parent session has Full access. This does not mutate the persisted bot profile or ordinary session mode.

- Codex receives the native read-only sandbox, network denied, no elevation and process-local empty MCP configuration.
- Claude, GLM and MiMo receive a native CLI allowlist (Read, Glob, Grep), plan mode, disabled hooks and strict empty MCP configuration.
- No PandaCode host tools are registered for Council reviewers: bot profiles/memory, schedules, approvals, private messages, channels, settings, skills, Connections, deployments and team management tools cannot be called through the app's tool transport.
- A native permission escalation request is rejected without a user-overridable approval.
- Named Connections cannot inject reviewer-specific arguments, environments, MCP or secrets. ACP, other unsupported harnesses and cloud execution fail with read_only_unsupported rather than falling back to a writable execution context.

This is an implementation boundary, not proof that all CLI versions have no undeclared side effects. AC-03 and AC-18 require actual native-harness mutation attempts and filesystem/database comparisons, not just unit tests or serialized permission flags.

## Baseline and required verification

Source baseline: extended/development at d0100f72c25e3ccff58c294d624505af8a216b48. Before this implementation, GitHub CI showed eight passing checks and one failing Bot manifest and collaboration UI Playwright check.

Commands: cargo test --locked --manifest-path src-tauri/Cargo.toml --lib; cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings; npm ci; npm run build; npm test; and relevant Council/Architect/bots Playwright regressions in issue #126.

Desktop/harness proof remains required: configure a normally full-access bot reviewer, attempt actual shell, script, file, MCP, bot-profile/memory, schedule, approval, message, skill and Connection mutations, capture redacted before/after tracked and untracked filesystem hashes and SQLite states, exact CLI version/model and explicit permission denials. Unsupported harnesses are blocked/not tested. No simulated protocol test is a replacement for those experiments.

Do not check S1 complete until AC-03 and AC-18 have the required live evidence.
