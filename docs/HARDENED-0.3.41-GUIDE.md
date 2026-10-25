# PandaCode Hardened 0.3.41 — usage guide

This version was built from the complete **Hardened 0.3.35.4** package. The internal number is `0.3.41-hardened`. The feature reference is upstream 0.3.41; the historical code origin remains 0.3.28, with later selective additions. The package contains the complete source code, without a prebuilt native installer.

## Where to find each improvement

| Feature | UI path | Result |
|---|---|---|
| Connections and account switching | **Settings → Providers**; then the agent/model picker in the composer | Switching preserves the conversation and starts a new native context. |
| Independent remote authorization | **Settings → Remote access → Full access for remote tasks** | Off by default; only the host computer can enable it. |
| Device fingerprint | **Settings → Remote access**, device list | 128-bit display; trust remains tied to the real public key. |
| GitHub catalogs | **Settings → Skills & rules → GitHub catalogs** | Discover, select and import Skills at a fixed commit. |
| Skills per bot/project | **Settings → Skills & rules → Bot & project skills** | Choose scope, target and versions; click **Save skill selection**. |
| Effective task skills | Same page, **Current task: effective skills** | Shows the source, delivery method and the reason for skipped items. |
| Edit/Rewind | **Edit** action on an earlier message sent by the user | Creates a new task with the earlier history and an editable draft. |
| Attachments in history | Chips next to the user message | Open a file, reveal its source, enlarge an image, or open an issue/PR. |
| Limits per connection | **Agent usage** icon, next to Help/Settings; the agent's tab | Separates the default setup from each named connection. |
| Installed agents | Agent/model picker in the composer | Offers the installed agents; keeps the current one visible as disabled if unavailable. |

The names above match the application's English labels. You do not need another interface to find these features.

## 1. Switch connection with history preserved

Register connections in **Settings → Providers**. In the composer picker, choose the connection's group and the model. The task keeps the same visible history but loses the resume identifier of the previous account. The next message uses the existing context recovery mechanism.

Editing or removing a connection also invalidates the contexts of linked tasks. A running task or console must be stopped before the change. Selecting the same connection again does not cause an unnecessary restart.

Recovery carries at most 200 recent entries and 64 KiB. It does not carry private reasoning, internal tool state or previous image pixels; re-attach images you still need. Switching accounts does not undo file changes.

Some older Codex controls open a separate client with the default account. In this version, these paths refuse tasks with a named connection before opening the client: independent account/configuration/import queries, inspection of a saved goal, certain Plan/Fast preferences, native fork/adopt/archive/rename operations, and opening a saved Codex conversation in the console when that inspection is required. Normal chat, controls forwarded to the already authenticated process, and PandaCode's Edit/Fork remain available. See the list in [connection-session-safety.md](connection-session-safety.md).

## 2. Remote Full access

**Full access for remote tasks** starts off, including when older settings are loaded. Trusted devices still need independent authorization from the host to start, continue or change a task in Full access. Enabling the control does not automatically change the mode of any task.

Checks run in the backend and are repeated when the execution is admitted. Trust, revocation, public key, hosting and replay protection are still verified. The remote device does not receive an operation to enable its own permission. Revoking a device or turning off the authorization blocks new remote requests and admissions; it does not retroactively cancel turns, commands or goals already accepted. To stop them, use **Stop** or **Pause** on the host.

Sending a remote message to a busy task does not silently enter a local queue without origin identity. Wait for it to be released or use the explicit steering action. This restriction prevents a request from remaining executable after it has lost its authorization context.

The fingerprint now shows 32 hexadecimal characters, corresponding to 128 bits of the key digest. There was no key exchange, no removal of trusted devices, and no replacement of authentication with text comparison.

## 3. Discover and import Skills from GitHub

1. Open **Settings → Skills & rules → GitHub catalogs**.
2. Enter `owner/repository` and, optionally, a branch, tag or commit.
3. Click **Add catalog**. Discovery uses the `gh` tool already installed and configured for GitHub.com.
4. Review the packages found, including those in nested directories.
5. Select the items and click **Import selected**. **Select all** prepares a batch selection.
6. Review the findings and activate through the existing library, or assign to a bot/project.

