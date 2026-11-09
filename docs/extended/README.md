# PandaCode Extended documentation

This directory defines the **complete upstream functional parity contract** and the bounded Extended-only configuration, dependency remediations, release validation and evidence-backed known behavior. It is intentionally separate from upstream PandaCode documentation in `docs/`; Extended release notes live under `releases/extended/`, distinct from upstream release files at `releases/*.md`.

- **[Mandatory full upstream functional parity contract](upstream/parity-contract.md)** — complete copy, allowed Extended-only differences and verification gates
- **[Extended release-tag and publication workflow](releasing.md)** — source app version, Extended tags, manual GitHub release gating, and no npm publishing
**Current planned Extended release documentation:** **[0.12.0](0.12.0/README.md)** (incremental development cycle; not tagged, approved or distributed). The upstream application/build version is synchronized separately with the fork.

- [Extended 0.12.0 draft release notes](../../releases/extended/0.12.0.md)
- [Extended 0.12.0 feature controls](0.12.0/feature-controls.md)
- [Extended 0.12.0 dependency and security tracking](0.12.0/dependency-security.md)
- [Extended 0.12.0 known-issues register](0.12.0/known-issues.md)
- [Extended 0.12.0 release checklist](0.12.0/release-checklist.md)
- [Extended 0.11.0 release notes](../../releases/extended/0.11.0.md)
- [Extended 0.11.0 feature controls](0.11.0/feature-controls.md)
- [Extended 0.11.0 dependency and security tracking](0.11.0/dependency-security.md)
- [Extended 0.11.0 known-issues register](0.11.0/known-issues.md)
- [Extended 0.11.0 release checklist](0.11.0/release-checklist.md)
- [Extended 0.10.0 draft release notes](../../releases/extended/0.10.0.md)
- [Extended 0.10.0 feature controls](0.10.0/feature-controls.md)
- [Extended 0.10.0 dependency and security evidence](0.10.0/dependency-security.md)
- [Extended 0.10.0 known-issues register](0.10.0/known-issues.md)
- [Extended 0.10.0 release checklist](0.10.0/release-checklist.md)
- [Extended 0.9.0 historical candidate notes](../../releases/extended/0.9.0.md)
- [Extended 0.8.0 previous-cycle release notes](../../releases/extended/0.8.0.md)
- [Extended 0.8.0 release checklist](0.8.0/release-checklist.md)
- [Extended 0.7.0 historical source-candidate documentation](0.7.0/README.md)
- [Upstream 0.3.87 /btw side-thread parity candidate](upstream/0.3.87-btw-side-threads.md)
- [Current pinned upstream source identity (version and commit in JSON)](upstream/source-baseline.json)
- [Original upstream 0.3.89 release notes, mirrored without edits](../../releases/0.3.89.md)
- [Original upstream 0.3.90 release notes, mirrored without edits](../../releases/0.3.90.md)
- [Original upstream 0.3.91 release notes, mirrored without edits](../../releases/0.3.91.md)
- [Original upstream 0.3.92 release notes, mirrored without edits](../../releases/0.3.92.md)
- [Original upstream 0.3.93 release notes, mirrored without edits](../../releases/0.3.93.md)
- [Original upstream 0.3.94 release notes, mirrored without edits](../../releases/0.3.94.md)
- [Upstream 0.3.87 source fidelity and version repair](upstream/source-fidelity-0.3.87.md)
- [Original upstream 0.3.86 release notes, mirrored without edits](../../releases/0.3.86.md)
- [Original upstream 0.3.87 release notes, mirrored without edits](../../releases/0.3.87.md)
- [Extended 0.7.0 canonical release notes](../../releases/extended/0.7.0.md)

Do not copy these statements into upstream release notes or claim they apply to the upstream fork. A source candidate is not an installer release.

- [Original upstream 0.3.95 release notes](../../releases/0.3.95.md)
- [Original upstream 0.3.96 release notes](../../releases/0.3.96.md)
- [Original upstream 0.3.97 release notes](../../releases/0.3.97.md)
- [Original upstream 0.3.98 release notes](../../releases/0.3.98.md)
- [Original upstream 0.3.99 release notes](../../releases/0.3.99.md)
- [Original upstream 0.3.100 release notes](../../releases/0.3.100.md)
- [Original upstream 0.3.101 release notes](../../releases/0.3.101.md)
- [Original upstream 0.3.102 release notes](../../releases/0.3.102.md)

