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
The read-only `--list-unresolved-effects` command pages exact pending/uncertain
receipt keys, payload SHA-256 digests, Ticket and Team IDs across all Tickets.
For a single reusable Team, use `pandacode serve --list-team-unresolved-effects <team-id>`
and page with `--before <updated-at-ms> <ticket-id> <effect-key>`. This filters
before pagination, rejects unknown/invalid Teams, and includes each Ticket's
current `ticketRevision` for a subsequent revision-guarded operator action.
Neither view returns private objectives, verifies remote outcomes, or issues
a grant; re-inspect the Ticket and receipt before any consequential decision. Use
`--before <updated-at-ms> <ticket-id> <effect-key>` from `nextCursor`
to inspect older identities, including terminal Tickets outside the recovery
inbox's first page. The snapshot never queries the remote provider, acknowledges,
retries or replays effects; current provider state must be reconciled separately.
Cursor timestamps are mutable when a receipt changes state, so re-list from the
first page after concurrent reconciliation. An unrelated applied effect is omitted.

For Team-specific recovery triage, use
`pandacode serve --list-team-recovery-inbox <team-id>` and continue with
`--before <priority> <updated-at-ms> <ticket-id>` from `nextCursor`.
The Team filter applies within the native SQLite recovery candidates **before**
the 200-item page limit and cursor validation. An unknown Team or cursor from
another Team is rejected, not silently treated as an empty authorized result.
This is read-only even beside a live server; no provider, mandate, approval,
automatic retry or external action is performed.

The existing `--inspect-ticket` and `--inspect-ticket-effects` commands show
redacted state without touching live sessions. Per-effect dispatch and full
headless admission/retry still require explicit user-authorized native grants.

### Quarantine one exact pending remote-effect receipt

A local headless operator may quarantine a pending receipt without treating an
unknown remote outcome as failed or applied. Inspect the current Ticket revision
and the receipt's exact effect key and lowercase SHA-256 digest first:

```sh
pandacode serve --inspect-ticket <ticket-id>
pandacode serve --inspect-ticket-effects <ticket-id> --unresolved-only
pandacode serve --quarantine-pending-effect <ticket-id> <expected-revision> <effect-key> <payload-sha256>
```

The native transaction verifies the current Ticket revision and exact stored
payload identity, marks **pending → uncertain**, and records one durable audit
entry together with the state change. Stale revisions, mismatched identities,
applied receipts and audit-write failures leave the receipt untouched. Repeating
the command on the same uncertain identity reports `changed: false` and
does not duplicate evidence. It never calls the provider, grants permissions,
retries the remote operation, acknowledges success, or changes task completion.
The provider outcome remains unknown until separately verified and reconciled.
A concurrent provider call may still be in flight; quarantining the receipt
does not stop that provider.

### Quarantine a running Ticket's detached native session

When recovery inspection reports one running native Assignment whose `sessions`
record is missing, a local operator can explicitly quarantine that anomaly at
the current Ticket revision:

```sh
pandacode serve --list-recovery-inbox
pandacode serve --quarantine-detached-ticket <ticket-id> <expected-revision>
```

The transaction requires exactly one linked running Assignment with a missing
session record. It revokes its Mandate, interrupts the Ticket and Assignment,
marks pending external effects `uncertain`, increments the Ticket revision
and records an audit. Healthy sessions, multiple attempts, stale revisions
and audit failures abort without a partial transition. The CLI does **not**
stop a live external provider, launch work, acknowledge a remote effect or
allow automatic replay. Uncertain effects need independent reconciliation.

### Quarantine an orphaned running Ticket

A legacy or corrupted native store can leave a Ticket marked running with no
active Assignment link, a lost Team, or zero/multiple running Assignments.
An authorized local operator can safely quarantine this exact anomaly:

```sh
pandacode serve --list-recovery-inbox
pandacode serve --quarantine-orphan-ticket <ticket-id> <expected-revision>
```

The native revision-checked transaction interrupts the Ticket and all surviving
running Assignments, revokes its Mandate, makes pending external effects
`uncertain`, and records an audit. Healthy Tickets, stale revisions and
duplicate quarantine calls are denied; an audit failure rolls back the
transaction. No provider or effect is replayed or presumed stopped. The
owning live host must cancel the revoked provider; uncertain remote outcomes
require independent reconciliation before retry.

