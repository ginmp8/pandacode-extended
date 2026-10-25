# Extended 0.5.0: PandaCode 0.3.42–0.3.47 integration

This document defines the release integration tracked in [issue #22](https://github.com/ginmp8/pandacode-extended/issues/22). The product version is **0.5.0** and the display identity is **0.5.0+pc.0.3.47**. It maps the upstream release changes to application flows, retained Extended behavior and validation requirements. It does not certify a native installer, live provider session or complete equivalence to every upstream version.

The subsequent [0.3.28–0.3.42 historical coverage supplement](upstream-parity-0.3.28-0.3.42.md), tracked in issue #24, starts from this integration's feature branch and retains the same candidate version. Its additional features, explicit exclusions and source identities are recorded separately; the frozen provenance below remains unchanged.

## Frozen inputs and scope

| Input | Identity |
| --- | --- |
| Extended starting point | `main`, commit `5d4cb69b0a97c46eef544303137c73a81f4c5b59` |
| Upstream delta baseline | PandaCode `v0.3.42`, commit `bca0b9f1f03a1cc3f8a8e6330881c34103f14a46` |
| Upstream target | PandaCode `v0.3.47`, commit `4187553cb48db96657609948f6b0904556845d45` |
| Supplied reference archive | `pandacode-0.3.47.zip` |
| Reference archive SHA-256 | `73a8dbe02d03c4aef37ff50362af266fa72c0f4b4dfb44d935ee649b1e3e85f1` |
| Reference comparison | 1,146 regular archive members match the upstream tag |
| Historical review | `HARDENED-REVISION.json`, SHA-256 `e64c3e5e51e4b161aaf4fbc7b2a540bda1487117f54b8724d3547f69b59e5ecf` with LF line endings |
| Historical source ancestry | PandaCode `0.3.28`, retained unchanged |

Release notes were checked against the original GitHub releases: [0.3.42](https://github.com/aicodeking/pandacode/releases/tag/v0.3.42), [0.3.43](https://github.com/aicodeking/pandacode/releases/tag/v0.3.43), [0.3.44](https://github.com/aicodeking/pandacode/releases/tag/v0.3.44), [0.3.45](https://github.com/aicodeking/pandacode/releases/tag/v0.3.45), [0.3.46](https://github.com/aicodeking/pandacode/releases/tag/v0.3.46), and [0.3.47](https://github.com/aicodeking/pandacode/releases/tag/v0.3.47). Versions `0.3.43`–`0.3.47` were all published on 2026-10-25. Their original source notes are retained without rewriting upstream claims as Extended test results.

The integration starts from Extended's code and incorporates the relevant upstream implementations and their prerequisites. Existing Extended modules and reviewed permission boundaries remain authoritative. Earlier selective ports do not establish global upstream parity: the active metadata keeps `full_upstream_parity` and `upstream_code_rebased` false.

## Feature and acceptance map

| Upstream | User-visible change | Main implementation surfaces | Required acceptance |
| --- | --- | --- | --- |
| 0.3.42 | Long worker names fit narrow conversations | `src/styles.css`, `src/team-progress.css` | Worker labels shrink without widening the transcript; code blocks retain their own horizontal scrolling. |
| 0.3.43 | Devin CLI in the harness/model picker | `src-tauri/src/providers/devin.rs`, `providers.rs`, `providers/acp.rs`, `src/model-catalog.ts`, provider maps | Detect an installed CLI or the supported macOS bundled CLI, report missing sign-in, discover the catalog without a prompt, and keep unavailable choices explicit. |
| 0.3.43 | Per-model reasoning levels and image capability | `providers/devin.rs`, `providers/acp_catalog.rs`, `providers/attachments.rs`, model picker | Each regular model offers its own levels; Fusion has no invented effort slider; text-only models reject image prompts before sending. |
| 0.3.43 | Devin permission modes and plan approval | `providers/devin.rs`, `providers/acp.rs`, `src/approval-modes.ts` | Standard maps to Code; Plan maps to Plan; only explicit Full access selects Bypass. Approval allows one action; implementing a plan cannot silently choose bypass. |
| 0.3.43 | Fast follow-ups, steering, console, slash commands | `providers/steering.rs`, `terminal.rs`, `slash_commands/acp.rs` | Reuse a matching session; route supported steering to the running turn; console resumes the session; hide account/mode commands from the chat composer; delete empty discovery sessions. |
| 0.3.44 | Remote follow-ups reach the host computer | `src/main.ts`, `src-tauri/src/lib.rs`, `remote/mirror.rs` | A mirror always sends to its remote host, even with Team enabled by default; hide the local Team toggle on mirrors; late events cannot resurrect a completed run. |
| 0.3.44 | Start progress and recoverable failure | `src/remote-progress.ts`, `remote/start.rs`, `remote/mirror.rs` | Show finding/downloading/preparing/starting stages and download percentage; retain the prompt on failure with Retry and Dismiss. |
| 0.3.44 | Images and screenshots in remote prompts | `src-tauri/src/remote/images.rs`, `remote/host.rs`, `remote/mirror.rs`, composer | Send at most four PNG/JPEG/WebP images, each at most 5 MiB, in bounded encrypted chunks; claim only uploads from the authenticated sender; use host-generated capture names. |
| 0.3.45 | Active, Needs review and Completed sidebar sections | `src/sidebar-sections.ts`, `src/sidebar-tree.css`, `src/main.ts` | Put each task in exactly one section. Running/waiting work stays Active; review visibility respects the 24-hour window and unseen state; opening a row does not move it out from under the click; Completed starts collapsed and persists its state. |
| 0.3.45 | Sidebar completion/archive/delete actions | `src/main.ts`, `src/task-board.ts` | Operate on the row's task rather than the currently selected task; offer restore/delete in the archive; retain existing confirmation and running-task protections. |
| 0.3.45 | Persistent inspector tab strip | `src/inspector-tabs.ts`, `src/inspector-bar.css`, `src/main.ts` | Start with Changes; add/close supported tabs; preserve order/selection; sanitize stale stored state; programmatic opens reveal their tab; closing a tab does not destroy task ownership. |
| 0.3.45 | Inline file diffs and staging controls | `src/changes-view.ts`, `src/changes-view.css` | Render current diffs together, show scope and totals, support per-file actions and Stage all/Commit/worktree actions, refresh changed files during runs, and reject stale task results. |
| 0.3.46 | First remote prompt appears immediately | `src/main.ts`, `src/preview-remote.ts`, `src/remote-progress.ts` | Render the user's prompt and start stages before the remote response; restore the draft after failure; avoid duplicate optimistic messages. |
| 0.3.46 | Remote stream continuity | `src/main.ts`, `remote/mirror.rs` | Keep prompt and streamed reply visible while the final message page catches up; task switching cannot mix transcripts. |
| 0.3.47 | Teams remain Running until their workers finish | `remote/host.rs`, `remote/socket.rs`, `architect.rs`, `mobile/src/store/team.ts` | Reflect architect and worker state; show worker wait counts; Stop pauses the whole team rather than only the architect. |
| 0.3.47 | Team details and live worker transcripts on mobile | `remote/host.rs`, `remote/socket.rs`, `mobile/src/connect/types.ts`, team/agent screens | Expose worker status/checkpoint/summary/messages and team messages only through authenticated Connect requests; update the selected task without cross-task state leakage. |
| 0.3.47 | Linked pull requests, changes and bots on mobile | `remote/host.rs`, `mobile/src/app/pull.tsx`, `changes.tsx`, `diff.tsx`, inbox | Return bounded existing task data and available bot summaries; show unsupported capabilities explicitly on older hosts. |
| 0.3.47 | Durable message images, project icons and activity | `src-tauri/src/attachments.rs`, `providers/attachments.rs`, `remote/host.rs`, mobile attachment/inbox stores | Keep image references with their message, return project icons, report sub-agent summaries and last activity; missing or expired data remains explicit. |
| 0.3.47 | Newest-first paging and faster reads | `remote/host.rs`, `remote/socket.rs`, mobile paging/cache code | Open the newest thread page; handle up to four independent reads while preserving write order; offload blocking work; send ready responses promptly; retain bounded PR caching. |
| 0.3.47 | Read batches | `remote/host.rs`, mobile RPC layer | Accept one to eight permitted read requests, reject nested batches and write methods, preserve result order and per-call errors, and enforce the combined response-size budget. |

## Code examples and integration boundaries

These examples illustrate the integration contracts. The production modules named above remain the source of truth.

### One authoritative sidebar partition

The section helper receives the existing task state and activity predicates. It does not replace the native scheduler or mutate task stages:

```ts
const sections = sidebarSections({
  tasks: visibleTasks,
  now: Date.now(),
  active: task => isRunning(task.id) || waitingTasks.has(task.id),
  stage: taskStage,
  unseen: task => unreviewed.has(task.id),
  selectedId: currentTaskId,
  held: openedFromReview,
});
```

Rendering must keep each task's stable row identity. Row action buttons carry their own task identifier and do not rely on selection, preventing a click on another row from modifying the open conversation.

### Keep provider modes explicit

The Devin adapter uses the provider's advertised modes:

```rust
match approval_mode {
    "plan" => "plan",
    "full" => "bypass",
    _ => "accept-edits",
}
```

This mapping does not grant remote Full access. Extended's independent host-side remote permission check runs before admitting a remote operation. A normal approval response must select a one-time option and must never substitute a persistent allow or bypass option.

### Batch reads inside the existing authenticated protocol

The client can group related reads; encryption, replay metadata and device authorization remain responsibilities of the Connect transport:

```json
{
  "method": "batch",
  "params": {
    "calls": [
      { "method": "task.messages", "params": { "sessionId": "task-id", "tail": true } },
      { "method": "subscribe", "params": { "sessionIds": ["task-id"] } },
      { "method": "task.pull", "params": { "sessionId": "task-id" } }
    ]
  }
}
```

The outer request retains authenticated peer identity. A batch containing `task.send`, `approval.respond`, `task.pullDetect` (which can save a link), another write method, or another `batch` is rejected before dispatch. A failure in one allowed read is returned for that entry without reordering the others. Concurrent reads do not authorize concurrent writes.

### Preserve Extended's database and scheduling contracts

Extended already has schema **27** for durable `github_issue_actions`. This integration adds message attachment metadata in migration **28**, within the existing migration transaction. The migration preserves issue intents, task rows and messages, and remains idempotent when an existing database is reopened. Stored image reads correlate the attachment with both its task and its owning message before opening a bounded regular file.

The Connect scheduler has three bounded paths: independent reads, repository starts, and ordered controls for existing tasks. Keeping long repository clones outside the existing-task control queue preserves responsive Stop and approval replies. Each path revalidates the exact authenticated public key and the relevant host permission before dispatch. An independent remote Full access grant is still required for that mode.

Remote progress uses a new opaque ID for every start attempt. A delayed event with an explicit unknown ID is ignored; an earlier request settling cannot clear its retry. Mirror prompts are remembered even when context preparation finishes after the user switches tasks. Removed inline-diff entries invalidate outstanding loads before disposing their viewers.

The mobile package includes the upstream screens and their prerequisite stores. Its token, trust and replay writes are serialized with account cleanup. Cache generations invalidate delayed reads and attachment handles, and native file writes/moves finish before cleanup reports success. These boundaries retain Extended's explicit trust model while supporting the new mobile flows.

## Extended behavior that must survive

| Area | Retained contract | Main regression coverage |
| --- | --- | --- |
| Review & coordination | Manual Advisor/Committee profiles, bounded budgets, exact installed model roster, read-only inspector/attention and draft-only prompt insertion | `tests/hardened-review*.test.mjs`, `tests/hardened-entrypoint.test.mjs` |
| Handoff | Explicit preview/copy, bounded same-task context, opt-in checkpoints and credential redaction | `tests/hardened-handoff-redaction.test.mjs`, review tests |
| Browser QA | Foreground ownership, fresh references, consent for mutations, cancellation/deadline, origin checks and explicit evidence | `tests/hardened-browser-*.test.mjs` |
| Workflow templates | Existing composer/recipe flow, no implicit send, no conflicting overwrite and state revalidation | `tests/hardened-workflow-library.test.mjs` |
| Connection and Skills isolation | No accidental default-account reuse; commit/hash-pinned Skills; explicit import/activation/scope changes | Connection/Skills native tests and `tests/hardened-041-connections.spec.ts` |
| Historical attachment handling | Validate source message, path and image reference; do not interpret fenced examples as attachment commands | `tests/message-attachments.test.mjs` |
| Chat/sidebar stability | Repeated chat selection is idempotent; row ordering and task identity remain stable | `tests/chats.spec.ts`, `tests/thread-order.spec.ts` |
| Extended GitHub/Board features | Issue drafts, task stage indicators, drag/drop stages and explicit linked-issue close/reopen controls | Existing GitHub, Board and linked issue tests |
| Release/installation policy | Stable app identifier, Extended bundle branding, locked builds, disabled updater/network installer and no automatic publication | `tests/install-policy.test.mjs`, release tests, branding/history tests |

The upstream integration does not remove per-device trust, the independent remote Full access grant, persistent replay receipts, OS credential storage, scriptless local file previews, explicit bot notification approval or explicit VM/CLI installation. These are retained product policies. New mobile and remote flows must use compatible Extended peers and honor the same host checks; a shared GitHub account alone is insufficient authorization.

## Failure modes and proving checks

| Failure mode | Required check |
| --- | --- |
| New Devin identifier omitted from one allowlist | Typecheck all provider maps and run native provider, storage, terminal, Skills, slash-command and team tests. |
| Devin discovery leaves unwanted sessions or performs a tool action | Simulate ACP metadata responses, rejected requests, timeout and cleanup; no prompt/tool execution during discovery. |
| Text-only model receives images or approval chooses bypass | Exercise attachment rejection and one-time/plan approval selection, including malformed/unsupported options. |
| Sidebar actions target the open task instead of the clicked row | Browser test on two different tasks; completion, archive and delete affect only the requested task. |
| New section markup breaks repeated chat selection or order | Re-run chat/sidebar regressions and section membership tests together. |
| Inspector state references removed/duplicate views | Normalize persisted state; close the selected and last tabs; open each view programmatically. |
| An old diff response renders after a task switch | Delay requests, switch tasks and verify stale results cannot overwrite the new task's Changes view. |
| Remote Team configuration steals mirror follow-ups | Start a mirror while automatic Team is enabled, send follow-ups and verify host routing. |
| Start failure loses draft or completion causes a transcript flash | Exercise start errors, retry, slow final page synchronization and late events. |
| Image chunks are forged, oversized, cross-device or incomplete | Native image tests for count/type/size/order/ownership/expiry; native checks precede prompt admission. |
| Read concurrency bypasses trust or reorders writes | Revoke trust during pending reads; verify authenticated peer propagation, replay rejection and ordered send/stop/approval dispatch. |
| Batch introduces a write or exceeds a resource budget | Reject writes/nested batches/overlength arrays; test per-entry errors and oversized combined results. |
| Architect finishes while workers continue | Team summary and mobile state tests keep Running and ensure Stop pauses the team. |
| Mobile changes silently assume unavailable host methods | Capability/error fallback tests against an older host contract. |
| Version bump rewrites historical security claims | The release gate pins historical evidence, validates the integration record and rejects metadata/history corruption in temporary fixtures. |

## Validation and publication status

The [executed verification report](verification-0.5.0.md) records build, unit, release, native, browser, mobile and relay results, including focused reruns and unexecuted environment-specific checks. The following commands reproduce the principal source gates.

```sh
npm ci --ignore-scripts
npm run build
npm test
npm run test:release
npm ci --prefix website --ignore-scripts
npm run site:build
npx playwright test tests/chats.spec.ts tests/thread-order.spec.ts tests/sidebar-sections.spec.ts tests/inspector-tabs.spec.ts tests/changes-view.spec.ts tests/remote-progress.spec.ts tests/remote-images.spec.ts tests/remote-first-message.spec.ts
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Run the mobile package's typecheck/unit commands separately when its dependencies are available. Native installers require their actual target OS. Authenticated Devin sessions, real computer-to-computer transfers, mobile devices, Keychain/Credential Manager, and installation/update behavior require explicit environment-specific checks; browser fixtures do not prove those paths. Upstream test claims in mirrored notes are not Extended evidence.

This change prepares a branch and PR. It does not merge the PR, create `v0.5.0`, publish installers, create an updater feed or publish an npm package. Existing `v0.4.0` artifacts and historical notes remain unchanged.
