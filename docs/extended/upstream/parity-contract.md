# Upstream functional parity contract — PandaCode Extended

## Mandatory invariant

`ginmp8/pandacode-extended:base/development` is the complete functional counterpart of `ginmp8/pandacode:main`. The fork's **entire tracked source tree** is the integration input: product features, providers, agent harnesses, bots, MCP, schedules, remote/Connect, cloud/VM flows, mobile, tests, documentation, release notes, assets, scripts, lockfiles and build workflows. No feature may be left out or removed because it appears risky, automated, networked, privileged or inconvenient.

**Extended changes how selected behavior is configured; it does not delete the underlying upstream functionality.** A user who explicitly enables an Extended option should regain the applicable upstream capability, subject to the upstream feature's normal authentication, approval and platform requirements. A switch being Off by default is **not** evidence that the corresponding feature is absent. Disabling a subset of automatic behavior must not disable unrelated manual paths.

If an upstream feature conflicts with an existing Extended control, **adapt the implementation** to retain both the upstream feature and the control. Never silently choose an omission, code deletion, permanently disabled stub, unsupported provider, ignored source file or shortened release content as a substitute for the integration.

## Permitted source differences

1. **Extended Settings and opt-in defaults:** app and agent update preferences; the PandaCode Connect master switch and direct-route preference; cloud-credential transfer; agent-facing cloud/VM management; external MCP configuration; and the global bot pause. Preserve the enabled behavior, and keep scope and timing explicit (for example, a setting applied to newly launched processes does not retroactively change running processes). Details: [Feature controls](../0.7.0/feature-controls.md).
2. **Dependency remediation:** use compatible versions with verified security fixes and the Extended-local `deepmerge` patch when appropriate. Keep every upstream capability that consumes those dependencies, and preserve the corresponding lockfiles and tests. Details: [Dependency security](../0.7.0/dependency-security.md).
3. **Necessary integration glue:** tests, migrations, product/release identity, CI and documentation required to preserve the above controls or Extended-specific data compatibility. These are additions or bounded adaptations, **not exemptions to feature parity**. The historical Extended release number may differ from the upstream application/source version.

A deviation does **not** become acceptable merely because it is mentioned in security documentation, accepted by a model or accompanied by a passing audit. Every divergent production path must have a concrete, reviewed reason within this contract. A new deliberate product behavior difference requires a separate explicit decision.

## Security evidence and terminology

- **Confirmed security finding:** tie it to a reproducible defect, applicable advisory, actual dependency version or observed exploit path. Name the affected scope and verification result.
- **Verified functional defect:** describe the failing behavior and reproduction without automatically classifying it as a vulnerability.
- **Unverified hypothesis / integration question:** say what is not established and which exact test would resolve it. Do not call it a confirmed vulnerability, critical issue or release blocker without supporting evidence.
- **Expected configuration behavior:** an Off switch, a setting that applies only to new sessions, or a local preference that does not revoke previously transferred credentials is a product-policy scope statement, **not by itself a security defect**.

Do not suppress real test failures or confirmed vulnerabilities. Conversely, do not remove upstream functionality to avoid hypothetical risks. Where a problem exists, correct its root cause or document the precise, evidence-backed limitation, preserving both source parity and user-configurable control.

## Required audit for each upstream synchronization

1. Pin fresh source and destination HEAD SHAs and compare **recursive Git trees**, not merely issue labels, commit titles, release summaries or a handpicked file list.
2. Import every added/changed upstream file, including release/version metadata and tests. For files modified only upstream, retain the exact upstream blob. For paths changed on both sides, merge every upstream hunk and preserve Extended-specific controls. Keep Extended-only files.
3. Inventory **all** remaining source-versus-Extended blob differences and attribute each to an allowed difference, with code-level evidence. Review both added and removed behavior; a code hunk appearing somewhere is not a complete behavioral proof.
4. Verify each configurable capability with the option **On and Off**, including its UI, native enforcement, manual path, error recovery and relevant provider/OS edge cases. Test the enabled upstream use case to catch accidental feature loss.
5. Reconcile upstream versions, manifests, historical release notes, build/release scripts, dependent lockfiles and CI. Execute focused tests, security checks and required complete branch CI; distinguish structural/file parity from behavioral, security and native-installer validation.
6. Publish directly to `base/development` only with authorized, non-forced, concurrency-checked updates. Re-read the final branch HEAD. Do not alter the source fork or publish a release as part of a parity copy.

## Pinned integration checkpoint — 2026-11-02

