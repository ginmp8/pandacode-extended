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
- The independently planned Extended `0.8.0` version, installer naming and release artifacts are not established by upstream `0.3.90` manifests.

See [release checklist](release-checklist.md) and [feature controls](feature-controls.md). Reclassify and remove a carried-forward concern only after recording verifiable evidence; never copy an old CI success to the new SHA.
