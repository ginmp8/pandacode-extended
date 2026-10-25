# Extended 0.5.0: historical PandaCode 0.3.28–0.3.42 coverage

This integration closes compatible historical gaps in the unpublished **0.5.0** candidate. Its display identity remains **0.5.0+pc.0.3.47**. It supplements the [0.3.42–0.3.47 integration](upstream-integration-0.5.0.md) and preserves Extended's additional behavior and security boundaries. [Issue #24](https://github.com/ginmp8/pandacode-extended/issues/24) records the implementation scope and acceptance requirements.

## Inputs, evidence and branch contract

| Input | Frozen identity |
| --- | --- |
| Extended base | `feat/0.5.0-upstream-0.3.47`, `7f14199973d7bfc62dfb07fa08cb1e3bbd328c5e` |
| Implementation branch | `feat/0.5.0-historical-parity-0.3.28-0.3.42` |
| Pull-request target | `feat/0.5.0-upstream-0.3.47` |
| Original 0.3.28 | `70bdc708303c8681d8eb0b46f2cdb0ff087aa6eb` |
| Original 0.3.42 | `bca0b9f1f03a1cc3f8a8e6330881c34103f14a46` |
| Retained newer reference | 0.3.47, `4187553cb48db96657609948f6b0904556845d45` |
| Release evidence manifest | [upstream-release-evidence-0.3.28-0.3.42.json](upstream-release-evidence-0.3.28-0.3.42.json) |
| Manifest SHA-256, LF | `1222807342583b683749d2014d200a8ea52bbe78479c9f9fb5581d30d8067e71` |

The feature sections of all **14 published GitHub Releases** in this interval match the original checked-in notes. GitHub adds an installation section to each body; those installation claims are not Extended installation guidance. The manifest records both source-note and full GitHub-body digests. It includes all 15 source notes and marks **0.3.36 as source-only**: the release lookup returned no published release, and its text duplicates 0.3.37. No publication date is asserted for 0.3.36.

The source audit also compares changes between the immutable tags. Release headlines alone omit prerequisites such as Kilo, bot profiles and approval-mode wiring. Original notes stay unchanged under `releases/`; this document states Extended behavior. `HARDENED-REVISION.json`, earlier published Extended notes, source ancestry, and the earlier integration's provenance remain unchanged. The metadata retains `full_upstream_parity: false` and `upstream_code_rebased: false` because the explicit policy differences below remain.

## Release-by-release feature map

“Preserved” means the implementation was already in the frozen Extended base. “Adapted” means this change adds missing functionality with Extended's boundaries. Neither status implies a live-provider or native-installer certification; executed checks are recorded in the [verification report](verification-historical-0.5.0.md).

