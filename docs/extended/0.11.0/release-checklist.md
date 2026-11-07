# PandaCode Extended 0.11.0 — release readiness

**Status:** Incremental documentation cycle. No 0.11.0 tag, installer, release approval, or publication.
**Opening Extended SHA:** `3eda289623850405893455e952ace45b6e0a4d89` on `base/development`.
**Opening upstream fork SHA:** `6b94bf1d8b579d08834578f6e24684ea0d3fccbd` (application 0.3.99).

## Scope and provenance

- [x] Open 0.11.0 release notes, controls, security, known issues and checklist without modifying the 0.10.0 evidence.
- [ ] Re-read latest upstream fork `main` HEAD at each incremental import; include all unreleased source commits.
- [ ] Compare **complete, nontruncated** upstream/Extended recursive trees, missing paths, modes, binaries and all shared modified hunks.
- [ ] Attribute each new behavior/change to upstream and Extended commit SHAs; separate earlier-cycle inherited features from 0.11.0-specific changes.
- [ ] Maintain a single incremental `base/development` → `base/hardened` PR; no automatic promotion into release branches.

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
