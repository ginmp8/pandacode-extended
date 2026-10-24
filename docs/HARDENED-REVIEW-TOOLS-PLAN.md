# Review & Coordination Plan

This plan describes what must be built for Review & Coordination to move beyond prompt preparation and become a bounded, locally executed, verifiable review. It is not a certification of the current implementation. The feature has not been implemented; the decisions below are requirements for its implementation.

The current behavior is documented in [HARDENED-REVIEW-TOOLS.md](./HARDENED-REVIEW-TOOLS.md). This remains the plan; do not create a separate plan under another name.

## Actual repository state

The following already exists and must not be presented as the requested system:

- The **Review & coordination** panel is a frontend extension in [src/hardened/review-panel.ts](../src/hardened/review-panel.ts), [src/hardened/review-model.ts](../src/hardened/review-model.ts), and [src/hardened/review-actions.ts](../src/hardened/review-actions.ts). `mountReviewTools` loads the module on demand. Its tabs are Review presets, Team inspector, Attention, and Handoff. Opening or refreshing the panel does not start an agent, approve a request, or apply a team.
- `buildReview` and `reviewProblems` prepare only **Codex** tasks in **plan** mode. The saved profile hardcodes `architect.provider` and every worker to `"codex"`. The prompt asks for `readOnly: true`. This is a preparation guard, not isolation.
- `ReviewToolsHost` can insert text into the composer (`insertPrompt`) and save a profile (`architect_save_profile`). There is no command to start or cancel a review, query its execution, obtain a report, propose a fix, or apply a patch.
- The only team-tools MCP server is `pandacode_architect`, in [src-tauri/src/providers/mcp_host.rs](../src-tauri/src/providers/mcp_host.rs). It uses HTTP on `127.0.0.1`, a per-process token, and up to 16 loopback clients; it stops with the process (`Drop` aborts the task). `rebind` replaces the entire context at the same address and with the same token. It is not bound to a review execution and does not reject calls from another task beyond that process's `session_id`.
- Team tools are listed in `TEAM_TOOLS` and governed by the rules in [src-tauri/src/architect.rs](../src-tauri/src/architect.rs). The worker's `read_only` flag only appears in the prompt. The code comment explicitly says workers share the checkout. A prompt or assignment flag does not contain a process.
- The 15 registered harnesses are in `PROVIDERS` ([src-tauri/src/providers.rs](../src-tauri/src/providers.rs)) and repeated in `HARNESSES` / `ModelSelection::validate` ([src-tauri/src/architect/profiles.rs](../src-tauri/src/architect/profiles.rs)): `claude`, `codex`, `kimi`, `opencode`, `glm`, `antigravity`, `grok`, `mimo`, `pi`, `deepseek`, `commandcode`, `cline`, `omp`, `github-copilot`, and `muse`. Existing team profiles already accept any of them. The review preset is what restricts them to Codex.
- Warm sessions exist only for Claude: `warm::supported` in [src-tauri/src/providers/warm.rs](../src-tauri/src/providers/warm.rs) requires `provider == "claude"`, with no goal or skill. The key includes connection, model, effort, mode, folder, resume ID, instructions, and tool catalog. A spare is adopted only when its key matches. `rebind` rebinds the architect MCP to the next turn.
- The schema is at `storage::SCHEMA_VERSION` = 26 ([src-tauri/src/storage.rs](../src-tauri/src/storage.rs)). There is no review execution column or table. Profiles live in the `architect_library` JSON with a 256 KiB limit. The design below requires no migration.
- Isolated worktrees already exist: `create_worktree` / `create_worktree_with` in [src-tauri/src/workspace.rs](../src-tauri/src/workspace.rs). The parent directory cannot be a symlink. `merge_worktree` requires a clean tree and performs a merge; review must not call it.
- Project listing already rejects symlinks that escape the root (`symlinks_cannot_escape_project`). This does not create a frozen snapshot or protect a harness process.
- Current process containment: on Windows, [src-tauri/src/providers/windows_job.rs](../src-tauri/src/providers/windows_job.rs) only enables `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. On Unix, the process is placed in a process group for `SIGTERM`/`SIGKILL`. Neither is a filesystem, network, or credential jail.
- Plan/read-only policies are requests to the harness, not proof of OS enforcement: Codex sends `sandboxPolicy.type = readOnly` and `networkAccess: false`; Muse uses `--disable-write` and `--disable-shell`; OpenCode sets permissions in `OPENCODE_CONFIG_CONTENT`; Grok has no read-only ACP session mode and declines state-changing requests (`GROK_PLAN_PREFIX` in [src-tauri/src/providers/acp.rs](../src-tauri/src/providers/acp.rs)); DeepSeek sets `DSH_PERMISSION_MODE`; Claude/GLM/MiMo use `--permission-mode`. None of these paths has been proven to prevent subprocess, symlink, or network escapes outside the provider.
- `child_env::sanitize` removes AppImage paths and disables harness auto-updates/telemetry. It does not limit cloud, bot, or connection credentials, or network access.
- There is no remote sandbox, updater, or review telemetry, and these remain prohibited. The analyzed content will be sent to the provider selected for inference through that provider's CLI, using the user's existing login. This is inference, not application telemetry.

## Existing plan decisions to retain

- Do not import a runtime, dependency, daemon, or code from Paseo.
- Do not phone home, add telemetry, or create a remote sandbox.
- Save profiles through the existing API. Reuse an identical profile. Never overwrite a different profile with the same name (`saveReviewProfile`).
- Keep file scopes as relative paths, at most 32, with no `..`, absolute path, `~`, or URL (`scopes` in `review-model.ts`).
- The default preset budgets may remain: Advisor has 1 reviewer, 1 parallel worker, 1 assignment, and 2 coordination turns; Committee has 2 reviewers, 2 parallel workers, 2 assignments, and 2 turns. The native scheduler remains authoritative. The prompt must not request rounds until consensus.
- Inspector, Attention, and Handoff do not approve, dispatch, or mutate a team. Attention remains a local snapshot, renders text as text, and shows at most 200 items. Handoff remains manual, with 4,096-byte field limits and a 16,384-byte total limit, and best-effort redaction. None becomes a patch-application channel.
- Closing the dialog discards unsaved drafts. Do not add a polling timer for the existing panel.
- Saved profiles are user data. Uninstalling the extension does not delete profiles.

This plan corrects any implication that a Codex preset, `readOnly` flag, plan mode, or prompt text constitutes isolation; that inserting a prompt starts a contained review; that harness support can be announced without proof; or that review should share the `pandacode_architect` MCP.

## Decisions required for implementation

1. Name the new server `pandacode_review`. Do not reuse the name, token, or tool catalog of `pandacode_architect`. Do not register review tools in `TEAM_TOOLS`.
2. The panel and chat call the same native functions. Chat does not get a second protocol. A model sees the MCP only in a process belonging to the execution and only with the tools for its role.
3. A model cannot start a harness, choose a substitute provider, enable auto-fix, or apply a patch without the user's explicit authorization. Review start and patch application require user confirmation.
4. Execution state, tokens, snapshots, proposals, and approvals live in process memory and in execution-scoped temporary files. An application restart invalidates everything in flight. No `SCHEMA_VERSION` migration is needed.
5. If the required containment has not been proven on a system, `start` fails closed and does not launch the harness. Do not fall back to “plan mode” or another provider.
6. Do not commit, push, merge, tag, or update dependencies. Do not call `merge_worktree`.
7. A review must not adopt a warm Claude session, and a review must not be prewarmed for an ordinary turn.

## 1. Dedicated MCP

### Transport

Add a native module following the general pattern of `mcp_host::Server`: bind only to `127.0.0.1`, use an ephemeral port and a 256-bit per-execution bearer token, limit to 16 loopback connections, bound request and result sizes, and ignore non-loopback clients. Publish the server to harnesses as `pandacode_review`, never `pandacode_architect`.

Use the attachment format each harness already accepts, changing only the server name and URL:

- Claude, GLM, and MiMo: an object in the `claude_config` format, passed through `--mcp-config` with `--strict-mcp-config`. Do not inherit user MCP configuration.
- Codex: an object in the `codex_config` format, only in `thread/start` for a new review execution thread. Do not use the `per_run_mcp_tool` shortcut, which currently covers only `pandacode_jev_*`, title, `pandacode_run_project`, and `pandacode_preview_file`.
- ACP (`kimi`, `opencode`, `antigravity`, `grok`, `deepseek`, `commandcode`, `cline`, `github-copilot`): an entry in the `kimi_config` format in `session/new.mcpServers`, and only if initialize advertises `mcpCapabilities.http == true`.
- Pi and Oh My Pi: URL and authorization in the `pi_config` format through the `--extension` / `PANDACODE_PI_BRIDGE` bridge. Pi model discovery uses `--no-mcp` and `--offline`; that is not the execution transport. The current bridge does not list review tools. Keep Pi and omp at `capability_missing` until a fixture shows `pandacode_review_*` being exposed through that bridge.
- Muse: `mcpServers` on MSP start/resume, only if the `sessionMcp` capability requested during `initialize` is accepted. The existing test reports the `pandacode_architect` server; that does not prove the new server.

Do not offer stdio MCP. No registered harness currently receives the review server over stdio. Do not invent a stdio transport just to complete the matrix.

### Operations

Every MCP call is authenticated by the execution token. The request body does not select a task or execution: the server is already bound to `(task_id, execution_id, role)`. Any argument attempting to name another task or execution is rejected as `execution_mismatch`.

| Operation | Caller | Input | Output | Rejection cases |
|---|---|---|---|---|
| `pandacode_review_capabilities` | Coordinator; panel through native IPC | None | Local proof: providers with a passing probe, models from the local catalog that passed the probe, transport, containment proof ID, snapshot policy, and whether the user enabled auto-fix for this execution | Invalid token, wrong role, closed execution |
| `pandacode_review_start` | Chat coordinator, only with current one-time start authorization granted by the user in chat or panel | User-approved request: preset, roles, models, scopes, criteria, apply target, and auto-fix setting | `execution_id`, state `snapshotting`, or a fail-closed error | Missing/expired authorization; request differs from the approved hash; unproven provider; unproven containment |
| `pandacode_review_state` | Coordinator; reviewer only for its own assignment; panel | None, or `assignment_id` for a reviewer | State, revision, hashes, and errors; no secrets or tokens | Foreign execution; role does not own the assignment |
| `pandacode_review_report` | Coordinator and panel | None | Reports proposed so far, each with its hash | Reviewer attempts to read another assignment's report; invalid execution |
| `pandacode_review_propose` | Reviewer assigned to the work | `assignment_id`, report text, unified patch, and hash of the manifest it read | Stored proposal; nothing applied | Non-diff patch; absolute path; closed execution; coordinator attempts to propose as a reviewer |
| `pandacode_review_cancel` | Coordinator and user through panel/chat | Short reason | State `cancelled` | Reviewer attempts to cancel the whole execution; call after token invalidation |

The start tool is available to the chat coordinator only after the user has confirmed the exact request. The native host binds that authorization to a canonical request hash and consumes it once. The model cannot mint, broaden, or replay it.

There is no MCP operation to apply or approve, change mode, save a profile, dispatch architect workers, open a browser, use a VM, deploy, or run git. Patch application is native IPC, described in section 6.

Use stable, short errors that contain no absolute credential path or token: `unauthenticated`, `execution_mismatch`, `execution_closed`, `role_forbidden`, `capability_missing`, `containment_unproven`, `snapshot_unstable`, `snapshot_escape`, `approval_required`, `proposal_mismatch`, `base_changed`, `apply_incomplete`, and `cancelled`.

### Lifecycle

1. The user confirms the request in the panel or through an explicit chat action. The application records the canonical request hash and a one-time authorization for that exact request.
2. The chat coordinator calls `start`. The native host consumes the authorization, creates an `execution_id`, starts `pandacode_review`, and then creates the snapshot.
3. If snapshot creation or containment fails, destroy the token and do not launch a harness; set the state to `failed_closed`.
4. Start the coordinator with its coordinator token. Start each reviewer with a separate token, limited to `state` for its assignment and `propose`.
5. On completion (the final report is accepted and no proposal is awaiting a decision) or cancellation, abort the listener, invalidate all tokens, and kill the process tree. Subsequent calls return `execution_closed`.
6. Warm-session `rebind` does not apply. A later turn, even in the same task, does not inherit the token.
7. A second execution for the same task is allowed only after the first is `cancelled`, `failed_closed`, or `applied`, and only on a new user-authorized start. No hidden queue.

The panel and chat show the same state. Chat events contain text, never HTML, following the existing Attention rendering approach.

Visible states: `idle`, `awaiting_confirmation`, `probing`, `snapshotting`, `running`, `awaiting_report`, `proposing`, `awaiting_approval`, `applying`, `applied`, `cancelled`, `failed_closed`, `containment_unproven`, `capability_missing`, and `apply_incomplete`.

## 2. Harnesses and models

Advertise support only when the probe proves it on this machine. An installed model catalog is not enough. Never silently switch providers, including for `default` or Codex.

For each selected provider, the probe must observe `pandacode_review` in the actual start/session frame and receive a successful `pandacode_review_capabilities` call. A fixture tests the recorded frame. An opt-in, ignored live CLI smoke test provides runtime evidence, following existing ignored smokes. Until fixtures for the new server exist, the status of every provider in this review matrix is **unproven**.

| Provider | Transport currently accepted by the code | Start | Resume | Warm session | Requirement before announcing support |
|---|---|---|---|---|---|
| `claude` | HTTP via `--mcp-config` (`claude_config`) | New process, stream-json, `--permission-mode` reapplied | `--resume` only with the review execution's own ID, never the ordinary task's `resume_id` | Exists (`warm::supported`), but review does not adopt a spare or call `prewarm` | Fixture with `pandacode_review` and a capabilities probe |
| `glm`, `mimo` | Same as Claude, with arguments from `glm::arguments` / `mimo::arguments` | Same as Claude; provider-specific credentials from secure storage, not Claude's | Same as Claude, no warm session | None | Run the probe separately; do not inherit Claude's proof |
| `codex` | HTTP in thread config (`codex_config`); dynamic tools are fixed when the thread is created | New thread for the execution; do not reuse the task's thread | Do not resume a foreign thread; `sandboxPolicy` would have to be reapplied each turn and remains CLI policy, not a jail | None | Register the server in `thread/start`, not through `per_run_mcp_tool` |
| `kimi` | ACP HTTP (`kimi_config`) if `mcpCapabilities.http` is advertised | `session/new`; code already errors if HTTP is false and a tool is required | `loadSession` only for the execution's own ID, if advertised by the agent | None | HTTP capability and successful probe; otherwise `capability_missing` |
| `opencode` | ACP HTTP; permissions through `OPENCODE_CONFIG_CONTENT` | Standard ACP start; plan denies everything except read/glob/grep/lsp | Standard ACP resume | None | HTTP probe; permission policy is not proof of isolation |
| `antigravity` | ACP HTTP | Standard ACP start; code already reports “Waiting for Antigravity startup” | Standard ACP resume | None | Probe; startup timeout must not trigger provider substitution |
| `grok` | ACP HTTP, with an `authenticate` step if a cached method is available | Standard ACP start; plan is a refusal policy plus a prefix, not a session mode | Standard ACP resume | None | Probe; permission refusal is not a jail |
| `deepseek` | ACP HTTP plus `DSH_PERMISSION_MODE` | Starts with `--profile acp` | Standard ACP resume | None | Probe; sandbox environment variable is not a substitute for the probe |
| `commandcode` | ACP; session mode is its permission mode | If the exact mode is absent, code refuses instead of widening access | Standard ACP resume | None | Probe and advertised mode |
| `cline` | ACP; Plan or Act | Plan maps to `plan`; other modes map to `act` | Standard ACP resume | None | Probe |
| `github-copilot` | ACP HTTP; model only if advertised by the CLI | `copilot --no-auto-update --acp --stdio` process, MCP HTTP in the session | Standard ACP resume | None | Probe and model present in the advertised catalog; arbitrary IDs remain rejected |
| `pi` | HTTP through the extension bridge, not the native MCP client | `--mode rpc` and `--extension` | `--session-id` only for this execution | None | Bridge fixture lists the new tools; until then, unsupported |
| `omp` | Same bridge; `--resume` because it has no `--session-id` | Currently `--approval-mode yolo`, with the bridge as the approval gate | `--resume` for this execution, without an interactive prompt | None | Same bridge fixture; do not reuse Pi's proof if the binary differs |
| `muse` | MSP HTTP if `sessionMcp` is accepted | `muse serve` plus mode sandbox flags | Current MSP resume reattaches session MCP for the old server name | None | `sessionMcp` capability and probe for the new server name |

Models come from the local catalog already used by the picker, refreshed on demand without polling. `default` is allowed only when the user chooses it and that provider passes its probe. A missing, uninstalled, or cross-provider model is an error, not a substitution.

A review can resume only the same `execution_id` while it remains `running` or `awaiting_report` and its original token is still valid. Resuming an ordinary task, an old Codex thread, or a warm spare does not resume the review. Cancellation invalidates the token; there is no resume after cancellation.

## 3. Execution binding and roles

Accept an MCP call only when its token, task, and execution match the `Context` owned by that listener. A token from another execution, even on the same task, is `execution_mismatch`. A token after completion or cancellation is `execution_closed`. An ID supplied by the model is not sufficient.

Separate roles and processes:

- **Coordinator:** one process using the provider/model selected for coordination. Tools: `capabilities`, `start` (only with the user's one-time authorization), `state`, `report`, and `cancel`. It does not receive `TEAM_TOOLS`, browser, VM, deploy, app terminal, or `propose`.
- **Reviewer:** one process per assignment, using the provider and model selected in the profile, not necessarily Codex. MCP tools: `state` for its own assignment and `propose`. It cannot cancel the execution, read another reviewer's proposal, or access the coordinator token.
- **Approver:** the user through panel/chat native IPC. This is not an MCP role. Auto-fix does not make the model an approver; it only skips confirmation of the diff, and only if the user enabled it before `start` for that execution.

Keep `staff_from_roster` rejecting workers outside the roster when `rosterOnly` is true. Use that rule to prevent a coordinator from replacing a reviewer with another provider. The review profile roster must be able to list any harness accepted by `ModelSelection::validate`.

The review server must not start on ordinary turns, enter a warm session's cached tool catalog, or be rebound by `rebind`.

## 4. Safe project reads

Reviewers do not receive the live checkout. They receive a local snapshot created at start, outside the checkout, in a `0700` directory without `.git`.

**Content decision:** snapshot the working tree as it exists at start, not only `HEAD`, so the review covers what the user is currently seeing. The manifest records separately:

- `HEAD` commit;
- index hash;
- staged and unstaged diffs, each with a hash;
- included untracked files;
- excluded ignored files;
- requested paths that did not exist;
- symlinks omitted because they escape or could not be classified.

Include staged and unstaged modifications for in-scope paths. Include untracked files that are not ignored or excluded. Exclude ignored paths (`git check-ignore`), `.git`, known application credential files, and paths outside any user-specified scopes. Report that ignored files were not reviewed. Handoff redaction is not the snapshot filter.

Do not follow symlinks or junctions whose canonical target is outside the copied root. Do not copy their targets. Record the omitted path. If the path was requested in scope and cannot be classified, fail with `snapshot_escape` instead of following it. The path-listing protection in `workspace.rs` is a reference, not a complete implementation: it does not freeze files and has no Windows junction test yet.

Hash source paths again during copying to detect concurrent changes. If a hash differs, retry once. If it differs again, return `snapshot_unstable`; do not launch a harness or review an inconsistent tree. Remove write bits from the snapshot after copying. This is not the jail: the owner can restore permissions. The section 5 jail is still mandatory.

The snapshot is not a prompt instruction or a `readOnly` flag. Those can remain in model-facing text, but they are insufficient.

The manifest (hashes, not content) is the proposal's base. The reviewer cannot choose a different base.

## 5. Isolation and tools

Containment must cover the harness process and its descendants. The current implementation is insufficient:

- Windows: the job object only kills the process tree when closed. It does not deny writes, network, or breakaway. The code has no AppContainer or restricted token.
- Unix: the process group only delivers signals. Harness launches do not use Landlock, bubblewrap, or `sandbox-exec`.

**Decision:** do not launch a review in this state. `start` returns `containment_unproven` until there is a mechanism on that OS whose tests in section 8 deny attempts to:

- write outside the snapshot, including through symlinks or junctions;
- open a socket other than the review MCP loopback connection or the provider-selected CLI's existing inference traffic;
- read another provider's environment variables, bot secrets, cloud keys, or another execution's token;
- invoke MCP tools other than those permitted for the current role.

If the mechanism cannot be implemented and proven on an OS, review remains disabled on that OS. Do not use plan mode as a substitute. Do not move review into a VM, Daytona, Cloudflare, AWS, or another remote sandbox. Do not add telemetry to “measure” escapes.

Network: the selected provider's CLI sends snapshot content to that provider for inference, using the user's existing login. Before confirmation, the panel identifies the provider and model and explains this transfer. Do not claim a host allowlist unless it is proven. If provider inference cannot be separated from arbitrary subprocess network access, fail closed rather than allow unrestricted networking. Loopback access is restricted to the current execution token.

Processes: place the process tree in the job (Windows) or process group (Unix) before work begins; cancellation kills the tree. This is partly implemented and remains necessary, but does not replace filesystem and network containment. Reuse provider limits such as `MAX_FRAME` and bounded stderr output. On limit exhaustion, kill the execution and set `failed_closed`.

Credentials: provide the review process only the credentials that provider already needs to contact its own service (including GLM/MiMo secure-store credentials when applicable). Do not provide other providers' connection secrets, bot secrets, cloud keys, or tokens for other roles. Pass the MCP token only in that process's MCP config and redact it from events, as `Server::redact` does for the existing server.

Reviewer MCP tools: no application write tools, no `pandacode_architect`, no browser, VM, or deploy tools. A reviewer's only write-like operation is submitting a patch through `propose`, not editing the checkout.

Analyzed content leaves the machine because the user selected that provider. The application sends the snapshot nowhere else.

## 6. Approval-gated fixes

Reviewers produce a report and a unified patch against the snapshot. They do not edit the checkout, receive the checkout as their working directory, or get a checkout write tool.

Store the exact proposal bytes, patch hash, manifest hash, `assignment_id`, and `execution_id`. Any byte change is a different proposal.

Default behavior: the user approves the exact diff, not a summary. Bind the approval to both hashes and the apply target. Auto-fix is allowed only if the user enabled it before `start`. MCP cannot enable this flag. Auto-fix skips confirmation of the diff content; it does not skip revalidation or target selection.

**Target decision:** when confirming the execution, the user selects either the current checkout or an isolated worktree. Changing the target after a proposal requires new confirmation. Auto-fix uses the already selected target; if that target cannot be revalidated, do not silently switch to the other.

Before applying:

- verify the same execution is still active and not cancelled;
- verify that the patch and manifest hashes still match the approval;
- verify every touched target path still has the snapshot base bytes;
- run `git apply --check` against a temporary copy of that base.

Current checkout: if any touched file has diverged, return `base_changed` and write nothing. Do not partially apply only the files that still match.

Worktree: call `create_worktree` from `HEAD`, without committing. Then copy the snapshot base for the patch's touched paths into the new worktree and apply the patch there. This is not a commit. If the snapshot cannot be reproduced on that `HEAD` (for example, an untracked-file collision or a patch that does not apply), return `base_changed`. Remove the worktree created for the attempt with `remove_worktree` only if it contains no other user work. If unsure, do not remove it; mark `apply_incomplete`. Never call `merge_worktree`.

Partial failure: apply to a temporary copy first. Only then replace target files, keeping a journal of their previous bytes. If replacement fails partway through, restore from the journal. If restoration fails, stop, set `apply_incomplete`, do not retry automatically, and report that restoration did not complete. Do not create a commit to “save” the partial state.

Never commit, push, merge, tag, stash automatically, or run `git add`.

## 7. Interfaces, profiles, and persistence

Add matching types in the frontend and native layer. Canonical names:

- `ReviewExecution`: `id`, `taskId`, `preset`, `status`, `createdAt`, `baseManifestHash`, `containmentProofId`, `autoFix`, `applyTarget` (`checkout` or `worktree`), `coordinator`, and `reviewers`.
- `ReviewRoleBinding`: `role` (`coordinator`, `reviewer`, `approver`), `provider`, `model`, `reasoningEffort`, `assignmentId`, and `tokenGeneration`. The approver is a UI identity only and never receives an MCP token.
- `ReviewProposal`: `id`, `executionId`, `assignmentId`, `patchSha256`, `manifestHash`, and `report` (text, capped at the team-summary limit of 2,048 useful bytes; patch has its own 1 MiB limit).
- `ReviewApproval`: `proposalId`, `patchSha256`, `manifestHash`, `applyTarget`, `approvedByUser`, and `autoFix`.
- `CapabilityReport`: only providers whose probe was marked `proven` on this machine. Do not present `unproven` or `missing` entries as supported.

Chat and panel show the same `status`, each role's provider/model, a short patch hash, target, and stable error code. Render report text as text, never HTML. The composer must not start a review automatically: confirmation is a distinct button, separate from Send.

Profiles: keep using `TeamSetup` / `architect_save_profile` / `architect_profiles`. Remove the hardcoded `provider: "codex"`. Each preset role stores the provider and model chosen by the user from the catalog. Keep `rosterOnly: true` and `auto: false`. Reuse an identical setup with the same name; do not overwrite a different setup with the same name, including one with a different provider/model. The task model picker may select this profile, but selecting it does not start a review or silently change the task's mode.

Update `buildReview` and `reviewProblems` so they no longer require `task.provider === "codex"` and `task.mode === "plan"` as substitutes for isolation. Require confirmed execution, proven providers, and proven containment instead. Inserting a prompt is no longer the start path. It may remain as an explicit manual action, clearly labeled as not isolated and outside this feature's acceptance criteria.

**Persistence:** repository analysis does not justify a migration. `SCHEMA_VERSION` 26 has no place to store tokens, manifests, and approvals without a new table, while `architect_library` is a profile library with a different lifecycle and a 256 KiB limit. Decision: do not migrate. In-flight executions die with the process. On startup, scan the temporary review-snapshot directory and remove snapshots with no live execution (after restart, none are live). Profiles remain where they are. If a future requirement needs approvals to survive a restart, revise this plan and add a schema migration and a test comparing `user_version`. Do not do that now.

## 8. Implementation phases and acceptance

Start each phase only after verifying the previous one. Do not announce support beyond what its tests prove.

1. **Probe and containment.** Implement an OS-specific mechanism and tests proving denial. Do not yet expose a UI start action. `containment_unproven` is an acceptable result on an OS without proof.
2. **Snapshot.** Implement copying, manifest, symlink/junction handling, staged, unstaged, untracked, ignored files, and concurrent-change detection. No harness yet.
3. **`pandacode_review` server.** Implement tokens, roles, invalidation, and rejection of calls from another task/execution. Do not launch the 15 CLIs yet.
4. **Harness attachment fixtures.** Add one frame test per provider in the matrix. Any provider without a passing frame remains `capability_missing`.
5. **Roles and proposals.** Verify that the coordinator cannot propose, reviewers cannot apply, and patches do not touch the checkout.
6. **Approval and application.** Implement hashes, revalidation, checkout/worktree targets, journaled rollback, auto-fix, and no commits.
7. **Panel and chat.** Show the same states, require explicit confirmation, support multi-harness profiles, and label insert-prompt as not isolated.
8. **Behavior documentation.** Update [HARDENED-REVIEW-TOOLS.md](./HARDENED-REVIEW-TOOLS.md) only when the behavior exists. Not in this plan-only change.

### Automated tests

These tests can run without installed CLIs or a specific OS, using fixtures:

- Permissions: reject model calls to `start` or apply; reviewer cannot cancel the execution; coordinator cannot call `propose`; a token from another role is rejected.
- Task and execution isolation: a token from execution A/task 1 does not work for execution B or task 2; calls after cancellation or completion fail; `rebind` does not preserve review tokens.
- Start, resume, and cancel: start without confirmation fails; resume never uses the ordinary task's `resume_id` or a warm spare; cancellation kills the listener and fixture process group/job.
- Missing capabilities: when `mcpCapabilities.http != true`, a required tool does not disappear while execution continues; do not change provider; Pi/omp without bridge tools remain `missing`.
- Snapshot: an outside symlink is omitted or causes failure if in scope; junction logic is covered with a metadata fixture; staged and unstaged changes are separately represented in the manifest; ignored files are excluded; a file changing during copy returns `snapshot_unstable`.
- Concurrency: a second proposal from the same assignment does not replace the first without a new ID; apply detects a changed base and returns `base_changed` without writing.
- Approval: a hash other than the approved one is not applied; auto-fix is enabled only by the user's prior flag; auto-fix does not skip revalidation; target never changes silently.
- Patch application: apply to a temporary copy; injected mid-apply failure restores the journal; restoration failure produces `apply_incomplete`; no git commit, push, or merge command is issued.
- Profiles: a non-Codex provider is accepted in the roster; a different setup with the same name is not overwritten; `staff_from_roster` rejects a provider outside the profile.
- Existing-panel regression: Handoff, Attention, and Team inspector still expose no approval command.

Once implemented, use the repository's existing commands: `npm test` for new Node tests alongside `tests/*.test.mjs`; Rust tests alongside `architect/tests.rs` and `providers.rs`. Do not run them as part of this documentation-only change.

### Manual and OS-dependent checks

Do not count these as passing CI evidence until verified on that OS:

- Windows: the job contains child and grandchild processes; writes outside the snapshot and junction traversal outside it fail. If this cannot be demonstrated, that OS remains `containment_unproven`.
- Unix: a process group does not count as a jail; Landlock, bubblewrap, or an equivalent counts only if an escape test actually fails.
- Each installed CLI: opt-in ignored smoke test confirms the live probe and that content went to that provider, not another one.
- Real worktree using `create_worktree`, without merge.
- Denial of arbitrary network access without blocking the provider's existing login, or fail closed if that separation cannot be demonstrated.

Passing fixtures for the existing panel, including `scripts/hardened-review/`, do not prove native containment. `PANDACODE_TEST_ALLOW_NO_SANDBOX` is not acceptable evidence.

Before a native release enables review, the frontend build, browser suite, `npm run test:release`, and Rust tests/build on the supported OS remain required. This is not work for this document.

## Out of scope

- Implementing the server, snapshot, or jail in this plan update.
- Remote sandbox, telemetry, updater, or automatic publishing.
- Commit, push, merge, or profile cleanup.
- Treating plan mode, `readOnly`, or prompt text as isolation.
- Announcing all 15 harnesses as supported before the `pandacode_review` probe passes.
