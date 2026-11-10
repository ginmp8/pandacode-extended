# Extended 0.12.0 — feature-control continuity

**Status:** Inherited control surface at the 0.11.0 baseline. No new 0.12.0 control or native acceptance is claimed.

Start from the [0.11.0 feature-controls record](../0.11.0/feature-controls.md) and the [full upstream parity contract](../upstream/parity-contract.md). Preserve existing Extended controls for automatic updates, Connect/remote access, paused bots, external MCP, cloud credentials and cloud/VM operations. Keep upstream code paths functional when the corresponding feature is enabled and authorized.

The 0.11.0 cycle also introduced **opt-in per-bot fresh-context behavior** and an **independent shared-channel history preference**. These are inherited starting functionality, not new 0.12.0 work. Ensure they remain separate, do not duplicate delegated context and do not silently reset queues.

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
