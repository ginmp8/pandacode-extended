> Work/evidence note. Consolidated test results are in ../SECURITY-HARDENING.md and verification.json.

# PandaCode 0.3.28 — Credentials, Data Egress, and Bot Approval

## Scope and Status

Review of immutable `audit/original/pandacode-0.3.28` with scoped fixes in `audit/pandacode-0.3.28-hardened`. Examined internal prompts, Jev integration, cloud CLI bridge, secret storage, bot tools/gateway, provider gates, shared HTTP, and—during a second independent review—the main agent's file/browser preview changes and rich UI surfaces.

This note distinguishes confirmed vulnerabilities, deliberate hardening for the user's privacy preference, and remaining limits. No internal prompt directing theft of code/keys or hidden collection server was found in scope. This does not guarantee the entire project/dependency tree is free of malware.

Rust tests described below were added but **were not run by this subagent**. The main agent owns native builds/tests and final results supersede this intermediate state. No code/credentials were sent to scanners, and no malicious payloads were run.

## Findings and Fixes

### EGR-01 — Jev Keys Persisted in Readable JSON (Medium; Local Credentials)

**Original evidence:** `src-tauri/src/jev.rs:56–65` included `typesafe_key` and `text_key` in serializable `Saved`. `jev_save_configuration`, lines 497–499, serialized all of `next` to `jev/settings.json`. Original UI `src/jev.ts:155` explicitly said keys were stored outside keychain. Private chmod reduces other Unix users' access but is not a system vault and does not protect copies/backups or same-user processes.

**Precondition:** user saves at least one Jev key. Both Jev features are off by default (`Settings::default`, lines 47–48); disabling a feature does not remove a saved key. UI has per-key Remove (`src/jev.ts:185–190, 233–234`).

**Fix:** `typesafe_key`/`text_key` retain legacy deserialization but are never serialized. JSON now stores only UUID reference and presence flags; values live in existing `secret_store`, service `app.pandacode.jev`. Each save creates a new vault generation; atomic JSON replacement is commit point; old generation is removed only afterward. Partial vault/file failure cleans the new generation and preserves old one. Loading migrates legacy JSON. If vault is locked/unavailable, integration is disabled and original file remains recoverable. A later Save first attempts reload/migration and fails without changing original if secrets remain unavailable.

**Tests added:** `legacy_credentials_move_to_the_vault_and_restart_without_plaintext`; `failed_vault_or_settings_write_preserves_previous_credential_generation`; `saving_after_unavailable_vault_preserves_the_recovery_file`; `clearing_credentials_commits_empty_refs_before_removing_old_vault_entries`.

**Limits:** tests use the project's existing in-memory vault under `cfg(test)`; they do not prove real Keychain/Secret Service/Credential Manager integration. Entry removal is best effort in the current API. Runtime `Saved` retains values in RAM; public `Configuration` returns booleans, not keys. Failed migration leaves legacy plaintext intact for recovery and disables use. Do not claim guaranteed removal of old copies/backups or absence of plaintext in RAM.

### EGR-02 — Automatic CLI Download/Execution with Credentials in Environment (Conditional High; Supply Chain)

**Original evidence:** `src-tauri/src/cloud/deploy.rs:240–267`: if Vercel/Wrangler/Railway was missing, `prepare` used `npx --yes` with package versions pinned at lines 20–22. Tokens were added to the same subprocess environment at lines 248, 258, 265; npx dependency/install scripts inherited it.

**Precondition:** cloud connected, user approved deploy tool, CLI absent. Pinning reduces accidental version drift but does not prevent downloaded code, transitive dependencies, or scripts from accessing credentials; this is not evidence of malicious package in ZIP.

**Fix:** removed all npx fallbacks. Tool accepts only preinstalled CLI and otherwise instructs manual installation of a reviewed version. App fallback downloads nothing. Cloud remains optional; deploy approval retained.

