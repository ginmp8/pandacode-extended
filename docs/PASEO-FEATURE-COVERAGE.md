# Paseo 0.12.0 coverage and selection - PandaCode 0.3.35-hardened.3

Scope: the supplied versioned ZIP, not a moving remote main. This is a feature-family selection map from public documentation, selected call paths and manifests; it is not a semantic audit of all 5,726 source members or a promise of all-feature parity. Seven archive symlinks were skipped without following their targets.

**Decision:** implement the missing bounded browser-QA and workflow features through existing Panda mechanisms; preserve equivalent existing mechanisms; exclude additions whose authority, dependencies or maintenance cost conflicts with the requested lightweight fork. Existing does not mean behaviorally equivalent to Paseo.

| Feature family | Decision | Paseo source | Panda implementation/reference | Scope and reason |
|---|---|---|---|---|
| Browser snapshot and semantic element references | Adapted | `public-docs/browser-tools.md` | `src-tauri/src/browser/review.js` | Visible light DOM, bounded names/text, stale/one-use references; not platform AX or all frames. |
| Browser layout and accessibility review | Adapted | `public-docs/browser-when.md` | `src-tauri/src/browser/review.js` | Heuristics plus workflow/manual screenshot review; not an automatic full visual/WCAG auditor. |
| Responsive browser inspection | Adapted | `public-docs/browser-tools.md` | `src/browser.ts` | Existing guest bounds and actual dimensions; not device/OS emulation. |
| Browser interaction and explicit expectations | Adapted | `public-docs/browser-tools.md` | `src/hardened/browser-flow.ts` | Finite scenarios and DOM actions; no generic evaluate, uploads or trusted OS input. |
| Screenshots and before/current comparison | Extended existing capture | `public-docs/browser-tools.md` | `src/hardened/browser-qa.ts` | Two local viewport images, manually reviewed; no automatic score or agent upload. |
| Console and network diagnostics | Partial safe adaptation | `public-docs/browser-tools.md` | `src-tauri/src/browser/review.js` | Opt-in ErrorEvent/unhandled rejection, not all console messages; Resource Timing, not full HTTP bodies/headers. |
| Browser tools callable by agents | Adapted through native path | `public-docs/browser.md` | `src-tauri/src/browser/review_tools.rs` | Two additive catalog entries under existing approval, task ownership and cancellation; no new IPC command. |
| Visual and frontend/backend testing workflow | Adapted | `public-docs/orchestration-workflows.md` | `src/hardened/workflow-library.ts` | Templates plus executable finite browser checks; static contract review is not a generated dedicated API validator. |
| Implementation/review and bug reproduction | Adapted as bounded templates | `public-docs/orchestration-workflows.md` | `src/hardened/workflow-library.ts` | Original executor/tests; draft or project recipe only. No enforced autonomous consensus loop. |
| Parallel research | Adapted as bounded template | `public-docs/parallel-development.md` | `src/hardened/workflow-library.ts` | Reuses already configured team limits; no new scheduler or authority. |
| Advisor, committee and handoff | Already in hardened.2 | `skills/paseo-advisor/SKILL.md` | `src/hardened/review-model.ts` | Retained previous isolated profiles, preflight and explicit handoff; no new claim of provider isolation. |
| Attention and dependency inspection | Already in hardened.2 | `public-docs/orchestration.md` | `src/hardened/review-panel.ts` | Existing loaded-snapshot attention and dependency view; native scheduler remains authoritative. |
| Agent profiles and orchestration | Existing Panda mechanisms | `public-docs/agent-profiles.md` | `src-tauri/src/architect.rs` | Original profiles, workers, dependencies and budgets; analogous purpose, not Paseo API parity. |
| Reusable recipes | Existing store, new content | `public-docs/sdk/recipes.md` | `src-tauri/src/recipes.rs` | Six new prompt recipes saved explicitly into original project store. |
| Workspace and worktree operations | Existing Panda mechanisms | `public-docs/worktrees.md` | `docs/worktrees.md` | No import of a second worktree/automatic merge manager; preserves the existing model. |
| Tasks, files and terminal | Existing Panda mechanisms | `docs/explorer-sidebar.md` | `docs/task-board.md` | Original task board/inspector/editor/terminal retained; no duplicate UI framework. |
| MCP and skills | Existing Panda integration; no transplant | `public-docs/mcp.md` | `docs/agent-tools.md` | Existing catalog/skills paths retained. No SDK/protocol compatibility claim. |
| Provider adapters and configuration | Existing Panda integration; no transplant | `public-docs/providers.md` | `docs/providers.md` | No provider protocol/config/secret-store rewrite or automatic provider installation. |
| Independent browser contexts, tabs and persistent sessions | Excluded from this low-impact increment | `public-docs/browser.md` | `docs/browser.md` | Would require a new native guest lifecycle/permission/ownership contract beyond the selected seams. |
| Generic browser evaluate, uploads and trusted keyboard/drag | Excluded | `public-docs/browser-tools.md` | `src-tauri/src/browser/review.rs` | Not approximated by a misleading API. Arbitrary execution and sensitive uploads need a separate reviewed authority boundary. |
| Remote service proxy / cross-device preview | Excluded | `docs/service-proxy.md` | `docs/remote-access.md` | Would introduce network routing/authentication and remote exposure; local existing browser is sufficient for this increment. |
| Hub, remote daemon, pairing, mobile and push | Excluded | `public-docs/hub/index.md` | `docs/remote-access.md` | Not included merely to copy the product; new services and authentication are outside the lightweight goal. |
| Schedules/daemon lifecycle replacement | Excluded | `public-docs/schedules.md` | `docs/goals-and-command-workflows.md` | Do not substitute recurring Paseo daemon scheduling for existing Panda task/goal semantics. |
| Plugin runtime and arbitrary lifecycle hooks | Excluded | `public-docs/plugins/index.md` | `docs/skills-library.md` | New executable extension/supply-chain surface is unnecessary for static templates. |
| Voice, audio models and remote speech | Excluded | `public-docs/voice.md` | `README.md` | Dependencies and external data handling do not solve the scoped review/integration gap. |
| Resident agents and streaming coalescing | Deferred/excluded | `docs/agent-stream-performance.md` | `src/streaming-markdown.ts` | No measured gain in this run; preserve existing lifecycle/streaming instead of speculative performance rewrites. |
| App/framework/design-system replacement and localization | Excluded | `docs/architecture.md` | `README.md` | No Electron/React Native/SDK migration, broad UI restyle or duplicated persistence. |

## Evidence and limits

Every named source above exists in the supplied archives; `coverage-sources.json` fixes its bytes. The original earlier review of orchestration missed the browser tool surface; this revision explicitly covers it. No Paseo application/runtime is shipped, and no foreign SDK tool name is presented as a working PandaCode command.

The six workflow templates are usable UI flows, not merely unused helper code: Review & coordination > Review presets > Workflow library. The Browser QA toolbar opens the real feature modules, and agent catalog entries route to that same task-owned browser. Native/provider/live-HTTP execution has not been demonstrated in this environment. See the release report rather than inferring end-to-end success from source presence.

Browser review intentionally provides a smaller contract than CDP: visible light DOM, current viewport and bounded metadata. An unsupported check is not a pass. All incomplete, excluded and existing categories are explicit; no absence result is inferred from filename-only searches.