### Quarantine an expired native Assignment

When the read-only recovery inbox identifies a running Ticket whose current
Assignment exceeded its 30-minute native execution budget (or has an invalid
clock), a local operator can explicitly revoke it using the exact current
Ticket revision:

```sh
pandacode serve --list-recovery-inbox
pandacode serve --quarantine-expired-ticket <ticket-id> <expected-revision>
```

This rechecks the native Assignment clock and Ticket/Assignment identity in one
SQLite transaction, revokes the Mandate, interrupts the Assignment, and marks
pending external receipts uncertain. Healthy attempts, stale revisions, missing
or multiple running Assignments are rejected without partial writes. The command
does **not** stop a live provider, assert any remote result, or authorize a
retry. A running serve host must observe the revoked task and cancel its provider
handle; independently reconcile any uncertain remote effect first.

### Quarantine a terminal Ticket's orphaned assignment

The read-only recovery inbox may find a terminal Ticket with a stranded native
running Assignment after a legacy/partial state transition. A trusted local
operator can explicitly quarantine the anomaly with the current Ticket revision:

```sh
pandacode serve --list-recovery-inbox
pandacode serve --quarantine-terminal-orphan <ticket-id> <expected-revision>
```

The native transaction revokes the Mandate, interrupts the stray Assignment,
turns pending external-effect receipts uncertain, clears the stale active link,
increments the Ticket revision and records one audit event. Terminal state and
evidence are preserved. A stale revision, live Ticket or duplicate quarantine
is denied. The detached CLI does not stop a provider in another process; the
owning serve watchdog must terminate it. This operation cannot launch work,
acknowledge a remote write or authorize effect replay. Independently reconcile
any uncertain remote effect before another authorized action.

### GitHub source cursor diagnostics

Use the trusted local headless inspector to examine a configured GitHub source
without polling GitHub, granting tools or interrupting another active server:

```sh
pandacode serve --inspect-source-health <source-id>
pandacode serve --inspect-source-decisions <source-id>
pandacode serve --inspect-source-decisions <source-id> --page
pandacode serve --inspect-source-decisions <source-id> --before <sequence> <delivery-id>
```

The legacy decision inspector returns an array containing the latest 50
immutable receipts. Use `--page` for a bounded page with `decisions` and
`nextCursor`, then pass the previous page's exact `sequence` and `deliveryId`
to `--before`. The source-local keyset order is descending by sequence and
delivery ID, so duplicate sequence values never skip or repeat receipts.
The optional cursor does not grant execution, poll a remote source, or expose
untrusted GitHub bodies; unknown sources and malformed cursors are rejected.

The read-only response reports Off/Observe mode, policy revision, receipt count,
permitted receipt count, stored cursor and highest immutable receipt sequence.
For GitHub Events the cursor is a verified event-ID high-watermark; for explicit
Check Runs it is a **local observation counter**, not an upstream event ID.
Neither value proves a complete provider subscription. When receipt capacity is
exhausted, operator intervention is necessary. If the cursor is behind durable
receipts, native Observe fails closed pending reconciliation: it does not reset
the cursor, delete receipts or reclassify historical observations. The inspector
redacts event bodies, actor filters and task content, uses no provider credentials
and never runs startup recovery.

### Edit a disabled Team's roster and attempt budget headlessly

The trusted local operator can change an existing Team's ordered Workers,
optional Reviewers, or retry budget without opening the desktop. These commands
call the **same native SQLite revision-guarded policy transactions** as the
desktop editor. First disable the Team and settle all Demands and external
receipts; inspect the latest revision before every edit:

```sh
pandacode serve --inspect-team-roster <team-id>
pandacode serve --set-team-roster <team-id> <expected-policy-revision> /private/roster.json
pandacode serve --set-team-attempt-limit <team-id> <new-policy-revision> 3
```

The private local UTF-8 roster file (maximum 16 KiB) must contain a complete
existing-bot roster using the portable JSON field names, for example:

```json
{"workerBotIds":["existing-worker-id"],"reviewerBotIds":[]}
```

