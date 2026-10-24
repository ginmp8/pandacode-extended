> Work/evidence note. Consolidated test results are in ../SECURITY-HARDENING.md and verification.json.

# PandaCode 0.3.28 — Remote Access and Channel Review

## Scope and Base

Original source: `audit/original/pandacode-0.3.28`, extracted from the supplied ZIP; ZIP SHA-256 `4d74c67ab175967fa247852cd8d678e3908cb51de9d5a9292b1dd5dc6d500836`.
Candidate: `audit/pandacode-0.3.28-hardened`.
Reviewed files: `remote.rs`, `remote/*`, `relay/src/*`, `bots/channels/*`, `bots/gateway.rs` (admission/send interfaces), `providers/mcp_host.rs`, `local_servers.rs`, and related configuration/documentation.
Line numbers below refer to the original. The original application was not run, samples were not detonated, and private content was not sent over the network. Isolated proofs execute only pure functions extracted from source files.

## Fixed Findings

### R-01 — High: Remote Authorization Remained Valid After Revocation

**Evidence:** `src-tauri/src/remote/socket.rs:205-237,292-297` queued `(message, key, trusted: bool)`. `remote/host.rs:176-188` checked this captured boolean without rechecking the current pin. The sequential queue may wait for other calls. `remote/start.rs:163-215` discovers providers, clones, and creates a worktree before starting the agent, without another authorization check.

**Failure:** A previously trusted device could continue executing queued requests after the user removed its trust or the key changed. Slow preparation could start a new execution after revocation.

**Fix:** The queue carries the public key that authenticated the message. Execution checks the current list and pin; slow preparation checks again before new actions and before launching the agent. Revocation does not undo actions already performed.

**Test:** `remote::tests::queued_messages_recheck_the_authenticated_key_after_revocation_or_replacement` covers removing trust, changing the listed key, trusting a new key, and removing a device. The full Rust test must be included in overall project validation.

### R-02 — High: Key-Swap Window and Delivery After Revocation

**Evidence:** `remote/socket.rs:402-405` called `callable()` and then fetched a key through `peer_key`, but ignored the second result's `trusted` status. Presence could change between these operations. `answer` sent results after an async call using a previously obtained key, without rechecking trust/hosting; events extracted for sending also did not recheck `host_enabled` at the final point.

**Failure:** In a race, task content could be encrypted to a replaced, untrusted key, or continue to a recipient whose trust was just revoked. Requires configured Connect and a revocation or relay-control/compromise scenario; this is not anonymous remote access by default.

**Fix:** The same public key used for encryption must remain listed and pinned when the frame is added to the outgoing queue, with state protected by the mutex. Host responses/events also require hosting to remain enabled. After revocation, only a generic refusal may be sent to the old key, without task content.

### R-03 — Medium: Overflow Bypassed Replay Window

**Evidence:** `remote/host.rs:65` used `(now_ms - ts).abs()` on `i64`. With `ts = now.wrapping_add(i64::MIN)`, subtraction/absolute value can remain negative in optimized builds without overflow checks; the timestamp test accepts it. The old receipt is eligible for immediate pruning during nonce cleanup.

**Failure:** An artificial timestamp in an authenticated message can bypass the ±60-second window. Builds with overflow checks panic. This does not allow forging device encryption/authentication.

**Fix:** Use unsigned `abs_diff` and saturating subtraction for retention. Final implementation is in `remote/replay.rs` and persists admission before dispatch, as described in R-08.

**Proof run:** The extracted original function, compiled with `rustc --test -C opt-level=2 -C overflow-checks=off`, fails the test requiring rejection of the timestamp; the fix passes. A debug-build proof also exists. Evidence: `audit/remote-regression/original-release-tests.log` and `hardened-release-tests.log`.

### R-04 — Medium: Disabling a Channel or Revoking a Link Did Not Stop All Pending Delivery

**Evidence:** `bots/channels/mod.rs:330-340` sent messages without checking `channel.enabled`. `bots/channels/web.rs:134-165` authorized once and waited up to 25 seconds before returning events, without rechecking token, enabled state, and tunnel access permission.

**Failure:** An agent reply finishing after disable could still go to Telegram/Slack/Discord/WhatsApp. A poll authenticated before link renewal could receive new responses after revocation.

