# PandaCode Extended 0.7.0 — Known issues and remediation status

**Status:** Five non-dependency issues open; dependency issue **KI-06 resolved in the source candidate** of the **unreleased** [Extended 0.7.0 source candidate](../../../release/extended/0.7.0.md).  
**Source reviewed:** [`2f21bb67`](https://github.com/ginmp8/pandacode-extended/commit/2f21bb67e3d59b5b01f794a8cc6d8b686f5a95eb), PR [#86](https://github.com/ginmp8/pandacode-extended/pull/86), reviewed 2026-10-31.  
**Scope:** Findings observed in the candidate code and risks that require runtime reproduction. **Documented does not mean fixed, accepted, or verified at runtime.**

## Summary

| ID | Severity | Confidence | Subject | Status |
| --- | --- | --- | --- | --- |
| [EXT-070-KI-01](#ext-070-ki-01--codex-mcp-isolation-may-not-be-enforced) | Blocker | Needs integration verification | Codex external MCP isolation | Open, deferred |
| [EXT-070-KI-02](#ext-070-ki-02--cloud-machines-may-be-provisioned-before-the-credential-gate) | Major | Confirmed code order | Cloud provisioning before opt-in preflight | Open, deferred |
| [EXT-070-KI-03](#ext-070-ki-03--credentials-already-sent-to-cloud-machines-are-not-revoked) | Major | Confirmed limitation | Previously transferred credentials | Open, deferred |
| [EXT-070-KI-04](#ext-070-ki-04--failed-settings-reads-can-reactivate-bots-or-lan-routes) | Major | Confirmed fallback behavior | Inaccessible or corrupted settings | Open, deferred |
| [EXT-070-KI-05](#ext-070-ki-05--running-agents-retain-launch-time-mcp-configuration) | Minor / policy-dependent | Confirmed launch-time behavior | Existing agent sessions | Open, deferred |

| [EXT-070-KI-06](#ext-070-ki-06--mobile-expo-dependency-audit-remediated-in-source) | Major / original release gate | Audit + source regression + mobile checks | Mobile/Expo transitive dependency vulnerabilities | Resolved in source candidate (Extended fix) |

Severities are provisional engineering priorities, not evidence of an exploited vulnerability. The owner may schedule remediation in a later revision, but merge/release risk acceptance requires a separate, explicit decision.

## EXT-070-KI-01 — Codex MCP isolation may not be enforced

**Severity:** Blocker (security-policy mismatch)  
**Confidence:** Potential defect; effective runtime behavior is **not yet established**.  
**Evidence:** [`src-tauri/src/providers.rs`](../../../src-tauri/src/providers.rs) configures Codex with `-c "mcp_servers={}"` when `external_mcp_servers` is false. The intended isolation depends on how the installed Codex version merges that override with existing user/project MCP definitions.

**Impact:** The UI could say the external MCP policy is Off while previously configured servers remain available to Codex. A green build cannot establish effective tool isolation.

**Interim workaround:** Do not rely on this switch alone for strict isolation. Individually disable or remove external MCP servers in the relevant Codex configuration, or run in an independently isolated profile, then inspect the available tool list.

**Future work:** Add a version-aware integration test with a deliberately configured external MCP server; verify that disabled mode advertises zero external MCP tools while retaining PandaCode's own authenticated in-process tools. Use a tested isolation mechanism rather than assuming an empty-map override clears inherited settings.

**Acceptance:** A real Codex process launched with the flag Off cannot discover/call the test MCP server; On restores the configured server. Validate fresh and resumed conversations.

## EXT-070-KI-02 — Cloud machines may be provisioned before the credential gate

**Severity:** Major (unintended cloud cost / misleading failure)  
**Confidence:** Confirmed source ordering.  
**Evidence:** [`src-tauri/src/cloud_agents/tasks.rs`](../../../src-tauri/src/cloud_agents/tasks.rs) calls `prepare_machine` before `credentials::push`; the Off check currently runs inside [`credentials.rs`](../../../src-tauri/src/cloud_agents/credentials.rs). Creating/starting a machine and syncing a checkout can therefore precede the refusal.

**Impact:** A rejected cloud-agent start can still create or start billable resources and upload workspace files.

**Interim workaround:** Check the Feature controls switches before initiating a cloud task; if a launch fails at the credential step, inspect the provider for machines created or left running and stop/delete them explicitly where safe.

**Future work:** Move capability/credential preflight ahead of machine preparation, and ensure failures do not leave unexpected resources. Provide a user-facing explanation before launch.

**Acceptance:** With the opt-in Off, no cloud create/start/upload operation occurs; with it On, existing authorized behavior is preserved. Include failed provision and retry cases.

## EXT-070-KI-03 — Credentials already sent to cloud machines are not revoked

**Severity:** Major (credential lifetime / revocation clarity)  
**Confidence:** Confirmed limitation by the control implementation and documentation.  
**Evidence:** [`src-tauri/src/cloud_agents/credentials.rs`](../../../src-tauri/src/cloud_agents/credentials.rs) rejects **new** transfers while Off; no corresponding cleanup or token revocation is performed by the Feature controls setter.

**Impact:** Turning the switch Off does not remove earlier uploaded login files or revoke tokens already available in cloud environments.

**Interim workaround:** If existing cloud credentials should no longer be valid, inspect affected machines and independently revoke/rotate tokens and remove the transferred login files using the appropriate provider controls. Avoid deleting workspaces or machines without checking for user data.

**Future work:** Add an explicit per-machine **Revoke/clean transferred credentials** flow, distinguish local opt-out from remote revocation in UI, and report partial failures without implying success.

**Acceptance:** Opt-out never claims retroactive revocation; a separately confirmed cleanup removes only the selected credentials, verifies its result and does not delete unrelated data.

## EXT-070-KI-04 — Failed settings reads can reactivate bots or LAN routes

**Severity:** Major (unsafe fallback / persistence integrity)  
**Confidence:** Confirmed fallback behavior; real corruption/restart case remains untested.  
**Evidence:** [`src-tauri/src/feature_controls.rs`](../../../src-tauri/src/feature_controls.rs) treats absent, unreadable and invalid stored JSON the same way by using `Settings::default()`, where `bots_paused=false`. [`src-tauri/src/remote/routes.rs`](../../../src-tauri/src/remote/routes.rs) also defaults `direct_routes=true` if route settings cannot be read.

**Impact:** A user-selected global bot pause can be silently lost when configuration storage becomes unavailable or corrupted. With Connect independently enabled, direct LAN routing may also be restored without the user's intent.

**Interim workaround:** After storage, system-account or keyring errors, verify bot/channel and Connect-route states before relying on them. Where necessary disable Connect, channels and bot schedules at their own controls.

**Future work:** Distinguish **missing** from **corrupt/unreadable** preferences; do not reactivate existing automation or network routes on a read error. Surface a persistent recovery warning and add startup/restart tests.

**Acceptance:** Normal saved values survive restart; missing settings use documented defaults; invalid or unreadable saved settings trigger a safe, visible recovery mode without silently enabling paused bots or previously disabled direct routes.

## EXT-070-KI-05 — Running agents retain launch-time MCP configuration

**Severity:** Minor for documented deferred application; Major if users expect immediate enforcement.  
**Confidence:** Confirmed process-level design; individual harness behavior may differ.  
**Evidence:** [`src-tauri/src/providers.rs`](../../../src-tauri/src/providers.rs) assembles MCP arguments before process spawn; [`src-tauri/src/providers/warm.rs`](../../../src-tauri/src/providers/warm.rs) includes the preference in the warm-process key. The preference setter does not forcibly restart existing processes.

**Impact:** Switching External MCP servers Off can leave already-running agents using their original MCP configuration until a new process starts.

**Interim workaround:** Stop and restart affected agent sessions when immediate isolation is required. Do not interpret the switch as revoking an existing process's permissions retroactively.

**Future work:** Label this preference **Applies to newly started agent processes** and, optionally, offer an explicit, confirmed **Stop/restart affected runs** action.

**Acceptance:** The UI clearly communicates timing, running work is not killed without approval, and new runs obey the saved preference.

## EXT-070-KI-06 — Mobile / Expo dependency audit remediated in source

**Previous severity:** Major; originally a release gate.  
**Resolution status:** **Resolved in the Extended source candidate**, subject to final CI and native-device release validation. This does **not** mean every upstream library has published an official fix.  
**Initial evidence:** [CI run 41242276061](https://github.com/ginmp8/pandacode-extended/actions/runs/41242276061) reported 27 npm audit findings (3 high, 23 moderate, 1 low) in Expo mobile dependencies.  
**Remediation source:** [Extended security commits](https://github.com/ginmp8/pandacode-extended/pull/86) and the [dependency security record](dependency-security.md).

**Remediation:**

- Upgrade mobile `glob` 13.0.6 → 14.0.3, `minimatch` 10.2.6/3.1.5 → 11.0.3 (including nested ESLint copies), `node-forge` 1.4.0 → 1.4.2, `zod` 3.25.76 → 4.6.7 and `path-scurry` 2.0.2 → 3.0.0; pin transitive versions via the mobile project's npm overrides.
- The public npm advisory for `deepmerge` has no official patched release. Downgrading to 4.1.1 made the audit green **but still reproduced prototype poisoning in a real regression test**. The Extended-only vendored fork [`mobile/vendor/extended/deepmerge`](../../../mobile/vendor/extended/deepmerge/README.md) is based on MIT-licensed upstream 4.3.1 and blocks malicious keys in both merge inputs. No change is made to the upstream PandaCode repository.
- Preserve `npm audit --audit-level=low` on all npm workspaces and add independent mobile frozen install, tests, lint, Expo config and iOS JS bundle export checks.

**Verified result:** The security audit for all four npm workspaces passed with 0 findings in [CI run 41243766188](https://github.com/ginmp8/pandacode-extended/actions/runs/41243766188), and the mobile regression progressed beyond the formerly failing poisoning assertion. Final mobile export/native validation must be read from the complete run; do not claim it passed without its job conclusion.

**Further action:** Monitor for an official patched `deepmerge` release and replace the Extended vendored copy after independent poisoning tests and full Expo compatibility verification. Native iOS/Android binary smoke tests remain a distinct release-acceptance requirement.

## Release-readiness notes (separate from the five open application findings)

- The planned Extended release number is `0.7.0`, but the imported app's package/Cargo version remains `0.3.85`. Version metadata, release assets and updater identity must be reconciled before tagging.
- The mobile npm dependency remediation is tracked as a source-level resolution, not a native binary release, and remains subject to final compatibility gates.
- The upstream source replacement omitted some Extended-only release/documentation files present on the prior `main`, including `releases/extended/0.6.0.md`. Preserve the required history and revalidate Extended-exclusive behavior before merging.
- [CI run #41224890859](https://github.com/ginmp8/pandacode-extended/actions/runs/41224890859) passed 3/3 checks at `2f21bb67`, but did **not** certify native Settings restart, effective Codex MCP isolation, cloud cost/preflight, or a two-device Connect scenario.
- This register is **documentation only**. No corresponding code remediation, issue tracker items, merge, release or explicit risk acceptance was performed here.

## Maintaining this register

Keep the IDs stable. For each future fix, add the implementing PR/commit, the actual regression-test evidence, the affected release and the status change. Never mark an entry **Resolved** solely because a CI pipeline completed successfully or because the setting is visible in the interface.
