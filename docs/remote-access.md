# Remote access (PandaCode Connect)

Opt-in. Off by default. Signing in lets a task *run on another computer*: one of the user's
computers signed in to the same account, trusted, with **Allow remote control of this computer**
turned on. The task stays an ordinary task in the local sidebar and conversation pane, with the
same streaming, composer, approvals and model picker as a local one; there is no separate remote
view. Nothing is sent anywhere until the user signs in.

**Full access for remote tasks** is a separate switch in Settings → Remote access,
off for new installs and existing settings. A trusted device cannot start, send to,
steer, approve, answer agent questions, or configure a Full access task until this
computer grants that option locally. It can still stop the task or change its mode
to an approving mode. Remote calls cannot change the option. Omitting a start mode
uses `ask` even when the option is on; enabling it never upgrades a task implicitly.
Native admission checks the current grant, hosting state and pinned caller key
again after asynchronous preparation. Revocation blocks new actions and does not
undo work already admitted.

## Pieces

| Piece | Where | Job |
|---|---|---|
| Relay | `relay/` (Cloudflare Worker + one Durable Object per account) | Sign-in, device list, forwards encrypted frames between an account's devices. Stores no task content. |
| Native | `src-tauri/src/remote/` | Sign-in, keys in the OS keychain, one WebSocket to the relay, host RPC (`host.rs`, `start.rs`), mirrors (`mirror.rs`), client calls and bounded image transfer (`images.rs`). |
| UI | `src/remote*.ts`, `src/remote-settings.css` | Settings → Remote access, and choosing where a new task runs. |
| Mobile | `mobile/` | Opt-in Expo client for tasks, teams, approvals, attachments, changes, pull requests and bots. Uses the configured relay and explicit peer trust. |
| Attachments | `src-tauri/src/attachments.rs` | Task-owned durable image metadata and files, paged through authenticated RPC. |

## Mirrors

