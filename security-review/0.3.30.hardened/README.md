# PandaCode 0.3.30.hardened — source-release correction

**Public distribution:** `0.3.30.hardened`  
**Cargo/npm/Tauri version:** `0.3.30-hardened` (SemVer-compatible prerelease identifier)  
**Source lineage:** `0.3.28-hardened.3` → `0.3.28-hardened.4` → `0.3.30.hardened`  
**Previous candidate SHA-256:** `9807b8958c00aeaf7ef1d07345878673ab94a669c248b19c793de11c16c5e977`  
**Compared upstream archive:** `pandacode-0.3.30.zip` (`893e571015787704584a8c6dae21b1e9188fa396192c496bb17891ffe91ebb00`)  
**Date:** 2026-10-23

## Meaning of this version

The previous candidate was labeled `0.3.28-hardened.4` even though it selectively backported features from upstream 0.3.30. This package corrects the **distributed version identity**, app About label, version manifests, local installer naming and current documentation. **It is not a full rebase to upstream 0.3.30 and does not include every upstream feature.** The original secure code lineage remains 0.3.28 hardened; the original hardening evidence is preserved, with its earlier candidate bytes and hashes, under `security-review/hardened.1` through `hardened.4`.

## Included 0.3.30 improvements (unchanged from previous candidate)

- Oh My Pi RPC v2 negotiated bounded model catalog discovery, with strict framing, size and timeout limits.
- Inspector open/closed preference retained when creating a new task.
- Sanitized model discovery errors rather than raw provider diagnostics.

The native RPC, inspector and model picker implementation bytes, along with existing tests, are unchanged from the previously supplied `0.3.28-hardened.4` candidate. No additional upstream features were merged for this release-label correction.

## Security contract and compatibility

- Keychain-backed Connect secrets; durable replay protection and trust rechecks retained.
- CSP, controlled browser navigation, bot/tool permissions, disabled automatic updates, and native installer policy retained.
- Rust third-party dependency versions and vendor GLib backport unchanged; only the root `pandacode` package version entry in `Cargo.lock` changed.
- Native bundle identifier `ai.pandacode.desktop` unchanged; a prior installation may share local data. **Back up data before upgrading.** No compatibility or migration run was performed.
- Local packaging now uses `release/0.3.30-hardened/hardened/` and the `PandaCode-0.3.30.hardened-<platform>` asset label. No artifact is uploaded or auto-installed.

## Verification

- **Executed:** ZIP CRC/integrity, path and member bounds, cross-manifest version checks, exact modified/additional file allowlist, byte-preservation checks for feature and security modules, file-hash manifest, static parser/source checks where available.
- **Not executed:** Rust build/test, TypeScript typecheck/bundle, Playwright/browser tests, Oh My Pi integration tests, native installer execution and AV/signature verification, upgrade or data migration.
- The `security-review/hardened.4/` report is **historical**, not a statement that this version's native binaries passed. The prior source baseline's scan/test findings cannot be automatically promoted to this version.
- The dotted distribution label `0.3.30.hardened` is **not valid SemVer**. Use `0.3.30-hardened` for package managers, Tauri and Cargo and `0.3.30.hardened` for the release ZIP and display text.

This is a fork of PandaCode — https://github.com/aicodeking/pandacode. The original license and attribution remain unchanged.
