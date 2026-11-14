# Extended 0.12.0 — dependency and security tracking

## 2026-11-14 — upstream app 0.3.126 without Extended dependency rollback

Source [`6c91efdb4d5f`](https://github.com/ginmp8/pandacode/commit/6c91efdb4d5ff317c2e7597fcb717e0814939349) declares application **0.3.126**. The candidate synchronizes root `package.json`, both root/version entries in `package-lock.json`, the app package in `src-tauri/Cargo.lock`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`; the last is source-exact. Existing Extended dependency overrides, remediated package versions and lockfile fixes remain. The 0.3.126 upstream manifest delta is a version-only update in these files, not a dependency downgrade.

Static source/destination paths, modes and blobs were checked for the frozen candidate. No new-SHA Node, Rust, Clippy, npm audit or RustSec result exists at documentation authoring. The previous destination [`7206c0592f65`](https://github.com/ginmp8/pandacode-extended/commit/7206c0592f652af3aea01848ea85c44906f0264d) passed [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43597837416) (11/11), but this does not validate the new integration or justify approving **Extended 0.12.0 DRAFT**.

## 2026-11-14 — source VM frame fix without dependency rollback

[upstream `da1ecb5fc452`](https://github.com/ginmp8/pandacode/commit/da1ecb5fc452bc8df313b4485da93d4d573e1e4d) changes RFB Rust implementation/tests and an agent reference only. Source and Extended application metadata still declare **0.3.125** in npm, the root npm lock metadata, Tauri config, Cargo manifest and the application entry in Cargo.lock. No dependency manifest or lockfile is imported in this checkpoint: Extended's existing remediations, overrides and compatible lockfile remain in place. The baseline [Extended `20795fc15cf4`](https://github.com/ginmp8/pandacode-extended/commit/20795fc15cf4cfd41aaf77a06c65be88757e939d) passed 22/22 CI jobs, including audits, but **no audits or Rust tests for the new candidate SHA have been executed yet**. The documentation version remains **0.12.0 DRAFT**.

## 2026-11-14 — verified CI for the Cargo.lock reconciliation

The frozen [source 6c42278ff598](https://github.com/ginmp8/pandacode/commit/6c42278ff5981e50c9dc494986b4380123d4686d) still declares application **0.3.125**. For [Extended 7d6897e87c3e](https://github.com/ginmp8/pandacode-extended/commit/7d6897e87c3e8a88c1ebb323783cb75a7071f65b), both [push CI #43568928848](https://github.com/ginmp8/pandacode-extended/actions/runs/43568928848) and [PR CI #43568932573](https://github.com/ginmp8/pandacode-extended/actions/runs/43568932573) finished **success (11/11 jobs each; 22/22 total)**. The jobs include Linux Rust check/tests, five Rust Clippy platform targets, macOS ONNX parity, TypeScript/unit/website checks, mobile/Expo, npm dependency audit and Cargo/RustSec audit. This replaces the *pending* status recorded when that SHA was first published; historical notes below remain as a record of what was known then. A fresh full-tree comparison of these same SHAs found **2,051 source blobs**, **2,109 Extended blobs**, **1,970 identical shared paths**, **81 customized shared paths**, **58 Extended-only paths**, **zero missing source paths** and **zero type/mode mismatches**. These results establish tracked-file coverage and passing CI, **not** functional parity of the 81 differences or native Settings On/Off and external Connect/MCP/bots/cloud/VM/credential workflows. Extended **0.12.0 remains DRAFT**, not an approved or distributed release.

The corrected Cargo.lock passed the exact-SHA Rust build/test and RustSec jobs. npm audits also passed. This is bounded CI evidence, not proof of absence of all vulnerabilities or a new remediation beyond the prior lockfile commit.


## 2026-11-14 — Cargo.lock consistency repair (validation pending)

The frozen fork [`6c42278ff598`](https://github.com/ginmp8/pandacode/commit/6c42278ff5981e50c9dc494986b4380123d4686d) still declares app **0.3.125**. The previous Extended [`c18ffde`](https://github.com/ginmp8/pandacode-extended/commit/c18ffdebe2ecbaf2b2b4d0741963087be006090c) failed both [push CI #43565065522](https://github.com/ginmp8/pandacode-extended/actions/runs/43565065522) and [PR CI #43565071057](https://github.com/ginmp8/pandacode-extended/actions/runs/43565071057) (**4/11 passed, 7/11 failed per run**): Rust `cargo --locked` rejected an outdated `src-tauri/Cargo.lock`; npm, website/TypeScript, Expo and RustSec audit jobs passed. The Linux diagnostic executed Cargo metadata without `--locked` and produced a lockfile diff. This candidate applies the exact generated Git blob `b9922223a60fc1da5628307009569758f20fb1a3` for `Cargo.lock`, with **zopfli 0.8.4 added**, four unneeded package records removed and no existing package-version replacements. The new commit requires fresh exact-SHA CI; Rust builds and native Settings On/Off, Connect/MCP/bots/cloud/VM/credential validation are not yet established. **Extended 0.12.0 remains DRAFT; no release is authorized.**

The authoritative Linux `cargo metadata --format-version 1 --manifest-path src-tauri/Cargo.toml` repair was captured from the failed CI job, not generated in this sandbox. It reconciles dependency selectors after `tauri-plugin-updater 2.14.1` while retaining Extended hardening. The old lock blob was `9724248cac0ce2ab26a7ece9d3f64fc032221ad4`; the corrected blob matches the runner's generated Git diff hash. No fresh vulnerability scan for the corrected commit has yet completed.

## 2026-11-14 — Tauri updater certificate trust and hardened lock reconciliation

Frozen [source `6c42278ff598`](https://github.com/ginmp8/pandacode/commit/6c42278ff5981e50c9dc494986b4380123d4686d) declares application **0.3.125**, with **five upstream commits** after the earlier `9739736d` source. The candidate incorporates **62 changed upstream paths** (47 source-exact modifications, three source-exact additions, 12 semantically reconciled shared paths), including Cursor CLI/ACP, Antigravity HTTP MCP fallback, OS-certificate-aware Tauri updater 2.14.1, related code, tests and original source release notes. Candidate tree verification found **2,051/2,051 upstream paths present** among **2,109 Extended tracked files**, 1,970 source-identical shared, 81 customized shared and 58 Extended-only paths; no mode/type mismatches. This is **static coverage**, not completed On/Off or native functional acceptance. The [previous Extended `5530431964e1`](https://github.com/ginmp8/pandacode-extended/commit/5530431964e114b90c4464450fde63bf4682544b) previously passed [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43544433263) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43544441354) (11/11 jobs each); **this new candidate has no CI result yet**. No release has been approved; planned Extended **0.12.0 remains DRAFT**.

The source updates `@tauri-apps/plugin-updater` to **2.14.1** and `tauri-plugin-updater` to **2.14.1** (OS certificate store via reqwest 0.13; Rust 1.90 required). Extended keeps updater default features disabled with explicit `rustls-tls`, preserves SQLite `rusqlite 0.41.1`/`libsqlite3-sys 0.39.1`, `rustls 0.23.46`, Tokio 1.53.3 and prior TLS and website remediations; direct ZIP pin advances **9.0.5 → 9.5.1** for dependency compatibility. Rust lock graph was checked for missing references and unreferenced packages, but **`cargo check --locked`, Clippy, native signed update testing, npm install/build/audits and security scans have not run for the candidate SHA**.

## 2026-11-13 — verified website Astro correction CI

For code [`19ec683`](https://github.com/ginmp8/pandacode-extended/commit/19ec683abce68d9951d5fca660c852494d435b5e), [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43507810199) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43507821448) both finished **success (11/11 jobs each)**. The recorded workflow steps `npm ci --prefix website`, `npm run site:build`, and `Audit website npm dependencies` all passed. The Extended website retains Astro **7.4.7** (instead of source Astro 7.3.3), while preserving existing hardened dependencies. This closes the previously failing website audit for this exact code SHA; it does **not** establish that every dependency is risk-free, that external services were tested, or that the subsequent documentation commit's CI has passed. Extended 0.12.0 remains a DRAFT.


## 2026-11-13 — verified website Astro audit failure and patch candidate

At [Extended `ea9dda3`](https://github.com/ginmp8/pandacode-extended/commit/ea9dda3db12bd04bf27712b11ced084f99fb9b1c), both [push CI #43498814241](https://github.com/ginmp8/pandacode-extended/actions/runs/43498814241) and [PR CI #43498824064](https://github.com/ginmp8/pandacode-extended/actions/runs/43498824064) failed **Dependency audit (npm lockfiles)** on website Astro 7.3.3. The audit reported [GHSA-mh32-jxcr-mh9f](https://github.com/advisories/GHSA-mh32-jxcr-mh9f) (dynamic tag XSS, patched in Astro 7.4.5) and [GHSA-r3h5-4585-7x3w](https://github.com/advisories/GHSA-r3h5-4585-7x3w) (image endpoint path checking, patched in Astro 7.4.7). Root and relay npm audit steps had no findings in those jobs. This is a concrete dependency issue, not a reason to disable website functionality.

The candidate [`7ccbb7c`](https://github.com/ginmp8/pandacode-extended/commit/7ccbb7c45ba039b3a86d224bb47d3eb8038e28a9) updates `website/package.json` to `astro ^7.4.7` and regenerates the website dependency graph using resolved npm URLs and SHA-512 integrity values from an existing [7.4.7 lock snapshot](https://github.com/conversejs/converse.js/blob/0c6ca4e65686d34d3fe455357e726aeb44f9b28c/docs/package-lock.json). The graph has 282 package entries with no unresolved nonoptional edges and preserves the Extended Sharp **0.35.5** remediation. The app release metadata remains **0.3.125**, planned Extended **0.12.0 DRAFT**. Local npm registry DNS failed with `EAI_AGAIN`; the candidate must still pass `npm ci`, website build and fresh exact-SHA npm audit in CI before claiming remediation verified.

## 2026-11-13 — upstream 0.3.125 reconciliation

Frozen upstream [`9739736d134ba29c6a07d92eca5745c3c4e5999b`](https://github.com/ginmp8/pandacode/commit/9739736d134ba29c6a07d92eca5745c3c4e5999b), tree `275bef5585e153bee54345166f9fafadea2c281e`, application **0.3.125**. Three upstream commits since `fd5942ec358a` changed **15 paths** (11 source-exact and four Extended-aware version/lockfile adaptations; zero deletions). Candidate inventory: **2,048 source / 2,106 Extended files; 1,968 exact shared / 80 customized shared / 58 Extended-only; zero missing source paths or mode/type differences**. Planned Extended **0.12.0 remains DRAFT**. This records a candidate, not exact-new-SHA CI success or native Settings acceptance.

Native app version metadata is aligned to 0.3.125 in npm/Cargo/Tauri. Preserve Extended-only `dompurify ^3.4.17`, `katex ^0.18.2`, `vite ^7.3.7`, Cargo `tokio 1.53.3`, `rusqlite 0.41.1`, `rustls 0.23.46`, `zip 9.0.5`, and updater feature overrides; no dependency rollback or unrelated package refresh was performed. New-SHA npm and RustSec audits are pending CI; never infer zero vulnerabilities from a version-only change.

## 2026-11-13 — completed 0.3.124 integration CI verification

For [Extended `cd182962990966bea002798c25b2218a1df78956`](https://github.com/ginmp8/pandacode-extended/commit/cd182962990966bea002798c25b2218a1df78956), [Push CI #43413789980](https://github.com/ginmp8/pandacode-extended/actions/runs/43413789980) and [PR CI #43413799925](https://github.com/ginmp8/pandacode-extended/actions/runs/43413799925) completed successfully (**11/11 jobs each**), including the named npm lockfile dependency-audit and Cargo RustSec audit jobs. The inherited application version remains 0.3.124, with Extended remediations retained in the divergent manifests and locks. This records CI evidence, not a new remediation or proof that every dependency risk is absent. No upstream source change occurred after [fork `fd5942ec358ac3cad0721524284e41904a99218e`](https://github.com/ginmp8/pandacode/commit/fd5942ec358ac3cad0721524284e41904a99218e); planned Extended 0.12.0 remains DRAFT.

## 2026-11-13 — 0.3.124 package alignment without hardening rollback

Source [fd5942ec358a](https://github.com/ginmp8/pandacode/commit/fd5942ec358ac3cad0721524284e41904a99218e), tree 675c46eeb3768952b6ca6a296184ec7cad5921ef, app **0.3.124** (six commits after preceding pin), changes 11 upstream paths: seven exact source blobs and four version/lockfile adaptations preserving Extended dependency remediation. Full inventories: 2,047 source files, 2,105 Extended, 1,967 exact shared, 80 divergent shared pending semantic audit, 58 Extended-only, zero missing or mode/type differences. Planned Extended **0.12.0 DRAFT** unchanged.

The source delta changes only version fields in npm/Cargo/Tauri manifests and locks. Extended preserves npm remediations (dompurify 3.4.17, katex 0.18.2, vite 7.3.7 and their resolved locks) and Rust controls (tokio 1.53.3, rusqlite 0.41.1, rustls 0.23.46, zip 9.0.5, updater TLS configuration and resolved lock adjustments). No new vulnerability remediation is claimed. Prior destination f7076cd6b0ae passed [Push CI 11/11](https://github.com/ginmp8/pandacode-extended/actions/runs/43390834157) and [PR CI 11/11](https://github.com/ginmp8/pandacode-extended/actions/runs/43390844083). New SHA CI must be checked after publication. Local Rust/Node/Expo tests not run (no checkout, no Cargo, GitHub DNS unavailable). Native Settings On/Off and external integrations unverified.

## 2026-11-13 — dependency and CI status following upstream repair

Source [`3026a7af132c`](https://github.com/ginmp8/pandacode/commit/3026a7af132c5b7684be414040a2d4f7158f80e4), tree `e50b17094d98c5631a819b67dd36d49929615546`, has **one additional upstream commit after the 0.3.123 release commit**, without an application-version change. Four existing source-exact paths were updated byte-for-byte: `docs/crash-recovery.md`, `releases/0.3.123.md`, `src-tauri/src/crash_guard/files.rs`, and `tests/crash-recovery.test.mjs`. Full nontruncated inventories: **2,046 upstream / 2,104 Extended tracked files**, **1,966 exact shared / 80 previously customized shared / 58 Extended-only**, **zero missing upstream paths or mode/type mismatches**. No Extended controls, dependency manifests, or app-version fields were changed; **Extended 0.12.0 remains a DRAFT**, separate from inherited app **0.3.123**.

There are **no changed npm/Cargo manifests or lockfiles** in this source delta and no newly claimed vulnerability remediation. The previous destination CI completed npm and RustSec audit jobs successfully in both runs, but full CI failed Node tests and Windows Clippy. The upstream fix narrows the compile-time Unix constant and strengthens test determinism without relaxing existing privacy/auth/approval controls.

The preceding destination commit `d3858b2e4462ce5a26d4b71d57e0b1590289c53e` failed both [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43369851585) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43369858494): two timezone-sensitive Node crash-notice assertions and Windows x64/arm64 Clippy's unused `LOG_FRESH_WITHIN`. The exact upstream correction pins the test timezone, permits the intended length-based notice trimming, and marks the constant Unix-only. Those source fixes directly address the observed failure signatures; **CI for the newly published destination commit is not yet known at this documentation checkpoint**. The eight other jobs in each previous run succeeded; that does not make the full run green. Local Node, Rust, Clippy, Expo and native installed-app tests were not executed (GitHub DNS unavailable; Cargo absent).


## 2026-11-13 — crash recovery dependencies and privacy

Source [`8001e5b98ed3`](https://github.com/ginmp8/pandacode/commit/8001e5b98ed3be0c6fb207fccfeada726d736e3c), tree `6f5a537dbd1c90ecd3e46132a990f974077774db`, declares app **0.3.123**; **5 upstream commits** since `420a163d3b92`, **69 changed paths (54 source-exact, 15 Extended-aware)**. Candidate inventory: **2,046 upstream / 2,104 Extended files; 1,966 identical shared / 80 customized shared / 58 Extended-only; zero upstream missing or mode/type differences**. Original [upstream release note](https://github.com/ginmp8/pandacode/blob/8001e5b98ed3be0c6fb207fccfeada726d736e3c/releases/0.3.123.md) retained byte-exact. Planned **Extended 0.12.0 remains DRAFT**. Prior destination [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43363128304) completed successfully; CI for the new candidate must be checked at its final published SHA.

The source's `Cargo.toml` now directly declares `rfd` and `dirs` for the native startup/restart path. Its lockfile changes and app-version alignment were reconciled with Extended's existing remediation versions, overrides and vendor patches; no vulnerability remediation is claimed without audit evidence. Crash reports and unsent drafts are stored as local plaintext and may contain sensitive work or secrets. Verify platform file permissions, retention, clear-drafts behavior, RustSec/npm audits and native functionality against the published SHA.

**Validation:** complete Git tree/blob/mode inventory and source-hunk reconciliation only. Local Node/Rust/Clippy/Expo, installed-app macOS/Windows/Linux, crash-dialog, Settings On/Off and external integration tests were not executed in this environment. GitHub CI pending/failed jobs must not be reported as passing.


## 2026-11-13 — source 0.3.122 version and dependency continuity

Source [`420a163d3b92`](https://github.com/ginmp8/pandacode/commit/420a163d3b921fd113e3d9ecc8437410a5ff7c6c), Git tree `bdca4b5ba5a32b3e54c7beaf38db8c989889584f`, declares application **0.3.122**. Three upstream commits since `54df76875360` modify **87 paths**: **73 source-exact** imports and **14 reviewed hunk-level Extended-aware reconciliations**. Candidate full-tree inventory: **2,011 upstream files / 2,069 Extended files; 1,931 byte/mode-exact shared, 80 previously customized shared, 58 Extended-only; zero missing upstream or type/mode discrepancies**. The original [upstream 0.3.122 note](https://github.com/ginmp8/pandacode/blob/420a163d3b921fd113e3d9ecc8437410a5ff7c6c/releases/0.3.122.md) is preserved byte-for-byte. The planned distribution version remains **Extended 0.12.0 DRAFT**, not a release.

The source advances **only the app version** from 0.3.121 to 0.3.122 in root `package.json`, root and `packages[""]` package-lock versions, `src-tauri/Cargo.toml`, the `pandacode` Cargo.lock package and Tauri config; it does **not** introduce a dependency-version change in this delta. Extended's remediated npm/Cargo dependency versions, overrides, vendor patches and lockfile differences were retained in the four overlapping manifest/lock paths. No new vulnerability, remediation, audit pass or release approval is asserted. Native npm/RustSec and build checks must be evaluated at the final commit SHA.

## 2026-11-12 — upstream 0.3.121 version and dependency continuity

Source [`54df76875360`](https://github.com/ginmp8/pandacode/commit/54df768753607f69f42b0c148a4ef665cb0f45a1) advances the application's declared version from **0.3.120 to 0.3.121** in the root npm manifest/lock (`version` and `packages[""].version`), Rust package/Cargo lock application entry and Tauri config. The source delta contains **no dependency version updates**; the existing Extended npm/Cargo remediations, overrides and vendor patches are retained rather than replaced by source lockfiles. The original upstream release notes and website release-date entry are incorporated exactly. No new vulnerability, remediation, audit pass or distribution approval is claimed.

Pre-integration Extended `bf20fce79523` completed 11/11 push and 11/11 PR CI jobs; the **new exact-SHA** npm/RustSec, Node, Rust, Clippy and Expo results are still to be verified. No local native or security audit ran in this environment. Extended **0.12.0 remains DRAFT**.

## 2026-11-12 — inherited 0.3.120 limitation notes

Pinned [upstream `f66e95267701`](https://github.com/ginmp8/pandacode/commit/f66e95267701750696d4a43eabdd6c74f6b56927), source tree `085808f3e028ff6ad4b515d562d65c6485f0b921`, app **0.3.120**. The sole source change since the previous snapshot is a five-bullet addition to the [original release note](https://github.com/ginmp8/pandacode/blob/f66e95267701750696d4a43eabdd6c74f6b56927/releases/0.3.120.md), blob `f49dba032c6236f85ee88642221c35a2a0b25275`. No new application, manifest, lockfile, test, workflow or Extended-control change was introduced. Git trees retain 2,007 upstream paths / 2,065 Extended paths, zero upstream files missing or mode/type mismatch, and 80 previously customized shared paths after this note is mirrored. Extended **0.12.0 remains DRAFT**, not released.

No dependency upgrades, overrides, lockfile changes or remediation rollbacks accompany this upstream documentation amendment. The documented classifications are inherited UI heuristics, not confirmed dependency vulnerabilities; retain independent npm/RustSec and platform validation.


## 2026-11-12 — source 0.3.120 version-only synchronization

Source [ginmp8/pandacode:main `69155055e1a9`](https://github.com/ginmp8/pandacode/commit/69155055e1a962c0af4b0cf041af0b1f87291588), Git tree `e6dfbb33ad2d03dc51005b21ae76f873028c7a54`; app version **0.3.120**. Since pinned source `b026526122c4` (0.3.119), three commits change **54 upstream paths: 49 imported byte/mode-exact, five adapted** (Extended release guidance and npm/Cargo manifests/locks). Candidate full-tree inventory: **2,007 upstream files, 2,065 Extended files, 1,927 shared identical, 80 shared divergent, 58 Extended-only, zero missing upstream paths or mode/type differences**. The other Extended code change repairs an existing Rust syntax error in `src-tauri/src/architect.rs:1436` (escaped `"auto"` in the dispatch description), independently reproduced in failed [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43266025146) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43266044987) on previous SHA `e0beb365b279`. New SHA CI and native functional equivalence remain to be checked. **Extended 0.12.0 is DRAFT**, not a published release.

The upstream delta changes application version metadata but makes no package-graph upgrades. Extended `package.json`, root `package-lock.json`, `src-tauri/Cargo.toml`, and the application entry in `src-tauri/Cargo.lock` update only `0.3.119` to `0.3.120`, retaining their existing remediated dependencies/overrides. Tauri configuration is copied source-exact. Rust syntax repair at `architect.rs:1436` changes a tool-description string delimiter, not tool authorization behavior. The preceding SHA passed npm and RustSec audit jobs, but Rust compilation failed; the new SHA must be audited/tested separately. No new vulnerability claim or native compatibility proof is made.


## 2026-11-12 — pinned 0.3.119 dependency and schema compatibility

Pinned source: [ginmp8/pandacode:main at `b026526122c4`](https://github.com/ginmp8/pandacode/commit/b026526122c45264f389c04f91a4e96010948926), Git tree `ba8cfd56d9eb91fe433abc1e9377cb069a8f848e`, native application version **0.3.119**. Compared with the last synchronized 0.3.117 source, 44 commits changed 132 paths; the reconciled candidate incorporates 109 source-exact paths and 23 Extended-aware adaptations, with zero upstream paths absent or type/mode discrepancies. Its full inventory is 2,002 upstream files, 2,060 Extended files, 1,922 shared byte-exact, 80 shared divergent and 58 Extended-only. Historical shared differences are not yet all behaviorally proven. The Extended distribution stays **0.12.0 DRAFT**; no release, tag or public package is authorized.

The upstream and Extended branches independently used SQLite schema version 37 for different changes. The candidate advances the effective schema to **38** and runs both the existing Extended cloud/bot compatibility work and upstream saved-remotely schedule provenance migration for databases from either v37 lineage. A regression test for the Extended-v37 upgrade path was added; **Rust migration tests have not yet executed**. This is a required compatibility adaptation, not an upstream feature removal.

Upstream npm/Cargo/Tauri application version metadata is synchronized to 0.3.119 independently of this Extended distribution version. Existing Extended dependency patches, lockfile remediations and their consumers are retained rather than overwritten by upstream lock versions. No new dependency-vulnerability clearance, npm audit or RustSec audit is claimed for this candidate; review the exact published-SHA CI results. Full bot/MCP/cloud behavior and migration execution need native verification.


## 2026-11-12 — application metadata 0.3.117 and storage compatibility

The [pinned source](https://github.com/ginmp8/pandacode/commit/9648d1c4c9e68f86265b9be04dfb1a8c1f960fb4) advances six app-version fields in root npm manifest/lockfile, Cargo manifest/application lock entry and Tauri config to **0.3.117**. Only source version hunks were applied to the four Extended-customized npm/Cargo files; Extended dependency overrides, remediations and security audit workflows were retained. The original Tauri version update is byte-exact. No new vulnerability remediation is claimed.

Upstream introduces restricted dismissal of stored error rows: only matching session error IDs, a 500-ID batch cap, and no deletion for mirrored remote tasks. Integration preserves Extended's existing SQLite schema version **37** instead of restoring source version 36. Security/regression tests for the published SHA and native Connect, remote mirrors and data persistence remain unverified; Extended **0.12.0 DRAFT** is not a released build.

## 2026-11-12 — source 0.3.116 version and security continuity

The [upstream 12a0468948c2](https://github.com/ginmp8/pandacode/commit/12a0468948c2ff1c9bc6be0bebf3f85411d73819) update (tree 1fc1d5aec803b0c6242d6293eceebcfff0982229) advances the six npm/Cargo/Tauri application version fields to **0.3.116** without changing Extended **0.12.0 DRAFT** distribution planning. Root package.json, package-lock.json root and workspace, src-tauri/Cargo.toml, the application entry in src-tauri/Cargo.lock, and src-tauri/tauri.conf.json are aligned. Existing Extended package constraints, overrides and Cargo dependency remediations are retained by applying only upstream version hunks to customized manifests. No vulnerability remediation is claimed from this import.

Upstream image serving includes task-scoped authorization, safe path constraints and byte/downscale limits; those source safeguards were imported. Native security acceptance, npm audit, RustSec, Rust tests/Clippy and exact published-SHA CI remain unverified here.
## 2026-11-12 — upstream 0.3.115 metadata and Skills safeguards

Fork [`7171e9606e48`](https://github.com/ginmp8/pandacode/commit/7171e9606e48b3645b5c4a1a6aac01386d5a6369), full tree `dd913037d13509ca9da764d77f9cb9eea4b1c4a9`, app 0.3.115, 15 source-changed paths (11 exact, four version-only Extended dependency-preserving adaptations). Planned Extended 0.12.0 remains DRAFT. Prior `64cd597d` push and PR CI each passed 11/11 jobs; the new integration commit needs independent exact-SHA CI.

Root npm, lockfile root and workspace, Cargo package/lock and Tauri versions advance to **0.3.115**. There are no upstream dependency changes in the two source commits; existing Extended versions, overrides and vendor patches are preserved rather than overwritten by the upstream manifests. Upstream Skills project-path checks and rollback improvements are inherited product behavior, not evidence of a new Extended-specific vulnerability fix. RustSec/npm scans for the final integration SHA remain to be verified.

## 2026-11-12 — source app 0.3.114

Source [`650537ae449d805656f531fc6fdb5dec09eceb80`](https://github.com/ginmp8/pandacode/commit/650537ae449d805656f531fc6fdb5dec09eceb80) advances the application-version fields to **0.3.114** in npm, Cargo and Tauri. This does not advance the planned Extended **0.12.0 DRAFT** release. Existing Extended package overrides, vendor patches and remediated Cargo dependencies are intentionally preserved; no new remediation is claimed. Dependency audits for the resulting commit require exact-SHA CI verification.

## 2026-11-12 — unchanged dependency manifests

[fork `9aa88776`](https://github.com/ginmp8/pandacode/commit/9aa88776f07c2274134e4a952ffcd7da71e70e50), tree `bd360c6d9d3d6f8bd209a8408914c4e1b80f85ef`, retains native app **0.3.112** across npm, Cargo and Tauri metadata. No package manifests or lockfiles changed in these two source commits. CI installs Linux `libasound2-plugins` for JACK smoke coverage, retains Extended npm/RustSec audits and requires a genuinely executed audio test. This does not establish a newly fixed vulnerability or new-SHA CI success. Previous Extended `c2c3789c` push CI passed 11/11 jobs.


**Status:** Opening baseline inherited from [0.11.0](../0.11.0/dependency-security.md); no new remediation is claimed for the 0.12.0 cycle.

Preserve the existing security remediations and compatible overrides, including the previously documented Rust `zip` 9.0.5 update (first applied in the 0.10.0 cycle). Do not describe inherited patches as new security fixes. The fork's native application version is 0.3.101 at opening; this is independent of Extended 0.12.0 documentation.

## 2026-11-12 source/CI identity correction

[fork `9e5d0988`](https://github.com/ginmp8/pandacode/commit/9e5d0988970785a9302938ecdc8d8848b7d79a7f) updates the real-audio smoke test only. Source and Extended still declare application **0.3.111** in all six npm/Cargo/Tauri fields; no manifest or lockfile changed. Previous `e1cd7e75` CI passed 10/11 jobs in each run, including Rust/RustSec, npm, Clippy and Expo; the lone release-provenance fixture failed on an obsolete original-note hash. Updated `source-baseline.json` to the actual upstream blob `357b3614`, preserving the upstream release note and strict test. New exact-SHA CI unverified.

## 2026-11-12 dependency and CI status

The unreleased fork [`8e5ab59c`](https://github.com/ginmp8/pandacode/commit/8e5ab59cdd22344a08e199dd94e6674af71ac7bd), tree `4fccf8f62aad4a59e3a03005db469819394713f4`, keeps application **0.3.111** unchanged and modifies two source paths. `releases/0.3.111.md` is mirrored exactly; CI is reconciled for the fully qualified ignored release-profile audio test with `--exact` and APT fetch retries `Acquire::Retries=3`, retaining Extended's audits, mobile checks, ARM64 recovery and macOS timeout. The Extended regression test now checks the real Linux/macOS capture requirement and both exact test commands. No upstream product capability or dependency version changed. Six npm/Cargo/Tauri application-version fields match upstream **0.3.111**; no dependency manifest changed. Previous Extended `e059e9a08623` [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43159765950) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43159773384) each reported a failing Node web-test job (526/527 passed): its stale assertion required macOS-only capture despite Linux/macOS now being required. Corrected without weakening coverage. Other jobs were running at inspection; new-SHA CI and real Settings On/Off, microphone and integration acceptance remain pending.

## Required checks for every imported upstream change

- [ ] Compare npm, Cargo, mobile/Expo and other lockfiles and manifests with the exact upstream commit, preserving remediations unless replaced by a verified safe alternative.
- [ ] Run RustSec, npm audit, TypeScript/website, Rust check/tests/Clippy, mobile/Expo and relevant native build checks at the new exact commit.
- [ ] Check updates to dependency overrides, MSRV/toolchain requirements, supported architectures and lockfile consistency.
- [ ] Trace each confirmed advisory to affected and fixed versions, applicability and mitigation evidence; do not claim exploitability without support.
- [ ] Verify that Extended tag/release identity remains separate from the upstream native installer version and that no npm publishing job was introduced.
- [ ] Track installation, update/rollback, signing, SBOM and artifact provenance separately from CI unit-test success.

| Evidence | State |
| --- | --- |
| Baseline | [0.11.0 dependency/security documentation](../0.11.0/dependency-security.md) |
| Prior source validation | [0.11.0 `main` CI #42392453324](https://github.com/ginmp8/pandacode-extended/actions/runs/42392453324), 6/6; **not** a future 0.12.0 test |
| New 0.12.0 fixes | None verified at cycle opening |
| Final 0.12.0 dependency and native validation | Pending |

Keep all future remediation notes source-linked, version-specific and reproducible.

## Upstream 0.3.102 integration — 2026-11-08

The [fork source `7134edf5`](https://github.com/ginmp8/pandacode/commit/7134edf501d31acc8bec28ff74be9d1dba171669) updates the upstream application's version metadata from 0.3.101 to **0.3.102** in root npm, Cargo package/lock and Tauri manifests; those fields were synchronized in Extended. The source change did not introduce dependency package version changes in the compared manifests. The existing Extended dependency overrides/remediations were preserved by applying only the source release-version hunks. No new security remediation is claimed. npm audit, RustSec, Rust/TypeScript builds and native packaging were not executed locally; CI for the integration commit must be checked by SHA before claiming it passed.

## PandaCode 0.3.103 integration

Upstream [`174f0c0b`](https://github.com/ginmp8/pandacode/commit/174f0c0b17ae1b036210cbc5dddf45f0701cd8b0) advanced the **native application version to 0.3.103** in npm root and lockfile, Cargo manifest and app lock entry, and Tauri configuration; Extended synchronizes these six fields without changing the planned distribution cycle 0.12.0. Existing Extended package/lock overrides and dependency remediations were retained through nonoverlapping three-way merges. The upstream feature introduced Council tests and implementation, not a separately verified dependency security fix.

No local `npm audit`, RustSec, Rust builds, mobile tests or installed-app checks ran in this session. Evaluate CI at the final published SHA and keep any native-platform, signed-updater, migration or approved cloud/credential tests separately pending.

## PandaCode 0.3.104 dependency/build reconciliation

The [source `d5d51769`](https://github.com/ginmp8/pandacode/commit/d5d51769ccac15145e67c071ea06a5502c4cadcb) changes six native application-version fields to **0.3.104**; the [Extended integration `6b6a39a8`](https://github.com/ginmp8/pandacode-extended/commit/6b6a39a82f508360572518519badb56e3df8313e) aligns them while preserving Extended npm/Cargo version overrides, including DOMPurify, KaTeX and Vite, and existing build/release protections. The upstream Vite `tsconfigRaw` change avoids requiring Expo's base tsconfig when pure mobile-brand modules are compiled for desktop. The `pandacode-run.sh` preview launcher is inherited unchanged and executable; no new auto-publication step was added.

The source-specific build regression and brand tests were copied. No additional remediation or advisory closure is claimed. The exact-SHA [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42503275362) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42503280072) were in progress when first observed; final code and documentation commit results, npm/RustSec audits and installed-app validation must be checked separately. Local build/audit commands were not run.

## PandaCode 0.3.105 dependency/version reconciliation

The pinned [fork `f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665`](https://github.com/ginmp8/pandacode/commit/f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665) advances the native application version from **0.3.104** to **0.3.105**, in `package.json`, both package-lock root version fields, the app Cargo package/lock entries and `tauri.conf.json`. The [Extended code integration `36228b949aaf8648109e2f8f8da441a8735cee41`](https://github.com/ginmp8/pandacode-extended/commit/36228b949aaf8648109e2f8f8da441a8735cee41) applied those exact version hunks to existing remediated manifests, preserving Extended dependency overrides and lockfile history. The source comparison did not introduce unrelated dependency-version changes in those manifest hunks; no new vulnerability fix or advisory resolution is claimed.

CI for the code SHA was [queued or running](https://github.com/ginmp8/pandacode-extended/actions/runs/42554247514) at the first check. Local `npm audit`, RustSec, dependency rebuild and installed-platform tests were not run; a passing CI job, if observed, would still be narrower than manual provider/credential and installer acceptance.


## Upstream 0.3.106 scanner and dependency continuity

The fork's application version **0.3.106** is carried in npm, Cargo and Tauri metadata. Existing Extended dependency remediations, overrides and lockfile-only modifications remain preserved; this source update did **not** introduce or claim a newly fixed dependency vulnerability. The inherited release workflow retries transient Syft/Grype download failures and returns an explicit failed coverage step when an SBOM coverage report is missing. The Extended manual-publication gate (no automatic public release or npm publish) remains unchanged. Run npm and RustSec audits and inspect the exact published commit's CI jobs before recording a passed audit.

## Exact-SHA CI security finding — 2026-11-09

At [Extended integration `34d8fb4c`](https://github.com/ginmp8/pandacode-extended/commit/34d8fb4caa985f2ceb4815945dc58ed3042c8353) (upstream [`0bfd5738`](https://github.com/ginmp8/pandacode/commit/0bfd57383621d5aa001d3871018920855cabec39), app **0.3.106**), [push CI #42644925066](https://github.com/ginmp8/pandacode-extended/actions/runs/42644925066) and [PR CI #42644937837](https://github.com/ginmp8/pandacode-extended/actions/runs/42644937837) both **failed**: five of six jobs succeeded in each, but **RustSec audit failed**. The lockfile contains `rusqlite 0.38.0` and `libsqlite3-sys 0.36.0`, whose bundled SQLite is affected by [RUSTSEC-2026-0459 / CVE-2026-119375](https://github.com/RustSec/advisory-db/blob/main/crates/libsqlite3-sys/RUSTSEC-2026-0459.md) (CVSS 6.8). The advisory identifies patched `libsqlite3-sys >=0.39.0`, released with `rusqlite 0.41`; the application explicitly enables `rusqlite`'s `bundled` feature. Its exposure involves opening a crafted local database plus WAL file; the failed audit does not independently establish exploitability in a deployed installation.

**Unresolved, release-blocking dependency remediation:** upgrade the compatible `rusqlite`/SQLite dependency graph, regenerate and commit `src-tauri/Cargo.lock` using Cargo (not hand-edited), verify the Rust toolchain floor (`rusqlite 0.41` requires Rust 1.89), check any affected SQLite API/storage and migration paths, and rerun `cargo check --locked`, relevant Rust/SQLite regression tests, and `cargo audit --file src-tauri/Cargo.lock` before treating the finding as fixed. The local sandbox has no Cargo and cannot resolve GitHub DNS, so no validated lockfile regeneration or Rust test was possible. No vulnerability suppression, CI bypass, or dependency-version-only edit was applied. The source and destination Git trees still have **zero missing upstream paths**, but this does not prove functional equivalence of the 80 customized shared files or validate Settings/Connect/MCP/cloud/VM native behavior. Planned Extended distribution **0.12.0** remains unapproved; checks on any documentation-only follow-up SHA are separate.

## SQLite/RustSec dependency candidate — 2026-11-09

- **Remediation candidate:** `rusqlite` was advanced to **0.41.1** with `bundled` still enabled. Its dependency graph now locks **libsqlite3-sys 0.39.1**, **hashlink 0.12.2**, and the updated `hashbrown 0.17.1` dependency entry. The 686 resolved package sections are identical to the lockfile graph from read-only `extended/development` at [`d0100f72`](https://github.com/ginmp8/pandacode-extended/commit/d0100f72c25e3ccff58c294d624505af8a216b48), except for the application package version, which stays at **0.3.106**. The lockfile entries were reused from that pre-existing CI-tested graph, not invented or suppressed.
- **Reference validation (different branch):** [CI #42651770546](https://github.com/ginmp8/pandacode-extended/actions/runs/42651770546) passed both RustSec and Linux Rust check/tests on the pinned read-only branch. The run failed an unrelated UI Playwright job. Those passing jobs support the dependency graph but **do not prove the new base/development SHA passes**.
- **Remaining gates:** no local Cargo/Rust runtime is available here. Run exact-new-SHA RustSec, locked Rust/Clippy tests and native SQLite storage/migration checks before declaring remediation complete. The upstream remains [`7cadfdbeb700`](https://github.com/ginmp8/pandacode/commit/7cadfdbeb7006573897a16e1d9c15ac0a59febad); planned Extended **0.12.0** remains draft, with no tag or release. No upstream path or feature was removed.

## Exact-SHA RustSec remediation validation — 2026-11-09

On Extended [`06cb9b0c0141`](https://github.com/ginmp8/pandacode-extended/commit/06cb9b0c0141e964d43d0c462cbd30e9fc44ec1c), [push CI #42680289120](https://github.com/ginmp8/pandacode-extended/actions/runs/42680289120) and [PR CI #42680300162](https://github.com/ginmp8/pandacode-extended/actions/runs/42680300162) each concluded **success (6/6)**. The committed `libsqlite3-sys 0.39.1` / `rusqlite 0.41.1` graph passed the RustSec audit in both runs; Linux `cargo test --locked --lib` and `cargo clippy --locked --all-targets -- -D warnings` also passed, as did the remaining npm, web, mobile, and ONNX jobs.

This resolves the **CI audit failure at this exact code SHA**, not the separate native installation and SQLite migration/rollback acceptance gates. No Cargo commands were executed locally, no advisory suppression was introduced, and the upstream application version remains `0.3.106`. Future commits require their own exact-SHA checks.

## Upstream 0.3.107 import dependency evidence

The application manifests were synchronized to **0.3.107** while retaining Extended's existing compatible dependency remediation: `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`, including their lockfile graph. The new inherited Skills manager invokes the upstream-pinned Skills CLI `skills@1.7.6`; this is an upstream feature description, not an independent security approval. npm/RustSec audits, Rust locked tests/Clippy, and live CLI installation remain pending for the new destination commit. No unrelated dependency downgrade or advisory suppression was introduced.

## Upstream 0.3.107 dependency CI follow-up — 2026-11-10

Both [push CI #42805588366](https://github.com/ginmp8/pandacode-extended/actions/runs/42805588366) and [PR CI #42805595388](https://github.com/ginmp8/pandacode-extended/actions/runs/42805595388) completed **successfully (6/6 jobs each)** on the reconciled code commit [`e4e5c0c51daa8a0149015bb26a00c8fb09bb2050`](https://github.com/ginmp8/pandacode-extended/commit/e4e5c0c51daa8a0149015bb26a00c8fb09bb2050). All reported jobs passed: Linux Rust check/tests, RustSec, Node/TypeScript/unit tests and website, npm dependency audit, Expo/mobile compatibility and macOS ONNX parity.

The RustSec and npm jobs passed with the Extended lockfiles at this exact code SHA; this replaces the earlier pending CI state for this commit only. Do not infer that live Skills CLI installations, native SQLite upgrade/rollback, Windows/Linux platform behavior or future commits have been validated. No new dependency remediation or security approval is claimed.

## Upstream 0.3.108 voice-dictation integration — 2026-11-10

- Source: [`ginmp8/pandacode:main` `952d0acbe02547af036ef6d7635ec415a472eb3b`](https://github.com/ginmp8/pandacode/commit/952d0acbe02547af036ef6d7635ec415a472eb3b), tree `202f77088372d261b263b806354d84ce094f25b8`, native app version **0.3.108**; includes commits after the source's release commit. The planned Extended distribution documentation remains **0.12.0 DRAFT**.
- Reconciled the 109-path source delta relative to `5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1`: 78 additions (including Rust/frontend audio input, model catalog, tests and notices), 17 exact source updates and 14 changes reconciled with Extended behavior. The inherited voice feature supports optional on-device speech models and configured transcription services, transcription into the composer, explicit recording/stop and tests. This is not an Extended-only invention.
- Preserved Extended Feature controls, release identity and dependency remediation, including `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`. Native microphone permissions, service credentials, model downloads, user-cancelled recording, offline transcription and Settings states need separate behavior checks.
- This section describes a source integration **candidate**, not evidence of a successful new-SHA CI run. Check the eventual exact commit SHA for Rust/Linux/Windows/macOS, RustSec, Node, npm, Expo, installer/platform and native feature-control acceptance. Historic successful CI applies only to the earlier SHAs.

The new voice pipeline introduces native `cpal`, `rubato`, `transcribe-cpp` and platform audio/CMake dependencies. The Extended Rust lockfile retains the prior remediated SQLite versions and must pass `cargo check --locked`, RustSec and all platform CI rows before any distribution approval.


## Upstream 0.3.108 post-version CI gate — 2026-11-10

The seven changed source paths at [`57f0aaf7cfc3`](https://github.com/ginmp8/pandacode/commit/57f0aaf7cfc35419d4a21b9823bbdf35dc73cfd1) do **not** change package or Cargo manifests/lockfiles. Existing Extended SQLite remediation (`rusqlite 0.41.1`, `libsqlite3-sys 0.39.1`) remains in place. Revalidate RustSec, locked Rust tests/Clippy and npm audit on the **exact new destination SHA**, and test native model downloads plus SQLite upgrade/rollback separately.

## Post-0.3.108 source follow-up — 2026-11-10

No npm/Cargo manifest or lockfile change; native app remains **0.3.108**, with Extended remediations `rusqlite 0.41.1` / `libsqlite3-sys 0.39.1` unchanged. Prior code SHA `f0c1cf82ff3261845eb2cd004088cc912cf4bf8f` passed both 11-job CI runs. Reverify RustSec, npm, and locked-build checks on the exact new destination commit; previous success is SHA-specific.

## Fork HEAD 979b7d4e integration — 2026-11-10

No package manifests or lockfiles changed in the six-path source delta. Existing Extended remediation (`rusqlite 0.41.1`, `libsqlite3-sys 0.39.1`) remains unchanged; no new vulnerability or remediation is asserted. Recheck RustSec, npm audit and locked Rust/Node builds on the new destination SHA; a previously cancelled ONNX/macOS push job does not demonstrate a dependency defect.

## Fork HEAD bc323966 integration — 2026-11-10

No npm/Cargo manifests or lockfiles changed in the 11-path upstream delta. Native application version remains **0.3.108** and Extended preserves `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`. The added Voice tests and CI gate changes do not claim a new vulnerability fix. Run RustSec, npm audits, Rust/Node locked checks and platform build verification against the **new destination SHA**; previous CI results cannot validate it.

## Mobile node-forge advisory remediation — 2026-11-10

The exact-SHA [push dependency-audit job](https://github.com/ginmp8/pandacode-extended/actions/runs/42920988289) on `c7c146067cfa6a176b2ee79a95f262b867ed175f` failed at `npm audit --prefix mobile --audit-level=low`: the mobile lockfile's **node-forge 1.4.2** is affected by [GHSA-6889-vf6x-f3g5](https://github.com/advisories/GHSA-6889-vf6x-f3g5), [GHSA-w4xr-hqv8-ch44](https://github.com/advisories/GHSA-w4xr-hqv8-ch44), [GHSA-6cv4-p8m7-4r25](https://github.com/advisories/GHSA-6cv4-p8m7-4r25), and [GHSA-j8rw-jp99-435v](https://github.com/advisories/GHSA-j8rw-jp99-435v). The GitHub-reviewed advisories specify **1.4.3** as the first fixed version. This is a confirmed dependency advisory and audit failure, not proof of a reachable application exploit.

The Extended mobile override and the one resolved `node_modules/node-forge` lock entry now target **1.4.3**, preserving Expo/CLI dependencies and other Extended overrides. The exact integrity (`sha512-P6wzfh6t70avmV8FrVEAd/36sOPJQ+hkaJVX1Laap9J/77m7M0eyozDvc8SwnDf4P7MP94gwbAAo0rg3YVXT+g==`) is sourced from the [npm registry's published metadata](https://registry.npmjs.org/node-forge/1.4.3). Local package install/audit was blocked by registry DNS. Confirm remediation with the **new exact-SHA CI**; keep installed-app and runtime acceptance separate.

## Verified node-forge remediation CI — 2026-11-10

The earlier mobile audit failure on `node-forge@1.4.2` was followed by the narrowly scoped lockfile and override update to **1.4.3** in [Extended commit `ab6c99787bf8ece8eae70b51eb3d071b91353e41`](https://github.com/ginmp8/pandacode-extended/commit/ab6c99787bf8ece8eae70b51eb3d071b91353e41). Both [push CI #42921416535](https://github.com/ginmp8/pandacode-extended/actions/runs/42921416535) and [PR CI #42921425457](https://github.com/ginmp8/pandacode-extended/actions/runs/42921425457) completed successfully with **11/11 jobs each** on that exact SHA, including the mobile dependency audit and Expo compatibility, root npm audit, RustSec and Linux Rust tests. This resolves the *observed audit/check failure* for the pinned commit, not an independently confirmed native-device security or compatibility test.

No new dependency version or advisory suppression was introduced by this documentation follow-up. Prior SQLite remediation (`rusqlite 0.41.1` / `libsqlite3-sys 0.39.1`) remains in the current lockfile. Later commits, installed-device behavior, SQLite migrations and all Extended control-state interactions require separate checks.

## Fork 0.3.109 audio dependency integration — 2026-11-11

The current source moves `cpal` from 0.16 to 0.17 (resolved to 0.17.3) to address a reported macOS release-build microphone enumeration crash and updates the CoreAudio/ALSA/Objective-C dependency closure in Cargo.lock. The Extended adaptation deliberately preserves previously remediated `rusqlite 0.41.1`, `libsqlite3-sys 0.39.1`, other Rust security constraints and JS/mobile `node-forge 1.4.3`. Compatibility of the composed lockfile, macOS optimized tests, npm audits and RustSec must be evaluated on the exact integration SHA; this note is not evidence of an independent security audit or installed-client acceptance.

## Fork application 0.3.111 and mobile package dependencies — 2026-11-11

The current source is [`a403c3d9`](https://github.com/ginmp8/pandacode/commit/a403c3d9bfd67ad4bb70eff3f1d2d4befebfa40e), application `0.3.111`. The candidate aligns npm, Cargo and Tauri application identity with this native version; the separate **Extended 0.12.0** distribution cycle remains DRAFT. Mobile adds `expo-document-picker ~58.0.4` and `expo-image-picker ~58.0.10` with merged lock records while preserving Extended's `deepmerge` patch. Root npm/Cargo locks retain Extended dependency remediations, including their security overrides; the upstream application-version-only changes in Cargo files are applied without downgrading those packages. These are static source/manifest checks, **not** evidence of npm audit, RustSec, installed-client compatibility, or a new vulnerability remediation.


## Mobile npm 10 clean-install failure after upstream 0.3.111 — 2026-11-11

At Extended [`d67a035d`](https://github.com/ginmp8/pandacode-extended/commit/d67a035d7a1a6e0635289b76ed1b07054263932f), the [push CI run](https://github.com/ginmp8/pandacode-extended/actions/runs/43028709840) and [PR CI run](https://github.com/ginmp8/pandacode-extended/actions/runs/43028720555) fail mobile `npm ci --prefix mobile --ignore-scripts` under npm 10.9.9. The error is `Invalid: lock file's brace-expansion@5.0.12 does not satisfy brace-expansion@5.0.13`. The Extended `minimatch` override is now scoped to the existing patched `brace-expansion 5.0.12` lock entry, preserving the `minimatch 11.0.3` remediation instead of downgrading or suppressing audit checks. A focused lockfile/override consistency regression was added. This is a correction candidate until `npm ci`, Expo, and the exact-new-SHA CI pass; installed-device functionality remains unverified.

## Fork HEAD `961b43fa` dependency continuity — 2026-11-11

The seven source-modified paths in [`961b43fa`](https://github.com/ginmp8/pandacode/commit/961b43fa88ad1d24eb083bc10779cb7034c4a0fd) include **no npm/Cargo dependency manifests or lockfiles** and no version-field changes; upstream native version remains **0.3.111**. The integration [`5beff6d8`](https://github.com/ginmp8/pandacode-extended/commit/5beff6d8737e952341023bfdf10aaeb38697fcae) retains the Extended dependency overrides, patched lockfiles, application identity and security remediations from the previous snapshot without attempting a downgrade. These facts reflect the seven-path source diff and exact tree inventory, **not** a fresh npm audit or RustSec result. Check the new commit's CI before claiming any automated audit success.

## Source 106b3852 dependency continuity — 2026-11-11

The six upstream changed paths do not include application manifests or lockfiles. The inherited native application version remains **0.3.111** in npm, Cargo and Tauri, separate from the planned Extended **0.12.0** distribution. Previously remediated dependencies, overrides, and Extended audit jobs remain unchanged; no newly fixed vulnerability is claimed. Audit and build results must be checked on the exact published commit rather than borrowed from prior commits.

## Source a2ba4b43 dependency continuity — 2026-11-12

The [single source commit](https://github.com/ginmp8/pandacode/commit/a2ba4b430951ee2995bdfda140d4f516cb493a78) edits only `releases/0.3.111.md`. npm/Cargo manifests, lockfiles, overrides, RustSec checks and installed dependency versions do not change; all six native app version fields remain **0.3.111**. No new vulnerability remediation, dependency downgrade or security-audit result is claimed. The new documentation-only destination SHA must be checked separately from previous successful CI.

## Source 802fa847 dependency and CI audit continuity — 2026-11-12

The two new fork commits change `.github/workflows/ci.yml`, `docs/voice-input.md` and `releases/0.3.111.md` only. They do not change package/Cargo manifests, lockfiles, overrides or native application version **0.3.111**. Existing Extended SQLite/dependency remediation, pinned CI actions, npm/mobile/RustSec audit jobs and APT recovery remain intact. Previous SHA `2fdacaa7` passed [push](https://github.com/ginmp8/pandacode-extended/actions/runs/43152555072) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43152561038) CI (**11/11 each**). New Linux release-profile audio CI and all audit jobs must be verified at the final commit; no new vulnerability remediation or local audit is asserted.

## Source 0.3.112 dependency checkpoint — 2026-11-12

Native upstream application metadata now declares `0.3.112` in the npm manifest/lock root and package entry, Cargo package/lock application entry, and Tauri configuration. No dependency upgrades were imported in the source delta. Extended remediations (including npm DOMPurify/KaTeX/Vite, and Rust SQLite/Tokio/TLS/ZIP/Updater adjustments) were preserved by modifying only the version fields in the existing Extended manifests and locks. No new vulnerability or remediation is claimed. The prior `17b8d4fa` CI passed npm and RustSec audit jobs; exact new-SHA audits and installed-app compatibility remain unverified at this documentation checkpoint.
