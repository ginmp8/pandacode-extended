# PandaCode Connect relay

A Cloudflare Worker with one SQLite-backed Durable Object per account. It signs devices in with
GitHub, keeps the device list, and forwards end-to-end encrypted frames between an account's
devices. It stores no task content. The contract is in [docs/remote-access.md](../docs/remote-access.md).

## Deploy

Deployment is an explicit maintainer operation. The checked-in configuration has
no upstream hosted route and keeps optional observability disabled. Choose your
own Cloudflare account, worker name and domain before using these commands.
Development, installation, tests and CI do not publish a relay.

1. Create a GitHub OAuth App (Settings → Developer settings → OAuth Apps) and tick **Enable Device Flow**.
   Generate a client secret too: the relay uses it only to check that a sign-in token was issued to this app (`POST /applications/{client_id}/token`). The desktop app never sees it. The client ID also goes into the desktop build as `PANDACODE_GITHUB_CLIENT_ID`.
2. From this folder:
   ```sh
   npm ci --ignore-scripts
   # Install/review Wrangler 4.151.0 independently; do not use npx or npm install here.
   export PANDACODE_WRANGLER_PATH=/absolute/path/to/reviewed/wrangler/bin/wrangler.js
   openssl rand -base64 48 | node ./scripts/wrangler.mjs secret put SESSION_SECRET
   node ./scripts/wrangler.mjs secret put GITHUB_CLIENT_ID
   node ./scripts/wrangler.mjs secret put GITHUB_CLIENT_SECRET
   npm run deploy
   ```
   Each `secret put` takes the secret's name and asks for its value (paste the OAuth App's client ID,
   then its client secret). Run them one line at a time.
   The deploy creates the `Account` Durable Object (SQLite migration `v1`), `AUTH_LIMITER` (10/min per IP), `GLOBAL_AUTH_LIMITER` (300 sign-ins/min), and `ACCOUNT_LIMITER` (30/min per account) rate-limit bindings. A signing secret shorter than 32 characters, or missing OAuth configuration for sign-in, receives a generic server error before requests are processed.
3. Build the desktop app with `PANDACODE_RELAY_URL` set to the Worker URL (or set it as an environment variable at run time).

Rotating `SESSION_SECRET` signs everyone out.

## Behaviour

- `GET /v1/connect?host=0|1` opens the socket and sets the host flag and `last_seen` in one write, then broadcasts presence once. A `{kind:"host"}` frame still works; repeating the stored value writes and broadcasts nothing.
- `POST /v1/token` (Bearer relay token) returns `{ token }`: a fresh 30-day token for the same account and device, `iat` = now. It passes the same checks as `/v1/devices` (signature and expiry, `ACCOUNT_LIMITER`, device still registered and registration generation unchanged), so a revoked device gets 401 and cannot refresh. Other failures are the usual 401/429 JSON errors; other methods get 405.
- `DELETE /v1/account` requires a current authenticated registered device and explicit confirmation in the mobile UI. It closes all account sockets, atomically removes the Durable Object's SQL/KV state, and returns `{ ok: true }` only after deletion succeeds. Other methods receive 405. Deletion in progress rejects racing requests; departed sockets cannot forward queued frames. A later explicit sign-in recreates the empty schema.
- Frames are `{ to, body, ref? }` (`ref`: opaque, at most 64 chars). When a frame can't go out, the sender gets `{ kind: "undeliverable", to, ref?, reason: "offline" | "rate" }` with `ref` echoed. Malformed frames get `{ kind: "error", error }`.
- Presence is `{ kind: "presence", devices: [{ id, name, platform, publicKey, host, online, lastSeen }] }`, the full registered list, the same shape as `GET /v1/devices`.
- Clients send `ping` every 30 s (answered by the runtime without waking the object). A socket is online only while its last ping, or its join before the first ping, is under 75 s old; stale sockets are closed (code 4002) when found and presence is broadcast.
- A reconnect keeps the device's rate bucket (20/s, burst 100), seeded from the old socket before it closes.
- Throttled binary frames and text longer than 4,096 characters are rejected before JSON parsing. Small throttled sends retain their `to` and `ref` correlation.
- Schema 2 adds a random UUID registration generation, included in signed tokens and forwarded only from verified claims. A newly registered row has a new generation after device revocation or account deletion, so an old token cannot revive even if re-registration occurs within the same second. Existing schema-1 rows keep legacy `iat` validation until removal; no-generation tokens are rejected for all fresh rows. Token contents remain opaque to desktop/mobile clients, which continue using `acct`, `dev`, `iat` and `exp` locally. The schema migration is gated on `PRAGMA user_version`.

## Develop

```sh
npm run typecheck   # tsc for the Worker and for the tests
npm test            # node --test (Node 24; integration fixture uses built-in SQLite)
npm run dev    # local; put SESSION_SECRET=... in .dev.vars; requires reviewed Wrangler
```

| File | Job |
|---|---|
| `src/index.ts` | Routes, auth rate limit, GitHub sign-in, token check |
| `src/account.ts` | Durable Object: device rows, hibernating WebSockets, forwarding, presence |
| `src/account-delete.ts`, `routes.ts` | Confirmed durable account deletion and exact authenticated routes |
| `src/token.ts`, `validate.ts`, `bucket.ts`, `frames.ts`, `github.ts`, `limits.ts` | Pure logic, unit tested |

The tests include the actual Worker and Durable Object with an in-memory SQLite
adapter: deletion ordering/failure, concurrent registration, object restart,
departed sockets, generation invalidation, untrusted header stripping and the
existing streamed request-body size limits. This is a local contract check; it
does not claim a deployed Cloudflare environment was exercised.
