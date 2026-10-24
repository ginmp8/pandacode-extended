# PandaCode 0.3.34.hardened — selective source backport and security review

**Status:** Source ZIP assembled from the supplied 0.3.30 hardened fork; **NOT** a verified, compiled or signed desktop release. It intentionally does **not** implement all upstream 0.3.34 features. Comparison and review performed 2026-10-23 (local user date). Results apply to these supplied archive identities, not to an independently retrieved Git release.

- Baseline source ZIP SHA-256: `2150f4c632538a0283aff6f65fbe09cb701102b6ffb483867ff0f4185b3c11b6` (`pandacode-0.3.30.hardened(1).zip`).
- Supplied upstream ZIP SHA-256: `f01ecdcd1edfc0de3b52c40dfdb838e8486f5394286f4d1b4668a684366a4f79` (`pandacode-0.3.34.zip`).
- Internal package version: `0.3.34-hardened`; ZIP/distribution label: `0.3.34.hardened`.
- Source lineage: an earlier 0.3.28-based fork, successively hardened/backported through 0.3.30; not equal to an independently verified original 0.3.30 tree.
- Attribution/license: original root LICENSE and fork attribution preserved.

## Change decision

| Upstream change | Decision | Reason and scope |
|---|---|---|
| 0.3.32 bot icons: ten local panda presets and user-selected images | **Backported** | Render local inline SVG constants; crop to 128x128 PNG. File size, image dimension, data URI, and native IPC payload restrictions added. No remote image fetch. |
| 0.3.32 reorganized bot Settings | **Backported** | Profile/Channels/Schedules/Memory/Activity sections; conversations and rules retained within Activity. Existing permissions, provider selection, default Ask and backend checks preserved. |
| 0.3.33 native tray/menu bar task progress, notifications, plan usage | **Backported** | Native task summary and open-session events, task titles shown in local OS menu; click-triggered usage refresh. 1024 in-memory unseen-ID bound and ten rows per section; unavailable tray does not stop app startup. |
| 0.3.31 remote/cloud agents and remote CLI provisioning | **Excluded** | External process/credential/copy and potentially paid/irreversible actions; no safe authority, consent, network/data boundary verification available. Existing hardened-base cloud capabilities remain exactly as before. |
| 0.3.34 macOS secrets migration from Keychain to local file | **Excluded** | Different security model and migration/deletion semantics. Preserve existing OS-native secret store and usage-keychain helper; no credential relocation on startup. |
| Upstream updater, auto-install and changed dependencies | **Excluded** | Preserve disabled updates, non-installing npx, locked manifests and source-only manual build policy. |

**Not full parity:** only selected code from 0.3.34 is integrated. This version label identifies the comparison target/backport release; it is not a claim of equivalent functionality or security certification.

## Security boundaries and evidence (SGR-2.0)

**H-01 — New bot image content crosses local UI → IPC → SQLite.**
- **Classification:** `confirmed` (structural control exists), **severity:** informational, **confidence:** high.
- **Evidence:** `src/bot-avatars.ts:10-84` checks accepted input MIME, 5 MiB upload limit, decoded pixel dimensions, PNG crop to 128x128, data URL size; `src-tauri/src/bots.rs:206-287` validates at native create/update, limits payload to 300,000 bytes, permits strictly `panda:0..9`, PNG data URLs with checked signature/IHDR/128x128 dimensions or at most four legacy glyphs; rejects other data schemes. `src/preview-bots.ts` has a parallel preview bound. This is source-level control presence only; no runtime attestation.
- **Residual risk:** browser decoding happens before the post-decode dimension guard. Extremely compressed or decoder-malformed images could still cause client resource pressure; full PNG validity is not proven by header checks. PNG decoder and platform-specific security must be validated in a separately isolated trusted environment.
- **Verification:** authorized real WebView test on PNG/JPEG/GIF/WebP (valid, invalid, oversized, high compression), Rust guard unit tests and IPC edit/create regression.

**H-02 — Native tray displays local task and project titles.**
- **Classification:** `governance-risk`, **severity:** low, **confidence:** high.
- **Evidence:** `src-tauri/src/tray.rs:78-154` builds OS-visible labels from local task/project strings, provides click-to-open and usage refresh; `src/main.ts:1805,3906` handles a local navigation event. No new remote sender was integrated. Users with access to the logged-in desktop or shared-screen view can see task names.
- **Risk/mitigation:** avoid sensitive task titles or hide them in future opt-in privacy settings if used on shared/unlocked desktops. Preserve existing OS/session access controls; do not mistake the tray for an authorization gate.
- **Verification:** manual privacy walkthrough on each OS and shared-screen/lock behavior; optional preference for generic titles, if requested.

