# Upstream-to-Extended reconciliation — 2026-11-10

> **Status:** Upstream tree reconciliation and automated validation passed on `fb67a0eb3af3b6c41a56917dc2aa16ff134afc9c` ([18/18 jobs](https://github.com/ginmp8/pandacode-extended/actions/runs/42910685007)). A follow-up integration hardening commit must pass its own exact-SHA CI; installed-app/manual acceptance remains separate.

## Frozen refs and constraints

- Source: `base/development` at `e7361c1c22355718cbbbf0e7ec4202a3e9673f61`, incorporating three commits since `4ef18f0fe5fda72190c6754711ae8d7fbc4ab200`; fork main at `9437b3a92cc3d41e899770f645f8a08449ce6bbf` (app 0.3.108).
- Original target: `extended/development` / PR #120 at `6467995ea3b533b039bcd393e4e6a15e847f5464` (not modified).
- Topic-branch prior integrated candidate: `bcf638821ee0295aa3bd1fe452dc7e1b965c746b` (one-parent, tree `c66fbd6ed621ee1520e427e62a3ed2d34881abb2`).
- Reconciliation updates the topic branch only, with a single-parent commit and no merges, force push or source-branch modifications.
- All original upstream and Extended capabilities remain in scope; a green CI result alone does not demonstrate native behavioral parity.

## Three-commit follow-up reconciliation

- Inspected all **23 changed paths** between `4ef18f0fe5fda72190c6754711ae8d7fbc4ab200` and `e7361c1c22355718cbbbf0e7ec4202a3e9673f61`. Adopted **14 source-only upstream updates** by their exact blob SHA, including native Rust Voice changes/tests, native Voice and release documentation, release gate script tests and the manually gated Extended release workflow.
- Semantically reconciled **9 concurrently modified paths**. Applied upstream release CI concurrency policy and its privacy wording to the existing Extended files. Retained both sets of Extended-specific release documentation rather than overwriting later Extended entries. In `docs/extended/README.md`, inserted the latest pinned source before the Extended-only ownership and documentation links; the remaining additions use matching source context.
- Native source changes: `src-tauri/src/voice/capture.rs`, `src-tauri/src/voice/local/{worker.rs,tests/worker_child.rs}`, `src-tauri/src/voice/session.rs`, and `src-tauri/src/voice/tests/basics.rs`.
- Preserved the user-controlled Extended **Catalogs** discovery/import experience in Rules & library while retaining upstream Skills Manager authority over per-harness installation and runtime state. Updated browser assertions to verify this intended difference and added explicit discovery/import/no-activation coverage. The native `skills_catalog*` contract remains opt-in.
- Earlier upstream 0.3.107 integration and the initial 32 conflict resolutions were published at `bcf638821ee0295aa3bd1fe452dc7e1b965c746b`. Their complex shared-feature behavior still requires independent manual/native acceptance; this follow-up does not certify all prior semantic conflict decisions.

## Verification gates

- [x] Confirm frozen source and target ref stability and single-parent ancestry at `fb67a0e`.
- [x] Exact-SHA CI on `fb67a0e`: 18/18 jobs passed, including TypeScript, Node/release, browser Skills/bot/Council, Rust, Clippy, Windows, macOS, Linux and mobile checks.
- [x] Recheck upstream path/mode inventory: no missing upstream tracked files, Extended-only paths retained.
- [ ] Run exact-SHA CI for subsequent integration fixes before marking them verified.
- [ ] Separately accept installed-app native Voice stop/close/crash paths, Settings/Skills/Catalogs persistence, real agent activation and bot isolation, Connect/MCP/cloud/VM controls and release failure modes where required.
- [ ] No merge, tag or release publication was authorized in this reconciliation.

## Complete 0.3.108 source refresh — 2026-11-10

- Newly pinned source: `base/development` at `ab6c99787bf8ece8eae70b51eb3d071b91353e41`, two commits ahead of the previous frozen source `e7361c1c22355718cbbbf0e7ec4202a3e9673f61`; the native application remains 0.3.108.
- Upstream delta contains six modified paths: `mobile/package.json` and `mobile/package-lock.json` were already byte-identical to the patched 1.4.3 upstream dependency fix in this topic branch; the four remaining Extended 0.12.0 documentation appendices were integrated without replacing preexisting Extended status/history.
- Complete Git tree inspection of this source and the prior topic candidate: 1,753 upstream blobs, 1,780 topic blobs; all upstream paths and modes present, 27 Extended-only blobs retained and 70 shared blobs intentionally divergent. Structural inclusion is not a guarantee of semantic equivalence for these shared files.
- The prior topic candidate `5b1db3950d622a9fbc682311435a9bf2814607c1` passed all 18 jobs in [exact-SHA CI #42920188868](https://github.com/ginmp8/pandacode-extended/actions/runs/42920188868), including native Rust/Clippy, TypeScript, mobile, security audit and Skills/Council browser regressions. This new source-refresh commit requires its own exact-SHA CI to demonstrate automated verification.
- The topic branch is strictly single-parent and the upstream and Extended development branches remain untouched. Installed-app/device and manually approved release acceptance remain outside automated CI. No merge, tags or publication are authorized.
