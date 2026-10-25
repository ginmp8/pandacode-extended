# PandaCode Hardened 0.3.41 — implementation and validation

Date: **24/10/2026 (UTC)**. Product: **`0.3.41-hardened`**. Distribution: complete source code.

## 1. Result and origin

This review starts from the supplied **PandaCode Hardened 0.3.35.4** package and selectively incorporates the prioritized improvements of the **PandaCode 0.3.41** reference. The historical code origin remains upstream **0.3.28**, with later additions. The product number does not represent a full replacement of the baseline or full parity with upstream.

The criteria were frozen before implementation in [ACCEPTANCE.md](ACCEPTANCE.md). The work was split between connections/remote authorization, Skills and history/attachments, in independent worktrees. The integration preserves the executor, secure credential storage, the database and the existing components. All 1,139 files of the source ZIP remain present.

| Input | SHA-256 |
|---|---|
| `pandacode-hardened-0.3.35.4(1).zip` | `479d00145a6c1794f4ca2aedfa2c1de7c5a84e4930c772fe63d2e948da005aab` |
| `pandacode-0.3.41.zip` | `7b988fcec724d6114d3144534bdb0e9a2b9078af2e10810d3fc36c7423e35b98` |

The comparison used the supplied files, extracted with path and symlink checks. The authenticity of these ZIPs was not inferred from any signature. Public GitHub links could not be consulted by the browsing service in this session; the technical reference was the supplied upstream code, identified by the hash above.

## 2. Features incorporated

| Criterion | Implementation | Main evidence |
|---|---|---|
| New session when switching connection | Assignment, editing and removal invalidate resume, context and native state that depend on the account; history stays in the database and is recovered on the next normal turn. Mutation is serialized with admission and refuses active tasks/consoles. | `connections::tests`, `providers::warm::tests`, explicit switch scenario when reaching a limit. |
| Independent remote Full access | Local setting with default `false`, including for older data. Native verification of authenticated origin, mode and trust on start/send/configure/steer/approval/input. | `remote::permissions`, `remote::host`, `remote::config` and `remote.spec.ts`. |
| Remote fingerprint | 128-bit / 32 hexadecimal digit representation. Public keys and trust records kept. | Configuration/identity tests and remote UI. |
| GitHub catalogs for Skills | Existing `gh`, fixed host, tree at an immutable commit, nested discovery, explicit imports, catalog state and source/hash record. | Catalog/import tests and `skills-library.spec.ts`. |
| Skills per bot and project | Selections by id/hash, union and source, verification during admission, limited context, no bot-specific content written to the shared checkout. | `skills::scoped`, `skills_admission_tests`, UI tests. |
| Edit/Rewind | Exclusive fork before the human message, new task without resume, draft and restored snapshots; native reservation against concurrent execution. | `conversation_fork::tests`, `history-upgrade.spec.ts`. |
| Clickable attachments in history | Individual chips; files by native scope, external reveal, images linked to the message/cache and limited GitHub links. Old format stays readable. | `message_attachments`, `prompt_attachments`, 12 Node tests and UI scenarios. |
| Limits per connection | Claude/Codex read the environment of the chosen connection; caches by id/revision, response bound to the account, explicit state for unsupported cases. Continuation on another account requires a click. | Read/cache tests and `hardened-041-connections.spec.ts`. |
| Installed agents picker | Hides uninstalled agents other than the current one; keeps the current one visible as disabled. Models remain subject to the existing catalog. | Two dedicated scenarios and connection/model tests. |

The [guide in English](../../docs/HARDENED-0.3.41-GUIDE.md) identifies the exact UI paths. Complementary technical documentation: [Skills](../../docs/skills-library.md), [connections](../../docs/connection-session-safety.md), [Edit/Rewind](../../docs/rewind.md), [attachments](../../docs/message-attachments.md) and [usage](../../docs/agent-usage.md).

## 3. Preserved controls and design limits

### Connections

Invalidating the context in SQLite finishes before JSON settings or secrets are changed. If a later write fails, it may be necessary to recover the context using the previous account, but the old native identifier does not remain usable with new credentials. There is no distributed transaction across the database, JSON and the system vault; this order favors separation between accounts.

Warm-up rechecks connection, revision, session and generation before starting a process. The console uses the executable, arguments and environment of the named connection. Older Codex controls that create a separate client unable to honor the named account fail explicitly before starting that client. The list is in `docs/connection-session-safety.md`. Normal chat and controls sent to the already authenticated process remain available.

The existing recovery is limited to 200 entries and 64 KiB. It does not transfer private reasoning, internal tool state or image pixels. Edit/Fork creates a task without automatic attribution to the named account; the user can select the connection before sending.

