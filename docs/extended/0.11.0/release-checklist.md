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
