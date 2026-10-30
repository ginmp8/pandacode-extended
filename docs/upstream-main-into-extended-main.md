# Integration: fork main into Extended main (experimental)

**This candidate is incomplete. No feature-parity, security-clearance, or release-readiness claim.**

## Pinned inputs
- Extended main (base + intended PR target): `ginmp8/pandacode-extended@f0105e7db85a0d7bc2e70726716fcfa939fac182` (v0.6.0).
- Fork main (all-feature source, not only release notes): `ginmp8/pandacode@0336a2fb5e3abaea35c095fcc63805652f49ea98`.
- Comparison anchor: upstream 0.3.50 `15541a51f3f5d8ee32d91349f9d5035a53c4cd19`, **not proven common Git ancestry**.
- Extended development and prior PR #77 are excluded from both base and source.

## Inventory
- Extended main 1545 tracked files; fork main 1406.
- 621 identical, 267 fork-only, 406 Extended-only, 518 common differing.
- Compared with upstream 0.3.50, 163 changed upstream only, 186 Extended only, and 169 changed on both sides.
- First import stages all **163 upstream-only updated files**; preserves **all Extended-only files and all concurrent Extended changes**.

## Remaining
1. Import fork-only runtime/features, with packaging and external egress review.
2. Reconcile all 169 common files modified on both sides: retain Extended security and exclusive Board/GitHub/product behavior while incorporating fork main changes.
3. Verify full frontend, Rust, mobile and relay tests/builds, static and runtime security regressions, Windows installer and up-to-date provider behavior.
4. Inspect same-SHA GitHub CI and issue acceptance; no issue closure/renumbering/release/merge to main without review.

## Non-negotiable Extended controls
Updater and automatic agent installation remain disabled; npx installer and publishing workflows cannot download/publish; Windows installs current-user without fetching WebView2; Cargo/npm dependencies are locked. Tauri capabilities remain main-only, preview CSP is constrained, credentials use OS keychain, Connect requires device trust/replay/revocation checks and remote Full remains default-off, bots require approval before outbound content, and cloud/network requests cannot bypass existing controls.
