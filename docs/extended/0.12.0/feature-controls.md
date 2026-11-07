# Extended 0.12.0 — feature-control continuity

**Status:** The 0.11.0 control surface is inherited. Issues #121–#123 add source functionality on `extended/development`, with native acceptance still pending.

Start from the [0.11.0 feature-controls record](../0.11.0/feature-controls.md) and the [full upstream parity contract](../upstream/parity-contract.md). Preserve existing Extended controls for automatic updates, Connect/remote access, paused bots, external MCP, cloud credentials and cloud/VM operations. Keep upstream code paths functional when the corresponding feature is enabled and authorized.

The 0.11.0 cycle also introduced **opt-in per-bot fresh-context behavior** and an **independent shared-channel history preference**. These are inherited starting functionality, not new 0.12.0 work. Ensure they remain separate, do not duplicate delegated context and do not silently reset queues.

## Extended-only 0.12.0 feature boundaries

| Issue | Surface and effective setting | Scope and limitation |
| --- | --- | --- |
| [#121](https://github.com/ginmp8/pandacode-extended/issues/121) | **Bot Settings → Shared Channels**: the bot-wide include-history default is **on** when unset; per-channel **Use default** inherits it, and **Include** or **Exclude** overrides it. | Affects automatic injection of up to eight recent channel posts/replies into **new assignments**; does not delete transcripts, change native context/memory or rewrite queued/active work. |
| [#122](https://github.com/ginmp8/pandacode-extended/issues/122) | Bot tools plus **Profile → Instructions → Communication examples**. No new global communication toggle or per-channel functional bot role. | Explicit local-channel post/request only; one/two named recipients for requests, member validation, no broadcast, and one shared three-hop budget across direct/channel work. |
| [#123](https://github.com/ginmp8/pandacode-extended/issues/123) | **Bots → + → Import / Sync bots… / Export bots…** and headless flags. Legacy `{"bots":[...]}` remains valid; added Extended settings are optional. | Every mutating import requires a verified local configuration backup. GitHub sync is manual, pinned to SHA and read-only to GitHub; `autoSync: true` is unsupported. Secrets, messages, memories and running state are excluded from exports. |

See the [context guide](../../extended-bot-context.md), [collaboration guide](../../extended-bot-collaboration.md) and [manifest guide](../../extended-bot-manifests.md) for examples and exact behavior.

## Incremental verification

- [ ] Pin the fork `main` and Extended `base/development` SHAs before each import, inspect every customized shared source hunk, and record justified exceptions.
- [ ] Verify **On** and **Off** states, persistence/restart behavior, disabled vs explicitly authorized operations and effective permissions.
- [ ] Regress bot harness selections, bot chat access levels, fresh-context and channel-history preferences, delegation, queue delivery and error visibility.
- [ ] Verify #121 override precedence and effective UI status after upgrade/restart, including member removal and channel archive; queued and active work must be unchanged.
- [ ] Verify #122 explicit recipient/channel boundaries, idempotent requests, loop and three-hop rejection, stop/restart states, and correlated callbacks without broadcast.
- [ ] Verify #123 portable settings/channel round-trip, backup failure-before-mutation, no-secret export, existing/partial import behavior and manual GitHub source validation in Desktop and headless modes.
- [ ] Regress account-pool sign-in/usage with real authorized provider accounts, Connect/MCP/cloud/VM and credential approvals.
- [ ] Validate that controls do not hide, delete or stub upstream features.
- [ ] Track runtime and installed-app evidence separately from static file parity or CI.

No 0.12.0-specific installed-app feature-control acceptance is established by the current source and documentation changes; the checks above remain open.
