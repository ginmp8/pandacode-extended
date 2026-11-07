# PandaCode Extended 0.11.0 — release readiness

**Status (2026-11-07):** 0.11.0 release notes finalized for source promotion. The tag, GitHub Release and installers remain unpublished; native acceptance requirements are open.
**Opening Extended SHA:** `3eda289623850405893455e952ace45b6e0a4d89` on `base/development`.
**Opening upstream fork SHA:** `6b94bf1d8b579d08834578f6e24684ea0d3fccbd` (application 0.3.99).

## Scope and provenance

- [x] Open 0.11.0 release notes, controls, security, known issues and checklist without modifying the 0.10.0 evidence.
- [ ] Re-read latest upstream fork `main` HEAD at each incremental import; include all unreleased source commits.
- [ ] Compare **complete, nontruncated** upstream/Extended recursive trees, missing paths, modes, binaries and all shared modified hunks.
- [ ] Attribute each new behavior/change to upstream and Extended commit SHAs; separate earlier-cycle inherited features from 0.11.0-specific changes.
- [ ] Keep synchronization changes limited to direct, SHA-verified commits on `base/development`; PR management and release promotion are outside the synchronization workflow.

## Validation on final exact candidate SHA

- [ ] Node/TypeScript tests, typecheck, website/UI/build, Rust check/test/Clippy, RustSec, npm audits, mobile/Expo, platform-specific and release script checks.
- [ ] Verify CI runs and individual required jobs at the exact final source SHA, including failed, skipped and pending statuses.
- [ ] Exercise actual installed Windows/macOS/Linux behavior, Settings On/Off, bots, harness choice, queue state, Connect/MCP/cloud/VM and credential approvals.
- [ ] Exercise real existing SQLite data upgrades, rollback/recovery, backups and interoperability as relevant.
- [ ] Verify security fixes and independent Extended distribution identity; retain upstream application-version parity.

## Release approval

