# PandaCode Extended 0.12.0 — release readiness checklist

**Status:** New development cycle (opened 2026-11-07). No 0.12.0 source release approval, tag, GitHub Release, installer, npm publication or deployment.

## Opening baseline

- [x] Prior 0.11.0 source promoted to `main` through rebase-only [PR #118](https://github.com/ginmp8/pandacode-extended/pull/118).
- [x] Opening `base/hardened` pinned at `642b25777b5a3d1600107ff4b9b23604ea90e8a2` with tree `8dc9dcd11ebbc2c18a8c604b4a9e6360e6af7aeb`.
- [x] Prior `main` `e0e807dd265e0ff3ee4260585a7767caa6965b40` has an identical Git tree; histories differ due to the prior rebase.
- [x] Capture the prior `base/development` tip `1a84a19a3a964dcc83dd40f326635952c6e5e04a`. Its one unpromoted change was a 0.11.0 documentation-index status marker; the 0.12.0 index replaces it.
- [x] Open fresh 0.12.0 release notes, controls, dependency/security, known-issues and readiness files without editing 0.11.0 history.
- [x] Confirm the first 0.12.0 documentation commit [`99206f1c`](https://github.com/ginmp8/pandacode-extended/commit/99206f1c0a9f1f6753227a6e33bb8df0bd6f8d45): [push CI #42394967107](https://github.com/ginmp8/pandacode-extended/actions/runs/42394967107) and [PR CI #42395013636](https://github.com/ginmp8/pandacode-extended/actions/runs/42395013636) passed 6/6 jobs each. This verifies the opening documentation commit, not the final release or native feature controls.

## Continuous upstream synchronization

- [ ] Read the live `ginmp8/pandacode:main` SHA each time; import every relevant source change, including unreleased commits, without version ceilings.
- [ ] Verify complete nontruncated upstream/Extended recursive trees, file modes, binaries, missing paths and modified shared-file hunks.
- [ ] Preserve Extended-only features, optional controls, security remediations and data/build compatibility; retain upstream functionality.
- [ ] Record upstream/integration SHAs, code changes, exceptions, tests and limitations in the 0.12.0 release notes.

## Final exact-SHA validation

- [ ] Run CI at the exact final 0.12.0 candidate, including Node/TypeScript, release-identity tests, Rust/Cargo/Clippy, RustSec, npm audits, website, mobile/Expo, ONNX and platform-specific checks.
- [ ] Review all jobs and distinguish success, failure, skipped, cancellation and pending outcomes.
- [ ] Validate actual installed Windows/macOS/Linux builds and mobile platforms as applicable.
- [ ] Exercise real bot, harness, fresh context, channel history, queue, Settings On/Off, MCP, Connect, cloud/VM and credential permission workflows.
- [ ] Test persistent-data migrations, upgrades, signing, updater manifests and rollback/recovery on supported systems.
- [ ] Confirm native source versions match the fork; Extended distribution `0.12.0` is not written into upstream application manifests.
- [ ] Review unresolved security/functional gaps and document any release-blocking issue.

## Promotion and publication

- [ ] Promote `base/development` to `base/hardened` through **Rebase and merge** only.
- [ ] Promote the equivalent `base/hardened` source tree to `main` through **Rebase and merge** only. If direct PR is blocked by historically rebased SHAs, replay commits individually on a temporary `main`-based branch, verify identical final tree, and do not modify `base/hardened`.
- [ ] Obtain separate approval for the `v0.12.0` tag, GitHub Release and distribution. Never create merge commits or use squash.
- [ ] Verify release assets, SBOM, checksums/signatures, updater behavior and explicit publication gating. Tag push is not automatic GitHub publication; npm publication remains disabled.

## Source evidence available at cycle opening

| Evidence | Observation / limitation |
| --- | --- |
| `base/hardened` | [`642b25777b5a3d1600107ff4b9b23604ea90e8a2`](https://github.com/ginmp8/pandacode-extended/commit/642b25777b5a3d1600107ff4b9b23604ea90e8a2), 0.11.0 source |
| `main` | [`e0e807dd265e0ff3ee4260585a7767caa6965b40`](https://github.com/ginmp8/pandacode-extended/commit/e0e807dd265e0ff3ee4260585a7767caa6965b40), same Git tree |
| Prior exact-main CI | [#42392453324](https://github.com/ginmp8/pandacode-extended/actions/runs/42392453324): 6/6 passed; only validates 0.11.0 source |
| Fork at opening | [`13a7cb68583cf77ac6c4ec351abc3ebc020ff62a`](https://github.com/ginmp8/pandacode/commit/13a7cb68583cf77ac6c4ec351abc3ebc020ff62a), app `0.3.101` |
| First 0.12.0 documentation SHA CI | [`99206f1c`](https://github.com/ginmp8/pandacode-extended/commit/99206f1c0a9f1f6753227a6e33bb8df0bd6f8d45): [push #42394967107](https://github.com/ginmp8/pandacode-extended/actions/runs/42394967107) and [PR #42395013636](https://github.com/ginmp8/pandacode-extended/actions/runs/42395013636), 6/6 jobs successful each. Final exact-SHA CI and native integration acceptance remain pending. |

This checklist records a plan and observed historical facts; unchecked items do not constitute executed verification.
