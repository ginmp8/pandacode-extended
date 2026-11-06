# PandaCode Extended 0.10.0 — release readiness

**Status:** New development documentation cycle. No final 0.10.0 release, tag, installer, publication or approval.
**Initial Extended SHA:** `e81222232314844d8fc52f65cb8e2dd37c5da2c8` (`base/development`).
**Initial source SHA:** `fc32c3f585db6a72fee967e8d91f2bb50fe67852` (`ginmp8/pandacode:main`, source version `0.3.97`).

## Source and scope

- [x] Identify the starting Extended and upstream source commits and open the planned 0.10.0 documentation cycle.
- [x] Keep previous 0.9.0 documentation as a historical reference instead of rewriting it.
- [ ] Verify the latest upstream `main` HEAD and full nontruncated trees against the **final 0.10.0 candidate**; copy all tracked source content, including unreleased commits, or document unresolved blockers.
- [ ] Review each allowed Extended-specific hunk; prove allowed On behavior as well as static file presence.
- [ ] Attribute each 0.10.0-specific feature, fix and behavior to immutable upstream/Extended SHAs; do not relabel old 0.9.0 features as new.

## Exact-SHA verification

- [ ] Run Node/TypeScript tests, typecheck and applicable UI/build checks on the **final exact candidate SHA**.
- [ ] Run Linux Rust check/test, relevant platform/build/release-script tests and macOS ONNX parity.
- [ ] Run mobile/Expo compatibility, npm dependency audit and RustSec against the same final SHA.
- [ ] Reconcile CI jobs/checks by name, URL, status and exact commit. Mark pending/failed/unrun checks explicitly; historical 0.9.0 CI does not qualify.
- [ ] Exercise actual Windows/macOS/Linux installed application behavior, installer provenance, signing and startup/upgrade paths.
- [ ] Exercise Settings On/Off, manual/automatic, bot pause, Connect, Cline/external MCP, cloud/VM and credential transfer with required approvals and process restarts.
- [ ] Test representative existing SQLite data upgrade and rollback/roll-forward constraints.

## Release identity and approval

- [ ] Establish independent Extended distribution version `0.10.0` in the packaging/release process without modifying the upstream source-application version for cosmetic alignment.
- [ ] Verify tag, artifact names, release-note mapping, updater identity, checksums and SBOM at exact published artifacts.
- [ ] Review [known issues](known-issues.md), [security evidence](dependency-security.md), native limitations and documented consumer upgrade steps.
- [ ] Obtain explicit release approval before tag, GitHub Release, npm publication, installer distribution or deployment.

## Evidence log

