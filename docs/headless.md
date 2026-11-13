# Headless server (`pandacode serve`)

The same PandaCode binary runs as an always-on **host** on a server: started over SSH, no
desktop session, no one watching. Your devices — the mobile app and other computers'
PandaCode — see the server as one more computer in PandaCode Connect and can start, watch,
steer and approve tasks on it, including cloud runs ([cloud agents](cloud-agents.md)). There
is no webview and no window: the desktop frontend never runs on a server.

Nothing changes for the desktop app. `pandacode` with no arguments is the ordinary GUI;
`pandacode serve …` is the host and explicitly enables Connect and hosting for that invocation, even when the desktop's saved Connect switch was off.

## What the server runs

`serve` builds the real app — storage, provider credentials, the queue, bots, the
cloud-agent resume loop, Connect hosting (`setup_common` in `lib.rs`, shared with the
desktop startup) — then closes the window from `tauri.conf.json` and keeps the process
alive. Everything user-facing happens on your devices. The few jobs the desktop window
would normally do are done natively in `src-tauri/src/headless.rs`:

| Job | On the desktop | In serve mode |
|---|---|---|
| Queue drain | frontend `dispatchQueue` (main.ts) after each run | native, after each clean run end and at startup; failed, stopped, archived and still-limited tasks hold their queue, as on the desktop |
| Pull-request links | frontend after each run | native `github_detect_pull_request` after each run |
| Usage-limit resumes | frontend timer | native 60 s check (`Store::due_limit_resumes`) |
| App-action tools (`pandacode_run_project`, `pandacode_preview_file`, `open_*`) | open in the window | refused with "headless host" so agents fail fast |
| Updates | checked every 6 h, may relaunch | never (no updater; update by reinstalling) |

Turn checks are desktop-only (they are opt-in and off by default). Browser-dependent work
(Jev, the built-in browser) is unavailable on a headless host.

**Secrets are stored in a file, not the OS keychain.** On Linux and Windows, `serve`
sets `PANDACODE_SECRETS_FILE=1` unless you set it yourself, so provider keys, channel
tokens and every other app secret are written to `secrets.json` in the app data
directory instead of the OS keychain (Secret Service / Windows Credential Manager). On
Linux the file is created with mode 0600 (owner read/write only); on Windows it has no
mode bits and relies on the app data directory's permissions. Anyone who can read that
file as the service user, or as root/an administrator, can read the secrets, so run
`serve` as a dedicated user and keep the app data directory private. Set
`PANDACODE_SECRETS_FILE=0` to use the OS keychain instead (it must be unlocked and able
to answer noninteractive reads). macOS always uses the same 0600 file.

When adding something new that the frontend does after a run or on a timer with real
effects, give serve mode a native twin in `headless.rs` — see the checklist at the bottom.

### SuperAgent watchdog recovery boundaries

The owning `serve` process checks a bounded batch of active native assignments
every 30 seconds. Besides the absolute attempt budget, it interrupts and revokes
assignments whose recorded start clock is invalid or whose Team, mandate,
coordinator, worker, or session is no longer authorized or available. The
interruption is revision-guarded and audited with a sanitized reason. A
late provider callback cannot approve the task, and the previous session is
stopped. This is a **stop-and-investigate** safety mechanism, not automatic
retry or recovery of unverified external work.

The watchdog also treats a mismatched Ticket/Team policy revision, a moved Team
project, or a persisted tool/effect grant unsupported by the read-only provider
as an immediate revocation condition. Provider admission and callback settlement
repeat the policy and project checks: late model responses cannot repair or
widen an invalidated mandate.

A coordinator may legitimately have a different project checkout from the
Ticket's worker. The watchdog validates an admitted coordinator against that
coordinator bot's current checkout and its exact native session, while Workers
remain bound to the Ticket's immutable worker project. A moved, missing or
unadmitted assignment remains interruptible; valid separate coordinator
checkouts are not falsely revoked. Coordinated Worker dispatch also repeats
the native session-aware lease check used by initial admission and retry:
another Team's active coordinator checkout and missing session records cannot
silently permit an overlapping Worker before the SQLite lease guard runs.