### EGR-03 — Destination Overrides and Credential Operations in Cloud Bridge (Conditional High)

**Original evidence:** `pandacode_cloud_cli` args reached Vercel/Wrangler through `command.args(args)` (`deploy.rs:248,258`) while vault token was in environment; no filter existed for these commands. Vercel documents `--api` to change API base URL. Original AWS protection, lines 88–106, blocked only `--debug` and some credential-return commands; `--endpoint-url`, `--no-verify-ssl`, and endpoints from environment/config remained allowed.

**Impact:** credentials kept out of prompt could be used by CLI against a destination specified in args. AWS signs requests and may transmit session token; do not claim signing secret access key is always sent as plaintext. `vercel tokens` and `wrangler auth token` also conflict with promise not to expose credentials to agent; literal redaction does not cover newly created credentials.

**Fix:** `keyed_cli_refusal` blocks API/config/directory overrides, debug, authentication/token operations, and recognized local execution in Vercel/Wrangler; normal deployments remain allowed. AWS blocks debug, arbitrary endpoint, TLS disable—including long argparse abbreviations such as `--endpoint`, `--endp=`, `--no-verify`, `--debu`—and sets `AWS_IGNORE_CONFIGURED_ENDPOINT_URLS=true`. Generic redactor removes all nonempty secrets, including values under eight characters.

**Test:** `credential_bridge_refuses_endpoint_and_credential_operations` covers `=`, attached short flags, AWS abbreviations, credential commands, allowed deploy examples; existing scrub test includes short key.

**Limit:** restriction applies to privileged bridge, not a complete CLI sandbox. Installed CLIs, user login/config, and local hooks from authorized builds remain trust dependencies. An agent with unrestricted shell may try other network mechanisms; user's choice to preserve full local command execution remains. This review cannot guarantee all future CLI behavior is constrained by a flag list.

### EGR-04 — Bot Could Proactively Send Without Approval (Conditional High)

**Original evidence:** `bots/tools.rs:29–30` treated all registered bot tools, including `pandacode_bot_notify`, as `self_scoped`. `with_bot:44–48` did not add notify to approval list. Dispatcher lines 73–74 called `gateway::notify`, which sent to last external channel (`gateway.rs:326–329`). `providers/mcp_host.rs:494` and `providers/host_tools.rs:255` protected only cloud tool in full mode.

**Precondition:** bot with external destination/last channel and a task. Creating/pairing channel expresses communication intent but should not approve every later code/key send. Disabling channels in UI is possible but did not fix approval enforcement when used.

**Fix:** notify removed from `self_scoped`, added to `approval_tools` and `ALWAYS_ASK`; both hosts require approval even in full mode. Bot guard prevents broad rules or “always” replies from permanently remembering notify/cloud_cli permission. Bot memory/local operations retained. Send failure now returns error rather than `sent=true`.

**Tests:** `outbound_tools_ask_even_in_full_mode` in MCP and host_tools verifies handler not called before decision; MCP denial does not execute. `outbound_bot_tools_cannot_be_covered_by_broad_allow_rules` tests protected precedence. `only_registered_bot_tools_skip_approval` updated for new boundary.

### EGR-05 — Approval Request Sent Sensitive Content Before Authorization (Conditional High)

**Original evidence:** `bots/gateway.rs:284–288` sent first 600 chars of `text` (action description/arguments) to external channel as approval request. A command/text containing a secret would leave the PC before denial could stop the action.

**Fix:** external approval message is fixed and excludes name/arguments/payload; asks user to review full action on desktop and answer yes/no for that action. Local UI retains decision details. Results for an externally initiated conversation still work.

### EGR-06 — Scheduled Results Were Forwarded Automatically (Deliberate Privacy Change)

**Original evidence:** `bots/gateway.rs:194–200` called `notify` with final scheduled-work text, sent to last external channel. This was documented behavior, not malware evidence.

