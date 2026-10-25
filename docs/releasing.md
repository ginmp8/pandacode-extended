# PandaCode Extended: local-only release packaging

PandaCode Extended does not automatically publish GitHub Releases or npm packages. The upstream release workflow was removed because it could upload installers/updater manifests and publish a network-installer package. The reviewed local packaging path remains in `scripts/release/build.mjs`.

See [versioning policy](VERSIONING.md). Technical application version and local folder use independent Extended SemVer `X.Y.Z` (e.g. `0.4.0`). The display identity `0.4.0+pc.0.3.42` records the tracked upstream reference but is **not** used to order releases or to name native build folders.

Use `npm run release:build -- <native-target>` on a matching operating system and architecture; supported targets are in `scripts/release/common.mjs`. Output goes to `release/<X.Y.Z>/extended/` under a distinct `PandaCode-Extended-<X.Y.Z>-...` filename with a SHA-256 sidecar. The output is local-only; nothing is uploaded automatically. App bundle filenames are derived from the Tauri product name **PandaCode Extended** while the main executable/internal crate remains named `pandacode`.

Build plans pass Cargo `--locked`, disable updater artifacts and inherited update-signing secrets, preserve platform-native signing behavior and keep the existing restricted update/installer policy. Windows NSIS installs per-user and requires a preinstalled WebView2 runtime rather than downloading it. Apple notarization, if explicitly configured, remains an external service.

`updater-manifest.mjs` refuses to write an update feed; retained manifest helpers are offline compatibility/test utilities. `packages/npx` remains private and refuses network bootstrap execution.

**Validation:** Native installers require checks on the actual target OS, including a successful bundle and installation smoke test. No Windows installer or malware scanner result is certified by this source-only release note. See [Windows instructions](install-windows-hardened.md) and [installation security review](../SECURITY-INSTALLATION-REVIEW.md).