Headless generic usage-limit resets and message-queue drain explicitly skip
every session recorded in native SuperAgent assignment history, including
settled and revoked attempts after process restart. Missing assignment-history
data fails closed. The ordinary non-SuperAgent session paths are unchanged.
A task-owned provider turn can only run through a new explicit native admission,
never through unattended generic resume or queue recovery.

An interrupted Ticket remains durable and cannot be restarted by an inferred
source event. On restart, existing `pending` external-effect receipts become
`uncertain`; they require independent provider reconciliation before any
future retry. A paused or revoked Team is not re-enabled by a watchdog tick.
The existing `--inspect-ticket` and `--inspect-ticket-effects` commands show
redacted state without touching live sessions. Per-effect dispatch and full
headless admission/retry still require explicit user-authorized native grants.

### Operator-staged headless Demands

A trusted local operator can persist a blocked, read-only manual Ticket in the
existing native Ticket/Mandate store, without launching a provider. Use a private
UTF-8 objective file (up to 16 KiB) to avoid exposing its content in process
arguments or stdout:

```sh
pandacode serve --queue-demand <team-id> <stable-request-key> /private/objective.txt
```

The Team must be enabled, its selected bots available, and its existing native
single-Worker policy valid. Repeating the stable key with identical content is
idempotent; changing the content or reusing a desktop-dispatch key is denied.
The output is redacted and includes the Ticket ID, blocked state and the
explicit-dispatch requirement. The objective file should only be readable by
the dedicated headless service account. **Staging is not provider admission**:
it creates no Assignment/session, never dispatches a bot, and grants no tools or
external effects. The live native retry/dispatch path must separately recheck
mandate expiry, revocation, policy revision, project and checkout lease. Stopped
Tickets cannot be restarted through key replay.

## First run

```sh
pandacode serve --sign-in
```

Prints a GitHub device-flow code, waits for you to approve it in a browser (anywhere), then
serves. `--device-name "build-server"` names the server in your device list (default: the
hostname). The server becomes a host automatically. Other flags, each exiting after it
saves: `--sign-out`, `--full-access` / `--no-full-access` (let remote devices pick Full
access — no approval prompts; off by default).

```sh
pandacode serve        # requires an existing sign-in; exits non-zero if none
```

## Linux server

1. Install the binary and Tauri's Linux runtime libraries (GTK, webkit2gtk — they are
   linked even though no webview is created). A display must exist to *start*: run under
   `xvfb-run` (a virtual X server; PandaCode renders nothing).
2. Install and log in each agent CLI (`claude`, `codex`, …) and `gh` (pull-request links
   and private clones use it), with the server's own logins.
3. Sign in (above), then enable the service:

```ini
# /etc/systemd/system/pandacode.service
[Unit]
Description=PandaCode headless host
After=network-online.target
Wants=network-online.target

[Service]
User=pandacode
ExecStart=/usr/bin/xvfb-run -a -s "-screen 0 1280x800x24" /usr/local/bin/pandacode serve
Restart=always
Environment=PANDACODE_SECRETS_FILE=1

[Install]
WantedBy=multi-user.target
```

- `PANDACODE_SECRETS_FILE=1` keeps provider credentials in the same 0600 `secrets.json`
  file macOS uses, instead of the Secret Service a server doesn't have (and which cannot
  answer noninteractive reads). Set `PANDACODE_SECRETS_FILE=0` to use a keyring anyway.
  Log in each CLI/provider once; a keyring's old values are not migrated.
- **NTP required.** Connect refuses requests whose timestamp is more than a minute off.
- One instance per data dir. Don't run serve and the desktop app on the same data dir:
  serve skips the single-instance guard.
- Status lines go to stdout (journald): signed-in account, hosting, relay connection,
  online devices — printed only when they change.

## Mac mini (headless-ish)

A Mac without anyone logged in may refuse window creation over plain SSH (the spike that
mattered; the process itself needs no window). The supported recipe: enable automatic
login for the server's user and run a LaunchAgent with `RunAtLoad`:

