# PandaCode Extended 0.10.0 — dependency and security evidence

**Status:** Historical remediation baseline carried forward; no new security fix, vulnerability clearance, or final 0.10.0 dependency audit is claimed.
**Opening Extended SHA:** [Extended `e8122223`](https://github.com/ginmp8/pandacode-extended/commit/e81222232314844d8fc52f65cb8e2dd37c5da2c8).
**Opening upstream source:** [source `fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852), application version **0.3.97**.

The prior Extended cycle retained npm/Rust dependency remediations and an Extended-only vendored mobile `deepmerge` implementation while matching the upstream application's source version in root npm, Cargo and Tauri manifests. See the [0.9.0 record](../0.9.0/dependency-security.md). This is a **previous-cycle observation**, not a fresh audit of every resolved dependency at the final 0.10.0 SHA.

Required before publishing:

- [ ] Reconcile upstream manifest and lockfile changes without downgrading evidence-backed Extended remediations or dropping the vendored fix.
- [ ] Check root npm, lockfile root/package entry, Cargo application and lock package, and Tauri version fields against the **current pinned upstream source**; do not replace them with `0.10.0` merely to label distribution.
- [ ] Run relevant npm audit, RustSec, dependency/build/test and mobile checks for the **final exact 0.10.0 candidate SHA**.
- [ ] Inspect actual advisories, dependency overrides and compatibility implications; record unresolved findings with version, scope, severity and mitigation.
- [ ] Verify packaging, installer/update identity and provenance independently from source-manifest version parity.

Previous CI jobs and historic audit results are evidence for their original commit only. No claim of vulnerability absence or native compatibility is made here.
