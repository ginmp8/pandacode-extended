# PandaCode Extended 0.8.0 — documentation index

**Status:** planned, unreleased Extended release candidate. The current documentation baseline is PandaCode source `0.3.90` from [`348add18a63906bd8928a7cef547eeaf91429fa7`](https://github.com/ginmp8/pandacode/commit/348add18a63906bd8928a7cef547eeaf91429fa7), initially integrated in Extended at [`f653c8641e23746a4038cfebf1a8a3df90c478e7`](https://github.com/ginmp8/pandacode-extended/commit/f653c8641e23746a4038cfebf1a8a3df90c478e7). This is not a claim that the Extended product version has been published, or that a binary is available.

**Latest targeted upstream parity:** PandaCode `0.3.91`, immutable fork commit [`a9be2ced7bb594ad28a09fbd204cdad34770c750`](https://github.com/ginmp8/pandacode/commit/a9be2ced7bb594ad28a09fbd204cdad34770c750), succeeding the original `0.3.90` documentation baseline. This remains a planned Extended `0.8.0` candidate, not a published product version. See [original upstream 0.3.91 release notes](../../../releases/0.3.91.md).

- [Extended 0.8.0 release notes and feature scope](../../../releases/extended/0.8.0.md)
- [Release readiness and validation checklist](release-checklist.md)
- [Feature controls and enabled/disabled behavior](feature-controls.md)
- [Dependency and security evidence](dependency-security.md)
- [Carried-forward known issues and verification gaps](known-issues.md)
- [Mandatory upstream parity contract](../upstream/parity-contract.md)
- [Pinned upstream source identity](../upstream/source-baseline.json)
- [Previous 0.7.0 candidate documentation](../0.7.0/README.md)

**Version separation:** The PandaCode upstream application version comes from the source manifests. `0.8.0` is the separate **planned Extended release line**. Keep historical `0.7.0` evidence immutable unless correcting a verified error; do not silently re-label those results as 0.8.0 validation.

## Current synchronization evidence

- Code import: [Extended commit `083e7644`](https://github.com/ginmp8/pandacode-extended/commit/083e764485f08ea1b5cabbf3b3d18ea9b426e89c), mirrored from source `0.3.91` / `a9be2ced7bb594ad28a09fbd204cdad34770c750`.
- [Six-job CI #41498498966](https://github.com/ginmp8/pandacode-extended/actions/runs/41498498966): all `success` on the code SHA; native installers and full Settings On/Off acceptance remain unverified.
- Earlier PR #105 is merged, but covers an older `base/hardened` integration and is not evidence of a `0.3.91` release.
