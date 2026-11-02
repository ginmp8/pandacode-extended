# PandaCode Extended 0.7.0 — Configuration behavior, open verification and confirmed fixes

**Source baseline:** Complete functional import through PandaCode `0.3.90` on Extended `base/development` (commit `d3980c0b`). The previously reviewed `0.3.85` candidate/PR #86 is historical evidence, not the current application version.  
**Scope:** This register records implementation details, expected setting scope, verification gaps and confirmed bugs. **An unverified concern, an Off-by-default setting, or a non-retroactive toggle is not by itself a demonstrated security vulnerability or justification to remove an upstream feature.** See the [full parity contract](../upstream/parity-contract.md).  
**CI checkpoint:** [41463892251](https://github.com/ginmp8/pandacode-extended/actions/runs/41463892251): npm and RustSec audits passed; the Linux Rust job had one failure in source-identical `remote/catalog.rs`. No full-CI success is asserted.

## Evidence classes

| ID | Classification | Verified observation or open question |
| --- | --- | --- |
| [KI-01](#ext-070-ki-01--codex-mcp-isolation-may-not-be-enforced) | **Integration verification pending** | Codex effective external-MCP isolation when Off is untested; **not** a confirmed security exploit |
| [KI-02](#ext-070-ki-02--cloud-machines-may-be-provisioned-before-the-credential-gate) | **Observed sequencing limitation** | Machine preparation precedes the credential-transfer check; review user-facing behavior and resource side effects |
| [KI-03](#ext-070-ki-03--credentials-already-sent-to-cloud-machines-are-not-revoked) | **Expected setting scope** | Disabling new transfers does not retroactively revoke earlier credentials |
| [KI-04](#ext-070-ki-04--failed-settings-reads-can-reactivate-bots-or-lan-routes) | **Recovery-path defect candidate** | Invalid preference reads use defaults; the corruption/restart impact requires a reproducible integration test |
| [KI-05](#ext-070-ki-05--running-agents-retain-launch-time-mcp-configuration) | **Expected setting scope** | New-process configuration does not retroactively reconfigure running agents |
| [KI-06](#ext-070-ki-06--mobile-expo-dependency-audit-remediated-in-source) | **Confirmed dependency finding remediated in source** | Patched versions, vendor workaround and passing npm audits; native release remains separate |
| [KI-07](#ext-070-ki-07--legacy-extended-sqlite-schema-collision) | **Confirmed compatibility defect fixed in source** | Migration 37 and its test; actual upgrades still require native validation |

**Interpretation:** Keep a verification task open where evidence is missing; do not assign speculative blocker/major security severities. Verified vulnerabilities may still require fixes, but **every fix must preserve source features and existing Settings controls**. The two documented setting-scope items (KI-03 and KI-05) are not unresolved bugs merely because they describe limits. The historical IDs and evidence below are retained for traceability.

## EXT-070-KI-01 — Codex MCP isolation may not be enforced

**Classification:** Integration verification pending; severity is **not established** without a tested behavior mismatch  
**Confidence:** Potential defect; effective runtime behavior is **not yet established**.  
**Evidence:** [`src-tauri/src/providers.rs`](../../../src-tauri/src/providers.rs) configures Codex with `-c "mcp_servers={}"` when `external_mcp_servers` is false. The intended isolation depends on how the installed Codex version merges that override with existing user/project MCP definitions.

**Impact:** The UI could say the external MCP policy is Off while previously configured servers remain available to Codex. A green build cannot establish effective tool isolation.

**Interim workaround:** Do not rely on this switch alone for strict isolation. Individually disable or remove external MCP servers in the relevant Codex configuration, or run in an independently isolated profile, then inspect the available tool list.

**Future work:** Add a version-aware integration test with a deliberately configured external MCP server; verify that disabled mode advertises zero external MCP tools while retaining PandaCode's own authenticated in-process tools. Use a tested isolation mechanism rather than assuming an empty-map override clears inherited settings.

**Acceptance:** A real Codex process launched with the flag Off cannot discover/call the test MCP server; On restores the configured server. Validate fresh and resumed conversations.

## EXT-070-KI-02 — Cloud machines may be provisioned before the credential gate

**Classification:** Confirmed operation ordering, not a confirmed security vulnerability  
**Confidence:** Confirmed source ordering.  
**Evidence:** [`src-tauri/src/cloud_agents/tasks.rs`](../../../src-tauri/src/cloud_agents/tasks.rs) calls `prepare_machine` before `credentials::push`; the Off check currently runs inside [`credentials.rs`](../../../src-tauri/src/cloud_agents/credentials.rs). Creating/starting a machine and syncing a checkout can therefore precede the refusal.

**Impact:** A rejected cloud-agent start can still create or start billable resources and upload workspace files.

**Interim workaround:** Check the Feature controls switches before initiating a cloud task; if a launch fails at the credential step, inspect the provider for machines created or left running and stop/delete them explicitly where safe.

**Future work:** Move capability/credential preflight ahead of machine preparation, and ensure failures do not leave unexpected resources. Provide a user-facing explanation before launch.

**Acceptance:** With the opt-in Off, no cloud create/start/upload operation occurs; with it On, existing authorized behavior is preserved. Include failed provision and retry cases.

## EXT-070-KI-03 — Credentials already sent to cloud machines are not revoked

**Classification:** Documented setting scope, not a bug in the transfer opt-out  
**Confidence:** Confirmed limitation by the control implementation and documentation.  
**Evidence:** [`src-tauri/src/cloud_agents/credentials.rs`](../../../src-tauri/src/cloud_agents/credentials.rs) rejects **new** transfers while Off; no corresponding cleanup or token revocation is performed by the Feature controls setter.

**Impact:** Turning the switch Off does not remove earlier uploaded login files or revoke tokens already available in cloud environments.

**Interim workaround:** If existing cloud credentials should no longer be valid, inspect affected machines and independently revoke/rotate tokens and remove the transferred login files using the appropriate provider controls. Avoid deleting workspaces or machines without checking for user data.

**Future work:** Add an explicit per-machine **Revoke/clean transferred credentials** flow, distinguish local opt-out from remote revocation in UI, and report partial failures without implying success.

**Acceptance:** Opt-out never claims retroactive revocation; a separately confirmed cleanup removes only the selected credentials, verifies its result and does not delete unrelated data.

## EXT-070-KI-04 — Failed settings reads can reactivate bots or LAN routes

**Classification:** Confirmed fallback code; actual restart/corruption behavior still needs reproduction  
**Confidence:** Confirmed fallback behavior; real corruption/restart case remains untested.  
**Evidence:** [`src-tauri/src/feature_controls.rs`](../../../src-tauri/src/feature_controls.rs) treats absent, unreadable and invalid stored JSON the same way by using `Settings::default()`, where `bots_paused=false`. [`src-tauri/src/remote/routes.rs`](../../../src-tauri/src/remote/routes.rs) also defaults `direct_routes=true` if route settings cannot be read.

**Impact:** A user-selected global bot pause can be silently lost when configuration storage becomes unavailable or corrupted. With Connect independently enabled, direct LAN routing may also be restored without the user's intent.

**Interim workaround:** After storage, system-account or keyring errors, verify bot/channel and Connect-route states before relying on them. Where necessary disable Connect, channels and bot schedules at their own controls.

**Future work:** Distinguish **missing** from **corrupt/unreadable** preferences; do not reactivate existing automation or network routes on a read error. Surface a persistent recovery warning and add startup/restart tests.

**Acceptance:** Normal saved values survive restart; missing settings use documented defaults; invalid or unreadable saved settings trigger a safe, visible recovery mode without silently enabling paused bots or previously disabled direct routes.

## EXT-070-KI-05 — Running agents retain launch-time MCP configuration

**Classification:** Expected launch-time application of a configuration preference; not a security defect by itself.  
**Confidence:** Confirmed process-level design; individual harness behavior may differ.  
**Evidence:** [`src-tauri/src/providers.rs`](../../../src-tauri/src/providers.rs) assembles MCP arguments before process spawn; [`src-tauri/src/providers/warm.rs`](../../../src-tauri/src/providers/warm.rs) includes the preference in the warm-process key. The preference setter does not forcibly restart existing processes.

**Impact:** Switching External MCP servers Off can leave already-running agents using their original MCP configuration until a new process starts.

**Interim workaround:** Stop and restart affected agent sessions when immediate isolation is required. Do not interpret the switch as revoking an existing process's permissions retroactively.

**Future work:** Label this preference **Applies to newly started agent processes** and, optionally, offer an explicit, confirmed **Stop/restart affected runs** action.

**Acceptance:** The UI clearly communicates timing, running work is not killed without approval, and new runs obey the saved preference.

## EXT-070-KI-06 — Mobile / Expo dependency audit remediated in source

**Historical classification:** Dependency advisory remediation and release compatibility gate.  
**Resolution status:** **Resolved in the Extended source candidate**, subject to final CI and native-device release validation. This does **not** mean every upstream library has published an official fix.  
**Initial evidence:** [CI run 41242276061](https://github.com/ginmp8/pandacode-extended/actions/runs/41242276061) reported 27 npm audit findings (3 high, 23 moderate, 1 low) in Expo mobile dependencies.  
**Remediation source:** [Extended security commits](https://github.com/ginmp8/pandacode-extended/pull/86) and the [dependency security record](dependency-security.md).

**Remediation:**

- Upgrade mobile `glob` 13.0.6 → 14.0.3, `minimatch` 10.2.6/3.1.5 → 11.0.3 (including nested ESLint copies), `node-forge` 1.4.0 → 1.4.2, `zod` 3.25.76 → 4.6.7 and `path-scurry` 2.0.2 → 3.0.0; pin transitive versions via the mobile project's npm overrides.
- The public npm advisory for `deepmerge` has no official patched release. Downgrading to 4.1.1 made the audit green **but still reproduced prototype poisoning in a real regression test**. The Extended-only vendored fork [`mobile/vendor/extended/deepmerge`](../../../mobile/vendor/extended/deepmerge/README.md) is based on MIT-licensed upstream 4.3.1 and blocks malicious keys in both merge inputs. No change is made to the upstream PandaCode repository.
- Preserve `npm audit --audit-level=low` on all npm workspaces and add independent mobile frozen install, tests, lint, Expo config and iOS JS bundle export checks.

**Verified result:** The security audit for all four npm workspaces passed with 0 findings in [CI run 41243766188](https://github.com/ginmp8/pandacode-extended/actions/runs/41243766188), and the mobile regression progressed beyond the formerly failing poisoning assertion. Final mobile export/native validation must be read from the complete run; do not claim it passed without its job conclusion.

**Further action:** Monitor for an official patched `deepmerge` release and replace the Extended vendored copy after independent poisoning tests and full Expo compatibility verification. Native iOS/Android binary smoke tests remain a distinct release-acceptance requirement.

## EXT-070-KI-07 — Legacy Extended SQLite schema collision

**Historical classification:** Reproduced database upgrade compatibility failure. Task and Schedules pages can fail when older Extended databases meet the imported upstream schema.  
**Source status:** Fixed in the unreleased source candidate; final CI and native upgrade acceptance are distinct gates.  
**Symptom:** The task-list query raises `no such column: cloud` while fetching sessions. The Schedules UI can surface the same error because it also loads the shared session list.  
**Cause:** Extended main assigned schema versions 27–32 to different changes than upstream 0.3.85. In Extended, migration 27 created `github_issue_actions`, whereas upstream migration 27 added `sessions.cloud`. Upstream migration 32 added `sessions.remote_backfill_base`, and bot capability/table additions also diverged. An existing Extended v32 database skips these older-numbered upstream migrations.  
**Fix:** Transactional schema migration **37** checks for missing `sessions.cloud` and `sessions.remote_backfill_base` before adding them and runs idempotent bot soul, capability and schema upgrades, including `bots.skills` and shared bot tables. Existing task records and MCP references are preserved; there are no destructive table rebuilds or automatic permission changes.  
**Regression test:** `storage::tests::upgrades_extended_v32_database_without_losing_tasks_or_bot_settings` uses a file-backed database with removed upstream objects and a preserved session/bot, then exercises `Store::open`, session and Schedules listing, bot values and reopen behavior. An isolated SQLite test reproduced the missing-column failure before applying the additive SQL.  
**Limits:** This fix takes effect only when a corrected build opens the database. Real upgraded installations, full bot permission parity and native installer behavior remain separate validation requirements. Back up the database before installing a new unreleased build.

## Release-readiness notes (separate from the five open application findings)

- The planned Extended release number is `0.7.0`; the imported upstream app's package/Cargo version is now `0.3.90` on `base/development`. Extended release assets/updater identity and native acceptance remain separate from the source version.
- The mobile npm dependency remediation is tracked as a source-level resolution, not a native binary release, and remains subject to final compatibility gates.
- Extended-specific history on other branches (including the older `releases/extended/0.6.0.md` on `main`) must not be conflated with upstream functional/source parity. Preserve relevant Extended history before any later branch merge; no upstream feature removal is justified.
- [CI run #41224890859](https://github.com/ginmp8/pandacode-extended/actions/runs/41224890859) passed 3/3 checks at `2f21bb67`, but did **not** certify native Settings restart, effective Codex MCP isolation, cloud cost/preflight, or a two-device Connect scenario.
- This register explains code and validation evidence; it does not independently approve release, prove every runtime path, or authorize deletion/deactivation of upstream capabilities.

## Maintaining this register

Keep the IDs stable. For each actual fix, add implementation and regression evidence. Distinguish verified defects, untested hypotheses and documented setting scope; do not escalate a theoretical concern to a confirmed security problem or mark a fix accepted solely because an audit passed.