**Fix:** Check enabled state before each sent part; poll revalidates token, channel, and remote access before every read/delivery. Gateway admission is handled separately by the agent responsible for secrets/egress.

### R-05 — Medium: WhatsApp Webhook Lacked Replay Protection and Number Binding

**Evidence:** `bots/channels/whatsapp.rs:26-45,67-75` verified body HMAC but discarded `message.id`, `timestamp`, and `metadata.phone_number_id`. Every valid delivery started work again.

**Failure:** Replaying a signed webhook, including provider retry, could repeat commands or approvals. If a Meta app served multiple numbers with the same secret, one number's body could be accepted by another number's channel. Requires an explicitly configured WhatsApp channel and signed message; this is not absence of public authentication.

**Fix:** Check channel number; require valid ID and timestamp; persist receipt for ID/channel in existing `bot_activity` before dispatch. Read/write use the Store mutex. Receipts survive database reopen. Reject timestamps older than 24 hours or more than 5 minutes in the future. A reset marker invalidates messages from before the user erased bot history. At 4,000 or more recent events, admission fails closed before the 5,000-record retention limit could erase evidence. No schema migration was added.

**Tradeoffs:** Very delayed commands are ignored; high volume may receive HTTP 503 for retry. The guarantee is at-most-once admission, not exactly-once transactional execution: a crash between receipt persistence and starting work can lose a delivery. Already executed effects are not undone.

**Tests added:** reopened database and duplicate ID; age/future/reset; capacity; number binding, missing ID, and timestamp overflow. The existing HMAC/payload test was updated with valid metadata.

### R-06 — Medium/Robustness: Network Limits Applied Too Late and Bot Connections Were Unbounded

**Evidence:** `remote/auth.rs:91-97` called `.bytes()` before checking 1 MiB; socket used library defaults rather than the protocol's 256 KiB ceiling. `bots/channels/http.rs:182-199` created one task per connection for up to 40 seconds without a concurrency limit.

**Failure:** A compromised endpoint/response without Content-Length could consume memory before rejection; many local clients or clients through a configured tunnel could accumulate tasks. No code execution was demonstrated.

**Fix:** Read the response in chunks and stop before exceeding 1 MiB; cap WebSocket message/frame at 256 KiB; limit bot-server concurrency to 64 connections.

### R-07 — Low/Defense in Depth: HTTP Parser Accepted Ambiguity

**Evidence:** `bots/channels/http.rs:105-121` accepted duplicate headers using the last value; it did not reject `Transfer-Encoding`/`Expect`, ambiguous versions/start lines. Header limit could be exceeded by a block that also contained the terminator.

**Fix:** Reject duplicate headers, unsupported framing, invalid start line, nondecimal Content-Length, and extra request bytes; check size before parsing. This reduces discrepancies when a tunnel/proxy is present; the audit did not demonstrate standalone bearer-token bypass from duplication.

**Proof run:** Parser test with duplicate Host/Authorization/Content-Length and invalid paths fails on original and passes on corrected implementation, using extracted real functions without external dependencies.

### R-08 — Medium: Encrypted Events Could Be Replayed by Relay

**Evidence:** `remote/socket.rs:239-242` delivered authenticated responses/events directly to client/mirror without timestamp or nonce. Only requests passed through `Replay`. `remote/mirror.rs:112-162` updates mirror status from events and accepts `run.finished` for any active turn in the same remote session.

**Failure:** A compromised relay could replay a valid `agent.events`/`run.finished` ciphertext from an earlier turn, changing visible state or prematurely ending the local turn. Encryption provides authenticity but does not itself prevent replay. Responses already depend on random pending-call IDs, limiting replay impact for responses; the direct issue is events.

**Fix:** Responses/events receive a new `ts` and nonce inside the encrypted body when sent. Reception checks current pin, ±60-second window, and nonce freshness before dispatch to mirror. Legacy messages without this metadata are rejected. Requests remain checked at host admission, without consuming the nonce twice.

