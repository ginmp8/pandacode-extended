# PandaCode Extended mobile 0.5.0

The iOS and Android companion views and controls tasks on your computers through
your configured PandaCode Connect relay. Tasks run on the computer. The mobile
source and dependency lock come from [PandaCode 0.3.47](https://github.com/aicodeking/pandacode/releases/tag/v0.3.47),
commit `4187553cb48db96657609948f6b0904556845d45`, with Extended's explicit device
trust and fail-closed security controls integrated here. See the
[0.5.0 integration map](../docs/upstream-integration-0.5.0.md) and
[remote-access design](../docs/remote-access.md).

## Features

| Area | Available behavior |
| --- | --- |
| Inbox and threads | Computer/task lists, live activity, paginated message history, local cached threads, pinned and snoozed tasks, held outbox messages |
| Task control | Start tasks, send/steer/stop, model and effort selection, approvals and questions; the host still enforces trust and its separate Full access setting |
| Teams | Architect/worker activity, active-worker counts, team-aware task status, worker message paging |
| Code review | Linked pull request details, checks, changed files, diffs and explicit merge actions |
| Bots | Bot lists, threads, messages and pause/resume |
| Images | Authenticated source-message attachments and bounded workspace image reads |
| Appearance | System/light/dark appearance, desktop daily wallpaper scenes and local demo mode |
| Account control | Explicit sign-out, per-computer trust removal, confirmed relay account deletion |

## Local setup

Use the checked-in lockfile with Node 24. The existing upstream dependency
versions are retained; installation scripts are disabled by the package's
`.npmrc` and the command below.

```bash
cd mobile
npm ci --ignore-scripts
npm test
npm run typecheck
EXPO_NO_TELEMETRY=1 npm run lint
```

Connect is **off by default**. Configure your own HTTPS relay origin and GitHub
OAuth application's public client ID in `mobile/.env` or the shell:

```dotenv
PANDACODE_RELAY_URL=https://relay.example.com
PANDACODE_GITHUB_CLIENT_ID=your_public_client_id
```

`EXPO_PUBLIC_PANDACODE_RELAY_URL` and `EXPO_PUBLIC_PANDACODE_GITHUB_CLIENT_ID` are
also accepted. These are public build settings; never put an OAuth client secret,
GitHub token, private device key or relay signing secret in them. Configure the
same relay/OAuth application for the desktop. The relay URL must be one HTTPS
origin with no credentials, path, query or fragment.

```bash
EXPO_NO_TELEMETRY=1 npm run start -- --clear
# A local Xcode or Android SDK installation is required for native builds:
EXPO_NO_TELEMETRY=1 npm run ios
EXPO_NO_TELEMETRY=1 npm run android
```

Restart Metro after changing the configuration. Metro inlines the public build
settings. The app rejects saved credentials from a different relay; **Clear saved
sign-in** removes only the old token/account so you can authenticate again while
keeping your device identity and explicit key pins.

The configuration uses Extended bundle IDs (`ai.pandacode.extended.mobile`), has
no upstream EAS owner/project, no submission profile and no hosted release
destination. Automatic app updates are disabled. Native signing, store ownership,
cloud builds and publication must be configured and reviewed separately by the
maintainer; none are performed by installation, tests or CI.

## Trust both devices

1. Sign in to the same configured relay/account on the phone and computer.
2. On the computer, open **Settings → Remote access** and find its own fingerprint.
3. On the phone, open **Computers → Compare fingerprint and trust…**. Compare all
   32 hexadecimal characters (128 bits) with the computer's own fingerprint, then
   confirm **Fingerprints match**.
4. In mobile **Settings → This phone's fingerprint**, find the phone's fingerprint.
   Compare it with the phone entry shown on the computer, and explicitly trust
   the phone there. Enable remote control and, only for commands that need it,
   the desktop's separate **Full access** control.

Presence and GitHub account membership provide discovery only. No peer is
automatically authorized. A changed key loses authority immediately and requires
a new comparison. Trust removal cancels pending calls and blocks subsequent
requests, replies and events. The client rechecks the current key after each
asynchronous replay-checkpoint write before applying the frame.

### Replay and storage failures

Every encrypted request, reply and event carries an epoch-millisecond timestamp
and a fresh printable nonce. The phone uses the host's inclusive ±60-second
window, rejects invalid timestamps and keeps at most 20,000 recent nonces without
evicting live receipts. A small SecureStore checkpoint is written **before** a
reply or event takes effect. Across app restarts, timestamps at or before the
last accepted maximum are refused even if their nonce is different.

This checkpoint deliberately favors refusal when a clock moves backwards or a
frame arrives late after a restart. A synchronized peer may need up to 60 seconds
to pass a saved future timestamp. At the two opposite permitted clock-skew
extremes, one peer may need up to 120 seconds to pass another's checkpoint. The task
list, message paging and subscriptions resynchronize state; an uncertain
mutation is never automatically retried as though it were unsent. Keep device
clocks synchronized.

An unreadable key, pin set, account, replay checkpoint or secure-store operation
stops Connect. Existing keys and pins are not silently deleted or regenerated.
Only a missing initial identity, or an identity explicitly erased by account
deletion, is created. Repair the OS secure-storage problem and reopen the app.
Clearing saved sign-in does not repair or erase corrupt keys/pins. Sign-out and
confirmed deletion invalidate the session immediately, drain any security write
already in progress, then erase the requested data. Pending token refreshes, pin
updates, replay receipts and delayed cache reads cannot recreate the erased
session. File writes/moves complete before attachment downloads return a URI.

## Demo

**Explore the demo** uses sample computers and tasks locally, with canned host
responses and no Connect network requests. It remains enabled until **Settings →
Leave demo**. `EXPO_PUBLIC_PANDACODE_DEMO=1` can be used for local screenshots;
leave it unset for a real Connect build.

## Privacy and local data

Relay credentials, the X25519 private key, device identity, explicit key pins and
the replay checkpoint use `expo-secure-store` with this-device-only keychain
access where supported. Local preferences and outbox entries use SecureStore and
are bound to the account that saved them. Signing out clears their in-memory
view; signing back into the same account can restore them. Entries without a
matching account binding are never delivered automatically. Before an outbox
message is sent, a durable delivery marker is saved. If the app closes or cannot
save the acknowledgement, that entry requires a manual check/retry instead of
automatically sending a possibly completed prompt again.
Cached transcripts, images and project icons use the app's local sandbox; they
are not a separate encrypted vault. A device user with access to the app can
read its cached data.

Connect communicates with the relay you configure and GitHub's device sign-in
endpoints. The account image can be loaded from GitHub's avatar service; opening
links in tasks or Settings is an explicit user action. Task payloads are
end-to-end encrypted between pinned devices (X25519, HKDF-SHA256 and
XChaCha20-Poly1305). The relay still sees account/device metadata, connection
timing and encrypted frame sizes. No analytics service or automatic app-update
endpoint is configured by this package.

**Sign out** stops the connection and removes this phone's saved relay
token/account while keeping its device identity and trust pins. **Delete account…**
first requires explicit confirmation, then calls authenticated `DELETE
/v1/account`. Only the relay's `{ "ok": true }` response authorizes the local
wipe of credentials, keys, trust, caches and phone preferences/outbox. The relay
removes every registered device for that account. Computer task data and the
GitHub account itself are unaffected. If deletion or storage fails, the UI reports
the error rather than claiming completion.

## Validation and limits

`npm test` exercises the imported message/turn/team/image/PR state and protocol
fixtures plus Extended trust, key-change races, replay persistence, storage
failures, credential origin binding and package configuration. TypeScript and
Expo lint check the complete app. These checks do not replace an iOS/Android
device run with a configured desktop and relay, OS keychain access, native live
activities and local signing. No store build or publication is implied.

Screen APIs and state ownership are documented in
[src/connect/README.md](src/connect/README.md).
