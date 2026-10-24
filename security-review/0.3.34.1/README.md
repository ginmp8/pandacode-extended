# PandaCode 0.3.34.1 hardened — Focused Audit and Mitigations

**Review:** 2026-10-23 · **base:** PandaCode 0.3.34 hardened ZIP (SHA-256 `7cf9a99bbfd3e3753d542753c954551007290e42e14464c43163b7d066f1a412`) · **target:** 0.3.34.1 hardened (SemVer `0.3.34-hardened.1`).

**Overall status: `blocked-critical-evidence` — NOT approved for production.** Patches and code inspection do not replace `cargo test --locked`, Tauri builds, integration tests, dependency audits using current data, real execution isolation, Windows/Keychain checks, Cloudflare deployment, or independent verification. Source-level regression tests were added but not run. This analysis is limited to the eight findings and nearby affected surfaces.

## Finding Summary

| ID | Original report assessment | Change in 0.3.34.1 | Assurance / gap |
|---|---|---|---|
| F01 | **Supported (high, cross-approval)** | Bind confirmation to an approval UUID; one-time use, 10-minute TTL, queue limit. `yes` without an ID does not approve. | Code inspected; test for A resolved by desktop plus stale response versus newly created B was not run. User must review the action on desktop. |
| F02 | **Supported (high, governance risk)** | New tasks in storage/UI/GitHub/overlay default to `ask`; `full` requires explicit selection. | Persisted tasks already set to `full` are preserved, without silent migration. Verify launchers/CLI at runtime. |
| F03 | **Supported (medium, cancellation)** | Supervised external deploy process receives an atomic stop flag and terminates the process/group on cancellation. Deploy-tool deadline set to 1,860 s versus the 1,800 s internal limit. Uncertain outcomes are explicit. | No Windows/Job Object proof; effects already sent to external APIs cannot be reversed; external MCP cancellation needs additional validation. |
| F04 | **Supported (medium, timeout)** | Worker applies `timeout --kill-after` to the command, serializes executions, and destroys the sandbox after a confirmed timeout or fallback. | Destroying the container removes state not persisted in the workspace; real execution and Cloudflare semantics unverified. |
| F05 | **Supported (medium, supply chain)** | Removed desktop-triggered `npm install`; requires Wrangler to be preinstalled/reviewed and verifies version 4.151.0. | **Intentional functional restriction:** deploy is not guaranteed without prepared Worker dependencies. No audited Worker lockfile; do not claim risk eliminated. |
| F06 | **Supported (medium, supply chain)** | Replaced `npx wrangler` with a local invoker, no fallback/download; validates version 4.151.0 before allowed commands; npm ignores hooks. | Invoker requires pre-provisioned Wrangler. Dependency was not added to relay lockfile; feature fails closed if absent. |
| F07 | **Supported (medium, HTTP limits)** | Relay streams request bodies with a real 256 KiB limit before JSON/GitHub; returns 413 when exceeded without `Content-Length`. Worker `/exec` also limits bytes. | Three regression tests defined but not run in a sandbox. Platform infrastructure was not audited. |
| F08 | **Supported (low, governance)** | SECURITY.md distinguishes hardened distribution from upstream, prohibits automatic updates, and documents a manual process. | A fork-specific security contact was not supplied/verified; no fictional address was created. |

F02, F05, F06, and F08 describe **policies or governance gaps**, not backdoors, observed exploits, or demonstrated CVEs. They were not labeled false positives merely because no dynamic reproduction exists. F01, F03, F04, and F07 have identified code paths and patches in the target; behavioral confirmation is pending.

## Input/Output Provenance (Summary)

- **F01:** `src-tauri/src/bots/gateway.rs` → `parse_command`, `approval_requested`, `answer_approval` → `respond_approval` in `src-tauri/src/lib.rs`. Actor: previously paired sender, external channel. Invariant: specific, non-reusable authorization.
- **F02:** `src-tauri/src/storage.rs` plus `src/main.ts`, `src/github.ts`, `src/overlay-client.ts`, `src/preview.ts` → `providers.rs`. Actor: agent content; invariant: least privilege for new tasks.
- **F03:** `src-tauri/src/providers/host_tools.rs`, `src-tauri/src/providers/mcp_host.rs` → `src-tauri/src/cloud/deploy.rs` → `src-tauri/src/workspace.rs`. Actor: permitted agent tool; invariant: no local effect continues indefinitely after cancellation. **Limit:** remote outcome is uncertain.
- **F04:** `src-tauri/src/cloud/cloudflare_worker/src/index.ts` → `container.exec` → `timeout` / `container.destroy`. Actor: remote token holder; resource: sandbox container.
- **F05:** `src-tauri/src/cloud/cloudflare.rs` → host-installed Wrangler, no automatic npm; missing prepared dependency tree prevents deploy.
- **F06:** `relay/package.json` → `relay/scripts/wrangler.mjs`, no `npx` invocation or implicit download.
- **F07:** `relay/src/index.ts` → `relay/src/limits.ts` → `githubUser`; oversized body rejected with 413 before GitHub request.
- **F08:** `SECURITY.md` update policy and fork reporting.

## Regression Tests Added (NOT RUN)

- `src-tauri/src/bots/gateway.rs`: delayed response for A never approves B; `yes` without UUID is blocked.
- `src-tauri/src/workspace.rs`: controlled process exceeds deadline and is cancelled without creating a marker file (Linux/Unix).
- `relay/test/request-body-limit.test.ts`: two-part body over 256 KiB without `Content-Length`, normal body, and invalid JSON.
- Existing tests updated to expect `ask` for new tasks.

## Validation, Assurance, and Pending Evidence

- Structural change checks, blocking `npm install` in `cloudflare.rs`, blocking `npx wrangler` in relay, version and lockfiles: **run by an external static script**, see `static-verification.json`.
- TypeScript syntax parsing for changed files (host TypeScript compiler, parse only; code not executed): **run**, see `static-verification.json`.
- `node --check` for JS invoker: **run**.
- Unit/integration tests, `cargo check --locked`, `cargo test --locked`, `npm ci`, `tsc --noEmit`, Playwright, real Cloudflare, Windows, macOS, SCA, and full secret scan: **not run** due to lack of verified isolation/toolchain, credentials, or suitable environment; they do not count as passes.
- An aborted deploy/tool result may remain **uncertain**; do not retry automatically without external reconciliation.
- F04 destroys the container after timeout; unpersisted data may be lost.
- F05/F06 fail closed when Wrangler/dependencies are not already available; manual installation and independent validation are operational prerequisites.
- No certificate, approval, or absence of CVEs/secrets is claimed.

## Criteria to Unblock a Release

1. Run `cargo check --locked`, `cargo test --locked`, and integration tests on Linux/Windows, including delayed-approval scenarios.
2. Prove cancellation propagation for deploys and MCP, including children and Windows Job Objects; define handling for uncertain remote outcomes.
3. Rebuild the Cloudflare Worker with pinned/audited dependencies and run a timeout test observing the process tree and later effects.
4. Resolve Wrangler/Worker supply chain with a lockfile and reproducible distribution, keeping scripts disabled by default.
5. Run the real deployment HTTP 413 test without `Content-Length` and verify that GitHub is not called.
6. Run contemporary SCA and secret scanning; review Windows/macOS/installer/antivirus support.
7. Independently bind the final ZIP, tree, toolchain, and execution records.

**Result:** candidate static corrections; release remains blocked by missing critical evidence, including functionality and potential regressions.
