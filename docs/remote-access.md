# Remote access (PandaCode Connect)

Opt-in. Off by default. Signing in lets a task *run on another computer*: one of the user's
computers signed in to the same GitHub account, with **Allow remote control of this computer**
turned on. The task stays an ordinary task in the local sidebar and conversation pane, with the
same streaming, composer, approvals and model picker as a local one; there is no separate remote
view. Nothing is sent anywhere until the user signs in. A computer can also be an always-on
server with no screen or desktop app: `pandacode serve` ([headless](headless.md)).

**Full access for remote tasks** is a separate switch in Settings → Remote access,
off for new installs and existing settings. Starting tasks requires a Full device
grant; calling `bot.message` requires both a Full device grant and this local switch.
Setting a task to `full` or `review` also requires both. A Control device may still
configure ordinary model/effort settings and nonprivileged modes, stop tasks, and
steer ordinary running tasks; bot-owned conversation rules are listed with the RPCs
below. Remote calls cannot change the local option. Omitting a start mode uses
`ask` even when the option is on; enabling it never upgrades a task implicitly.
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
  as for a local run. A mirror interrupted by this computer's restart takes H's status at the
  next subscription (and its messages are copied). A run end always copies the messages, even
  when the turn already ended here (Stop, an offline host).
- A subscribed task that H says no longer exists (`missing`) ends a running mirror as failed
  (`This task is no longer on <host>.`). A row left without `remote_session` by a start this
  computer quit during is a local draft again at the next launch.
- Approval and question cards follow H's `pending` list at each subscription: a card missing
  here (after a restart or a lost frame) is shown again, and one answered elsewhere is cleared.
- Anything that needs the checkout (git, worktree and diff commands, file reads, launching an
  agent) goes through `Store::workspace_root`, which refuses a mirror with
  `This task runs on <name>.` and never touches the local disk.

## Accounts

Sign in with GitHub using the OAuth **device flow** (no client secret anywhere):

1. Desktop: `POST https://github.com/login/device/code` with `client_id`, and the scope is empty (identity only).
2. The user enters the code at `verification_uri`. The desktop polls `https://github.com/login/oauth/access_token`.
3. Desktop → relay `POST /v1/auth/github` `{ githubToken, device: { id, name, platform, publicKey } }`.
   The relay checks the token against its own OAuth app (`POST /applications/{client_id}/token`, so a
   token issued to any other app is refused) and gets the numeric id, login and avatar_url. It
   registers the device and returns `{ token, account: { id: "gh:<id>", login, avatarUrl } }`.
4. The desktop stores only the relay token, in `connect-secrets.json`, and discards the GitHub token.

**Where secrets live.** The relay token and the base64 X25519 private key are one JSON file,
`<app data dir>/connect-secrets.json` (`{ token?, key? }`), mode 0600 on unix, written
atomically (temp file in the same folder, fsync, rename). Each install's data folder is its own
scope, so a dev build and the installed app never share secrets. A missing, unreadable or
corrupt file means none (signed out). Connect never uses the OS keychain: macOS pins each item
to the exact code hash of the build that made it, and PandaCode has no Apple Team ID, so every
update asked for the login password on every read. Other app secrets moved the same way, to
`secrets.json` (see `secret_store.rs`). Nothing is migrated from the keychain for Connect
(reading it is what prompted); users from before this change sign in once more, and the `409`
path below gives them a new device id.

The device key is made once, at sign-in, behind a lock (so two sign-ins cannot race) and before
polling starts. Only a missing (or undecodable) key is ever generated, so an existing key is
never overwritten; a failed write fails sign-in. `remote_sign_in_cancel`
stops the device-flow poll, and a new `remote_sign_in_start` cancels any earlier flow.

A relay `401` (or close code `4001`) signs this computer out only if the token that request
used is still the current token, so a late refusal of an old token never signs out a newer
sign-in.

If the relay answers `409` (this device id is already registered with a different public key), the
desktop makes a new device id, saves it, and retries once.

Relay token: `base64url(JSON {acct, dev, iat, exp, gen}) + "." + base64url(HMAC-SHA256(SESSION_SECRET))`.
It is valid for 30 days, and only while the device is still registered on the account with the same
`gen` (a random value set when the device row is created; older tokens without `gen` fall back to
`iat >= registered_at`, and a refresh adds it). Revoking a device closes its socket and makes its
tokens invalid, even after a re-registration in the same second. The relay refuses to run with a
`SESSION_SECRET` shorter than 32 characters.

