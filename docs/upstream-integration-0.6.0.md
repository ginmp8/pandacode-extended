# Upstream integration assessment: PandaCode 0.3.49–0.3.50

Status: **incomplete; implementation and independent verification pending**. This document is not a feature-parity claim or a security certification.

## Immutable scope
- Extended `main` baseline: `479f2e6ac700d216d62ae08681e36aaaa20fa074` (technical version `0.5.2`, display `0.5.2+pc.0.3.48`).
- Upstream 0.3.48: `9cc1a9a58d259f7316e4f3ad621e8ca120358d89`.
- Upstream 0.3.49: `e71acdda9eb5d488122081c4dbc49ee1a5b4f9e2`.
- Upstream 0.3.50: `15541a51f3f5d8ee32d91349f9d5035a53c4cd19`.
- Required next **technical** version: `0.6.0`; display identifier after validated port: `0.6.0+pc.0.3.50`.
- Original upstream release notes retained at `releases/0.3.49.md` and `releases/0.3.50.md`. These do not constitute implementation.

## 0.3.49 — remote Connect
See [issue #38](https://github.com/ginmp8/pandacode-extended/issues/38) for paths, mapping, sample implementations, exclusions and test matrix.

| Upstream behavior | Intended integration | Current verification |
| --- | --- | --- |
| Encrypted direct LAN/Tailscale route plus relay fallback | Adapt with verified device trust before private-address probing, preserve relay | Not implemented or verified |
| Host approval, View/Control/Full grants and single-use 10-minute code | Preserve Extended default-off Full access, revocation, per-key identity, retry limits | Not implemented or verified |
| Diagnostic route/stage/latency and redacted report | Preserve local privacy and no telemetry | Not implemented or verified |
| Reconnection replay and resynchronization | Reconcile desktop and mobile event paging, dedup and stop semantics | Not implemented or verified |

The upstream's **known limitation** explicitly allows an unapproved non-blocked device to cause direct-route probing of private addresses it announces. **Do not import that behavior unchanged.** Security review is a hard gate for this upstream release. The mobile client still uses relay transport.

Native files requiring conflict-aware review include `src-tauri/src/remote/{host,socket,mirror,start,config,crypto,client}.rs` plus new `routes.rs`, `grants.rs`, `diagnostics.rs`, and mobile replay logic. Extended has separate device-key, replay, permissions and execution-admission hardening. Whole-file replacements are not safe.

## 0.3.50 — traces and streaming
See [issue #39](https://github.com/ginmp8/pandacode-extended/issues/39) for paths, examples, exclusions and checks.

| Upstream behavior | Intended integration | Current verification |
| --- | --- | --- |
| Thin live trace rail, active step, finished summary | Integrate with existing Extended conversation grouping | Not implemented or verified |
| Two recent cards expanded, earlier collapsed | Respect manual disclosure and accessibility | Not implemented or verified |
| Rich escaped cards, shell tail/exit, diffs, search/web/MCP/team tools | Bound untrusted output, preserve escapes and privacy | Not implemented or verified |
| Inline local brands, cited source chips | Validate URLs; no remote tracking pixels | Not implemented or verified |
| Live incomplete links and code spans | Keep partial streaming text visible, no unsafe DOM | Not implemented or verified |
| Trace-only animation and reduced motion | Keep existing no-motion behavior elsewhere | Not implemented or verified |

New upstream components: `src/{tool-cards.ts,tool-cards.css,trace-brands.ts,trace-motion.ts,trace-motion.css,preview-traces.ts}`; tests: `tests/{agent-traces.spec.ts,tool-cards.test.mjs,trace-brands.test.mjs}`. Other changed files include `src/main.ts`, `src/preview.ts`, `src/tool-trace.ts`, `src/enriched-markdown.ts` and `src/streaming-markdown.ts`.

## Explicit non-goals
No auto-updater or automatic npx download; no new telemetry, broadened filesystem/network permissions, unsafe preview, remote Full-by-default, unchecked credential access, removal of Extended-only Board/bot/GitHub features, rewriting released tags or historical review evidence, merge or distribution publication.

## Verification gates
1. Audit every relevant upstream delta and the corresponding Extended owners; record each feature's disposition (included, equivalent, intentionally excluded, deferred) and evidence.
2. Preserve all Extended security and identity contracts. Confirm call-path trust checks **after** awaits and before dispatch/effects, not only at connection start.
3. Run `npm run build`, `npm test`, `npm run test:release`, `npm run site:build`, targeted Playwright and relevant native/mobile suites; show exact SHA and CI jobs. Never weaken tests to turn green.
4. Review isolated two-device behavior for direct transport, key changes, revocation, replay, pairing-code races and report redaction. Mark cross-platform/native validation unknown unless executed.
5. Use one integration branch, one PR targeting `main`, **linear Conventional Commits**. CI queries at least 60 seconds apart. No merge, tag or release without approval.

## Acquisition / evidence limitation
The full GitHub tree inventory was available as metadata (`truncated=false` for both source refs), and canonical release notes plus changed-file metadata were inspected. The execution environment could not clone/download GitHub repository snapshots via Git CLI/HTTP (DNS resolution failed), so no independent complete local source tree, Rust build or test execution exists. API reads and API writes alone do not justify full parity or passing-test claims.
