# Extended 0.11.0 — feature-controls continuity

**Status:** Opening baseline only. No new controls are introduced or assumed tested by this document.

Use the [0.10.0 feature-controls record](../0.10.0/feature-controls.md) and the [full upstream parity contract](../upstream/parity-contract.md) as the starting reference. For every newly imported source change, verify both settings states and preserve the actual upstream behavior when a user enables it.

The existing Extended controls include optional restrictions on automatic updates, Connect/directed remote access, bot pauses, external MCP servers, cloud-agent credential transfers and cloud/VM management. A disabled automatic or remote operation must not eliminate the corresponding upstream feature when explicitly enabled and authorized.

## Validation to maintain incrementally

- [ ] Compare the current live fork SHA and new Extended commit SHA; identify every shared customized path.
- [ ] Check settings **On** and **Off**, including effective behavior after restart, permissions/approvals and persisted chat selections.
- [ ] Validate bot chat permissions, harness selection, outbound queue drain and user-visible sending status when those flows change.
- [ ] Verify Connect, MCP, cloud/VM, credentials and other affected workflows with authorized/native test evidence.
- [ ] Record real regressions and fixes with commit/CI links; do not disable features to hide failures.

No 0.11.0 functional acceptance evidence is established merely by opening this cycle.

## Upstream 0.3.100 account pool integration

Source: [`e0b65e51`](https://github.com/ginmp8/pandacode/commit/e0b65e51c4b6bc9c67982eaed08f7f13985fa01a); imported code: [`a28ce807`](https://github.com/ginmp8/pandacode-extended/commit/a28ce807a9bcc3a8583440ff6035deb1fd0854da). The provider account-pool option and one-click CLI sign-in are inherited upstream features, **not** new Extended-only security controls. The original provider toggle and account selection must remain functional while the existing Extended controls independently govern their own operations. Static import preserved the separate Extended control paths and dependency remediations; real On/Off, restart, multi-account and queued-message acceptance is still pending.

## PandaCode 0.3.101 compatibility checkpoint

The upstream [0.3.101 import](https://github.com/ginmp8/pandacode-extended/commit/d7cf15467bf8f25563db253e7b35a1efeb2e5b5e) patched 24 shared files and added three upstream source files. Existing Extended control modules were retained in the resulting Git tree. Static hunk application does **not** demonstrate that On/Off settings or delegated accounts work at runtime. Test real-account sign-in/refresh, account-pool eligibility and usage display alongside optional bot/Connect/MCP/cloud controls before acceptance.
