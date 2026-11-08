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

**Latest pinned source snapshot (2026-11-08):** [`7134edf5`](https://github.com/ginmp8/pandacode/commit/7134edf501d31acc8bec28ff74be9d1dba171669), native application **0.3.102**. This includes the ZCode model-selection/team-catalog correction and original upstream `releases/0.3.102.md`. Integration source commit: [`7db3e2c4`](https://github.com/ginmp8/pandacode-extended/commit/7db3e2c4ae4c5b5dc48e2de50eed5980d247e8be). Complete recursive tree comparison after code integration: **0 upstream paths missing, 80 pre-existing modified shared files, 57 Extended-only files**. The 12 changed upstream paths were reconciled with existing Extended changes. Runtime Settings On/Off, native platform and external integration acceptance remain unverified; exact-SHA CI must be checked independently. The planned Extended distribution remains **0.12.0**, not a released artifact.
