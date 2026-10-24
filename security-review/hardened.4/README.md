# PandaCode 0.3.28-hardened.4 — selective backport

**Date:** 2026-10-23  
**Base:** `pandacode-0.3.28-hardened.3(1).zip` (`94eb26152589a4292aa258b73b15769bcde7ae8ac8e656b2f0c11cd0a265a012`)  
**Compared upstream:** `pandacode-0.3.30.zip` (`893e571015787704584a8c6dae21b1e9188fa396192c496bb17891ffe91ebb00`)  
**Status:** source archive with static structural verification; no native binaries built or signed.

## Applied changes

1. `src/main.ts`: new conversations respect the last explicit inspector toggle for the current UI session (button and keyboard shortcut); saved per-thread state still takes precedence.
2. `src/model-picker.ts`: show useful, allowlisted failure classes (timeout, login, RPC frame/output limits, invalid provider data) as `textContent`; never echo raw provider/CLI diagnostics or arbitrary stack traces.
3. `src-tauri/src/providers/pi.rs`: read Oh My Pi `ready` frame; negotiate RPC v2 only if advertised; accept `rpc_chunk` only after successful v2 ack; fall back to v1 on refusal; preserve Pi v1 request. Sequential chunk IDs/indexes, consistent counts and byte lengths, canonical base64, 512-chunk cap, 1.5 MB base64-field cap, 12 MiB decoded-frame cap, 16 MiB total stdout cap, 15-second timeout and subprocess cleanup. Error text from provider is mapped to safe high-level categories.
4. `tests/inspector-new-task.spec.ts`: upstream new-task regression scenario. Three native Rust unit tests in `pi.rs` cover negotiation, chunk safety, v1 fallback, and error redaction.

## Intentionally unchanged from hardened.3

- Native Connect keychain/keyring, durable replay receipts, recheck of trust at execution/reply time.
- Native app/agent auto-updater disabled; no automatic VM desktop setup.
- HTML preview CSP and browser-navigation restrictions.
- Bot notify approval and disabled external automatic sends.
- Dependency manifests, lockfiles, vendor GLib backport, installer/runtime policy, secret-shaped Slack fixture redactions.
- All other baseline files are byte-identical (verified via archive member hashes).

**Not carried forward:** upstream Connect JSON secret storage/concurrency patch, price refresh network requests, updater, weakened CSP, transient replay prevention, approvals relaxation, or dependency downgrades. Connect concurrency would require an independent credential-store-safe design/validation and is out of this patch.

## Validation and constraints

- **Performed:** frozen ZIP SHA-256 identity; archive CRC, path/member checks; precise allowed-file diff; byte identity of sensitive modules/lockfiles/vendor; static feature checks; TypeScript parser diagnostics (if available); tests in source exist and are inspected.
- **Not performed:** Rust build/test, TypeScript type-check/build, browser/Playwright tests, end-to-end Oh My Pi / Pi integration, security scanner with runtime reachability, native install/signing on Windows/macOS/Linux. Previous hardening runs are historical evidence for hardened.3, **not** revalidation of this release.
- **Owner checks required before production use:** offline/isolated `cargo test --locked --manifest-path src-tauri/Cargo.toml`, `npm ci --ignore-scripts` and `npm run build`, Playwright test with approved fixtures, CSP/updater/Connect/approvals integration and Windows/macOS smoke; audit dependencies/credential exposure. Run only in a trusted controlled environment; dependency/target scripts were not executed for this backport.

This is a fork of PandaCode — https://github.com/aicodeking/pandacode. The source license and attribution are retained. Release metadata uses the `hardened.4` revision; app and Cargo upstream product version remain `0.3.28` to avoid inventing an upstream version or changing package/installer compatibility.