| Original release | Feature | Extended disposition and implementation |
| --- | --- | --- |
| [0.3.28](https://github.com/aicodeking/pandacode/releases/tag/v0.3.28) | Muse Code MSP sessions, models, tools, questions, images, steering and permission mapping | **Preserved** in `src-tauri/src/providers/muse*`, provider registries and model menus. Muse's own judge and lack of native Plan mode remain provider limitations. |
| [0.3.29](https://github.com/aicodeking/pandacode/releases/tag/v0.3.29) | Connect credential storage | **Adapted safety fix**, without plaintext migration. Malformed existing device keys and invalid UTF-8 fail without creating or saving a replacement identity (`remote/config.rs`). OS protected storage remains authoritative. |
| [0.3.30](https://github.com/aicodeking/pandacode/releases/tag/v0.3.30) | New-task inspector state; Oh My Pi large catalogs/RPC v2; unreadable-store protection | **Preserved** inspector state and bounded RPC chunk assembly. Existing inspector browser regression is now included in normal test discovery. Connect tests cover read errors, malformed values and zero-write refusal. |
| [0.3.31](https://github.com/aicodeking/pandacode/releases/tag/v0.3.31) | Cloud providers and cloud-agent pipeline | **Adapted manual management** in `src-tauri/src/cloud/`, `src/cloud-settings.ts` and the VM panel. Eleven missing provider adapters are added. Automatic cloud-agent runtime, login and checkout transport is excluded. See the capability matrix below. |
| [0.3.32](https://github.com/aicodeking/pandacode/releases/tag/v0.3.32) | Panda/uploaded bot icons and reorganized settings | **Preserved** desktop profiles; **adapted** web-chat header, welcome and assistant avatars. Only validated local PNG data, known panda choices and bounded emoji are rendered; arbitrary remote image URLs are refused (`bots/web.rs`, web assets). |
| [0.3.33](https://github.com/aicodeking/pandacode/releases/tag/v0.3.33) | Tray state, task opening and on-demand usage | **Preserved**, including Extended's exact tray IPC allowlist. Existing tray browser tests now run in the normal suite. Usage remains requested on opening/refresh, with no background polling. |
| [0.3.34](https://github.com/aicodeking/pandacode/releases/tag/v0.3.34) | Plaintext macOS secret migration | **Excluded**. Keychain, Credential Manager and Secret Service remain in use. No promise that OS credential prompts disappear. |
| [0.3.35](https://github.com/aicodeking/pandacode/releases/tag/v0.3.35) | Graphical tray, keyboard navigation, usage bars/reset times | **Preserved** in `src/tray-popover.ts` and `src-tauri/src/tray.rs`; finished-task state remains in memory as upstream documents. |
| 0.3.36 source / [0.3.37](https://github.com/aicodeking/pandacode/releases/tag/v0.3.37) | Mobile tasks, transcript, approvals, input, queue, steering, stop, local projects | **Preserved** through the newer 0.3.47 mobile implementation. **Adapted** compact Inbox search plus computer/project filters and computer-to-Inbox navigation; existing bots, status groups and outbox remain. |
| [0.3.38](https://github.com/aicodeking/pandacode/releases/tag/v0.3.38) | Mobile appearance, project icons, native tabs, offline outbox and busy replies | **Preserved** through the newer port. Same-account discovery never substitutes for explicit pinned-device trust. |
| [0.3.39](https://github.com/aicodeking/pandacode/releases/tag/v0.3.39) | Full-access authorization, replay, revocation, fingerprints, relay limits | **Preserved** stronger native/mobile trust and replay controls. **Adapted** global sign-in limiter, deployment configuration guards, and cheap rejection of oversized throttled frames in `relay/`. |
| [0.3.40](https://github.com/aicodeking/pandacode/releases/tag/v0.3.40) | Per-connection usage/continue and fresh sessions | **Preserved** isolated named accounts, session reuse checks and installed/current picker. Claude/Codex provide supported named-account usage; unsupported GLM/MiMo usage stays explicit, never borrowed from a default account. |
| 0.3.40 | Jev OpenRouter | **Adapted** provider/model selectors, on-demand bounded model catalog, selected-provider credentials, atomic protected-store configuration and sanitized answers (`jev.rs`, `src/jev*.ts`). TypeSafe remains the default. |
| 0.3.40 | InstaBox images and activity leases | **Adapted** explicit manual operations, ready-image validation, bounded provider-supported lease extensions and no privileged connection-reset watcher. |
| 0.3.40 | Cline console and capabilities | **Adapted** installed CLI console support, read-only capability discovery and explicit console permission refusals. No key autodiscovery across accounts. |
| 0.3.40 | Provider layout, single-instance focus and AppImage override | **Adapted** provider plans and OpenCode placement, single-instance plugin pinned to 2.4.5, and `GDK_BACKEND` handling in the local AppImage hook. Updaters remain disabled. |
| 0.3.40 | Linux Browser behavior | **Adapted** explicit system-browser choice. Extended Browser QA and origin restrictions remain; prepared local-file preview origins cannot be opened through the external-browser command. Native Linux WebView geometry still needs platform verification. |
| [0.3.41](https://github.com/aicodeking/pandacode/releases/tag/v0.3.41) | Saved attachment chips | **Preserved** the newer durable attachment implementation and its native ownership/recovery checks. |
| 0.3.41 | File tabs, PDF/DOCX/sheets/images and drop fallback | **Adapted** task-owned tabs and bounded local viewers (`src/file-tabs.ts`, `office-viewer.ts`, `src-tauri/src/workspace/documents.rs`). Unsupported drops insert quoted path/name text only into the original draft. |
| [0.3.42](https://github.com/aicodeking/pandacode/releases/tag/v0.3.42) | Narrow-thread worker-label overflow | **Preserved** shrinkable grid, transcript overflow controls and existing regression oracle. |

## Additional source-level features

### Kilo, Cline and explicit review mode

Kilo joins the native provider registries, ACP transport, named connections, model/slash discovery, Skills, terminal controls, remote host and frontend/mobile identities. Devin and existing providers remain available. Kilo console operations support only modes the installed CLI can enforce: Standard and explicit Full access. Unsupported Plan, Accept edits and effort overrides are rejected instead of silently relaxing policy. Provider environment policy is applied after named-connection options, including disabled auto-update settings.

Cline's installed CLI receives an explicit `--auto-approve false` outside Full access. Capability discovery is read-only and does not install software or search unrelated credentials. Missing local tools remain missing until the user installs them outside the application.

Claude-family **Approve for me** is a separate `review` value, selected explicitly; stored Standard, Plan, Accept edits and Full access values retain their meanings. It is not the default. Native launch, desktop/mobile selectors, remote authorization and team worker configuration must agree on this value. Remote automatic approval is privileged and requires the host's independent Full-access grant, with rechecks before execution. Extended Advisor, Committee and review workflows remain separate features. Automatic `plan_reads` permission widening is not adopted.

### Bot personality and selected capabilities

Bot profiles gain personality text and explicit MCP selections. Skill selections reuse Extended's scoped, hash-pinned records and acknowledgement rules. A bot can be assigned to an eligible unstarted local project task without changing that task's project, worktree or permission mode. Native turn admission supplies the bot brief and tracks its version inside the task-admission transaction; failed, interrupted or stopped runs cannot consume the brief permanently.

Ordinary project tasks do not receive bot-channel notification tools. Chat sessions, mirrors, bot workspaces, team/worker tasks and already-started tasks are refused as binding targets. Bot-bound tasks cannot later become Connect mirrors. Console launch respects the bot context boundary. Asynchronous settings and task selection must recheck the selected bot/task before applying results.

Schema **29** adds the bot profile/task binding fields after Extended's **27** (`github_issue_actions`) and **28** (durable attachments). The upstream schema numbers are not transplanted over Extended migrations. Upgrade and rollback/error paths retain existing task, bot, attachment and issue-action data.

## Manual cloud management

The existing Daytona, Cloudflare, EC2 and AgentCore implementations remain. New cloud backends are commands-only: a desktop/screenshot capability is not fabricated. See [cloud-sandboxes.md](cloud-sandboxes.md) for settings, exact commands, provider prerequisites and limits.

| Added backend | Supported lifecycle beyond list/create/delete | Explicit lease behavior |
| --- | --- | --- |
| Google Cloud VM | Start/stop; installed `gcloud`, private VM and IAP | No lease extension |
| E2B | Start/resume and stop/pause | Up to 1,440 minutes; plan restrictions can limit the request |
| Modal | Exec and delete; prepared OCI image and installed SDK | No extension beyond the original 24-hour lifetime |
| Vercel Sandbox | Exec and delete; configured sandbox project | Up to 300 minutes, with plan-aware fallback |
| CodeSandbox | Start/resume and stop/hibernate; Pint required | Up to 1,440 minutes idle |
| Runloop | Start/resume and stop/suspend | Fixed 360-minute renewal |
| Blaxel | Start/wake; automatic standby, no fabricated Stop | No lease extension |
| Tensorlake | Start/resume and stop/pause; prepared image | No lease extension |
| Freestyle | Start/resume and stop/pause | Up to 1,440 minutes idle |
| boat | Start and stop | Up to 1,440 minutes |
| InstaBox | Start and stop; ready account snapshot or plan template | Up to 60 minutes |

Credentials remain in the protected store. Authenticated HTTP disables redirects and bounds response bytes/time. Provider-returned secondary endpoints require provider-specific validation before secrets can be used. Errors and command output are redacted. User/agent mutations retain existing approval boundaries. GCP does not open firewall rules or public addresses on the user's behalf. Modal/Tensorlake require prepared images; no implicit package installation repairs an unsuitable image. CodeSandbox does not fall back from Pint to an unsupported transport.

This deliberately excludes the original cloud-agent pipeline: no automatic Git/Node/CLI install, copying provider logins or checkout content, unattended task bridge, or background worktree synchronization. Explicit manual sandbox operations do not authorize that separate pipeline.

## Local documents and draft ownership

File tabs are separate from inspector tabs. Opening a file keeps Conversation mounted but inert while the file is active. Each task/workspace owns its tabs, late loads are ignored after closure or selection changes, and closing a task drops its cache. The UI caps open tabs/tasks and cached bytes. Copy, Attach, source and prepared HTML preview actions keep their existing context checks.

```ts
const owner = captureDraftContext();
const result = await readDroppedFile(file);
if (!stillOwnsDraft(owner)) return;
if (result.supported) addAttachment(owner, result);
else insertQuotedPathOrName(owner, file.path ?? file.name);
// This is drop-only text insertion. It never submits the composer.
```

The example describes the contract; `attachments.ts` implements it with the task, workspace and draft generation. Picker/paste failures retain their established behavior. A browser file without a native path is labeled as a filename with no known local path.

Native document reads derive the root from the selected project/task, reject mirrors and protected/outside-workspace paths, open regular files without following replacement symlinks on Unix, and bound actual bytes. The limits are **64 MiB** per document, **2,048** ZIP entries, **16 MiB** per decompressed entry and **64 MiB** total decompressed data. Archive checks read actual decompressed bytes, validate size/CRC, reject ambiguous paths, and validate embedded image dimensions independently of filenames. Images are capped at **8 MiB** and **32 million pixels**.

- PDF uses a pinned PDF.js canvas renderer with XFA disabled, bounded canvas/image dimensions, two concurrent render jobs and recycling/cancellation. It does not mount PDF annotation or scripting layers.
- DOCX is staged in an inert document and sanitized before adoption; fonts, HTML chunks, external images, active SVG/MathML and unsafe CSS are excluded. HTTP(S) links require an explicit click.
- Spreadsheets run in a disposable worker with a deadline and sheet/row/column/cell/text limits. Values render as text; formulas, macros and HTML are not executed.
- Images use bounded local data. SVG uses a static allowlist without external references or active content. Markdown images resolve only safe relative PNG/JPEG/WebP paths through the existing workspace boundary; remote images become links.

The application CSP and scriptless prepared-preview origin remain intact. Archive/image limits reduce resource risk; they do not make arbitrary documents trusted or certify all parser behavior. The dependency pins and reproducible lockfile are part of this candidate.

## Features intentionally excluded

| Behavior | Concrete reason and retained alternative |
| --- | --- |
| Plaintext Connect/app secrets or migration out of OS protected storage | Conflicts with `SECURITY-HARDENING.md`; unavailable or malformed protected records fail closed without overwrite. |
| Removing pinned device trust for same-account peers | Discovery and account membership do not authorize remote execution. Preserve current key pin, revocation, persistent replay and host gates. |
| App/agent update polling, automatic runtime/CLI downloads and price refresh | Disabled by project policy. Local installed-version/capability discovery remains available. |
| Cloud-agent runtime/login/checkout transport and unattended bridge | Outside the approved remote execution boundary. Manual cloud management is explicitly scoped above. |
| Broad Cline credential fallback | Could cross named-account/provider boundaries. Use the chosen installed CLI and selected connection only. |
| `plan_reads` silently granting outside-workspace reads/web searches | Existing permissions remain authoritative; broader actions require explicit approval. |
| Replacing Linux Browser QA with a wholesale external-browser bypass | Retain QA and origin checks; add an explicit compatible external-browser action. |
| Replacing Extended scoped Skills, review/coordination, issue actions, attachment ownership or replay with weaker upstream variants | These are maintained Extended features, not merge conflicts to discard. |
| Upstream installer warnings bypass, public `npx pandacode`, updater archives or publication automation | They do not install/certify this candidate. Use the local Extended packaging procedure; publication remains separate. |

The local SBOM helper is adapted for Extended installer names and existing offline scanners. It does not download tools/databases, publish artifacts, or treat an empty/failed inventory as clean. Dependency inventories are distinguished from installer inventories. Original upstream installation-note generation is not adopted because its updater and installation claims contradict this distribution policy.

## Validation and future maintenance

The [verification report](verification-historical-0.5.0.md) distinguishes baseline failures, candidate regression results, source-only inspection, mock-based cloud/provider behavior and untested live/platform behavior. CI includes the historical feature regressions in addition to the parent integration gates. Release validation hashes the historical manifest and its source notes, checks the range and immutable commits, and retains the original hardened-record digest.

Future integration work should add a similarly explicit release/source map and preserve intentional differences. A checked-in upstream note or a newer display suffix is not evidence that the corresponding behavior was implemented. The candidate stays 0.5.0; no tag, release, installer publication or merge is performed by this change.
