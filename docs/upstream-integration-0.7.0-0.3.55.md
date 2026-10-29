# Upstream 0.3.55 sidebar integration ledger — PandaCode Extended 0.7.0

Issue: [#49](https://github.com/ginmp8/pandacode-extended/issues/49).
Reference: maintained [ginmp8/pandacode 0.3.55](https://github.com/ginmp8/pandacode/commit/62f06530b4cadfe12fd26ec3a909a948622528aa).
Observed Extended implementation through `44ace675579b16fa8104dd60e2817fd3d07e010f`.

**Status:** integration candidate; not published. This ledger records source/code/test coverage, not full behavioral parity or acceptance. The last fully successful baseline CI is [run 40828670642](https://github.com/ginmp8/pandacode-extended/actions/runs/40828670642) (10/10 jobs and check-runs on `48f5c5efd32d1fee9fbe79bf793e9bc835cd9b89`). A subsequent input-validation fix `44ace675579b16fa8104dd60e2817fd3d07e010f` and this documentation change still require exact-final-SHA CI; this is not final acceptance.

## Source-to-target requirements

| Upstream 0.3.55 / Extended requirement | Target implementation and evidence | State |
| --- | --- | --- |
| Stable Active/Finished/Pinned/Chats/Tasks/Settled sections, selected-row hold and settled paging | `src/sidebar-sections.ts`, `src/main.ts`; `tests/sidebar-sections.test.mjs`, `tests/sidebar-sections.spec.ts` | Present in source; final exact-SHA browser CI required |
| Manual, recent and created task sorting without mutating saved manual ranks | `src/sidebar-sort.ts` comparators; `src/main.ts` Sort and view; `tests/sidebar-sort.test.mjs`, `tests/sidebar-keyboard.spec.ts` | Present; restart and larger-workload manual evidence pending |
| Stable added-project metadata, persisted/revalidated manual folder ordering | `src/sidebar-sort.ts` uses `Project.addedOrder`, rejects blank saved project IDs (`44ace675579b16fa8104dd60e2817fd3d07e010f`); `src/sidebar-tree.ts`; `tests/sidebar-sort.test.mjs` | Code retained for compatibility; new corrupt-preference regression requires final CI; project-folder presentation is superseded below |
| Slash search, title/branch/project and both GitHub issue and PR references, including collapsed/settled results | `src/main.ts` filters with `matchesFilter`; `src/sidebar-sort.ts`; `tests/sidebar-sections.spec.ts`, `tests/sidebar-sort.test.mjs`. Commit `3defdd475a10cd37672932498b62d410dfd7bd32` fixes the dual-link gap | Candidate fix published; final CI pending |
| Keyboard-only list focus, arrows, shortcuts and menu access without stealing text editor/IME keystrokes | `src/main.ts`, `tests/sidebar-keyboard.spec.ts`; commit `c2d3eb353156571783a27fab6854203a3e574e71` protects nested editors and IME composition | Browser coverage in green CI `40828670642`; independent screen-reader and cross-OS evidence pending |
| Task menu: pin/unpin, rename, unread/read, new task from branch, copy branch/path/ID, settle/reopen, Move to project and confirmed deletion | `src/main.ts::showSidebarTaskMenu` restricts Move to project to the selected, unstarted local task and uses the existing `showProjectPicker`; `tests/sidebar-keyboard.spec.ts` exercises the chooser and disallowed started task | Covered in green CI `40828670642`; native permission-failure and other-platform evidence pending |
| Undo unpin and settle; preserve selected stage and saved row order | `src/main.ts::pinTask`, `sidebarTaskAction`, `notify`; `tests/sidebar-sections.spec.ts` and `tests/sidebar-keyboard.spec.ts` | Basic paths present; externally concurrent/stale undo and exact position rollback need verification |
| Compact rows, stage/attention indicators, accessibility and theme contrast | `src/main.ts`, `tests/sidebar-sections.spec.ts`, `tests/sidebar-keyboard.spec.ts` | Automated scenarios exist; physical devices, zoom, wallpaper, screen readers and cross-OS evidence pending |
| Extended-specific Board/GitHub/approval/remote restrictions | Existing Board, GitHub, task-stage and remote regression suites | Preserve; full final exact-SHA regression/security evidence required |

## Explicit later-version supersession

[Issue #58 / upstream 0.3.66](https://github.com/ginmp8/pandacode-extended/issues/58) takes precedence over incompatible historical 0.3.55 presentation/lifecycle details. The current integration uses **Finished** (not age-limited Needs review), **Settled** (persisted task stage still `completed`), flat All-projects sections, no archive screen or seven-day auto-archive, and no project folder/group/sort controls. Do **not** reintroduce deprecated 0.3.55 UI to satisfy this historical issue. Underlying sort/tree helpers may remain for compatibility.

## Acceptance work still pending

1. Check complete required CI on the exact final `development` SHA after the `44ace675...` input-validation fix and this ledger; older green runs are baseline evidence, not final acceptance.
2. Record the affected sidebar, keyboard, Board, GitHub and security regressions on the final SHA. Run `40828670642` passed ten checks including desktop browser shards on the previous SHA; do not extrapolate manual platform/accessibility acceptance.
3. Validate Move to project in native-app environments and error/permission paths. The guarded chooser and preview browser test exist, but no direct project/path reassignment may bypass native permission, worktree ownership or task consistency.
4. Verify undo against concurrent changes, error responses and stage/position restoration; record manual focus, IME, screen reader, narrow layout, zoom and cross-platform evidence when executed.
5. Close #49 only when applicable unsuperseded requirements are evidenced or expressly accepted by the operator. No product version bump, release/tag, merge or publication is implied.