A task started on host H is a normal local session row (a *mirror*) with three extra columns
(schema version 26): `remote_device` (H's device id), `remote_session` (H's session id, null
while H is still starting it) and `remote_synced_id` (the last of H's message ids copied here).

- `get_messages` reads local rows. Messages are copied from H after each run end, and when a
  subscription shows H has newer ones.
- `send_message`, `stop_session`, `respond_approval`, `respond_user_input`, `steer_task`,
  `configure_session` (model), `set_reasoning_effort` and `update_session` (mode, so
  permissions change on H) go to H natively, with H's session id.
  A prompt marks the mirror running before H answers (and restores the status if H refuses).
  Up to four images travel through authenticated `image.put` chunks before the prompt;
  the host assigns fresh capture IDs and accepts them only from the sending device's exact key.
- Stopping a mirror whose host is offline (the relay reports it undeliverable, or presence
  shows it offline) ends the local turn as stopped, so the spinner clears. A run end is applied
  once: a mirror that is no longer running or waiting is never ended again.
- The queue stays on this computer: the window delivers the next entry with `send_message` at
  the run end, as for any task, so it reaches H. (Routing the queue to H as well would let both
  windows deliver the same entry.)
- `list_sessions` (and every `Session`) carries `remoteDevice: { id, name }`. The name comes
  from presence ("another computer" until presence names it).
- Running state is the row's status, set from H's events, so the sidebar and composer show it
  as for a local run. A mirror is never reset to interrupted because no local agent runs it.
- Anything that needs the checkout (git, worktree and diff commands, file reads, launching an
  agent) goes through `Store::workspace_root`, which refuses a mirror with
  `This task runs on <name>.` and never touches the local disk.

## Accounts

Sign in with GitHub using the OAuth **device flow** (no client secret anywhere):

1. Desktop: `POST https://github.com/login/device/code` with `client_id`, and the scope is empty (identity only).
2. The user enters the code at `verification_uri`. The desktop polls `https://github.com/login/oauth/access_token`.
3. Desktop → relay `POST /v1/auth/github` `{ githubToken, device: { id, name, platform, publicKey } }`.
   The relay calls `GET https://api.github.com/user` and gets the numeric id, login and avatar_url. It
   registers the device and returns `{ token, account: { id: "gh:<id>", login, avatarUrl } }`.
4. The desktop stores only the relay token in the keychain (`app.pandacode.remote`, with account names scoped to the install's data folder, so a dev build and the installed app never share an item). It discards the GitHub token.

The device key is made once, at sign-in, behind a lock (so two sign-ins cannot race) and before
polling starts. A keychain *error* (a denied prompt, a locked keychain) fails sign-in; only a
missing key is ever generated, so an existing key is never overwritten. `remote_sign_in_cancel`
stops the device-flow poll, and a new `remote_sign_in_start` cancels any earlier flow.

A relay `401` (or close code `4001`) signs this computer out only if the token that request
used is still the current token, so a late refusal of an old token never signs out a newer
sign-in.

If the relay answers `409` (this device id is already registered with a different public key), the
desktop makes a new device id, saves it, and retries once.

Relay token: `base64url(JSON {acct, dev, iat, exp}) + "." + base64url(HMAC-SHA256(SESSION_SECRET))`.
It is valid for 30 days, and only while the device is still registered on the account. Revoking a
device closes its socket and makes its token invalid.

**Refresh.** While connected, the desktop checks the token at connect and every 12 hours. With
fewer than 7 days left it calls `POST /v1/token` and stores the fresh token (same account and
device, later expiry) in the keychain and in memory. A failure is left for the next check; a
`401` follows the sign-out rule above.

**Startup.** Only a computer with an account in `remote.json` reads the keychain. A missing (or
expired) token means signed out. Any other keychain error (a locked keychain, `-25308` right
after an update relaunch) is not a sign-out: status shows signed in but not connected, and the
token is read again after 5 s, 15 s and 60 s; when it reads, the socket starts and
`remote://status` is sent.

**Adopting pre-0.3.20 items.** Before 0.3.20 the keychain accounts were `relay-token` and
`device-key`. At startup, when the scoped token or key is missing, the installed app (data
folder `ai.pandacode.desktop`, never a dev build) reads those once per launch. It adopts them only
if the legacy token parses, is unexpired and its `dev` is this device id: the token is written
to the scoped name, and so is the legacy key when it differs from the scoped one (the relay
registered the legacy key for this device id). After both writes succeed the legacy items are
deleted.

Build config is `option_env!("PANDACODE_RELAY_URL")` and `option_env!("PANDACODE_GITHUB_CLIENT_ID")`.
Environment variables of the same names override them at run time. If either is missing, remote
access is shown as "not configured in this build".

## Relay HTTP API

All routes except auth take `Authorization: Bearer <relay token>`. Errors are `{ error: string }`.

- `POST /v1/auth/github` → `{ token, account }`
- `GET /v1/devices` → `{ devices: [{ id, name, platform, publicKey, host, online, lastSeen }] }`.
  The desktop uses it only for an explicit `remote_devices` (or trust) call made before the
  socket's first presence frame.
- `DELETE /v1/devices/:id` → `{ ok: true }`
- `POST /v1/token` → `{ token }`: a fresh 30-day token for the same account and device
- `GET /v1/connect?host=0|1` (WebSocket upgrade) → joins the account's Durable Object, advertising
  whether this device hosts

## Relay frames (WebSocket text, JSON)

Client → relay: `{ to: deviceId, body: string, ref?: string }`. `ref` is the request id of the
request or response the body carries (events have none). The relay adds `from` and delivers
`{ from, to, body }` to the target device's socket. If it cannot, it answers
`{ kind: "undeliverable", to, ref?, reason: "offline" | "rate" }`. With a `ref` the desktop fails
only that pending call; without one it fails every pending call to `to`.

Relay → client:
- `{ kind: "presence", devices: [{ id, name, platform, publicKey, host, online, lastSeen }] }`, the
  account's full device rows, sent on connect and on every join, leave or device change.
  Each presence frame makes the viewer subscribe again to the hosts it mirrors (a host that
  restarted has forgotten its subscribers).
- `{ kind: "undeliverable", ... }` (above)
- `{ kind: "error", error }`

Client → relay control: `{ kind: "host", enabled: boolean }` changes the advertised host flag
after connecting. `"ping"` gets `"pong"` as a hibernation auto-response.

Presence is the device list: the desktop takes devices, names and keys from it and never fetches
on connect or for an unknown sender. A device missing from presence is removed, and its event
subscription with it.

Close codes: `4000` (replaced by a newer socket) makes the client wait at least 60 s before it
reconnects. `4001` (device revoked) signs this computer out locally. Otherwise the client
reconnects with a backoff that starts at 1 s and doubles to 60 s. The backoff resets to 1 s only
after a socket stayed up for 30 s or more.

The relay uses the hibernation API (`ctx.acceptWebSocket`, tagged with the device id), so idle
sockets cost no duration. Device rows live in the Durable Object's SQLite storage.

## Limits (free)

| Limit | Value | Why |
|---|---|---|
| Devices per account | 10 | Fits personal use. Stops abuse. |
| Sockets per device | 1 | A new socket closes the old one. |
| Frame size | 256 KiB | Message pages and event frames are capped at 180 KiB of JSON. |
| Frame rate per socket | 20/s sustained, burst 100 | Token bucket. Over the limit → `error` and drop. |
| Auth calls | 10/min per IP | Workers rate-limit binding |
| Idle socket | kept (hibernated). The desktop client pings every 25 s. | |

## End-to-end encryption

Each device has an X25519 key pair. The private key stays in the keychain, and the public key is
registered with the device. For a pair of devices:

`key = HKDF-SHA256(ikm = X25519(mine, theirs), salt = sorted(idA, idB).join("|"), info = "pandacode-remote v1")`.

`body = base64(nonce24 || XChaCha20-Poly1305(key, nonce24, plaintext, aad = from + "|" + to))`.
Pair keys are cached by `(deviceId, publicKey)`, so a key change derives a new one.
The relay sees only device ids and ciphertext. Settings shows each device's key fingerprint (first
16 bytes / 128 bits of SHA-256 of the public key, 32 hex characters) so the user can
compare them. Existing device IDs, key pairs and pinned public keys are unchanged;
the longer display requires no re-pairing and is never used as an authentication key.

### Trust (key pinning)

The relay lists the public keys, so it could add a device or swap a key. A host therefore answers
only computers that the user trusted on that host. `remote.json` keeps
`trusted: { deviceId: publicKeyBase64 }`, and the key is pinned when the user trusts the device.

- A device is trusted only while its pinned key equals the key the relay lists now. After a key
  change it is untrusted until the user trusts it again.
Trust is symmetric: `remote_set_trusted` pins a device's key for both directions.

- Host side: a request from an untrusted device gets only the error `This computer isn't trusted
  on <host name>. Trust it in Settings → Remote access on that computer.` Trust is checked before
  the replay check, so untrusted callers never fill the nonce set. Task content and events are
  encrypted only to pinned keys.
- Viewer side: the desktop refuses to call or seal to a host that isn't trusted with its current
  key: `<host name> isn't trusted on this computer yet. Trust it in Settings → Remote access.`
  Responses and events from an untrusted device are dropped.
- Subscribers that are no longer on the device list or no longer trusted are dropped.
- Revoking a device removes its pin.

## App protocol (encrypted plaintext, JSON)

- Request `{ type: "req", id, method, params, ts, nonce }`
- Response `{ type: "res", id, ok: true, result, ts, nonce }` or `{ type: "res", id, ok: false, error, ts, nonce }`
- Event `{ type: "event", name, data, ts, nonce }`

Every encrypted frame carries a Unix-millisecond timestamp and a unique 16–64
character printable nonce. Freshness is limited to ±60 seconds. Admission commits
only a SHA-256 nonce hash and timestamp to the private SQLite replay ledger before
executing work or changing mirrored state. The ledger survives restarts, retains
all live receipts, caps admission at 20,000 entries, and fails closed if storage is
unavailable, truncated or unexpectedly missing. Clock rollback is refused until
the persisted watermark is reached again. Do not delete the ledger while using
Connect. Restoring/deleting all application data as its owner is outside this
anti-replay guarantee.

**Both peers must implement the hardened protocol.** Extended 0.5.0 retains this protocol
when adding the upstream 0.3.47 features. Legacy response/event frames without freshness
metadata are refused. Current trust, the pinned public key and hosting state are
rechecked when queued work starts and after slow preparation steps. Revocation
blocks subsequent admissions and sends; it cannot undo already executed actions.
A relay can still delay, reorder or drop a first delivery. Sign out to disconnect
all Connect traffic; disabling hosting alone leaves the connection active.

A host answers only while **Allow remote control** is on. Otherwise it returns
`{ ok: false, error: "Remote control is off on this computer" }`. It answers only devices that are
trusted on it (see Trust). A host still applies its own approval policy: remote prompts get no
more power than local ones.

| Method | Params | Result |
|---|---|---|
| `subscribe` | `{ sessionIds: [...], epoch?, afterSeq?: { [sessionId]: number } }` (≤ 500 session ids) | `{ epoch, more, tasks: [{ sessionId, status, lastId, title, seq, events, more?, gap? }] }` for those that exist. The viewer drains bounded replay pages using the returned cursor, holds live frames until replay settles, and resynchronizes gaps. Adds the ids to this device's subscription for 10 minutes; the viewer renews every 9 minutes and on every presence frame. An expired subscription starts over. |
| `tasks.list` | `{ limit?: number }` (default 200, maximum 500; 0 lists projects only) | Task/project summaries, pending approval and input payloads, live timing and active team state, bounded to one reply. |
| `projects.icons` | `{ projectIds: [...] }` (1–50) | `{ icons: { [projectId]: dataUrl or null } }`; icons larger than 64 KiB are omitted. |
| `providers.list` | `{ force?: boolean }` | the host's `list_providers` result: `[{ id, name, installed, path, version, models }]`. `force` drops the same ACP catalog caches. If it would not fit one reply, the longest `models` lists are halved until it does. |
| `providers.models` | `{ provider, force?: boolean }` | the host's `list_provider_models` result: `[{ id, name, description, reasoningEfforts, defaultReasoningEffort, contextWindow }]`, its tail dropped to fit one reply. `provider` must be a known harness id (`Unknown provider.`). |
| `task.start` | `{ repo?: "owner/name", projectId?, startId?, branch?, worktree, prompt, provider, model, mode?, effort?, imageIds? }` | `{ sessionId }`; exactly one of `repo` and `projectId` is required. `startId` correlates progress to the initiating attempt. |
| `task.messages` | `{ sessionId, afterId?, beforeId?, limit?, tail? }` | Paged transcript with title, cursor metadata, attachment metadata and turn/coordination details. `tail: true` requests the newest page; legacy `afterId` remains supported. |
| `task.team` | `{ sessionId }` | `{ team }`; compact team/worker/checkpoint/coordination data, or `null` without a team. |
| `task.workerMessages` | `{ sessionId, workerId, afterId?, limit? }` | Worker transcript with ownership checked against its parent team. Maximum 100 messages per request. |
| `task.attachment` | `{ sessionId, id, variant: "thumb" or "full", offset? }` | `{ data, mime, total, offset, done }`; base64 image chunks, only from a saved message owned by that task. |
| `task.workspaceImage` | `{ sessionId, path, variant: "thumb" or "full", offset? }` | The same chunk shape, using the existing workspace image path/format/size checks. |
| `image.put` | `{ uploadId, index, size, data }` | Uploads one base64 chunk; the completed upload returns a host-generated `imageId`. |
| `task.send` | `{ sessionId, text, imageIds? }` | `{ queued: false }` when admitted. A busy host refuses the request; keep it in the viewer's queue or use explicit steering. Anonymous host-side queue entries are not created because they would lose the caller identity needed to recheck revocation and Full access. |
| `task.stop` | `{ sessionId }` | `{}` |
| `task.steer` | `{ sessionId, text }` | `{}` (as `steer_task`) |
| `task.configure` | `{ sessionId, provider?, model?, effort?, mode? }` | `{}`. `provider` and `model` together (a provider not installed there is refused as in `task.start`); `effort: null` clears it; `mode` changes permissions. |
| `approval.respond` | `{ sessionId, requestId, approve: boolean }` | `{}` |
| `input.respond` | `{ sessionId, requestId, response }` | `{}` (as `respond_user_input`) |
| `task.pull` | `{ sessionId }` | `{ pull }` with linked pull request details, or `null`. |
| `task.pullDiff` | `{ sessionId, path? }` | File list when `path` is absent; bounded diff when present. |
| `task.pullMerge` | `{ sessionId, method }` | `{}` after the explicit merge action using `merge`, `squash` or `rebase`; the authenticated peer is rechecked before invoking `gh`. |
| `task.pullDetect` | `{ sessionId }` | Detects and links a pull request mentioned in the latest task turn; current trust is checked again before saving the link. |
| `task.changes` | `{ sessionId }` | Git status, changed files and ahead/behind counts. |
| `task.diff` | `{ sessionId, path?, staged? }` | `{ diff, truncated }`, capped at 120 KiB. |
| `bots.list` | `{}` | Bot summaries and their desktop conversations, with bounded avatars. |
| `bot.thread` | `{ botId }` | `{ sessionId }`; opens or creates that bot's desktop conversation. |
| `bot.message` | `{ botId, text }` | `{ sessionId }`; uses the existing bot brief, memory and approval path. Busy remote bot sends are refused rather than queued without caller provenance. |
| `bot.pause` | `{ botId, paused }` | `{}`; pausing stops running bot work. Resuming a Full access bot requires the host's separate Full access grant. |
| `batch` | `{ calls: [{ method, params }] }` (1–8) | `{ results: [{ ok, result? , error? }] }`; only the explicit read-method allowlist is accepted, with a shared 180 KiB reply budget. Nested or mutating batches are refused. |

A host refuses a session id that is itself a mirror, so tasks never hop twice. Four read
requests may run concurrently per peer, while commands for existing tasks run in one
ordered lane. Batches contain only reads. `task.start` retains Extended's separate lane,
with a global limit of four starts, so a repository clone never blocks Stop or approval
on an already running task. Status and transcript reads also remain responsive during
that clone. PR detection stays ordered because it can change the task's linked pull request.
Parent and worker transcripts use Extended's typed coordination projection: app-origin
scheduler instructions become compact status labels, with their coordination kind retained
when a desktop mirror saves the page. A person's pasted JSON remains an ordinary user message.

### Attachments and durable ownership

Extended schema **28** adds `message_attachments` without changing Extended schema 27's
`github_issue_actions` table. Migration is transactional and idempotent; existing tasks,
messages, issue decisions and mirror columns are retained. New user messages link their
prompt captures to private `attachments/<session-uuid>/<image-uuid>.png` copies, and link
only moodboards belonging to the task's project. Image IDs are never arbitrary file paths.
Both the message and attachment row must belong to the requested session. Deleting a task
cascades its metadata and removes only that task's owned attachment directory. Existing
transcripts without durable attachment rows remain readable; their expired cache images
cannot be reconstructed by the migration.

Uploads accept PNG, JPEG and WebP, up to 5 MiB each and four per prompt. Raw upload chunks
are at most 72 KiB and are sent no faster than every 75 ms. A sender may hold four partial
uploads and 16 unclaimed completed images; both expire after 15 minutes. Claims are
all-or-none, single-use and bound to the sender's current authenticated key. Changing or
revoking that key clears its uploads. The host decodes accepted uploads through the existing
capture pipeline before exposing their IDs.

Attachment downloads use at most 120 KiB of raw bytes per response. Thumbnails fit a
480-pixel long edge and are JPEG; full images are chunked, with oversized sources converted
to a bounded preview. Regular-file checks, explicit byte limits and UUID paths apply to
the persisted files. A workspace image is resolved through `workspace::image_data_url`,
with the same task root restrictions used by desktop rich questions.

For example, a client uploads an image and then uses only the host's returned ID:

```json
{"method":"image.put","params":{"uploadId":"client-upload-1","index":0,"size":2048,"data":"<base64 bytes>"}}
```

```json
{"method":"task.send","params":{"sessionId":"<host-task-id>","text":"Review the attached screenshot.","imageIds":["<host-image-uuid>"]}}
```

These are method/parameter excerpts. The transport adds the encrypted `req` envelope,
fresh timestamp and nonce, and refuses an untrusted current peer key before sending.

### Starting a task on another computer

`task.start` on the host:

1. Refuses a provider that isn't installed there: `<Provider> is not installed on <host>.`
2. For `projectId`, uses that existing host project. For `repo`, finds a project whose `origin` is that GitHub repository (as `github_project_repos`, case
   insensitive). With none, it clones it with `gh` (as `github_clone_repo`, at most 6 minutes)
   into `~/PandaCode/<name>`, or `<name>-2`, `-3`… when taken, and adds the project.
3. Fetches `origin` (at most 90 s), then uses the viewer's branch if origin has it, otherwise
   origin's default branch. With a worktree it creates `pandacode/remote-<id>` from
   `origin/<branch>` the normal way. Without one, a fresh clone is checked out on that branch;
   an existing checkout is never switched: it runs in place only when it is clean, has no local
   agent running in it and is already on that branch (then it is fast-forwarded to origin when
   it can be). Otherwise the start is refused, for example `<host> has uncommitted changes in
   <project>. Start it in a Worktree instead.` (or `… has an agent working in …`, `… has
   <project> on <branch>, not <branch>.`).
4. Creates the task with the provider, model, mode and effort, adds it to the caller's
   subscription, and sends the prompt.

The host's worst case is about 9.5 minutes (clone, fetch and local git steps, each capped); the
viewer waits 3 minutes longer. Optional `task.progress` events report `finding`,
`downloading`, `preparing`, `starting` or `error`, with the caller's opaque `startId` and
clone percentage when available. Progress and task-event batches stay bound to the exact
authenticated key throughout asynchronous work; re-pinning a device ID cannot redirect
an older operation's content to its replacement key.

### Model lists for another computer

A mirror's model picker lists the host's harnesses and models, not this computer's:
`list_providers` and `list_provider_models` take an optional `deviceId` and then ask that host
(`providers.list`, `providers.models`). This computer's own id, or an empty one, lists locally.
The viewer keeps each reply for 10 minutes per device (and per device and provider for models);
`force` asks again, and concurrent identical requests share one call. A device's lists are
dropped when it goes offline, comes back, changes hosting, is untrusted or revoked, and on
sign-out. Every request, cached or not, first gets the usual errors for an unknown, untrusted or
offline (`That computer is offline.`) host.

`remote_start_task` and a mirror's `configure_session` are refused here when the cached lists
show the provider isn't installed there (`<Provider> is not installed on <host>.`) or the model
isn't among its models (`<model> is not available for <provider> on <host>.`; `default` always
passes). With nothing cached the host decides, and it refuses a provider it doesn't have.

