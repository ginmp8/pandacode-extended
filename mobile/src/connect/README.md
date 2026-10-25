# Connect API for screens

The phone is a viewer: it signs in, lists the user's computers, and sees, prompts and
controls their tasks. Screens read state with hooks from `src/store` and act with
`connect` from `src/connect/client.ts`. Nothing else should touch the socket or secrets.

`connect.init()` runs once in `src/app/_layout.tsx`.

## Hooks (`src/store/index.ts`)

| Hook | Returns |
|---|---|
| `useStatus()` | `{ configured, ready, signedIn, connected, account, error, demo, fingerprint }`. `fingerprint` is this phone's 128-bit key fingerprint. |
| `useDevices()` | every discovered device: `Device & { current, trusted, keyChanged }`; discovery does not authorize it |
| `useHosts()` | other explicitly trusted computers with hosting on (`online` tells if it answers now) |
| `useTasks(deviceId)` | `{ tasks, projects, loadedAt }` from the last `listTasks`, statuses and titles kept live, or `undefined` |
| `useMessages(deviceId, sessionId)` | stored messages, oldest first |
| `useHistory(deviceId, sessionId)` | `null` while a message copy runs, the last copy's error, or `undefined` (the transcript shows a spinner or the error with Retry) |
| `useLive(deviceId, sessionId)` | the live run: `{ status, activity, items, approvals, inputs, usage?, limited? }`. `items` are `text` / `tool` (ToolActivity) / `note` / `error`, in order. `approvals` and `inputs` are `{ requestId, text, data }`; `data` of an input is the AgentInputRequest (questions/form/url). |
| `useProviders(deviceId)`, `useModels(deviceId, provider)` | last fetched lists |
| `useConnect(selector)` | the raw zustand store, for anything else |

## Actions (`connect.*`, all async, throw `Error` with a user-facing message)

- Sign-in: `signInStart()` → `{ userCode, verificationUri, expiresIn, … }` (show the code, open the URL), then `signInFinish(code)` waits for GitHub and registers the phone. `signInCancel()`, `signOut()`.
- Demo (`src/demo.ts`): `enterDemo()` / `leaveDemo()`; while on, every host call goes to `src/demo-host.ts` and nothing reaches the network. `signOut()` in demo mode leaves it.
- Account: `deleteAccount()` → relay `DELETE /v1/account`, then wipes secrets and the icon cache and signs out (screens also call `clearLocal()`).
- Devices: `setTrusted(deviceId, expectedPublicKey, allow)` only after the user compares and confirms the displayed fingerprint; a changed key rejects stale UI confirmation. `revokeDevice(deviceId)`. `refreshDevices()` only matters before the socket's first presence frame.
- Tasks: `listTasks(deviceId, limit?)`, `refreshLists(maxAgeMs?)` (every online trusted host), `startTask(deviceId, { repo | projectId, branch?, worktree, prompt, provider, model, mode?, effort? })` → sessionId (waits up to 12.5 min; the new task is watched).
- Open a task: `watch(deviceId, [sessionId])` subscribes (renewed every 9 min and on presence), copies missed messages and streams events into `useLive`; `unwatch` when the screen goes. `loadMessages(deviceId, sessionId)` copies new messages on demand (follows `hasMore`).
- In a task: `sendMessage(…, text)` → `{ queued }`, `steerTask(…, text)`, `stopTask(…)`, `configureTask(deviceId, { sessionId, provider?, model?, effort?, mode? })`, `respondApproval(…, requestId, approve)`, `respondInput(…, requestId, response)`.
- Model pickers: `listProviders(deviceId, force?)`, `listModels(deviceId, provider, force?)`.
- Pull requests and changes (newer hosts): `loadPull(deviceId, sessionId, force?)` → the linked or newly detected PR (cached in `pulls`), `pullDiff(…, path)`, `mergePull(…, method)`, `taskChanges(…)`, `taskDiff(…, path?, staged?)` → `{ text, truncated }`.
- Bots: `listBots(deviceId)` (also after every `listTasks`), `botThread(deviceId, botId)` → sessionId, `botMessage(deviceId, botId, sessionId, text)`, `pauseBot(deviceId, botId, paused)`. An older host's `Unknown method.` becomes "Update PandaCode on <computer> to use this." and sets `outdated[deviceId]`.

At a run end the host's messages are copied, then the live view goes idle (`stopped`/`failed` when the run was).

## Rules kept here

- Crypto is byte-for-byte `src-tauri/src/remote/crypto.rs` (`tests/crypto.test.ts` checks a vector from the Rust crates).
- Presence and account membership only discover peers. Every outgoing call and decrypted reply/event requires the exact explicitly pinned key. A removed or changed key cancels pending authority. Pins are retained at startup and ordinary sign-out.
- `admitReply` rechecks the current peer key, pin and session before and after `PersistentReplay` writes a secure checkpoint. Frames have an inclusive ±60-second timestamp window, printable 16–64-character nonce, and a 20,000-nonce capacity that refuses new frames when full. After restart, timestamps at/below the prior high watermark are refused. Opposite permitted peer clock skews can require up to 120 seconds of catch-up; task lists and transcripts resynchronize.
- `SecurityWrites` invalidates a session before sign-out/deletion, drains in-progress OS writes, cancels queued writes from the prior session, and then clears credentials or the complete identity. Asynchronous calls and cache reads carry a session generation; stale results cannot restore a signed-out account. Cache clear operations also drain file writes and invalidate old file handles.
- Local outbox/preferences are account-bound and use a separate serialized persistence boundary. Account changes invalidate late loads and active flushes. Outbox sends persist an uncertain-delivery marker before the network effect; an interrupted acknowledgement requires an explicit retry rather than automatic replay.
- Startup reads credentials, device identity, pins and the replay checkpoint before opening the socket. Storage corruption/read/write failure fails closed; no identity or pins are silently replaced. Relay tokens are bound to the configured HTTPS origin and their account/device claims are checked locally before use.
- Sync: task lists refresh on every presence frame, on foreground, after each `run.finished`, and every 15 s while a task runs (60 s otherwise); a list that shows a run ended settles a live view still showing Working. Subscribe replies catch up on statuses, requests and messages after a reconnect.
- In development Fast Refresh re-runs `client.ts` with fresh module state; the newest instance disposes the previous socket and `init()` runs once per instance, so calls never go through an instance without a token.
- A 401 or close code 4001 signs out only if it refused the current token. Tokens with < 7 days left are refreshed at connect and every 12 h.
- Socket: backoff 1 s → 60 s (reset after 30 s up), 60 s after close 4000, 30 s pings, closed after 30 s in the background, reconnected on foreground.
- Secrets (token, X25519 key, device id, account, pins and replay checkpoint) live in `expo-secure-store`, this device only. Missing initial state is distinct from unreadable state. A relay key conflict is reported without automatic re-registration under another device id.
