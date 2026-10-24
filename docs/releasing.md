# Local-only release packaging

This hardened distribution does not publish GitHub releases or npm packages.
The upstream `.github/workflows/release.yml` was removed because it uploaded
installers/update manifests and could publish the public npx package. Local
packaging is preserved in `scripts/release/build.mjs`.

Use `npm run release:build -- <native-target>` on the matching operating system
and CPU architecture. Supported target names remain in `scripts/release/common.mjs`.
The output goes to `release/<version>/hardened/` with a distinct hardened
filename and SHA-256 sidecar. No output is uploaded.

All local release plans pass `--locked` to Cargo, disable updater artifacts,
remove inherited update-signing secrets, and preserve platform-native signing
behavior. Windows installs per user and does not install/download WebView2.
Apple notarization, when explicitly configured, remains an external service.

`updater-manifest.mjs` refuses to write a feed; the retained signature-reading
and manifest utility exports are offline compatibility/test helpers, not a
publication path. `packages/npx` is private and its installer refuses execution.
No Windows installer was generated or scanned with Avast in this review.
See [Windows instructions](install-windows-hardened.md) and
[the current report](../SECURITY-INSTALLATION-REVIEW.md).
