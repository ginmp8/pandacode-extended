# Extended 0.12.0 — feature-control continuity

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
