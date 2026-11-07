# Extended 0.11.0 — dependency and security evidence

**Status:** New-cycle monitoring; no new 0.11.0 remediation is claimed.

The [0.10.0 security record](../0.10.0/dependency-security.md) is the preceding baseline. In particular, the Rust ZIP advisory remediation is **inherited from 0.10.0**, not a new 0.11.0 fix. Preserve existing remediated versions, compatible overrides and security checks while importing new upstream source.

## Incremental requirements

- [ ] Compare upstream lockfiles/manifests with Extended at the exact imported SHAs; preserve patched dependencies unless a verified safe update supersedes them.
- [ ] Run/inspect RustSec and npm audits on the **new final SHA**; identify advisory IDs, affected versions and exploitability evidence where known.
- [ ] Check Cargo, Node/mobile/Expo, platform/toolchain compatibility and relevant release/build workflows.
- [ ] Record any new remediation separately with before/after versions, upstream compatibility, tests and CI links.
- [ ] Keep Extended release packaging identity separate from the upstream application manifests.

| Evidence | Result |
| --- | --- |
| Opening prior cycle | [0.10.0 dependency-security](../0.10.0/dependency-security.md) |
| New 0.11.0 remediation | None verified at cycle opening |
| Final 0.11.0 audits and native acceptance | Pending |

## Source 0.3.100 import — verified static evidence

- [Upstream source `e0b65e51`](https://github.com/ginmp8/pandacode/commit/e0b65e51c4b6bc9c67982eaed08f7f13985fa01a) and [Extended integration `a28ce807`](https://github.com/ginmp8/pandacode-extended/commit/a28ce807a9bcc3a8583440ff6035deb1fd0854da). The upstream delta updated the root npm, Cargo and Tauri **application versions** to 0.3.100; it did not add a new third-party dependency in the inspected source delta.
- Extended's Cargo.lock continues to pin `zip` **9.0.5**, rather than reverting to upstream's previous ZIP version. The npm/Cargo lockfile security differences remain Extended-specific.
- All six root app-version fields were independently read at the integrated code SHA `7fbcb5582e835a0ea3c607cb6ba3be8790f01117` and matched upstream 0.3.100. [Push CI #42223802536](https://github.com/ginmp8/pandacode-extended/actions/runs/42223802536) and [PR CI #42223813242](https://github.com/ginmp8/pandacode-extended/actions/runs/42223813242) completed successfully at that exact SHA, including RustSec, npm audit, Rust checks/tests and mobile/Expo. These CI results do not demonstrate native binary packaging, real account-pool authentication, feature-control On/Off behavior or full platform acceptance. No new 0.11.0 vulnerability remediation is claimed.

## PandaCode 0.3.101 dependency checkpoint

Source: [`008017de`](https://github.com/ginmp8/pandacode/commit/008017de3316aea56af9294d4d3f8556b52174bb); integration: [`d7cf1546`](https://github.com/ginmp8/pandacode-extended/commit/d7cf15467bf8f25563db253e7b35a1efeb2e5b5e). The upstream 0.3.101 diff changes application-version metadata in the npm and Cargo manifests/lockfiles without claiming a new dependency remediation. The Extended-specific dependency overrides and lockfile differences were not overwritten wholesale. [Initial CI #42325893564](https://github.com/ginmp8/pandacode-extended/actions/runs/42325893564) was cancelled. Exact-SHA [push #42325973460](https://github.com/ginmp8/pandacode-extended/actions/runs/42325973460) and [PR #42325979123](https://github.com/ginmp8/pandacode-extended/actions/runs/42325979123) completed with npm dependency-audit and RustSec audit **successful**; release identity tests **failed** because the pinned source baseline remained on 0.3.100 while the application manifests already matched 0.3.101. The baseline is updated in the subsequent correction; new exact-SHA CI and native validation are still required. No new security-fix claim is made here.
