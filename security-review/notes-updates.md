> Work/evidence note. The consolidated test results are in ../SECURITY-HARDENING.md and verification.json.

# PandaCode 0.3.28 — updates, installers, child environments, and Rust advisory review

## Scope and evidence

Review of the immutable source at `audit/original/pandacode-0.3.28` and scoped remediation in `audit/pandacode-0.3.28-hardened`. Original-source line numbers below refer to the immutable copy. This review did not run the original application, install its external tools, execute untrusted package lifecycle hooks, upload project source, or perform live malware detonation. The user permits AI execution and local commands; automatic external updates and undisclosed data transmission are outside the requested policy.

This is the updates/supply-chain workstream. Other workstreams own remote access, application permissions, secrets, outbound messages, dependencies, and integrated documentation. It is not an assertion that the entire application or every supported external AI CLI is free of vulnerabilities.

## Findings and implemented remedies

### U1 — Application updates are enabled by default and install external releases

Original evidence: `src/app-updates.ts:39` defaults to enabled when its saved preference is absent or cannot be read. The launch delay is 30 seconds; startup and recurring checks are scheduled at lines 94–95. The updater downloads in the background and calls `pending.install()` at line 85; its policy can restart the application when idle. This path does verify release signatures using Tauri: it should not be described as an unsigned downloader or proof of malware. It nonetheless contradicts the requested no-automatic-updates policy. The About UI has an opt-out for this component alone.

Remedy: the application controller is now inert, preserves its public interface, always reports `off`, `automatic: false`, `supported: false`, does not read old settings, and never loads the updater, schedules a timer, checks, installs, or restarts. Calls to `setAutomatic(true)`, `check()`, or `restart()` cannot restore networking. The policy's restart predicate also always refuses. About and settings display the actual hardened policy. Root separately removes the native updater plugin, registrations, capabilities, dependencies, and configuration.

### U2 — Independent agent version checks start without a UI opt-out

Original evidence: `src/agent-updates.ts:32,42,154,170,223` defines six-hour checks, an eight-second launch delay, invokes `check_agent_updates`, and reschedules work. Installing a CLI update requires a separate click, but checking external package/release servers starts independently of the application update toggle. Native entrypoints are `src-tauri/src/agent_updates.rs:540,562`.

Remedy: the agent update controller only opens a static explanation; it has no bridge imports, checks, installation actions, timers, focus/visibility handlers, or progress subscriptions. Native `check_agent_updates` and `update_agent` return a policy error before any probe or request. The internal installer `apply` also refuses before reaching installer logic, covering a future accidental internal call. Existing public shapes and native commands are retained for compatibility. An old enabled UI preference or a direct command call does not restore the disabled functions.

### U3 — Child harnesses can perform their own updates or nonessential telemetry

Original evidence: the application had opt-outs on isolated version-probe commands in `agent_updates_install.rs:62`; its update path removes an updater opt-out at line 748. Ordinary `child_env::sanitize` only cleans AppImage paths. A provider-specific MiMo option did not cover all normal Claude/GLM launches.

Remedy: supported, documented opt-outs are now applied to child processes through the common sanitizer, without changing the user's global environment, credentials, model endpoint, proxy, or normal command PATH. Existing AppImage cleanup remains. The values are `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, `DISABLE_AUTOUPDATER=1`, `DISABLE_TELEMETRY=1`, `DISABLE_ERROR_REPORTING=1`, `DO_NOT_TRACK=1`, `KIMI_CODE_NO_AUTO_UPDATE=1`, legacy alias `KIMI_CLI_NO_AUTO_UPDATE=1`, and `OPENCODE_DISABLE_AUTOUPDATE=true`. `FORCE_AUTOUPDATE_PLUGINS` is removed because the documented option overrides updater opt-outs.

Limits: these flags apply to CLIs that honor them; they are not an operating-system network sandbox. CLI settings and subsequent user commands can override inherited environment settings. The Claude documentation explicitly notes this precedence. No claim is made that an arbitrary external command or AI harness can no longer send data. `DISABLE_UPDATES`, which also blocks explicit manual Claude updates, was deliberately not introduced; the user permits explicit command execution.

Primary documentation checked:

- <https://code.claude.com/docs/en/env-vars>
- <https://opencode.ai/docs/cli/>
- <https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/env-vars>

### U4 — Included npx launcher would install the latest upstream application

