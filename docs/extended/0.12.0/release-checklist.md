# PandaCode Extended 0.12.0 — release readiness checklist

## 2026-11-14 — CI verified for the 0.3.126 source-pinned code SHA

- [x] Freeze and recheck upstream commit `6c91efdb4d5ff317c2e7597fcb717e0814939349` and Extended code commit `8dca72763c5a3a798d9d9b84bd7a0641df7f8682`.
- [x] Check complete Git trees with zero missing source paths and zero file type/mode mismatches.
- [x] Verify [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43607984368) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43607990721) completed successfully (11/11 jobs each, 22/22 total).
- [x] Verify the CI TypeScript/website job explicitly ran and passed `npm run test:release`.
- [ ] Execute native On/Off and external Connect/MCP/bots/cloud/VM/credential integrations.
- [ ] Independently establish behavior parity for all 81 customized shared files.
- [ ] Complete candidate acceptance and release review; Extended 0.12.0 remains DRAFT.

The upstream [source commit](https://github.com/ginmp8/pandacode/commit/6c91efdb4d5ff317c2e7597fcb717e0814939349) (tree `23478e308beefa66091d1d2fbceebabc913e4de3`) declares application version `0.3.126`; the corresponding [Extended code commit](https://github.com/ginmp8/pandacode-extended/commit/8dca72763c5a3a798d9d9b84bd7a0641df7f8682) is the verified integration checkpoint. At that exact Extended SHA, [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43607984368) and [pull-request CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43607990721) both completed successfully (**11/11 jobs each; 22/22 total**). Successful checks include Rust tests/Clippy, TypeScript and unit tests, `npm run test:release`, website build, Expo/mobile, dependency audits, RustSec and ONNX parity. The full source/destination Git trees were independently retrieved without truncation: **2,062 upstream paths, 2,120 Extended paths, 1,981 identical shared paths, 81 customized shared paths, 58 Extended-only paths, zero missing upstream paths and zero type/mode mismatches**. This is a source-coverage and CI receipt, **not** evidence of end-to-end equivalence of the 81 customizations or native Settings On/Off, Connect, MCP, bots, cloud, VM and credential flows. The planned Extended **0.12.0 remains DRAFT**; no release or distribution was authorized.

## 2026-11-14 — upstream 0.3.126 source identity CI correction

- [x] Diagnose the exact-SHA [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43603320753) failure: two release-identity assertions incorrectly expect **0.3.125** while all application manifests declare source **0.3.126**.
- [x] Verify current source commit/tree, all 40 historical release-note blobs and the release-date blob against the complete, nontruncated source/destination trees; update the pinned Extended baseline without rewriting original source notes or weakening tests.
- [ ] Confirm `npm run test:release`, the full push/PR CI job matrix and release metadata on the new corrective SHA.
- [ ] Independently validate native Extended Settings On/Off, Connect, bots, MCP, cloud/VM, credential flows and the 81 shared customization paths.
- [ ] Keep planned Extended **0.12.0 DRAFT**; no distribution or release approval is implied.

## 2026-11-14 — upstream 0.3.126 candidate gate

- [x] Freeze and verify source [`6c91efdb4d5f`](https://github.com/ginmp8/pandacode/commit/6c91efdb4d5ff317c2e7597fcb717e0814939349) versus destination baseline [`7206c0592f65`](https://github.com/ginmp8/pandacode-extended/commit/7206c0592f652af3aea01848ea85c44906f0264d); all recursive trees complete, not truncated.
- [x] Include 59 changed upstream paths: 50 source-exact (11 new, including two binary PNG screenshots), nine uniquely matched Extended-aware patch reconciliations, no upstream deletions.
- [x] Candidate static inventory: 2,062 source, 2,120 Extended files; 1,981 shared identical, 81 customized, 58 Extended-only; no missing source or type/mode divergence.
- [x] Align app/build version metadata to **0.3.126** while leaving planned Extended distribution documentation at **0.12.0 DRAFT**.
- [x] Retain upstream original release notes, existing Extended configurable controls, security remediations and exclusive paths.
- [ ] Run and inspect all CI jobs against the newly published destination commit (prior baseline CI is not sufficient).
- [ ] Run native project explorer/editor safety and editing tests, ZCode team MCP session bridge and worktrees, animated-sky performance, and platform screenshots.
- [ ] Validate Extended Settings On/Off, manual versus automatic actions, bots, Connect, remote MCP, cloud/VM and credential flows. Independently review all 81 customized shared paths.
- [ ] Approve release after applicable cross-platform validation; **do not** tag, publish, create an installer or otherwise treat this candidate as released.

## 2026-11-14 — inherited VM RFB fix candidate from `da1ecb5fc452`

- [x] Pin [upstream `da1ecb5fc452`](https://github.com/ginmp8/pandacode/commit/da1ecb5fc452bc8df313b4485da93d4d573e1e4d) and [Extended `20795fc15cf4`](https://github.com/ginmp8/pandacode-extended/commit/20795fc15cf4cfd41aaf77a06c65be88757e939d), and compare both complete recursive Git trees without truncation.
- [x] Identify exactly three changed upstream paths; prepare byte-exact copies and preserve 81 customized shared plus 58 Extended-only paths.
- [x] Confirm application metadata **0.3.125** independently of planned Extended **0.12.0 DRAFT** and preserve dependency hardening.
- [x] Record pre-import [Extended `20795fc15cf4`](https://github.com/ginmp8/pandacode-extended/commit/20795fc15cf4cfd41aaf77a06c65be88757e939d) CI: [push](https://github.com/ginmp8/pandacode-extended/actions/runs/43574216539) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43574221160), 22/22 passed.
- [ ] Verify CI and build checks of the **new published SHA**; prior CI is not transferable.
- [ ] Execute Rust RFB mock-server tests and native QEMU/Virtualization.framework VM capture, resize, click and type acceptance.
- [ ] Validate existing VM management controls and related Extended Settings On/Off paths; independently review all 81 customized shared paths for functional parity.
- [ ] Obtain separate release approval; no tag, public installer, GitHub Release or deploy is authorized.

## 2026-11-14 — verified CI for the Cargo.lock reconciliation

- [x] Verify exact-source [source 6c42278ff598](https://github.com/ginmp8/pandacode/commit/6c42278ff5981e50c9dc494986b4380123d4686d) and exact-code [Extended 7d6897e87c3e](https://github.com/ginmp8/pandacode-extended/commit/7d6897e87c3e8a88c1ebb323783cb75a7071f65b); the native application remains **0.3.125**.
- [x] Verify complete Git tree inventories: **2,051 upstream** and **2,109 Extended** paths, **zero missing** and **zero mode/type differences**; **81 customized shared** paths remain.
- [x] Verify [push CI #43568928848](https://github.com/ginmp8/pandacode-extended/actions/runs/43568928848) and [PR CI #43568932573](https://github.com/ginmp8/pandacode-extended/actions/runs/43568932573): **11/11 successful jobs each**, including Rust locked build/tests, Clippy, RustSec, npm, TypeScript/website, Expo and ONNX.
- [ ] Independently validate functional parity across all 81 customized paths, native Settings On/Off, direct/automatic and existing/new bot execution, Connect/MCP, cloud/VM and credential-transfer integrations.
- [ ] Complete platform/release approval gates. **Extended 0.12.0 remains DRAFT**; CI success alone is not approval.


## 2026-11-14 — Cargo.lock consistency repair (validation pending)

- [x] Reproduce the exact `Cargo.lock` correction from the failed CI's `cargo metadata` diff and match its Git blob SHA `b9922223a60fc1da5628307009569758f20fb1a3`.
- [ ] Confirm `cargo --locked` and all Rust/Clippy/ONNX jobs on the **new** destination commit (previous push/PR runs failed).
- [ ] Validate native Settings On/Off and all external integrations; 81 customized upstream paths remain a separate semantic acceptance gate.
- [ ] Approve Extended 0.12.0 for distribution only after all required release gates; currently DRAFT.

## 2026-11-14 — source HEAD import candidate (not approved)

- [x] Freeze [source `6c42278ff598`](https://github.com/ginmp8/pandacode/commit/6c42278ff5981e50c9dc494986b4380123d4686d) and confirm app version **0.3.125**.
- [x] Reconcile **62** changed source paths and verify complete untruncated Git tree inventory (2,051 source / 2,109 Extended; 0 missing or mode differences).
- [x] Preserve planned Extended **0.12.0 DRAFT**, previous release history and upstream notes exactly.
- [x] Preserve Extended configuration gates and dependency remediation at source level; verify basic Rust lock dependency graph consistency.
- [ ] Obtain exact published candidate SHA and **that SHA's** CI results; previous [previous Extended `5530431964e1`](https://github.com/ginmp8/pandacode-extended/commit/5530431964e114b90c4464450fde63bf4682544b) CI is historical, not candidate evidence.
- [ ] Run Rust `cargo check/test/clippy --locked`, Node `npm ci` and builds, Expo and dependency audits on the reconciled tree.
- [ ] Validate Cursor, Antigravity MCP, updater certificates, installed platforms and native Extended On/Off/direct/automatic workflows.
- [ ] Independently establish functional parity across the remaining **81** customized shared paths and release readiness approvals.

Extended **0.12.0 is not an approved distribution**.

## 2026-11-13 — Astro remediation CI evidence checkpoint

- [x] Confirm the exact inherited source SHA [`9739736d`](https://github.com/ginmp8/pandacode/commit/9739736d134ba29c6a07d92eca5745c3c4e5999b) and declared native application version **0.3.125**; retain planned Extended **0.12.0 DRAFT** separately.
- [x] Verify the complete, nontruncated source/target Git trees: 2,048 upstream paths, 2,106 Extended paths, 1,967 byte-identical shared paths, 81 customized shared paths, 58 Extended-only paths, zero missing paths or type/mode differences.
- [x] Verify code SHA [`19ec683`](https://github.com/ginmp8/pandacode-extended/commit/19ec683abce68d9951d5fca660c852494d435b5e) exact-SHA [Push CI #43507810199](https://github.com/ginmp8/pandacode-extended/actions/runs/43507810199) and [PR CI #43507821448](https://github.com/ginmp8/pandacode-extended/actions/runs/43507821448) completed **success (11/11 jobs each)**, including website `npm ci`, website build, website npm audit and cross-platform checks.
- [ ] Run native Settings On/Off and external Connect/MCP/bots/cloud/VM/credential integration acceptance; perform independent semantic review of all 81 shared customizations.
- [ ] Check exact-SHA CI separately for any subsequently published documentation-only commit; never carry forward the predecessor SHA's result.
- [ ] Obtain independent release authorization outside this maintenance workflow. **Do not tag or publish Extended 0.12.0** here.


## 2026-11-13 — website Astro remediation gates (not release approval)

- [x] Freeze current fork source `9739736d134ba29c6a07d92eca5745c3c4e5999b` and preserve native app version 0.3.125.
- [x] Preserve Extended Sharp 0.35.5 and existing control code in the website patch.
- [x] Verify nontruncated source/destination inventories: 2,048 source files, 2,106 Extended files, zero missing source paths and zero type/mode mismatches.
- [ ] Verify `npm ci`, website build and zero findings from the website npm audit on the **new exact destination SHA**, not the preceding failed run.
- [ ] Verify all required latest-SHA CI jobs; the preceding `ea9dda3` website audit failed and the security patch CI is pending.
- [ ] Validate native Settings On/Off and Connect/MCP/bots/cloud/VM/credential behavior; review 81 Extended-divergent shared paths semantically.
- [ ] Obtain separate release authorization. **0.12.0 remains DRAFT**; no tag, GitHub Release or distribution approval is implied.

## 2026-11-13 — upstream 0.3.125 reconciliation

Frozen upstream [`9739736d134ba29c6a07d92eca5745c3c4e5999b`](https://github.com/ginmp8/pandacode/commit/9739736d134ba29c6a07d92eca5745c3c4e5999b), tree `275bef5585e153bee54345166f9fafadea2c281e`, application **0.3.125**. Three upstream commits since `fd5942ec358a` changed **15 paths** (11 source-exact and four Extended-aware version/lockfile adaptations; zero deletions). Candidate inventory: **2,048 source / 2,106 Extended files; 1,968 exact shared / 80 customized shared / 58 Extended-only; zero missing source paths or mode/type differences**. Planned Extended **0.12.0 remains DRAFT**. This records a candidate, not exact-new-SHA CI success or native Settings acceptance.

- [x] Inventory frozen upstream tree (2,048 files) and the prior Extended tree (2,105 files) without truncation.
- [x] Plan 11 exact copies (including `releases/0.3.125.md`) and four Extended-aware version edits; preserve the 58 Extended-only files.
- [ ] Verify the newly published destination commit, single-parent lineage and final 2,106-file tree.
- [ ] Check Node/TypeScript, Rust/Clippy, Expo, dependency audits, GitHub Actions and native Settings On/Off on the new SHA.
- [ ] Audit all 80 customized shared paths and obtain separate Extended release approval; no release is authorized.

## 2026-11-13 — completed 0.3.124 integration CI verification

- [x] Reconfirm source [fork `fd5942ec358ac3cad0721524284e41904a99218e`](https://github.com/ginmp8/pandacode/commit/fd5942ec358ac3cad0721524284e41904a99218e) and destination [Extended `cd182962990966bea002798c25b2218a1df78956`](https://github.com/ginmp8/pandacode-extended/commit/cd182962990966bea002798c25b2218a1df78956); no upstream file missing in the complete 2,047/2,105-file inventory.
- [x] Verify [Push CI #43413789980](https://github.com/ginmp8/pandacode-extended/actions/runs/43413789980) and [PR CI #43413799925](https://github.com/ginmp8/pandacode-extended/actions/runs/43413799925) completed **11/11 jobs each** on the exact destination code SHA.
- [ ] Review all 80 customized shared paths semantically and execute native Settings On/Off and external integration acceptance.
- [ ] Complete separate release approval; Extended 0.12.0 remains DRAFT.

These checks refer to the code SHA named above, not to a later documentation-only commit.

## 2026-11-13 — upstream 0.3.124 import verification checklist

- [x] Freeze [source fd5942ec358a](https://github.com/ginmp8/pandacode/commit/fd5942ec358ac3cad0721524284e41904a99218e), tree 675c46eeb3768952b6ca6a296184ec7cad5921ef, app 0.3.124.
- [x] Inventory complete source/target trees (2,047 / 2,105 files), preserve 58 Extended-only and 80 customized shared paths; no source path or mode/type missing.
- [x] Reconcile 11 changed upstream paths (7 exact, 4 adapted) and retain dependency hardening.
- [x] Keep Extended 0.12.0 planned and DRAFT.
- [x] Record prior destination f7076cd6b0ae CI 11/11 push and 11/11 PR success.
- [ ] Verify new destination SHA Push/PR CI after publication.
- [ ] Execute native Skills Library rollback/collision checks and Settings On/Off integration.
- [ ] Review all 80 shared divergences semantically and verify critical integrations.
- [ ] Obtain separate release approval (out of scope here).

Prior destination f7076cd6b0ae passed [Push CI 11/11](https://github.com/ginmp8/pandacode-extended/actions/runs/43390834157) and [PR CI 11/11](https://github.com/ginmp8/pandacode-extended/actions/runs/43390844083). New SHA CI must be checked after publication. Local Rust/Node/Expo tests not run (no checkout, no Cargo, GitHub DNS unavailable). Native Settings On/Off and external integrations unverified.

## 2026-11-13 — post-0.3.123 source-fix checkpoint

- [x] Freeze [source `3026a7af132c`](https://github.com/ginmp8/pandacode/commit/3026a7af132c5b7684be414040a2d4f7158f80e4) and tree `e50b17094d98c5631a819b67dd36d49929615546` (one unreleased commit; app version remains 0.3.123).
- [x] Inventory all 2,046 source and 2,104 destination paths; preserve 58 Extended-only and 80 customized paths, with zero source paths missing or mode/type differences after the four-file import.
- [x] Import all four upstream changed files byte/mode-exact, including the original revised `releases/0.3.123.md`.
- [x] Identify [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43369851585) / [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43369858494) failures on destination `d3858b2e4462`: two Node timezone assertions and Windows x64/arm64 Clippy unused constant.
- [x] Verify final CI for destination code SHA `a87be35fa883842555dc494e81948a89e7db6ccf`: [Push CI #43376914754](https://github.com/ginmp8/pandacode-extended/actions/runs/43376914754) **11/11 success**, [PR CI #43376923244](https://github.com/ginmp8/pandacode-extended/actions/runs/43376923244) **11/11 success** (2026-11-13). Future documentation-only SHAs require separate checks.
- [ ] Run native crash recovery and Extended Settings ON/OFF, Connect/MCP/bots/cloud/VM/credentials tests on supported installations; review all 80 customizations semantically.
- [ ] Obtain independent release authorization before any tagging, distribution or deployment. Extended 0.12.0 is DRAFT.


## 2026-11-13 — PandaCode 0.3.123 candidate checklist

Source [`8001e5b98ed3`](https://github.com/ginmp8/pandacode/commit/8001e5b98ed3be0c6fb207fccfeada726d736e3c), tree `6f5a537dbd1c90ecd3e46132a990f974077774db`, app **0.3.123**. **5 source commits**, **69 changed paths: 54 source-exact and 15 Extended-aware**. Full candidate: **2,046 upstream / 2,104 Extended files; 1,966 shared exact / 80 shared customized / 58 Extended-only; 0 missing and 0 type/mode discrepancies**. Original [upstream 0.3.123 note](https://github.com/ginmp8/pandacode/blob/8001e5b98ed3be0c6fb207fccfeada726d736e3c/releases/0.3.123.md) remains byte-identical. Planned **Extended 0.12.0 is DRAFT**; prior [0.3.122 Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43363128304) passed, and the final new commit requires its own CI.

- [x] Pin source `8001e5b98ed3be0c6fb207fccfeada726d736e3c` and initial destination `010dbd9b517a6c069c2a903fa6bd67d026105f79`; retrieve full nontruncated recursive trees.
- [x] Stage all **69** changed upstream paths: **54 exact** and **15 reconciled**; preserve 58 Extended-only paths, original release note and existing Git modes.
- [x] Verify candidate inventory **2,046 source / 2,104 destination**, **1,966 identical / 80 modified shared / 58 Extended-only**, no missing source paths or mode/type mismatches.
- [x] Align source-declared application metadata to **0.3.123** while keeping planned Extended **0.12.0 DRAFT**.
- [ ] Read back the final commit's parent/ref/tree after nonforced expected-SHA publication, and check **every job** of the final-SHA CI; earlier CI is not sufficient.
- [ ] Run Node/TS, Rust check/test/Clippy, Expo, release/version scripts, npm/RustSec, native crash/restart/drafts and platform-specific On/Off tests, Connect/MCP/bots/cloud/VM/credential integrations.
- [ ] Complete semantic review of all 80 retained shared divergences and separate release approval. No tag, deploy, public installer or npm publish is authorized.

**Unverified:** local Node/Rust/Clippy/Expo audits/builds, native macOS/Windows/Linux crash handling, Settings On/Off, external integrations and semantic validation of all 80 customized paths. No release approval is implied.


## 2026-11-13 — PandaCode 0.3.122 candidate integration checklist

- [x] Pin authenticated `ginmp8/pandacode:main` at `420a163d3b921fd113e3d9ecc8437410a5ff7c6c` (tree `bdca4b5ba5a32b3e54c7beaf38db8c989889584f`) and destination `6f28006424162b3e650cedbe9bb855d6841a3d94`; inspect full recursive nontruncated inventories.
- [x] Incorporate all **87** source-changing paths (73 exact; 14 hunk-reconciled), including original `releases/0.3.122.md`, new Alibaba provider Rust module and logo; preserve all 58 Extended-only paths and existing modes.
- [x] Preserve existing Extended controls, remediated dependencies, and the live **0.12.0 DRAFT** selector while aligning six upstream app-version metadata fields to **0.3.122**.
- [x] Verify the staged candidate's **2,011 source / 2,069 destination** files, zero missing or type/mode mismatches, **1,931 exact / 80 customized / 58 Extended-only**.
- [ ] Verify final published commit, single parent, fresh source/destination refs, full-tree readback, and all exact-SHA GitHub Actions CI jobs; earlier green runs do not establish this candidate's status.
- [ ] Run Node/TS and build/release tests, Rust check/test/Clippy, Expo, npm/RustSec, native speed/provider flows, Settings On/Off and external Connect/MCP/bots/cloud/VM/credentials acceptance; review 80 historical shared divergences independently.
- [ ] Approve an Extended distribution only through its separate release process; do not tag, publish, deploy or modify the planned distribution version in this import.

## 2026-11-12 — source 0.3.121 candidate integration checklist

- [x] Authenticate GitHub and pin full source `54df768753607f69f42b0c148a4ef665cb0f45a1`, Git tree `503546c881f2b24230ba4a8616e163739dc39db4`, and prior Extended `bf20fce79523`; compare nontruncated recursive trees and the 25-commit / 28-path delta.
- [x] Prepare all 28 changed upstream paths (17 source-exact, 11 Extended-aware), including original `releases/0.3.121.md`; preserve all other source/Extended paths and file modes.
- [x] Advance only upstream application version metadata to `0.3.121`; retain existing Extended dependency hardening and planned **0.12.0 DRAFT** marker.
- [x] Carry the inherited remote approval/ZCode, bot, schedule and Workers functionality into intersecting Extended code without deleting configurable settings.
- [ ] Confirm the final published SHA, single parent, complete candidate inventory and exact-SHA successful CI; prior `bf20fce79523` [push](https://github.com/ginmp8/pandacode-extended/actions/runs/43295833034) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43295845945) both succeeded 11/11, but do not validate this candidate.
- [ ] Run or obtain independent Node/TypeScript, build/release, Rust check/test/Clippy, Expo, npm/RustSec, installed-app Settings On/Off and external Connect/MCP/bot/cloud/VM/credential acceptance, then review the 80 historical shared divergences before claiming full functional parity.
- [ ] Approve an Extended distribution only through its separate release process; do not tag, publish, deploy or alter the planned version in this integration.

## 2026-11-12 — inherited 0.3.120 limitation notes

Pinned [upstream `f66e95267701`](https://github.com/ginmp8/pandacode/commit/f66e95267701750696d4a43eabdd6c74f6b56927), source tree `085808f3e028ff6ad4b515d562d65c6485f0b921`, app **0.3.120**. The sole source change since the previous snapshot is a five-bullet addition to the [original release note](https://github.com/ginmp8/pandacode/blob/f66e95267701750696d4a43eabdd6c74f6b56927/releases/0.3.120.md), blob `f49dba032c6236f85ee88642221c35a2a0b25275`. No new application, manifest, lockfile, test, workflow or Extended-control change was introduced. Git trees retain 2,007 upstream paths / 2,065 Extended paths, zero upstream files missing or mode/type mismatch, and 80 previously customized shared paths after this note is mirrored. Extended **0.12.0 remains DRAFT**, not released.

- [x] Confirm full, nontruncated source/Extended tree inventory and one-path upstream note delta.
- [x] Mirror the upstream release-note bytes and update `source-baseline.json` provenance.
- [x] Retain Extended 0.12.0 planning marker and all existing controls/dependency remediations.
- [ ] Confirm CI for the new published SHA (previous SHA `f432b854fe2b`: 9/11 push and 10/11 PR jobs passing at inspection, macOS Clippy still in progress).
- [ ] Validate native Settings On/Off, integrations and historical divergent-file behavior.


## 2026-11-12 — 0.3.120 parity checkpoint (not approval)

Pinned source [`69155055e1a9`](https://github.com/ginmp8/pandacode/commit/69155055e1a962c0af4b0cf041af0b1f87291588), tree `e6dfbb33ad2d03dc51005b21ae76f873028c7a54`, app **0.3.120**; three source commits/54 changed paths, **49 byte-exact imports, 5 Extended adaptations**. Proposed structural inventory: 2,007 upstream blobs, 2,065 Extended blobs, 1,927 identical shared, 80 divergent shared and 58 Extended-only; no missing upstream files or mode/type mismatch. Previous Extended SHA `e0beb365b279` failed Rust compilation on all CI targets because `architect.rs:1436` contained an unescaped `"auto"` string; staged correction escapes the quotes without changing tool parameters. No new SHA CI, local Rust, Node, native or external On/Off results are claimed. Extended **0.12.0 remains DRAFT**.

- [x] Pin and compare complete nontruncated `main`/`base/development` recursive Git trees, modes and blob IDs
- [x] Account for every changed upstream path (49 exact, five reconciled) and preserve 58 Extended-only files
- [x] Keep independent Extended distribution marker at `0.12.0` and original upstream release notes unchanged
- [x] Diagnose the existing multi-platform Rust syntax CI failure and stage a minimal quote-escape fix
- [ ] Confirm final published commit SHA/parent/ref with GitHub readback and exact-SHA CI success
- [ ] Execute full Rust, Node/TypeScript, Expo, release/audit and native On/Off functional acceptance
- [ ] Review the remaining 80 shared divergent files semantically before claiming complete behavioral parity or release readiness


## 2026-11-12 — 0.3.119 source synchronization evidence, not release approval

Pinned source: [ginmp8/pandacode:main at `b026526122c4`](https://github.com/ginmp8/pandacode/commit/b026526122c45264f389c04f91a4e96010948926), Git tree `ba8cfd56d9eb91fe433abc1e9377cb069a8f848e`, native application version **0.3.119**. Compared with the last synchronized 0.3.117 source, 44 commits changed 132 paths; the reconciled candidate incorporates 109 source-exact paths and 23 Extended-aware adaptations, with zero upstream paths absent or type/mode discrepancies. Its full inventory is 2,002 upstream files, 2,060 Extended files, 1,922 shared byte-exact, 80 shared divergent and 58 Extended-only. Historical shared differences are not yet all behaviorally proven. The Extended distribution stays **0.12.0 DRAFT**; no release, tag or public package is authorized.

The upstream and Extended branches independently used SQLite schema version 37 for different changes. The candidate advances the effective schema to **38** and runs both the existing Extended cloud/bot compatibility work and upstream saved-remotely schedule provenance migration for databases from either v37 lineage. A regression test for the Extended-v37 upgrade path was added; **Rust migration tests have not yet executed**. This is a required compatibility adaptation, not an upstream feature removal.

Validation for this candidate: full Git path/blob/mode tree inventory and source delta reconciliation; no complete local Node, Rust, Clippy, Expo, native desktop or external-service test suite was executed in this environment. Both CI workflows on the **preceding** Extended SHA `f7197e67f4d1d9d0362f7597f6e34447ac23f0b1` passed ([push](https://github.com/ginmp8/pandacode-extended/actions/runs/43236804107), [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43236826490)); that does **not** validate this integration. The new published SHA's checks must be judged separately. Native acceptance and behavior-level audit of 80 divergent shared files remain open.

Release readiness remains **NOT APPROVED**. The source tree is present with 23 Extended-aware reconciliations and 80 shared divergences overall; verify all remediated dependencies, both v37→v38 migration lineages, full Rust/Node/TS/mobile/website builds and native On/Off / cloud / remote / MCP workflows. Require completed CI for the exact commit eventually published on `base/development`, rather than a predecessor. Do not tag, publish npm, open a release, auto-merge a PR, or deploy from this maintenance flow.


## 2026-11-12 — upstream 0.3.117 candidate checkpoint

- [x] Freeze fork `9648d1c4c9e68f86265b9be04dfb1a8c1f960fb4` / tree `597285f6e291294cd230c4f09befa578fa87bd54`; complete recursive Git inventory is not truncated (1,971 upstream paths; 2,029 candidate Extended paths).
- [x] Stage all 90 source-changed paths (79 exact, 11 source-hunk/Extended-aware adaptations), including 11 newly added source files and byte-exact original release notes; no missing source paths, type differences or mode changes.
- [x] Carry forward Extended schema v37 (source v36), Connect master gate, configuration, dependency remediations and original upstream app-version alignment to **0.3.117**, independently of Extended **0.12.0 DRAFT**.
- [ ] Obtain all CI/build/audit results for the exact published integration SHA. Prior Extended `84f9f754` CI passed but cannot establish new-SHA success.
- [ ] Run full Node/TypeScript/browser/Rust/Clippy/Expo checks, error dismissal/undo and storage/remote mirror scenarios, screen-reader and native Settings On/Off/external integrations.
- [ ] Independently review the 83 customized shared paths at the hunk and behavioral level; complete release approval separately, without tag, installer, npm publish or deployment.

## 2026-11-12 — upstream 0.3.116 synchronization checkpoint

- [x] Pin complete fork source SHA 12a0468948c2ff1c9bc6be0bebf3f85411d73819, Git tree 1fc1d5aec803b0c6242d6293eceebcfff0982229, application version **0.3.116**; recursive Git trees were not truncated.
- [x] Incorporate all 86 changed upstream paths (55 new exact, 23 modified exact, eight source-patch/Extended-preserving merges), including original upstream release notes and text test fixtures; zero upstream paths missing and no mode differences.
- [x] Align six npm/Cargo/Tauri app version values while retaining Extended dependency patches and **0.12.0 DRAFT** release planning.
- [ ] Obtain completed CI statuses for the **published** destination SHA; no green conclusion is presumed.
- [ ] Execute native Node/TS, Rust/Clippy, Expo/mobile, audits, tool image authorization, screen-reader and browser viewer tests, Settings On/Off and external integrations.
- [ ] Complete semantic review of 81 historical customized shared paths, preserve required approvals and obtain separate release approval.

The tool rows, local image viewer, task-scoped thumbnail command and error/risk labels are inherited from the fork; they are not newly exclusive Extended features.
## 2026-11-12 — upstream 0.3.115 Skills-library checkpoint

- [x] Freeze [fork `main` source](https://github.com/ginmp8/pandacode/commit/7171e9606e48b3645b5c4a1a6aac01386d5a6369) and complete recursive Git tree `dd913037d13509ca9da764d77f9cb9eea4b1c4a9` (not truncated).
- [x] Integrate all 15 upstream-changed paths: 11 byte/mode-exact imports (including original `releases/0.3.115.md`); four version-only manifest adaptations retaining Extended dependency remediation.
- [x] Align all six npm/Cargo/Tauri app-version fields to `0.3.115`, without advancing Extended `0.12.0` DRAFT.
- [ ] Check CI for the final destination SHA (Node, Rust, RustSec/npm, mobile/Expo and platform Clippy). Prior `64cd597d` passed 11/11 jobs in each push and PR run, not yet evidence for this new commit.
- [ ] Validate installed-app Skills project activation/deactivation, phone disabled-skill commands and remote clone/worktree refresh, plus the existing Extended On/Off controls.
- [ ] Review all 81 historical customized shared paths semantically, and obtain separate release approval.

## 2026-11-12 — upstream 0.3.114 checkpoint

- [x] Pin full upstream source SHA `650537ae449d805656f531fc6fdb5dec09eceb80` and Git tree `e6a217fa48157f11f9bc03739d7995b0c8c4cd74`; include all 41 changed source paths, including original release files and new transcript modules/tests.
- [x] Preserve the Extended-only controls and dependency differences in eight modified shared paths; align npm/Cargo/Tauri native version to 0.3.114 without changing the planned Extended 0.12.0 version.
- [ ] Verify exact published commit CI across Node, Rust, RustSec/npm, macOS/Windows/Linux, Expo and audio fallback.
- [ ] Verify runtime transcript windowing, loaded-turn Find, send/scroll and installed-platform performance.
- [ ] Verify native Settings On/Off, Connect, MCP, bots, cloud/VM, credentials, and all remaining historical shared-file differences.
- [ ] Approve an Extended release separately. No tag, GitHub Release, npm publication, installers or deploy in this sync.

## 2026-11-12 — source 9aa88776 reconciliation

- [x] Pin fork `main` source SHA `9aa88776f07c2274134e4a952ffcd7da71e70e50`, tree `bd360c6d9d3d6f8bd209a8408914c4e1b80f85ef`, and native version `0.3.112`.
- [x] Import four changed upstream files byte/mode-exact, reconcile CI without dropping Extended jobs and strengthen `tests/ci-audio-smoke.test.mjs`.
- [x] Preserve the planned Extended `0.12.0` DRAFT marker and original upstream notes.
- [ ] Verify published candidate against complete Git trees, exact new-SHA CI, real microphone fallback and native Settings On/Off independently.


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

## 2026-11-12 source/test checkpoint

- [x] Pin [fork `9e5d0988`](https://github.com/ginmp8/pandacode/commit/9e5d0988970785a9302938ecdc8d8848b7d79a7f), tree `f82fa609b8f4e74d322729f7c189e783d98e4cad`, destination `e1cd7e7592276b88c7e808836cc053c5ea1e336d`, and full nontruncated inventories; application version unchanged at **0.3.111**.
- [x] Copy the sole upstream change (`src-tauri/src/voice/tests/real_audio.rs`) byte/mode-exact; preserve 81 customized shared and 58 Extended-only paths.
- [x] Update the stale Extended baseline hash for the existing upstream `releases/0.3.111.md` blob. Prior push and PR CI each passed 10/11 jobs and failed only the pin-based release fixture.
- [ ] Verify CI on the newly published exact destination SHA; pending is not success.
- [ ] Validate native microphone, Settings On/Off, external integrations and full shared-difference semantics.

## 2026-11-12 synchronization checkpoint

- [x] Pin source [`8e5ab59c`](https://github.com/ginmp8/pandacode/commit/8e5ab59cdd22344a08e199dd94e6674af71ac7bd) and previous destination `e059e9a08623eb9ccf3fbafabde5bdf679136de4`; verify full trees and application **0.3.111**.
- [x] Mirror the one changed upstream release file; reconcile the source CI exact audio selection and APT retries, preserving Extended gates; fix the stale Extended audio test assertion.
- [ ] Verify the new destination SHA's CI; Previous Extended `e059e9a08623` [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43159765950) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43159773384) each reported a failing Node web-test job (526/527 passed): its stale assertion required macOS-only capture despite Linux/macOS now being required. Corrected without weakening coverage. Other jobs were running at inspection; new-SHA CI and real Settings On/Off, microphone and integration acceptance remain pending.
- [ ] Complete native microphone, Settings On/Off, bots, MCP, Connect, cloud/VM and credentials, plus semantic review of the 81 customized shared files.

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

## Fork HEAD 979b7d4e integration — 2026-11-10

- [x] Pin upstream `979b7d4e3c073e9bebf74f70bf80440a27564237`, tree `ba33a37daed7860854926617c90197d3182bd8ed`, app **0.3.108**; preserve planned Extended **0.12.0 DRAFT**.
- [x] Stage five exact upstream blobs and one Extended-preserving workflow adaptation; add the malformed CI-lookup regression assertion.
- [x] Preserve Extended controls, dependency remediation, source original release notes, and manual-only release publishing.
- [ ] Verify **new destination SHA** push/PR CI by job; the preceding `4ef18f0f` push had a cancelled ONNX/macOS job while PR CI passed.
- [ ] Independently test manual publish CI lookup failure/pending/green paths without publishing, native voice behavior, Settings On/Off, Connect, MCP, bots, cloud/VM, credentials, platform installers and all 80 historic divergent shared-file hunks.
- [ ] Approve a release separately; no tag, installer publication or npm release is authorized here.

## Fork HEAD bc323966 integration — 2026-11-10

- [x] Pin [fork main `bc323966a0e0`](https://github.com/ginmp8/pandacode/commit/bc323966a0e0f24b13dfd5732cc300f791a965fe), source tree `74ac097cad986c31715b54553e2017ae978d1351`, application **0.3.108** and the existing planned Extended **0.12.0** documentation marker.
- [x] Stage all 11 upstream path updates: 8 exact and 3 adapted for Extended's CI/manual release and agent guidance; extend regression tests.
- [x] Preserve prior Extended controls, SQLite remediation and historical release files; make no PR, tag, public installer, GitHub Release or npm publication.
- [ ] Verify published one-parent commit, complete source/destination path/mode/blob inventory, final statuses of all CI jobs and remaining shared-file differences.
- [ ] Validate native Voice, Settings On/Off, Connect, MCP, bots, cloud/VM, credential workflows, installers, SQLite upgrade/rollback and release gate without actually publishing. Separate release approval required.

## Fork HEAD 9437b3a9 follow-up — 2026-11-10

- [x] Pin fork `9437b3a92cc3d41e899770f645f8a08449ce6bbf`, source tree `5b8666849fcd7f3cbba918754efbfaedbc8fef41`, native app **0.3.108**; keep Extended planned **0.12.0 DRAFT**.
- [x] Import five exact upstream source blobs and reconcile two CI/release workflows without losing manual publication or adding npm publish; update the voice/privacy/release documentation unchanged from source.
- [x] Add release-gate source-object/run-ID regression assertions and update the exact source-baseline release-notes blob hash.
- [ ] Review exact-SHA CI after publication, complete native Voice/Settings On/Off/Connect/MCP/bots/cloud/VM/credential, migration and installer acceptance, and semantically classify every divergent shared hunk.

## Exact-SHA CI and sampled shared-difference inspection — 2026-11-10

- [x] Re-read the frozen fork main `9437b3a92cc3d41e899770f645f8a08449ce6bbf` and Extended base/development `e7361c1c22355718cbbbf0e7ec4202a3e9673f61`; their Git tree objects are `5b8666849fcd7f3cbba918754efbfaedbc8fef41` and `d560b10f0d9daf1273ef1d1effc8a4306ec0b6bc`, respectively.
- [x] Verify full nontruncated inventories: 1,696 upstream tracked blobs and 1,753 Extended tracked blobs; 1,615 shared byte-exact, 81 shared divergent, 57 Extended-exclusive, zero upstream missing, zero mode/type discrepancies.
- [x] Verify the last **code** SHA `e7361c1c22355718cbbbf0e7ec4202a3e9673f61` passed [push CI #42892078107](https://github.com/ginmp8/pandacode-extended/actions/runs/42892078107) (11/11) and [PR CI #42892089259](https://github.com/ginmp8/pandacode-extended/actions/runs/42892089259) (11/11). All 22 GitHub check runs for that exact SHA completed successfully, including npm/RustSec, TypeScript, Linux Rust, five Clippy targets, Expo and ONNX.
- [x] Inspect source and Extended blobs and changed lines in **18 of 81** inherited shared-path differences. Provisional categories, **not** completed full functional acceptance:
  - `EXTENDED_CONTROL` (13): `src-tauri/src/providers/mcp_host.rs` (default-off strict MCP fixture), `src/about-settings.ts` (update mode UI), `src-tauri/src/remote/catalog.rs` (Connect-enabled test fixture), `src-tauri/src/cloud_agents/credentials.rs` (credential opt-in), `src-tauri/src/vm/tools.rs` (agent VM/cloud write control), `src-tauri/src/remote/routes.rs` (Connect and direct routes), `src-tauri/src/bots/tools.rs` (opt-in fresh-context delivery), `src-tauri/src/bots/schedule.rs` (pause), `src-tauri/src/context_recovery.rs` (fresh-context replay), `src-tauri/src/headless.rs` (headless Connect opt-in), `src-tauri/src/providers/warm.rs` (external MCP setting), `src-tauri/src/remote/socket.rs` (Connect listener gating), `src-tauri/src/cloud/deploy.rs` (cloud-management opt-in).
  - `EXTENDED_DEPENDENCY` (2): `src-tauri/src/cloud/cloudflare_worker/Dockerfile` and `src-tauri/src/cloud/cloudflare_worker/package.json` use the same upgraded Cloudflare sandbox version `1.0.3`; this is a version divergence, **not a newly proven vulnerability fix**.
  - `EXTENDED_ONLY` integration/compatibility (3 shared paths): `.gitattributes` (line-ending stability), `src-tauri/src/remote/orchestrate/tests.rs` and `src-tauri/src/schedules/tests.rs` (Extended SQLite schema version 37).
- [ ] Complete hunk-level causal review of **all 81** shared differences (63 paths not sampled in this pass); verify actual provider/agent, updater, credential, MCP, Connect, bot pause, cloud/VM, Voice, fresh-context, SQLite migration and native installer On/Off behavior. The 18 provisional classifications above do not prove complete functional parity.
- [ ] Verify CI of any later documentation-only commit at its own SHA. Passing code-SHA CI does not constitute approval of Extended release 0.12.0.

## Mobile node-forge advisory remediation — 2026-11-10

- [x] Identify failure in [exact-SHA push CI `c7c14606`](https://github.com/ginmp8/pandacode-extended/actions/runs/42920988289): mobile node-forge 1.4.2, while root, website and relay npm audits succeeded.
- [x] Verify patched version **1.4.3** from GitHub-reviewed advisories and its tarball integrity from the official npm registry.
- [x] Update only `mobile/package.json` override and the matching `mobile/package-lock.json` entry; preserve all unrelated dependency versions and upstream files.
- [ ] Validate **new published SHA** using `npm audit --prefix mobile --audit-level=low`, Expo/mobile compatibility and all other CI jobs.
- [ ] Validate native/mobile runtime and SDK compatibility independently; the advisory fix alone does not approve a release.

## Exact-SHA remediation CI and additional shared-path sample — 2026-11-10

- [x] Re-read the current fork HEAD `9437b3a92cc3d41e899770f645f8a08449ce6bbf` (tree `5b8666849fcd7f3cbba918754efbfaedbc8fef41`) and pre-documentation Extended HEAD `ab6c99787bf8ece8eae70b51eb3d071b91353e41` (tree `45f1c61da42d6ac03655168458bf8d15d09e3a78`).
- [x] Confirm nontruncated recursive Git inventories: **1,696 upstream / 1,753 Extended** tracked files, **1,615 shared exact, 81 shared divergent, 57 Extended-only, zero missing or mode/type changes**. No source import or version increment was warranted.
- [x] Verify `mobile/package.json` override and `mobile/package-lock.json` resolve `node-forge@1.4.3` on the remediation SHA.
- [x] Verify [push CI #42921416535](https://github.com/ginmp8/pandacode-extended/actions/runs/42921416535) and [PR CI #42921425457](https://github.com/ginmp8/pandacode-extended/actions/runs/42921425457): **11/11 jobs passed in each workflow on `ab6c99787bf8ece8eae70b51eb3d071b91353e41`**, including the npm dependency audit, Expo/mobile, RustSec, Rust check/tests, TypeScript/unit/website, five Clippy targets and ONNX/macOS. This supersedes the pending exact-SHA CI note in the mobile remediation entry for that SHA.
- [x] Compare source and destination text at **20 additional distinct shared paths**, inspecting added/source-only line sets as provisional evidence rather than claiming finished hunk/caller/runtime validation:
  - Extended control or bot-flow wiring: `src-tauri/src/lib.rs`, `src-tauri/src/bots.rs`, `src-tauri/src/remote.rs`, `src-tauri/src/bots/gateway.rs`, `src-tauri/src/bots/channels/mod.rs`, `src-tauri/src/bots/shared_channels.rs`, `src-tauri/src/providers.rs`, `src-tauri/src/remote/config.rs`, `src/bridge.ts`, `src/remote-settings.ts`, `src/settings.ts`, `src/settings-sections.ts`.
  - Compatibility, tests or Extended-only integration: `src-tauri/src/storage.rs` (schema 37), `src-tauri/src/remote/host.rs` (Connect fixture), `src-tauri/src/cloud/cloudflare.rs` (sandbox-version fixture), `src-tauri/src/architect.rs` and `src-tauri/src/architect/coordination.rs` (tool catalogs/scopes), `src/main.ts` and `src/btw-panel.ts` (side-thread delivery), `mobile/src/ui/push.ts` (Expo notification import compatibility).
- [ ] Complete semantic, hunk-level and caller/test validation for all **81** shared divergences. Together with the previous 18-path sample, **38 distinct paths have received preliminary line-difference inspection**, leaving **43 without even this preliminary sample**; none of the 81 is certified by that count.
- [ ] Complete native/installed-app Settings On/Off, Connect/MCP, bot recovery and delegation, Voice, cloud/VM, credentials, SQLite upgrade/rollback and installer/release acceptance. CI does not approve distribution 0.12.0.
- [ ] Check CI for any later documentation-only SHA separately; do not treat the code SHA's passing CI as a result for this documentation commit.

## Fork HEAD 64996a1a checklist — 2026-11-11

- [x] Freeze fork SHA `64996a1a8b1d722eb10b0c4ec4b3c1ffef1fd197` and tree `7be2186e18047098aedbbd18ff0b4ea11d50b806`; prior Extended SHA `05cd6fb123e96cd294be2c77f7d38eba002f3c27`.
- [x] Reconcile all 17 changed upstream paths (12 exact copies including two additions, five Extended-preserving adaptations); retain original upstream release documentation and hardening versions.
- [ ] Verify CI on the published Extended commit, including optimized macOS real-audio smoke, Rust check/test/Clippy, RustSec, Node/TS tests, npm audits, mobile/Expo and website.
- [ ] Independently test installed macOS Voice capture, saved device IDs, and Extended Settings On/Off/manual/automatic behavior, plus native Connect/MCP/bots/cloud/VM/credential paths.
- [ ] Complete behavioral review of the 81 historical shared-code differences; a complete Git path inventory is not functional parity proof.
- [ ] Validate distribution artifacts and platform installers before approving Extended 0.12.0. No tags or GitHub Releases are authorized by this sync.

## 0.3.109 release-CI regression follow-up — 2026-11-11

- [x] Review exact-SHA [push #42950948699](https://github.com/ginmp8/pandacode-extended/actions/runs/42950948699) and [PR #42950956081](https://github.com/ginmp8/pandacode-extended/actions/runs/42950956081) for `7a624076c0ed44b9ecf89861497e9ef4e697f9be`: each failed the single Extended release-workflow expectation while other jobs passed, including the macOS optimized real-audio smoke.
- [x] Correct the stale `scripts/release/workflow-publish-gate.test.mjs` predicate to require the existing push-preserving CI policy `github.event_name != 'push'`; keep the real CI workflow unchanged and retain release publication restrictions.
- [ ] Verify the correction in both push and PR CI at the **new exact commit SHA**, including all release tests and platform jobs. Do not carry forward success from earlier commits.
- [ ] Complete hunk-level and native On/Off, installed-app/Voice, bots, Connect/MCP, cloud/VM, credentials and migration acceptance before release approval.

## Fork HEAD 66a7df2f synchronization — 2026-11-11

- [x] Retrieve new fork HEAD and complete nontruncated recursive trees, including binary file SHA/mode inventory; locate the new upstream ALSA test absent from Extended.
- [x] Import six source-exact files (five updates, one new test), plus one CI change adapted to Extended's validated longer macOS timeouts and push-CI policy.
- [x] Retain upstream native version **0.3.109** across npm/Cargo/Tauri and the separate planned Extended **0.12.0 DRAFT**; preserve upstream release notes byte-exact.
- [ ] Verify every source path and hash against the newly published destination SHA; document the complete list of 81 customized shared files with hunk-level behavior evidence.
- [ ] Require new exact-SHA push and PR CI, including Rust, Clippy, Node/TypeScript, release tests, npm/RustSec, Expo and macOS real-audio.
- [ ] Test Linux saved microphone selection and installed macOS capture, along with Extended Settings On/Off, bots, Connect/MCP, cloud/VM, credentials and release/installers. CI is not native acceptance.

## Source `a403c3d9` (app 0.3.111) integration gate — 2026-11-11

- [x] Fetch both recursive Git trees without truncation; inspect source blob hashes, modes and source deletions.
- [x] Include all 191 new upstream paths and 102 upstream-only modifications; reconcile 15 additional jointly changed source paths in a single-parent candidate.
- [x] Preserve 57 Extended-only paths and 81 allowed-or-pending-review shared divergences; no upstream path missing, no mode mismatch.
- [x] Retain Extended-specific configuration and dependency overrides in source-reconciled manifests and native call paths.
- [ ] Verify the **published** destination SHA, its single parent, source/target readback, and latest-source parity inventory.
- [ ] Verify exact-SHA CI jobs: Node/TS, release/website, Rust check/test/Clippy, npm and RustSec, mobile/Expo and macOS.
- [ ] Independently test mobile and remote RPC refactor, Settings On/Off, Connect/MCP, bots, cloud/VM, credentials, native installs and migrations; complete the remaining 81-file semantic audit.

This is an integration **candidate**, not approval to distribute Extended `0.12.0`.


## Fork `961b43fa` post-0.3.111 synchronization — 2026-11-11

- [x] Pin fork HEAD [`961b43fa`](https://github.com/ginmp8/pandacode/commit/961b43fa88ad1d24eb083bc10779cb7034c4a0fd), tree `3506e38d545f11c57ebb3f2ebe3acdec449606bc`, and verify its single new commit and seven modified source paths; native version remains **0.3.111**.
- [x] Import six source-exact blobs (Voice, Rust test, release notes and startup-smoke script); reconcile CI to run startup smoke on Windows ARM64 while preserving Extended 70/120-minute timeouts.
- [x] Publish direct single-parent integration [`5beff6d8`](https://github.com/ginmp8/pandacode-extended/commit/5beff6d8737e952341023bfdf10aaeb38697fcae) and verify commit/ref/tree readback.
- [x] Compare complete nontruncated source and integration trees: 1,890/1,947 files, 1,809 exact shared, 81 divergent shared, 57 Extended-only, no missing source paths or mode/type mismatch.
- [ ] Confirm final documentation SHA and **its exact-SHA** CI (Node/TS, Rust, Clippy, RustSec, npm, Expo and Windows ARM64 startup-smoke). [Code PR run #43045289031](https://github.com/ginmp8/pandacode-extended/actions/runs/43045289031) started; [code push run #43045275146](https://github.com/ginmp8/pandacode-extended/actions/runs/43045275146) pending at inspection.
- [ ] Independently verify saved/non-default Linux microphone fallback, real hardware, installer behavior and Extended Settings On/Off, Connect/MCP, bots, cloud/VM, credentials, and the 81 carried shared differences before claiming functional release readiness.

No release approval, tag, installer, npm publication or deployment is implied.

## Linux Rust CI APT mirror recovery — 2026-11-11

- [x] Diagnose [PR CI #43045852885](https://github.com/ginmp8/pandacode-extended/actions/runs/43045852885): the Linux Rust job was cancelled while APT was downloading from the Azure mirror, before application build or tests.
- [x] Preserve the exact dependency list and Rust test/Clippy commands while replacing the Ubuntu 22.04 job's unreliable mirror and setting bounded APT retries and HTTP timeouts.
- [ ] Confirm all required jobs in both PR and push CI for the **published remediation SHA**; a pending, cancelled or skipped job is not successful validation.
- [ ] Continue independent native Settings On/Off, bots, Connect/MCP, cloud/VM, credentials, migration and installed-app acceptance; no new release approval.

## Linux ARM64 APT resilience follow-up — 2026-11-11

- [x] Inspect [PR CI #43055772877](https://github.com/ginmp8/pandacode-extended/actions/runs/43055772877) at `b21df4a7`: `Rust clippy (linux-aarch64)` failed at APT (DNS/index/404, exit 100) before Clippy; the PR Linux Rust job was cancelled during dependency acquisition. Do not report these as Rust assertion failures.
- [x] Preserve the ARM64 runner, original native dependency list and Rust commands while adding bounded retries for both APT refresh and installation; require the refresh to succeed, avoiding silent stale-index reuse.
- [ ] Verify both push and PR CI on the **new exact SHA**, including the Linux ARM64 job and Ubuntu 22.04 Rust test job; record jobs separately (queued/cancelled/failed is not green).
- [ ] Complete all 81 shared-path semantic reviews and native Settings On/Off, Connect/MCP, bots, cloud/VM, credentials and installer acceptance before 0.12.0 release approval.

## Fork HEAD 106b3852 integration checkpoint — 2026-11-11

- [x] Resolve the source at `106b3852fcfc737922189cb3ef64b98b6b727c69` (tree `155ab5c52bf39cfa6b0df48ede563e078297055a`) and destination at `bb360f4c0ef98dfe7a0afc61ea36a3d1d7d12f0e`; receive full recursive trees without truncation.
- [x] Account for all six new upstream-modified paths: five source-exact blobs and one adapted CI workflow, with zero upstream paths omitted and no type/mode differences.
- [x] Keep native app metadata at **0.3.111** and Extended planned notes at **0.12.0 DRAFT**.
- [x] Retain Extended audit/mobile jobs, recovery of Linux APT downloads, source push-CI policy and Windows ARM64 startup tests while gaining new upstream audio-smoke targets.
- [ ] Finish CI and audit jobs at the exact published Extended SHA; do not treat queued/cancelled jobs as success.
- [ ] Independently validate real Linux microphone selection/failures and Windows/Linux ARM64 audio behavior.
- [ ] Review all 81 older customized shared paths by hunk and exercise Settings On/Off, Connect/MCP, bots, cloud/VM and credentials.

## Exact-SHA CI completion — 2026-11-11

- [x] Confirm frozen source [`106b3852`](https://github.com/ginmp8/pandacode/commit/106b3852fcfc737922189cb3ef64b98b6b727c69) and published code SHA [`d3a59446`](https://github.com/ginmp8/pandacode-extended/commit/d3a59446bd9c208b89e8d999f3d001d5be2f6955), with single-parent lineage.
- [x] Confirm [push CI #43075335735](https://github.com/ginmp8/pandacode-extended/actions/runs/43075335735) **11/11 passed** and [PR CI #43075350618](https://github.com/ginmp8/pandacode-extended/actions/runs/43075350618) **11/11 passed** on the same code SHA; all jobs were completed successfully, including Linux Rust/ARM64, platform Clippy, RustSec, npm, mobile/Expo and Node/website.
- [x] Recompute nontruncated recursive inventories: 1,890 upstream, 1,948 Extended, 1,809 identical shared, 81 divergent shared, 58 Extended-only, zero missing or mode/type mismatches; version fields aligned at upstream `0.3.111`.
- [x] Inspect added/source-only changed-line sets for all 81 divergent shared file versions; preliminary grouping does not establish full semantic acceptance or native behavior.
- [ ] Review every shared divergence in context, including control-dependent callers, error/edge cases and native Settings On/Off transitions; resolve any independently reproduced regression without removing upstream features.
- [ ] Perform native Voice, Connect/MCP, bots, cloud/VM, credential, SQLite migration/rollback and installer acceptance before any Extended `0.12.0` release approval.
- [ ] Check CI for the separate documentation-only SHA after publication; passing code-SHA checks do not transfer to the new SHA.

## Fork a2ba4b43 source-documentation checkpoint — 2026-11-12

- [x] Pin [source `a2ba4b43`](https://github.com/ginmp8/pandacode/commit/a2ba4b430951ee2995bdfda140d4f516cb493a78) and initial Extended `base/development` `1c9fcc3e`; verify complete nontruncated recursive Git trees.
- [x] Mirror the only changed source path, `releases/0.3.111.md`, with exact blob `ce5842359a33679869b8843565b467d8d438266a` and original `100644` mode.
- [x] Keep all six native application-version fields at **0.3.111** and the marker at planned **Extended 0.12.0 DRAFT**.
- [x] Record prior SHA `1c9fcc3e` [push](https://github.com/ginmp8/pandacode-extended/actions/runs/43093250270) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43093260565) CI success, 11/11 jobs each; these results do not validate the new commit.
- [ ] Check all CI jobs at the exact new published SHA.
- [ ] Finish contextual review of 81 shared Extended differences and native Settings On/Off, phone access, retry, photo, audio, Connect/MCP, bots, cloud/VM, credentials and installers before release approval.

## Fork HEAD 802fa847 source checkpoint — 2026-11-12

- [x] Pin source `802fa847f6890d729e98bfe4f2f44190cf045861` (tree `52668daa62dd6a20211545bdacd8d0026e8c638f`) and initial destination `2fdacaa7c493d42327ea040814aa75cdb87ecca6`; retrieve both complete nontruncated recursive inventories.
- [x] Account for all **three** modified source paths: two byte/mode-exact Voice/release documents and one CI workflow reconciliation retaining Extended audit, mobile, pinned actions, APT recovery and macOS 70-minute smoke.
- [x] Preserve native application version **0.3.111**, planned Extended distribution **0.12.0 DRAFT**, historical upstream release notes, 58 Extended-only files, and zero missing source paths or mode mismatches.
- [x] Verify previous SHA `2fdacaa7` [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43152555072) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43152561038) — **11/11 successful jobs each**.
- [ ] Independently verify all new exact-SHA CI jobs including Linux x64/ARM64 required release-profile audio smoke; prior green jobs do not cover the new checks.
- [ ] Finish 81 shared-path semantic hunk/caller reviews and native real-microphone, Settings On/Off, Connect/MCP, bots, cloud/VM, credential, migration and installer acceptance before approving 0.12.0.

## Source 0.3.112 synchronization — 2026-11-12

- [x] Pin [fork `eb78ef90`](https://github.com/ginmp8/pandacode/commit/eb78ef90df6809219349e23fadf2e659ba00da2c), native app `0.3.112`, source tree `9440057b8012ac1ccd9bbf5a7b5656d488e889c1`; compare complete source/destination recursive trees.
- [x] Integrate new `quit_guard.rs` and original `releases/0.3.112.md`, native exit path, installer notes and tests, and remaining source paths without removing Extended code or dependency hardening.
- [x] Align all six app-version fields and Extended source provenance, leaving independent planned release `0.12.0` unchanged.
- [ ] Confirm CI of the **new exact destination SHA**; the [prior push](https://github.com/ginmp8/pandacode-extended/actions/runs/43173312668) and [prior PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43173319356) passed 11/11 jobs each but do not certify a new commit.
- [ ] Exercise native quit dialog (Quit/Cancel, repeated requests, local/cloud-only, serve and restart), all Extended Settings On/Off, native platform builds, dependency audits and integration paths; audit remaining customized source hunks before any parity/release approval.

## 2026-11-12 — CI cancellation guard regression test correction

- [x] Pin source `9aa88776f07c2274134e4a952ffcd7da71e70e50`, destination `dd15a184c6184df330896299ba50443dc559e6b2` and both complete recursive trees; confirm zero upstream paths missing and all application-version fields remain at `0.3.112`.
- [x] Diagnose the exact failed step in [push CI #43188294463](https://github.com/ginmp8/pandacode-extended/actions/runs/43188294463) and [PR CI #43188308169](https://github.com/ginmp8/pandacode-extended/actions/runs/43188308169): the release-test suite has one stale CI cancellation assertion, not a failed runtime feature test.
- [x] Update only the Extended release-gate regression assertion to match the upstream/Extended enforced `github.event_name == 'pull_request'` predicate, preserving active push and manual CI; leave production workflow behavior unchanged.
- [ ] Verify the published fix by complete, exact-SHA push and PR CI runs; a queued, in-progress or failed job is not successful validation.
- [ ] Continue contextual review of 81 shared divergent paths and native Settings On/Off, real audio, quit guard, Connect/MCP, bots, cloud/VM, credentials, migrations and installers before release approval.
