# PandaCode Extended 0.10.0 — unresolved verification register

**Status:** Candidate evidence gaps, **not** confirmation that each item is a product defect.
**Opening reference:** [Extended `e8122223`](https://github.com/ginmp8/pandacode-extended/commit/e81222232314844d8fc52f65cb8e2dd37c5da2c8); upstream [source `fc32c3f`](https://github.com/ginmp8/pandacode/commit/fc32c3f585db6a72fee967e8d91f2bb50fe67852) (0.3.97).

| Area | Classification at cycle opening | Required verification for 0.10.0 |
| --- | --- | --- |
| Shared upstream/Extended differences | **PARTIALLY VERIFIED (2026-11-05):** complete, nontruncated Git trees at upstream `fc32c3f585db6a72fee967e8d91f2bb50fe67852` and Extended `abe19dbde19bb2f44b3d61bdf8668941425725e0` show 1,501 upstream blobs: 1,436 exact, 65 shared-path differences, zero missing; 40 Extended-only blobs. Per-hunk and runtime parity are not certified | Recompute complete inventory at final source/destination SHAs and justify all retained product hunks |
| Settings and remote capabilities | **UNVERIFIED:** native On/Off behaviors for Connect, bots, MCP, direct routes, cloud/VM, credential transfer and updates | Exercise enabled, disabled, manual, automatic, restart, existing-process and approval paths |
| Desktop, model and UI behavior | **UNVERIFIED:** upstream 0.3.95–0.3.97 conversation folding, window restoration, IME, macOS drag/drop, Cline MCP and model configuration in installed apps | Native platform checks, provider compatibility and focused regressions |
| Mobile and simulators | **UNVERIFIED:** iOS transcript fix, Linux Android emulator availability/boot and actual device behavior | Exercise relevant devices and simulator startup |
| Upgrade/data compatibility | **UNVERIFIED:** existing SQLite data and real installer upgrades were not certified by historical CI | Restore a representative prior database, test migration/upgrade and recovery |
| Release identity and publication | **BLOCKED:** source Tauri version and `releases/{version}.md` are used by the current release workflow, not the independent Extended label | Implement/validate a distinct 0.10.0 distribution identity and release-note mapping without breaking source-version parity |
| Dependencies and security | **PENDING:** previous-cycle npm/RustSec success does not apply to the final 0.10.0 SHA | Re-run audits and review actual dependency/override deltas |
| Rust ZIP advisory GHSA-m665-v7pm-7grj | **TARGETED PATCH, CI PASS AT `dcef242c`:** The direct `zip` pin and Cargo.lock entry were changed from 9.0.4 to 9.0.5 (minimum patched release). The VSIX import path uses `ZipArchive`, not a confirmed streaming extraction path; no full-app exploit claim | Exact-SHA [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/41969392506) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/41969398025) passed all 12 jobs; focused VSIX and installed-platform acceptance remain unverified; monitor `main` separately |

No upstream capability should be removed on the basis of unverified concerns. Confirm actual defects with source evidence and focused regression tests, and preserve Extended's existing security/permission controls.

Reference: [0.9.0 historical register](../0.9.0/known-issues.md), [0.10.0 dependency evidence](dependency-security.md), and [release checklist](release-checklist.md).

## 2026-11-05 screenshot-documentation verification gap

