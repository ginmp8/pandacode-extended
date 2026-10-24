# PandaCode 0.3.35-hardened.3 - implementation and evidence

Date: 2026-10-24. Deliverable: complete source ZIP, not a signed native installer.

## Outcome and release status

Implemented bounded browser-QA and reusable workflow features selected from the supplied Paseo 0.12.0 reference. They are connected to the existing Panda UI and agent-tool routing, not unused helpers. No Paseo runtime is embedded. The previous hardened.2 advisor/committee, attention, dependencies and handoff features remain.

**Source implementation and scoped checks are delivered. Native operational approval remains blocked by missing build/runtime evidence.** No full Paseo parity, upstream parity, whole-repository security clearance, automatic visual auditor, complete HTTP-contract validator or guaranteed future merge is claimed.

The version is **0.3.35-hardened.3**, following the user's requested fixed 0.3.35 reference and incrementing the fork revision. The historical source base is still 0.3.28 plus selective backports; the metadata does not falsely relabel every source file as upstream 0.3.35.

## Features and actual entrypoints

| Feature | Entry and implementation | Boundary |
|---|---|---|
| Visible-page snapshot and semantic refs | Browser inspector > Browser QA > Inspect | Bounded visible light DOM; not the platform accessibility tree, frames or shadow roots |
| Layout/accessibility heuristics | Inspect > Audit | Names, labels, language, title, alt, duplicate IDs and overflow; not complete WCAG certification |
| Responsive inspection | Viewport target / Apply viewport | Uses existing guest bounds; actual dimensions reported, no touch/OS emulation |
| Browser interactions | Interactions; explicit confirmation | Non-secret text, select, checkbox, click, focus, scroll; DOM events, no arbitrary JS/upload/trusted OS-input claim |
| Finite browser checks | Scenarios > Validate / Run / Stop | Closed JSON vocabulary, 32 steps, 45 seconds, explicit expectations, cancellation and first-failure stop |
| Frontend/backend observations | Resources / expect_resource, plus expected UI state | Last 100 Resource Timing entries; missing/zero status is unknown and cannot pass; no headers or bodies |
| Runtime error collection | Explicit Observe errors / Stop observing | New page errors/unhandled rejections only; 80 records, 10 minutes; no interception of all console methods |
| Local screenshot comparison | Screenshots > Capture reference / Capture current | Two viewport captures for manual comparison; not automatically sent to agents or text report |
| Evidence preview and copy | Collected evidence > confirmation > Copy evidence | Bounded, explicit sharing; masking is best effort, clipboard history is external |
| Agent access | pandacode_browser_review / pandacode_browser_viewport | Existing native catalog approval, same-task foreground route and cancellation; two new tools, zero new IPC command names |
| Six workflow templates | Review & coordination > Review presets > Workflow library | Visual/responsive, integration smoke, bug reproduction, contract review, implementation/review, two-perspective research |
| Workflow preparation/persistence | Prepare in composer / Save project recipe | Empty current idle local composer or explicit original recipe store; no auto-send/queue/agent launch or permission change |

The workflows are usable prompts/recipes that employ the existing executor; they are not an independent engine enforcing every natural-language instruction. Actual provider tool availability must be checked in the user's runtime. The UI opens the original browser/Recipes controls rather than creating parallel stores.

## Change footprint and compatibility

Four upstream runtime files have small integration changes: `src/browser.ts`, `src/main.ts`, `src-tauri/src/browser.rs`, `src-tauri/src/agent_tools.rs`. One previously added hardened module, `src/hardened/review-panel.ts`, mounts the workflow library without changing its four-tab contract. Eight new runtime files contain 66,822 source bytes / 646 lines. These are source measurements, not CPU, RAM, startup or binary-size measurements.

Only 12 pre-existing files differ: those five runtime files; five version metadata files (`package.json`, `package-lock.json`, `Cargo.toml`, the own-package Cargo.lock version, `tauri.conf.json`); README; HARDENED-REVISION.json. No original file was removed. All 103 pre-existing test files are byte-identical.