Original evidence: `packages/npx/installer.mjs:12` selects GitHub's `releases/latest`. `install` starts at line 134, verifies a minisign signature at line 140, and executes the selected platform installer at line 144. It is signed, but installing upstream would bypass this fork's policy changes.

Remedy: the package no longer contains any downloader or platform installation execution path. `install()` refuses both ordinary and dry-run invocation with guidance to build/install reviewed local source. The CLI help describes the policy. Offline signature verification and pure asset/desktop-entry utility functions remain intact. This does not publish or replace any external npm package; it changes the included source only.

### U5 — Starting a Linux VM silently adds desktop packages and auto-login

Original evidence: `vm/desktop.rs:49,57` starts desktop installation in the background when missing. Both native UI commands (`vm/commands.rs:58,75`) and agent actions (`vm/tools.rs:123,143`) invoke it on create/start. `vm/scripts/linux-desktop.sh` downloads packages using the guest's package manager and configures Xfce auto-login. These changes happen inside the guest, not directly on the host PC.

Remedy: removed all background desktop installation. Creating or starting a local VM starts that VM without invoking the desktop installer. The running local Linux VM exposes `Add a desktop`; the existing explicit `install_desktop` action remains in the agent tool catalog and its approval set. Descriptions explain that this action downloads packages into the guest. Explicit installation holds the existing desktop-in-progress state, prevents duplicate installation, checks for an existing desktop, and releases state on both success and failure. Preview mocks and browser tests match this behavior. VM image downloads remain part of an explicit create action; this was not replaced with an unrelated VM implementation.

## Other installation paths assessed

| Component | Evidence and trigger | Assessment |
|---|---|---|
| Jev / computer-use | `src-tauri/src/jev.rs:728`, called by explicit UI setup at `src/jev.ts:238`; Jev archive commit pinned at native line 24; browser-harness 0.1.13 at lines 803/815 | Deliberate optional installation. Python transitive dependencies are not fully hash-locked. Retained; no background installation trigger found. Other agent owns secret migration. |
| Tart | `vm/setup.rs:15,71`; explicit VM setup from panel | Fetches latest release but validates Cirrus team `9M2P8L4D89` and notarization. Do not describe as unsigned. Retained explicit installation. |
| Android tools | `simulator.ts:327` to `android_setup.rs:110` | Explicit setup installs Homebrew/SDK components and accepts SDK licenses. No automatic startup call found. Retained. |
| Commandcode, Muse, code search | Native adapters | Launch installed CLI tools; no installer/download fallback found in these adapters. Code-search telemetry already disabled. |
| npm application install | `package.json`, `package-lock.json` | No application lifecycle install hook. Only esbuild 0.28.2 and fsevents 2.3.3 in the reviewed lock have install scripts. Safe acquisition used by root: pinned lock plus `npm ci --ignore-scripts`; do not assume this proves dependency code is harmless. |

Reported to root for separate remediation: AWS Linux user-data `cloud/aws.rs:51` uses `set -eux`, risking password exposure in trace output. Windows user-data lines 93–94 download and execute a fixed TightVNC MSI without independent hash or signature validation; execution is in the cloud guest. Line 89 writes the intended AutoAdminLogon password to the guest registry. These were not silently reclassified as host compromise.

## Rust dependency advisory audit

On 2026-10-23, queried OSV for every public crates.io name/version pair from the then-current Cargo lockfile. Only names and versions were transmitted. No code, credentials, local paths, or lockfile contents were uploaded. The scanner uses the primary RustSec records distributed through OSV and retrieves the full advisory entries. It does not install packages or execute their code.

- Input snapshot: 593 registry package entries.
- Input SHA-256: `acb4ae7f8936fa6dd39c181fd8283efa8449301be8b894c0e21dc1a4f7363db9`.
- Original results: `audit/native-checks/cargo-osv-results-before-tls-updates.json`.
- Reproducible read-only query: `audit/native-checks/cargo-advisory-scan.py`.
- Fifteen package/version matches; glib's GHSA and RustSec entries describe the same issue. Nine matches are maintenance notices, not demonstrated exploit paths.
- The versions in the following table are the first scan snapshot. A subsequently authorized, narrowly scoped lockfile update is recorded below.

