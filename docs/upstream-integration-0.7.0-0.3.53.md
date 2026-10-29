# Upstream 0.3.53 integration ledger — PandaCode Extended 0.7.0

Issue: [#48](https://github.com/ginmp8/pandacode-extended/issues/48). Reference: maintained [ginmp8/pandacode 0.3.53](https://github.com/ginmp8/pandacode/commit/89f422ab70fb89eac26c5e8f619ce633ad751a18). Observed Extended source snapshot: `b08e885283c8d81d2e70e91a30c5ae5e4389e86b`.

**Evidence convention:** "Present" means the implementation was identified in source, not that all acceptance tests, mobile devices, operating systems, load profiles or benchmarks passed. The issue remains open until those requirements are verified on an exact final `development` SHA. No published upstream parity or product-version advance is implied.

| 0.3.53 requirement | Extended implementation / regression evidence | State |
| --- | --- | --- |
| Authenticated optional `z` raw DEFLATE and legacy fallback | `src-tauri/src/remote/crypto.rs`, `src-tauri/src/remote/socket.rs` (per-peer request capability), `mobile/src/connect/crypto.ts`, `mobile/tests/packed-frame.json`, `mobile/tests/crypto.test.ts` | Present; mixed-version device verification pending |
| Authenticated decode, 2 MiB output ceiling and malformed frames | `remote/crypto.rs` bounded decoder, mobile `unpack` slice ceiling; Rust and mobile tests, including empty payload rejection and exact mobile 2 MiB boundary | Present; cross-platform malformed-frame matrix pending |
| Newest-first 32 KiB tail, backward history and uncut latest messages | `src-tauri/src/remote/host.rs` paging, `src-tauri/src/remote/mirror.rs`, `mobile/src/connect/paging.ts`, `mobile/src/connect/client.ts`, `mobile/src/connect/cut-recovery.ts` and `mobile/tests/cut-recovery.test.ts` | Present; full 100 KiB/restart end-to-end scenario pending |
| Read-only batch, 1–8 calls, individual results, no nested writes | `src-tauri/src/remote/host.rs` `batch_calls` / `run_one` / parser tests; `src-tauri/src/remote/socket.rs` read lane | Present on host; queued-revocation and cross-scope integration checks pending |
| Replay/catch-up with durable cursors and deduplication | `src-tauri/src/remote/mirror.rs`, `mobile/src/connect/replay.ts`, `mobile/src/store/replay.ts`, `mobile/tests/replay.test.ts`, `mobile/tests/replay-pagination.test.ts` | Present; simultaneous reconnect/restart fault-injection pending |
| 12-second mobile socket opening deadline and bounded retry | `mobile/src/connect/socket.ts` `CONNECT_MS = 12_000`, `mobile/src/store/backoff.ts`, `mobile/tests/backoff.test.ts` | Present; real stalled-handshake and foreground/background integration checks pending |
| Multi-viewer scheduling, relay rate/queue protections | `src-tauri/src/remote/socket.rs` lanes, `relay/src/bucket.ts` token bucket | Code observed; multi-viewer load/fairness and memory evidence pending |
| Comparable 3G benchmark (baseline and candidate) | `src-tauri/src/remote/bench.rs`, `docs/connect-benchmark.md`; documented command: `cargo test --profile fast-release --lib remote::bench -- --ignored --nocapture --test-threads=1` | **Not executed for this integration snapshot**; no Extended before/after measurement claimed |

## Acceptance work not established by this source ledger

1. Verify the final exact-SHA CI jobs, including Rust, mobile, relay and the affected Extended Connect regression suites. A passing run for another SHA does not qualify.
2. Execute cross-version desktop/mobile transport tests with old/new peer combinations, mixed transport paths, and grant/key revocation during queued batches and replay.
3. Reproduce the 100 KiB newest-message recovery and durable backfill across desktop/mobile restarts, including loss/reordering and multiple viewers.
4. Capture repeated warm/steady runs on the same simulated 3G profile with hardware, SHA, protocol support, variance and median. Reference upstream timings must not be reused as Extended measurements.
5. Record Windows/macOS and physical-device/foreground/background evidence when available. Unrun manual checks are pending, never marked passed.
6. Reconcile impacted issue #44 and unresolved security decisions in #45 before asserting full Connect acceptance.

The work here preserves Extended's existing authorization, user-controlled remote Full access, opaque relay, device grants, replay fences and no-telemetry default. It neither relaxes security nor advances `tracked_upstream_version`.
