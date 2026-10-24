> Work/evidence note. The consolidated test results are in ../SECURITY-HARDENING.md and verification.json.

> Independent review before the final hardening: the final code also rejects an existing ledger that is truncated or missing. See notes-remote.md and the final persistent-storage tests.

# Final Independent Review — Persistent Connect Replay Protection

Read-only review of `src-tauri/src/remote/replay.rs`, initialization in `remote.rs`, and admission in `remote/host.rs` and `remote/socket.rs`. No changes were made to those files; this reviewer did not start a build or Cargo command.

## Result

No replay bypass or blocking regression was identified in the implementation examined.

- `Replay::check` validates the timestamp and nonce format/length, opens an SQLite IMMEDIATE transaction, checks for duplicates using the nonce's SHA-256 hash and primary key, inserts a receipt, and commits before returning success. The host executes only after this succeeds; the socket delivers responses/events only after admission. IMMEDIATE also serializes two connections/processes using the same database.
- Failure to open or write SQLite remains fail closed. `Replay::default` is not an ephemeral cache: it has `db=None` and rejects messages. An insert/commit error has no path that executes an action while the API reports an error.
- The window allows an absolute difference of up to 60,000 ms. Pruning uses `ts < now-60000`, so the receipt at the inclusive boundary is retained. The 20,000-receipt limit rejects new messages without evicting still-valid entries. SQLite is also limited to 2,048 pages.
- The `last_ms` watermark and pruning are committed in the same transaction. If the clock moves backward, the gate blocks instead of making a ciphertext valid again after its receipt has been removed. A failed transaction neither advances the watermark nor commits partial pruning.
- `host::handle` checks sender/key trust and whether hosting is enabled before consuming a nonce. `socket::admit_delivery` requires a currently trusted key for responses/events; requests are not consumed there and are admitted by the host before dispatch. Response/event timestamps and nonces are included in the authenticated payload before sealing.
- Existing tests cover restart/second connection, unavailable/read-only database, pruning and watermark rollback on insert failure, capacity/time limits, and clock rollback after restart. Execution results belong to the main build.

## Material Limitations

1. The guarantee assumes continuity of private storage. Deleting, truncating, or restoring an old SQLite copy can recreate an empty/old history; this does not protect against disk rollback by a local user/admin. The remote relay does not gain this capability from the reviewed implementation.
2. A large backward clock correction after an admission with a future timestamp blocks new messages until the saved watermark is reached. This is a security choice that can affect availability.
3. If the database cannot be opened at startup, the app keeps the gate unavailable for that run, logs the error, and does not retry automatically. Restart after fixing storage/permissions is required. Received responses/events remain rejected; this does not mean all app network/outbound connections are blocked.
4. Permissions 0700/0600 and NOFOLLOW protect normal paths from exposure/access by another user, but are not intended to stop a user/admin who controls the data directory and process. Windows uses the inherited ACL on app data.

## Final Hardcoded Credential Scan

The `Secure Code Review/scripts/scan_secrets.py` scanner was used as a library to avoid emitting its `evidence` field, which only truncates content. Output was limited to paths, lines, rules, and classifications in `secret-scan-sanitized.json`.

Coverage: 591 first-party files, including source/config/docs/scripts and mjs/cjs/lock extensions, excluding node_modules/vendor, target/dist/build, expected fixtures, test files, and Rust `#[cfg(test)] mod tests` modules. An additional check covered modern token formats, JWTs, PEM blocks, credential-bearing URIs, and sensitive literals with name prefixes in variable names.

The review examined 127 scanner alerts (100 lines and 27 entropy duplicates) plus two supplemental alerts. They were dynamic expressions, types, secret-store entry names, markers/labels, and a preview fixture. **No real hardcoded credential was confirmed in this scope.** Potential secret values were not emitted. This does not replace runtime-flow analysis or prove that no secret or obfuscated material exists.