- **Source:** `ginmp8/pandacode:main` at `348add18a63906bd8928a7cef547eeaf91429fa7`, upstream application version `0.3.90`. Exact source identity: [source-baseline.json](source-baseline.json).
- **Extended integration:** [`d3980c0b`](https://github.com/ginmp8/pandacode-extended/commit/d3980c0b3898d0a8883a4bcfb32fc66d7c412786) on `base/development`.
- Compared with the common `0.3.87` source baseline, **67 changed/new upstream paths** were copied with original blob content, **17 common modified paths** were merged, **41 previously Extended-modified paths** were retained, and **20 Extended-only files** remained. The 148 upstream patch hunks in the 17 merged files were accounted for: 140 applied with exact new context, 8 adapted to preserve Extended controls; every upstream added source line in those hunks was present in the resulting file. This is **structural audit evidence**, not a claim that every runtime scenario was executed.
- CI [`41463892251`](https://github.com/ginmp8/pandacode-extended/actions/runs/41463892251) for that commit: npm audit, RustSec audit, web/build/tests, mobile and macOS ONNX checks succeeded. Linux Rust unit tests finished **1874 passed / 1 failed / 74 ignored**; the failure was `remote::catalog::tests::a_warm_sends_nothing_once_it_may_not`, in a source-identical file with a 20 ms timeout. Do **not** claim completed runtime parity or successful full CI from this snapshot; investigate/re-run independently.

This checkpoint is historical. For any later sync, compare fresh HEADs; do not hardcode `0.3.90` as the perpetual target.

## Follow-up synchronization evidence — 2026-11-02

- Frozen fork `ginmp8/pandacode:main`: `348add18a63906bd8928a7cef547eeaf91429fa7` (application version `0.3.90`). Fork tree: `8a8f206dac527ded72d26c2a342c6bf6f15d26ab`.
- [Extended mode parity commit](https://github.com/ginmp8/pandacode-extended/commit/0504b1c25532e363b2508725984e654cea19a216) restores the upstream executable bit (`100755`) on `packages/npx/bin/pandacode.mjs` and `src/assets/providers/cline.svg`. The two original blob SHAs remain byte-identical. No product code or controls were changed.
- Recursive tree readback (both responses `truncated=false`): 1,448 upstream blobs; 1,389 identical blobs and modes, 59 content-divergent paths, zero missing upstream paths, and 21 Extended-only paths. The latter include Extended Settings/control, tests, docs and dependency adaptations. The remaining 59 differences are **not** automatically accepted: code-level scope, preserved upstream enabled behavior and security justification must be validated per relevant hunk.
- All six application-version fields in five manifest/lock/config files were read from both frozen SHAs and equal `0.3.90`; dependency versions and patches intentionally differ where Extended remediations are present.
- [CI run 41478201907](https://github.com/ginmp8/pandacode-extended/actions/runs/41478201907) on `0504b1c25532e363b2508725984e654cea19a216`: all six jobs reported **success** (TypeScript/unit/website, Linux Rust check/tests, RustSec, npm audit, mobile/Expo and macOS ONNX). This is executed CI evidence for the mode-only code commit, not a native installer test.
- No local full repository clone/build occurred because the sandbox could not resolve `github.com`. GitHub's authenticated commit/tree/blob APIs supplied untruncated source/destination inventory and verified the changed metadata. Native installer packaging, two-real-host Connect, and comprehensive On/Off UI/runtime scenarios were **not executed** here. Global functional parity remains **unverified** despite zero missing tracked upstream paths.

## Upstream 0.3.91 source synchronization candidate — 2026-11-02

- Pinned upstream fork HEAD: [`a9be2ced7bb594ad28a09fbd204cdad34770c750`](https://github.com/ginmp8/pandacode/commit/a9be2ced7bb594ad28a09fbd204cdad34770c750), tree `3f0d2d06ef3376033be6ee71b8e686f0a229a3fb`, application version `0.3.91`.
- Relative to the previously reviewed `0.3.90` source, 34 source paths were added or modified: 27 are eligible for verbatim blob integration, and seven require source-hunk/Extended-control reconciliation. No source path may be omitted, including the new original `releases/0.3.91.md`.
- Source-introduced functionality includes project relinking/worktree repair, Linux clipboard and undo fallbacks, ACP mode behavior, Claude large catalog, DeepSeek slash commands, OpenCode MCP inventory, Windows conversation import and cloud Settings ordering.
- Preserve Extended controls, patched dependencies and its independent `0.8.0` candidate documentation. Runtime parity of all Settings On/Off, native platform validation and two-device Connect are **not certified** by this source inventory. Verify CI against the exact resulting Extended commit.

## Published source-integration evidence — 2026-11-02

- Exact upstream source: [PandaCode `0.3.91` at `a9be2ced7bb594ad28a09fbd204cdad34770c750`](https://github.com/ginmp8/pandacode/commit/a9be2ced7bb594ad28a09fbd204cdad34770c750).
- Direct Extended integration commit: [`083e764485f08ea1b5cabbf3b3d18ea9b426e89c`](https://github.com/ginmp8/pandacode-extended/commit/083e764485f08ea1b5cabbf3b3d18ea9b426e89c) on `base/development`.
- [CI run #41498498966](https://github.com/ginmp8/pandacode-extended/actions/runs/41498498966) reported **success in all six jobs** on that exact code SHA: TypeScript/unit/website, Linux Rust test/Clippy, mobile/Expo, npm dependency audit, RustSec audit and macOS Panda Router ONNX.
- Complete recursive Git-tree readback, neither truncated: 1,455 upstream blobs, zero missing source paths, 1,396 byte/mode-identical paths, 59 source-versus-Extended content differences, and 27 Extended-only paths. Seven of the 59 differences are the reviewed three-way application of source `0.3.91` hunks; the others are carried Extended-specific deviations that still need individual functional review.
- The six application-version fields match upstream `0.3.91`. The independent Extended `0.8.0` remains an **unreleased documentation candidate**, with no public installer or distribution claim.
- No local full repository checkout/build was possible because the sandbox could not resolve GitHub DNS; the integration used pinned GitHub API trees/blobs with exact SHA verification. Native Windows/Linux/macOS GUI checks, two-real-device Connect and complete Settings On/Off/runtime scenarios were **not executed**. GitHub CI success does not prove these behaviors.
- Historical [PR #105](https://github.com/ginmp8/pandacode-extended/pull/105) was already merged from `base/hardened` to `main`, based on an earlier `0.3.90` Extended checkpoint; it does not include the new `base/development` `0.3.91` integration. This run created or changed no PR.

