# Current revision: 0.3.41-hardened

The current source release extends the supplied `0.3.35-hardened.4` selectively. See [current validation and limits](security-review/0.3.41-hardened/REPORT.md), [connection/session safeguards](docs/connection-session-safety.md), and the [Portuguese feature guide](docs/HARDENED-0.3.41-GUIDE.pt-BR.md). The sections below are historical records; their findings, test counts and environment limitations must not be read as the results of this revision.

This revision adds native account-bound session invalidation, a separate remote Full access gate (default off), commit/hash-pinned Skills imports and scope resolution, and validated historical attachment actions. It preserves secure credential storage, device trust/revocation, replay checks, disabled automatic updates and the existing fail-closed isolated-review admission. Dependency versions are unchanged.

---

> **Historical report for hardened.1.** It does not describe tests from the current review. Later reviews are in `SECURITY-INSTALLATION-REVIEW.md` (hardened.2) and `security-review/hardened.3/README.md` (hardened.3). The hardened.2 installation review, fixes, and limitations are in [SECURITY-INSTALLATION-REVIEW.md](SECURITY-INSTALLATION-REVIEW.md). Older manifests, patch, and verification summary are in `security-review/hardened.1/`; other historical logs remain under `security-review/`.

# Security Review — PandaCode 0.3.28 / hardened.1

Date: October 23, 2026. Report language: English.

