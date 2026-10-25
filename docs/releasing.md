# PandaCode Extended: local-only release packaging

PandaCode Extended does not automatically publish GitHub Releases or npm packages. The upstream release workflow was removed because it could upload installers/updater manifests and publish a network-installer package. The reviewed local packaging path remains in `scripts/release/build.mjs`.

See [versioning policy](VERSIONING.md). The current candidate uses technical version `0.5.0` and display identity `0.5.0+pc.0.3.47`. Technical application versions and local folders use independent Extended SemVer `X.Y.Z`; the display suffix records the tracked upstream reference and is not used to order releases or name native build folders. See [the integration coverage](upstream-integration-0.5.0.md) for the source identities, implemented release range and validation requirements.

Use `npm run release:build -- <native-target>` on a matching operating system and architecture; supported targets are in `scripts/release/common.mjs`. Output goes to `release/<X.Y.Z>/extended/` under a distinct `PandaCode-Extended-<X.Y.Z>-...` filename with a SHA-256 sidecar. The output is local-only; nothing is uploaded automatically. App bundle filenames are derived from the Tauri product name **PandaCode Extended** while the main executable/internal crate remains named `pandacode`.

Build plans pass Cargo `--locked`, disable updater artifacts and inherited update-signing secrets, preserve platform-native signing behavior and keep the existing restricted update/installer policy. Windows NSIS installs per-user and requires a preinstalled WebView2 runtime rather than downloading it. Apple notarization, if explicitly configured, remains an external service.

`updater-manifest.mjs` refuses to write an update feed; retained manifest helpers are offline compatibility/test utilities. `packages/npx` remains private and refuses network bootstrap execution.

**Validation:** Native installers require checks on the actual target OS, including a successful bundle and installation smoke test. No Windows installer or malware scanner result is certified by this source-only release note. See [Windows instructions](install-windows-hardened.md) and [installation security review](../SECURITY-INSTALLATION-REVIEW.md).

## Explicit offline inventory

After producing a local installer, an operator with **already installed Syft and Grype** and an existing local Grype database can run:

```sh
node scripts/release/sbom.mjs release/0.5.0/extended release/0.5.0/sbom 0.5.0 <full-source-commit>
```

The output directory must be new. The helper selects only version-matching Extended installers, records their hashes, and copies the exact npm and Cargo lockfiles into isolated source-inventory directories. It writes CycloneDX inventories, successful Grype JSON reports and `SBOM-COVERAGE.json`/`.md`. No updater archive is selected. Source dependency inventories are not proof of installer contents.

The explicit scanner policy overrides inherited scanner options: app/database updates and remote enrichment are disabled. It uses the existing database location (including an explicit `GRYPE_DB_CACHE_DIR`) and retains database-age validation. Missing tools, malformed/empty output, unavailable/stale databases and unsupported installer formats remain incomplete or failed; the helper does not install/update anything to repair them. Exit 2 means incomplete coverage; exit 1 means an invocation/setup error. Finding counts do not certify safety or authorize publication. Review the full reports separately.

Policy references: [Syft configuration](https://oss.anchore.com/docs/reference/syft/configuration/) and [Grype configuration](https://oss.anchore.com/docs/reference/grype/configuration/). Scanner versions and database age are recorded per run. No real installer scan is implied by unit tests of the report helper.