**Fix:** scheduled summary appears on desktop via `notify_local`. Proactive external transmission uses notify with explicit approval. Schedules/local execution retained. Generic pending-action notice may still go to channel without action content.

### EGR-07 — Gateway Did Not Revalidate Revoked Channel (Conditional Medium/High)

**Original evidence:** `bots/gateway.rs:91–101` checked sender pairing but not `channel.enabled` or channel ownership by bot. An event already obtained by poller could be admitted after revocation.

**Fix:** `validate_channel` checks existence, enabled state, and ownership at ingress and again before composing/enqueuing task. Desktop and internal scheduled tasks have explicit treatment. Remote agent separately implemented immediate validation in `channels::send`; errors propagate through notify, avoiding false success. In-flight network packet cannot be withdrawn after revocation.

**Test:** `bot_admission_rechecks_channel_revocation_and_ownership` covers enabled channel, different bot, disabled channel, and desktop.

### EGR-08 — HTTP Body Limit Checked After Full Allocation (Medium; Availability)

**Original evidence:** `cloud/http.rs:40–45` checked Content-Length if present but called `response.bytes()` before actual-size check. A configured endpoint or malicious chunked response without Content-Length could cause excessive allocation.

**Fix:** read through `response.chunk()`, compare saturating total against 16 MiB before extending buffer. Preliminary Content-Length check retained.

**Test:** `rejects_oversized_chunked_responses_without_content_length` uses loopback-only server with oversized chunked body and requires rejection; no external server.

## Primary CLI Documentation

- Vercel global options: https://vercel.com/docs/cli/global-options and https://github.com/vercel/vercel/blob/main/skills/vercel-cli/references/global-options.md (`--api`, config/cwd/debug).
- Vercel tokens: https://vercel.com/docs/cli/tokens.
- Cloudflare Wrangler general commands: https://developers.cloudflare.com/workers/wrangler/commands/general/ (`auth token` and `--json`).
- AWS endpoints: https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-endpoints.html.
- AWS abbreviated options: https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-troubleshooting.html, incomplete parameter names/argparse `allow_abbrev` section.
- Claude environment variables (forwarded to updates agent): https://code.claude.com/docs/en/env-vars.

## Examined Flows Not Classified as Hidden Collection

- `lib.rs:447–459` and helper prompts guide title, run scripts, change summaries, and rich UI. No instruction to send data to a secret domain was found.
- `bots/brief.rs:47–50` requires approval before external send, prohibits revealing tokens/keys, and says treat pages/files as data. Confirmed issue was enforcement, not malicious prompt text.
- Selected providers (including GLM/MiMo) and Jev text model are explicit AI integrations; sending context to selected provider is expected. Jev text/model base is configurable with HTTPS validation; user choice is not a hidden endpoint.
- `support.rs:6` links to GitHub issues; opening is user initiated and prepared details are version/platform/architecture, not silent repository upload.
- `code_search.rs:46–49` runs local codedb with `--no-telemetry`, `CODEDB_NO_TELEMETRY=1`, and `CODEDB_NO_CLI_DAEMON=1`.
- Existing cloud keys use `secret_store`; Jev finding should not be generalized incorrectly to every application credential.

## Independent Second Review of Preview and Rich UI

Reviewed main agent's changes to `file_preview.rs` and `browser.rs` without editing those files.

