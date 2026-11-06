# Extended 0.11.0 — dependency and security evidence

**Status:** New-cycle monitoring; no new 0.11.0 remediation is claimed.

The [0.10.0 security record](../0.10.0/dependency-security.md) is the preceding baseline. In particular, the Rust ZIP advisory remediation is **inherited from 0.10.0**, not a new 0.11.0 fix. Preserve existing remediated versions, compatible overrides and security checks while importing new upstream source.

## Incremental requirements

- [ ] Compare upstream lockfiles/manifests with Extended at the exact imported SHAs; preserve patched dependencies unless a verified safe update supersedes them.
- [ ] Run/inspect RustSec and npm audits on the **new final SHA**; identify advisory IDs, affected versions and exploitability evidence where known.
- [ ] Check Cargo, Node/mobile/Expo, platform/toolchain compatibility and relevant release/build workflows.
- [ ] Record any new remediation separately with before/after versions, upstream compatibility, tests and CI links.
- [ ] Keep Extended release packaging identity separate from the upstream application manifests.

| Evidence | Result |
| --- | --- |
| Opening prior cycle | [0.10.0 dependency-security](../0.10.0/dependency-security.md) |
| New 0.11.0 remediation | None verified at cycle opening |
| Final 0.11.0 audits and native acceptance | Pending |
