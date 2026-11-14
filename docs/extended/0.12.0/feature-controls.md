# Extended 0.12.0 — feature-control continuity

## 2026-11-14 — VM RFB import and existing controls

The three source changes in [upstream `da1ecb5fc452`](https://github.com/ginmp8/pandacode/commit/da1ecb5fc452bc8df313b4485da93d4d573e1e4d) affect only `src-tauri/src/vm/rfb.rs`, its Rust tests and `docs/agent-reference.md`; they do not edit Settings, permission gates, Cloud/VM tool selection, Connect, MCP, bots, credential-transfer controls or defaults. This import therefore retains those pre-existing Extended code paths rather than replacing them with source versions. Existing controls must continue restricting only their configured operations when Off and permitting upstream behavior when On. That functional contract and live VM screenshots have **not been independently exercised** for this candidate; prior CI success on [Extended `20795fc15cf4`](https://github.com/ginmp8/pandacode-extended/commit/20795fc15cf4cfd41aaf77a06c65be88757e939d) does not constitute On/Off acceptance.

## 2026-11-14 — verified CI for the Cargo.lock reconciliation

The frozen [source 6c42278ff598](https://github.com/ginmp8/pandacode/commit/6c42278ff5981e50c9dc494986b4380123d4686d) still declares application **0.3.125**. For [Extended 7d6897e87c3e](https://github.com/ginmp8/pandacode-extended/commit/7d6897e87c3e8a88c1ebb323783cb75a7071f65b), both [push CI #43568928848](https://github.com/ginmp8/pandacode-extended/actions/runs/43568928848) and [PR CI #43568932573](https://github.com/ginmp8/pandacode-extended/actions/runs/43568932573) finished **success (11/11 jobs each; 22/22 total)**. The jobs include Linux Rust check/tests, five Rust Clippy platform targets, macOS ONNX parity, TypeScript/unit/website checks, mobile/Expo, npm dependency audit and Cargo/RustSec audit. This replaces the *pending* status recorded when that SHA was first published; historical notes below remain as a record of what was known then. A fresh full-tree comparison of these same SHAs found **2,051 source blobs**, **2,109 Extended blobs**, **1,970 identical shared paths**, **81 customized shared paths**, **58 Extended-only paths**, **zero missing source paths** and **zero type/mode mismatches**. These results establish tracked-file coverage and passing CI, **not** functional parity of the 81 differences or native Settings On/Off and external Connect/MCP/bots/cloud/VM/credential workflows. Extended **0.12.0 remains DRAFT**, not an approved or distributed release.

The successful automated checks do not exercise every state and authorization transition behind the Extended controls; no On/Off setting was modified in this documentation-only checkpoint.


## 2026-11-14 — Cursor/ACP and updater control continuity

Frozen [source `6c42278ff598`](https://github.com/ginmp8/pandacode/commit/6c42278ff5981e50c9dc494986b4380123d4686d) declares application **0.3.125**, with **five upstream commits** after the earlier `9739736d` source. The candidate incorporates **62 changed upstream paths** (47 source-exact modifications, three source-exact additions, 12 semantically reconciled shared paths), including Cursor CLI/ACP, Antigravity HTTP MCP fallback, OS-certificate-aware Tauri updater 2.14.1, related code, tests and original source release notes. Candidate tree verification found **2,051/2,051 upstream paths present** among **2,109 Extended tracked files**, 1,970 source-identical shared, 81 customized shared and 58 Extended-only paths; no mode/type mismatches. This is **static coverage**, not completed On/Off or native functional acceptance. The [previous Extended `5530431964e1`](https://github.com/ginmp8/pandacode-extended/commit/5530431964e114b90c4464450fde63bf4682544b) previously passed [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43544433263) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43544441354) (11/11 jobs each); **this new candidate has no CI result yet**. No release has been approved; planned Extended **0.12.0 remains DRAFT**.

The Cursor CLI provider is wired into model/provider discovery, architect dispatch, channel bots, remote provider catalog, handoff and native ACP protocol paths. Antigravity HTTP MCP fallback and update checks remain subject to the existing Extended settings and upstream authorization/approval boundaries. No controls were intentionally disabled or defaulted more strictly. These are source-level observations; **On/Off, Connect/direct, new/existing processes, MCP sessions, cloud/VM/credential transfers and manual/automatic updater behavior need native testing**.

## 2026-11-13 — unchanged feature controls during website security patch

Only the website's Astro package manifest and npm lockfile are changed by [`7ccbb7c`](https://github.com/ginmp8/pandacode-extended/commit/7ccbb7c45ba039b3a86d224bb47d3eb8038e28a9). Automatic/manual updates, Connect, global bot pause, external MCP, cloud/VM operations, credential handoff and other Extended Settings retain their existing code and intended On/Off behavior. This file-level nonimpact observation is **not** new native runtime acceptance; all Settings/remote/provider integration tests remain outstanding.

## 2026-11-13 — upstream 0.3.125 integration candidate

Frozen upstream [`9739736d134ba29c6a07d92eca5745c3c4e5999b`](https://github.com/ginmp8/pandacode/commit/9739736d134ba29c6a07d92eca5745c3c4e5999b), tree `275bef5585e153bee54345166f9fafadea2c281e`, declares application **0.3.125**. Since `fd5942ec358a`, three upstream commits changed **15 paths: 11 source-exact copies (10 modifications and the original `releases/0.3.125.md`) and four Extended-aware manifest/lockfile version updates**, with no upstream deletions. Expected candidate inventory: **2,048 upstream / 2,106 Extended tracked files; 1,968 identical shared; 80 divergent shared; 58 Extended-only; no missing upstream paths or mode/type conflicts**. The four retained divergences protect Extended dependency remediations. **Planned Extended 0.12.0 remains DRAFT**, independent of the inherited app version. CI for the new destination commit and native control-path acceptance are **not yet available**; prior CI must not be attributed to the new SHA.

The 0.3.125 source delta does **not** modify the existing Extended-control code paths. Preserve current optional On/Off behavior, manual and automatic paths, and upstream behavior when controls are enabled; no native installation or end-to-end verification of these behaviors was run here.

## 2026-11-13 — completed 0.3.124 integration CI verification

For [Extended `cd182962990966bea002798c25b2218a1df78956`](https://github.com/ginmp8/pandacode-extended/commit/cd182962990966bea002798c25b2218a1df78956), [Push CI #43413789980](https://github.com/ginmp8/pandacode-extended/actions/runs/43413789980) and [PR CI #43413799925](https://github.com/ginmp8/pandacode-extended/actions/runs/43413799925) completed successfully (**11/11 jobs each**). Static comparison against [fork `fd5942ec358ac3cad0721524284e41904a99218e`](https://github.com/ginmp8/pandacode/commit/fd5942ec358ac3cad0721524284e41904a99218e) still shows 80 customized shared paths and no source path missing. Optional Extended feature controls remain present, but native On/Off paths, Connect/direct routing, pause/resume, external MCP, credential transfer and cloud/VM integration remain **unverified end to end**. Planned Extended 0.12.0 remains DRAFT. This evidence applies only to the named code SHA.

## 2026-11-13 — 0.3.124 Skills Library integration

Source [fd5942ec358a](https://github.com/ginmp8/pandacode/commit/fd5942ec358ac3cad0721524284e41904a99218e), tree 675c46eeb3768952b6ca6a296184ec7cad5921ef, app **0.3.124** (six commits after preceding pin), changes 11 upstream paths: seven exact source blobs and four version/lockfile adaptations preserving Extended dependency remediation. Full inventories: 2,047 source files, 2,105 Extended, 1,967 exact shared, 80 divergent shared pending semantic audit, 58 Extended-only, zero missing or mode/type differences. Planned Extended **0.12.0 DRAFT** unchanged.

The Rust Skills Library implementation, adapter and tests match upstream exactly; no new Extended control was introduced and no existing control removed. The optional update/manual, Connect/direct routing, bot pause, MCP, cloud-credential and VM-management controls are preserved in their existing files and behavior contracts. **No native On/Off verification performed**; the 80 historical divergences require semantic review. Prior destination f7076cd6b0ae passed [Push CI 11/11](https://github.com/ginmp8/pandacode-extended/actions/runs/43390834157) and [PR CI 11/11](https://github.com/ginmp8/pandacode-extended/actions/runs/43390844083). New SHA CI must be checked after publication. Local Rust/Node/Expo tests not run (no checkout, no Cargo, GitHub DNS unavailable). Native Settings On/Off and external integrations unverified.

## 2026-11-13 — source-only crash guard build/test fixes

Source [`3026a7af132c`](https://github.com/ginmp8/pandacode/commit/3026a7af132c5b7684be414040a2d4f7158f80e4), tree `e50b17094d98c5631a819b67dd36d49929615546`, has **one additional upstream commit after the 0.3.123 release commit**, without an application-version change. Four existing source-exact paths were updated byte-for-byte: `docs/crash-recovery.md`, `releases/0.3.123.md`, `src-tauri/src/crash_guard/files.rs`, and `tests/crash-recovery.test.mjs`. Full nontruncated inventories: **2,046 upstream / 2,104 Extended tracked files**, **1,966 exact shared / 80 previously customized shared / 58 Extended-only**, **zero missing upstream paths or mode/type mismatches**. No Extended controls, dependency manifests, or app-version fields were changed; **Extended 0.12.0 remains a DRAFT**, separate from inherited app **0.3.123**.

No Extended configurable behavior was changed by this four-file import. Previous optional controls for automatic/manual updates, Connect master/direct routes, global bot pause, external MCP, cloud credential transfer and agent-managed VMs remain in their existing code. The upstream crash recovery preference paths remain available; source inclusion alone does not validate native ON/OFF combinations or interactions with these Extended controls.

The preceding destination commit `d3858b2e4462ce5a26d4b71d57e0b1590289c53e` failed both [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43369851585) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43369858494): two timezone-sensitive Node crash-notice assertions and Windows x64/arm64 Clippy's unused `LOG_FRESH_WITHIN`. The exact upstream correction pins the test timezone, permits the intended length-based notice trimming, and marks the constant Unix-only. Those source fixes directly address the observed failure signatures; **CI for the newly published destination commit is not yet known at this documentation checkpoint**. The eight other jobs in each previous run succeeded; that does not make the full run green. Local Node, Rust, Clippy, Expo and native installed-app tests were not executed (GitHub DNS unavailable; Cargo absent).


## 2026-11-13 — crash recovery and Extended controls

Source [`8001e5b98ed3`](https://github.com/ginmp8/pandacode/commit/8001e5b98ed3be0c6fb207fccfeada726d736e3c), tree `6f5a537dbd1c90ecd3e46132a990f974077774db`, declares app **0.3.123**; **5 upstream commits** since `420a163d3b92`, **69 changed paths (54 source-exact, 15 Extended-aware)**. Candidate inventory: **2,046 upstream / 2,104 Extended files; 1,966 identical shared / 80 customized shared / 58 Extended-only; zero upstream missing or mode/type differences**. Original [upstream release note](https://github.com/ginmp8/pandacode/blob/8001e5b98ed3be0c6fb207fccfeada726d736e3c/releases/0.3.123.md) retained byte-exact. Planned **Extended 0.12.0 remains DRAFT**. Prior destination [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43363128304) completed successfully; CI for the new candidate must be checked at its final published SHA.

The upstream General → Crash recovery controls (automatic restart, keeping unsent drafts, clearing drafts) are preserved as upstream features. Extended controls for manual/automatic updates, Connect master/direct access, global bot pause, external MCP, cloud credential forwarding and cloud/VM management remain on their existing paths. One bridge overload conflict was resolved by retaining Extended `queue-changed` delivery metadata and adding upstream `app-quitting`. No new Extended feature restrictions or defaults are introduced. The combined behavior still needs native On/Off integration testing.

**Validation:** complete Git tree/blob/mode inventory and source-hunk reconciliation only. Local Node/Rust/Clippy/Expo, installed-app macOS/Windows/Linux, crash-dialog, Settings On/Off and external integration tests were not executed in this environment. GitHub CI pending/failed jobs must not be reported as passing.


## 2026-11-13 — source 0.3.122 feature-control continuity

Source [`420a163d3b92`](https://github.com/ginmp8/pandacode/commit/420a163d3b921fd113e3d9ecc8437410a5ff7c6c), Git tree `bdca4b5ba5a32b3e54c7beaf38db8c989889584f`, declares application **0.3.122**. Three upstream commits since `54df76875360` modify **87 paths**: **73 source-exact** imports and **14 reviewed hunk-level Extended-aware reconciliations**. Candidate full-tree inventory: **2,011 upstream files / 2,069 Extended files; 1,931 byte/mode-exact shared, 80 previously customized shared, 58 Extended-only; zero missing upstream or type/mode discrepancies**. The original [upstream 0.3.122 note](https://github.com/ginmp8/pandacode/blob/420a163d3b921fd113e3d9ecc8437410a5ff7c6c/releases/0.3.122.md) is preserved byte-for-byte. The planned distribution version remains **Extended 0.12.0 DRAFT**, not a release.

The upstream change introduces selectable per-model speed tiers and a local Alibaba Token Plan connection, not a new Extended-wide restriction. The 14 overlapping paths were patched at source hunks while preserving Extended's existing optional manual/automatic updates, Connect master/direct routes, bot pause, external MCP, cloud agent credential forwarding and cloud/VM agent management controls. **Off** still governs only its configured operation; enabling an existing control should retain the corresponding upstream path. This is source-level integration evidence only, not independent installed/native On/Off acceptance.

The source documents Alibaba Token Plan as Singapore-endpoint only, local desktop execution (not cloud machines), absent from the mobile provider list, with a placeholder logo. Claude Code speed changes during a running task apply from its next turn. Git tree/hunk checks do **not** prove runtime parity for all 80 customized shared files or native Settings On/Off, remote, MCP, bot and cloud/VM/credential behavior. Local Node, Rust, Clippy, Expo, installed-app and external integration tests were **not executed** in this environment. Check GitHub Actions against the final published Extended commit; CI pending is not success.

## 2026-11-12 — upstream 0.3.121 feature controls (candidate)

Source [`54df76875360`](https://github.com/ginmp8/pandacode/commit/54df768753607f69f42b0c148a4ef665cb0f45a1) adds **inherited** checks rather than new Extended switches. In ZCode Full mode, a lasting-only project permission is not silently auto-granted; a human is asked, and team workers decline it. Remote `approval.respond` cannot grant lasting-only permissions without the existing **Full access for remote tasks** authorization (declines remain possible); `/btw` on a side thread that became a team lead rechecks the existing remote team gate. Bots' existing in-chat approval behavior is unchanged. Bot team claims, schedule provenance marks, Workers UI and wake displays are also updated upstream.

These controls coexist with Extended's independently configured update policy, Connect master/direct routing, global bot pause, external MCP, cloud credential transfer and cloud/VM agent management. No Extended control is removed, hard-disabled, or given a new default in this delta. Source-hunk compatibility was checked for the 11 overlap files; installed-app On/Off, existing/new processes, remote clients, approvals and external integrations were **not** exercised here. Extended **0.12.0 remains DRAFT**.

## 2026-11-12 — inherited 0.3.120 limitation notes

Pinned [upstream `f66e95267701`](https://github.com/ginmp8/pandacode/commit/f66e95267701750696d4a43eabdd6c74f6b56927), source tree `085808f3e028ff6ad4b515d562d65c6485f0b921`, app **0.3.120**. The sole source change since the previous snapshot is a five-bullet addition to the [original release note](https://github.com/ginmp8/pandacode/blob/f66e95267701750696d4a43eabdd6c74f6b56927/releases/0.3.120.md), blob `f49dba032c6236f85ee88642221c35a2a0b25275`. No new application, manifest, lockfile, test, workflow or Extended-control change was introduced. Git trees retain 2,007 upstream paths / 2,065 Extended paths, zero upstream files missing or mode/type mismatch, and 80 previously customized shared paths after this note is mirrored. Extended **0.12.0 remains DRAFT**, not released.

No Settings or backend gate changed: manual/automatic updates, Connect master/direct routes, global bot pause, external MCP, cloud/VM and credential transfer remain as implemented. Native On/Off coverage still requires testing.


## Source 0.3.120 — control continuity, 2026-11-12

Source [ginmp8/pandacode:main `69155055e1a9`](https://github.com/ginmp8/pandacode/commit/69155055e1a962c0af4b0cf041af0b1f87291588), Git tree `e6dfbb33ad2d03dc51005b21ae76f873028c7a54`; app version **0.3.120**. Since pinned source `b026526122c4` (0.3.119), three commits change **54 upstream paths: 49 imported byte/mode-exact, five adapted** (Extended release guidance and npm/Cargo manifests/locks). Candidate full-tree inventory: **2,007 upstream files, 2,065 Extended files, 1,927 shared identical, 80 shared divergent, 58 Extended-only, zero missing upstream paths or mode/type differences**. The other Extended code change repairs an existing Rust syntax error in `src-tauri/src/architect.rs:1436` (escaped `"auto"` in the dispatch description), independently reproduced in failed [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43266025146) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43266044987) on previous SHA `e0beb365b279`. New SHA CI and native functional equivalence remain to be checked. **Extended 0.12.0 is DRAFT**, not a published release.

The changed source paths concern tool activity, image recognition/authorization, tool summaries, supporting tests, version metadata and guidance. They do not change Extended update timing, Connect master/direct routes, global bot pause, external MCP authorization, cloud credential forwarding or VM management switches. The existing Extended `AGENTS.md` manual release gate remains alongside new upstream guidance. Off/On behavior must be exercised separately; this file-inventory result is **not** behavioral proof and introduces no new restrictions or defaults.


## 2026-11-12 — upstream team and provider behavior, Extended gates

Pinned source: [ginmp8/pandacode:main at `b026526122c4`](https://github.com/ginmp8/pandacode/commit/b026526122c45264f389c04f91a4e96010948926), Git tree `ba8cfd56d9eb91fe433abc1e9377cb069a8f848e`, native application version **0.3.119**. Compared with the last synchronized 0.3.117 source, 44 commits changed 132 paths; the reconciled candidate incorporates 109 source-exact paths and 23 Extended-aware adaptations, with zero upstream paths absent or type/mode discrepancies. Its full inventory is 2,002 upstream files, 2,060 Extended files, 1,922 shared byte-exact, 80 shared divergent and 58 Extended-only. Historical shared differences are not yet all behaviorally proven. The Extended distribution stays **0.12.0 DRAFT**; no release, tag or public package is authorized.

Newly inherited since the prior Extended 0.3.117 candidate: team workers handle approvals and input under their lead's full-access authority, model-specific per-worker reasoning effort, a compact Workers panel, and remote/bot/schedule permission gates around team leads (0.3.118). Claude Code usage now has an opt-in, bounded CLI fallback and saved reading behavior (0.3.119). Existing 0.3.115–0.3.117 Skills, tool activity, image viewer and error controls are carried forward but are **not** new Extended-only features. Upstream release notes under `releases/` remain source-exact.

The Extended global bot pause, Connect master/direct-route switch, optional external MCP servers, cloud credential transfer and VM/agent management controls, and manual/automatic update preferences remain in the candidate. The source's team-lead Full-access checks are inherited and coexist with these controls. Static source reconciliation alone does not certify actual On/Off, old/new process, device, approval or external integration behavior.

The new upstream Full-access team-lead restriction applies to remote devices, bot-driven turns, and schedules saved remotely. It is independent of Extended's global bot pause and Connect master gates; enabling Extended's relevant options must retain the normal upstream path. Preserve Extended fresh-context routing and channel behavior where configured. Native permission scenarios and On/Off checks are still unverified in this candidate.


## 2026-11-12 — 0.3.117 error reporting and existing controls

The [0.3.117 source](https://github.com/ginmp8/pandacode/commit/9648d1c4c9e68f86265b9be04dfb1a8c1f960fb4) changes error presentation and dismissal, not the intended scope of Extended feature switches. Upstream changes were applied to the Extended bot UI, main thread, app update feedback and Connect settings. The Connect master switch is still required before remote functions can be reached; the source's error-row UI was reconciled around that Extended gate and failed master-switch operations now focus the related banner. No new restrictive default or global MCP/bot/cloud/VM/credential block was introduced. The underlying On/Off behavior and external integrations still require native regression evidence.

## 2026-11-12 — no new Extended control introduced for upstream 0.3.116

The [upstream 12a0468948c2](https://github.com/ginmp8/pandacode/commit/12a0468948c2ff1c9bc6be0bebf3f85411d73819) tool-activity changes were integrated without changing existing optional Extended controls. The new tool_image command is authorized by existing task/session records and its own local image safeguards; it is not a new global switch that disables MCP, bots, Connect, cloud/VM, credentials or automation. The eight source-overlap adaptations apply exact upstream hunks onto pre-existing Extended files and preserve unrelated Extended modifications, including Settings defaults. Enabled/disabled native flows, resumed bots and external integrations were **not** exercised; code presence is not proof of runtime parity. Planned release remains **0.12.0 DRAFT**.
## 2026-11-12 — Skills control compatibility with upstream 0.3.115

Source [`7171e9606e48`](https://github.com/ginmp8/pandacode/commit/7171e9606e48b3645b5c4a1a6aac01386d5a6369) (Git tree `dd913037d13509ca9da764d77f9cb9eea4b1c4a9`) advances two commits from `650537ae` and declares native app **0.3.115**. All 15 upstream-changed paths (14 modified, one new) are mirrored: 11 exact blobs/modes and four version-only manifest adaptations retaining Extended dependency remediation. The upstream `releases/0.3.115.md` is unchanged. The planned Extended distribution remains **0.12.0 DRAFT**, without release or publication. Prior Extended `64cd597d` passed 11/11 push CI jobs and 11/11 PR CI jobs; final integration SHA CI and native acceptance need separate evidence.

No new Extended Settings switch or restrictive default is introduced. Upstream project-scope Skills activation/deactivation guards, disabled mobile slash-command handling and clone/worktree invalidation are incorporated alongside existing optional Extended update, bot pause, Connect/direct, external MCP, cloud/VM and credential-transfer controls. Their enabled/disabled native flows remain pending independent validation; project safeguards must not be generalized into disabling these other features.

## 2026-11-12 — inherited transcript and audio changes

Upstream transcript windowing, loaded-turn search, cached rows and Linux JACK/OSS fallback are imported from [`650537ae449d805656f531fc6fdb5dec09eceb80`](https://github.com/ginmp8/pandacode/commit/650537ae449d805656f531fc6fdb5dec09eceb80). No new Extended control or more restrictive default is introduced. Existing automatic/manual update, Connect, bot pause/resume, MCP, cloud/VM and credential-transfer controls retain their prior configurable On/Off semantics; those native behavioral paths were not exercised in this integration. The 0.12.0 distribution remains unapproved.

## 2026-11-12 — inherited audio and CI update

[fork `9aa88776`](https://github.com/ginmp8/pandacode/commit/9aa88776f07c2274134e4a952ffcd7da71e70e50) changes only voice-test helpers/tests, voice documentation, upstream notes and CI. No Extended Settings behavior is modified, suppressed or made more restrictive. The inherited JACK/OSS **dictation** path falls back to the default input when a saved JACK/OSS backend fails; **Test microphone** reports the error. Native hardware acceptance remains outstanding. Existing Extended On/Off controls for bots, updates, Connect, MCP, cloud/VM and credential transfer remain untouched and are not verified by this import.


**Status:** Inherited control surface at the 0.11.0 baseline. No new 0.12.0 control or native acceptance is claimed.

Start from the [0.11.0 feature-controls record](../0.11.0/feature-controls.md) and the [full upstream parity contract](../upstream/parity-contract.md). Preserve existing Extended controls for automatic updates, Connect/remote access, paused bots, external MCP, cloud credentials and cloud/VM operations. Keep upstream code paths functional when the corresponding feature is enabled and authorized.

The 0.11.0 cycle also introduced **opt-in per-bot fresh-context behavior** and an **independent shared-channel history preference**. These are inherited starting functionality, not new 0.12.0 work. Ensure they remain separate, do not duplicate delegated context and do not silently reset queues.

## 2026-11-12 inherited test update

[fork `9e5d0988`](https://github.com/ginmp8/pandacode/commit/9e5d0988970785a9302938ecdc8d8848b7d79a7f) changes only the release-profile microphone cap assertion and diagnostics. No new Extended Settings or changes to updates, Connect, bots, MCP, cloud/VM or credentials. Retain all outstanding native On/Off acceptance scenarios.

## 2026-11-12 CI/test correctness

The unreleased fork [`8e5ab59c`](https://github.com/ginmp8/pandacode/commit/8e5ab59cdd22344a08e199dd94e6674af71ac7bd), tree `4fccf8f62aad4a59e3a03005db469819394713f4`, keeps application **0.3.111** unchanged and modifies two source paths. `releases/0.3.111.md` is mirrored exactly; CI is reconciled for the fully qualified ignored release-profile audio test with `--exact` and APT fetch retries `Acquire::Retries=3`, retaining Extended's audits, mobile checks, ARM64 recovery and macOS timeout. The Extended regression test now checks the real Linux/macOS capture requirement and both exact test commands. No upstream product capability or dependency version changed. No Extended controls (updates, Connect, bots, MCP, credentials, cloud/VM) changed; their native On/Off acceptance remains unverified.

## Incremental verification

- [ ] Pin the fork `main` and Extended `base/development` SHAs before each import, inspect every customized shared source hunk, and record justified exceptions.
- [ ] Verify **On** and **Off** states, persistence/restart behavior, disabled vs explicitly authorized operations and effective permissions.
- [ ] Regress bot harness selections, bot chat access levels, fresh-context and channel-history preferences, delegation, queue delivery and error visibility.
- [ ] Regress account-pool sign-in/usage with real authorized provider accounts, Connect/MCP/cloud/VM and credential approvals.
- [ ] Validate that controls do not hide, delete or stub upstream features.
- [ ] Track runtime and installed-app evidence separately from static file parity or CI.

No 0.12.0-specific feature-control acceptance was established at cycle opening.

## Inherited upstream 0.3.102 model-selection behavior

The [upstream ZCode model-selection fix](../../../releases/0.3.102.md) is integrated into the Extended 0.12.0 source candidate. ZCode cannot join or lead an agent team, so automatic team catalogs exclude it, and selecting a ZCode model ends a team with no worker history. If workers have already run, the user must start a new task to use ZCode. This is a known upstream harness limitation, not an Extended permission toggle or removal of the ZCode harness. The imported tests have not been run locally; verify real harness transitions and other Extended controls separately.

## PandaCode 0.3.103 integration

Council's target score, round cap, reviewer count and start-on-new-thread preference are upstream team/composer settings, not new Extended-only security toggles. The review/fix/decision flow must remain functional with each pre-existing Extended control enabled or disabled as intended, without silently bypassing file-scope restrictions, authentication or approval paths. Council review/fix loops, settings persistence, cancellation/retry and stale-score invalidation require native acceptance; the code and tests were imported, but runtime acceptance is **not verified**.

The Extended `architect.rs` file-scope contract and shorter existing dispatch/catalog descriptions were retained, while upstream `councilLens` and Council implementation were incorporated. This is an intentional shared-file exception; it is not permission to omit upstream Council tools.

## PandaCode 0.3.104 inherited desktop branding

The upstream desktop intro, replayable wordmark and star panda are integrated without changing the Extended's existing optional Settings (updates, Connect, bot pause, external MCP, cloud/VM, credential forwarding, fresh context or shared channels). The inherited tests describe reduced-motion handling, keyboard access, nonblocking text entry and narrow-sidebar behavior; native Windows/macOS/Linux and interactions with Extended Settings have **not** been independently accepted. Both enabled and disabled operational paths remain subject to the separate control-state regression matrix. No previously permitted upstream feature was intentionally removed or stubbed by this import.

## PandaCode 0.3.105 account pooling and Extended controls

Inherited account-pool selection, sign-in identity, provider-local usage and account-switch history were integrated from [fork `f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665`](https://github.com/ginmp8/pandacode/commit/f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665) in [Extended `36228b949aaf8648109e2f8f8da441a8735cee41`](https://github.com/ginmp8/pandacode-extended/commit/36228b949aaf8648109e2f8f8da441a8735cee41). Pool opt-in and task-specific account routing are upstream behavior, **not** a newly introduced Extended safety switch. The model picker continues to offer one choice per harness/model; named provider accounts remain manageable under Settings. The Extended /btw deferred-slash test hook was preserved alongside the upstream account-switch test additions.

No Extended setting was intentionally removed or changed to a stricter default. Actual opt-in/opt-out pool behavior, provider login/fallback, account-switch persistence, and interaction with existing bot pause, external MCP, Connect, cloud/VM and credential permissions still require independent runtime/native tests.


## Upstream 0.3.106 task tools and existing Extended Settings

The upstream source now registers app task tools for supported non-team harnesses and team leads, while team workers remain scoped to their permitted team tools. The Extended `architect.rs` tool-catalog version advanced and the existing optional controls were retained. **Task-tool availability does not override upstream authentication/approval or Extended user choices.** Verify launch, cancellation and restart for every harness, with global bot pause, MCP, Connect and cloud/VM/credential switches both On and Off where relevant. No native end-to-end control-state test was executed as part of this source copy.

## Upstream 0.3.107 Skills manager and Extended settings

The inherited Skills manager exposes per-project and global enable/disable switches, Skills CLI installation, quarantine/restore, uninstall/Undo and slash-command/bot enforcement. It occupies Settings → Tools → Skills; the upstream Rules & library page remains separate. Extended Feature controls remain an independent Settings section. A Skills switch does not replace or silently tighten the Extended Connect, MCP, bot pause, cloud/VM, credential-transfer or update controls. CLI installs, switch enforcement and native Windows/Linux/macOS behavior need independent acceptance tests.

## Upstream 0.3.108 voice-dictation integration — 2026-11-10

- Source: [`ginmp8/pandacode:main` `952d0acbe02547af036ef6d7635ec415a472eb3b`](https://github.com/ginmp8/pandacode/commit/952d0acbe02547af036ef6d7635ec415a472eb3b), tree `202f77088372d261b263b806354d84ce094f25b8`, native app version **0.3.108**; includes commits after the source's release commit. The planned Extended distribution documentation remains **0.12.0 DRAFT**.
- Reconciled the 109-path source delta relative to `5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1`: 78 additions (including Rust/frontend audio input, model catalog, tests and notices), 17 exact source updates and 14 changes reconciled with Extended behavior. The inherited voice feature supports optional on-device speech models and configured transcription services, transcription into the composer, explicit recording/stop and tests. This is not an Extended-only invention.
- Preserved Extended Feature controls, release identity and dependency remediation, including `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`. Native microphone permissions, service credentials, model downloads, user-cancelled recording, offline transcription and Settings states need separate behavior checks.
- This section describes a source integration **candidate**, not evidence of a successful new-SHA CI run. Check the eventual exact commit SHA for Rust/Linux/Windows/macOS, RustSec, Node, npm, Expo, installer/platform and native feature-control acceptance. Historic successful CI applies only to the earlier SHAs.

Voice is an additional upstream Settings page. It must not replace or alter the Extended Feature controls section, its persisted values or the enable/disable semantics of bots, Connect, MCP, cloud/VM, credentials and updates.


## Upstream 0.3.108 release gate follow-up — 2026-11-10

The [post-version source `57f0aaf7cfc3`](https://github.com/ginmp8/pandacode/commit/57f0aaf7cfc35419d4a21b9823bbdf35dc73cfd1) checks CI for the exact commit before publication. Extended scopes the inherited gate to an **explicit manual publish** (`workflow_dispatch` and `publish=true`); tag pushes continue to build artifacts only. No user-facing update, Voice, bot, MCP, Connect, cloud/VM, credential or Skills switch changes. Native On/Off acceptance remains open.

## Post-0.3.108 source follow-up — 2026-11-10

The updated release gate checks exact-SHA successful push-to-main CI only when manual `workflow_dispatch` has `publish=true`. Tag builds remain artifact-only. No Voice, Skills, update, bot, Connect, MCP, cloud/VM, or credential Feature control changes; independent enabled/disabled acceptance remains outstanding.

## Fork HEAD 979b7d4e integration — 2026-11-10

This source update affects upstream Voice documentation and the release-gating workflow, **not** the Extended update/bot-pause/Connect/direct-route/MCP/cloud/VM/credential switches. The inherited empty/malformed CI lookup guard is incorporated exclusively into manually authorized GitHub Release publication. Tag pushes still produce build artifacts without publishing. Native enabled/disabled feature paths and actual manual publish acceptance remain unverified.

## Fork HEAD bc323966 integration — 2026-11-10

The new fork Voice changes and CI/release fixes do not change any Extended Feature control. The revised `AGENTS.md` includes upstream's push-main CI prerequisite but explicitly points to the Extended manual-publication override. Tag pushes remain artifact-only; publishing still requires separately authorized manual `workflow_dispatch` with `publish=true`. Automatic updates, Connect/direct routes, bots pause/resume, MCP, cloud/VM management and cloud credential controls retain prior semantics; independent On/Off acceptance remains outstanding.

## Fork 0.3.109 controls checkpoint — 2026-11-11

[Source `64996a1a`](https://github.com/ginmp8/pandacode/commit/64996a1a8b1d722eb10b0c4ec4b3c1ffef1fd197) adds Voice and CI fixes without adding or removing any Extended feature toggle. Keep automatic/manual updates, Connect master/direct paths, bot pause, external MCP, cloud-credential transfer, cloud/VM operations, fresh context and shared-channel behavior available under their current configuration. Runtime On/Off testing remains separate and unverified; the CI macOS audio smoke never authorizes publishing.

## Upstream phone/Connect parity — 2026-11-11

Fork `ginmp8/pandacode:main` at [`a403c3d9`](https://github.com/ginmp8/pandacode/commit/a403c3d9bfd67ad4bb70eff3f1d2d4befebfa40e), source tree `79a4d1aa2c8dc3537f98f0f5d6cd166837cf4565`, application version **0.3.111**, contains six source commits after `66a7df2f`. The integration candidate imports **293 source-exact new or changed paths** and semantically reconciles **15 concurrently modified paths** to retain Extended controls, dependency remediations and compatibility. Frozen recursive inventory: **1,890 upstream paths, 1,947 Extended paths; 1,809 exact common, 81 divergent common, 57 Extended-only, zero missing upstream, zero mode/type differences**. Native On/Off, remote/Connect, bot, MCP, cloud/VM and credential scenarios have not been independently accepted; GitHub CI is unverified for the integration candidate until its final SHA is published and checks complete.

Preserve the Extended switches for auto/manual updates, Connect master/direct route, bot pause, MCP, cloud/VM and credential transfer, plus native fresh-context controls. The 15 shared-path reconciliations include native remote host/socket, bot delivery, headless, bridge and main-thread adaptations. The source feature remains eligible when enabled and normally authorized. No new restrictive default is proposed. On/Off scenarios and live two-host/device behavior were not executed in this integration candidate.


## Fork Linux microphone and Windows ARM smoke update — 2026-11-11

The source [`961b43fa`](https://github.com/ginmp8/pandacode/commit/961b43fa88ad1d24eb083bc10779cb7034c4a0fd) modifies Voice, release smoke tests and CI, but changes no Extended-specific Settings implementation. The direct integration [`5beff6d8`](https://github.com/ginmp8/pandacode-extended/commit/5beff6d8737e952341023bfdf10aaeb38697fcae) leaves the existing optional update, Connect master/direct, bot pause, external MCP, cloud credential and agent cloud/VM controls intact in the 81 remaining shared divergences. This is a **static change-scope observation**, not an independent On/Off, restart, authorization or native integration test. Keep the upstream voice fallback enabled according to normal permissions; do not introduce a new restrictive toggle.

## Source 106b3852 continuity — 2026-11-11

Upstream microphone selection, Windows x64/Linux ARM64 release audio smoke and APT time bounds are incorporated without changing Extended feature-control semantics. Existing manual/automatic updates, global bot pause, Connect master/direct routing, external MCP, cloud/VM management, credential transfer and other Settings controls remain present and optional; no stricter default or blanket feature ban was introduced. This does not establish runtime acceptance: independent enabled/disabled tests and native integration checks are still pending.

## Phone-access clarification from source a2ba4b43 — 2026-11-12

The [upstream release-note correction](https://github.com/ginmp8/pandacode/commit/a2ba4b430951ee2995bdfda140d4f516cb493a78) distinguishes a phone's **Full access level** from the separate desktop **Full access for remote tasks** switch. It changes no permission enforcement code or Extended configuration; current auto/manual updates, Connect master/direct routes, bot pause, external MCP, cloud/VM, credential forwarding and fresh-context Settings remain unchanged. Do not treat upstream release-note wording as proof of successful runtime authorization or native On/Off verification; those tests remain pending.

## Source 802fa847 controls continuity — 2026-11-12

The upstream changes modify only CI and Voice/release documentation, not native product Settings, auto/manual updates, Connect master/direct routes, global bot pause, external MCP, cloud/VM management, credential forwarding or fresh context. The Extended-enabled upstream paths remain in scope; no new control or restrictive default was introduced. This is a **static change-scope finding**, not proof of native On/Off, remote or external-provider behavior. Null ALSA CI setup is not an application microphone request.

## Source 0.3.112 compatibility — 2026-11-12

The upstream local-agent exit guard is a native confirmation feature, **not** an Extended capability switch. When quitting with local agents running it asks Quit/Cancel before the existing shutdown and stop path. Cloud-only runs continue remotely; headless serve and updater restart skip the dialog. No existing Extended preferences for update timing, Connect routing, MCP, global bot pause, cloud credential transfer or VM controls were changed. Enabled upstream paths remain available at source integration level; native dialog interactions and combinations with these settings remain untested on installed applications. Do not infer On/Off acceptance from static integration.