- `file_preview.rs:23–25`: separate CSP for files/bootstrap. Files use `script-src 'none'`, `sandbox allow-same-origin`, block external connect/worker/object/frame/form. Bootstrap allows only its generated JS for Strict cookie acquisition and redirect.
- `file_preview.rs:559–568`: path reconstructed from URL and serialized path in JSON; body does not interpolate workspace HTML. Search/hash appended at runtime as strings. No filename injection into script identified in this implementation.
- Headers include nosniff, no-store, CORP same-origin, X-Frame-Options DENY, no-referrer, and `X-DNS-Prefetch-Control: off`. Host/Origin/Fetch-Metadata/referrer and cookie remain controlled; symlinks canonicalized inside workspace.
- `browser.rs:47–56,257–279`: `preview_origin` set by trusted UI `browser_open`; `on_navigation` rejects origin change while preview is open. Popups/downloads remain denied. Static inspection found no blocking security regression; real coverage depends on native engines and main agent tests.
- Rich markdown/HTML uses DOMPurify; HTML removes link/meta/base; custom views remove resources and sanitize CSS. Native UI CSP restricts SVG/CSS URLs not removed by sanitizer. Design canvas uses raw srcdoc in sandboxed iframe without scripts and inherits app CSP. No other high-severity egress was confirmed in these flows.
- Suggested to main agent as additional defense, without claiming reproduced leakage: add early `x-dns-prefetch-control=off` meta in main HTML to prevent speculative link/Design DNS resolution in engines that enable it. Chromium source: https://www.chromium.org/developers/design-documents/dns-prefetching/.

## Files Modified by This Subagent

`src-tauri/src/jev.rs`; `src-tauri/src/cloud/deploy.rs`; `src-tauri/src/cloud/http.rs`; `src-tauri/src/bots/tools.rs`; `src-tauri/src/bots/gateway.rs`; `src-tauri/src/bots/guard.rs`; parts of `src-tauri/src/bots/tests.rs`; specific gates/tests in `src-tauri/src/providers/mcp_host.rs` and `src-tauri/src/providers/host_tools.rs`.

Channel/remote changes, child_env/updaters, Jev frontend, browser/preview, dependencies, and final documentation belong to other agents. This subagent created no commit or remote submission.

## Requested Addendum: AWS Provisioning and Speculative DNS

Main agent added `src-tauri/src/cloud/aws.rs` and the `index.html` DNS meta tag to scope after review above.

### EGR-09 — Guest Password in Tracing and Permissive Temporary File (Medium; Optional VM Secret)

**Original evidence:** `cloud/aws.rs:51` emitted `set -eux`. Line 56's `printf` included password as shell argument, printed to stderr/cloud-init log by `-x`. `create:178–179` wrote user data (containing password) through `std::fs::write` in temp directory: file creation depended on umask and could truncate an existing path. Removal after `self.call` covered ordinary success/error, but no lifetime guard guaranteed cleanup on other returns/unwind.

**Fix:** Linux prologue uses `set +x` before secret, then `set -euo pipefail`. `umask 077` is applied after apt installation, protecting VNC files without changing system package file creation. `UserDataFile` creates with `create_new(true)` and Unix mode `0o600` before writing, syncs and closes handle. Guard is built only after successful creation; a collision/symlink is neither overwritten nor removed. `Drop` removes file after use, including AWS call error; caller also drops explicitly immediately after `call`.

Internal AWS provisioning command also sets `AWS_IGNORE_CONFIGURED_ENDPOINT_URLS=true`, as deploy bridge does, so user-data upload cannot be silently redirected by inherited CLI endpoint config.

**Tests added:** `user_data_file_is_private_exclusive_and_removed_after_use`, `user_data_file_is_removed_when_the_caller_fails`, `user_data_file_never_follows_or_removes_an_existing_symlink` (Unix), `linux_user_data_disables_inherited_tracing_before_secrets` (Unix). Last test runs only generated prologue and a fixture printf to `/dev/null` under `bash -x`, without apt/network/provisioning, and requires fixture secret absent from stderr.

**Limits:** guard attempts cleanup during normal flow/unwind; no guarantee after abrupt crash/kill/power loss or forensic disk erasure. Windows inherits user's temp-directory ACL; 0600 change is Unix-only. EC2 user data still carries secret to guest user explicitly provisioned. This is not encryption of user data, VM, or AWS backups.

### EGR-10 — TightVNC MSI Ran Without Signature Validation (Conditional High; Windows VM)