| Locked dependency | Advisory / minimum fixed version | Reachability assessment |
|---|---|---|
| rustls-webpki 0.103.15 | RUSTSEC-2026-0382; 0.103.16 | Certificate verification can accept the distrusted emSign intermediary keys when trusted roots permit the path. Used through rustls and rustls-platform-verifier. Material: fix alongside trust stores. |
| webpki-roots 1.0.9 | RUSTSEC-2026-0381; 1.0.10 | Trust store includes emSign root permitting distrusted intermediates. `tokio-tungstenite` explicitly enables `rustls-tls-webpki-roots`. Material. The separate 0.26.11 compatibility wrapper reexports its 1.x parent, which must also be fixed. |
| webpki-root-certs 1.0.9 | RUSTSEC-2026-0383; 1.0.10 | Same trust-store concern; pulled by rustls-platform-verifier. Fix both roots and verifier. |
| rustls 0.23.45 | RUSTSEC-2026-0346..0349; 0.23.46 | Update recommended. Finer conditions: 0346 requires custom ClientSessionStore debug logging (none found); 0347 requires enabled ECH plus pre-handshake application writes (no ECH config found); 0348 requires nondefault TLS Brotli certificate feature (not enabled); 0349 loses post-quantum protection, while this app explicitly installs the ring provider, which is classical-only. Do not claim these establish an actual secret leak in this app. |
| zip 2.4.2 | RUSTSEC-2026-0367, 0368, 0369; 9.0.0 | All concern extraction APIs writing permission bits or symlinks to disk. The only application consumer is `vsix_themes.rs`: `ZipArchive::new` and `by_name` read JSON into bounded strings (`MAX_JSON`), then validate colors. No `extract`, `ZipStreamReader`, permission application, or archive entry filesystem write found. No path to these three vulnerable extraction functions observed. Test-only ZipWriter creates fixtures. |
| glib 0.18.5 | RUSTSEC-2024-0429 / GHSA-wrw7-89jp-8q8g; 0.20.0 | Unsound `VariantStrIter::impl_get` may dereference null after optimization. Primary fix changes out-pointer to mutable. Whole downloaded Rust registry search found `array_iter_str` only in glib's definitions, tests, and docs, not production callers. No application caller found. Cannot infer complete exploitability from version alone. GTK3 stack pins glib 0.18 types, so a blind 0.20 bump is not compatible; upstream patch backport or architectural migration is needed if eliminating the latent vulnerable implementation. |

Maintenance-only notices in the scan: `proc-macro-error 1.0.4` (RUSTSEC-2024-0370); `unic-char-range`, `unic-common`, `unic-char-property`, `unic-ucd-version`, `unic-ucd-ident` all 0.9.0 (RUSTSEC-2025-0075, 0080, 0081, 0098, 0100); `zvariant 4.2.0`, `zbus_names 3.0.0`, `zvariant_derive 4.2.0` (RUSTSEC-2026-0370, 0371, 0372). They need maintenance planning; they are not automatically nine remotely exploitable vulnerabilities.

### Intermediate TLS lockfile update and second scan

Following explicit assignment from root, changed **only** `src-tauri/Cargo.lock` using sequential `cargo update -p NAME@OLD --precise NEW` against the official crates.io index. No builds or dependency code execution were performed by this workstream during the update.

| Package | Before | Final |
|---|---|---|
| rustls | 0.23.45 | 0.23.46 |
| rustls-webpki | 0.103.15 | 0.103.16 |
| webpki-roots | 1.0.9 | 1.0.10 |
| webpki-root-certs | 1.0.9 | 1.0.10 |

The diff changes only these four versions/checksums and the existing `webpki-roots 0.26.11` wrapper's reference to its updated 1.0.10 parent. The graph still contains 594 package entries (application plus 593 registry entries); no extra package was added or removed. Baseline preserved as `audit/native-checks/Cargo.lock.before-tls-updates`.

- Intermediate lock SHA-256: `f4ac1da09b768f233a79c9ac4f9a1e385d19661687f2375b0e230a67a58e2b3f`.
- Intermediate full OSV results: `audit/native-checks/cargo-osv-results-after-tls-before-glib-zip.json`, checked at `2026-10-23T05:20:59.128698+00:00`.
- None of the four updated packages matches an advisory in the second scan.
- Eleven package/version matches remain: glib, zip, and the nine maintenance-only packages listed above. This is **not** a clean advisory scan and should not be presented as one.

Residual reachability was checked against actual application code and the downloaded dependency sources. For `zip`, the only production use is bounded VSIX JSON reading, and the dependency graph lists only PandaCode as its consumer. The vulnerable extraction/write APIs are absent. For glib, the private `VariantStrIter::new` constructor is reached by `Variant::array_iter_str`; whole-source search found that entrypoint only in glib's definition, examples, and unit tests, with no production caller in PandaCode or its downloaded dependencies. These observations apply to this source snapshot and are not a guarantee about a future feature or library update.

