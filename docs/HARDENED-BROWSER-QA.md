# Browser QA and workflow library - 0.3.35-hardened.3

This source revision extends the existing PandaCode native browser. It does not install Paseo, Chromium, Playwright, a daemon, a plugin system or another agent executor. Reference upstream is 0.3.35; the fork's older selective-backport lineage is preserved in HARDENED-REVISION.json.

## Open and use the UI

Open a local project task, select its existing Browser inspector and navigate to the authorized test application. Click **Browser QA** in the browser toolbar. The browser must be a real loaded native guest; an HTML preview does not gain browser-review privileges.

- **Inspect:** snapshot visible light-DOM text and element references; run heuristic layout/name/language/alt checks; inspect the last 100 Resource Timing entries; explicitly start, read and stop page-error observation. Viewport targets fit inside the current panel and report the actual dimensions. Enlarge the panel when a requested target does not fit. This is not phone/OS emulation.
- **Interactions:** take a fresh snapshot, select an element, supply non-secret dummy data, then confirm the exact interaction. Click, fill, select, checkbox, focus and scrolling dispatch DOM events. Changed/expired/consumed references require a new snapshot. Password, file, hidden and credential-autocomplete fields are rejected. This is not trusted OS input; validate the resulting state.
- **Scenarios:** enter a bounded JSON scenario, validate it and explicitly run it. Mutating scenarios require consent. Stop cancels waiting and prevents subsequent steps, but cannot undo an action already sent to the page. The runner stops on the first failed assertion, task/origin change, cancellation or deadline. It never retries a mutation automatically.
- **Screenshots:** capture a local reference and a current viewport for side-by-side review. At most two images, each at most 8 MiB, stay in this panel. They are not automatically sent to an agent or copied into text reports. Raw pixels may contain private data: inspect them before sharing. There is no automatic visual-equivalence score.

The evidence preview and copying require explicit confirmation. Closing QA clears its transient data, references and observers and resets the viewport constraint. There is no background polling. Opt-in runtime-error observation expires after 10 minutes or navigation/clear; it cannot recover earlier errors.

## A small scenario

Replace the origin, selector and independently chosen expected text with your test application. Start the application through the original project/terminal controls yourself; opening QA does not start a server or install packages.

```json
{
  "version": 1,
  "origin": "http://localhost:3000",
  "steps": [
    { "op": "snapshot" },
    { "op": "audit" },
    { "op": "expect_element", "selector": "#save" },
    { "op": "click", "selector": "#save" },
    { "op": "wait_text", "value": "Saved", "timeoutMs": 3000 },
    { "op": "expect_resource", "value": "/api/settings", "status": 200 }
  ]
}
```

Maximum 32 steps, 64 KiB scenario input, 45 seconds total, and 100-5000 ms for an explicit text wait. Additional operations are `expect_text`, `expect_title`, `expect_url`, `resources`, `screenshot`, `fill`, `select`, `check`, `focus`, `scroll` and `resize`. Closed schemas reject unknown operations and fields. `evaluate`, scripts, file uploads and arbitrary shell commands are not accepted.

`expect_resource` is deliberately strict: an unavailable status is not 200 and fails the assertion. Resource Timing is not a complete HTTP recorder, does not reliably expose all response statuses on every WebView, and cannot validate response bodies, API schemas, backend authorization or business data. No headers, request/response bodies, cookies, credentials or URL query strings are collected by this addition. Browser/page handlers can still perform network requests; this is not network isolation.

## Workflow library

Open **Review & coordination > Review presets**, expand **Workflow library: visual, integration and code review**. Choose one of six templates:

1. Visual and responsive review.
2. Frontend/backend smoke check.
3. Bounded bug reproduction.
4. Frontend/backend contract review.
5. Implement, verify, then review.
6. Two-perspective research.