### Remote authorization

The Full access control is in addition to per-device trust, not a replacement for it. A remote device does not receive an RPC to grant its own permission. The setting keeps local paths compatible and does not automatically change the mode of tasks.

Trust and permission are verified before admitting new requests and before forwarding new commands. **Revoking the device or disabling remote Full access does not retroactively cancel work already admitted.** An admitted process may be created later; commands already accepted on the channel and local continuations of an already-started team may proceed. To stop that work, use Stop/Pause on the host. Turning off only Full access still allows Stop and rejection of an approval from a trusted device.

Remote sending to a busy task is not converted into an anonymous local queue: the queue would lose its source identity and would allow later execution without the corresponding verification. The viewer's queue and explicit steering remain available.

### Skills

Discovery does not execute repository content. Importing does not activate a Skill. Commit, source and hash are recorded; locally changed content, conflicting source, symlinks, truncated trees and excessive files are refused. Updating the index does not replace imports, activations or pins.

The text fallback has 8 KiB per item and 32 KiB in aggregate, including the header. It delivers the main text, without automatically materializing auxiliary files. A healthy native activation of the same version is reused. Changes to selection/version/drift invalidate old context on the next admission; a running turn keeps its snapshot.

Tasks with the text fallback must use the chat; the console is not accepted as proof that the Skills prompt was delivered. Healthy native activations remain compatible. Explicitly removing a Skill clears only pins whose bots/projects are verifiably absent. Query or persistence errors abort the removal, without treating unavailable targets as deleted.

### History and attachments

Edit does not modify the original conversation or revert files/commits. The backend validates the human role of the message and the task, and uses the existing reservation to prevent a race with execution. Bot forks are independent tasks without channel identity; they receive the current brief and respect the bot's paused state.

The legacy attachment format is textual and offers no cryptographic provenance. The UI and backend respect fences: examples of `Attached image`, paths or `Bot brief` inside documents/code remain text. Image UUIDs require the expected record structure, a linked message/task and a limited PNG file. External reveal does not execute the file. Project files pass through the existing resolver. No elimination of the inherited TOCTOU window between validation and opening local paths is claimed.

### Usage and compatibility

Supported subscription limits are separated per connection. **Historical monetary costs remain aggregated per provider**. Custom executables/arguments, agents without a verified reader and API credentials do not receive another account's quotas as a substitute. An alternative connection in the banner is compatible with the agent, but there is no guarantee of remaining quota.

There was no SQLite migration, dependency upgrade, new provider, telemetry, updater or automatic installation. Lockfile comparisons confirmed that only the application's own version changed. License and attribution were preserved. Isolated review remains unavailable with `containment_unproven`, as in the baseline.

## 4. Validation performed

Environment: Linux x86_64, Node **24.22.1**, npm **11.9.0**, Rust/Cargo **1.99.1** and Chrome for Testing **153.0.8010.12**. Dependencies were installed explicitly in the validation environment; this does not introduce automatic installation into the product. `npm ci` used `--ignore-scripts`; Cargo used `--locked` and, after provisioning, `--offline`.

| Gate | Baseline 0.3.35.4 | Candidate |
|---|---|---|
| TypeScript + Vite (`npm run build`) | Passed | Passed |
| Node (`npm test`) | 198 passed, 0 failures | 210 passed, 0 failures |
| Release/installation (`npm run test:release`) | 40 passed, 0 failures | 40 passed, 0 failures |
| Native Rust (`cargo test … --lib`) | 1,204 passed, 0 failures, 39 ignored | 1,254 passed, 0 failures, 39 ignored |
| Chrome UI | Focused comparisons for inherited failures | 117 passed / 1 intermittent WhatsApp preview failure in the JSON run; 0 skipped |
| Unpackaged Linux binary | Not executed | Debug-profile compilation passed; executable not launched |
| Lockfiles, license, baseline files | Immutable reference | Dependencies/license preserved; all original files present |

The 39 ignored Rust cases remained ignored; they were not counted as passed. The browser tests use fixture preview/IPC and prove the interface, not authentication or execution with real accounts. The Rust suite exercises native code and policies without starting paid providers.

