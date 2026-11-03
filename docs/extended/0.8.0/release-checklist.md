# PandaCode Extended 0.8.0 — release readiness checklist

**Status:** unapproved and unreleased. This is a planning checklist; unchecked entries are not acceptance evidence.

## Source and release identity
- [ ] Pin the exact final `ginmp8/pandacode:main` source HEAD and the exact Extended candidate SHA.
- [ ] Reconcile the full upstream recursive tree, including modes, binaries, release notes, workflows, tests, version manifests and locks; justify every Extended-owned divergence.
- [ ] Record an actual `0.8.0` Extended distribution identity separately from the upstream application/source version; confirm install/upgrade/package compatibility, naming and version-mismatch risks.
- [ ] Compare against the preceding accepted **Extended product release**, not merely an old unshipped documentation candidate; record the true new features and migrations.

## Build and behavior
- [ ] Pass required TypeScript/website/unit/browser, Rust test/Clippy, mobile/Expo, npm audits, RustSec, macOS ONNX and release/build checks on the final candidate.
- [ ] Validate Windows, macOS and Linux native builds/launch; verify updater and installer behavior without publishing artifacts.
- [ ] Test Extended feature controls On and Off, including security boundaries and allowed upstream operations.
- [ ] Exercise OpenCode V2 ACP model/effort selection and key providers, bots/teams, `/btw`, and real two-device Connect/machine orchestration.
- [ ] Recheck [known issues](known-issues.md), record actual severities and any necessary blocking mitigations.

## Additional source 0.3.91 acceptance cases
- [ ] Exercise Linux WebKitGTK undo/redo and Wayland clipboard-image paste on a native desktop, including large screenshot handling.
- [ ] Exercise ACP mode fallback (including a provider without Plan/Accept edits), Claude model catalogs above 256 KiB, DeepSeek slash commands and OpenCode JSONC MCP discovery.
- [ ] Exercise project-folder relinking, worktree repair and Windows conversation import on representative native platforms.
- [ ] Confirm `arboard` and its new lockfile graph compile under the Extended remediated dependency versions without loosening Settings policy.

## Explicit publication approval

- A `v*` tag push may build installer artifacts, but must not create a GitHub Release or publish to npm.
- Only a manually dispatched `Release` workflow with `publish: true` can create a GitHub Release; npm additionally requires that manual run to target a tag.
- Preparing this candidate must not dispatch publishing, create tags, or treat a successful build as release approval.

## Approval and distribution boundary
- [ ] Review the completed source changes, test results, release notes, compatibility/migration guidance and any remaining risks.
- [ ] Obtain explicit release approval.

**Prohibited by this preparation:** automatic merge, version tag, GitHub Release, npm publication, public installers, and deployment. Completion of this checklist must be evidenced by executed checks, not by declaration.

## Code-level CI checkpoint

On the source-integration commit `083e764485f08ea1b5cabbf3b3d18ea9b426e89c`, [CI #41498498966](https://github.com/ginmp8/pandacode-extended/actions/runs/41498498966) completed six green jobs. This is a **CI checkpoint**, not satisfaction of the release-readiness checklist above: native installer, real-host, GUI Settings and release approval checks remain open.

## Latest verified CI/build evidence — 2026-11-02

- Pinned fork source: [`a9be2ced7bb594ad28a09fbd204cdad34770c750`](https://github.com/ginmp8/pandacode/commit/a9be2ced7bb594ad28a09fbd204cdad34770c750), application version `0.3.91`. Extended source-integrated HEAD: [`3af810b18771485568e9199e7b603f4ef0ddf71f`](https://github.com/ginmp8/pandacode-extended/commit/3af810b18771485568e9199e7b603f4ef0ddf71f).
- [CI #41523546088](https://github.com/ginmp8/pandacode-extended/actions/runs/41523546088): completed successfully, six of six jobs (TypeScript/unit/website, Rust check/tests, npm dependency audit, RustSec, Expo/mobile, ONNX macOS).
- [Release preparation #41523436756](https://github.com/ginmp8/pandacode-extended/actions/runs/41523436756): completed successfully with eight jobs successful (prepare, SBOM and six native OS/architecture builds); two publication jobs (`npm` and `publish`) were skipped. Neither a tag nor a public release was created by this documentation update.
- Full recursive Git-tree inventories at the pinned SHAs, both `truncated=false`: 1,455 upstream blobs, 1,396 content/mode-identical, 59 differing, zero missing, plus 27 Extended-only files. The 59 differences still require complete functional acceptance, not automatic approval.
- The checks above belong to the exact `3af810b` snapshot. A later documentation-only commit has a new SHA and requires separate CI status lookup. Native GUI/installer launch, complete Settings On/Off testing and real two-device Connect were **not** executed. The `0.8.0` documentation remains an unapproved release candidate.

## Additional source 0.3.92 acceptance cases

- [ ] Launch the redesigned Settings UI in an installed app and confirm the separate Extended Feature controls section mounts, saves and reopens correctly.
- [ ] Exercise all Feature controls Off/On after redesign, including Connect sign-in, connection pause, manual/automatic paths and device pairing.
- [ ] Check live scenes in dark/light themes, imported themes, time changes, visibility suspension, reduced motion and six tiled threads.
- [ ] Verify turn cards, Read more, diff actions, active versus read-only tiles, focus navigation, approval and real agent streaming.
- [ ] Validate platform-specific native browser/GPU behavior, release workflows and no automatic publication before release approval.