**Refresh.** While connected, the desktop checks the token at connect and every 12 hours. With
fewer than 7 days left it calls `POST /v1/token` and stores the fresh token (same account and
device, later expiry) in the secrets file and in memory. A failure is left for the next check; a
`401` follows the sign-out rule above.

**Startup.** Only a computer with an account in `remote.json` reads the secrets file. A
missing (or expired) token means signed out. Nothing prompts and nothing touches the network
unless signed in.

Build config is `option_env!("PANDACODE_RELAY_URL")` and `option_env!("PANDACODE_GITHUB_CLIENT_ID")`.
Environment variables of the same names override them at run time. If either is missing, remote
access is shown as "not configured in this build".

## Relay HTTP API

All routes except auth take `Authorization: Bearer <relay token>`. Errors are `{ error: string }`.

- `POST /v1/auth/github` → `{ token, account }`
- `GET /v1/devices` → `{ devices: [{ id, name, platform, publicKey, host, online, lastSeen }] }`.
  The desktop uses it only for an explicit `remote_devices` call made before the
  socket's first presence frame.
- `DELETE /v1/devices/:id` → `{ ok: true }`
- `DELETE /v1/account` → `{ ok: true }`: deletes all relay state for the account and closes its
  sockets with `4001 (signed out)`. Since tokens are accepted only for registered devices, all
  existing tokens become invalid; signing in again creates a fresh relay account.
- `POST /v1/token` → `{ token }`: a fresh 30-day token for the same account and device
- `POST /v1/push/devices` → `{ ok: true }`: registers a push device token for this device
  (`{ platform: "ios" | "android", token }`; up to 3 per platform). A revocation or account
  deletion drops the device's tokens with it.
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
  restarted has forgotten its subscribers). Frames within 250 ms share one round, except the
  socket's first presence, whose round starts at once: with it, the desktop asks the afterId
  page of up to 3 mirrors with messages copied (running ones first) right behind `subscribe`,
  and the phone does the same for its open threads, so a reconnect catches up in one round trip
  after the upgrade instead of two. One round
  runs at a time (a call during it runs one more after it), every host is asked at once, and a
  round in which a host with a running mirror failed is retried after 2 s, 4 s… up to 60 s.
- `{ kind: "undeliverable", ... }` (above)
- `{ kind: "error", error }`

Client → relay control: `{ kind: "host", enabled: boolean }` changes the advertised host flag
after connecting. A host can also send `{ kind: "notify", body: { kind: "waiting" | "finished",
count } }`: the object coalesces these (30 s window, halved when the waiting count grows) and
pushes a generic notice — counts and a deep link, never task text — to every push token the
account's phones have registered via `POST /v1/push/devices`. `"ping"` gets `"pong"` as a
hibernation auto-response.

Presence is the device list: the desktop takes devices, names and keys from it and never fetches
on connect or for an unknown sender. A device missing from presence is removed, and its event
subscription with it.

Close codes: `4000` (replaced by a newer socket) makes the client wait at least 60 s before it
reconnects. `4001` (device revoked) signs this computer out locally. Otherwise the client
reconnects at once, then after 1 s, doubling to 60 s, each wait spread over 75–125 %. The
backoff starts over only after a socket stayed up for 30 s or more, or after a wake. The phone
(`mobile/src/store/backoff.ts`) follows the same schedule.

The desktop keeps one rustls client config for every connect, so a reconnect resumes the TLS
session: no certificate chain is sent or checked again (bytes and CPU, not a round trip). TLS
1.3 0-RTT early data is not used. The upgrade request carries the bearer token, and a replay
of it would let anyone on the path replace the device's socket at the relay (close `4000`, which
then waits 60 s), so a replayable upgrade is not safe; the relay would also need Cloudflare's
0-RTT zone setting.