The logs preserve the initial UI run and the accepted run, plus the baseline comparison results. The first integrated run had 94 passes and 24 failures: 21 inherited outdated expectations/selectors, one fixture incompatible with the requested installed-agents filter, the real Copy defect, and the menu timeout under load. The next run finished with approved status, but its text log was incomplete; the definitive count was recorded in a new run with the JSON reporter, without changing source, selection, asserts or timeouts. That run passed 117 of 118 scenarios; the only failing case was the WhatsApp wizard waiting for the callback after Save. The same case had passed in the previous run with the same source and passed again in an isolated repetition, with no code, assert or timeout changes. The trace links the loss of the newly entered value to a redraw during the simulated 400 ms pairing; HTTPS validation continued to refuse the previous HTTP value. This specific race was not reproduced on the baseline nor tested in the native backend. This result is treated as observed instability, not as a fully green suite; all new scenarios for the requested features passed. The first Node attempt on the candidate happened before `dist` was generated and failed on three bundle preconditions; after the build, the full suite passed. This was not treated as a code change made to make tests pass.

### Findings and fixes during the cycle

- The independent review found improper promotion of markers inside documents into active attachments and text loss on Edit. Equivalent TypeScript/Rust scanners and regressions eliminated the concrete reproduction.
- The Skills resume review found that fingerprint/resume could be used as proof of delivery to the console. The console now requires the chat for the text fallback.
- Pins of deleted targets could block removal forever. Conservative cleanup was integrated into explicit removal, with coverage for errors and valid but inaccessible targets.
- Warm-up and native Codex controls were reviewed to prevent silent use of global credentials in named tasks.
- The UI run exposed outdated inherited selectors for bots and provider settings. The tests were aligned with the real controls and the Hardened `ask` default, preserving content checks, drafts and restrictions. The full catalog test now declares installed agents in its fixture, consistent with the new visibility rule.
- A Copy button overlap defect during streaming was reproduced on the baseline too. The spacing fix is localized; the test keeps the real click, without `force` or relaxing the oracle.
- The first attachment-menu test exceeded the lazy-open deadline during parallel compilation. The trace showed the five correct items just after the limit; the isolated repetition kept the same timeout and the same asserts.

No personal account was used, no credentials were sent to external scanners, and no paid inference was used. The mandatory secret-fixture scanner and `git diff --check` are part of the final check. Old GLib lifetime warnings from vendored code and Vite chunk-size warnings remained visible; they were not silenced nor treated as evidence of a security failure.

## 5. Reproduce

With native dependencies suitable for the system and a test Chrome installed:

```sh
npm ci --ignore-scripts
npm run build
npm test
npm run test:release
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib -- --test-threads=4
cargo build --manifest-path src-tauri/Cargo.toml --locked --bin pandacode
PANDACODE_TEST_CHROME=/path/to/chrome PW_WORKERS=4 npx playwright test \
  tests/hardened-041-connections.spec.ts tests/history-upgrade.spec.ts \
  tests/skills-library.spec.ts tests/remote.spec.ts tests/connections.spec.ts \
  tests/message-actions.spec.ts tests/attachments.spec.ts tests/token-saver.spec.ts \
  tests/usage-limit.spec.ts tests/agent-usage.spec.ts tests/bots.spec.ts \
  tests/bots-web.spec.ts tests/provider-settings.spec.ts
python3 security-review/hardened.3/check-slack-fixtures.py .
```

Do not run heavy compilation at the same time as the UI tests if the machine has few resources. `--offline` can be used with Cargo once the cache is prepared, as in this run. The Chrome path above is an environment parameter, not a file shipped with the distribution.

## 6. Outside validation and selection

Windows or macOS installers were not produced or validated; nor was distribution signing, Explorer/Finder behavior on those platforms, graphical execution of the Tauri window, real access to the user's vault, real providers, authenticated GitHub, or real remote devices. The Linux compilation does not prove those environments. The minimum Rust version announced by the baseline was not tested separately with an old toolchain.

The optional Cline interactive console was not incorporated; the existing provider was kept. Still excluded: removal of per-device trust, updater, automatic installation, new mobile/remote infrastructure, transport of credentials/checkouts to clouds, automatic reads outside the project in Plan, migration of secrets to files, and new Jev/OpenRouter/Kilo providers.

This report records the scope, evidence and limits of a localized functional/security review. It is not an exhaustive audit, a security certification or a guarantee of compatibility with all external versions. Earlier reports included in the package are historical and do not replace the results of this review.

## 7. Verifiable contents of the delivery

`verification.json` consolidates the results and limits. `input-archives.json` identifies the inputs. `source-diff.json` relates added/modified files to before/after hashes, excluding the new evidence directory. `changes.patch` contains the source/configuration/documentation/test diff against the baseline. `source-manifest.json` lists the size and SHA-256 of each packaged file, except the manifest itself. The textual logs and the browser JSON are in `evidence/`. New browser captures, traces, installed dependencies, builds, cache and the Git repository are not distributed.
