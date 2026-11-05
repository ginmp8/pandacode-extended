# PandaCode Extended 0.10.0 — dependency and security evidence

**Status:** Targeted ZIP patch verified by exact-commit GitHub CI, including Rust/Linux, RustSec, npm audit, mobile/Expo, TypeScript and macOS ONNX; focused VSIX runtime and native release acceptance remain pending.
**Opening Extended SHA:** [Extended `e8122223`](https://github.com/ginmp8/pandacode-extended/commit/e81222232314844d8fc52f65cb8e2dd37c5da2c8).
**Opening upstream source:** [source `fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852), application version **0.3.97**.

The prior Extended cycle retained npm/Rust dependency remediations and an Extended-only vendored mobile `deepmerge` implementation while matching the upstream application's source version in root npm, Cargo and Tauri manifests. See the [0.9.0 record](../0.9.0/dependency-security.md). This is a **previous-cycle observation**, not a fresh audit of every resolved dependency at the final 0.10.0 SHA.

## Targeted Rust ZIP security remediation (2026-11-05)

- **Finding:** [GHSA-m665-v7pm-7grj / CVE-2026-115905](https://github.com/advisories/GHSA-m665-v7pm-7grj) classifies Rust `zip <= 9.0.4` as affected (published critical severity); the first patched release is `9.0.5`. This is **package-level** severity, not a proven RCE path in PandaCode Extended.
- **Minimal change:** pin the direct `zip` dependency in `src-tauri/Cargo.toml` from `=9.0.4` to **`=9.0.5`** and align the resolved entry in `src-tauri/Cargo.lock`. The checksum `6e04976da629d3b04204ba8105b396a193af8ad2583b2c79bfb3ab8af580a11e` is recorded in the [official crates.io index](https://github.com/rust-lang/crates.io-index/blob/master/3/z/zip) for `9.0.5`.
- **Compatibility:** compare `zip-rs/zip2` [9.0.4](https://github.com/zip-rs/zip2/blob/v9.0.4/Cargo.toml) with [9.0.5](https://github.com/zip-rs/zip2/blob/v9.0.5/Cargo.toml): same Rust minimum (1.88), selected `deflate` feature and normal dependency requirements. The change only updates the ZIP version and checksum; no transitive dependency, updater feature, API call or local mobile `deepmerge` fork is deliberately changed.
- **Reachability:** the Extended `src-tauri/src/vsix_themes.rs` parser uses `zip::ZipArchive::new` and `by_name` for VSIX themes; no direct `ZipStreamReader::extract` call was found in that implementation. Transitive uses and native packaging require separate verification. An affected package is not equivalent to an exploitable application path.
- **Verification required:** static manifest/lockfile checks establish the intended pin and checksum, **not** a successful Rust build. At the final commit, run `cargo test --manifest-path src-tauri/Cargo.toml vsix_themes::tests`, `cargo check --manifest-path src-tauri/Cargo.toml --locked`, `cargo audit`/RustSec, and applicable CI platform checks; record exact-SHA results before claiming a fully verified remediation.
- **Exact-SHA CI evidence (2026-11-05):** [Extended `dcef242c`](https://github.com/ginmp8/pandacode-extended/commit/dcef242c7e201f060893a899cf04944f9f812b47) passed [Push CI 41969392506](https://github.com/ginmp8/pandacode-extended/actions/runs/41969392506) and [PR CI 41969398025](https://github.com/ginmp8/pandacode-extended/actions/runs/41969398025) (6/6 jobs each, 12/12 total). The Linux job reports Rust check and tests and the RustSec job reports success. This evidence does not establish that the focused `vsix_themes::tests` command, installed-app scenarios, or final release-platform verification ran independently.\n- **Future monitoring:** [Extended dependency issue #94](https://github.com/ginmp8/pandacode-extended/issues/94) and the existing read-only dependency monitoring task must check any new ZIP advisory and the **current main** Cargo lock entry on every run, including regressions from upstream imports. Do not assume `9.0.5` stays sufficient for future advisories.

Required before publishing:

- [ ] Reconcile upstream manifest and lockfile changes without downgrading evidence-backed Extended remediations or dropping the vendored fix.
- [ ] Check root npm, lockfile root/package entry, Cargo application and lock package, and Tauri version fields against the **current pinned upstream source**; do not replace them with `0.10.0` merely to label distribution.
- [ ] Run relevant npm audit, RustSec, dependency/build/test and mobile checks for the **final exact 0.10.0 candidate SHA**.
- [ ] Inspect actual advisories, dependency overrides and compatibility implications; record unresolved findings with version, scope, severity and mitigation.
- [ ] Verify packaging, installer/update identity and provenance independently from source-manifest version parity.

Previous CI jobs and historic audit results are evidence for their original commit only. No claim of vulnerability absence or native compatibility is made here.
