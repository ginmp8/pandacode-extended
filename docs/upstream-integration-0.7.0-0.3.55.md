# Upstream 0.3.55 sidebar integration ledger — PandaCode Extended 0.7.0

Issue: [#49](https://github.com/ginmp8/pandacode-extended/issues/49).
Reference: maintained [ginmp8/pandacode 0.3.55](https://github.com/ginmp8/pandacode/commit/62f06530b4cadfe12fd26ec3a909a948622528aa).
Observed Extended implementation through `ed96b4654eea750f849651c4869be8020b168374`.

**Status:** integration candidate; not published or issue-accepted. [CI run 40833540899](https://github.com/ginmp8/pandacode-extended/actions/runs/40833540899) completed all 10/10 GitHub check-runs and jobs successfully at `ed96b4654eea750f849651c4869be8020b168374` (Rust, typecheck/unit/site, browser, Connect, chat regressions). This ledger update itself requires a new final-SHA CI check. CI success proves the automated suites ran; it does not establish complete manual accessibility/OS evidence or mandatory specification-conformance acceptance.

## Source-to-target requirements

| Upstream 0.3.55 / Extended requirement | Target implementation and evidence | State |
| --- | --- | --- |
| Stable Active/Finished/Pinned/Chats/Tasks/Settled sections, selected-row hold and settled paging | `src/sidebar-sections.ts`, `src/main.ts`; `tests/sidebar-sections.test.mjs`, `tests/sidebar-sections.spec.ts` | Automated coverage passed at `ed96b465` in run `40833540899`; manual acceptance remains open |
| Manual, recent and created task sorting without mutating saved manual ranks | `src/sidebar-sort.ts` comparators; `src/main.ts` Sort and view; `tests/sidebar-sort.test.mjs`, `tests/sidebar-keyboard.spec.ts` | Present; restart and larger-workload manual evidence pending |
| Stable added-project metadata, persisted/revalidated manual folder ordering | `src/sidebar-sort.ts` uses `Project.addedOrder`, rejects blank saved project IDs (`44ace675579b16fa8104dd60e2817fd3d07e010f`); `src/sidebar-tree.ts`; `tests/sidebar-sort.test.mjs` | Unit regressions passed in `40833540899`; project-folder presentation is superseded below |
| Slash search, title/branch/project and both GitHub issue and PR references, including collapsed/settled results | `src/main.ts` filters with `matchesFilter`; `src/sidebar-sort.ts`; `tests/sidebar-sections.spec.ts`, `tests/sidebar-sort.test.mjs`. Commit `3defdd475a10cd37672932498b62d410dfd7bd32` fixes the dual-link gap | Automated coverage passed in `40833540899` |
| Keyboard-only list focus, arrows, shortcuts and menu access without stealing text editor/IME keystrokes | `src/main.ts`, `tests/sidebar-keyboard.spec.ts`; commit `c2d3eb353156571783a27fab6854203a3e574e71` protects nested editors and IME composition | Browser coverage in green CI `40828670642`; independent screen-reader and cross-OS evidence pending |
| Task menu: pin/unpin, rename, unread/read, new task from branch, copy branch/path/ID, settle/reopen, Move to project and confirmed deletion | `src/main.ts::showSidebarTaskMenu` restricts Move to project to selected, unstarted local tasks, and uses existing `showProjectPicker`; commit `fde2edc179c0317230e248c0a5aca0d0036c8fd0` additionally rejects remote task rehoming in native `Store::configure` and UI; `tests/sidebar-keyboard.spec.ts` covers normal and remote menu behavior and Rust tests assert no mutation | Automated frontend and Rust coverage passed in `40833540899`; full native/manual platform evidence remains pending |
| Undo unpin and settle; preserve selected stage and saved row order | `src/main.ts::pinTask`, `sidebarTaskAction`, `notify`; `tests/sidebar-sections.spec.ts` and `tests/sidebar-keyboard.spec.ts` | Basic paths present; externally concurrent/stale undo and exact position rollback need verification |
| Compact rows, stage/attention indicators, accessibility and theme contrast | `src/main.ts`, `tests/sidebar-sections.spec.ts`, `tests/sidebar-keyboard.spec.ts` | Automated scenarios exist; physical devices, zoom, wallpaper, screen readers and cross-OS evidence pending |
| Extended-specific Board/GitHub/approval/remote restrictions | Existing Board, GitHub, task-stage and remote regression suites, plus `Store::configure` remote project guard | Required automated suites passed on `ed96b465`; platform/manual security and permission evidence remain pending |

## Explicit later-version supersession

[Issue #58 / upstream 0.3.66](https://github.com/ginmp8/pandacode-extended/issues/58) takes precedence over incompatible historical 0.3.55 presentation/lifecycle details. The current integration uses **Finished** (not age-limited Needs review), **Settled** (persisted task stage still `completed`), flat All-projects sections, no archive screen or seven-day auto-archive, and no project folder/group/sort controls. Do **not** reintroduce deprecated 0.3.55 UI to satisfy this historical issue. Underlying sort/tree helpers may remain for compatibility.

## Acceptance work still pending

1. Recheck all mandatory CI/check-runs on the latest `development` SHA after this ledger change. Previous `ed96b465` candidate CI `40833540899` was green; do not reuse it as final-SHA proof.
2. Preserve the documented `40833540899` 10/10 automated regression result and obtain manual screen-reader, focus, zoom, wallpaper, cross-OS/device and native UI evidence. Browser automation does not substitute for those observations.
3. Validate Move to project natively across devices, access restrictions, failures and retries. `fde2edc` prevents remote local rehoming in Rust and UI, and `ed96b465` adds the browser regression; direct project/path reassignment still must respect native permissions and worktree ownership.
4. Verify undo against concurrent changes, error responses and stage/position restoration; record manual focus, IME, screen reader, narrow layout, zoom and cross-platform evidence when executed.
5. Execute the required installed spec-conformance auditor with global, nonpartial `COMPLIANT` and every applicable requirement `PASS`, plus final exact-SHA green CI; if the auditor is inaccessible, keep #49 open. No product version bump, release/tag, merge or publication is implied.
