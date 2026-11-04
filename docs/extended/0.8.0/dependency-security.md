# PandaCode Extended 0.8.0 — dependency and security tracking

**Status:** initial security baseline inherited from the Extended 0.7.0 source candidate. No independent 0.8.0 audit or new dependency remediation is claimed by creation of this file.

The [0.7.0 dependency-security record](../0.7.0/dependency-security.md) contains the original npm/Rust advisory investigation, resolved dependency versions, local `deepmerge` security patch, regression evidence, CI records and source-compatibility boundaries. Keep those original facts intact rather than copying their historical audit dates or severities into a fictional new result.

## Security baseline to preserve

- Desktop, website and mobile npm dependency fixes already integrated in the inherited source tree.
- The Extended-local, MIT-licensed `mobile/vendor/extended/deepmerge` fix for prototype-pollution behavior, pending separately verified upstream replacement.
- Rust TLS/certificate/Tokio/ZIP remediation and RustSec checks already documented for the earlier candidate.
- Existing audit and compatibility tests without deleting upstream features or bypassing authentication, permission, or approval controls.

## Imported upstream 0.3.91 dependency requirements

Linux Wayland clipboard-image support requires upstream's `arboard` crate with `wayland-data-control` and its transitives. The Extended target retains its existing remediated TLS, archive and runtime dependency versions; the Cargo lockfile therefore must be reconciled rather than replaced with the upstream lockfile. A successful dependency-resolution or RustSec audit for the new candidate is **not yet asserted** here.

## Checks required for the 0.8.0 candidate

- [ ] Recompute resolved dependency versions from the **current** root, website, mobile and other workspace lockfiles.
- [ ] Run npm audits and RustSec on the **exact final `base/development` SHA**, verifying the audit scope and severity threshold.
- [ ] Re-run relevant mobile/Expo, TypeScript, Rust and local vendor-patch regressions.
- [ ] Review any newly imported upstream dependencies for compatibility with Extended fixes before adjusting versions.
- [ ] Record any advisory IDs, severity, exploitability assumptions, updates, test commands and outcome in this document.

**No new vulnerability, new remediation, clean-audit result or release approval is asserted until fresh evidence is recorded.**

## Executed audit evidence for upstream 0.3.91 integration

On [Extended code commit `083e764485f08ea1b5cabbf3b3d18ea9b426e89c`](https://github.com/ginmp8/pandacode-extended/commit/083e764485f08ea1b5cabbf3b3d18ea9b426e89c), [CI #41498498966](https://github.com/ginmp8/pandacode-extended/actions/runs/41498498966) completed its npm dependency audit and RustSec audit jobs with `success`. The same run also passed Linux Rust test/Clippy and the other four jobs. The lockfile retained Extended's remediated `zip` and `rustls` versions while adding the upstream `arboard` Wayland clipboard dependencies. This evidence establishes the configured CI result for that SHA, not absence of every vulnerability or native platform compatibility.

## Source 0.3.92 dependency reconciliation

The upstream change from 0.3.91 to 0.3.92 updates application-version metadata in the root npm and Rust manifests/locks. The Extended-resolved dependency remediations and locally vendored mobile patch are retained. Fresh npm, RustSec, Rust build and mobile results for the 0.3.92-integrated SHA remain required; historical 0.3.91 CI is not proof for this candidate.

## Verified 0.3.92 integration CI checkpoint — 2026-11-03

For exact Extended code SHA [`80c438b47e06d1df330564dc8a4cb17318230310`](https://github.com/ginmp8/pandacode-extended/commit/80c438b47e06d1df330564dc8a4cb17318230310), both [push CI #41692930559](https://github.com/ginmp8/pandacode-extended/actions/runs/41692930559) and [PR CI #41692943491](https://github.com/ginmp8/pandacode-extended/actions/runs/41692943491) completed with successful npm dependency audit and RustSec audit jobs. Both also passed Linux Rust checks/tests and mobile/Expo compatibility. These results supersede the earlier *pending CI* observation for the source-0.3.92 integrated **code SHA**, without asserting that all possible advisories, native integrations or runtime policy behaviors were independently verified. No new dependency remediation is claimed by this evidence-only documentation update.

## Source 0.3.93 reconciliation

Upstream 0.3.93 changes the application's declared version and related npm/Cargo lock metadata without requiring a rollback of Extended's resolved dependency remediations. The staged source update retains the Extended dependency overrides and vendored mobile fix. No fresh RustSec or npm audit for this new SHA has yet been observed, and this paragraph does not claim a new security remediation.

## Source 0.3.94 dependencies

The source release updates application version metadata; Extended's dependency overrides, lockfile remediations and vendored mobile patch were retained. No new advisory fix is claimed. Current-SHA npm audit, RustSec and build evidence is tracked in the release checklist.
