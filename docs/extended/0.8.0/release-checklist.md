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

## Approval and distribution boundary
- [ ] Review the completed source changes, test results, release notes, compatibility/migration guidance and any remaining risks.
- [ ] Obtain explicit release approval.

**Prohibited by this preparation:** automatic merge, version tag, GitHub Release, npm publication, public installers, and deployment. Completion of this checklist must be evidenced by executed checks, not by declaration.
