# Extended 0.12.0 — known issues and acceptance gaps

## 2026-11-12 — verification gaps after source 0.3.114 integration

No regression is claimed solely from the imported upstream changes. GitHub CI for the eventual publication commit is not yet complete; local Node/Rust builds and tests, installed-platform transcript performance, Linux fallback when both microphones fail, native Settings On/Off, Connect/MCP/bots/cloud/VM/credentials and historical divergent-file semantic acceptance remain unverified. The inherited transcript Find function searches only loaded pages, not unfetched history (upstream `releases/0.3.114.md`). Keep these limits separate from proven failures and release approval.

## 2026-11-12 — release-smoke false-positive prevention

[fork `9aa88776`](https://github.com/ginmp8/pandacode/commit/9aa88776f07c2274134e4a952ffcd7da71e70e50) fixes the possibility that an overly narrow Rust `--exact` filter silently passes with zero tests; the smoke now verifies exactly one passing test, and an Extended regression assertion covers both jobs. The JACK/OSS fallback test distinguishes dictation from microphone-probe errors. This is inherited behavior and stronger test coverage, not proof of native acceptance or a verified Extended product defect. Final CI for the newly integrated Extended SHA, real Linux microphone/JACK/OSS and Settings On/Off/external integration flows remain outstanding.


**Status:** Initial register inherited from [0.11.0](../0.11.0/known-issues.md). An unverified scenario is **not** a confirmed bug. Do not silently declare prior gaps fixed or regressions present.

| Area | Opening classification | Evidence needed before 0.12.0 release |
| --- | --- | --- |
| Full upstream functional parity | Static/source checks were recorded in 0.11.0; remaining shared divergence and live behavior are unverified | Fresh complete tree, file modes/binaries, hunk-level explanations and Settings On/Off tests |
| Bot and channel context | Fresh-context and shared-channel preferences inherited from 0.11.0 | Real delegation, fresh-context, channel history, restart, queue and permissions regression |
| Account pools and sign-in | Imported upstream features, native multi-account validation not documented as complete | Live authorized Claude/Codex accounts, usage/limit transitions, restart, queue preservation and provider UI |
| Mobile redesign | Source inherited in 0.11.0; real device behavior not verified | iOS/Android navigation, authentication, notifications, badge and TestFlight usage |
| Connect, MCP, cloud/VM, credentials | No full native acceptance recorded | Test both settings states, approvals, remote authorization and recovery |
| Existing installations / data migrations | Acceptance pending | Representative SQLite upgrades, backup/recovery, rollback and native platform installation |
| Independent Extended packaging | Tag/version workflow patched in 0.11.0, but runtime auto-update behavior has limitations | Real installers, updater manifest, signing and differing Extended tag vs upstream app-version behavior |
| Confirmed 0.12.0-specific defects | None at cycle opening | Reproduction, impacted SHA, fix and regression evidence |
| ZCode participation in agent teams (inherited upstream 0.3.102 limitation) | ZCode supports single-agent tasks; automatic teams exclude it, and a team with completed worker activity cannot be switched to ZCode in the same task | Verify the imported model-picker and team-catalog regression tests and test single-agent/team transitions in native app; do not treat the upstream harness limitation as an Extended regression |

## 2026-11-12 inherited test and provenance fixture

**Confirmed CI fixture issue:** Prior `e1cd7e75` [push](https://github.com/ginmp8/pandacode-extended/actions/runs/43165784610) and [PR](https://github.com/ginmp8/pandacode-extended/actions/runs/43165792686) runs each passed 10/11 jobs; the remaining Node job flagged a stale pinned `releases/0.3.111.md` SHA (`ce584235`) in `source-baseline.json`, while the correct upstream/Extended Git blob is `357b3614`. Corrected the fixture, not the original notes or test. [fork `9e5d0988`](https://github.com/ginmp8/pandacode/commit/9e5d0988970785a9302938ecdc8d8848b7d79a7f) also improves inherited real-audio cap coverage. New-SHA CI and native microphone/Settings On/Off parity remain outstanding; no product regression was established.

## 2026-11-12 source CI adjustment and outstanding verification

**Confirmed stale CI-test assertion, not an app regression.** Previous Extended `e059e9a08623` [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43159765950) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/43159773384) each reported a failing Node web-test job (526/527 passed): its stale assertion required macOS-only capture despite Linux/macOS now being required. Corrected without weakening coverage. Other jobs were running at inspection; new-SHA CI and real Settings On/Off, microphone and integration acceptance remain pending. The unreleased fork [`8e5ab59c`](https://github.com/ginmp8/pandacode/commit/8e5ab59cdd22344a08e199dd94e6674af71ac7bd), tree `4fccf8f62aad4a59e3a03005db469819394713f4`, keeps application **0.3.111** unchanged and modifies two source paths. `releases/0.3.111.md` is mirrored exactly; CI is reconciled for the fully qualified ignored release-profile audio test with `--exact` and APT fetch retries `Acquire::Retries=3`, retaining Extended's audits, mobile checks, ARM64 recovery and macOS timeout. The Extended regression test now checks the real Linux/macOS capture requirement and both exact test commands. No upstream product capability or dependency version changed. Nontruncated candidate inventory: 1,890 upstream files, 1,948 Extended files, 1,809 source-exact shared, 81 customized shared, 58 Extended-only, none missing and no mode/type differences. The 81 differences require separate hunk-level and runtime acceptance.

## Baseline provenance

- [0.11.0 known issues](../0.11.0/known-issues.md)
- [0.11.0 release readiness](../0.11.0/release-checklist.md)
- [0.11.0 native/application release identity](../releasing.md)

As new evidence arrives, distinguish **observed failures**, **known inherited limitations**, **planned tests** and **validated fixes**. Do not disable upstream functionality in lieu of a justified correction.

## PandaCode 0.3.103 integration

Council's scored review/fix rounds, composer panel, controls and stale-score tracking were copied from the fork's current HEAD, with original tests. **Known upstream limitations:** paid-model review/fix sessions have not been validated; external file edits and shell/script changes not reported through file tracking may not invalidate a score. Native Settings On/Off interactions, cancellation/restart, resume, Connect/MCP/cloud permissions and real platform behavior are **unverified**, not confirmed regressions. The 80 previously customized shared source files have not been exhaustively accepted hunk by hunk.

## PandaCode 0.3.104 acceptance gaps

- **Unverified, not confirmed defects:** desktop panda startup/replay, star-button external link behavior, keyboard access, reduced motion, sidebar sizing and mobile shared-source build behavior have only imported source tests and code review, not independent native-app validation.
- **Static parity:** at the [integrated source SHA](https://github.com/ginmp8/pandacode/commit/d5d51769ccac15145e67c071ea06a5502c4cadcb) all 1,561 tracked upstream blobs have corresponding paths; the 80 existing Extended customized shared files still lack exhaustive hunk-by-hunk and real Settings On/Off acceptance. This is not a confirmed loss of functionality.
- **CI:** code-commit [push run #42503275362](https://github.com/ginmp8/pandacode-extended/actions/runs/42503275362) and [PR run #42503280072](https://github.com/ginmp8/pandacode-extended/actions/runs/42503280072) were still running when initially checked. Validate final documentation SHA independently before calling the candidate build green.

## PandaCode 0.3.105 inherited-account acceptance (2026-11-08)

- **Observed code incorporation, not a confirmed bug:** all 38 changed paths from [source `f28c4df0`](https://github.com/ginmp8/pandacode/commit/f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665) were reconciled in [Extended `36228b94`](https://github.com/ginmp8/pandacode-extended/commit/36228b949aaf8648109e2f8f8da441a8735cee41). No upstream files were omitted from the complete static tree. Eight files required local reconciliation, retaining prior Extended behavior.
- **Unverified runtime scenarios:** actual Claude/Codex credentials across distinct config folders and subscriptions, token precedence, identity refresh while requests are in flight, opt-in account pooling, handoff/resume, compact expandable account-switch notices and /btw isolation across task changes; repeat with existing Extended Settings On/Off and real authorized provider/native builds.
- **Evidence boundary:** static tree parity and imported tests cannot prove runtime behavior. No local tests executed; CI was pending on the exact code commit at first observation. The 80 shared-file differences still require complete code-level justification and functional acceptance.


## Upstream 0.3.106 acceptance status — 2026-11-09

**No new confirmed Extended defect is asserted by this integration.** The source introduces transcript/final answer following, accessibility-aware themed headers, RTL prose, non-English keyboard shortcuts, recovery from inspector task-load failures, and task tools for every supported harness. Their source code and regression tests have been staged, but native/Playwright behavior has not been independently exercised with Extended controls. The 80 source-divergent Extended shared paths still need full hunk-level semantic classification. Missing scanner coverage is now explicitly a failed assurance step rather than an apparent success; this does not substitute for an SBOM or actual security scan.

## Exact-SHA CI security finding — 2026-11-09

At [Extended integration `34d8fb4c`](https://github.com/ginmp8/pandacode-extended/commit/34d8fb4caa985f2ceb4815945dc58ed3042c8353) (upstream [`0bfd5738`](https://github.com/ginmp8/pandacode/commit/0bfd57383621d5aa001d3871018920855cabec39), app **0.3.106**), [push CI #42644925066](https://github.com/ginmp8/pandacode-extended/actions/runs/42644925066) and [PR CI #42644937837](https://github.com/ginmp8/pandacode-extended/actions/runs/42644937837) both **failed**: five of six jobs succeeded in each, but **RustSec audit failed**. The lockfile contains `rusqlite 0.38.0` and `libsqlite3-sys 0.36.0`, whose bundled SQLite is affected by [RUSTSEC-2026-0459 / CVE-2026-119375](https://github.com/RustSec/advisory-db/blob/main/crates/libsqlite3-sys/RUSTSEC-2026-0459.md) (CVSS 6.8). The advisory identifies patched `libsqlite3-sys >=0.39.0`, released with `rusqlite 0.41`; the application explicitly enables `rusqlite`'s `bundled` feature. Its exposure involves opening a crafted local database plus WAL file; the failed audit does not independently establish exploitability in a deployed installation.

**Unresolved, release-blocking dependency remediation:** upgrade the compatible `rusqlite`/SQLite dependency graph, regenerate and commit `src-tauri/Cargo.lock` using Cargo (not hand-edited), verify the Rust toolchain floor (`rusqlite 0.41` requires Rust 1.89), check any affected SQLite API/storage and migration paths, and rerun `cargo check --locked`, relevant Rust/SQLite regression tests, and `cargo audit --file src-tauri/Cargo.lock` before treating the finding as fixed. The local sandbox has no Cargo and cannot resolve GitHub DNS, so no validated lockfile regeneration or Rust test was possible. No vulnerability suppression, CI bypass, or dependency-version-only edit was applied. The source and destination Git trees still have **zero missing upstream paths**, but this does not prove functional equivalence of the 80 customized shared files or validate Settings/Connect/MCP/cloud/VM native behavior. Planned Extended distribution **0.12.0** remains unapproved; checks on any documentation-only follow-up SHA are separate.

## Status update: SQLite dependency remediation candidate — 2026-11-09

The historical `libsqlite3-sys 0.36.0` RustSec failure remains a valid observation on earlier commits. The new candidate uses `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`, reusing the full compatible dependency graph from a read-only branch where RustSec and locked Rust checks passed. **Resolution is pending exact-SHA base/development CI and native SQLite/data-migration validation.** Settings On/Off, Connect, MCP, bots, cloud/VM and credential integrations remain separately unverified; no functionality was disabled.

## SQLite audit failure follow-up — 2026-11-09

The former RustSec failure on `libsqlite3-sys 0.36.0` is **historical**, affecting previous commits rather than the current `base/development` dependency graph. Both [push #42680289120](https://github.com/ginmp8/pandacode-extended/actions/runs/42680289120) and [PR #42680300162](https://github.com/ginmp8/pandacode-extended/actions/runs/42680300162) passed RustSec and Linux locked Rust tests/Clippy on [code SHA `06cb9b0c0141`](https://github.com/ginmp8/pandacode-extended/commit/06cb9b0c0141e964d43d0c462cbd30e9fc44ec1c). **Remaining unverified release gates:** native SQLite data migration/upgrade/rollback, installed-app behavior across platforms, the 80 shared-file hunk reconciliations, and independent Settings On/Off, Connect/MCP, bots, cloud/VM, and credential workflows. Do not reopen the old audit result as a current CI failure, or interpret passing CI as installed-app release approval.

## New Skills manager acceptance gaps — upstream 0.3.107

The upstream introduces a Skills manager with global/project switches, quarantine, local/link installation and uninstall/Undo. Upstream release notes explicitly report incomplete native Windows/Linux cross-volume and live CLI validation; these limitations are inherited, not confirmed Extended regressions. On this Extended candidate, interactions between skill switches and existing Feature controls, bots, shared channels, MCP/Connect and restart are unverified. The 80 previously divergent shared source files still require full semantic acceptance. CI on the new exact destination SHA, native installers and data migration are pending; do not interpret a complete file inventory as functional parity certification.

## Upstream 0.3.107 exact-SHA CI follow-up — 2026-11-10

Both [push CI #42805588366](https://github.com/ginmp8/pandacode-extended/actions/runs/42805588366) and [PR CI #42805595388](https://github.com/ginmp8/pandacode-extended/actions/runs/42805595388) completed **successfully (6/6 jobs each)** on the reconciled code commit [`e4e5c0c51daa8a0149015bb26a00c8fb09bb2050`](https://github.com/ginmp8/pandacode-extended/commit/e4e5c0c51daa8a0149015bb26a00c8fb09bb2050). All reported jobs passed: Linux Rust check/tests, RustSec, Node/TypeScript/unit tests and website, npm dependency audit, Expo/mobile compatibility and macOS ONNX parity.

The pending-CI statement above is an earlier observation, now resolved for the referenced code SHA. Remaining acceptance gaps include native Skills CLI install/quarantine/restore/uninstall/Undo, cross-volume behavior, SQLite migration/rollback, Extended controls On/Off and a hunk-by-hunk reconciliation of all 80 customized shared files. A docs-only follow-up SHA requires its own CI verification.

## Upstream 0.3.108 voice-dictation integration — 2026-11-10

- Source: [`ginmp8/pandacode:main` `952d0acbe02547af036ef6d7635ec415a472eb3b`](https://github.com/ginmp8/pandacode/commit/952d0acbe02547af036ef6d7635ec415a472eb3b), tree `202f77088372d261b263b806354d84ce094f25b8`, native app version **0.3.108**; includes commits after the source's release commit. The planned Extended distribution documentation remains **0.12.0 DRAFT**.
- Reconciled the 109-path source delta relative to `5998a8fc085efbd9d0d53b65f8fb3a3f80a7b9a1`: 78 additions (including Rust/frontend audio input, model catalog, tests and notices), 17 exact source updates and 14 changes reconciled with Extended behavior. The inherited voice feature supports optional on-device speech models and configured transcription services, transcription into the composer, explicit recording/stop and tests. This is not an Extended-only invention.
- Preserved Extended Feature controls, release identity and dependency remediation, including `rusqlite 0.41.1` and `libsqlite3-sys 0.39.1`. Native microphone permissions, service credentials, model downloads, user-cancelled recording, offline transcription and Settings states need separate behavior checks.
- This section describes a source integration **candidate**, not evidence of a successful new-SHA CI run. Check the eventual exact commit SHA for Rust/Linux/Windows/macOS, RustSec, Node, npm, Expo, installer/platform and native feature-control acceptance. Historic successful CI applies only to the earlier SHAs.

Native microphone permission, on-device model integrity, restart/cancellation, cloud speech-service configuration and differing CPU/OS build baselines have not received independent installed-app acceptance; no regression or vulnerability is asserted without evidence.


## Post-0.3.108 source regression and assurance status — 2026-11-10

Fork [`57f0aaf7cfc3`](https://github.com/ginmp8/pandacode/commit/57f0aaf7cfc35419d4a21b9823bbdf35dc73cfd1) hardens its voice-model tamper test for coarse timestamp filesystems and introduces a green-CI release gate. These are inherited changes, **not a confirmed new Extended defect**. Manual-only publishing is preserved. Native voice download checks, all relevant Feature controls On/Off paths, and the 80 historically divergent source files still require independent semantic/runtime acceptance.

## Post-0.3.108 source follow-up — 2026-11-10

No new source-code defect or vulnerability identified. Manual publication intentionally fails when only PR/non-main CI exists for the SHA or when main push CI is pending, failed or cancelled. This gate has not been independently exercised in a live manual run. Native controls, Voice, installers and 80 shared-file semantic differences remain open; no release approval.

## Fork HEAD 979b7d4e integration — 2026-11-10

The upstream update adds clearer documentation of x64 on-device voice CPU requirements, Windows on Arm limitations, transcription cancellation/retention behavior and a diagnostic for an unavailable/malformed GitHub Actions CI response. These are source clarifications, **not** confirmed Extended product defects. The preceding destination SHA `4ef18f0fe5fda72190c6754711ae8d7fbc4ab200` had a [failed push workflow](https://github.com/ginmp8/pandacode-extended/actions/runs/42862512017) due to a cancelled ONNX/macOS job, while its [PR workflow succeeded](https://github.com/ginmp8/pandacode-extended/actions/runs/42862529729). Check the new exact SHA; do not mislabel prior PR success as push-CI success. The 80 historical divergent shared paths and installed-app feature acceptance remain open.

## Fork HEAD bc323966 integration — 2026-11-10

The upstream fixes correct automatic recording-stop instructions (the visible button says **Transcribe recording**, not Retry), improve unexpected speech-engine termination messaging, prevent later main pushes cancelling a release commit's CI, and treat a nonzero `gh run list` exit as a failure. These are **inherited fixes**, not proof of previously failing Extended behavior. [previous Extended `858253745655`](https://github.com/ginmp8/pandacode-extended/commit/858253745655e2c7ebf4b0115679aaceeef69140) passed [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42872820878) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42872835520); new CI and native voice acceptance must be checked independently. Eighty historic modified shared paths and the intentional Extended release-guide deviation still need complete hunk-level confirmation. No release approval.

## Fork HEAD 9437b3a9 follow-up — 2026-11-10

The fork documents inherited Voice constraints: a custom local transcription URL requires a literal loopback IP (not `localhost`), a path and no query/fragment/embedded credentials; speech-service uploads and local speech-model downloads do not use system/HTTP proxy settings, so proxy-only networks cannot reach them. It also clarifies when a recording stopping sends audio to the chosen service. These disclosures are **not** newly reproduced Extended defects. The release gate now reports CI run IDs and permits rerunning cancelled or infrastructure/flaky jobs; manual publishing remains opt-in only. Native platform and service integration has not been independently exercised. The 80+ shared-file divergences still need hunk-level and runtime acceptance.

## Mobile node-forge advisory remediation — 2026-11-10

**Confirmed:** the Extended `c7c14606` push CI's mobile npm audit failed for pinned `node-forge@1.4.2`; [job evidence](https://github.com/ginmp8/pandacode-extended/actions/runs/42920988289). GitHub-reviewed advisories identify **1.4.3** as patched. The narrow Extended mobile override/lockfile remediation is in the next candidate; new-SHA npm audit, Expo compatibility and full CI remain **pending**, not passed. No application exploitability has been demonstrated.

## Fork 0.3.109 macOS Voice regression status — 2026-11-11

[Source `64996a1a`](https://github.com/ginmp8/pandacode/commit/64996a1a8b1d722eb10b0c4ec4b3c1ffef1fd197) fixes an upstream 0.3.108 crash observed on macOS in optimized release builds when opening Voice settings or microphone selection. The imported cpal 0.17.3 device-name compatibility and real-audio regression test need exact-SHA CI, then installed-app testing with microphone permissions and actual device hardware; no Extended-native reproduction or final acceptance is asserted yet. Other Settings, Connect, bots, MCP, cloud/VM, credentials, SQLite migration and installer checks remain open.

## Verified CI test-oracle mismatch after upstream 0.3.109 — 2026-11-11

[Push #42950948699](https://github.com/ginmp8/pandacode-extended/actions/runs/42950948699) and [PR #42950956081](https://github.com/ginmp8/pandacode-extended/actions/runs/42950956081) failed one Extended-only release test at source-integration SHA [`7a624076`](https://github.com/ginmp8/pandacode-extended/commit/7a624076c0ed44b9ecf89861497e9ef4e697f9be). The test expected only pull requests to be eligible for cancellation, but the actual workflow correctly protects every push CI run using `github.event_name != 'push'` while also permitting cancellation of manually dispatched runs. The mismatch is a stale static assertion, not evidence of an upstream feature loss or failed native Voice test. The correction updates the test oracle while preserving the substantive push-CI invariant; candidate CI and native acceptance remain unverified.

## Confirmed macOS Intel real-audio CI timeout — 2026-11-11

At [Extended `e0391b444d`](https://github.com/ginmp8/pandacode-extended/commit/e0391b444d57493c9c9ff16facef4d472b08faf3), [push CI #42956934291](https://github.com/ginmp8/pandacode-extended/actions/runs/42956934291) passed all 11 jobs, but [PR CI #42956941612](https://github.com/ginmp8/pandacode-extended/actions/runs/42956941612) failed its `Rust clippy (darwin-x86_64)` job: the `Real audio smoke (release profile)` step reached its 40-minute limit while compiling the optimized Rust application. The logs show no test assertion failure; the job was terminated by the step timeout. The Extended-only follow-up [CI adjustment `7e6bba07`](https://github.com/ginmp8/pandacode-extended/commit/7e6bba0716ebfa8483e15cd5e2ca6d6c881ede01) raises the smoke step allowance to 70 minutes and the platform job allowance to 120 minutes, preserving the optimized real-audio test, macOS Intel target, source capabilities, and publication gates. Completion on the new exact SHA and native microphone testing remain unverified; this is not release approval.

## Fork 66a7df2f Linux saved-microphone compatibility — 2026-11-11

Upstream [`66a7df2f`](https://github.com/ginmp8/pandacode/commit/66a7df2f58ed529056f549fd48b83790153f613d) adds ALSA name-to-PCM selection and regression tests so saved card names from cpal 0.16/0.3.108 remain recognizable after cpal 0.17. Original `releases/0.3.109.md` still describes the earlier Linux re-selection limitation. This is an inherited source-documentation discrepancy, **not** proof of a remaining runtime regression. New-SHA CI, actual Linux PipeWire/ALSA recording and installed macOS microphone permissions remain unverified. The imported macOS CI setting now fails on absent audio input instead of quietly skipping capture; CI virtual inputs do not establish real-device acceptance.

## Upstream 0.3.111 integration acceptance gaps — 2026-11-11

Fork `ginmp8/pandacode:main` at [`a403c3d9`](https://github.com/ginmp8/pandacode/commit/a403c3d9bfd67ad4bb70eff3f1d2d4befebfa40e), source tree `79a4d1aa2c8dc3537f98f0f5d6cd166837cf4565`, application version **0.3.111**, contains six source commits after `66a7df2f`. The integration candidate imports **293 source-exact new or changed paths** and semantically reconciles **15 concurrently modified paths** to retain Extended controls, dependency remediations and compatibility. Frozen recursive inventory: **1,890 upstream paths, 1,947 Extended paths; 1,809 exact common, 81 divergent common, 57 Extended-only, zero missing upstream, zero mode/type differences**. Native On/Off, remote/Connect, bot, MCP, cloud/VM and credential scenarios have not been independently accepted; GitHub CI is unverified for the integration candidate until its final SHA is published and checks complete.

No new confirmed runtime defect is claimed from the diff. The high-volume phone and Rust Connect refactor requires full TypeScript/Expo and Rust compiler/test validation on the **published destination commit**. The 81 remaining shared-file divergences still require complete hunk-level semantic review; absence of missing paths does not independently prove enabled runtime parity. CI pending or absent, real mobile devices, two-host Connect, Settings On/Off, permission prompts, native installers and data migrations are unverified until executed.


## Mobile npm clean-install regression — 2026-11-11

At Extended `d67a035d`, push and PR jobs failed at `npm ci` under npm 10.9.9 because the mobile dependency override and lock resolved `brace-expansion` inconsistently. A scoped override candidate pins the existing patched 5.0.12 version for `minimatch` 11.0.3. The next exact-SHA CI must confirm installation and Expo tests before this issue can be closed.

## Saved Linux microphone fallback — 2026-11-11

Fork [`961b43fa`](https://github.com/ginmp8/pandacode/commit/961b43fa88ad1d24eb083bc10779cb7034c4a0fd) adds fallback when a saved non-default ALSA device appears in the picker but fails to open. The new Rust helper test checks the fallback predicate, including `jack`, `oss`, a card name and raw PCM ID; it excludes System default and the explicit `default` choice. The code is mirrored in [`5beff6d8`](https://github.com/ginmp8/pandacode-extended/commit/5beff6d8737e952341023bfdf10aaeb38697fcae). Actual Linux device opening, sound quality, microphone permissions and native settings restart behavior are **not independently tested**, and no runtime regression is asserted. The Windows ARM64 startup-smoke retry is bounded to 16 seconds and newly enabled in CI; the exact new-SHA result is pending. All prior 81 shared-path semantic questions and Extended On/Off and remote acceptance gaps remain open.

## Linux Rust CI APT mirror timeout — 2026-11-11

At Extended commit [`7b895190`](https://github.com/ginmp8/pandacode-extended/commit/7b895190e6146233d03654d06d61cb5d98cf3073), [PR CI #43045852885](https://github.com/ginmp8/pandacode-extended/actions/runs/43045852885) cancelled **Rust check and tests (Linux)** at the job's 45-minute limit during `Install Linux native dependencies`. The log repeatedly reported ignored/stalled package downloads from `azure.archive.ubuntu.com`; npm, Rust tests and Clippy had not started. This establishes a CI mirror/acquisition timeout, **not** a Rust assertion or upstream feature failure.

The proposed Extended-only remediation changes **only the Ubuntu 22.04 Linux Rust job** to the standard Ubuntu archive and bounds APT retries and HTTP waits. The required native packages, Rust commands, permissions and source features remain unchanged. CI at the new exact commit SHA must finish successfully before this recovery is called verified. The fork remains pinned at `961b43fa` (application `0.3.111`); Extended distribution documentation remains `0.12.0 DRAFT`.

## Linux ARM64 CI package-index inconsistency — 2026-11-11

At source-pinned Extended SHA [`b21df4a7`](https://github.com/ginmp8/pandacode-extended/commit/b21df4a7e9bb64d13507083969d3201539d022c9), [PR CI #43055772877](https://github.com/ginmp8/pandacode-extended/actions/runs/43055772877) failed the `Rust clippy (linux-aarch64)` job **before Rust compilation**. Its Ubuntu 24.04 ARM64 runner reported DNS failures for `ports.ubuntu.com`, an APT index refresh that continued with stale indexes, and HTTP 404 for `libsoup-3.0-dev` (exit 100). The `Rust check and tests (Linux)` PR job was also cancelled during package acquisition; the separate push workflow was still pending at diagnosis. This is a demonstrated runner/package-acquisition failure, not a Rust or upstream product regression.

The Extended-only CI candidate retains the exact native dependency list and Rust/Clippy commands, adding an ARM64-only bounded update+install retry that requires a complete APT index refresh before package installation. This may mitigate intermittent DNS/index failures but cannot guarantee an unavailable package mirror recovers; check exact-SHA CI before asserting success. Native feature Settings On/Off and external integrations remain independently unverified. No Extended 0.12.0 release is approved.

## Source 106b3852 validation gaps — 2026-11-11

The Linux microphone regression has unit coverage, but saved card/raw-ID failures, JACK/OSS fallback and probe behavior still require real native audio-device acceptance. Newly added Windows x64/Linux ARM64 release-profile smoke jobs may pass with no usable input; macOS explicitly requires recording. Windows and Linux coverage is therefore a build/runtime smoke rather than evidence of real microphone capture. The 81 pre-existing shared customized files remain semantically unclassified in full, and Settings On/Off, Connect/MCP, bots, cloud/VM and credential integrations remain unverified. No feature was intentionally removed.

## Source-pinned CI resolution and remaining native acceptance — 2026-11-11

The former CI-pending observation for integration code SHA [`d3a59446`](https://github.com/ginmp8/pandacode-extended/commit/d3a59446bd9c208b89e8d999f3d001d5be2f6955) is now resolved: [push #43075335735](https://github.com/ginmp8/pandacode-extended/actions/runs/43075335735) and [PR #43075350618](https://github.com/ginmp8/pandacode-extended/actions/runs/43075350618) each passed **11/11 jobs**, including the prior Linux/ARM64 CI failure points. This verifies those GitHub Actions workflows on that exact code SHA, not a real-device or installed-app run.

All 81 shared file divergences were screened by changed-line sets. Their semantics, caller coverage and On/Off transitions are **not fully verified**, and no new definite runtime defect was established by this comparison. Real Linux microphone access, native Settings On/Off, Connect/MCP, bot delegation, cloud/VM, credential transfers, SQLite migration/rollback and installers remain outstanding acceptance gates. Extended **0.12.0 DRAFT** is not approved for release; CI of any subsequent documentation-only SHA must be evaluated independently.

## Original upstream 0.3.111 release-note qualifications — 2026-11-12

The [corrected upstream text](https://github.com/ginmp8/pandacode/commit/a2ba4b430951ee2995bdfda140d4f516cb493a78) clarifies existing limitations: start/send retry deduplication lasts at most one hour while the computer stays running; unresized photos can exceed the 5 MB attachment limit; raw Linux ALSA ids may not capture audio, `null` records silence, and unavailable saved JACK/OSS can error. It also distinguishes phone Full access from the desktop Full access for remote tasks switch. This is **documentation-only**, not evidence of new Extended defects or fixes. The text's mention of 0.3.112 is a source release-note statement, **not** proof that 0.3.112 functionality exists at this fork HEAD. Real-device and Settings On/Off acceptance remain unverified.

## Source 802fa847 Linux audio CI validation boundary — 2026-11-12

The updated fork configures a null ALSA default input for Linux x64 and ARM64 CI and requires release-profile `real_audio` capture. This exercises an audio pipeline with **synthetic silence**, not physical microphones, device routing, card enumeration or failed JACK/OSS services. The original 0.3.111 release note accurately qualifies its historical CI coverage rather than the new source HEAD. The added steps require independent CI validation; an ALSA initialization failure, if observed, must be diagnosed from its exact job/step, not assumed in advance. Extended Settings On/Off, remote/Connect/MCP, bots, cloud/VM, credential transfers, migrations and installed-platform behavior remain unverified.

## Source 0.3.112 acceptance gaps — 2026-11-12

The new upstream native quit confirmation has source-unit assertions for running-local vs cloud/headless/restart cases, but this Extended integration has **no independently executed native Quit/Cancel click-through**, process-stop verification or installed-build validation. Repeated quit requests and updater restarts should be exercised in the app. Full semantic review of source-vs-Extended customized paths, Settings On/Off, Connect, MCP, bots, cloud/VM, credential transfer and mobile/native behavior remains incomplete. This is an unverified acceptance gap, **not** a confirmed feature loss or security vulnerability. New integration SHA's CI must be checked separately; distribution `0.12.0` is still DRAFT.

## 2026-11-12 — CI cancellation guard regression test correction

The [push #43188294463](https://github.com/ginmp8/pandacode-extended/actions/runs/43188294463) and [PR #43188308169](https://github.com/ginmp8/pandacode-extended/actions/runs/43188308169) CI jobs for `dd15a184c618` both failed the same `test:release` assertion (73/74 release tests passed; 528/528 app tests passed). The Extended assertion was stale: both source and target workflows intentionally use `cancel-in-progress: ${{ github.event_name == 'pull_request' }}` to preserve active push **and manual** checks. The narrow test correction replaces its prior `!= 'push'` expectation, which would permit cancellation of manual runs; production CI semantics are unchanged. New exact-SHA push/PR CI must independently pass before this is classified as resolved. This test problem is not a demonstrated bot/Connect/MCP/VM runtime regression. Other native and semantic parity gaps remain open; **Extended 0.12.0 remains DRAFT**.