```xml
<!-- ~/Library/LaunchAgents/ai.pandacode.serve.plist -->
<plist version="1.0"><dict>
  <key>Label</key><string>ai.pandacode.serve</string>
  <key>ProgramArguments</key><array>
    <string>/Applications/PandaCode.app/Contents/MacOS/pandacode</string>
    <string>serve</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
</dict></plist>
```

No xvfb, no `PANDACODE_SECRETS_FILE` (macOS already uses the 0600 file). Try plain
`ssh host pandacode serve` first — if it starts, skip the auto-login.

## Bots

For the Extended desktop import/export workflow, source registry, SHA-pinned Git sync,
optional settings, external channels and verified pre-import backups, see
[Portable bot manifests and Git synchronization](extended-bot-manifests.md).


Bots (channels, schedules, approval rules, their whole profile) are configured with one
JSON file applied inside the serve process — no UI, and channels come up live:

```sh
pandacode serve --import-bots bots.json
```

```json
{ "bots": [ {
    "name": "build-bot",
    "role": "Build assistant", "instructions": "…", "goals": "…", "soul": "optional persona",
    "provider": "claude", "model": "…", "mode": "auto", "heartbeatMinutes": 30,
    "skills": ["skill-id"], "mcp": ["server-name"],
    "channels": [
      { "kind": "telegram", "label": "main", "config": {},
        "secrets": { "token": "env:TELEGRAM_BOT_TOKEN" } },
      { "kind": "web" }
    ],
    "schedules": [ { "title": "Morning report", "prompt": "…", "kind": "daily", "timeOfDay": "09:00" } ],
    "rules": [ { "pattern": "npm test*", "decision": "allow" } ]
} ] }
```

- Fields and validation are the Settings UI's (`mode` is one of ask/auto/review/plan/full;
  schedule kinds are once/interval/daily/weekdays with `intervalMinutes`, `timeOfDay`
  HH:MM or a `runAt` ms timestamp for `once`; secret names per channel kind: telegram and
  discord need `token`, slack `token` + `app_token`, whatsapp `access_token` +
  `signing_secret` and a `config.phoneNumberId`; web needs nothing).
- Secret values are inline or `"env:NAME"`, read from the environment at import — put the
  tokens in the systemd unit's `Environment=`/`EnvironmentFile=`, not in the file.
- Importing is idempotent: bots match by name (existing profiles are updated), channels
  by kind+label, schedules by title, and rules by pattern (an edited decision replaces
  the old rule) — re-applying converges, so keep the file in version control and
  re-apply after edits. A bot that fails is reported and skipped; serve exits non-zero
  until the file is clean. Token shapes are checked at import (a truncated or swapped
  token fails the import instead of dying later).
- Web channels print their chat links at import
  (`http://127.0.0.1:<port>/chat/<id>#<token>`). The chat server binds loopback only:
  reach it through an SSH tunnel (`ssh -L 47321:127.0.0.1:47321 host`).
- `--export-bots bots.json` writes every bot's configuration without secrets (channels
  list `secretsNeeded` names; web tokens are excluded — a re-import mints a fresh link;
  already-fired once-schedules are omitted, since their run time is gone). Use it to
  snapshot or move a server's setup; put the tokens back before re-importing.

Each `--import-bots` run now creates and verifies a secret-free configuration backup
before changing any bots. The backup path is printed, and a backup failure prevents
all imports. The new one-shot `--sync-bots bot-sources.json` command reads GitHub source
registries and exits after one sync attempt, without starting a Connect host.
It prefers an installed, authenticated GitHub CLI (`gh api`; configure with
`gh auth login`) and falls back to the existing HTTPS transport if `gh` is
missing or not signed in. Public sources need no authentication, and existing
headless environment tokens remain optional for private repositories. It
persists per-registry sync receipts in private application data rather than modifying
the source JSON, so a second unchanged run can skip backup and import. It does not
automatically poll; the original serve and sign-in flows are unchanged.

Schedules, rules, profiles and secrets are read fresh by a running serve; channels added
by hand (not through the import) need a restart to connect.

