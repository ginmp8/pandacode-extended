# Extended 0.7.0 — experimental upstream consolidation

Status: **experimental, partial integration; not reviewed for release; no claim of upstream parity**.

## Frozen identities

- Integration branch: `integration/0.7.0-upstream-0.3.79`, branched from Extended `main@f0105e7db85a0d7bc2e70726716fcfa939fac182`.
- Preservation baseline: Extended `development@c840e3cf92c366ba8e7cc8a4b6f1d1f83a1bccb0`, 277 commits ahead of main at the comparison date. Incorporated by fast-forward into the isolated integration branch; no mutation of `development`.
- Upstream feature snapshot: `ginmp8/pandacode@ffac9f445a99542d3ee97db1f71cb9293119fad9` (`chore: release 0.3.79`). Newer upstream `main` documentation commits are deliberately outside the snapshot.
- This candidate retains the existing Extended product version and historical `EXTENDED-RELEASE.json` provenance. **Do not** change the tracked upstream version until reviewed/accepted coverage is demonstrated.

## Already retained, not reimplemented

The prior `development` work remains present. It includes existing 0.7.0 integration and Extended-specific changes, but **a commit count is not acceptance evidence**. Open 0.7.0 issues #51–#61, #65, #75 and #76, and 0.8.0 issues #68–#74, are not automatically resolved by this consolidation.

## Implemented from upstream 0.3.79 (first controlled delta)

- `src/main.ts`: New task must not reopen a completed/settled draft; retain Extended's `reuseDraft` parameter and existing task-selection behavior.
- `src/sidebar-tree.css`: frosted rather than opaque row-actions treatment, with a solid fallback when reduced transparency is requested, while retaining Extended's keyboard/focus and unread indicator semantics.
- `tests/worktrees.spec.ts`: regression for settled draft after worktree removal; added to browser CI shard without disabling existing tests.
- Original upstream feature commit: `ed6d736746202e662e186c2d0380a3a42c06a70e` (three touched files); adapted, not blindly replaced.
- `0.3.77` Changes diff virtualization: incorporated pinned source `src/diff-viewer.ts` with Extended's keyboard-navigation fix retained; added worker-based highlight/virtualization, pure diff chunking/preload helper, `.cv-list` scroll container and per-shadow style propagation. Existing Extended `changes-view` behavior and tests are retained. This is **not** performance-parity evidence and requires independent same-SHA validation.

## Hard security invariants for subsequent integration

Preserve Extended-specific boundaries, including: disabled app/agent auto-update and updater capabilities; no silent install or `npx` bootstrap; safe Windows packaging; per-device Connect trust, revocation/replay checks and remote Full default-off; explicit bot outbound approvals; secrets in OS vault; safe preview/CSP; no implicit credential/provisioning flows; no unsolicited telemetry or privileged network calls; Board/GitHub/worktree integration, and independent Extended release identity. Preserve `HARDENED-REVISION.json` as historical evidence.

Reference: `SECURITY-HARDENING.md`, `SECURITY-INSTALLATION-REVIEW.md`, `docs/VERSIONING.md` and `tests/install-policy.test.mjs`. The new 0.3.77 Shiki worker must be bundled as an ES module (`vite.config.ts: worker.format = 'es'`), not Vite's default IIFE, because worker code splits into local chunks. This was detected by the draft PR build failure and fixed in the candidate; no external worker CDN is allowed.

## Outstanding upstream coverage (must be implemented and independently verified)

| Upstream range | Primary domain | Status |
| --- | --- | --- |
| 0.3.51–0.3.71 | Conversation, bots, Connect, sidebar, agent teams, router/model health | Existing development work retained; requirement-by-requirement audit **pending** |
| 0.3.72–0.3.73 | Model catalogs and Finished/Settled sidebar | **Pending** |
| 0.3.74 | Rich trace rendering | **Pending** |
| 0.3.75 | Durable nested subworkers and model coordination | **Pending** |
| 0.3.76 | Long-session memory control | **Pending** |
| 0.3.77 | Virtualized Changes diff rendering | Code adapted from pinned upstream; CI/performance/security acceptance **pending** |
| 0.3.78 | Progress snapshot/PiP | **Pending** (high-risk local filesystem and SVG boundaries) |
| 0.3.79 | Settled draft fix and sidebar frost | Adapted in this branch; CI/behavioral acceptance **pending** |

This document is a candidate ledger, **not** a promise of feature equivalence. Only mark a row delivered after its upstream source/test diff, Extended consumers, negative/security tests and required same-SHA CI evidence are reconciled. Do not close, move or supersede existing issues based on this candidate alone.

## Validation contract and review gate

- Run `npm run build`, `npm test`, `npm run test:release`, `npm run test:install-security` and `npm run site:build`.
- Run browser regressions including `tests/worktrees.spec.ts`, `tests/changes-view.spec.ts`, `tests/sidebar-sections.spec.ts`, `tests/board.spec.ts`, `tests/github-hub.spec.ts`, `tests/remote-security.spec.ts`.
- Verify diff worker is local/self-hosted under the existing Tauri CSP (no `blob:`), serialized patches are bounded, no external calls, stale UI viewers are disposed, and keyboard horizontal scrolling remains operable. Run a reproducible large-diff performance benchmark before claiming a speed improvement.
- Run mobile + relay typecheck/tests, Rust tests/clippy with `--locked`, and native/physical Windows checks for packaging and Connect security when available.
- Prove absence of restored updater plugin, network bootstrap, remote Full-by-default, unapproved telemetry and privilege expansion with dedicated negative tests/static diff review.
- Do not merge to `main`, retarget `development`, publish a release, update global automation scope or change issue labels until review/acceptance.
