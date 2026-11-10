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


## Upstream 0.3.106 source checkpoint — 2026-11-09

- [x] Freeze [fork main `0bfd5738`](https://github.com/ginmp8/pandacode/commit/0bfd57383621d5aa001d3871018920855cabec39), tree `391951e2d59e81946195ed2d66281991b33294a0`, application version **0.3.106**; compare to the previous fork source pin `f28c4df0` across all ten new commits.
- [x] Reconcile all 34 upstream-changed paths (26 verbatim source files, eight uniquely applicable three-way patches) and preserve the original `releases/0.3.106.md`, Extended-only files, configurable controls and dependency remediation.
- [x] Inspect complete nontruncated candidate trees: **1,567 source paths / 1,624 Extended**, **1,487 exact**, **80 divergent shared**, **57 Extended-only**, **zero missing**, **no mode mismatches**.
- [ ] Complete semantic hunk review and behavioral acceptance of all 80 shared divergences; validate task tools across harnesses/team leads/workers and all Extended control On/Off paths.
- [ ] Check exact final Extended publication SHA against npm/Node/website, Rust check/test/Clippy, RustSec, mobile/Expo, ONNX and release tests. A staged candidate tree and previous green CI do **not** count as final CI success.
- [ ] Execute native Windows/macOS/Linux, two-device Connect, authenticated MCP/cloud/VM/credential, RTL/IME and browser scroll/inspector tests. They were **not** run locally due to inaccessible GitHub DNS.
- [ ] Keep **Extended 0.12.0** a documentation-only planned distribution until separate release authorization, independent of upstream app version **0.3.106**.

## Exact-SHA CI security finding — 2026-11-09

- [x] Inspect exact-SHA [push #42644925066](https://github.com/ginmp8/pandacode-extended/actions/runs/42644925066) and [PR #42644937837](https://github.com/ginmp8/pandacode-extended/actions/runs/42644937837) for [`34d8fb4c`](https://github.com/ginmp8/pandacode-extended/commit/34d8fb4caa985f2ceb4815945dc58ed3042c8353): **five of six jobs passed in each run; RustSec failed in both**. These are **not green** runs.
- [x] Identify the inherited, **bundled** `libsqlite3-sys 0.36.0` vulnerability [RUSTSEC-2026-0459](https://github.com/RustSec/advisory-db/blob/main/crates/libsqlite3-sys/RUSTSEC-2026-0459.md), exposed through `rusqlite 0.38.0` in the pinned Cargo lockfile. Record the 6.8-severity finding without suppressing audit.
- [ ] Upgrade the compatible `rusqlite` graph to a release supporting patched `libsqlite3-sys >=0.39.0`, regenerate `Cargo.lock` with Cargo, verify Rust >=1.89/toolchain compatibility, and test SQLite storage/migration behavior; then rerun locked Rust build/tests and RustSec on the **new SHA**.
- [ ] Verify CI for the documentation-only follow-up SHA separately; exact prior failures do not become passing merely because notes were updated.
- [ ] Complete 80 shared-path semantic reconciliations and native Settings, Connect, MCP, bots, cloud/VM, credentials, cross-platform installation and release checks. No 0.12.0 publication approval is implied.

## Post-0.3.106 upstream HEAD synchronization — 2026-11-09

- [x] Pin source [`7cadfdbeb7006573897a16e1d9c15ac0a59febad`](https://github.com/ginmp8/pandacode/commit/7cadfdbeb7006573897a16e1d9c15ac0a59febad), app version **0.3.106**, and identify the sole post-release source file modification.
- [x] Mirror `scripts/release/sbom.test.mjs` exactly (CRLF and workflow section parsing) and preserve the Extended controls and dependency overrides.
- [x] Compare nontruncated Git trees: 1,567 source / 1,624 Extended, 1,487 exact shared, 80 divergent shared, 57 Extended-only, zero missing or mode mismatches.
- [ ] Execute the release Node tests including Windows CRLF behavior; no local clone was available to run them here.
- [ ] Fix the existing RustSec failure with a Cargo-generated lockfile and compatible SQLite/rusqlite graph, then validate Rust build/tests/audit on the newly published SHA.
- [ ] Verify final SHA CI and independent Settings On/Off, Connect/MCP/cloud/VM, credential and installer workflows; **0.12.0** remains a draft, not a release.

## SQLite dependency remediation candidate — 2026-11-09

- [x] Verify advisory [RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459.html) against the original bundled SQLite graph.
- [x] Port the existing, CI-checked `rusqlite 0.41.1` / `libsqlite3-sys 0.39.1` graph from the read-only [`extended/development` commit `d0100f72`](https://github.com/ginmp8/pandacode-extended/commit/d0100f72c25e3ccff58c294d624505af8a216b48), retaining app version `0.3.106` and all 686 package records.
- [ ] Inspect RustSec and `cargo test --locked`/Clippy jobs on the **new exact base/development SHA**, not just the source branch's [CI #42651770546](https://github.com/ginmp8/pandacode-extended/actions/runs/42651770546).
- [ ] Verify native SQLite storage, upgrades/migrations, rollback and Rust >=1.89 compatibility on the supported platforms; the local environment has no Cargo.
- [ ] Complete 80 shared source-file hunk reviews and Settings On/Off, Connect/MCP, bots, cloud/VM and credential runtime checks. Extended `0.12.0` remains an unapproved candidate.

## Exact-SHA CI completion evidence — 2026-11-09

- [x] Verify [push #42680289120](https://github.com/ginmp8/pandacode-extended/actions/runs/42680289120) and [PR #42680300162](https://github.com/ginmp8/pandacode-extended/actions/runs/42680300162) for code SHA [`06cb9b0c0141`](https://github.com/ginmp8/pandacode-extended/commit/06cb9b0c0141e964d43d0c462cbd30e9fc44ec1c): all six jobs passed in both runs.
- [x] Verify RustSec audit against the remediated lockfile, `cargo test --locked --lib`, Clippy with warnings denied, Node/release tests, website build, npm audits, mobile/Expo and macOS ONNX in those exact-SHA jobs.
- [ ] Independently validate native SQLite storage migrations, upgrade/rollback, installer behavior and supported Rust/platform compatibility.
- [ ] Classify the remaining 80 divergent shared files hunk by hunk and execute Settings On/Off, Connect/MCP, bots, cloud/VM and credential integrations end to end.
- [ ] Approve Extended `0.12.0` release separately; passing CI does not authorize tagging or distribution.

The earlier unchecked CI items were authored while the remediation jobs were pending. They are superseded only for the verified code SHA above. Any new documentation commit has its own CI identity.

## Upstream 0.3.107 Skills manager checkpoint — 2026-11-10

- [x] Pin upstream [`5998a8fc085e`](https://github.com/ginmp8/pandacode/commit/5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1) and reconcile all 92 changed tracked paths in a complete nontruncated candidate tree (75 source-exact, 15 adapted, two deleted), with zero missing upstream files and retained Extended-only files.
- [x] Align the six npm/Cargo/Tauri application version fields to 0.3.107 without changing the planned Extended distribution 0.12.0.
- [x] Verify the upstream 0.3.107 code commit as a one-parent update on `base/development`, followed by exact-source/destination ref readback (see follow-up below).
- [x] Verify all six existing GitHub Actions jobs on each exact-code-SHA push/PR run; live native Skills CLI, platform integration and Settings On/Off acceptance remain unverified (see follow-up below).
- [ ] Verify new Skills controls coexist with Feature controls, Connect/MCP/cloud/VM/credentials/bot pause and legacy migrated settings, without disabling upstream behavior.

## Verification follow-up on the published code SHA — 2026-11-10

- [x] Confirm the imported code commit [`e4e5c0c51daa`](https://github.com/ginmp8/pandacode-extended/commit/e4e5c0c51daa8a0149015bb26a00c8fb09bb2050) is a single-parent child of `590f7cc40f860669266414226c408bb8a1667349`; verify live source and destination HEADs and all tracked paths by nontruncated Git tree SHA/mode inventory.
- [x] Verify the two exact-code-SHA GitHub Actions runs: [push #42805588366](https://github.com/ginmp8/pandacode-extended/actions/runs/42805588366) and [PR #42805595388](https://github.com/ginmp8/pandacode-extended/actions/runs/42805595388), each with six successful jobs and no failed/skipped jobs.
- [ ] Independently exercise live Skills CLI, Windows/Linux installation, SQLite migrations/rollback, Settings On/Off, Connect/MCP/bots/cloud/VM/credentials and review all 80 customized shared-file hunks.

Passing CI does not constitute Extended 0.12.0 release approval. Any subsequent documentation commit has its own SHA and must not inherit code-commit CI as a claim of exact-SHA CI success.

## Upstream 0.3.108 voice-dictation integration — 2026-11-10

- Source: [`ginmp8/pandacode:main` `952d0acbe02547af036ef6d7635ec415a472eb3b`](https://github.com/ginmp8/pandacode/commit/952d0acbe02547af036ef6d7635ec415a472eb3b), tree `202f77088372d261b263b806354d84ce094f25b8`, native app version **0.3.108**; includes commits after the source's release commit. The planned Extended distribution documentation remains **0.12.0 DRAFT**.
- Reconciled the 109-path source delta relative to `5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1`: 78 additions (including Rust/frontend audio input, model catalog, tests and notices), 17 exact source updates and 14 changes reconciled with Extended behavior. The inherited voice feature supports optional on-device speech models and configured transcription services, transcription into the composer, explicit recording/stop and tests. This is not an Extended-only invention.
- Preserved Extended Feature controls, release identity and dependency remediation, including `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`. Native microphone permissions, service credentials, model downloads, user-cancelled recording, offline transcription and Settings states need separate behavior checks.
- This section describes a source integration **candidate**, not evidence of a successful new-SHA CI run. Check the eventual exact commit SHA for Rust/Linux/Windows/macOS, RustSec, Node, npm, Expo, installer/platform and native feature-control acceptance. Historic successful CI applies only to the earlier SHAs.

- [ ] Confirm exact-SHA CI on the 0.3.108 voice integration commit, including every new platform Clippy matrix row.
- [ ] Exercise microphone permission/denial, local model download/test/transcribe, configured provider transcription, stop/cancel/exit and composer insertion without sending, and Extended Feature controls On/Off independently.


## Post-0.3.108 source checkpoint — 2026-11-10

- [x] Freeze fork [`57f0aaf7cfc35419d4a21b9823bbdf35dc73cfd1`](https://github.com/ginmp8/pandacode/commit/57f0aaf7cfc35419d4a21b9823bbdf35dc73cfd1), tree `6d7f582267471080a5752e243769a4684e1994a3`, and retain the native version `0.3.108`.
- [x] Integrate all seven changed upstream paths, six exact and one Extended-manual-publish release gate adaptation; preserve tag-only builds and disable npm publication.
- [x] Verify untruncated source Git inventory (1,696 paths), and reconcile against 1,753 Extended paths with no missing/mode mismatches.
- [ ] Verify CI on the **published destination SHA** and distinguish pending, skipped, successful and failed jobs.
- [ ] Verify native Voice, Settings On/Off, Connect/MCP/bots/cloud/VM, credentials, SQLite migration and installer behavior; classify 80 longstanding shared-file differences by hunk.

## Post-0.3.108 source follow-up — 2026-11-10

- [x] Freeze source `8682acaf97dcd57217087d07deaa21e1dc6bb751`, tree `9405c4f98758a73fb559066bda62acd920fe75fd`, version **0.3.108**, Extended planned documentation **0.12.0**.
- [x] Reconcile four upstream paths: three exact, one manual-release guard adaptation; preserve no automatic publish/npm.
- [ ] Verify CI of the newly published destination SHA (prior `f0c1cf82ff3261845eb2cd004088cc912cf4bf8f` was green in 11/11 push and 11/11 PR jobs).
- [ ] Exercise release gate for green/pending/failed/cancelled/PR-only/non-main CI without publishing a release.
- [ ] Verify native Settings, Voice, Connect/MCP/bots/cloud/VM/credentials, SQLite migrations, installers and 80 historic shared-file divergences.