No new runtime package, resident service, port, scheduler, database migration, provider protocol, credential store, Tauri IPC command name, capability or CSP permission was introduced. The existing browser_inspect command now accepts a closed `review` action validated in a separate Rust module. Existing approval policy is reused, not replaced or declared unnecessary. Full access retains its original semantics.

The incremental functional patch applies to exact hardened.2. Applying that incremental patch directly to raw upstream fails because the earlier hardened review modules are absent; that is an expected dependency, not hidden compatibility evidence. A separately supplied **cumulative feature-only patch** (hardened.2 features plus this increment) passed `git apply --check` on the supplied original 0.3.35. It deliberately does not contain all historical security patches. Textual application does not establish compilation, behavior, security parity or compatibility with unknown future versions.

## Executed checks

| Check | Baseline | Candidate | Evidence and interpretation |
|---|---:|---:|---|
| Complete Node test command | 162 passed / 4 failed | 185 passed / 4 failed | 23 added passing cases; exact same four failures; no skips or weakened assertions |
| Real new browser script in reviewed Chromium DOM fixture | Not present | 22 checks passed | Actual DOM actions, refs, masking, audit; timing entries synthetic; not live HTTP |
| New UI using real modules with native boundary doubles | Not present | 16 passed | UI actions, consent, cancellation, disposal, workflow integration, keyboard and responsive fixture |
| Previous UI suite, unchanged | Prior evidence | 15 passed in two complete final repetitions | One earlier final attempt hung at last test and hit outer timeout; isolated case and two full repetitions passed without edits. Cause remains unresolved; no claim of elimination of flakiness |
| Extracted actual main.ts entrypoint | New route absent | 4 passed, included in 23 | Executes the real function with native/browser boundary doubles, tests ownership and late cancellation; not a Tauri runtime test |
| Full TypeScript 5.8.3 | 63 diagnostics | Same 63 normalized diagnostics | Both failed; dependencies missing. Project declares ^5.9.3, so not accepted as a full build |
| Reviewed fixture transpilation | - | Passed | Selected modules only, not Vite/full app |
| Diff/version/protected-surface checks | Frozen base | Passed | Dependency graph, capabilities, CSP, native executor/provider files and original tests unchanged; five mutation probes correctly rejected |
| Secret triage of changed/new code and tests | Not a full scan | 19 files scanned; 12 generic signals rejected after semantic review | Eight synthetic-fixture/reference uses, four ephemeral DOM-ref uses; no credential finding in this bounded coverage |
| Incremental/cumulative patch application checks | Exact archives | Passed for intended bases | Textual checks only |

The four Node failures are three tests requiring missing dist/index.html and one import of unavailable material-icon-theme. Existing assertions/fixtures were not changed to conceal them. New test execution showed failures before the corresponding implementation, including invalid operations, stalled cancellation, browser disappearance, wrong capture-tool name and missing foreground route. A new UI fixture initially counted unrelated selects; its assertion was narrowed to the intended element-reference control, not a weakened behavior oracle. All original test files remain unchanged.

## Not executed / blocked

- Native Rust compilation and the three new native validator unit tests: cargo/rustc unavailable.
- Full Vite/Tauri build, installed-app smoke, all original Playwright suites and release packaging tests: not run; missing declared dependencies/native toolchain.
- Real frontend-to-HTTP fixture: Chromium refused loopback navigation with `ERR_BLOCKED_BY_ADMINISTRATOR`. The policy was not bypassed. Synthetic Resource Timing records are explicitly not substituted as live-network evidence.
- Providers, actual agent approval/denial and filesystem restriction enforcement, Windows/macOS/Linux native WebView behavior: not run.
- CPU/RAM/binary benchmark, complete source/history/binary secret audit, current dependency CVE audit and independent security verification: not performed.

## Security and authority review

The scope is these changes and preserved boundaries, not a comprehensive independent audit. Source/UI checks are not proof of native runtime enforcement. Existing browser ownership, trusted-main-webview validation, input/result caps, run cancellation and original agent approval policy remain in place. Native review input is limited to 8 KiB and fields to 2,048 UTF-8 bytes; this can reject a multibyte value that passed a frontend character-length check, with an explicit error rather than silent truncation.