### Events

Push only; nothing polls. The host buffers the agent events of each subscriber's tasks and sends
each subscriber at most one `agent.events { events: [...] }` frame every 100 ms (more only when
the events pass the 180 KiB frame budget; an event over 120 KiB is cut). Consecutive text deltas
of the same message are merged, so streaming costs at most 10 frames a second per subscriber,
whatever the token rate. A subscriber more than 4096 events behind stops buffering until it
catches up at the run end. Each event is the host's raw `AgentEvent` (`sessionId`, `kind`, `text`,
`requestId`, `data`, `tool`), approval ids included.

At a run end the host queues `run.finished { sessionId, successful, stopped, text }` after that
run's last events. The viewer copies the new messages, ends the local turn, then emits the
`done` agent event with `text`, so the transcript reloads with everything in it.

The viewer re-emits each event as the window's ordinary `agent-event` with the mirror's *local*
session id (events for tasks it doesn't mirror from that host are dropped), and moves the row's
status (`approval`/`input_request` → waiting, their cancellations → running, any other event
→ running). A subscription reply catches up on what was missed: a status that changed, a run
that ended while this computer wasn't listening, and messages newer than `remote_synced_id`.

### Message copies

`task.messages` returns the messages after `afterId`, oldest first: at most 400 rows, kept
within 180 KiB of JSON, `hasMore` when more remain. Any message over 120 KiB is cut and ends with
`…[cut on the host to fit a remote page]` (JSON tool content is cut inside, so it still parses).
The viewer stores a page and advances `remote_synced_id` in one transaction, and skips a page
that no longer starts at the stored position (a concurrent copy got there first), so messages
are never copied twice. It follows `hasMore` for up to 50 pages per copy, and also takes the
host's title.

Known limit: a message the host rewrites in place after it was copied (a tool's status, say)
is not copied again.

Responses and events are handled as soon as they arrive and never wait behind a slow request.
Requests from one peer are answered in order, except `task.start` (above).

## Tauri commands (UI contract)

| Command | Args | Returns |
|---|---|---|
| `remote_status` | | `{ configured, signedIn, account?: { login, avatarUrl }, device: { id, name, fingerprint }, hostEnabled, allowFullAccess, connected }` |
| `remote_sign_in_start` | | `{ userCode, verificationUri, expiresIn }` |
| `remote_sign_in_finish` | | status (waits for the device flow, error on expiry, denial or cancel) |
| `remote_sign_in_cancel` | | status. Stops the device-flow poll. |
| `remote_sign_out` | | status |
| `remote_set_host_enabled` | `{ enabled }` | status |
| `remote_set_full_access` | `{ enabled }` | status. Local settings IPC only; not a remote RPC method. |
| `remote_devices` | | `[{ id, name, platform, fingerprint, host, online, lastSeen, current, trusted }]`, from presence (fetched only before the first presence frame) |
| `remote_set_trusted` | `{ deviceId, trusted }` | the `remote_devices` list. Pins the device's current key for both directions, or removes the pin. |
| `remote_revoke_device` | `{ deviceId }` | `{}` |
| `remote_targets` | | `[{ deviceId, name, platform, online }]`: other devices trusted here (so, symmetrically, trusting this one) with hosting on |
| `remote_repo_check` | `{ projectId }` | `{ repo: "owner/name" \| null, branch, ahead, dirty, pushed }`. `repo` is null without a GitHub `origin`; `ahead` is `rev-list --count @{u}..HEAD` (0 without an upstream); `pushed` is whether `origin/<branch>` exists. Local refs only, no network. |
| `list_providers` | `{ force?, deviceId? }` | as without `deviceId`, from that host (see Model lists) |
| `list_provider_models` | `{ provider, force?, deviceId? }` | as without `deviceId`, from that host |
| `remote_start_task` | `{ deviceId, projectId, prompt, provider, model, mode?, effort?, worktree?, branch?, sessionId? }` | the mirror `Session` plus `sessionId`. `branch` defaults to the checkout's. An empty New task draft without its own worktree passed as `sessionId` becomes the mirror; otherwise a new task is made. On failure nothing is left: a new row is deleted, a draft stays a draft. |

Window events: `remote://status` (status), `remote://presence` (devices), and for mirrors the
ordinary `agent-event` and `sessions-changed`.

The agent-event hook in `lib.rs` clones and allocates nothing while hosting is off or nobody
mirrors that task.

## Privacy

Listed in the README privacy table: remote access contacts GitHub (sign-in only) and the relay
(while signed in). Task content crosses the relay only encrypted end to end.