## Security

- The server dials the relay outbound (one WebSocket). No inbound ports, and the relay
  stores no task content: frames between your devices are end-to-end encrypted
  ([remote access](remote-access.md)).
- The server takes one of the account's 10 device slots; revoke it from any device's
  Remote access settings to cut it off instantly.
- It holds the worst a host can hold: provider CLI logins and project checkouts. Use a
  dedicated user, keep `secrets.json` (0600) and the app data dir private, and leave
  `--full-access` off unless you mean it.
- No new automatic requests beyond what Connect already makes on the desktop: the same
  relay socket, no update checks, no telemetry.

## Tasks after a crash

A task that was mid-run when the server restarted shows as interrupted; send it another
message (from any device) to resume. Queued messages survive and are delivered about ten
seconds after startup, unless their task is archived or still inside a usage limit (the
limit-resume send ends a run, which then delivers the queue). Cloud runs keep going on
their machines through server restarts.

## Maintenance checklist

Anything the frontend does after a run, on a timer, or on boot with real effects needs a
native twin in serve mode (`headless.rs`), or server behavior silently diverges:

- [ ] run-end side effects (the desktop's `done` handler in `main.ts`)
- [ ] timers with side effects (limit resumes live in `headless.rs`; turn checks are
      deliberately desktop-only)
- [ ] queue events (`queue-changed` emitters call `headless::queue_changed`)


## Headless SuperAgent preset administration (Extended)

The local operator can explicitly export or import the same portable
configuration-only companion manifest used by the desktop UI:

```sh
pandacode serve --export-orchestrations orchestrations.json
pandacode serve --preview-orchestrations orchestrations.json
pandacode serve --import-orchestrations orchestrations.json
```

Each command exits after the local operation. Import parses and preflights the
1 MB-bounded document, writes a verified private recovery snapshot before a
new preset is inserted, commits native SQLite changes transactionally, and
leaves every imported preset **Off**. No task starts, no credential or live
Ticket is imported, and no remote unauthenticated management endpoint is
opened. Repeating the same import is idempotent; headless errors produce a
non-zero exit code. This is configuration administration, **not** headless
task dispatch/approval parity; that remains a separate implementation gate.

### Read-only task inspection (local CLI)

```sh
pandacode serve --inspect-orchestrations
pandacode serve --inspect-ticket <ticket-id>
```

These one-shot local commands return bounded JSON operational metadata from the
existing native orchestration and Ticket queries. Ticket listing and inspection
include the state/revision, mandate revocation and policy version, assignment
IDs/statuses, and audit event kinds/timestamps. They deliberately exclude
objectives, message transcripts, worker results, rejection reasons, approval
identifiers and secrets. Unknown Ticket IDs and incompatible database schemas
fail with a nonzero exit code. Output is local stdout, not an unauthenticated
remote API.

Ticket inspection also computes a **read-only recovery assessment** from all
native running Assignments and all pending/uncertain effect receipts (not only
the truncated receipt history). The `recovery` object reports native
assignment consistency, mandate expiry/revocation, live-host uncertainty and
an actionable next-step code. `automaticReplayAllowed` is always `false`:
a running provider may still exist in another process; an uncertain remote
write requires authoritative provider reconciliation, not an inferred retry.
These observations do not create, admit, resume or approve a Ticket.

Unlike import/export or ordinary serve startup, inspection opens
`kingcode.sqlite3` **read-only before `setup_common`**. It neither starts bots
nor runs the Store's migration/restart recovery; inspecting a live host must
not mark a healthy worker as interrupted.

### Authenticated GitHub Observe from headless (local operator)

```sh
pandacode serve --observe-source <source-id>
pandacode serve --observe-check-run <source-id> <check-run-id>
```

An explicit, one-shot CLI invocation polls a previously configured **Observe**
source through the same authenticated native GitHub Events adapter, bounded
history validation, policy-revision recheck and transactional decision/cursor
persistence as the desktop command. It opens only the existing SQLite schema,
not the migration/restart recovery path, so a concurrently running host is not
interrupted. The command reports a redacted poll summary, never event bodies.
An Off source, stale revision, unsupported Check Run source, missing GitHub
authentication, incomplete Events window or API failure exits nonzero without
admitting a Ticket. For an explicit `check_run.completed` source, the separate
`--observe-check-run` command reuses desktop's authenticated Checks API
lookup, immutable result/commit SHA receipt, one-PR correlation, timestamp
and transactional policy guards. A missing, ambiguous, stale or changed Check
Run is rejected rather than treated as approval. Both commands return redacted
receipts and never expose event bodies. They do **not** register a subscription,
run continuously, enable Auto, launch a Worker, approve a Council review or
execute an external effect.

### Ticket review and stop commands

```sh
pandacode serve --list-tickets
pandacode serve --list-review-inbox
pandacode serve --list-team-demands <team-id> blocked
pandacode serve --accept-ticket <ticket-id> <expected-revision>
pandacode serve --reject-ticket <ticket-id> <expected-revision> "Needs changes"
pandacode serve --stop-ticket <ticket-id>
```

Accept and reject use the current Ticket revision; reject also requires a
nonempty reason. Stop revokes the Ticket mandate in the shared database. A
provider call already held in memory by a running serve process is not forcibly
terminated; its late completion is denied by the native state and mandate
checks. These commands do not create, retry, or resume tasks, and authenticated
event intake remains unavailable.

### Emergency Team disable (local operator)

```sh
pandacode serve --disable-orchestration <orchestration-id> <expected-policy-revision>
```

This one-shot local command uses the desktop's native revision-guarded policy
transition against the existing SQLite database. It atomically turns the Team
Off, revokes nonterminal Ticket mandates, interrupts active assignments,
releases native leases and preserves audit/history. It does **not** open
`Store::open`, run startup recovery, start a provider, re-enable a Team, or
create an external network management endpoint. A stale revision, missing Team
or incompatible schema produces an error without changing that Team.

A running provider may still have an in-memory call; the CLI cannot forcibly
terminate it, but subsequent SuperAgent-authorized tool admissions and late
callbacks are denied by native mandate/state checks. This is not process
termination or completed external-effect reconciliation. Use the inspection
commands above to verify the final native state.


### Cross-Team headless review inbox (read-only)

`pandacode serve --list-review-inbox` returns up to 200 actual pending
`waiting_approval` and `waiting_review` Tickets from the same native query as
Bots → Orchestrations. Its bounded JSON includes Team ID, Ticket ID, revision,
state, update time and a distinct next operator action, **not** objective,
private evidence, worker result, Mandate or review contents. `mayHaveMore` only
indicates that the cap was reached; it does not certify an additional page.
A pending native Council review is never represented as a pass. This command
opens existing SQLite read-only and cannot perform provider dispatch, recovery,
review acceptance, external effects or policy grants. Use a separate current
Ticket inspection before any explicitly authorized native human decision.

### Read-only Team Demand status inspection

Use `pandacode serve --list-team-demands <team-id> <state>` to inspect up to
200 actual native Tickets for one Team, filtered by an exact Ticket state
(`queued`, `running`, `waiting_approval`, `waiting_review`, `blocked`,
`completed`, `failed`, `stopped`, `interrupted`, `inconclusive`) or
`all`. JSON includes Ticket ID, state, revision and updated timestamp, but
not objectives, transcripts, worker results, mandates or authentication data.
`mayHaveMore` indicates the 200-row cap, not that the next page was verified.
When present, `nextCursor` supplies `beforeUpdatedAt` (milliseconds) and
`beforeId` (Ticket ID) for `--list-team-demands <team-id> <state> --before
<beforeUpdatedAt> <beforeId>`. Use the same Team and state for subsequent pages;
page reads are live, so a concurrently changing Ticket may move in the index.
The CLI rejects malformed or unpaired cursor values; without `--before`, the
existing first-page behavior is unchanged. This is observational only: it reuses the native state/Team filter before
pagination and opens SQLite read-only without running startup recovery,
granting permissions, admitting assignments or replaying external effects.

### Read-only Team readiness inspection

`pandacode serve --inspect-team-readiness <team-id>` opens the existing database
read-only without running startup recovery or interrupting the serving process.
It reuses the native manual coordinator/worker availability, policy revision,
Team project and checkout-occupancy checks. The JSON returns redacted role IDs,
current reasons for denial and the existing read-only tool/effect ceiling; it
never creates a Ticket, mandate, assignment, lease or provider call.

`eligibleAtSnapshot` is **not an authorization or admission guarantee**: the
one-shot CLI cannot observe every live in-memory provider session and cannot
reserve a checkout. `requiresLiveAdmissionRecheck` is always true. The real
native admission path must repeat its checks and may still reject the request.
No objective text, private evidence or authentication credential is printed.


### Inspecting durable external-effect receipts (read-only)

`pandacode serve --inspect-ticket-effects <ticket-id>` inspects up to 100
existing native receipt identities on the same local SQLite database without
running restart recovery against a healthy serve process. An unknown Ticket
or malformed ID exits nonzero. The native UI also offers **Inspect receipts**
under a Ticket's read-only inspector; neither surface can dispatch an effect.

Each entry includes an exact effect key, canonical payload SHA-256, persistent
`pending` / `uncertain` / `applied` state, and creation/update timestamps.
These are **local operator diagnostics**, so protect CLI output as operational
metadata. No payload, provider response or private conversation is returned.
`pending` may represent a live in-flight attempt; on a genuine process restart
native recovery changes it to `uncertain` (read-only inspection does not).
Never retry pending/uncertain receipts or mark them applied based on this
inspection alone. Independent authoritative remote reconciliation and a
separate explicitly authorized dispatcher are still required; this command
is not headless admission, resume or effect-dispatch parity.


### Unresolved external effects block final acceptance

Native manual and headless Ticket acceptance now share an atomic guard: if a
Ticket has any `pending` or `uncertain` external-effect receipt, it cannot
transition to `completed`, even when the worker has recorded evidence and a
human explicitly accepts it. No `human_accepted` audit or final reply is
written on denial. An operator must first reconcile the **authoritative
remote provider state**, including exact effect key and payload identity;
text-only model evidence and a timeout do not establish that a remote action
happened. A receipt marked `applied` by verified reconciliation no longer
blocks this particular guard. This does not enable a production external
dispatcher or bypass native Council-required completion checks.

### Revocation leaves external work explicitly uncertain

When native Stop, Team Off, watchdog interruption or bot reset/removal revokes
an active Ticket, any `pending` external-effect receipt is marked `uncertain`
in the same SQLite transaction. No provider call, retry, acknowledgment or
terminal approval is inferred. A failure during revocation rolls back the
receipt change as well as the mandate transition. Only independent
authoritative remote reconciliation can move an uncertain receipt to
`applied` after verification.


### Council-required Tickets never use legacy manual acceptance

When the worker's native Architect team has Council enabled, or its Ticket has
an immutable native Council review target, the existing Council-Off manual
Accept action refuses completion. A cached `passed` label, deleted Council team
or stale artifact never authorizes a terminal result. This is a protective
fail-closed guard, not a new reviewer invocation or exception workflow. The
native task-bound reviewer request/verdict and attributed, reason-bound human
exception remain unimplemented; Council-Off Tickets retain normal manual
acceptance.


### Cross-process Stop propagation to the serve-owned provider

`pandacode serve --stop-ticket <ticket-id>` is a separate one-shot CLI
process. It atomically revokes the native mandate and marks the Ticket stopped
in the shared SQLite database; the CLI cannot directly access a running
`serve` process's in-memory provider handle. The serve-owned native watchdog
now also reads **already stopped/interrupted** task assignments at each
30-second tick and calls the existing `stop_session` for any still-active
provider session belonging to that exact native assignment. Ordinary bot
sessions are never selected. A failed stop is logged and retried on a later
tick while the provider remains active; no Ticket or external-effect receipt is
replayed or reauthorized. The SQLite-side effect and callback gates apply
immediately, even before the asynchronous provider handle is terminated.

### Inspect linked GitHub source activity without Desktop

`pandacode serve --inspect-ticket-source <ticket-id>` returns up to 50 native
verified Observe receipt metadata entries for the Ticket's explicitly linked
repository/issue or PR thread. The query combines same-Team and same-repository
source classes without duplicating receipts or exposing another Ticket's
private objective. It never polls GitHub, issues grants, starts Workers,
replays effects, approves Council or invokes startup recovery against another
live serve process. An observed Check Run result is not acceptance evidence.

### Explicit Team rename with durable history

A trusted local operator can rename a disabled reusable Team without discarding completed Demands or editing their original admission policy:

```bash
pandacode serve --rename-orchestration <team-id> <expected-policy-revision> "New Team name"
```

The native write is a CAS-guarded, revisioned **name-only** update through the same SuperAgent policy handler as the desktop. It does not change Coordinator, Workers, Reviewers, project, task grants, history, provider sessions or external effects. Active/reviewable Tickets, running assignments and pending/uncertain external effects reject the action. The live database is opened without startup recovery, so a concurrent server's unrelated sessions are not interrupted. A repeated command with a stale revision fails closed. This is a local operator action, not a GitHub Auto trigger.

### Headless revocation of a GitHub Ticket context link

An authorized **local operator** can revoke an exact manually created source
association without opening the desktop workspace:

```sh
pandacode serve --unlink-source-thread <source-id> <ticket-id> issue:42
pandacode serve --unlink-source-thread <source-id> <ticket-id> pull:123
```

This one-shot operation uses the **same native transactional unlink** as the
Ticket inspector. IDs and typed keys must match, or the operation exits nonzero.
It atomically removes only that link and writes a durable Ticket audit entry.
Historical authenticated Observe receipts, source cursors, Mandates, Ticket
state and original evidence are never deleted or modified. Remaining explicit
links to the same Team/repository/thread continue to correlate that context.
The response is bounded JSON metadata (`unlinked`, identities,
`authorizationIssued: false`); it includes no private objectives or event
bodies. No GitHub call, provider admission, worker or automatic retry occurs.
The CLI opens the existing storage path without running process-start recovery
on a concurrently active headless host.

### Read-only multi-role Team inspection

Use `pandacode serve --inspect-team-roster <team-id>` for a bounded JSON
projection of the saved Coordinator, ordered Workers and optional Reviewers
while a desktop or headless service may still be active. The one-shot command
uses the existing native role tables and bot profiles via a **read-only
SQLite connection**; no startup recovery, provider, Council seat or grant is
created. It exposes configured provider/model/mode and assigned Skill IDs
without credentials, transcripts, Ticket objectives, effects or private tasks.
It does not perform a Skills Library scan: `skillLibraryStatus=not_scanned`,
`harnessCapability=not_verified`, `invocationEvidence=not_verified`.
`multiRoleExecutionAvailable=false` remains explicit; all execution
requires a separate task-scoped native authorization and live recheck.

### Read-only headless Ticket execution graph

Use `pandacode serve --inspect-ticket-graph <ticket-id>` to retrieve the same
bounded native Ticket/Assignment graph used by the Orchestrations workspace.
The result is JSON containing `graph.ticketId`, `graph.ticketRevision`,
`graph.nodes`, `graph.edges` and `graph.truncatedAttempts`. Up to the most
recent 50 durable assignment attempts are represented; the graph distinguishes
exactly 50 attempts from a truncated history.

This command is **observational only**: it opens SQLite read-only beside a
running headless or desktop host, does not run startup recovery, and never
admits a provider, replays external effects or grants permissions. Its graph
contains no objectives, tool outputs, worker results, private evidence or
transcripts. Assignment edges mean **Ticket owns attempt**, not a verified
dynamic subagent hierarchy. The response sets
`dynamicSubagentsRepresented: false` and
`councilVerdictIncluded: false`; a pending-review node is **not** a Council
pass. Native authorization, live state and independent review still require
the corresponding guarded operations.
