# PandaCode Extended — original upstream release history

An upstream PandaCode release note describes the original project. Versions `0.3.29`–`0.3.48` are retained as upstream references; the [0.5.0 integration coverage](upstream-integration-0.5.0.md) separately maps the implemented `0.3.42`–`0.3.47` changes and Extended differences. See [Extended source releases](https://github.com/ginmp8/pandacode-extended/releases) for published builds/tags and [upstream releases](https://github.com/aicodeking/pandacode/releases) for original release notes.

| Upstream reference | Note | Extended status |
|---|---|---|
| 0.3.29 | [Upstream 0.3.29](../releases/0.3.29.md) | Reference only; no parity claim |
| 0.3.30 | [Upstream 0.3.30](../releases/0.3.30.md) | Reference only; no parity claim |
| 0.3.31 | [Upstream 0.3.31](../releases/0.3.31.md) | Reference only; no parity claim |
| 0.3.32 | [Upstream 0.3.32](../releases/0.3.32.md) | Reference only; no parity claim |
| 0.3.33 | [Upstream 0.3.33](../releases/0.3.33.md) | Reference only; no parity claim |
| 0.3.34 | [Upstream 0.3.34](../releases/0.3.34.md) | Reference only; no parity claim |
| 0.3.35 | [Upstream 0.3.35](../releases/0.3.35.md) | Reference only; no parity claim |
| 0.3.36 | [Upstream 0.3.36](../releases/0.3.36.md) | Reference only; no parity claim |
| 0.3.37 | [Upstream 0.3.37](../releases/0.3.37.md) | Reference only; no parity claim |
| 0.3.38 | [Upstream 0.3.38](../releases/0.3.38.md) | Reference only; no parity claim |
| 0.3.39 | [Upstream 0.3.39](../releases/0.3.39.md) | Reference only; no parity claim |
| 0.3.40 | [Upstream 0.3.40](../releases/0.3.40.md) | Reference only; no parity claim |
| 0.3.41 | [Upstream 0.3.41](../releases/0.3.41.md) | Reference only; no parity claim |
| 0.3.42 | [Upstream 0.3.42](../releases/0.3.42.md) | Narrow-thread CSS fix selectively ported; not full parity |
| 0.3.43 | [Upstream 0.3.43](../releases/0.3.43.md) | Devin CLI integration in Extended 0.5.0 |
| 0.3.44 | [Upstream 0.3.44](../releases/0.3.44.md) | Remote routing, progress and image integration in Extended 0.5.0 |
| 0.3.45 | [Upstream 0.3.45](../releases/0.3.45.md) | Sidebar sections, inspector tabs and inline diffs in Extended 0.5.0 |
| 0.3.46 | [Upstream 0.3.46](../releases/0.3.46.md) | Immediate remote prompts and stream continuity in Extended 0.5.0 |
| 0.3.47 | [Upstream 0.3.47](../releases/0.3.47.md) | Richer Connect task/team data, paging and bounded read concurrency in Extended 0.5.0 |
| 0.3.48 | [Upstream 0.3.48](../releases/0.3.48.md) | Centered harness handoff disclosure in Extended 0.5.1 |
| 0.3.49 | [Upstream 0.3.49](../releases/0.3.49.md) | Connect integration candidate in Extended 0.6.0 |
| 0.3.50 | [Upstream 0.3.50](../releases/0.3.50.md) | Agent traces integration candidate in Extended 0.6.0 |

The current Extended source candidate is [`0.6.0+pc.0.3.50`](../releases/extended/0.6.0.md), while original source ancestry remains upstream `0.3.28`. Its [integration record](upstream-integration-0.6.0.md) identifies the reviewed inputs, feature paths and verification requirements. Full upstream parity remains unclaimed. The published `0.4.0` note and legacy Extended notes stay in [`releases/extended/`](../releases/extended/). [`HARDENED-REVISION.json`](../HARDENED-REVISION.json) and the [0.3.42 validation report](../security-review/0.3.42-hardened/REPORT.md) retain their historical bytes and do not attest this candidate's test results.

The [read-only upstream version watch](../.github/workflows/upstream-version-watch.yml) checks GitHub's latest official stable tag daily and fails if a newer version exists. It never updates dependencies, merges code, runs upstream executables or publishes a release automatically. Each new release requires an independently reviewed code integration, tests and explicit publication decision.
