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