The client pings every 25 s and treats the socket as dead when nothing arrives within 10 s of a
ping, or when a frame can't be written within 10 s. Every 2 s it compares the wall clock with
the monotonic one (which, like tokio's timers, stands still while the computer sleeps); a jump
of more than 10 s means it slept, and it reconnects at once. At most 512 frames wait for the
socket; past that a send fails at once (`Too much is waiting to go to the relay.`).

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

Each device has an X25519 key pair. The private key stays in `connect-secrets.json`, and the public key is
registered with the device. For a pair of devices:

`key = HKDF-SHA256(ikm = X25519(mine, theirs), salt = sorted(idA, idB).join("|"), info = "pandacode-remote v1")`.

`body = base64(nonce24 || XChaCha20-Poly1305(key, nonce24, plaintext, aad = from + "|" + to))`.
Pair keys are cached by `(deviceId, publicKey)`, so a key change derives a new one.
The relay sees only device ids and ciphertext. Settings shows each device's key fingerprint (first
16 bytes / 128 bits of SHA-256 of the public key, 32 hex characters) so the user can
compare them. Existing device IDs, key pairs and pinned public keys are unchanged;
the longer display requires no re-pairing and is never used as an authentication key.

### Account access

Devices signed in to the same GitHub account can control each other while **Allow remote control**
is on. The host accepts requests only from devices listed in the account's current presence, and
still checks the timestamp and nonce for replay. The viewer opens responses and events only from
devices in presence, using each device's current public key to derive the encrypted pair key.
Subscribers are dropped when their device leaves presence or their subscription expires.

There is no per-device key pinning. The relay could add a device to the account, and that device
could then control a host while remote control is enabled. This is the trade-off of removing
pinning. Fingerprints remain visible in Settings as identifying information; they do not gate
access.

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

Every sealed message carries `ts` and `nonce`. The viewer (desktop and phone) applies the same
window and seen-nonce check to responses and events and drops a stale, repeated or unstamped one;
a dropped response fails its call at once with `That computer's reply was stale or repeated…`.
A viewer therefore needs a host on a version that stamps replies.

**Packed frames.** Requests from current viewers carry `z: 1`: the sender opens packed frames.
The host remembers this per device (in memory) from each fresh request (a stale or repeated one
changes nothing) and forgets a device that leaves presence; an older app's requests lack it and
turn packing off again. A reply is packed by its own request's `z`, never by what the host
remembers when it is sent. To such a peer, a reply or event whose JSON plaintext is
over 1 KB is sealed as `"z"` + raw deflate (level 6) when that is smaller. JSON plaintexts
start with `{`, so `open` tells the two apart. Inflated output is capped at 2 MB (the largest
real frame, a 1 MiB messages page, is about 1.05 MB). Requests are never packed; a host drops a
packed request. The relay and direct links carry the same sealed body. A real 178 KB
messages page deflates to 16 % (29 KB).

A host answers requests from other devices listed in its account presence only while **Allow
remote control** is on. Otherwise it returns
`{ ok: false, error: "Remote control is off on this computer" }`. A host still applies its own
approval policy: remote prompts get no more power than local ones. Access (remote control on, the
sender in presence, replay) is checked again when a queued request runs, not only when it arrives.