**Original evidence:** `cloud/aws.rs:92–94` downloaded `tightvnc-2.8.90-gpl-setup-64bit.msi` and immediately ran msiexec as admin without hash/signature; used fixed TEMP name and did not remove installer. Administrator password/autologon were set before confirming install worked.

**Research/choice:** Current official page confirms version 2.8.90 and x64 installer size 2,543,616 bytes: https://www.tightvnc.com/download/. A verifiable binary could not be obtained in audit environment: earlier main-agent attempt received error HTML; this subagent attempt was blocked by sandbox network. No hash of error page or aggregator was used as anchor. Maintainer Constantin Kaplinsky advises checking valid signature from “GlavSoft LLC” or “GlavSoft OOO” rather than checksum: https://sourceforge.net/p/vnc-tight/mailman/vnc-tight-list/?limit=25&page=7, “Re: Checksums for latest installers?”, 2021-03-23. Official page https://www.tightvnc.com/announce-2.0beta2.php also identifies GlavSoft LLC signing.

**Chosen fix:** fail-closed Authenticode validation, as authorized by main agent. Keeps URL/version 2.8.90 and explicit VM creation. Creates random staging with noninherited DACL limited to SYSTEM/Administrators. Download timeout 120s; redirects forbidden. `Get-AuthenticodeSignature` must return `Status=Valid` and existing signer certificate; `GetNameInfo(SimpleName,false)` must exactly match `GlavSoft LLC`, `GlavSoft LLC.`, or `GlavSoft OOO`, no wildcard. Only then runs System32 msiexec; accepts codes 0/3010. `finally` removes staging/download on success/failure. Administrator password/autologon are set only after successful install and cleanup.

Microsoft verifier docs: https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/get-authenticode-signature?view=powershell-5.1. Signature validates origin/integrity against guest certificate trust; it does not prove program has no vulnerabilities or publisher was never compromised. No pinned checksum is used in this alternative.

**Test added:** `windows_verifies_publisher_before_install_and_cleans_up_before_autologon` checks generated script contract: check ordering, exact publisher, execution only after checks, finally block, and autologon after cleanup. Structural test does not replace PowerShell/Authenticode execution on Windows. Windows/VM flow was not run here; download/signature failure aborts install without unsafe fallback.

**Secrets retained by explicit design:** to open Windows desktop without lock screen, project still configures Administrator and Winlogon AutoAdminLogon, including `DefaultPassword` in **AWS guest** registry. VNC uses traditional eight-character authentication, restricted to loopback and SSM tunnel; this password also appears in install parameters inside guest. User guidance must say not to use VM as secret vault, manage IAM/SSM, and terminate VM when no longer needed. Change prevents accidental Linux log and improves local temp storage; it does not remove every guest secret or change cloud access model.

`run-instances` does not select its own security group: account/VPC default SG policy still applies, including possible intra-SG rules. Old comment promised “no inbound ports”; updated to match implementation: creates no SSH key pair and adds no VNC ingress rule; VNC is loopback-only. This audit created/changed no real AWS resources or account network rules.

### EGR-11 — Additional Defense Against Speculative DNS

`index.html` now includes `<meta http-equiv="x-dns-prefetch-control" content="off">` immediately after charset, before links/rendering. Expected effect: disable speculative host resolution without blocking explicitly opened links. Chromium documents that explicit opt-out cannot later be re-enabled by content and child frames inherit it: https://www.chromium.org/developers/design-documents/dns-prefetching/. This is additional hardening, not a leak reproduced in original app.

**Verification run:** extracted emitted Linux prologue from `aws.rs` and ran under `bash -x` with only a fixture printf to `/dev/null`. Passed: exit code zero, empty stdout, fixture absent from stderr. No provisioning command ran. `aws.rs` diff also had no whitespace alerts.

**Final status of this subagent:** edits and tests added; no parallel Rust compilation/tests and no npm build. Main agent will report native execution results.