[Upstream `78f77a0`](https://github.com/ginmp8/pandacode/commit/78f77a0becee2029f4b89ef60fca0c97e48251e2) updates the screenshot capture suite and visual evidence. The source changes were imported by exact blob identity, but the interactive Playwright screenshot recapture and native On/Off integrations were not executed in the restricted local environment. This is an **unverified test gap**, not a demonstrated product defect or a justification for disabling any upstream feature.

## Inherited 0.3.98 behavior requiring acceptance

[Source `0461e02`](https://github.com/ginmp8/pandacode/commit/0461e02b85c95f8a52e927382c5069d0799dfed8) adds Recommended Settings, UI pointer drag, queue ordering and team-lead response-card changes. The upstream [release notes](../../../releases/0.3.98.md) limit testing to previews and describe a possible brief duplicate wake card, a permission-menu timing hypothesis, and model-picker harness-list staleness. These are upstream-documented limitations or unverified hypotheses, **not newly demonstrated Extended vulnerabilities**.

Validate Recommended and the Extended Feature controls together, the enabled/disabled capability paths, macOS native drag/drop and OS file drops, bot wake card consolidation and keyboard queue fallback in installed applications. Until executed, full behavioral parity cannot be certified.

## Verified source and CI checkpoint — 2026-11-05

- **Source:** [`0461e02b`](https://github.com/ginmp8/pandacode/commit/0461e02b85c95f8a52e927382c5069d0799dfed8) (**0.3.98**); **integrated code:** [`1d549e2b`](https://github.com/ginmp8/pandacode-extended/commit/1d549e2b01357d813e72ead373565de199481520). Recursive trees (`truncated=false`) verify **1,512 upstream files: 1,447 exact, 65 shared differences, none missing; 40 Extended-only files**, with no file-mode mismatches. The 65 retained differences still require complete hunk and enabled/disabled behavior verification.
- **Exact-code-SHA CI:** [Push #42101783450](https://github.com/ginmp8/pandacode-extended/actions/runs/42101783450) and [PR #42101794523](https://github.com/ginmp8/pandacode-extended/actions/runs/42101794523) completed successfully, **6/6 jobs each** (TypeScript/unit/website, Rust/Linux, RustSec, npm, mobile/Expo, macOS ONNX). This does **not** prove installed-app functionality or count as CI for subsequent documentation-only commits.
- **Outstanding:** release-script and native acceptance; end-to-end Settings On/Off, Connect/MCP/cloud/VM/credentials; actual upgrades, release identity and approval. No tag, installer, publication, or release approval follows from this checkpoint.

## Upstream 0.3.99 integration checkpoint — 2026-11-06

Source [`6b94bf1d`](https://github.com/ginmp8/pandacode/commit/6b94bf1d8b579d08834578f6e24684ea0d3fccbd) declares application **0.3.99** and a 14-path delta (9 exact source copies, 5 adapted paths). The post-write full-tree comparison must verify **1,513 upstream files: 1,446 exact, 67 diverging, none missing; 40 Extended-only**. The remaining divergences require per-hunk and native behavior certification.

Upstream [0.3.99 notes](../../../releases/0.3.99.md) document summed rather than net changed-line counts and the lack of a changes row when added files are later all removed. These are inherited limitations, not independently reproduced Extended regressions. Verify per-run tabs/review, installed desktop UI, Settings On/Off, Connect, MCP, cloud/VM, bot permissions, credentials and upgrades; none is certified here.

## Bot queue status reconciliation — 2026-11-06

**Source-backed defect, installed-app reproduction pending:** the bot gateway in upstream PandaCode 0.3.98 and 0.3.99 takes queued messages from SQLite and starts the next turn without emitting `queue-changed`. The desktop UI keeps its previous queue snapshot and can display already-answered prompts as queued indefinitely. The generic panel also rendered **Sending next…** merely because a queue was present and no run/hold appeared, without any dispatch attempt. If a native queued launch failed, it restored the entry and wrote to bot Activity, but did not surface the failure in the queue UI.

**Extended correction:** the native bot drain now emits an existing `queue-changed` event **after** successful launch or after failed-launch restoration, with optional `delivery` and a privacy-limited error summary. The desktop re-reads the durable queue and presents a failed hold/notification. The animated **Sending next…** appears only when the frontend dispatcher is actually busy; otherwise the idle state says **Queued for delivery**. An inert window-side **Resume** control is hidden for native bot queues; no new retry endpoint or automatic retry is implied. A queued message whose restoration fails is explicitly reported as such. Ordinary desktop task queues retain their existing delivery path.

**Upstream compatibility:** compare `src-tauri/src/bots/gateway.rs`, `src/message-queue.ts`, `src/main.ts` and `src/bridge.ts` with the latest upstream on each parity import, remove the patch when equivalent event/error reporting ships, and preserve the original SQLite take/restore semantics. Regression: `tests/message-queue.spec.ts` exercises native-take synchronization and failure presentation through the preview bridge. Native bot execution and error paths still require installed-app testing before release sign-off.
