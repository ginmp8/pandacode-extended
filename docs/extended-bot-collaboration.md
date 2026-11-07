# Extended: explicit bot collaboration across local Shared Channels

Issue: [#122](https://github.com/ginmp8/pandacode-extended/issues/122). This feature is implemented only on `extended/development`, without changing upstream bot profiles or ordinary direct-message semantics.

## Tools and routing

A bot can use these tools only in a bot-owned conversation, subject to its existing model, workspace, permission policy and channel membership:

| Tool | Arguments | Effect |
| --- | --- | --- |
| `pandacode_bot_post_channel` | `channel`, `text`, optional `idempotencyKey` | Append an attributed informational post to one existing local Shared Channel. **Never** assign work; even `@mentions` are inert text. |
| `pandacode_bot_request_channel` | `channel`, `recipients` (one or two exact bot names or IDs), `text`, optional `idempotencyKey` | Create a tracked, attributed request and assign it to the specified other members through the existing gateway/queue. Send each terminal result back to the requester, not to unrelated channels. |
| `pandacode_bot_message_bot` | `to`, `text`, optional `freshContext` | Preserve the original private direct delegation. A direct request made during channel work appears in the channel **only as a status marker**, without sharing its request or response text. |

The channel parameter accepts an exact channel ID or unique name, optionally beginning with `#`. The sending bot must be a current member of an active channel. Assignments require explicit, distinct members; omitting recipients does **not** fall back to the lead. Self-assignment, unknown or ambiguous names, non-members, repeat visits to a bot in a collaboration route, and more than three combined direct/channel hops are rejected. A bot may participate in multiple channels without copying messages between them.

Example informational post:

```json
{"channel":"#release","text":"The build passed. @Reviewer is informational only.","idempotencyKey":"release-update-41"}
```

Example work request:

```json
{"channel":"#release","recipients":["Reviewer","Writer"],"text":"Review the release notes and report findings.","idempotencyKey":"release-review-41"}
```

The requesting bot's existing conversation receives `Update from <recipient> in #<channel> (request <ID>): ...` when each assignment finishes, stops or fails. The result is queued by the existing gateway if the requester is busy. The channel timeline shows who asked, who received work and each delivery's status; the private callback content is not broadcast. The destination bot receives the selected channel's recent history only if the existing **Bot Settings → Shared Channels** preference permits it. No other channel's history is implicitly forwarded.

## Storage, retries and recovery

All collaboration-specific data lives in additive Extended tables:

- `extended_bot_channel_actions`: original channel message ID, actor and origin thread, target recipients, correlation parent, route/depth, timestamp, and idempotency identity.
- `extended_bot_direct_routes`: ancestry for direct delegations and optional source-channel delivery ID, for status-only channel timeline markers.
- `extended_bot_channel_results`: delivery correlation, completion text, and callback attempt state.

Existing `bot_shared_channel_messages` and `bot_shared_channel_deliveries` remain the source of truth for the channel timeline, work queue, stop/reassign and delivery status. In the upstream schema, an Extended informational post is stored as an attributed `reply` without deliveries, and an Extended channel request is stored as an attributed `user` message with deliveries. A side-table join exposes the distinct `bot_post` and `bot_request` kinds to the frontend. Upstream history reading and the optional per-recipient history policy continue to work.

A repeated invocation with the same `idempotencyKey` and identical arguments returns the existing post/request ID without sending work again; reusing the key with different arguments fails. Without an explicit key, an identity derived from the current source turn and canonical payload prevents identical replay within that turn. **Use an explicit key for retries**, particularly after uncertain tool responses. Repeating an identical request in the same turn without a different key does not create a second assignment.

Each recipient has an independent durable delivery and depth. A delivery progresses queued → in progress → completed/failed/stopped; app startup reconciliation marks interrupted work as failed and attempts the missing source callback without rerunning recipient work. A callback is claimed before gateway dispatch to avoid duplicate work after an uncertain crash. If the app closes between claiming and persisting that callback, the callback may remain incomplete; the `failed`/ `sending` attempt stays recorded and the recipient is **not** automatically rerun. This is an at-most-once callback attempt, not a claim of exactly-once external delivery.

## Safety and compatibility

Bot instructions and permissions remain separate from collaboration tooling. Only bot-owned tool sessions can use it. The existing gateway, approvals, queue, pause, stop, scheduler, direct messages and per-channel history settings remain authoritative. Archived channels cannot accept new posts or requests. Removing a channel member prevents new assignments to or from that member. Stored message text is data, not permission to override the recipient's instructions.

This is an Extended-only implementation; on later upstream updates prefer behavior-compatible upstream code while preserving stored Extended rows and migration tests.

## Verification

Run the following from a checkout of `extended/development`:

```sh
npm ci
npm run build
npm test
npx playwright test tests/bots.spec.ts tests/bots-channels.spec.ts
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib bots::
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

The Rust unit tests cover inert informational posts and mention text, per-channel isolation, idempotency replay/rejection, mandatory/unique recipients, ambiguous names, self-targets, cycles and the combined three-hop budget. The browser test covers the instructions tooltip and collapsible examples. End-to-end bot-to-bot callbacks and stop/restart recovery should also be verified in a desktop build with a disposable local database before release.