Discovering and importing does not run scripts, start an agent or activate a Skill automatically. The record keeps the source, path, ref, immutable commit and the SHA-256 hash of the imported content.

**Refresh catalog** updates only the index. **Update available** reports a commit change without replacing copies or selections. **Disabled** and **Ignored** block updates and imports. **Remove catalog** removes the index and keeps the copies already imported.

Name/source conflicts and local changes block overwriting. A batch may import some items and skip others; the UI reports each reason. Large packages, truncated trees, unsafe paths, symlinks and submodules are rejected or excluded by the limits described in [skills-library.md](skills-library.md).

## 4. Assign Skills to bots and projects

In the **Bot & project skills** section, choose **Project** or **Bot**, select the target, check the Skills and click **Save skill selection**. If there are findings, review them in the library and confirm acceptance of the specific versions selected.

Each selection stores the id and hash. A new import does not silently change approved versions. After an update, review and save the selection again. Removed or modified items, items incompatible between bot and project, or items still awaiting approval appear as skipped, with a reason.

A task combines the scopes of its bot and project and deduplicates shared items. The source appears as `bot`, `project`, `both` or `user`. The delivery appears as `native`, `prompt` or `skipped`.

Specific selections add limited text to the context, without writing to the shared checkout. The limit is 8 KiB per Skill and 32 KiB in total, including the header. An existing native activation that is intact and of the same version is reused without duplicating text. Changing Skills invalidates the old resume on the next admission and recovers the history without the previous synthetic block. A turn already started keeps its snapshot; the change applies to the next one.

The text fallback delivers the main content of `SKILL.md`. Auxiliary files are not materialized automatically for bots, nor read outside the project. Skills that depend on those files may require an explicit native activation in the project. Tasks using this fallback must use the chat: the interactive console cannot confirm delivery of the text and stays blocked while that context is present. With healthy native activations, after changing selections send a normal message before resuming the console.

## 5. Edit/Rewind and attachments

On a stopped task, click **Edit** on the user message from which you want to redo the conversation. PandaCode creates an independent task, copies only the history before the chosen message and loads its text into the composer. Review the draft, the attachments and the connection before sending.

The original conversation stays intact. The new task does not reuse the old resume ID and does not automatically inherit a named connection; choose the account you want in the picker. Independent forks created in a bot's project can be reopened in **Bot settings → Activity → Conversations → Independent tasks**. **Editing does not revert the filesystem**: changes made by later messages may still exist in the files.

Chips show each message's attachments. Project files use the existing tabs; external items are revealed in the system file manager; available images open in an enlarged view; valid GitHub issue and pull request links open in the browser. Clicking historical content does not bypass the native checks for paths, source message and permissions.

If a file has changed or an image is no longer in the cache, the draft reports the limitation. Review the restored content before resending. Text examples inside code blocks remain text and are not automatically promoted to attachments.

## 6. Limits per account and agent choice

Open **Agent usage** and the Claude Code or Codex tab. Each named connection shows its own result, separate from **Default setup**. Unavailable data stays unavailable. Setups with custom executables/arguments and other providers without a verified reader do not receive default-account numbers as a substitute.

The panel separates **subscription limits per connection**. The historical monetary estimate remains aggregated per agent/provider; no per-account spending breakdown was invented.

When a task reaches a reported limit, the banner may offer **Continue on…** another connection of the same agent. The choice is explicit, starts a new native session, and does not guarantee that the alternative account has remaining quota. There is no automatic switching.

The model picker shows installed agents. If the current task's agent is no longer available, it stays visible and disabled, so you can see the state without being offered an impossible run.

## 7. Build, validate and know the limits

For the frontend:

```text
npm ci --ignore-scripts
npm run build
npm test
npm run test:release
```

With Rust and the system's native dependencies installed:

```text
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
```

On Windows, follow [install-windows-hardened.md](install-windows-hardened.md). This delivery does not contain a signed Windows/macOS installer and does not prove operation with your real accounts. Executed results, commands, comparison with the baseline and limitations are in the [version report](../security-review/0.3.41-hardened/REPORT.md).

The optional Cline interactive console was not incorporated. The existing Cline provider was preserved. The updater, automatic installers, new cloud providers, migration of secrets to files and removal of device trust also remain excluded.
