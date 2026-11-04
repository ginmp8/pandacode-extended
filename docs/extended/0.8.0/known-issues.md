# PandaCode Extended 0.8.0 — inherited verification register

**Status:** initial carry-forward from the [0.7.0 evidence register](../0.7.0/known-issues.md). This is not evidence that a concern remains exploitable, or that it was fixed during 0.8.0. Each item must be reassessed on the final candidate SHA before release acceptance.

| Original ID | Evidence classification carried forward | 0.8.0 verification |
| --- | --- | --- |
| EXT-070-KI-01 | Unverified integration concern: external MCP isolation for Codex when disabled | Test effective new-session Codex configuration |
| EXT-070-KI-02 | Observed sequencing limitation: cloud provisioning may precede credential-transfer gating | Confirm actual resource/cost side effects and user disclosure |
| EXT-070-KI-03 | Intended non-retroactive behavior: disabling transfers does not revoke credentials already copied | Validate scope; do not classify alone as a bug |
| EXT-070-KI-04 | Recovery-path defect candidate: invalid settings reads may fall back to enabled defaults | Reproduce corruption/restart behavior and verify desired fail-safe policy |
| EXT-070-KI-05 | Intended new-process scope: already running agents retain their launch-time MCP settings | Verify user-facing behavior; do not call it an unproven exploit |
| EXT-070-KI-06 | Prior source remediation: mobile/Expo dependency advisories | Recheck dependency graph and mobile tests |
| EXT-070-KI-07 | Prior source remediation: legacy Extended SQLite schema collision | Validate upgrade against a representative existing user database |

## Newly inherited upstream 0.3.91 verification limits

- Linux Wayland paste and composer undo/redo were reported as browser-tested upstream, not verified on real Linux desktop compositors.
- Windows conversation import behavior was verified upstream using Windows-style paths on macOS, not an actual Windows desktop.
- ACP mode fallback judges relative permission level by mode name; validate unusual provider mode names before calling that behavior fully proven.
- New clipboard dependencies and existing Extended dependency remediations require fresh cargo test/Clippy and RustSec evidence on the final source-integrated commit.

These are evidence limitations, not automatically confirmed vulnerabilities or disabled features.

## Other release-gate gaps

- Real Windows, macOS and Linux native-app startup, onboarding and installer checks are not established merely by successful source CI.
- OpenCode V2 effort selection should be exercised with an installed CLI and a model advertising adjustable levels; unit/recorded-protocol tests do not replace this check.
- Multi-machine orchestration and Connect need testing across **two real computers**, including pairing, access downgrades, offline behavior and stop semantics.
- The independently planned Extended `0.8.0` version, installer naming and release artifacts are not established by upstream `0.3.91` manifests.

See [release checklist](release-checklist.md) and [feature controls](feature-controls.md). Reclassify and remove a carried-forward concern only after recording verifiable evidence; never copy an old CI success to the new SHA.

## CI status for current source integration

[CI #41498498966](https://github.com/ginmp8/pandacode-extended/actions/runs/41498498966) completed six jobs successfully for the 0.3.91 import commit `083e764485f08ea1b5cabbf3b3d18ea9b426e89c`. This does not close the native Linux Wayland/Windows GUI test gaps, unusual ACP mode-name behavior, real two-device Connect or full Extended control On/Off acceptance.

## Source 0.3.92 acceptance gaps

The upstream redesign adds live GPU-backed thread scenes, one-card-per-turn rendering and up to six tiled thread views. Upstream's [0.3.92 release notes](../../../releases/0.3.92.md) report browser preview coverage but not native installed-app tiling/scene verification or real agent streaming into an unfocused tile. Extended additionally requires verification that the new Settings navigation still exposes working Feature controls, that Connect Off/On gates both relay and direct operations correctly, and that prior bot, MCP and cloud permissions still behave as configured. These are unverified scenarios, not proven regressions.

## Source 0.3.93 verification gaps

- Verify nested/background subagents eventually report back to their lead, including concurrent completions and silent or empty turns; verify intentional limits documented in upstream [0.3.93 notes](../../../releases/0.3.93.md).
- Exercise team persistence beyond the previous history/round/workload ceilings, delayed lead retries and interruption/resume paths; verify the new remote/mobile Resume affordance and paged worker history.
- Verify Codex with and without a user-defined thread limit while Extended MCP isolation is Off and On; validate that disabling external MCP does not suppress the permitted authenticated per-run host.
- Recheck Settings, bots, credential transfer and Connect with new upstream orchestration behavior. The source diff contains no evidence that native On/Off testing was executed for this candidate.

These are release-gate tests, not a confirmed vulnerability or a reason to remove inherited functionality.

## Source 0.3.94 unverified behavior

- Verify expanding/collapsing worker progress across streaming, redraw, error and completed states, including screen-reader state.
- Verify shrinking/replacing live reply cards do not leave blank space or scroll jumps, including reduced motion.
- Verify Extended controls, remote progress and bot pause work with the inherited UI update. No regression is asserted without reproduction.
