# PandaCode Extended 0.12.0 — release readiness checklist

**Status:** New development cycle (opened 2026-11-07). No 0.12.0 source release approval, tag, GitHub Release, installer, npm publication or deployment.

## Opening baseline

- [x] Prior 0.11.0 source promoted to `main` through rebase-only [PR #118](https://github.com/ginmp8/pandacode-extended/pull/118).
- [x] Opening `base/hardened` pinned at `642b25777b5a3d1600107ff4b9b23604ea90e8a2` with tree `8dc9dcd11ebbc2c18a8c604b4a9e6360e6af7aeb`.
- [x] Prior `main` `e0e807dd265e0ff3ee4260585a7767caa6965b40` has an identical Git tree; histories differ due to the prior rebase.
- [x] Capture the prior `base/development` tip `1a84a19a3a964dcc83dd40f326635952c6e5e04a`. Its one unpromoted change was a 0.11.0 documentation-index status marker; the 0.12.0 index replaces it.
- [x] Open fresh 0.12.0 release notes, controls, dependency/security, known-issues and readiness files without editing 0.11.0 history.
- [x] Confirm the first 0.12.0 documentation commit [`99206f1c`](https://github.com/ginmp8/pandacode-extended/commit/99206f1c0a9f1f6753227a6e33bb8df0bd6f8d45): [push CI #42394967107](https://github.com/ginmp8/pandacode-extended/actions/runs/42394967107) and [PR CI #42395013636](https://github.com/ginmp8/pandacode-extended/actions/runs/42395013636) passed 6/6 jobs each. This verifies the opening documentation commit, not the final release or native feature controls.

- [x] Verify the next 0.12.0 documentation-only SHA [`d925f820`](https://github.com/ginmp8/pandacode-extended/commit/d925f8201b3c3c46dad174d3d0ec2b7ed63899e5): [push #42399278195](https://github.com/ginmp8/pandacode-extended/actions/runs/42399278195) and [PR #42399283071](https://github.com/ginmp8/pandacode-extended/actions/runs/42399283071) both passed 6/6 jobs. This is a historical checkpoint, not approval of the next commit, native integration, or final 0.12.0 release.

## Continuous upstream synchronization

- [ ] Read the live `ginmp8/pandacode:main` SHA each time; import every relevant source change, including unreleased commits, without version ceilings.
- [ ] Verify complete nontruncated upstream/Extended recursive trees, file modes, binaries, missing paths and modified shared-file hunks.
- [ ] Preserve Extended-only features, optional controls, security remediations and data/build compatibility; retain upstream functionality.
- [ ] Record upstream/integration SHAs, code changes, exceptions, tests and limitations in the 0.12.0 release notes.

- [x] **2026-11-08 source checkpoint:** imported fork [`7134edf5`](https://github.com/ginmp8/pandacode/commit/7134edf501d31acc8bec28ff74be9d1dba171669) (app 0.3.102) as Extended [`7db3e2c4`](https://github.com/ginmp8/pandacode-extended/commit/7db3e2c4ae4c5b5dc48e2de50eed5980d247e8be): 12 upstream paths applied, including original `releases/0.3.102.md`; 0 upstream paths missing, all file modes preserved. Existing 80 modified shared paths still require full hunk-by-hunk acceptance. This checkbox verifies this source integration, **not** full functional parity.

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

CI for source integration [`7db3e2c4`](https://github.com/ginmp8/pandacode-extended/commit/7db3e2c4ae4c5b5dc48e2de50eed5980d247e8be): [push #42449497421](https://github.com/ginmp8/pandacode-extended/actions/runs/42449497421) and [PR #42449500801](https://github.com/ginmp8/pandacode-extended/actions/runs/42449500801) were **queued when first inspected**. Do not classify as passed until exact-SHA results are read. No local Node/Rust or native Settings/Connect/MCP/cloud checks were executed.

This checklist records a plan and observed historical facts; unchecked items do not constitute executed verification.

## PandaCode 0.3.103 integration checkpoint

- [x] Freeze [fork main `174f0c0b17ae`](https://github.com/ginmp8/pandacode/commit/174f0c0b17ae1b036210cbc5dddf45f0701cd8b0) and import all **41 changed paths** into [Extended `3f95db516e60`](https://github.com/ginmp8/pandacode-extended/commit/3f95db516e607cbe6fee5585e61d09ab2c118a71): 23 exact upstream replacements, 9 new files, 9 three-way adaptations. Preserve the original [release notes](../../../releases/0.3.103.md).
- [x] Reconcile source (1,551 blobs) and Extended (1,608 blobs) recursive nontruncated trees: 1,471 matching shared files, 80 pre-existing customized shared files, 57 Extended-only, zero missing source files, no type/mode mismatches.
- [x] Align the six native application-version fields to **0.3.103** without changing the planned Extended **0.12.0** documentation cycle.
- [ ] Classify all existing 80 shared divergences semantically and verify both Settings On/Off, real Council model runs, Connect/MCP/cloud/VM and native installers.
- [ ] Verify exact-SHA CI on the final documentation commit and review all jobs, including skipped or pending states. No local Node/Rust/native tests were executed here.

Council's review loop and imported test files establish source coverage, not release approval. No tag, PR, installer, release or npm publication was made.

## PandaCode 0.3.104 source checkpoint

- [x] Import all 36 upstream paths changed since the prior source pin: 10 new blobs, 14 refreshed binary screenshots and 12 patched existing textual files, preserving five Extended-customized shared files. Source: [`d5d51769`](https://github.com/ginmp8/pandacode/commit/d5d51769ccac15145e67c071ea06a5502c4cadcb); Extended integration: [`6b6a39a8`](https://github.com/ginmp8/pandacode-extended/commit/6b6a39a82f508360572518519badb56e3df8313e).
- [x] Confirm full nontruncated tree inventory (1,561 upstream / 1,618 Extended blobs, 0 missing source, 0 mode mismatches) and align all six upstream native version fields to **0.3.104** while retaining the planned Extended **0.12.0** distribution cycle.
- [x] Preserve original [0.3.104 upstream release notes](../../../releases/0.3.104.md), Git file modes, Extended-only files, dependency remediations and existing control paths.
- [ ] Complete 80 shared customized-file hunk classifications and native app control-state testing; do not use absence of missing paths as proof of functional parity.
- [ ] Verify final exact-SHA CI jobs and native installed-app/mobile/Connect/MCP/cloud/VM/credential scenarios. The integration-commit [push](https://github.com/ginmp8/pandacode-extended/actions/runs/42503275362) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/42503280072) were in progress on first inspection; local tests did not run.

## Verified CI checkpoint — 2026-11-08

- [x] Verify [push CI #42503386681](https://github.com/ginmp8/pandacode-extended/actions/runs/42503386681) and [PR CI #42503391142](https://github.com/ginmp8/pandacode-extended/actions/runs/42503391142) at **`e33fb2fd93aaa130f5e480db7d0e227fe966e1cf`**: both completed successfully with **6/6 jobs** (typecheck/Node/website, Rust check/tests, npm dependency audit, RustSec, mobile/Expo and ONNX/macOS).
- [ ] Verify CI again for any later `base/development` SHA, including this documentation-only correction; earlier green CI does not transfer to a different SHA.
- [ ] Complete all 80 shared hunk classifications and native Settings On/Off, Council, Connect/MCP/cloud/VM, credentials, and installer acceptance before claiming functional parity or approving Extended 0.12.0. Local tests were not executed during this follow-up.

## PandaCode 0.3.105 source checkpoint — 2026-11-08

- [x] Pin the exact [fork `main` SHA `f28c4df0`](https://github.com/ginmp8/pandacode/commit/f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665), tree `49077426e74a7134da8f3393a6abff32b9cc07c4`, and read native app version **0.3.105** from source manifests.
- [x] Publish [Extended integration `36228b94`](https://github.com/ginmp8/pandacode-extended/commit/36228b949aaf8648109e2f8f8da441a8735cee41) as a single-parent non-forced commit on `base/development`: 4 added upstream paths, 26 source-exact replacements, 8 adapted paths, original release notes preserved, source provenance updated; 1,565 source/1,622 candidate blobs, zero missing source paths or mode changes.
- [x] Preserve Extended-only prior customizations, dependency remediations, /btw slash-resolution test and separate 0.12.0 planned release identity while aligning six upstream application-version fields.
- [x] Recheck the code-commit CI: [push #42554247514](https://github.com/ginmp8/pandacode-extended/actions/runs/42554247514) and [PR #42554252219](https://github.com/ginmp8/pandacode-extended/actions/runs/42554252219) both finished **cancelled**, with mixed successful/cancelled jobs; neither is a green CI receipt.
- [x] Verify [documentation commit `002e433fb0bf`](https://github.com/ginmp8/pandacode-extended/commit/002e433fb0bfe5d51bdf89ed8cde5cc21ba3a4da) exact-SHA CI: [push #42554384509](https://github.com/ginmp8/pandacode-extended/actions/runs/42554384509) and [PR #42554389369](https://github.com/ginmp8/pandacode-extended/actions/runs/42554389369) each completed **successfully (6/6 jobs)**. The follow-up documentation commit needs its own CI; native functional acceptance remains unchecked.
- [ ] Independently validate account pool, identity/usage/fallback, real credentials and account-switch message persistence, and existing Extended Settings On/Off, MCP, Connect, cloud/VM, bot/channel and native platform behavior.
- [ ] Classify each remaining shared-file divergence by hunk and verify final documentation-commit CI separately. Neither source coverage nor CI is release approval; `0.12.0` is still planned.
