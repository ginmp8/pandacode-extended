# PandaCode Extended — original upstream release history

**Important:** An upstream PandaCode release note is a reference, not a release or implemented feature of PandaCode Extended. Versions 0.3.29–0.3.42 are tracked as historical upstream references. See [Extended source releases](https://github.com/ginmp8/pandacode-hardened/releases) for actual published builds/tags and [upstream releases](https://github.com/aicodeking/pandacode/releases) for original release notes.

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

The current Extended source candidate is [`0.4.0+pc.0.3.42`](../releases/extended/0.4.0.md), while original source ancestry remains upstream `0.3.28`, with selective feature ports. Legacy Extended notes are in [`releases/extended/`](../releases/extended/). A complete upstream `0.3.42` source rebase **has not occurred**. See [`HARDENED-REVISION.json`](../HARDENED-REVISION.json) and [0.3.42 validation](../security-review/0.3.42-hardened/REPORT.md).

The [read-only upstream version watch](../.github/workflows/upstream-version-watch.yml) checks GitHub's latest official stable tag daily and fails if a newer version exists. It never updates dependencies, merges code, runs upstream executables or publishes a release automatically. Each new release requires an independently reviewed code integration, tests and explicit publication decision.