This is a fork of [PandaCode](https://github.com/aicodeking/pandacode). The original project's license, credits, architecture, and features were preserved. `hardened.1` identifies this reviewed distribution; internal version 0.3.28 remains for compatibility.

## Result

**The original copy does not meet the requirement to avoid automatic external updates and reduce data egress without specific authorization.** Concrete paths were found for premature bot data transmission, remote revocation/replay issues, inadequate storage of optional keys, automatic installation with credentials in the environment, and dependencies with available security fixes. Some behaviors were documented features; they are not, by themselves, proof of malware.

Corrections are in this tree, based on the supplied ZIP. The review found no hidden internal instruction directing AI to steal code, keys, or data for a separate collection server. It also did not confirm real embedded credentials in the first-party files examined. These findings describe the reviewed scope and are not certification that the project is free of malware or unknown vulnerabilities.

Use of selected models, command execution, Git, teams, tasks, terminal, explicit deployments, and optional integrations was preserved. This distribution is not a firewall or an additional sandbox for every command an AI may run.

## Origin and Scope

| Item | Record |
| --- | --- |
| Supplied file | `pandacode-0.3.28.zip` |
| Original SHA-256 | `4d74c67ab175967fa247852cd8d678e3908cb51de9d5a9292b1dd5dc6d500836` |
| Original inventory | 888 ZIP entries, 820 files; extraction had no path traversal or symlinks |
| Project | Tauri 2/Rust desktop app, TypeScript/Vite frontend; relay, website, and installer sources included |
| Review | Credential flows, internal prompts, external calls, updates/installation, IPC, browser/previews, Connect, bots/webhooks, cloud, resource limits, and pinned dependencies |
| Change record | `security-review/hardened.1/changes.patch` and `security-review/hardened.1/manifest.json` |
| Evidence | `security-review/hardened.1/verification.json`, logs, and subsystem notes |

Analysis combined source inspection, comparison against an immutable copy, official advisory lookups, executable regressions, existing tests, and an independent review of the highest-risk changes. Dependency queries sent only public package names and versions; repository contents and user credentials were not sent to external scanners. No personal accounts were connected and no paid models were used for testing.

## Findings and Treatment

Severity reflects real-use conditions. “Conditional high” means an integration must be enabled or specific content opened; it does not mean anonymous remote access is available by default.

| Area / severity | Original evidence and consequence | Treatment in this review |
| --- | --- | --- |
| App updates — privacy requirement | `src/app-updates.ts` started checks, downloads, and automatic installation; Tauri config/capabilities exposed updater/restart. UI opt-out depended on user preference. | Inert controller; removed plugin, dependencies, native registration, endpoint, and capabilities. Old preferences cannot re-enable it. |
| Agent updates — privacy requirement | Periodic checks and native install/update commands. | Disabled polling and installation in frontend and native entrypoints. Documented opt-outs are passed to CLIs that support them. |
| npx / version continuity — privacy requirement | Included installer downloaded and installed latest upstream, replacing this distribution's fixes. | Included copy refuses download, installation, and execution, including dry-run. This revision must be built from supplied source. |
| Tauri dependency — high | `Cargo.lock` pinned Tauri 2.11.5, affected by GHSA-w28w-mhc8-qvjv (insufficient channel-message isolation between webviews). | Pinned corrected Tauri 2.11.6. Guest still has no application IPC permissions. |
| HTML preview — conditional high | `file_preview.rs` served workspace HTML/JS with only frame-ancestors CSP. Opened files could run scripts, read same-origin permitted resources, and attempt transmission. Blocking `.env` did not cover all potentially sensitive files. | Sandboxed files without scripts; blocked fetch, workers, forms, frames, objects, and external networking. Guest navigation restricted to preview origin. Local HTML/CSS/images remain visible. |
| Connect revocation — conditional high | Queue retained stale trust; slow operations could continue after revocation. Key-change and send-after-trust-loss windows existed. | Revalidate authenticating key, pin, trust, and hosting during dispatch, after slow preparation, and before responses/events. |
| Connect replay — conditional medium/high | Extreme timestamp could overflow; responses/events lacked anti-replay metadata; request cache was volatile. | Safe arithmetic; timestamp/nonce in all encrypted bodies; persistent receipts before admission, fail-closed and resistant to clock rollback. |
| Bot notify — conditional high | `pandacode_bot_notify` was self-scoped and could proactively send text without specific approval, including Full access. | Notify always requires approval; broad/“always” rules do not make authorization permanent. |
| External bot approval request — conditional high | `bots/gateway.rs` sent the first 600 action characters to the channel before user approval. | Channel receives a fixed notice without action arguments/content. Details remain on desktop. |
| Scheduled results — privacy preference | Results automatically went to the last external channel. This was functional behavior, not a malicious instruction. | Summary stays on desktop. Proactive external delivery requires an approved tool call. |
| Revoked channel/link — medium | Collected events and long polls could continue after disabling a channel or renewing a token. | Revalidate channel, ownership, and token at admission, during waits, and before each sent part. Failures do not report false success. |
| WhatsApp — medium | Signed webhook lacked sufficient persistent deduplication, age checks, and binding to configured phone number. | Check `phone_number_id`, ID, and timestamp; persist receipt before admission; fail closed at capacity. |
| Jev — medium, local credential | Optional keys were serialized in `jev/settings.json`; private permissions were not equivalent to a system vault. | Migrate to system vault with UUID reference, atomic commit, and rollback. Failure preserves recovery and blocks destructive use/save. |
| Deploy CLI — conditional high | If missing, Vercel/Wrangler/Railway ran via `npx --yes` with tokens in the environment. Downloaded code/scripts inherited tokens. | Require a preinstalled CLI; remove automatic download from this privileged bridge. |
| Cloud endpoint/credentials — conditional high | Vercel `--api` and AWS `--endpoint-url`/`--no-verify-ssl` could redirect credential use or remove TLS checks. | Block sensitive options, credential operations, and AWS abbreviations; ignore inherited AWS endpoints. |
| HTTP limits — medium availability | Some bodies were limited only after `bytes()`, enabling large allocation for chunked responses; bot HTTP accepted ambiguous framing. | Limit while streaming; cap WebSocket messages and HTTP connections; reject duplicate headers and unsupported framing. |
| AWS user data — conditional medium | `set -eux` could expose VNC password in trace; local temp file lacked exclusive creation/explicit permissions. | Disable xtrace before secrets; exclusive file, 0600 on Unix, cleanup guard on success/error, reject symlink/preexisting file. |
| TightVNC in AWS guest — conditional medium | External MSI ran without signature/publisher verification. | Private staging; require valid Authenticode and expected GlavSoft publisher before msiexec; no redirects; cleanup and exit-code check. |
| VM desktop — installation preference | Creating/starting a Linux VM could install desktop/packages in background. | Install only through explicit Add a desktop action. |
| Prices and DNS — traffic reduction | Cost estimate fetched public price table daily; links could allow speculative DNS depending on engine. | Use saved/embedded prices, no refresh; disable DNS prefetch in app and preview. |

`notes-egress.md`, `notes-remote.md`, and `notes-updates.md` record original files/lines, functions, conditions, tests, and decisions in more detail. They are working notes; the validation table below is the consolidated final result.

### Why Disabling UI Options Alone Was Not Enough

Turning off app updates and leaving Connect, Jev, or channels disconnected would avoid some optional paths. It would not fix the vulnerable Tauri library, active file previews, body limits, or defects that arise when integrations are used. A corrected version was therefore prepared instead of declaring the original ZIP safe without changes.

## Dependencies and Provenance

| Component | Original | This review | Reason / evidence |
| --- | --- | --- | --- |
| Tauri | 2.11.5 | 2.11.6 | GHSA-w28w-mhc8-qvjv, isolation between webviews |
| DOMPurify | 3.4.15 in lock | 3.4.17 | GHSA-mrv2-3pqr-j5jr and GHSA-cg2g-xh64-v4q7; specific conditions not demonstrated in app, preventive update |
| KaTeX | 0.16.47 | 0.18.2 | GHSA-238p-pmpm-9mq7; exploitation requires prior prototype pollution, not demonstrated here |
| Vite | 7.3.6 | 7.3.7 | Fixes in 7.x for dev server/Windows; do not confuse dev server with a server exposed by default |
| source-map-js | 1.2.1 | 1.2.2 | GHSA-68fv-2mgg-jv7q, CPU exhaustion with malicious source map |
| rustls | 0.23.45 | 0.23.46 | RUSTSEC-2026-0346 through 0349; several specific conditions are not enabled here |
| rustls-webpki | 0.103.15 | 0.103.16 | RUSTSEC-2026-0382, rejection of distrusted emSign intermediates |
| webpki-roots / root-certs | 1.0.9 | 1.0.10 | RUSTSEC-2026-0381/0383, trust store; wrapper 0.26.11 resolves the corrected parent |
| zip | 2.4.2 | 9.0.0 | RUSTSEC-2026-0367/0368/0369; extraction code also removed from resolved tree |
| glib | 0.18.5 | 0.18.5 + local backport | RUSTSEC-2024-0429: two lines from official fix, GTK3-compatible types retained |

The GLib backport preserves the authentic license/version, with hashes and verifier in `src-tauri/vendor/glib/BACKPORT.md` and `BACKPORT_PROVENANCE.json`. The version was not falsified to silence tools. The optimized iterator test covers the behavior behind the advisory. The ZIP update retains the VSIX consumer and requires Rust 1.88 or later.

**npm:** the final root lockfile query returned zero known vulnerabilities. This does not automatically cover packages users install later, external CLIs, or every optional subproject.

**Rust:** OSV was queried for 591 registry packages and local GLib by name/version, 592 queries total. TLS and ZIP advisories no longer appeared. The public database still flags GLib; the issue is addressed by the verifiable backport. Nine maintenance notices remain: `proc-macro-error`, `unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident`, `unic-ucd-version`, `zbus_names`, `zvariant`, and `zvariant_derive`. These are transitive dependency maintenance debt; they were not reclassified as proven exploits or hidden.

## Validation Performed

Environment: Linux x86_64, Node 24.19.0, Rust/Cargo 1.99.1, GTK/WebKit libraries, and isolated test browsers. npm dependencies were installed with install scripts disabled. UI tests use local simulated data. Relevant HTTP tests use loopback.

| Check | Consolidated result |
| --- | --- |
| TypeScript + production bundle | Passed: TypeScript and final Vite bundle |
| App Node tests | 126 passed; zero failures |
| Release-script / npx installer tests | 39 passed; zero failures |
| Full Rust backend | 1,189 passed; zero failures; 39 explicitly ignored |
| Chromium UI, ten affected files | 95 scenarios passed across two runs: 90 in initial suite and five VM cases after correcting the fixture's simulated platform (MacIntel to Tart); no remaining failures |
| Native preview server + Chromium/WebKit | Passed: one native fixture ran both engines, including blocked scripts/fetch, HTML/CSS/SVG, cookies, reload, and restore |
| GLib + ZIP compatibility, release optimization | 6 passed; zero failures |
| Persistent remote receipts, isolated production module | 4 passed; zero failures; also integrated into native suite |
| Parser/timestamp comparative proofs | Original fails, fix passes; overflow also exercised with optimization |
| GLib provenance | 121 upstream files verified; only the patch file differs in the two documented lines |
| Hardcoded secrets | 591 first-party files; no real credentials confirmed after triage; sanitized report included |
| npm audit / OSV | JSON results included, subject to caveats above |

Ignored tests were not counted as successes. They include integrations requiring authenticated CLIs, platforms, or external services. Real file preview was explicitly exercised separately where indicated. Logs, commands, and results are in `security-review/`; a test's mere presence in source does not mean it ran.

## Behavior and Compatibility

### Data Egress That Remains by User Choice

| Selected feature | Data/destination | How to avoid it when unwanted |
| --- | --- | --- |
| AI provider/CLI | Prompt, attachments, context, and tool results go to configured provider | Choose provider/endpoint and limit context/permissions for the task |
| Jev | Visible goal/text and, during assistance, selected snippets/diffs go to TypeSafe and configured model | Keep Jev features disabled |
| Connect | GitHub login sends token to configured relay for exchange; relay then forwards encrypted traffic between devices | Do not sign in; if connected, sign out |
| Bots/channels | Reply to selected external conversation goes to the channel; proactive notify requires approval | Disable/remove channel and renew link as needed |
| Deploy/cloud | Code/artifacts and credentials go to selected service, subject to approval | Do not connect/provision/deploy the service |
| Browser, Git/GitHub, shell | Traffic from explicitly used sites, repositories, and commands | Control these actions in the task; they are retained orchestrator features |

The app does not replace provider credentials with a collection endpoint. This review does not promise that code intentionally sent to a model remains exclusively on the PC.

### Intentional Convenience Changes

- App/agent updates and included npx installer are disabled in code, with no switch to re-enable them.
- Estimates use local prices, which may be outdated.
- File preview does not execute JavaScript. To test an interactive app, explicitly open its development server in a browser.
- Scheduled summaries are not automatically sent to external channels; notify requires new approval.
- Deploy CLIs must be installed manually before use; Linux VM desktop installation requires an explicit action.
- Connect requires hardened.1 on both computers. Older frames without metadata are rejected.

## Material Limitations

1. **Platforms:** Linux backend and frontend were built/tested. No Windows/macOS installer was produced or validated in this review. AWS guest PowerShell/Authenticode flow and real vaults on all three platforms were not tested natively; vault tests use the project's test implementation.
2. **External CLIs:** opt-out flags/variables are not network blocks. A CLI, plugin, custom configuration, or authorized command can download/send data. Provider settings may override inherited variables. External executables are not contained or fully audited in this ZIP.
3. **Key storage:** runtime-required keys exist in RAM. Jev migration with unavailable vault preserves the legacy file, including plaintext, for recovery and blocks use. Old backups are not retroactively deleted.
4. **Connect:** a crash after receipt commit but before execution can lose a message; avoiding duplicate execution takes priority. A backward clock blocks until the watermark. Failure to open the ledger requires repair/restart. Protection does not resist deliberate restoration of the entire local data directory by its owner/admin, nor does it prevent relay delay/drop/reordering of first delivery. Revocation does not undo work already started.
5. **Optional AWS:** guest password appears in user data and Windows guest uses autologon/DefaultPassword in the registry; MSI receives the VNC password during installation. This is not general encryption of guest secrets. VNC is loopback-only and uses SSM, but the account's default security group still determines other ingress. The official MSI download was not obtained as a valid binary in this environment; runtime signature/publisher verification was therefore required without inventing a checksum.
6. **Security scope:** no Windows installer detonation, audit of every external package, formal proof of no exfiltration, or full assessment of third-party services was performed. This is a review with fixes and reproducible evidence for identified risks.

## Build and Use This Review

The ZIP is a **source-code** distribution containing the complete existing project, patch, lockfiles, and evidence. To obtain these controls, build this tree on the target system. Upstream downloads, Homebrew, and public `npx pandacode` do not automatically include these changes.

Install Node 22+, stable Rust 1.88+, and the system's [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Windows, follow the Windows section for MSVC and WebView2. These downloads are manual build-preparation steps.

```sh
cd pandacode-0.3.28-hardened
npm ci --ignore-scripts
npm run build
npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npm run tauri -- build -- --locked
```

For local development, run `npm run desktop`. The app identifier was retained for compatibility: a local build may open/migrate data from a previous installation with that identifier. Jev migration is designed to preserve secrets on failure. The About interface identifies `hardened.1`.

Reproduce independent backport/ZIP verification:

```sh
python src-tauri/vendor/glib/verify-backport.py
cargo test --release --locked --manifest-path src-tauri/security-tests/Cargo.toml
```

Future updates should be deliberate manual operations, with diff review and maintenance of these controls. Website sources, historical documentation, and upstream automation were preserved to maintain the project; they do not represent an installer for this review.

## Primary Sources Consulted

- [Tauri: GHSA-w28w-mhc8-qvjv](https://github.com/tauri-apps/tauri/security/advisories/GHSA-w28w-mhc8-qvjv).
- [DOMPurify: GHSA-mrv2-3pqr-j5jr](https://github.com/cure53/DOMPurify/security/advisories/GHSA-mrv2-3pqr-j5jr) and [GHSA-cg2g-xh64-v4q7](https://github.com/cure53/DOMPurify/security/advisories/GHSA-cg2g-xh64-v4q7).
- [Vite: GHSA-rq7h-c2jc-7f22](https://github.com/vitejs/vite/security/advisories/GHSA-rq7h-c2jc-7f22); other IDs are in the original npm JSON.
- [KaTeX: GHSA-238p-pmpm-9mq7](https://github.com/KaTeX/KaTeX/security/advisories/GHSA-238p-pmpm-9mq7).
- [source-map-js 1.2.2](https://github.com/7rulnik/source-map-js/releases/tag/v1.2.2).
- [RustSec 0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html), [0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html), [0383](https://rustsec.org/advisories/RUSTSEC-2026-0383.html), and [ZIP 0367](https://rustsec.org/advisories/RUSTSEC-2026-0367.html). OSV JSON includes other notices and references.
- [Official GLib patch](https://github.com/gtk-rs/gtk-rs-core/pull/1343), commit `b5a4071e439bef2b5eea76c3aa25e5ae84839e34`.
- [Claude: environment variables and precedence](https://code.claude.com/docs/en/env-vars), [Kimi: variables](https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/env-vars), [OpenCode: CLI](https://opencode.ai/docs/cli/).
- [AWS: endpoints](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-endpoints.html) and [abbreviated options](https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-troubleshooting.html).
- [Vercel: global options](https://vercel.com/docs/cli/global-options), [Wrangler: general commands](https://developers.cloudflare.com/workers/wrangler/commands/general/).
- [TightVNC: official distribution](https://www.tightvnc.com/download/), [maintainer guidance on signing](https://sourceforge.net/p/vnc-tight/mailman/vnc-tight-list/?limit=25&page=7), [Microsoft: Get-AuthenticodeSignature](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/get-authenticodesignature).

The VM fixture simulates macOS in a Linux browser; this is not equivalent to testing Tart on a real Mac. Initial and repeated runs of the five cases are preserved in logs. GLib style/lifetime warnings, the unused Live pricing variant, and Vite chunk-size warnings were not hidden; they did not prevent build/tests.

Local evidence of project behavior consists of original files identified by hash, the patch, and included tests. External links document advisories and vendor contracts; they do not replace this local evidence.