| Evidence | Observed result | Applicability |
| --- | --- | --- |
| Initial Extended branch at cycle opening | `e81222232314844d8fc52f65cb8e2dd37c5da2c8` | Starting reference only |
| Initial upstream fork branch | `fc32c3f585db6a72fee967e8d91f2bb50fe67852` (application 0.3.97) | Starting source reference only |
| Previous cycle's CI | See [0.9.0 notes](../../../releases/extended/0.9.0.md) | Historical, **not** 0.10.0 final validation |
| 2026-11-05 interim complete-tree inventory | Upstream [`fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852) vs Extended [`abe19dbd`](https://github.com/ginmp8/pandacode-extended/commit/abe19dbde19bb2f44b3d61bdf8668941425725e0): 1,501 upstream files; 1,436 exact; 65 different; zero missing; 40 Extended-only | File-presence and blob/mode comparison only; not complete per-hunk or functional certification |
| CI on interim Extended `abe19dbd` | [Push run 41960799509](https://github.com/ginmp8/pandacode-extended/actions/runs/41960799509) and [PR run 41961034561](https://github.com/ginmp8/pandacode-extended/actions/runs/41961034561): both completed successfully, six jobs each | CI for this exact interim SHA only; not final 0.10.0 native, release, or upgrade acceptance |
| ZIP patch commit CI, 2026-11-05 | Extended [`dcef242c`](https://github.com/ginmp8/pandacode-extended/commit/dcef242c7e201f060893a899cf04944f9f812b47); [Push CI 41969392506](https://github.com/ginmp8/pandacode-extended/actions/runs/41969392506) and [PR CI 41969398025](https://github.com/ginmp8/pandacode-extended/actions/runs/41969398025), 6/6 jobs each | **CI PASS for this exact code SHA**; focused VSIX, installed app, on/off and distribution approval not established |\n| Final 0.10.0 CI and native integration | Not yet executed/verified | **PENDING** |
| Final release approval and published artifacts | Not present | **PENDING** |

The next sync must refresh this log with real exact-SHA evidence. A documentation-only commit must not be described as passing native or end-to-end product acceptance.

## 2026-11-05 inherited documentation source checkpoint

- Source: [`78f77a0becee2029f4b89ef60fca0c97e48251e2`](https://github.com/ginmp8/pandacode/commit/78f77a0becee2029f4b89ef60fca0c97e48251e2), tree `1181cf82a6cf33834d620746f2ef57f8c195b270`, application version 0.3.97; parent `fc32c3f`.
- Source delta: 18 added/modified blobs (6 added screenshots, 8 replacement screenshots, README and 3 screenshot scripts), plus deletion of 1 obsolete screenshot. Previous destination contents of all modified/removed source paths matched the parent source blobs; no Extended runtime customization was overwritten.
- Post-write inventory, exact-SHA CI, screenshot Playwright execution and native control acceptance are **pending independent verification**. This is not a final 0.10.0 release sign-off.

## Interim 0.3.98 source reconciliation checkpoint — 2026-11-05

- Source `0461e02b85c95f8a52e927382c5069d0799dfed8` (tree `9cf46dc20979ea45a4e85c54bce83e52a86224f0`) declares application **0.3.98**. Previously integrated source: `78f77a0becee2029f4b89ef60fca0c97e48251e2`.
- The source delta contains **56 changed files**: 45 exact source blob copies and 11 shared-file source-hunk reconciliations, including six new source paths and no upstream deletions. Extended Feature controls and remediated dependency versions must remain intact.
- Final exact-SHA tree readback, CI, native builds, browser tests, On/Off Settings, Connect, cloud/VM, MCP, credential transfer, mobile/Expo, RustSec/npm audits and upgrade checks are **pending in this pre-publication record**. Update only with real evidence; do not mark final release gates complete.

## Verified source and CI checkpoint — 2026-11-05

- **Source:** [`0461e02b`](https://github.com/ginmp8/pandacode/commit/0461e02b85c95f8a52e927382c5069d0799dfed8) (**0.3.98**); **integrated code:** [`1d549e2b`](https://github.com/ginmp8/pandacode-extended/commit/1d549e2b01357d813e72ead373565de199481520). Recursive trees (`truncated=false`) verify **1,512 upstream files: 1,447 exact, 65 shared differences, none missing; 40 Extended-only files**, with no file-mode mismatches. The 65 retained differences still require complete hunk and enabled/disabled behavior verification.
- **Exact-code-SHA CI:** [Push #42101783450](https://github.com/ginmp8/pandacode-extended/actions/runs/42101783450) and [PR #42101794523](https://github.com/ginmp8/pandacode-extended/actions/runs/42101794523) completed successfully, **6/6 jobs each** (TypeScript/unit/website, Rust/Linux, RustSec, npm, mobile/Expo, macOS ONNX). This does **not** prove installed-app functionality or count as CI for subsequent documentation-only commits.
- **Outstanding:** release-script and native acceptance; end-to-end Settings On/Off, Connect/MCP/cloud/VM/credentials; actual upgrades, release identity and approval. No tag, installer, publication, or release approval follows from this checkpoint.

## Upstream 0.3.99 integration checkpoint — 2026-11-06

Source [`6b94bf1d`](https://github.com/ginmp8/pandacode/commit/6b94bf1d8b579d08834578f6e24684ea0d3fccbd) declares application **0.3.99** and a 14-path delta (9 exact source copies, 5 adapted paths). The post-write full-tree comparison must verify **1,513 upstream files: 1,446 exact, 67 diverging, none missing; 40 Extended-only**. The remaining divergences require per-hunk and native behavior certification.

- [ ] Validate the exact published Extended SHA, first parent, full recursive Git tree and source-mode parity.
- [ ] Verify exact-SHA CI jobs for Node/TS, Rust/check/test/Clippy, audits, website, Expo/mobile, macOS ONNX and relevant build preparation; no final-SHA result was available at authoring.
- [ ] Validate combined changes per-run diffs and review, native Settings On/Off, bot pause, Connect/MCP/cloud/VM/credentials, existing-data upgrade and separate Extended release identity before any approval.