Provide a goal, explicit files/flow, independent acceptance criteria and, when applicable, an authorized HTTP(S) origin. Review the exact preview and confirm. **Prepare in composer** only fills the empty composer of the same idle local task; it does not send. **Save project recipe** explicitly uses the existing recipe store; apply it later through original Recipes controls. Existing recipes with different content are not overwritten by the ordinary UI path. The underlying read/save API is not an atomic create-only operation, so a concurrent external writer remains a limitation.

Opening the original browser or Recipes uses their existing controls. Browser workflows are not silently prepared in Read only mode; the user chooses the appropriate original permissions explicitly. The prior Codex-plan advisor/committee presets remain unchanged. The new templates are instructions, not enforced transaction/approval gates or proof that agents completed tests.

## Agent tools

Two additive app-tool entries, `pandacode_browser_review` and `pandacode_browser_viewport`, use the existing task-owned browser, foreground-task checks, native approval policy, run cancellation and UI serialization. Existing `pandacode_capture_browser` supplies image capture. No new Tauri IPC command or capability is added: the existing `browser_inspect` accepts one closed `review` action validated in Rust.

The page-local ref token prevents accidental stale interactions, not a malicious page forging evidence. All page content, agent output and reports remain untrusted. A user-selected Full access mode retains the original behavior; this extension does not turn it into a sandbox. Prompt limits alone do not constrain a provider's authority.

## Maintenance and validation

Four upstream runtime seams: `src/browser.ts`, `src/main.ts`, `src-tauri/src/browser.rs`, `src-tauri/src/agent_tools.rs`. One previously added hardened file, `src/hardened/review-panel.ts`, mounts the workflow library. New logic lives in isolated `src/hardened/` and `src-tauri/src/browser/` files. No existing tests, provider implementations, scheduler, database, CSP or capabilities were changed.

The incremental patch is `patches/paseo-browser-qa-hardened.3.patch`. It applies to the exact hardened.2 baseline. A cumulative feature-only patch, `patches/paseo-features-reference-0.3.35.patch`, also passed `git apply --check` against the supplied original 0.3.35. That patch intentionally omits the fork's historical security changes and is not a substitute for this full hardened source package. Future upstream changes may require adapting these five integration seams; a clean textual merge is not behavioral compatibility. Keep security patches and feature patches separate. Review tool catalog/action names, BrowserPanel ownership/bounds/disposal, recipe contract and composer lifecycle on every upstream update.

With the declared dependencies/toolchain installed in a trusted development environment:

```sh
npm ci --ignore-scripts
npm test
node node_modules/typescript/bin/tsc --noEmit
npm run build
npm run test:rust
npm run test:release
```

Review any necessary dependency build scripts before enabling them. These commands are reproduction guidance, not evidence that they passed in this delivery. Native Rust/WebView, real providers and a real HTTP test application still need validation, especially Windows/macOS/Linux viewport behavior, page navigation, approval/denial/cancellation, hidden/foreign tasks and status-unavailable cases.

Optional reviewed fixture checks (Playwright/Chromium are development dependencies only, not shipped into the application):

```sh
node scripts/hardened-qa/compile-fixture.cjs . /tmp/panda-fixture
python scripts/hardened-qa/browser-dom-checks.py --root . --output /tmp/panda-evidence
python scripts/hardened-qa/ui-checks.py /tmp/panda-fixture /tmp/panda-evidence
python scripts/hardened-review/browser-regressions.py /tmp/panda-fixture /tmp/panda-evidence
python scripts/hardened-qa/browser-engine-checks.py --root . --output /tmp/panda-http-evidence
```

The final command requires permitted isolated loopback HTTP. It was blocked by this execution environment's browser policy and is not reported as passing. Other browser fixtures use in-memory reviewed DOM and boundary doubles; they do not prove Tauri/provider or live network enforcement. The complete evidence distinctions are in the current release report.

Rollback: close QA and revert only the incremental feature/version patch on a clean branch; no database migration is needed. Project recipes saved explicitly are user data: remove them through the original UI only when intended. Do not delete user changes or reset a live working tree automatically.
