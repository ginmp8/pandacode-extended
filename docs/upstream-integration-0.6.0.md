# Upstream integration assessment: PandaCode 0.3.49–0.3.50

Status: **integration candidate; full native/behavioral verification pending**. This document is not a feature-parity claim or a security certification.

## Immutable scope
- Extended `main` baseline: `479f2e6ac700d216d62ae08681e36aaaa20fa074` (technical version `0.5.2`, display `0.5.2+pc.0.3.48`).
- Upstream 0.3.48: `9cc1a9a58d259f7316e4f3ad621e8ca120358d89`.
- Upstream 0.3.49: `e71acdda9eb5d488122081c4dbc49ee1a5b4f9e2`.
- Upstream 0.3.50: `15541a51f3f5d8ee32d91349f9d5035a53c4cd19`.
- Current **technical** candidate: `0.6.0`; tracking label: `0.6.0+pc.0.3.50`; neither establishes full feature parity.
- Original upstream release notes retained at `releases/0.3.49.md` and `releases/0.3.50.md`. These do not constitute implementation.

## 0.3.49 — remote Connect
See [issue #38](https://github.com/ginmp8/pandacode-extended/issues/38) for paths, mapping, sample implementations, exclusions and test matrix.

| Upstream behavior | Intended integration | Current verification |
| --- | --- | --- |
| Encrypted direct LAN/Tailscale route plus relay fallback | Adapt with verified device trust before private-address probing, preserve relay | Source port staged; not independently verified |
| Host approval, View/Control/Full grants and single-use 10-minute code | Preserve Extended default-off Full access, revocation, per-key identity, retry limits | Source port staged; not independently verified |
| Diagnostic route/stage/latency and redacted report | Preserve local privacy and no telemetry | Source port staged; not independently verified |
| Reconnection replay and resynchronization | Reconcile desktop and mobile event paging, dedup and stop semantics | Source port staged; not independently verified |

The upstream's **known limitation** explicitly allows an unapproved non-blocked device to cause direct-route probing of private addresses it announces. **Do not import that behavior unchanged.** Security review is a hard gate for this upstream release. The mobile client still uses relay transport.

Native files requiring conflict-aware review include `src-tauri/src/remote/{host,socket,mirror,start,config,crypto,client}.rs` plus new `routes.rs`, `grants.rs`, `diagnostics.rs`, and mobile replay logic. Extended has separate device-key, replay, permissions and execution-admission hardening. Whole-file replacements are not safe.

## 0.3.50 — traces and streaming
See [issue #39](https://github.com/ginmp8/pandacode-extended/issues/39) for paths, examples, exclusions and checks.

| Upstream behavior | Intended integration | Current verification |
| --- | --- | --- |
| Thin live trace rail, active step, finished summary | Integrate with existing Extended conversation grouping | Source port staged; not independently verified |
| Two recent cards expanded, earlier collapsed | Respect manual disclosure and accessibility | Source port staged; not independently verified |
| Rich escaped cards, shell tail/exit, diffs, search/web/MCP/team tools | Bound untrusted output, preserve escapes and privacy | Source port staged; not independently verified |
| Inline local brands, cited source chips | Validate URLs; no remote tracking pixels | Source port staged; not independently verified |
| Live incomplete links and code spans | Keep partial streaming text visible, no unsafe DOM | Source port staged; not independently verified |
| Trace-only animation and reduced motion | Keep existing no-motion behavior elsewhere | Source port staged; not independently verified |

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

## Current candidate source and archive verification

- Extended baseline: `479f2e6ac700d216d62ae08681e36aaaa20fa074`.
- Latest reference source commit: `15541a51f3f5d8ee32d91349f9d5035a53c4cd19`.
- Canonical pinned archive SHA-256: `db23e5aaa2b11c940874606156920be0ee3118046b2b49d1e17fbf8174932b8c` (local ZIP verified from the GitHub Actions source-archive artifact; not an installer signature).
- Reference snapshot contained **1,232 files**, excluding untracked dependencies, submodules and generated runtime artifacts. The upstream Git tree listing at this commit was not truncated.
- Extended 0.3.48 source-note blob: `639bd02d0f5513aad54bd253c6f496676b824962`. New 0.3.49 note blob: `abe3a8415558e2f176c0213b84d0040b23983f6c`; 0.3.50: `74919eec0d2bd79d21f81a9cc89563fa64d58e8c`.
- Native 0.3.49 conflicts were mechanically reconciled against source 0.3.48 while retaining Extended modifications where they overlapped, except for the upstream connection state-machine components explicitly selected for integration. The resulting native implementation requires **independent review**; a mechanical merge is not security clearance.
- 0.3.50 live tool cards, trace rendering, markdown and preview/test sources were integrated as source code and test cases, not merely copied release notes. Targeted browser and regression checks determine whether the feature works.

### Security and compatibility gates still required

The Extended anti-replay SQLite database must remain durable and fail-closed; host authorization and active grants must be rechecked after asynchronous work; local Full access must remain disabled by default; previously hardened preview/network/update paths and bot/task behavior must remain unchanged. Direct route announcements must not cause any unapproved device to initiate a private-address probe. The new grant and old trusted-key state must have an explicitly verified migration and reapproval path. Missing real two-device tests, unsupported native targets or red CI checks block a parity claim and release publication.
