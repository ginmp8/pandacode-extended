# Extended-only bot context and upstream integration contract

This document describes the Extended-owned bot context features relative to `aicodeking/pandacode:main` (mirrored via `ginmp8/pandacode:main`), including [issue #121's per-channel history settings](https://github.com/ginmp8/pandacode-extended/issues/121). Fresh native context remains opt-in; channel history defaults to inclusion unless explicitly disabled. The origin's current and future bot functionality must be preserved when the branches synchronize.

## Features

| Extended feature | Scope | Default | Origin behavior to preserve |
| --- | --- | --- | --- |
| `bot_message_bot(to, text, freshContext?: boolean)` | One delegated assignment | `false` | Existing desktop-thread routing, result messages, three-hop limit, durable queue |
| **Fresh context each message** in **Bot Settings → Profile → Brain** | Each new bot-owned desktop, external, scheduled or delegated message | Off | Provider resume, visible transcript, permissions, queues, skills, memories, schedules, failover and approvals |
| **Include channel history by default** in **Bot Settings → Shared Channels** | Automatic history for new local shared-channel assignments to one recipient bot, unless overridden for that channel | On | Upstream's latest eight posts/replies, per-post truncation, routing, history visibility and delivery tracking |
| **Per-channel history** in **Bot Settings → Shared Channels** | `Use default`, `Include` or `Exclude` for each active local shared channel the bot has joined | Use default | Same context window, clipping, delivery pipeline and stored transcript |

A delegated `freshContext: true` creates a *separate* conversation. The profile preference instead starts a new native harness context for each eligible message **inside the existing conversation**. These features are independent. `freshContext: false` preserves recipient-thread routing but does not override a recipient bot's own profile preference. Neither feature changes an ordinary project task linked to a bot or the Architect's worker sessions.

### Independent context sources

A fresh **native harness context** does not imply that the incoming request contains no prior context. The origin's local shared channels explicitly append up to eight earlier posts/replies to each new assignment, and bot memories remain available in the brief. These are three separate sources of information.

The **Shared Channels** section has a bot-wide default (existing saved on/off values are retained) and a selector for every active local shared channel the bot belongs to. **Use default** inherits the bot-wide preference, **Include** always injects recent channel history, and **Exclude** omits it even if the bot-wide default is on. New bots and unsaved preferences default to include. These options affect only assignments composed after they are saved; already-queued messages retain their originally composed payload, and active work is uninterrupted. Channel history remains visible and stored, and other bots keep their own settings. Native harness history, fresh native context, saved memories, external Connections, direct bot messaging and routing are not affected. Omitting automatic context does **not** isolate the underlying workspace, memories or information that another participant explicitly includes in a new message.

### Mechanism and safety

- `extended_bot_context` is an additive, bot-ID-keyed settings table created by the existing bot upgrade hook; the upstream `Bot`, `BotInput` and queue schemas do not change. A missing record means fresh context is disabled.
- `extended_bot_channel_history` is the existing additive bot-ID-keyed preference table; a missing row means **include shared-channel history**, matching upstream. The per-bot setting and saved values survive the additive upgrade without changes to the upstream `Bot`, `BotInput` or queue schema.
- `extended_bot_channel_history_overrides` stores only `include` / `exclude` exceptions by `(bot_id, channel_id)`, with foreign keys to the bot and local shared channel. Membership and active channel state are checked before reads/writes; removing a member or archiving a channel clears its overrides. Restoring or rejoining starts from the bot default. The upgrade is additive and idempotent.
- **Fresh context** stays under Profile → Brain; **Shared Channels** owns the existing bot-wide history switch and the per-channel selectors. Only the main app window can change these preferences using guarded `bot_set_fresh_context`, `bot_set_channel_history` and `bot_set_shared_channel_history_override` commands. A bot cannot change them through a bot tool.
- The gateway attaches current instructions, memories, personality, goals, skill references and channel etiquette on each new message when enabled.
- After native task admission rejects live runs, the Extended hook clears only the provider resume reference and context usage, suppressing transcript replay for that bot session. It never deletes saved messages, pending approvals, scheduled jobs, queue entries or workspace files.
- Incoming messages while a run is active go through the existing durable queue instead of steering into the live context. The next queued turn starts fresh. A request queued before the preference changed receives the latest brief at launch without rewriting its stored payload.
- Turning the option on never stops the current turn. Once off, the regular origin native resume and context recovery behavior is restored. The setting survives ordinary bot reset and is removed when its bot is deleted.
- A fresh turn cannot automatically refer to the previous exchange: carry essential facts in the new prompt or save them as bot memory. The workspace remains shared even when the native contexts differ.

### Upstream sync contract

1. Compare the current original implementation of bot profile/settings, tool schemas, delegation, conversation routing, admission, context recovery, native providers, queues, scheduler, permissions and failover before applying changes.
2. Import **all** origin functionality, then carry forward the smallest Extended-only hooks. When origin has no equivalent feature, preserve both Extended preferences and their per-bot stored values; do not remove any upstream capability. In particular, default shared-channel history must stay intact.
3. If origin ships an equivalent setting or `bot_message_bot` option, prefer the origin code **after** behavior and data migration tests verify compatibility. Preserve existing Extended preferences and external `freshContext` input or provide an explicit adapter. Do not run duplicate history resets, briefs or message dispatches.
4. Re-run off-by-default behavior, per-bot isolation, active-run/queue/approval safety, launch recovery, scheduler, bot collaboration, native context reset, text history retention, and permission tests. Confirm browser toggle and tooltip. Treat mismatches as regressions requiring diagnosis, not permission to silently discard origin features.
5. Record any divergence by path/hunk, reason, impacted upstream feature and passing proof. Do not bind synchronization to an old version number.

Future upstream compatibility is a verification requirement, not an automatic guarantee.

## Protected integration points

- `src-tauri/src/bots/extended.rs`: Extended-owned policies, additive persistence, guarded IPC and tests
- `src-tauri/src/bots/shared_channels.rs`: per-recipient effective policy at assignment-prompt construction; membership/archive cleanup without changing routing, tracking or history
- `src-tauri/src/bots.rs`: minimal upgrade/module/detail hooks, otherwise upstream profile untouched
- `src-tauri/src/bots/gateway.rs`: conditional brief and queue-before-steer
- `src-tauri/src/context_recovery.rs`: history-replay exclusion only for explicitly opted-in bot sessions
- `src-tauri/src/lib.rs`: fresh native context after admission and before context recovery
- `src/bots.ts`, `src/preview-bots.ts`, `tests/bots.spec.ts`: settings, tooltip, preview and browser regression

## Verification

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib bots::extended::tests
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib bots::gateway::tests
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build && npm test
npx playwright test tests/bots.spec.ts
```

Keep test thresholds intact; a failing unrelated test must be reported rather than silently weakened.
