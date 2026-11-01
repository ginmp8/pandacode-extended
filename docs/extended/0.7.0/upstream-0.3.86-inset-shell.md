# Upstream PandaCode 0.3.86: inset shell and calmer motion

**Status:** Feature integration candidate for PandaCode Extended 0.7.0. Not released or merged. Upstream 0.3.86 parity is **not** claimed.

## Pinned source and destination

- Base: `ginmp8/pandacode-extended` `base/hardened@c795e4c7307c257ccb24b9b0faa651c3dc500fb2` (Extended 0.7.0 / upstream 0.3.85 baseline).
- Previous upstream release: `ginmp8/pandacode@df884aa0d347a00e7407bedbd46c6f7ee69342e3` (0.3.85).
- Feature commit: `ginmp8/pandacode@944251a9a6ba5db8d93fd2e6f674aed17c493389`.
- Pinned upstream main: `ginmp8/pandacode@e55b19acc3d9ef43567c9654a688916376634901` (0.3.86).
- Review target: Extended `main`. The protected concept of a clean `base/hardened` is left unchanged.

## Scope and implementation method

The upstream 0.3.85 → 0.3.86 diff was assessed as one feature, rather than reimplementing its CSS, motion, state transitions and tests independently. The 20 imported paths (AGENTS.md, 13 `src/` files, 6 `tests/` files) had byte-identical Git blobs in Extended `base/hardened` and upstream 0.3.85. Their upstream 0.3.86 blob contents were then copied intact into a separate feature branch, with Git blob SHA equality checked after upload. The new `src/inset-shell.css` is included and imported by `src/main.ts`.

The feature introduces an inset conversation/inspector/Settings layout, smoother thread switching, delayed closing of the inspector to prevent text reflow under the transition, focus handoff, sidebar transitions, first-load transcript hold, and updated UI browser regression cases.

**Deliberately excluded:** upstream `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, `website/src/data/release-dates.json`, and `releases/0.3.86.md`. Their upstream changes are version/release metadata, not needed for the visual feature. Extended security-patched npm/Rust dependencies, its version and distribution policy, and its separate release documentation are retained. No upstream release tag or auto-updater behavior is imported by this change.

## Acceptance and verification

- Desktop TypeScript build: `npm ci && npm run build`.
- Unit and release regressions: `npm test && npm run test:release`.
- Browser coverage: `npx playwright test tests/layout-preferences.spec.ts tests/ui.spec.ts tests/wallpapers.spec.ts tests/browser.spec.ts tests/thread-header-overlay.spec.ts tests/action-dock.spec.ts`. Exercise dark/light modes, inspector open/close and resize, Settings, sidebar collapse, Board/GitHub, wallpaper and reduced motion.
- Repository CI must pass on the PR's exact final head SHA, including dependency audits and Rust/mobile regression jobs. Do not infer CI success from a different commit.
- Manual native UI review on macOS/Windows is still needed to confirm window controls, material/tint contrast, accessibility preferences and embedded browser overlay behavior.

**Not verified at the point of initial import:** full local desktop/browser execution, native macOS/Windows rendering, or release readiness. These require executed checks on the submitted candidate; imported upstream tests alone are not proof.

## Recovery and boundaries

The source snapshot remains available at `base/hardened@c795e4c`. The feature is isolated in `feature/0.7.0-inset-shell-fork-0.3.86`, with no changes to `base/hardened`, no merge, no release and no edits to the fork. If a regression is found, amend only this feature branch after examining the affected behavior and tests.
