# PandaCode Extended 0.9.0 — dependency and security evidence

**Status:** inherited remediation baseline with verification gaps, not a new vulnerability remediation or a blanket security certification.

At upstream [`5ab09c5a`](https://github.com/ginmp8/pandacode/commit/5ab09c5a4da4a819b30b20771ef768866fa4ef56) and Extended [`e069ba4b`](https://github.com/ginmp8/pandacode-extended/commit/e069ba4bcf7411c1d9aab6f69c763a228141c29e), the application version fields were directly read and matched at **0.3.94** in root `package.json`, root and `packages[""]` of `package-lock.json`, Tauri `Cargo.toml`, the PandaCode package in `Cargo.lock`, and `tauri.conf.json`. This is distinct from the planned Extended distribution version **0.9.0**.

The Extended tree retains its own differing npm/Rust manifests and lockfiles and the locally vendored `mobile/vendor/extended/deepmerge` implementation. Previous dependency-remediation rationale and historical evidence remain in [0.8.0 dependency security](../0.8.0/dependency-security.md) and [0.7.0 evidence](../0.7.0/dependency-security.md). The present audit does not establish that every resulting dependency version or override was independently re-reviewed.

For exact Extended SHA [`e069ba4b`](https://github.com/ginmp8/pandacode-extended/commit/e069ba4bcf7411c1d9aab6f69c763a228141c29e), [push CI #41828235015](https://github.com/ginmp8/pandacode-extended/actions/runs/41828235015) and [PR CI #41828349624](https://github.com/ginmp8/pandacode-extended/actions/runs/41828349624) each completed successfully, including configured npm-lockfile and RustSec audit jobs. These checks do not prove universal vulnerability absence, native integration safety, or the current documentation commit's CI result.

**Still required before release approval:** review resolved lockfile/override diffs at the final code SHA, run native and mobile regressions, verify the existing vendored fix, inspect relevant advisory findings, and reconcile any subsequent upstream dependency change without downgrading documented remediations. No security fix is claimed for 0.9.0 by this file.

## 0.3.97 dependency/version checkpoint (2026-11-04)

The pinned fork source is [`fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852), declaring **0.3.97**. The root npm and Tauri/Rust application version fields were synchronized without replacing the Extended npm/Cargo dependency remediation set. Source-changed lockfile version entries were reconciled rather than wholesale replacing Extended locks; the mobile vendored dependency remains Extended-only. No new vulnerability correction or clean audit is claimed. Npm/RustSec CI for the eventual exact commit SHA and native compatibility remain unverified.