The original first Worker must remain first under this version of the native
roster contract. Additional Workers and Reviewers are permitted as **configuration
only**: native multi-member dispatch is still unsupported, so the command reports
`executionReady: false` rather than attempting work. Neither command creates
bots, grants tools, activates Council, changes old Demand snapshots, starts a
provider or retries a Ticket. An invalid member, disabled/missing bot, duplicate
role, stale revision, enabled Team, running/reviewable Demand or unresolved remote
effect fails the native transaction. Roster output contains counts rather than
bot profiles; retain the source file in an appropriately private directory.

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

For a Team with several configured Workers and no Reviewers, a local
operator can stage an exact selected Worker rather than implicitly choosing
the first Worker:

```sh
pandacode serve --queue-worker-demand <team-id> <worker-bot-id> <stable-request-key> /private/objective.txt
```

The selected Worker must be an active member of the enabled Team and share
the currently supported native project scope. The command captures the entire
ordered Team roster and the exact Worker identity in the immutable Ticket,
without creating a provider session, acquiring an execution lease or granting
any tools. A stable key cannot be reused to select a different Worker. The
existing Desktop native retry is a separate explicit action that rechecks the
admitted roster, profile, Mandate expiry and physical checkout occupancy.
Multi-project work, configured Council Reviewers and automatic delegation
remain unsupported on this path; these constraints must not be bypassed by
a detached CLI.

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
leaves every imported preset **Off**. Preview and export use an existing-schema,
read-only connection; import uses the existing native write connection without
Store startup recovery. They can run beside a healthy headless host without
interrupting its live Tickets or changing pending external-effect receipts.
An absent or outdated schema fails closed rather than running migrations.
No task starts, no credential or live Ticket is imported, and no remote
unauthenticated management endpoint is opened. Repeating the same import is idempotent; headless errors produce a
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
pandacode serve --create-source <team-id> <owner/repo> <actor> <event-kind>
pandacode serve --list-team-sources <team-id>
pandacode serve --set-source-mode <source-id> <expected-policy-revision> observe
pandacode serve --set-source-mode <source-id> <expected-policy-revision> off
pandacode serve --observe-source <source-id>
pandacode serve --observe-team-sources <team-id>
pandacode serve --observe-check-run <source-id> <check-run-id>
pandacode serve --observe-linked-pr-checks <check-source-id> <ticket-id> pull:123
```

The local operator may create an exact source for an existing Team without the
desktop. Creation uses the same native source registry as the UI, starts in
`off`, rejects unsupported event classes and cross-Team IDs, and returns the
source ID/policy revision. Repeating an equivalent configuration preserves the
existing source ID, enabled state, cursor and receipts. `--list-team-sources`
inspects one Team's source metadata without polling GitHub or running startup
recovery. Source creation is configuration only, not an authenticated event
subscription or a request to run a Demand.

The local operator can first enable or revoke an existing exact source using
`--set-source-mode`. This reuses the desktop's native optimistic policy
revision guard; the expected revision is available from
`--inspect-source-health`. A stale revision or unsupported `auto` mode
fails without changing policy. Disabling a source is durable across restarts
and prevents in-flight Observe requests from committing receipts after a
policy change. Changing mode performs no GitHub request, grants no worker
permissions and does not create or run a Demand. Existing source receipts
and cursors are preserved for audit and safe reactivation.

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


For a PR explicitly linked to an existing Ticket under the same enabled Check
Run source, \`--observe-linked-pr-checks\` verifies the repository's numeric ID,
the PR base identity and exact head revision via authenticated \`gh api\`, then
reads at most 100 Check Runs for that head. Link the exact \`pull:123\` key
first using \`--link-source-thread <check-source-id> <ticket-id> pull:123\`.
Only completed runs matching the configured GitHub App slug and a unique exact
PR association are persisted through the existing immutable, revision-guarded
Observe receipt path. Pending runs are counted but not marked as passed.
A result set larger than 100, a truncated page, ambiguous PR association or
head change is denied rather than claimed complete. The command never infers
Issue→PR causality, approves a Ticket, launches workers or issues tool grants.
Receipts are committed independently and deduplicated; if a later Check Run
fails verification, earlier committed receipts remain inspectable and a new
operator-initiated invocation safely deduplicates them. This is a bounded
snapshot, **not** continuous CI subscription, a complete quality gate, or a
Council verdict.

The explicit `--observe-team-sources` command processes every enabled GitHub
Events source configured for one Team using the same individual authenticated
adapter and native policy checks. Off sources and Observe Check Run sources
are skipped without making requests; Check Runs still require an exact ID
through `--observe-check-run`. Each source commits its own immutable receipts
and cursor. On a provider error or source-policy change, the batch exits
nonzero; earlier successfully committed source observations are retained.
Re-running is idempotent for unchanged provider delivery IDs, and there is no
batch-wide promise of atomic rollback or a gap-free live subscription.
This command does not register a background poller or enable Auto.

Activated GitHub Events Observe sources are also checked by a running desktop
or `pandacode serve` host every five minutes, up to twelve sources per pass.
The host rotates across Teams in stable source order, and source revocation
is checked both before the GitHub request and when storing each receipt.
A process restart restarts the scan; durable delivery IDs deduplicate previously
recorded events. An Off source makes no GitHub request, and Check Runs still
require an exact explicit ID. This bounded polling is **not** a guarantee of
gap-free GitHub event history or a grant for Auto, tools or external writes;
a missing historical boundary fails closed and requires operator diagnosis.
The one-shot CLI commands above retain their explicit, exit-after-poll behavior.

### Ticket review and stop commands

```sh
pandacode serve --list-tickets
pandacode serve --list-ticket-activity <ticket-id>
pandacode serve --list-ticket-activity <ticket-id> --before <audit-id>
pandacode serve --list-team-member-activity <team-id> <bot-id>
pandacode serve --list-team-member-activity <team-id> <bot-id> --before <created-at-ms> <assignment-id>
pandacode serve --list-review-inbox
pandacode serve --list-review-inbox --before <updated-at-ms> <ticket-id>
pandacode serve --list-team-review-inbox <team-id>
pandacode serve --list-team-review-inbox <team-id> --before <updated-at-ms> <ticket-id>
pandacode serve --list-recovery-inbox
pandacode serve --list-team-demands <team-id> blocked
pandacode serve --accept-ticket <ticket-id> <expected-revision>
pandacode serve --reject-ticket <ticket-id> <expected-revision> "Needs changes"
pandacode serve --stop-ticket <ticket-id>
pandacode serve --stop-ticket-at-revision <ticket-id> <expected-revision>
```

The Ticket activity command pages a durable audit stream in descending event-ID order
(100 entries per page). Use the response's `nextCursor` with `--before` while
`hasMore` is true. Each invocation is a new read-only snapshot. It shows only
event ID, controlled event kind and timestamp; private reasons, objectives,
provider output, secrets and evidence are excluded. It neither authorizes
execution nor runs startup recovery beside another live host.

Team member activity uses the same native page handler as Bots → Orchestrations:
100 assignment records per page, ordered by immutable creation time and ID.
The `nextCursor.createdAt` and `nextCursor.assignmentId` pair must both be
provided to continue. Cursors from another Team or bot, or fabricated
boundaries, are rejected. The CLI uses a read-only SQLite connection, returns
assignment status and session metadata without objective/result text, and never
starts a provider, modifies audit records, acknowledges effects or performs
startup recovery.

Accept and reject use the current Ticket revision; reject also requires a
nonempty reason. Use the revision reported by `--inspect-ticket` or the recovery
inbox for `--stop-ticket-at-revision`; a stale observation fails without state,
mandate or audit changes. Unconditional `--stop-ticket` remains available for
emergency revocation. Stop revokes the Ticket mandate in the shared database. A
provider call already held in memory by a running serve process is not forcibly
terminated; its late completion is denied by the native state and mandate
checks. These commands do not create, retry, or resume tasks, and authenticated
event intake remains unavailable.

### Explicit Team enable (local operator)

```sh
pandacode serve --enable-orchestration <team-id> <expected-policy-revision>
```

This local operator action uses the same native revision-checked eligibility
checks as the desktop Team enable control. It rejects paused bots,
unsupported harness profiles, configured multi-member execution that lacks native
scoped admission, and stale revisions. Enabling makes an eligible Team available
for **future explicitly submitted manual Demands only**. It creates no Ticket,
Mandate, provider session, GitHub Auto subscription or external effect.
A repeated enable with the same current revision is a no-op.

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

`pandacode serve --list-review-inbox` returns pages of up to 200 actual pending
`waiting_approval` and `waiting_review` Tickets from the same native query as
Bots → Orchestrations. Its bounded JSON includes Team ID, Ticket ID, revision,
state, update time and a distinct next operator action, **not** objective,
private evidence, worker result, Mandate or review contents. When `mayHaveMore`
is true, use `nextCursor.updatedAt` and `nextCursor.ticketId` in
`pandacode serve --list-review-inbox --before <updated-at-ms> <ticket-id>`.
The pair preserves reviews with identical timestamps across pages; missing or
invalid cursor values are rejected. `mayHaveMore` checks an actual subsequent
native row instead of assuming another page exists whenever 200 rows are found.
Each page is a read-only snapshot, not a frozen history while Tickets change.
A pending native Council review is never represented as a pass. This command
opens existing SQLite read-only and cannot perform provider dispatch, recovery,
review acceptance, external effects or policy grants. Use a separate current
Ticket inspection before any explicitly authorized native human decision.

### Team-scoped native review inbox (read-only)

`pandacode serve --list-team-review-inbox <team-id>` uses the same actual
pending decision states as the global inbox, but filters by the native Team
**before** applying its 200-row limit. Pass the returned
`nextCursor.updatedAt` and `nextCursor.ticketId` with `--before` to page.
Unknown Teams, foreign-Team cursors, stale/non-pending anchors and malformed
timestamps or IDs fail explicitly rather than silently skipping reviews. The
output includes a Team ID and redacted Ticket revision, state and next action.
It cannot invoke Council, prove reviewer approval, start recovery, execute a
provider, issue grants or accept Tickets. Re-inspect the current native
revision before an explicitly authorized decision.

### Cross-Team recovery and reconciliation inbox (read-only)

`pandacode serve --list-recovery-inbox` returns a native SQLite
snapshot of up to 200 Tickets needing a decision or recovery investigation:
queued, blocked, review/approval-pending, failed/interrupted/inconclusive,
expired or inconsistent running assignments (including assignments whose
original provider session is missing), terminal Tickets with orphan running
assignments, and terminal Tickets with still-unresolved external-effect
receipts. The current single-active-assignment contract also
flags duplicate running attempts even when one active link looks valid.
Healthy running assignments and genuinely settled terminal Tickets are excluded. Unresolved effects are prioritized; the
returned JSON includes Ticket/Team IDs, revision, state, a redacted native
recovery assessment, `mayHaveMore`, and an explicit next operator action.
A page is limited to 200 Tickets. When `mayHaveMore` is true, `nextCursor`
contains the last returned `priority` (0 = unresolved remote effects,
1 = live/inconsistent assignment, 2 = other decisions), `updatedAt` in
milliseconds, and `ticketId`. Request the next page with
`pandacode serve --list-recovery-inbox --before <priority> <ms> <ticket-id>`.
Cursor ordering preserves older unresolved effects that would otherwise be
hidden by the first-page cap. Each invocation is a new read-only snapshot:
concurrent state changes may reorder candidates, so do not treat several pages
as an atomic audit or as authorization to act.

The command deliberately does **not** run startup recovery, inspect a live
provider handle, resend/acknowledge a remote write, resume an assignment,
issue grants, invoke Council or change Ticket state. An expired assignment is
reported for investigation, not silently terminated by an inspector. A native
operator must separately verify the current revision, server state and remote
effect receipt before any authorized action. Sensitive objectives, results,
credentials, effect keys and private audit details are not returned.

### Read-only global Demand queue

Use `pandacode serve --list-demands <state>` to page native Ticket status
across all Teams, with `all` or an exact native state. Append
`--before <beforeUpdatedAt> <beforeId>` and optionally
`--search "objective fragment"` and/or `--project <admitted-project-id>`
to filter before the 200-item page limit. Search and project may appear in
either order following the optional cursor. The Project filter uses each
Ticket's immutable admitted project, not the Team's current project; this
supports archived Project IDs. IDs are bounded ASCII alphanumeric, hyphen or
underscore, and the same filters must be retained across cursor pages.
Results include each Ticket ID, Team ID, state, revision, timestamp and
`nextCursor`; objectives, search terms, actors, sessions, mandates and evidence
are withheld. The query opens SQLite read-only and cannot launch providers,
change authorization or perform restart recovery. Cursor pages are live
snapshots rather than one long transaction; concurrently changing Tickets
may move between pages. Avoid sensitive CLI search phrases in shell history
and process arguments.

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
existing first-page behavior is unchanged. To search before pagination, append `--search "objective fragment"` after the
optional `--before` cursor. The literal substring filter is
ASCII-case-insensitive and limited to 128 UTF-8 bytes. JSON includes
`searchApplied` but never returns search phrases or matching objectives.
Avoid sensitive phrases in CLI arguments, since process listings and shell
history may expose them. Omitting `--search` preserves the previous view.
Add `--project <admitted-project-id>` to filter using the original Project ID
recorded on a Ticket, even if the Team was later reconfigured. Search and
project can be supplied in either order following the optional `--before`.
Unknown projects return an empty page, while invalid IDs fail closed.
`projectFilterApplied` indicates use of a Project filter without echoing its
value. Omitting `--project` retains the legacy native query path.
This is observational only: it reuses the native state/Team filter before
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

`pandacode serve --inspect-ticket-effects <ticket-id>` inspects the first 100
native receipt identities. Follow the response's `nextCursor.createdAt`
and `nextCursor.effectKey` to page the complete history:

```bash
pandacode serve --inspect-ticket-effects <ticket-id> --before <created-at-ms> <effect-key>
pandacode serve --inspect-ticket-effects <ticket-id> --unresolved-only
```

The second form filters out `applied` receipts; both flags can be combined.
A compound (timestamp, key) cursor prevents skipping records created in the
same millisecond. This is keyset pagination, not a frozen snapshot: concurrent
effect reconciliation may change unresolved-only results between pages.
The unfiltered `totalUnresolved` remains authoritative across every page.

All reads use the same local SQLite database without restart recovery against
a healthy serve process. An unknown Ticket, invalid cursor or malformed ID
exits nonzero. The native UI also offers **Inspect receipts** under a Ticket's
read-only inspector; neither surface can dispatch an effect.

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

`pandacode serve --stop-ticket <ticket-id>` (or
`--stop-ticket-at-revision <ticket-id> <expected-revision>`) is a separate one-shot CLI
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

### Headless Team creation (native, disabled)

A trusted local operator can create a reusable Team from **existing local bots**
without opening a Desktop Webview. Save a private UTF-8 JSON file (maximum 16 KiB):

```json
{
  "config": {
    "name": "Release Engineering",
    "coordinatorBotId": "existing-coordinator-id",
    "workerBotId": "existing-first-worker-id"
  },
  "roster": {
    "workerBotIds": ["existing-first-worker-id", "existing-second-worker-id"],
    "reviewerBotIds": []
  },
  "maxWorkerAttempts": 2,
  "purpose": "Deliver authorized repository changes",
  "strategy": "balanced"
}
```

```sh
pandacode serve --create-orchestration /private/team.json
```

The first Worker must match `config.workerBotId` for compatibility with older
native Team records. `maxWorkerAttempts` defaults to 2; `purpose` defaults
to empty and `strategy` to `balanced`. An empty reviewer set is permitted.
Unknown fields (including `enabled`, `allowedTools` and permissions) are
rejected, not silently applied. The command checks that all selected bots are
eligible and that the full roster and policy are valid **in one native SQLite
transaction**. Invalid bots, duplicate roles or unsupported budgets leave no
partial Team. Existing bots are neither created nor modified.

New Teams always start **disabled** with no Ticket, Mandate, Assignment,
source subscription, Council verdict, tool permission or provider launch.
The output contains only Team identity, policy revision, role counts, strategy
and explicit false authority flags; it does not echo the private purpose or bot
instructions. Enable the Team separately after inspecting its policy. This
command does not run startup recovery against an active serve process.

### Headless Team strategy (future Demands only)

A trusted local operator can update a disabled Team's scenario purpose and
optimization strategy without launching workers, changing permissions or rewriting
historical Demand snapshots. Store the purpose in a private local UTF-8 file:

```sh
pandacode serve --set-team-strategy <team-id> <expected-policy-revision> economy /private/purpose.txt
```

Valid strategies are `balanced`, `economy`, `performance`, `security_critical`
and `custom`. The purpose is limited to 400 characters. This command reuses
the **same native CAS and transaction as the desktop Team editor**: the Team
must be disabled, and active/reviewable Demands or unresolved effects prevent
the update. A stale revision or invalid strategy fails without a partial write;
the command never issues grants, selects a different model, or dispatches a
provider. Earlier admitted Tickets retain their immutable strategy snapshots.
The CLI opens the existing SQLite database without startup recovery, protecting
concurrent headless sessions from an unrelated interruption. Its JSON result
reports `purposeConfigured` but never echoes the private purpose file content;
the saved value remains inspectable through authorized native Team configuration.

### Headless operator association of a GitHub Ticket context link

The authorized local operator can add a context-only source link using the same
native transaction as the Desktop Ticket inspector:

```sh
pandacode serve --link-source-thread <source-id> <ticket-id> issue:42
pandacode serve --link-source-thread <source-id> <ticket-id> pull:123
```

This command validates the exact Team/Ticket/source/thread relationship and
repository identity ownership, writes one durable audit row, and is idempotent.
It is not evidence that GitHub linked the issue and PR; use the separate
explicit issue-to-PR association when required. Headless operators can make
that same native, audited association after explicitly linking the Issue:

```sh
pandacode serve --link-source-thread <issue-source-id> <ticket-id> issue:42
pandacode serve --link-issue-pull <pr-source-id> <ticket-id> issue:42 pull:123
```

For an optional authenticated GitHub object-existence check, use:

```sh
pandacode serve --verify-link-issue-pull <pr-source-id> <ticket-id> issue:42 pull:123
```

This one-shot command uses the signed-in `gh api` identity, verifies the exact
Issue and PR number plus immutable base-repository ID, and rechecks the GitHub
account before committing an operator-asserted native link. Its Issue context
link must already exist with a matching attested repository identity. A changed
source revision, deleted/recreated repository, missing object or network/auth
error cannot produce a new association. The receipt distinguishes
`githubObjectsVerified: true`, `authenticatedRelationship: false` and
`authorizationIssued: false`. Object existence is **not** proof that the
PR genuinely resolves the Issue. No providers, grants, Auto work, reviews,
external writes or new subscriptions are started.

The Issue and PR source IDs must resolve to the same Team, Ticket, and
repository identity. A conflicting, missing or terminal Ticket, unlinked
Issue, or wrong key is rejected atomically. Replays are idempotent. The JSON
receipt explicitly marks `operatorAsserted: true`,
`authenticatedRelationship: false` and `authorizationIssued: false`.
No GitHub API call, provider session, task authority or grant is issued. No provider or GitHub request,
Ticket admission, authorization grant or remote effect occurs. Invalid,
ambiguous, cross-Team, terminal or conflicting links fail before the native
transaction commits. The bounded JSON reports `linked` and
`authorizationIssued: false`; no private objectives or event bodies are
returned. A concurrent healthy host is not interrupted by startup recovery.

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


### Preflight an exact headless Demand without admission

```sh
pandacode serve --preflight-demand <team-id> direct /path/to/objective.txt
pandacode serve --preflight-demand <team-id> coordinated /path/to/objective.txt
```

This operation reads the bounded (maximum 16 KiB) UTF-8 objective and calls the
existing native manual policy preflight for the selected Worker or Coordinator.
It returns policy revision, selected role, eligibility at the database snapshot,
a bounded attempt budget and a reason when blocked. It **does not print the
objective**, issue a Mandate, reserve a session/checkout, dispatch a provider,
or modify Ticket history. A separate headless process cannot attest currently
running in-memory sessions; `eligibleAtSnapshot` is **not** permission or a
reservation. Explicit native admission must independently check live provider
occupancy, permissions, revision and leases. The existing `--queue-demand`
continues to create a blocked manual Ticket without automatically executing it.

### GitHub Observe history diagnostics

Inspect an enabled source using `--inspect-source-health <source-id>` after a bounded GitHub Events poll fails. Source health now includes a durable `historyGapCursor`, `historyGapReason` (`cursor_missing` or `window_unverified`), `historyGapDetectedAt` and `historyGapOccurrences` when a previously stored event cursor could not be verified within the authenticated, bounded feed. These observations are diagnostic, not proof of which events were missed; the warning remains after restart and source mode changes until a separately verified reconciliation is available. No cursor is rewound, no missing events are invented, and no Auto work or effects are admitted. Transient authentication, network and provider-order failures are not classified as a history gap.