**H-03 — Native runtime compatibility is unknown.**
- **Classification:** `needs-verification`, **severity:** medium (delivery gate, not a confirmed vulnerability), **confidence:** high.
- **Evidence:** new `src-tauri/src/tray.rs` and Tauri `tray-icon,image-png` feature flags; `src-tauri/Cargo.lock` retains the originally resolved package graph and the hardened `tauri =2.11.6` pin. The supplied sandbox had no Rust toolchain and did not provide a verified OS-enforced isolation boundary for running target code.
- **Risk/mitigation:** cannot assert compilation, event correctness, native tray availability, packaging or OS-specific behavior. Do not distribute as a verified install until native build + focused tests + acceptance smoke tests pass in a trusted isolated build environment.
- **Verification:** `cargo test --locked`/`cargo check --locked` on an approved host, JS UI tests and release build per OS; rerun applicable security gates.

**H-04 — Excluded upstream high-impact features.**
- **Classification:** `needs-verification` (not candidate findings), **severity:** informational, **confidence:** high for exclusion.
- **Evidence:** upstream `src-tauri/src/cloud/sandbox.rs:18-89` contains remote machine file/exec primitives; upstream `src-tauri/src/secret_store.rs:2-138,211-260` describes file-based secret migration. The corresponding hardened-base files remained unchanged. New integrations of these upstream behaviors are absent.
- **Recommendation:** if adopted in a future revision, require per-run data export consent, allowlisted paths (exclude secrets by default), credential audience/narrowing, remote attestation and deletion/retention, action approvals, payment budgets and reversible migration; verify independently before enabling.

**H-05 — Secret scanner candidates in inherited test fixtures.**
- **Classification:** `suspicious-pattern` → source-context assessment: synthetic examples, **severity:** informational, **confidence:** medium.
- **Evidence:** static provider-prefix scan found 1 GitHub-like, 2 AWS-like and 1 private-key marker in unchanged files `src-tauri/src/skills/safety.rs` and `src-tauri/src/cloud/deploy.rs`; they occur in tests/exemplar strings and are identical to the prior hardened baseline. No complete secret is printed in this report and provider authentication was not performed.
- **Residual risk:** scanner coverage is not complete; Git history, CI logs, containers, binaries, external providers and runtime state were **not** reviewed. No claim that all secrets or CVEs are absent.

## Static verification receipt

- **23/23** candidate structural gates passing (file identity, version/lock consistency, unchanged security controls, source wiring and comparison-only secret scan). See `static-checks.json`.
- **7/7** touched TypeScript source files parsed with an independently installed TypeScript AST parser, **zero syntax diagnostics**. This is **not** full typechecking or frontend test execution.
- The exact `package-lock.json` resolved tree equals the supplied hardened baseline after normalizing the root version; same for `Cargo.lock` package graph.
- All checked security files (`.npmrc`, update shims, Keychain, secret store, bot guards/gateway, remote module) are **byte-identical** to baseline.
- Included unexecuted regression tests: `tests/bot-avatars.test.mjs` (upstream tests plus hardened markup/size cases) and Rust `avatar_security_tests` inside `src-tauri/src/bots.rs`.
- No native compile, JS UI behavior, installed app, OS menu interaction, ZIP-based CVE attestation, Git history, production deployment or external service execution was performed; these states remain `not-run` or `unavailable`.

## Recommended trusted-host checks

1. Check out the ZIP in a reviewed private workspace. Reconfirm its SHA-256 and compare the included source manifest.
2. Install the approved pinned Rust/Node toolchain from verified sources; then `npm ci --ignore-scripts`. Do not run unknown install hooks.
3. Run source and regression tests in a correctly sandboxed development environment: `npm run build`, focused UI/avatar tests (per Node version), `cargo check --locked`, `cargo test --locked`, then focused browser preview and desktop tray smoke tests across supported OSes. Inspect test harness/tooling before execution.
4. Build and sign native installers using the existing source-only release scripts, independently check artifact/lockfile signatures and malware alerts, then perform explicit approval/security review prior to distribution. Retain old app data backups.

## Source change ledger

Relative to the supplied hardened base at the time of report generation: 26 existing files modified, 5 new files added, 0 files removed. Version and active documentation changed; historical prior security-review manifests and tests remain unmodified. For full SHA-256 details use `source-manifest.json` (separate from source content); this report does not imply clean compilation.