- [ ] Resolve independently tracked issues affecting acceptance and review the 0.11.0 known-issues register.
- [ ] Verify distinct Extended release tags against upstream application versions using `scripts/release/resolve-release-identity.mjs`, including the regression for [failed v0.10.0 run #42220490913](https://github.com/ginmp8/pandacode-extended/actions/runs/42220490913).
- [ ] Verify packaging/version/tag, artifact names, updater, signing, checksums, SBOM and publication gating without changing upstream app versions to 0.11.0.
- [ ] Confirm tag pushes produce build artifacts only; GitHub publishing needs explicit manual approval and **no npm publishing job exists**.
- [ ] Obtain explicit approval before tag, GitHub Release, installer publication or deployment.

## Initial evidence log

| Evidence | Result / limitation |
| --- | --- |
| Historical 0.10.0 candidate `3eda289623850405893455e952ace45b6e0a4d89` | [CI #42217229065](https://github.com/ginmp8/pandacode-extended/actions/runs/42217229065): success on the original code tree; does **not** validate a future 0.11.0 candidate |
| 0.10.0 main promotion | [PR #114](https://github.com/ginmp8/pandacode-extended/pull/114) and [linear-rebase alternative #115](https://github.com/ginmp8/pandacode-extended/pull/115) at cycle opening |
| First 0.11.0 documentation commit | This record; inspect actual published SHA after the commit |
| 0.11.0 new upstream delta and final acceptance | Not yet collected |

## 2026-11-06 — integrated upstream 0.3.100 evidence

| Evidence | Verified observation |
| --- | --- |
| Source | [`ginmp8/pandacode:main` `e0b65e51`](https://github.com/ginmp8/pandacode/commit/e0b65e51c4b6bc9c67982eaed08f7f13985fa01a), 0.3.100, recursive source tree `5254ba8539430d29f4128318206321e6060c530d`, not truncated |
| Code integration | [`a28ce807`](https://github.com/ginmp8/pandacode-extended/commit/a28ce807a9bcc3a8583440ff6035deb1fd0854da), parent `5c38850e5f289e8b159d605cb386fec140db2d97`; 7 exact added blobs + 18 patched shared files; 25 path updates total |
| Whole-tree parity | Proposed code tree `0fef289e712f9d8616ccfd54fc5518a1974693ff`: 1,520 source paths, 1,569 Extended paths, 0 missing, 71 shared divergent, 49 Extended-only; all modes preserved. Differences still require complete hunk/behavior classification |
| Application version | Root npm, npm lock root/package entry, Cargo manifest, Cargo.lock app package and Tauri config all verified as **0.3.100**; Extended Rust ZIP remains **9.0.5** |
| Source functionality | Claude/Codex account pooling, usage-limit rollover, account sign-in, UI and tests imported from upstream; independently validated native behavior **pending** |
| CI for imported code checkpoint `7fbcb5582e835a0ea3c607cb6ba3be8790f01117` | [Push CI #42223802536](https://github.com/ginmp8/pandacode-extended/actions/runs/42223802536) and [PR CI #42223813242](https://github.com/ginmp8/pandacode-extended/actions/runs/42223813242) both completed successfully (6/6 jobs each): TypeScript/unit/website, Rust check/tests, RustSec, npm audit, mobile/Expo and ONNX/macOS. This validates that SHA only; native account-pool/Settings behavior, upgrades and a complete 0.11.0 release build are not certified. |

This is a draft readiness record only. An imported code tree and a green CI run, if one is later recorded, do not alone approve Extended 0.11.0 for distribution.

## 2026-11-06 — upstream 0.3.101 source and integration checkpoint

| Evidence | Verified observation / outstanding gate |
| --- | --- |
| Source | [`008017de3316aea56af9294d4d3f8556b52174bb`](https://github.com/ginmp8/pandacode/commit/008017de3316aea56af9294d4d3f8556b52174bb), application 0.3.101, source tree `170cbf5939773846c2fc5e02f8d3a4fd831a8e7a` |
| Code import | [`d7cf15467bf8f25563db253e7b35a1efeb2e5b5e`](https://github.com/ginmp8/pandacode-extended/commit/d7cf15467bf8f25563db253e7b35a1efeb2e5b5e), parent `92c8b116498dba4cf2bdbc5ecb3de9858f12b876`; 3 added + 24 patched source paths |
| Recursive tree | Source 1,523 blobs, Extended 1,574 blobs; zero missing, 1,443 exact shared, 80 differing shared, 51 Extended-only; no mode divergence. The remaining 80 shared differences require hunk-level classification and functional verification. |
| Integration CI | [`#42325893564`](https://github.com/ginmp8/pandacode-extended/actions/runs/42325893564) was **cancelled**. Exact-SHA [push #42325973460](https://github.com/ginmp8/pandacode-extended/actions/runs/42325973460) and [PR #42325979123](https://github.com/ginmp8/pandacode-extended/actions/runs/42325979123) for `76202bc5b029817c9b82dc9833f9627871aaa445` **failed** in release identity tests because the baseline JSON remained at 0.3.100; all five other jobs per run succeeded. The baseline now pins 0.3.101, and the corrective commit requires its own exact-SHA CI readback. |
| Native gates | Real Claude/Codex account-pool identity/usage, multi-login, restart, Settings On/Off, mobile/native packaging, Connect/MCP/cloud and credential paths remain unverified. |

A green CI run, if confirmed, does not independently approve the 0.11.0 release.

## 2026-11-07 — verification after 0.3.101 baseline correction

- Pinned fork: [`008017de3316aea56af9294d4d3f8556b52174bb`](https://github.com/ginmp8/pandacode/commit/008017de3316aea56af9294d4d3f8556b52174bb), application 0.3.101. Previous Extended baseline-correction commit: [`b7bd4af0`](https://github.com/ginmp8/pandacode-extended/commit/b7bd4af07b02bf2cb783b8fd1b6fd6025866c04e). The subsequent [`d38df20e`](https://github.com/ginmp8/pandacode-extended/commit/d38df20e316724a495aad7ccfdee8dc0f66f1185) is an Extended bot-settings adjustment, not a newer upstream import.
- Nontruncated recursive trees at `d38df20e316724a495aad7ccfdee8dc0f66f1185`: 1523 upstream blobs, 1574 Extended blobs, 1442 identical shared, 81 modified shared, 51 Extended-only, 0 missing upstream paths and 0 mode differences. Divergent code still needs hunk-level attribution and runtime acceptance.
- [Push CI #42327608080](https://github.com/ginmp8/pandacode-extended/actions/runs/42327608080) and [PR-triggered CI #42327611710](https://github.com/ginmp8/pandacode-extended/actions/runs/42327611710) both completed successfully with **6/6 jobs** at `b7bd4af0`. [Release workflow #42328625919](https://github.com/ginmp8/pandacode-extended/actions/runs/42328625919) passed its six OS/architecture build jobs, prepare and SBOM, with publication **skipped** at that same SHA. The newer `d38df20e` CI [#42332206411](https://github.com/ginmp8/pandacode-extended/actions/runs/42332206411) and [#42332211752](https://github.com/ginmp8/pandacode-extended/actions/runs/42332211752) were **in progress** at inspection; documentation-only commits require their own exact-SHA check.
- No native Settings On/Off, real Claude/Codex account, Connect, cloud/VM, installed-app or full divergent-hunk verification was performed. Green CI and zero missing source paths do **not** establish functional parity or approve Extended 0.11.0.

## 2026-11-07 — unreleased upstream mobile source checkpoint

- [x] Read source [`61b03bf2dc9f83b645088273af0b4dee05563102`](https://github.com/ginmp8/pandacode/commit/61b03bf2dc9f83b645088273af0b4dee05563102) and destination `190d61d9e168db2271c7c90f312f2146a2fc7b2d` at complete nontruncated recursive tree snapshots. Upstream application version stays 0.3.101; Extended planned documentation version stays 0.11.0.
- [x] Stage source delta: 65 added, 71 modified (3 reconciled with Extended), 47 deleted; exact source modes and binary splash hashes retained; 0 source paths missing, 0 mode mismatches, 80 differing shared files and 51 Extended-only paths.
- [ ] Complete hunk-level assessment of 77 other differing shared files, native app/iOS/Android tests and Extended control On/Off acceptance.
- [ ] Verify exact-SHA CI Node/TS, npm/RustSec, Expo/mobile, platform builds and workflow gating for the integration commit. **Pending publication and CI readback.** No release approval.

## 2026-11-07 — upstream iPhone TestFlight README update

- [x] Verify complete nontruncated source/Extended trees and pin source commit [`13a7cb68`](https://github.com/ginmp8/pandacode/commit/13a7cb68583cf77ac6c4ec351abc3ebc020ff62a) (app 0.3.101).
- [x] Copy the only new upstream `README.md` blob exactly, including the TestFlight beta URL, and retain all Extended-only paths and prior dependency controls.
- [ ] Independently test the live TestFlight enrollment, iOS installation, device authorization, Connect and push notifications.
- [ ] Confirm CI against the exact newly published Extended commit (prior `ab977cfa` push/PR runs passed, but do not validate the new commit).
- [ ] Finish 80 pre-existing shared-file hunk justifications and native Settings On/Off acceptance.
