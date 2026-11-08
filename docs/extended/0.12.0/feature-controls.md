# Extended 0.12.0 — feature-control continuity

**Status:** Inherited control surface at the 0.11.0 baseline. No new 0.12.0 control or native acceptance is claimed.

Start from the [0.11.0 feature-controls record](../0.11.0/feature-controls.md) and the [full upstream parity contract](../upstream/parity-contract.md). Preserve existing Extended controls for automatic updates, Connect/remote access, paused bots, external MCP, cloud credentials and cloud/VM operations. Keep upstream code paths functional when the corresponding feature is enabled and authorized.

The 0.11.0 cycle also introduced **opt-in per-bot fresh-context behavior** and an **independent shared-channel history preference**. These are inherited starting functionality, not new 0.12.0 work. Ensure they remain separate, do not duplicate delegated context and do not silently reset queues.

## Incremental verification

- [ ] Pin the fork `main` and Extended `base/development` SHAs before each import, inspect every customized shared source hunk, and record justified exceptions.
- [ ] Verify **On** and **Off** states, persistence/restart behavior, disabled vs explicitly authorized operations and effective permissions.
- [ ] Regress bot harness selections, bot chat access levels, fresh-context and channel-history preferences, delegation, queue delivery and error visibility.
- [ ] Regress account-pool sign-in/usage with real authorized provider accounts, Connect/MCP/cloud/VM and credential approvals.
- [ ] Validate that controls do not hide, delete or stub upstream features.
- [ ] Track runtime and installed-app evidence separately from static file parity or CI.

No 0.12.0-specific feature-control acceptance was established at cycle opening.

## Inherited upstream 0.3.102 model-selection behavior

The [upstream ZCode model-selection fix](../../../releases/0.3.102.md) is integrated into the Extended 0.12.0 source candidate. ZCode cannot join or lead an agent team, so automatic team catalogs exclude it, and selecting a ZCode model ends a team with no worker history. If workers have already run, the user must start a new task to use ZCode. This is a known upstream harness limitation, not an Extended permission toggle or removal of the ZCode harness. The imported tests have not been run locally; verify real harness transitions and other Extended controls separately.