**Full access for remote tasks** (`remote_full_access` in `remote.json`, off by default): while
off, `task.start` without a mode starts in `ask`; an explicit `full` in `task.start` or
`task.configure`, and any `task.send`, `task.steer` or mode-less `task.configure` on a task whose
stored mode is `full`, are refused with `Full access is off for remote tasks on <host>.` This
prevents accidents; it is not a boundary against a device that can also answer approvals.

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
| `task.send` | `{ sessionId, text, imageIds? }` | Rejects bot-owned conversation sessions at every peer scope; send through `bot.message`, which requires a Full device grant and the host's local Full-access switch. `{ queued: false }` when admitted. A busy host refuses the request; keep it in the viewer's queue or use explicit steering. Anonymous host-side queue entries are not created because they would lose the caller identity needed to recheck revocation and Full access. |
| `task.stop` | `{ sessionId }` | `{}` |
| `task.steer` | `{ sessionId, text }` | Ordinary running tasks use Control authorization (subject to the task's current mode). Steering a bot-owned conversation requires a Full device grant and the host's local Full-access switch. Returns `{ "steered": true, "id": "...", "mode": "..." }` (as `steer_task`). |
| `task.configure` | `{ sessionId, provider?, model?, effort?, mode? }` | `{}`. Control may configure provider/model and effort or select a nonprivileged mode. Setting mode to `full` or `review` requires a Full device grant and the host's local Full-access switch. `provider` and `model` must be supplied together (a provider not installed there is refused as in `task.start`); `effort: null` clears it; `mode` changes permissions. |
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
| `bot.message` | `{ botId, text }` | Full access only; returns `{ sessionId }` and uses the existing bot brief, memory and approval path. Busy remote bot sends are refused rather than queued without caller provenance. |
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

The viewer sends a `startId`; the host runs each one once, on its own, so a lost reply never
stops it halfway. When the reply to `task.start` times out or the socket drops, the mirror row
stays (running, no `remote_session`) and, once the host is online again, the viewer asks
`task.startStatus` every 3 s until the start is done (the row is linked), failed or unknown
(the row is put back and `remote://start-progress` reports `error`). `remote_cancel_start`
sends `task.cancelStart` and puts the row back; a start cancelled after its task was made is
stopped on the host. Older hosts refuse both methods (`Unknown method.`): the row is put back.

### Model lists for another computer

A mirror's model picker lists the host's harnesses and models, not this computer's:
`list_providers` and `list_provider_models` take an optional `deviceId` and then ask that host
(`providers.list`, `providers.models`). This computer's own id, or an empty one, lists locally.
The viewer keeps each reply for 10 minutes per device (and per device and provider for models);
`force` asks again, and concurrent identical requests share one call. A device's lists are
dropped when it goes offline, comes back, changes hosting or is revoked, and on sign-out. Every
request, cached or not, first gets the usual errors for an unknown or offline
(`That computer is offline.`) host.

`remote_start_task` and a mirror's `configure_session` are refused here when the cached lists
show the provider isn't installed there (`<Provider> is not installed on <host>.`) or the model
isn't among its models (`<model> is not available for <provider> on <host>.`; `default` always
passes). With nothing cached the host decides, and it refuses a provider it doesn't have.

### Events

Push only; nothing polls. The host buffers the agent events of each subscriber's tasks and sends
each subscriber an `agent.events { events: [...], epoch }` frame: the first event after a quiet
100 ms goes at once (a turn's first delta shows without waiting), the next ones wait out the rest
of 40 ms since the last flush, and after a burst's first 5 frames of 100 ms per frame the last flush
sent (more frames only when the events pass the 180 KiB frame budget; an event over 120 KiB is cut).
Pacing counts frames, not flushes, so however many viewers subscribe, streaming costs the host's
socket about 10 frames a second, whatever the token rate, under the relay's 20 frames a second
(replies included). Consecutive text deltas of the same message are merged.

Every agent event of a followed task carries `seq`, one counter per host launch (`epoch`, new
at each launch). The host keeps each task's last 2000 events (at most 2 MiB per task, 32 MiB in
all) in memory. A viewer remembers the newest `seq` per task and subscribes with `epoch` and
`afterSeq`; the reply then holds, per task, the events after that position (`events`, in order,
deltas merged, within a third of the reply; for a viewer that sent `"z": 1`, within 180 KiB of JSON
while the packed reply fits 180 KiB) or `gap: true` when some are gone (dropped, too
many, or another epoch), when the viewer copies the messages instead. Each task's `seq` is the
position to use next. The viewer drops an event at or before the newest `seq` it saw, so a
replay and live frames never show twice. Events without `seq` (older hosts) are shown as before,
and older viewers ignore the new fields.

A subscriber more than 4096 events behind stops buffering until it catches up at the run end.
Each event is an `AgentEvent` with `sessionId`, `kind`, `text`,
`requestId`, `data`, and optional `tool`; request ids are included where applicable. These are
the kinds sent in `agent.events`:

| Kind | Meaning |
|---|---|
| `approval` | The agent requests permission for a tool action. `requestId`, `data` and optional `tool` describe the request. |
| `approval_cancelled` | An outstanding approval was withdrawn or cancelled. |
| `commands_changed` | The harness's available command catalogue changed. |
| `error` | The provider reports a run error; the explanation is in `text`. |
| `goal` | Snapshot of the Codex goal state. |
| `goal_input` | The goal runtime is asking for user input. |
| `goal_ready` | Goal setup completed and its initial state is available. |
| `input_cancelled` | An outstanding structured-input request was withdrawn or cancelled. |
| `input_request` | The agent requests structured user input; `requestId`, `data` and optional `tool` carry the answerable request. |
| `limited` | The provider reports an account usage limit; `data` may include `resetsAt` and `window`. |
| `message_start` | A new assistant message or phase starts; `data.messageId` identifies it and `data.phase` may identify its phase. |
| `session` | The provider's native session id is available in `text`. |
| `status` | Progress or state text. Plan updates and reasoning/thinking notices use this kind; any provider metadata is reduced to recognized scalar fields. |
| `steer` | A steer was accepted, delivered, or failed; `data` includes its id, status and mode. |
| `subagent` | A child-agent snapshot changed. `data` is a desktop `SubagentActivity` (`id`, `parentId`, `name`, `prompt`, `status`, `response`, `tools`, `startedAt`, `updatedAt`); ids are scoped to the parent run. |
| `team` | A worker row changed in the task's architect team. `sessionId` is the architect task id and `data` is the changed worker row in `task.team` shape. If the team is removed, `data` is `{ "removed": true }`. The desktop emits full `architect-event` snapshots; Connect sends only changed rows through the same event buffer. |
| `task_settings` | Goal runtime task settings changed. |
| `text` | Assistant text delta. `data.messageId` and optional phase associate it with its message. |
| `tool` | A tool started or changed; the current `ToolActivity` is in `tool` (`id`, `name`, `status`, `input`, `output`, and optional `writePaths`). |
| `turn_complete` | A Codex goal turn completed. |
| `turn_start` | A Codex goal turn started. |
| `usage` | Sanitized context-token usage snapshot; `data` carries recognized usage fields only. |

A desktop mirror keeps `subagent` snapshots (live and from `subscribe`'s `subagents`) in its own store, so its Subagents panel matches the host's after missed frames or a reopen. It reads the host's team with `task.team` when the task opens and after each `team` event or run end (`remote_team` in `remote/mirror.rs`, mapped to the desktop TeamSnapshot) and a worker's transcript with `task.workerMessages` (`remote_worker_messages`). The workers panel is read-only there: no pause, resume, setup or team messages. Hosts without these methods show no team.

There are no separate `plan`, `todo`, `thinking` or `reasoning` event kinds: those provider
updates are represented as `status` text and/or data. The provider's `done` event is consumed by
the desktop run loop and is not sent in `agent.events`. Likewise, local `status: running`,
`turn_changes`, persistence errors, and final `done` emissions are window events rather than
Connect `AgentEvent`s. The desktop's `architect-event` is also a window event; Connect translates
it to the `team` kind for subscribed architect tasks. Connect signals run end with `run.finished`.

For example, a live child update is an ordinary `agent.events` item:

```json
{
  "sessionId": "host-session-id",
  "kind": "subagent",
  "text": "",
  "requestId": null,
  "data": {
    "id": "run-id:child-id",
    "parentId": null,
    "name": "Tests",
    "prompt": "Run the focused tests",
    "status": "running",
    "response": "Checking the project setup…",
    "tools": [{ "id": "run-id:child-id:tool-id", "name": "Bash", "status": "running", "input": "cargo test", "output": "" }],
    "startedAt": 1792866000000,
    "updatedAt": 1792866000500
  }
}
```

At a run end the host queues `run.finished { sessionId, successful, stopped, text, status?, team? }`
after that run's last events. The viewer copies the new messages, ends the local turn, then emits the
`done` agent event with `text`, so the transcript reloads with everything in it.

#### Team tasks

An architect task can be working while no agent of its own runs: the architect ends its turn
(the team becomes `waiting`) while its workers carry on, and the desktop still shows the task as
running. Connect reports it the same way. Every task row of `tasks.list` and `subscribe`, and every
`run.finished`, of a task that leads a team carries
`team: { status, activeWorkers }`, where `status` is the team snapshot's status (`ready`,
`running`, `waiting`, `paused`, `failed`, `completed`) and `activeWorkers` counts workers that are
not abandoned and are `queued`, `running` or `waiting`. Other tasks have no `team`. The values are
read in one query from the saved team, on the blocking pool.

- While the team is `running` or `waiting`, the row's `status` is `running`, whatever the saved
  status (`completed`, `idle`, or `interrupted` because no agent is registered). The one exception
  is a row whose own architect run is registered and `waiting` for input or approval: it stays
  `waiting`. Once the team is paused, failed or completed, `status` is what it would be without a
  team.
- A `run.finished` whose team is still running or waiting has `status: "running"`: the run ended,
  but the task did not, so the phone keeps it under Running. Without `status`, `run.finished`
  means what it always did.