- [Original upstream 0.3.103 release notes](../../releases/0.3.103.md)
- [Original upstream 0.3.104 release notes](../../releases/0.3.104.md)
- [Original upstream 0.3.105 release notes](../../releases/0.3.105.md)

- [Original upstream 0.3.106 release notes](../../releases/0.3.106.md)

**Latest pinned source snapshot (2026-11-09):** [`0bfd5738`](https://github.com/ginmp8/pandacode/commit/0bfd57383621d5aa001d3871018920855cabec39), upstream app **0.3.106** (ten new source commits since the previous pin). A full-tree staged reconciliation incorporates **34 upstream changed paths** (26 exact upstream blobs, eight Extended-preserving patch adaptations), including two new files and original release notes. The candidate trees contain **1,567 upstream / 1,624 Extended paths, 1,487 byte/mode-exact shared, 80 divergent shared, 57 Extended-only and zero missing paths**. CI of the eventual Extended publication SHA and native Settings/Connect/MCP/cloud/VM acceptance are **not yet verified**. Planned Extended documentation remains **0.12.0**.

**Previous pinned source snapshot (2026-11-08):** [`f28c4df0`](https://github.com/ginmp8/pandacode/commit/f28c4df06c1f2d4ca3575eb9db186a2fe7cf8665), upstream application **0.3.105**, [Extended integration `36228b94`](https://github.com/ginmp8/pandacode-extended/commit/36228b949aaf8648109e2f8f8da441a8735cee41). All 38 upstream-changed paths were reconciled (4 new, 26 source-exact, 8 adapted); complete recursive inventories contain **1,565 upstream / 1,622 Extended paths, 1,485 exact shared, 80 divergent shared, 57 Extended-only, 0 missing or mode mismatches**. Original upstream release notes are preserved. Exact-code-SHA [push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42554247514) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42554252219) were pending when first queried. The 80 divergent shared paths and native Settings/account-pool/Connect/MCP/cloud/VM acceptance remain incomplete. Extended candidate version stays **0.12.0**.

**Previous pinned source snapshot (2026-11-08):** [`d5d51769`](https://github.com/ginmp8/pandacode/commit/d5d51769ccac15145e67c071ea06a5502c4cadcb), upstream application **0.3.104**, reconciled in [`base/development` code commit `6b6a39a8`](https://github.com/ginmp8/pandacode-extended/commit/6b6a39a82f508360572518519badb56e3df8313e) and documented at [`e33fb2fd`](https://github.com/ginmp8/pandacode-extended/commit/e33fb2fd93aaa130f5e480db7d0e227fe966e1cf). All **36 upstream-changed paths** were integrated (10 new, 14 refreshed binary screenshots, 7 source-exact textual updates, 5 adapted textual files). Complete nontruncated inventories: **1,561 upstream blobs, 1,618 Extended blobs, 1,481 exact shared, 80 modified shared, 57 Extended-only; 0 missing source paths or mode mismatches**. [Push CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42503386681) and [PR CI](https://github.com/ginmp8/pandacode-extended/actions/runs/42503391142) for `e33fb2fd` each passed **6/6 jobs**. Planned Extended release documentation remains **0.12.0**. The 80 shared differences require complete hunk review; native Settings On/Off, Council, Connect/MCP/cloud/VM, and installed-platform acceptance remain unverified.

**Previous pinned source snapshot:** [`174f0c0b`](https://github.com/ginmp8/pandacode/commit/174f0c0b17ae1b036210cbc5dddf45f0701cd8b0), upstream application **0.3.103**, including the Council scored review/fix loops and original `releases/0.3.103.md`. Integrated on [`base/development` at `3f95db51`](https://github.com/ginmp8/pandacode-extended/commit/3f95db516e607cbe6fee5585e61d09ab2c118a71) with **41 changed upstream paths** (9 additions, 23 exact replacements, 9 three-way adaptations). Complete recursive inventories: **1,551 upstream files, 1,608 Extended files, 0 upstream paths missing, 80 existing shared divergences, 57 Extended-only files**, and no mode differences. The planned Extended release remains **0.12.0**. Live Council runs, Settings On/Off, Connect/MCP/cloud and native-platform acceptance remain unverified; exact-SHA CI is separate from this source-copy evidence.
