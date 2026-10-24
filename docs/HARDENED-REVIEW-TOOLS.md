# Local review and coordination tools

Added in **0.3.35-hardened.2**. This is an optional frontend extension over the existing PandaCode executor, not a second agent platform. No Paseo runtime, dependency, daemon, plugin system or source code was imported.

## Open the tools

Use **Review & coordination** (the branch/worktree icon) at the bottom of the sidebar. The module and its stylesheet load on demand. Four tabs are available: **Review presets**, **Team inspector**, **Attention**, and **Handoff**. The dialog uses the existing close/escape behavior. Closing it discards unsaved form drafts and removes its event handlers.

Opening or refreshing the dialog does not start an agent, approve a request, change permission modes or apply a team. Model discovery uses the existing native catalog. There is no new polling timer, network listener, background service, repository watcher or database migration.

## Advisor and committee

These presets currently prepare **Codex Read only (plan)** tasks only. Other providers remain available in the original application; the inspector, attention view and manual handoff are not Codex-specific. Restricting the presets is intentional: a worker's `readOnly` assignment flag alone does not enforce filesystem permissions.

1. Open **Review presets**, select Advisor or Committee and an installed Codex model. Enter the review question, optional relative file scopes, and acceptance criteria.
2. Use **Save team profile**. This saves a new named profile through the original API; it does not apply it or change task/default settings. An identical profile is reused. A different existing profile with the same name is never overwritten.
3. Create a **new project task** using the original controls. In the original model picker select the saved profile. Select **Read only** using the original mode picker. Selecting a profile retains PandaCode's pre-existing global-default behavior; this extension does not silently configure those defaults.
4. Reopen **Review & coordination**, choose the matching preset/model, enter the review request and review its prompt preview. Use **Insert prompt** when the preparation checks pass.
5. Review the text in the original composer and use the normal **Send** control. Inserting text does not send or enqueue it.

Use a new task without previous messages, a resumed context, previous worker assignments, or a pending team setup. Existing composer drafts and attachments are not overwritten or silently reused. Broader permission modes, remote tasks, unavailable models and mismatched team limits are refused. State is re-read before text preparation and the selected task/composer are checked again before insertion. These are preparation guards, not a replacement for native authorization.

| Preset | Reviewers | Maximum parallel workers | Assignment budget | Lead-turn budget |
|---|---:|---:|---:|---:|
| Advisor | 1 | 1 | 1 | 2: dispatch and synthesis |
| Committee | 2: correctness and risk | 2 | 2 | 2: dispatch and synthesis |

The existing scheduler enforces the configured budgets. The prompt requests one dispatch and a synthesis that preserves disagreements; it does not request repeated rounds until consensus. Model compliance and live provider behavior were not validated in this environment. Manual resume/configuration remains an original user capability and can change the limits later.

File scopes are bounded relative paths, not a filesystem sandbox. The native Codex plan mapping requests read-only access with network access disabled; no live filesystem denial test was executed for this release. Do not represent this as a security certification or provider-independent isolation guarantee.

## Team inspector

Displays up to 50 workers, states, dependencies, file scopes and local waiting reasons. It reuses PandaCode's existing queue-reason helper. The native scheduler remains authoritative and also considers other teams. A missing local blocker is explicitly not a promise that a worker can run. Use **Refresh snapshot** to update; no background polling is added.

Worker buttons open the original worker panel after checking parent-task membership. The inspector cannot reorder, dispatch, cancel, approve or mutate a team.

## Attention

Shows up to 200 items from the currently loaded tasks/teams: approvals, questions, failed/interrupted teams, failed/waiting workers and failed dependencies. Items are deduplicated. Their text is rendered as text, never interpreted as HTML. This view is deliberately a local snapshot, not a new notification or approval system.

**Open task / Open worker** navigates to the original controls. No approve, answer, retry or resume command is exposed by this extension. Pending-request coverage may be incomplete after restart or before a task/team has been loaded. Refreshing the view does not run the original auto-resume helper.

## Handoff

Fill in goal, selected context, decisions/failed attempts, next steps, reported checks and acceptance criteria. Basic task/project-name metadata is included; the full transcript, paths, attachments, environment variables and credential stores are not collected for the handoff. Worker checkpoints are opt-in and limited to the selected team.

Each manual field is limited to 4,096 UTF-8 bytes; the final handoff is limited to 16,384 bytes. Checkpoints are clipped and clearly labelled as agent-reported, not independently verified. Common credential patterns and private-key blocks are redacted, including incomplete private-key blocks. This is best-effort filtering, not a complete secret detector.

Read the preview, then check **I reviewed this preview for sensitive information** and click **Copy handoff**. Any changed preview requires a new confirmation. No agent starts, no history is removed and no automatic file is created. Clipboard history or clipboard-sync software may retain copied information; inspect the text before copying. When clipboard access is denied, the error remains visible and the preview can be selected manually.

## Development verification

Use the project's existing dependencies and installation policy; these tools do not download dependencies. Existing tests remain unchanged, and the three new test files are discovered by `npm test`. Node must support TypeScript stripping, as already needed by the repository's existing `.ts` imports. On Node 22.16 use `node --experimental-strip-types --test tests/*.test.mjs`.

Optional browser-fixture checks require an already available Python Playwright installation and Chromium. They run the new panel's real modules against a synthetic native-IPC boundary, not the desktop application:

```text
node scripts/hardened-review/compile-ui.mjs ABSOLUTE_PROJECT_ROOT ABSOLUTE_OUTPUT_DIR
python scripts/hardened-review/browser-regressions.py ABSOLUTE_OUTPUT_DIR ABSOLUTE_SCREENSHOT_DIR
```

Set `CHROMIUM_EXECUTABLE` only when needed for an installed browser. The fixture aborts network requests and uses in-memory HTML/data modules. `PANDACODE_TEST_ALLOW_NO_SANDBOX=1` is an explicit option only for an already assessed disposable CI/container host; it is not needed or recommended for ordinary local development. It was used in the supplied constrained browser fixture, which does not prove native process isolation.

Before accepting a native release, run the complete frontend build, original browser suite, `npm run test:release`, and native Rust tests/build in the supported OS/toolchain. Exercise both presets through actual providers and verify denial of writes in plan mode. These checks remain required; the recorded fixture passes do not replace them.

## Upstream maintenance and rollback

Only `src/main.ts` was changed among original application-code files. It adds the sidebar entry, lazy mounting/cleanup, a narrow existing-API adapter, and action routing. The four new runtime files are under `src/hardened/`. Rust source, provider protocols, scheduler, capabilities, CSP, existing tests and dependency graphs are unchanged. Version metadata and one README paragraph were updated separately.

`patches/paseo-review-tools.patch` contains only the runtime extension and its integration, without dependency or version changes. `git apply --check` succeeded against the supplied original PandaCode 0.3.35 `src/main.ts`; this is a textual check, not full runtime compatibility. Check against each new upstream revision, then build and test. Do not blindly apply the patch to an unreviewed future release.

To remove the extension, reverse its integration patch on the exact matching tree, or remove the small sidebar/action/cleanup/helper additions and `src/hardened/` after reviewing your diff. Do not reset unrelated changes. Saved profiles are user data in the original profile library and should be removed through its original UI only when desired. No automatic database rollback or profile deletion is performed.

See `security-review/0.3.35-hardened.2/REPORT.md` for evidence and limitations. Historical reports shipped with the baseline are retained as historical material, not renewed approvals.
