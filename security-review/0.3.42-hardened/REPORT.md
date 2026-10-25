# PandaCode Hardened 0.3.42 — bounded security/change review

## Source identities

- Supplied Hardened 0.3.41 SHA-256: `e527daa7b05ee58ad70031d2308002070b88756631daf1c873269b84993cc85a`
- Supplied upstream 0.3.42 SHA-256: `164737d7da2a233362668ad28126a707c50020d69114723c697d28f22ede79c5`
- Candidate ancestry: upstream 0.3.28 plus selective hardening and ports. Not a full 0.3.42 rebase.

## Scope and evidence

Static ZIP comparison identified 211 upstream-only files, 335 Hardened-only files, and 242 shared-path files with differing contents. The upstream 0.3.41→0.3.42 functional delta is limited to `src/styles.css`, `src/team-progress.css`, and the Playwright case in `tests/architect.spec.ts`; the remaining official changes are release metadata and version files. The candidate changes these same styles, carries that regression case, adds release-note provenance and a read-only GitHub Releases version check. No native agent, credential, remote IPC, installer, relay, updater or package dependency runtime files were imported from upstream.

## Risk surface coverage

| Surface | Status |
|---|---|
| S01 Source identity/release | Partial: ZIP hashes, source comparisons; no independent binary provenance |
| S02 Build/supply chain | Partial: no dependencies changed; native build unverified |
| S03 Install/update | Partial: retained previous disabled updater/installers; OS install unverified |
| S04 Agent authority | Partial: no agent execution source change; no runtime checks |
| S05 Egress/privacy | Partial: new CI reads public GitHub release metadata only; runtime unchanged |
| S06 Credentials/identity | Partial: credentials code unchanged; native storage unverified |
| S07 Relay, bots, webhooks | Partial: backend source unchanged; runtime unverified |
| S08 Webview/IPC | Partial: CSS-only change; WebKit/browser regression not executed |
| S09 Local MCP/API | Partial: no IPC/MCP source changes; no runtime check |
| S10 Jobs/state/revocation | Partial: no additional jobs in runtime; lifecycle checks unrun |
| S11 Host/network isolation | Partial: no execution-path source change; OS unverified |
| S12 AV/OS | Not run: no Windows/macOS install or AV provenance |
| S13 Release QA/observability | Partial: targeted local Node checks passed; complete CI suite unrun |

`actions/checkout` is SHA-pinned in the version watch; the CI runner image remains a moving platform dependency. A failing version watch signals a newer tag, **not** an automatic upgrade or security approval.

## Local checks and release decision

Targeted Node.js static and mocked version-check tests: **4 passed, 0 failed**. The upstream Playwright UI test was incorporated but **not executed**. Cargo/Rust toolchain and network dependencies unavailable; native, TypeScript full build, installed-app tests, Windows/macOS installers, integration/real-device checks were **not run**. Prior reports cannot attest this candidate.

**Install verdict: inconclusive.** Candidate is reviewable source only. Before publish as a production-ready release, run full CI, WebKit/UI viewport check, native build, security regression suite and OS-specific installer tests. Do not represent the version label as full upstream feature parity.