### glib backport feasibility — initial assessment before subsequent authorization

Verified the exact official gtk-rs fix through the GitHub API for commit `b5a4071e439bef2b5eea76c3aa25e5ae84839e34`, linked from upstream PR <https://github.com/gtk-rs/gtk-rs-core/pull/1343>. It changes two lines in `glib/src/variant_iter.rs`: make `p` mutable and pass `&mut p` to the variadic C out-parameter. This patch matches the affected source in glib 0.18.5 and does not change its public API or dependent GTK types.

A reproducible backport can vendor the authentic glib 0.18.5 crate, retain its license and original version, apply those two edits, record original crate checksum/upstream commit/patch hash, and select it with `[patch.crates-io] glib = { path = "vendor/glib" }`. The cached crate is 122 files and 1,717,591 bytes (94 source files, 1,633,772 source bytes). It is a modest distribution cost but introduces an explicitly maintained local dependency. Do not relabel 0.18.5 as 0.20.0 merely to silence a version scanner. A local backport may still require a documented advisory exception tied to exact provenance; absence from registry-only OSV results must not be called proof of a fix.

Necessity: no vulnerable production call was found in the current application, so this is not evidence of an exploitable existing input path. If the deliverable must also eliminate the latent unsound implementation, the small backport is preferable to a broad GTK migration. None of the glib implementation work was performed under the initial lockfile-only scope; the follow-up implementation below was separately assigned by root.

### Final GLib backport, ZIP 9 update, and transparent third scan

Root subsequently authorized eliminating both latent implementations. The resulting source is now stable:

- `src-tauri/vendor/glib` contains the authentic glib 0.18.5 crate, extracted only after its archive SHA-256 matched `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`. LICENSE, COPYRIGHT and the original package version remain intact. Only `src/variant_iter.rs` differs from the 121 upstream archive files, by the two-line official fix. `[patch.crates-io]` selects the local copy for every GTK3 consumer.
- `BACKPORT_PROVENANCE.json`, `BACKPORT.patch`, `BACKPORT.md` and `verify-backport.py` record original file hashes, patched file hash, original archive hash, exact patch hash and upstream commit. The offline verifier passed all 121 files and confirms only the documented source patch.
- The application dependency is pinned to `zip = "=9.0.0"`, keeping default features off and the existing `deflate` feature. Verified the official ZIP 9 archive against SHA-256 `711e8c9646791c49b7eb799a19c4ae70a9b8372ddc6a1fa4778defd1e2aa5d5e`. Its `ZipArchive`, `by_name`, `size`, bounded `Read`, `ZipWriter`, `SimpleFileOptions` and `finish` APIs remain compatible with this consumer. **No application consumer code was rewritten.** ZIP 9 requires Rust 1.88 or newer.
- Lock changes relative to the TLS-fixed baseline are limited to glib becoming a local dependency, ZIP 2.4.2 → 9.0.0, adding ZIP's `typed-path 0.12.4`, and removing now-unused `arbitrary` and `derive_arbitrary`. Other versions remain locked.
- Final application lock SHA-256: `56028b0cc537d28ea043de33eb2918874c4a341425a3d91c8556642bc27e840a`.

Added an independent, locked test package in `src-tauri/security-tests`. It tests the actual vendored GLib APIs with release optimization (forward/reverse/indexed/last/mixed/end iteration) and ZIP 9's actual deflate/read APIs, plus invalid-archive rejection. Its dependency versions are all present in the application lock, apart from its own test-package identity. **Six tests passed** using Rust 1.99.1 with `cargo test --manifest-path security-tests/Cargo.toml --release --locked --offline`, in a separate target directory. Compilation emitted 34 existing upstream GLib style/lifetime warnings; no additional source changes or warning suppressions were introduced. This workstream did not build the full app; root owns integrated execution.

Final OSV scan at `2026-10-23T05:25:03.400573+00:00` is in `audit/native-checks/cargo-osv-results.json`. The scanner queries all 591 registry dependencies **and explicitly also queries the public glib 0.18.5 identity despite its local path**, marking the result `source_type: local-backport`. This prevents removal from the registry set from masquerading as remediation. No TLS or ZIP advisory remains. Raw matches are the nine maintenance notices and glib's one advisory (with GHSA/RustSec aliases), whose fix is established by the verified backport and optimized behavioral tests. Do not describe the raw scan as empty; classify glib as patched locally with provenance and retain the maintenance disclosures.

