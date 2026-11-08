# PandaCode Extended 0.12.0 — release readiness checklist

**Status:** Three Extended-only source features have been integrated into `extended/development` in [draft PR #120](https://github.com/ginmp8/pandacode-extended/pull/120). No 0.12.0 release approval, tag, GitHub Release, installer, npm publication or deployment.

## Opening baseline

- [x] Prior 0.11.0 source promoted to `main` through rebase-only [PR #118](https://github.com/ginmp8/pandacode-extended/pull/118).
- [x] Opening `base/hardened` pinned at `642b25777b5a3d1600107ff4b9b23604ea90e8a2` with tree `8dc9dcd11ebbc2c18a8c604b4a9e6360e6af7aeb`.
- [x] Prior `main` `e0e807dd265e0ff3ee4260585a7767caa6965b40` has an identical Git tree; histories differ due to the prior rebase.
- [x] Capture the prior `base/development` tip `1a84a19a3a964dcc83dd40f326635952c6e5e04a`. Its one unpromoted change was a 0.11.0 documentation-index status marker; the 0.12.0 index replaces it.
- [x] Open fresh 0.12.0 release notes, controls, dependency/security, known-issues and readiness files without editing 0.11.0 history.
- [ ] Confirm the first 0.12.0 documentation commit and its exact-SHA CI after publication.

## Continuous upstream synchronization

- [ ] Read the live `ginmp8/pandacode:main` SHA each time; import every relevant source change, including unreleased commits, without version ceilings.
- [ ] Verify complete nontruncated upstream/Extended recursive trees, file modes, binaries, missing paths and modified shared-file hunks.
- [ ] Preserve Extended-only features, optional controls, security remediations and data/build compatibility; retain upstream functionality.
- [ ] Record upstream/integration SHAs, code changes, exceptions, tests and limitations in the 0.12.0 release notes.

## Extended-only feature acceptance — draft PR #120

- [ ] **#121:** Test **Bot Settings → Shared Channels** with inherited, explicit include and explicit exclude preferences; verify prior saved values, migration, restart, non-member rejection and membership/archive cleanup.
- [ ] Confirm the eight-post context boundary, unchanged stored conversations, separate fresh-context policy and no rewrite/interruption of queued or active work.
- [ ] **#122:** Test informational posts (including `@mentions`) without triggering work, explicit one/two-recipient requests, attribution, channel isolation and direct bot conversations without transcript broadcast.
- [ ] Verify membership, self/ambiguous target and cycle rejection, combined three-hop limit, retry idempotency, per-recipient state, callback updates, stop and restart recovery; verify interrupted callback recovery and avoid duplicate native admission.
- [ ] **#123:** Round-trip selected/all bots, optional native and Extended settings, local Shared Channel members/lead and overrides; retain legacy `{"bots":[...]}` compatibility and safe external-channel configuration.
- [ ] Test verified pre-import backups, backup failure-before-mutation, partial failures, no-clobber Desktop export, no-secret manifest content and the limits of configuration-only recovery.
- [ ] Exercise the Desktop local JSON/Git source UI and existing headless import/export plus `pandacode serve --sync-bots`; verify read-only SHA-pinned GitHub sync, unsafe/invalid source refusal, private-source credentials and manual-only operation.
- [ ] Run the isolated Playwright CI job for bot selection/export, malformed/successful import, Git source UI and attributed collaboration messages. Run the existing #121 history-controls browser tests separately against the integrated preview. Distinguish mocked UI fixtures from native import/queue execution.
- [ ] Cross-check the [#121 context guide](../../extended-bot-context.md), [#122 collaboration guide](../../extended-bot-collaboration.md), [#123 manifest guide](../../extended-bot-manifests.md) and release notes against the final candidate SHA.

These checks remain unchecked until directly executed. Code presence, existing unit tests and this documentation do not establish native or cross-machine acceptance.

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
| 0.12.0 native / integration CI | Pending; no runtime result implied by scaffolding |

This checklist records a plan and observed historical facts; unchecked items do not constitute executed verification.
