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