**Additional persistence:** The original request cache was lost on restart, allowing replay of a capture still within the time window. Final version uses `<data directory>/connect-replay/receipts.sqlite`, separate from the main database, for requests, responses, and events. It stores only nonce SHA-256 hash, timestamp, and a time marker. SQLite `IMMEDIATE` transaction with `synchronous=EXTRA` commits receipt before admission and serializes two instances in the same directory. On Unix, directory is 0700 and file 0600; Windows uses inherited ACL of the user's data directory. There is no in-memory fallback or automatic recreation of a corrupt database. A truncated or missing file in an existing receipt directory is also rejected. Open/write/commit failure prevents admission. Limit is 20,000 live receipts; only expired timestamps are pruned; at capacity, new messages are refused. Time marker also prevents clock rollback from making a capture valid after its receipt has expired and been pruned.

**Compatibility and operational limits:** Both computers need the corrected version; an original 0.3.28 peer does not provide required response/event metadata. If the clock goes before the last admission time, Connect rejects new messages until it reaches the saved marker. This fix does not promise event ordering or prevent a relay from delaying/dropping first delivery. To avoid relay dependency, remain signed out of Connect.

**Regressions added:** `remote::socket::tests::responses_and_events_need_fresh_unique_authenticated_metadata` checks first receipt, replay, missing metadata, age, future timestamps, replaced key, and continued request admission. Four tests in `remote::replay::tests` check database reopen/second connection, hashed stored data, Unix permissions, unavailable/read-only storage, transaction rollback after insert failure, capacity/expiry, and clock rollback after restart. Earlier time tests use the same SQLite implementation through a `cfg(test)`-only in-memory constructor.

## Optional Behaviors That Are Not Proven Malware

- Connect is opt-in: `remote/config.rs` sets `host_enabled` false by default. `remote.rs:235-253` opens the socket when login/configuration already exist. Without login, that traffic does not occur. **Turning off Allow remote control does not end the relay session; Sign out ends Connect traffic.**
- In `remote/auth.rs:33,72-74`, login uses OAuth device flow with empty scope and sends the GitHub token to the configured relay to exchange it for its own token. This is an explicit credential transfer in Connect, not an embedded secret or hidden prompt instruction. Users who do not trust the service should stay signed out. Relay URL comes from build/environment variables; source alone cannot prove which endpoint was embedded in a published installer.
- Encryption uses X25519/HKDF/XChaCha20-Poly1305, with per-device pinning; ZIP relay forwards encrypted bodies and verifies login token against configured OAuth app. No hidden file-export instruction was found in this subsystem.
- Chat channels work only after configuration and sender pairing. Ordinary replies to a conversation initiated through an external channel still go through that channel; this behavior must remain explicit to users.
- Agent MCP listens only on IPv4 loopback at an ephemeral port, requires literal Host and per-server bearer, rejects Origin and ambiguous headers, and enforces limits. `providers/mcp_host.rs:82-94,300-343`. No anonymous access to MCP host was identified on these routes.
- `local_servers.rs` enumerates local listeners and does not publish them to the network; opening a URL is an explicit action.

## Validation for This Stage

- Two isolated tests against original code: two expected failures demonstrating permissive parser and overflow.
- Same two isolated tests against candidate: two passed.
- Re-ran timestamp case optimized with overflow checks disabled: original fails; candidate passes.
- Isolated tests compile complete source functions without reimplementing logic in another language; they do not replace integrated Rust build/suite.
- Final `remote/replay.rs` was included directly in a `rustc --test` harness using project-compiled Rust dependencies: **4 passed, 0 failures**, including final truncated/missing-ledger checks. No parallel Cargo run. Logs: `audit/remote-regression/persistent-replay-compile.log` and `persistent-replay-tests.log`; harness: `persistent-replay.rs`. It uses the complete production module, without copying/reimplementing logic.
- Independent read-only review by another agent found no blocking defect in atomicity, retention, restart, clock, or host/socket admission. Protection does not defend against a user/admin deliberately restoring an old version of all local storage; open failure stays closed for that run until storage is fixed and the app restarted.
- First integrated Rust suite for main flow (`audit/cargo-tests.log`) successfully ran seven new R-01/R-03/R-05/R-07 tests and updated WhatsApp fixture; overall: 1,178 passed, one `file_preview` expectation failure, 39 ignored. This build predates R-08/persistence regressions, which must be included in final main-flow validation. Remote work adds 12 tests to original total.
- UI regressions, any suite rerun, and packaging belong to main flow. Record actual result before claiming they passed.