| Action | Authority/scope | Failure behavior and residual risk |
|---|---|---|
| Read page / diagnostics | User-opened current HTTP(S) browser; approved agent tool path | Exact origin checked; page data remains untrusted and can be forged by page scripts |
| Change page via DOM | Explicit user confirmation or existing native agent approval | Stale refs rejected, mutations dispatched once; page handlers can trigger external side effects; no network sandbox or automatic rollback |
| Resize/capture | Current task-owned existing native guest | Actual available bounds; screenshots may contain private pixels; not globally anonymized |
| Copy evidence | Exact preview confirmation | Clipboard managers may retain content; common-pattern redaction is not complete |
| Save recipe | Explicit current local-project save using original API | Ordinary UI name collision refused; concurrent external writers are not protected by an atomic create-only transaction |
| Start agent/change permissions | No new authority path | Workflow preparation never calls execution/queue/configuration approval commands; actual send stays with original controls |

Browser inspection has bounded output, loop counts and waiting, but does not prove a hard CPU limit against a malicious/very large DOM. A native evaluation timeout stops waiting, not necessarily already-running page JS. The page-local token is an element-reference freshness guard, not authentication. Closing/reopening the panel discards late UI results and transient data. Old report artifacts are preserved as history, not current attestations.

Secret scan details are in secret-triage.json with source hashes; .mjs/.cjs files were byte-identically aliased to .js in an isolated scan directory because the scanner extension list otherwise omitted them. No credentials were authenticated against a provider. The scan does not establish absence of all secrets.

**Operational assurance status: blocked-critical-evidence**, because native build and real browser/provider boundaries have not been validated. This is a missing-evidence status, not a claim of a demonstrated critical exploit.

## Paseo coverage and exclusions

The matrix in docs/PASEO-FEATURE-COVERAGE.md records 27 feature families with hashed source references from the supplied Paseo 0.12.0. Browser capabilities are now covered explicitly; the previous omission is not repeated as an absence claim. Existing Panda tasks, teams, worktrees, recipes, terminal, Git/files, providers, MCP and review tools are retained, not counted as all-new features or proven equivalents.

Excluded/deferred: Paseo Electron/daemon/SDK/protocol transplant; remote service proxy/Hub/mobile/pairing/push; voice/models; arbitrary plugins/hooks/evaluate/upload; a second scheduler/database; automatic worktree merging; always-resident lifecycle; speculative streaming changes without measurements; new independent native browser sessions and full CDP/console/body interception. These would change cost, authority or lifecycle beyond the chosen low-impact contract. This does not assert that all such features are impossible in a future separately scoped change.

## Reproduction, rollback and provenance

The usage guide includes reproduction commands, precise UI paths, limits, platform gates and rollback guidance. Current receipts and logs are stored beside this report. Node tests ran in a verified clean, read-only source/toolchain Linux namespace with network isolation, scratch-only writes and resource limits. Reviewed browser fixtures used in-memory content and blocked network; their isolation is not claimed equivalent to the Node namespace. No uploaded app/provider or installer was launched.

One final-suite browser attempt timed out without a diagnostic root cause. Its log is retained separately from subsequent complete passing repetitions. Test results are reported per actual run, not chosen to imply perfect stability.

Package identity: source-manifest.json covers source outside this evidence folder; package-manifest.json covers every packaged file except itself. The external ZIP SHA-256 identifies the exact archive. Historical evidence manifests describe their historical revisions only.

The distribution version is a project-specific anchored hardened prerelease, not a standard feature-minor increment of the upstream. The generic release skill's alpha/beta/rc-only channel validator was not used to falsely validate the custom hardened channel. Project-specific version, graph and byte-preservation checks are recorded in release-invariants.json. No tag, publication or deployment was performed.

Current external API/security references consulted on 2026-10-24: MDN PerformanceResourceTiming.responseStatus (limited availability; status 0 can mean unavailable/cross-origin), https://developer.mozilla.org/en-US/docs/Web/API/PerformanceResourceTiming/responseStatus ; Tauri security trust boundaries (IPC capabilities are not a sandbox for arbitrary native Rust/subprocess code), https://v2.tauri.app/security/ . These references do not establish application safety or CVE applicability.