Primary advisory links:

- <https://rustsec.org/advisories/RUSTSEC-2026-0381.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0382.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0383.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0346.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0347.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0348.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0349.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0367.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0368.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0369.html>
- <https://rustsec.org/advisories/RUSTSEC-2024-0429.html>

## Verification actually completed in this workstream

- `node tests/app-updates.test.mjs`: 3 passed. Traps fetch, storage, and timers around the actual controller, exercises old preference/direct entrypoints, and verifies restart refusal.
- `node tests/hardened-agent-updates.test.mjs`: 1 passed. Loads the actual TS controller with a CSS-only loader hook, verifies no network/timer/provider calls, and validates dialog lifecycle/cleanup.
- `node packages/npx/installer.test.mjs`: 8 passed, retaining crypto validation and checking installer refusal with a fetch trap.
- `node packages/npx/bin/pandacode.mjs --help`: manual build guidance shown.
- `node packages/npx/bin/pandacode.mjs --dry-run`: refused with expected exit 1, no download.
- `node node_modules/typescript/bin/tsc --noEmit`: succeeded after VM preview/test changes.
- Isolated Rust `child_env` harness: 3 tests passed using rustc 1.99.1. The harness copies actual source and removes only implementations tied to external tokio/portable_pty types; it retains std command behavior, sanitizer, and tests. This is not a full Tauri/native integration test.
- Browser coverage added/updated in `tests/agent-updates.spec.ts` and `tests/vm.spec.ts`. Root's initial Chromium run passed 90 of 95 selected tests; all five VM tests stopped in the opening helper because the Linux runner exposed `navigator.platform` as Linux while the preexisting app shows the local Tart tab only on macOS. Screenshot and trace confirmed a hidden platform-gated tab, not overflow or an installation regression. The helper now explicitly initializes the macOS fixture (`MacIntel`) before navigation, consistent with its mocked supported Tart backend. No application UI or security assertion was changed.
- Reran only `tests/vm.spec.ts` with the supplied Chromium executable and two workers: **5 passed**. The original 15 VM failure artifacts (five screenshots, traces, and contexts) are preserved with hashes under `audit/browser-vm-initial-failures`; root's initial run log remains unchanged. Final targeted log: `audit/browser-vm-final.log`. Automatic-install refusal, explicit installation, screen input, read-only sharing, deletion and cloud UI assertions all remain in place.
- Full Cargo build/tests not run by this workstream while root owns native dependency setup/build. Native guard and VM state unit tests are present for that run.

## Owned changed files and exact vendored inventory

1. `src/app-updates.ts`
2. `src/app-updates-policy.ts`
3. `src/agent-updates.ts`
4. `src/about-settings.ts`
5. `src/settings.ts`
6. `src-tauri/src/agent_updates.rs`
7. `src-tauri/src/agent_updates_install.rs`
8. `src-tauri/src/child_env.rs`
9. `packages/npx/installer.mjs`
10. `packages/npx/bin/pandacode.mjs`
11. `packages/npx/installer.test.mjs`
12. `tests/app-updates.test.mjs`
13. `tests/agent-updates.spec.ts`
14. `tests/hardened-agent-updates.test.mjs` (new)
15. `src-tauri/src/vm/desktop.rs`
16. `src-tauri/src/vm/commands.rs`
17. `src-tauri/src/vm/tools.rs`
18. `src/vm-panel.ts`
19. `src/preview-vm.ts`
20. `tests/vm.spec.ts`
21. `src-tauri/Cargo.lock` (four targeted TLS fixes, then the separately authorized GLib/ZIP changes)
22. `src-tauri/Cargo.toml` (only ZIP's version line and the GLib patch table)
23. `src-tauri/security-tests/Cargo.toml`
24. `src-tauri/security-tests/Cargo.lock`
25. `src-tauri/security-tests/src/lib.rs`
26. `src-tauri/vendor/glib/` — 121 original crate files (only `src/variant_iter.rs` changed) plus four provenance/verification files. Original paths and hashes are listed exactly in `BACKPORT_PROVENANCE.json`; the full workstream path inventory is `audit/update-workstream-files.txt`.

Documentation impacts handed to root: README privacy/update claims; AGENTS auto-update and automatic VM desktop paragraphs; release/install instructions pointing at upstream npx; About/updater test expectations; native updater dependencies/capability/plugin references. Existing license and PandaCode attribution were preserved.
