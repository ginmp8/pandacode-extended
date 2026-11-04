# PandaCode Extended 0.9.0 — feature controls

**Status:** 0.9.0 development candidate. Source-inventory evidence only; runtime On/Off acceptance is outstanding.

**Source snapshot:** [ginmp8/pandacode main at `5ab09c5a`](https://github.com/ginmp8/pandacode/commit/5ab09c5a4da4a819b30b20771ef768866fa4ef56), application version `0.3.94`.
**Extended evidence baseline:** [`e069ba4b`](https://github.com/ginmp8/pandacode-extended/commit/e069ba4bcf7411c1d9aab6f69c763a228141c29e).

The 0.9.0 cycle inherits the existing Extended user-configurable behavior described in the [0.8.0 feature-control record](../0.8.0/feature-controls.md) and governed by the [parity contract](../upstream/parity-contract.md). These are obligations to preserve, **not** freshly certified behavior for this candidate.

| Existing control | Required permitted path | Required restricted path |
| --- | --- | --- |
| App/agent automatic update preferences | Manual checks remain available; enabled automatic checks work | Disable only the configured automatic operation |
| Connect master/direct route settings | Authenticated relay and explicitly allowed direct access remain usable | Block precisely the disabled access channel, including restart |
| Global bot pause | Unpaused, configured bots and schedules resume | Paused bots cannot perform deferred work; configurations remain intact |
| External MCP server access | Permitted newly launched Claude/Codex processes retain supported tools | Suppress only disallowed external MCP exposure for relevant new processes |
| Cloud-credential transfer | Explicitly enabled transfer can work with normal approvals | No new automatic credential transfer when disabled |
| Agent-driven cloud/VM operations | Explicitly enabled agent operations remain available | Disable agent mutations without removing manual/read-only capabilities |

**Unverified 0.9.0 scenarios:** On/Off, manual/automatic, process restart and already-running process scope; real two-device Connect; bot pause/resume; authenticated MCP; real cloud provisioning/credential transfer and VM operation. No new control or stricter default was introduced by this documentation checkpoint.

The current tracked-tree comparison found **zero absent upstream paths**, but 65 divergent shared blobs whose full hunk-by-hunk functional reconciliation is not certified. Presence of a file or a green CI job does not prove the user-visible On behavior. See [known issues](known-issues.md) and [release checklist](release-checklist.md).

## 0.3.97 integration checkpoint (2026-11-04)

Source [`fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852) was reconciled into a staged Extended tree with no missing upstream files. Existing control paths and settings were retained, including the global bot pause, external MCP exposure, Connect restrictions, credential-transfer and agent-driven cloud/VM gates and manual update paths. Preservation by source reconciliation is **not** runtime validation: full enabled/disabled, restart and existing-agent integration cases remain untested. Upstream 0.3.97 Cline MCP configuration and provider executable settings require additional checks under the Extended MCP gate.