- When a team the host has reported or seen as active stops being `running` or `waiting` and no
  agent of the architect task is registered, the host sends a further `run.finished`:
  `{ sessionId, successful: team.status == "completed", stopped: team.status == "paused", text:
  "Team completed" | "Team paused" | "Team failed" | "Team stopped", status, team, teamSettled: true }`,
  with `status` as a task row would show it then. If the architect itself still runs, its own run
  end reports the settled team instead. A settling `run.finished` is skipped while the same task's
  `run.finished` is still waiting in the buffer, but it can follow one already sent; viewers treat
  a repeated `run.finished` as a no-op (the desktop mirror ends a turn only once).

The viewer re-emits each event as the window's ordinary `agent-event` with the mirror's *local*
session id (events for tasks it doesn't mirror from that host are dropped), and moves the row's
status (`approval`/`input_request` → waiting, their cancellations → running, any other event
→ running). A subscription reply catches up on what was missed: a status that changed, a run
that ended while this computer wasn't listening, and messages newer than `remote_synced_id`.

### Message copies

`task.messages` keeps the legacy `afterId` path for desktop mirrors: rows after the cursor, oldest
first, at most 400 rows by default. `tail: true` starts at the newest message, and `beforeId` pages
backward from that id; those modes default to 100 rows. Every mode is kept within 180 KiB of JSON,
and newest-first pages keep the newest rows that fit. An optional `budget` sets the page's JSON
bytes instead: clamped to 4 KiB..1 MiB, and to at most 180 KiB unless the request carried `"z": 1`
(the viewer opens packed frames). A page over 180 KiB is cut until it packs (deflates) into 180 KiB,
so its sealed frame still fits the relay's 256 KB. A page always holds at least one message. The
desktop mirror's and phone's first tail page ask for 32 KiB (about 12 KB on the wire, one round trip
on 3G), and the mirror's afterId and `beforeId` copies ask for 1 MiB. Hosts before `budget` ignore it. `hasMore` reports newer rows after the page;
`hasOlder` reports older rows before its first row. `lastId` is the current newest message id, or
`null` for an empty thread. Any message over 120 KiB is cut and ends with
`…[cut on the host to fit a remote page]` (JSON tool content is cut inside, so it still parses).
When a page's single message does not fit a smaller `budget`, it is cut to the budget the same way
and marked `cut: true` (same id); viewers fetch it whole later with `afterId: id - 1` and a full
budget and replace their copy. The desktop mirror does this at the end of each copy and marks such
a copy in `messages.remote_cut` (schema 33, H's message id) until it is replaced, so a failed fetch
or a restart leaves it for the next copy; older viewers ignore the flag. The desktop copies older
pages (`beforeId`) in rounds of 8 until none are left, each page waiting (up to 10 s) while other
calls to that host are pending.
The desktop mirror stores an `afterId` page and advances `remote_synced_id` in one transaction,
and skips a page that no longer starts at the stored position (a concurrent copy got there first),
so messages are never copied twice. It follows `hasMore` for up to 50 pages per copy and also takes
the host's title.

Known limit: a message the host rewrites in place after it was copied (a tool's status, say)
is not copied again.

Responses and events are handled as soon as they arrive and never wait behind a slow request.
Each peer can have up to four read requests in flight. The read lane includes `subscribe`,
`tasks.list`, `projects.icons`, `providers.list`, `providers.models`, `task.startStatus`,
`task.cancelStart` (never behind the start it cancels), `task.messages`, `task.team`,
`task.workerMessages`, `task.attachment`, `task.workspaceImage`, `task.pull`, `task.pullDiff`, `task.pullDetect`,
`task.changes`, `task.diff` and `bots.list`. Replies may arrive out of order; both the phone
and desktop client match them by request id. The desktop mirror still copies messages by its
`afterId` cursor and commits each page before advancing it.

State-changing requests from one peer remain serial and keep their arrival order relative to one
another. This includes `task.send`, `task.steer`, `task.stop`, `task.configure`,
`approval.respond`, `input.respond`, `bot.thread`, `bot.message`, `bot.pause` and `task.pullMerge`.
Each `task.start` runs on its own, so a stop or an answer never waits behind a clone.
Read requests may run while that ordered lane is busy. The event flush only batches mirror
events; an RPC reply is placed directly on the socket outbox and does not wait for an event tick.

A request's `ts` and `nonce` are checked when it arrives, not when it runs, so time in the queue
never makes it expire. A request (other than `task.start`) that waited 18 s or more, past its
caller's 20 s timeout, is refused instead of run, so a prompt the caller sent again can't run twice.

### Batch requests

`batch` combines 1–8 read-only calls into one request. Calls are validated before running; a
write method or malformed batch shape rejects the batch, while invalid parameters for one allowed
call become that call's error. Calls run in the listed order and results stay in the same order.
The `calls` list cannot contain another `batch`.

For example, opening a task can fetch its newest messages, subscribe for live updates and check
for a pull request in one phone round trip:

```json
{
  "type": "req",
  "id": "request-id",
  "method": "batch",
  "params": {
    "calls": [
      { "method": "task.messages", "params": { "sessionId": "task-id", "tail": true } },
      { "method": "subscribe", "params": { "sessionIds": ["task-id"] } },
      { "method": "task.pullDetect", "params": { "sessionId": "task-id" } }
    ]
  }
}
```

The response uses the ordinary outer request id and returns one `{ok, result|error}` entry per
call:

```json
{
  "type": "res",
  "id": "request-id",
  "ok": true,
  "result": {
    "results": [
      { "ok": true, "result": { "messages": [], "lastId": null } },
      { "ok": true, "result": { "tasks": [] } },
      { "ok": false, "error": "..." }
    ]
  }
}
```

The combined results fit within the 180 KiB JSON budget, so calls inside a batch get plain-sized
`task.messages` pages and replays even for a viewer that opens packed frames. If successful results would exceed it,
the largest entries are returned as per-call errors asking the client to request them separately.

### Host timing and slow paths

Set `PANDACODE_CONNECT_TIMING=1` before launching the desktop app to emit one debug timing line
per handled request. It includes method, time waiting in the peer queue, handler time, JSON
serialization time, sealing time, and payload/frame byte counts. `task.start` also reports any
wait for the global four-start limit. A batch logs its outer queue wait and per-call handler times.
It logs no task or peer ids.

The request path previously did message-page queries, subscription snapshots, and session/root
lookups while polling on the async socket worker. Those database operations now use the blocking
pool; the existing `(session_id, id)` message index already supports paging. Icon generation,
image reads/transforms, git changes/diff and GitHub detail/diff work already used blocking
workers. Pull detail and file-diff results are now cached for 30 seconds per session and PR;
pull-detection results are keyed by the latest message id and linked PR number, and a merge
invalidates that session's cache. This avoids repeated GitHub CLI work while the inspected task
state is unchanged.

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
| `remote_targets` | | `[{ deviceId, name, platform, online }]`: other online devices with hosting on |
| `remote_repo_check` | `{ projectId }` | `{ repo: "owner/name" \| null, branch, ahead, dirty, pushed }`. `repo` is null without a GitHub `origin`; `ahead` is `rev-list --count @{u}..HEAD` (0 without an upstream); `pushed` is whether `origin/<branch>` exists. Local refs only, no network. |
| `list_providers` | `{ force?, deviceId? }` | as without `deviceId`, from that host (see Model lists) |
| `list_provider_models` | `{ provider, force?, deviceId? }` | as without `deviceId`, from that host |
| `remote_start_task` | `{ deviceId, projectId, prompt, provider, model, mode?, effort?, worktree?, branch?, sessionId? }` | the mirror `Session` plus `sessionId`. `branch` defaults to the checkout's. An empty New task draft without its own worktree passed as `sessionId` becomes the mirror; otherwise a new task is made. On failure nothing is left: a new row is deleted, a draft stays a draft. With `startId`, a timeout or a dropped socket keeps the row and returns it (see Starting a task). |
| `remote_cancel_start` | `{ sessionId }` (the local row) | `{ cancelled }`. Cancels a start that has no host task yet and puts the row back (also when the host can't be reached); `cancelled: false` when the host had already made the task, which then runs as a mirror. |

Window events: `remote://status` (status), `remote://presence` (devices), and for mirrors the
ordinary `agent-event` and `sessions-changed`.

The agent-event hook in `lib.rs` clones and allocates nothing while hosting is off or nobody
follows (or followed, since this launch) that task.

## Privacy

Listed in the README privacy table: remote access contacts GitHub (sign-in only) and the relay
(while signed in). Task content crosses the relay only encrypted end to end.
