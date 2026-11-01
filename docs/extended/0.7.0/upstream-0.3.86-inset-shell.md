# Upstream PandaCode 0.3.86: inset shell and calmer motion

**Status:** Feature source merged into Extended `main` by [PR #90](https://github.com/ginmp8/pandacode-extended/pull/90) on 2026-11-01, at commit `94b61ac97af1fb14a9f9aafca30a3ccceb20cebe`; the Extended product release remains unpublished, with native QA still pending. Upstream 0.3.86 package/version parity is **not** claimed.

## Pinned source and destination

- Historical import base (at the time of PR #90): Extended commit `c795e4c7307c257ccb24b9b0faa651c3dc500fb2` (upstream 0.3.85 baseline). This is not the current branch HEAD.
- Previous upstream release: `ginmp8/pandacode@df884aa0d347a00e7407bedbd46c6f7ee69342e3` (0.3.85).
- Feature commit: `ginmp8/pandacode@944251a9a6ba5db8d93fd2e6f674aed17c493389`.
- Pinned upstream main: `ginmp8/pandacode@e55b19acc3d9ef43567c9654a688916376634901` (0.3.86).
- Merge destination: Extended `main@94b61ac97af1fb14a9f9aafca30a3ccceb20cebe` (PR #90). On the later `base/development@ab08af492446d889d4a4ddc60a5c06a8c6a11bd0`, the 0.3.86 feature remains present, with only `src/main.ts` advanced to upstream 0.3.87.

## Scope and implementation method

The upstream 0.3.85 → 0.3.86 diff was assessed as one feature, rather than reimplementing its CSS, motion, state transitions and tests independently. The 20 imported paths (AGENTS.md, 13 `src/` files, 6 `tests/` files) had byte-identical Git blobs in Extended `base/hardened` and upstream 0.3.85. Their upstream 0.3.86 blob contents were then copied intact into a separate feature branch, with Git blob SHA equality checked after upload. The new `src/inset-shell.css` is included and imported by `src/main.ts`.

The feature introduces an inset conversation/inspector/Settings layout, smoother thread switching, delayed closing of the inspector to prevent text reflow under the transition, focus handoff, sidebar transitions, first-load transcript hold, and updated UI browser regression cases.

**Faithful upstream release artifacts (corrective import):** [`releases/0.3.86.md`](../../../releases/0.3.86.md) is copied **byte-for-byte at the original fork path** from the pinned 0.3.86 release (`54f325723f1f8a5bbcf7842759e3ea96d060782c`). The successive `releases/0.3.87.md`, `docs/btw.md`, updated `AGENTS.md` and `website/src/data/release-dates.json` (which includes both 0.3.86 and 0.3.87 dates) are also copied at their original paths from the pinned 0.3.87 release; the original release notes must not be stripped just because the Extended release version is different.

**Explicitly Extended-owned exceptions (not a full-tree mirror):** `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and `src-tauri/tauri.conf.json` retain the Extended version/dependency/build/distribution state instead of applying the fork's 0.3.86/0.3.87 version-only changes. These exact exceptions must be verified from their diffs, not assumed for arbitrary future commits. Retaining them protects Extended security remediation and prevents claiming the product was released at upstream 0.3.86/0.3.87. Copying a Markdown release note **does not** tag, package, authorize or publish an Extended release.

## Acceptance and verification

- Desktop TypeScript build: `npm ci && npm run build`.
- Unit and release regressions: `npm test && npm run test:release`.
- Browser coverage: `npx playwright test tests/layout-preferences.spec.ts tests/ui.spec.ts tests/wallpapers.spec.ts tests/browser.spec.ts tests/thread-header-overlay.spec.ts tests/action-dock.spec.ts`. Exercise dark/light modes, inspector open/close and resize, Settings, sidebar collapse, Board/GitHub, wallpaper and reduced motion.
- Repository CI on the merged `main` commit [`94b61ac`](https://github.com/ginmp8/pandacode-extended/actions/runs/41270578636) completed successfully. This does not validate the later `base/development@ab08af` `/btw` change; its exact-SHA CI and native/browser acceptance still need execution.
- Manual native UI review on macOS/Windows is still needed to confirm window controls, material/tint contrast, accessibility preferences and embedded browser overlay behavior.

**Not verified at the point of initial import:** full local desktop/browser execution, native macOS/Windows rendering, or release readiness. These require executed checks on the submitted candidate; imported upstream tests alone are not proof.

## Recovery and boundaries

The original source baseline is the immutable Extended commit `c795e4c`. The feature was initially isolated in `feature/0.7.0-inset-shell-fork-0.3.86`, then **merged into `main` through PR #90**. Do not amend or rewrite the old feature branch as if it were still unmerged. If a regression is verified, use a new scoped corrective commit on the authorized integration branch with fresh HEAD/CI checks; do not reset, force-push or discard subsequent work. The upstream fork is unchanged.

## Sequential source fidelity verification (2026-11-01)

The complete upstream `0.3.85..0.3.86` comparison contains two commits (the functional commit and `chore: release 0.3.86`) and 27 changed paths: 19 implementation/test files, `AGENTS.md`, and 7 release/version/lockfile paths. At the pinned Extended `base/development@ab08af492446d889d4a4ddc60a5c06a8c6a11bd0`, **18 of the 19 implementation/test blobs exactly match the 0.3.86 fork release**. The 19th, `src/main.ts`, exactly matches the *successor* fork release 0.3.87 (which includes the 0.3.86 feature). This is correct forward progression, not a missing or downgraded predecessor.

The `AGENTS.md` document is now advanced to the pinned upstream 0.3.87 revision, preserving its 0.3.86 feature description. The upstream 0.3.86 and 0.3.87 original release notes and website dates are now present with matching Git blobs; only the five explicitly identified Extended-owned version/build/dependency files deliberately differ from the upstream snapshots. The 0.3.87 feature's four source/test files are all exact matches to upstream 0.3.87, as recorded in [the separate, release-unmapped /btw port audit](../upstream/0.3.87-btw-side-threads.md). **These are source hash checks, not executed integration, native, accessibility or security tests**.
