# Extended 0.5.0 historical integration: verification

This report covers the historical PandaCode **0.3.28–0.3.42** integration in [issue #24](https://github.com/ginmp8/pandacode-extended/issues/24). The technical version remains **0.5.0** and the display version **0.5.0+pc.0.3.47**. It supplements the [parent verification report](verification-0.5.0.md). The [feature map](upstream-parity-0.3.28-0.3.42.md) specifies inclusions, intentional exclusions, source identities and implementation paths.

## Reproducible inputs

- Frozen Extended parent: `7f14199973d7bfc62dfb07fa08cb1e3bbd328c5e`, on `feat/0.5.0-upstream-0.3.47`.
- Original tag commits: `70bdc708303c8681d8eb0b46f2cdb0ff087aa6eb` (0.3.28) and `bca0b9f1f03a1cc3f8a8e6330881c34103f14a46` (0.3.42).
- All 14 published release feature sections were compared with their checked-in source notes. Generated upstream installation sections were evaluated separately, not adopted as Extended guidance. The 0.3.36 note is recorded as source-only.
- A separate evidence manifest records source-note/full-release-body digests. Release tests check that manifest, all 15 historical source notes, the immutable legacy review digest, and synchronized versions.
- Independent feature slices used isolated worktrees. Native Cargo builds used a shared target sequentially. Vite caches were isolated because shared optimized-dependency caches can invalidate other worktrees' browser sessions.

The initial local assembled native candidate combines the root historical integration through `a986b40ebc285d10b0c2c5210d45692ac957596d` with cloud commit `79f9e82d97715f15254d2b3051cb09d3b0915c4c`. Its full 1,436-test run used tree `58ca47e591b49828be15b542fe24fe1fe47c961a`. Clippy then identified two storage convenience wrappers whose only remaining callers are tests. Local validation commit `e53f17ac6f31225b0fe52b104d1fa402e8c26b3b` adds exactly two `#[cfg(test)]` attributes; the affected storage suite (52 passed, 1 ignored) and complete Clippy gate passed on tree `fd7c0aa7c000b62e0793ae37fccf28f1f2c5e69e`. Subsequently, GitHub CI repeated the complete native suite on published commit `277423f77dd472abfe92c0210bf78687175e8962`: **1,436 passed, 42 ignored**, with all-target Clippy also passing without `--no-deps`.

The native-gate source fingerprint is `c662a6e0c1b3194d1ac4ca8e3daa8899a3ab72998372adb9783e6979352ab9fd`, calculated from `git ls-tree -r HEAD -- src src-tauri mobile relay package.json package-lock.json tsconfig.json vite.config.ts | sha256sum`. Final frontend validation adds the independently reviewed queued-close guards in `src/jev-assist.ts` and `src/motion.ts`, producing fingerprint `921d1a4ee5d3ca92d424cf32a944099997f0f587cd1dc2a495f05da62a020fb6`. The final branch matches that tested production source exactly. Native tree `fb9d639f6de2601f16a044d534cbef12a2078288`, mobile, relay, manifests and all four lockfiles remain identical to the native gate. The frontend build and unit tests passed again after the guards. Local validation commit IDs identify isolated checkouts; identical Git trees establish the content relationship to the published branch.

## Executed gates

Validation ran on Linux. Browser tests use disposable Chromium contexts and the local Vite preview. Mobile tests exercise the TypeScript logic; they are not physical-device installation tests.

| Gate | Result |
| --- | --- |
| Combined desktop TypeScript and production build | Passed |
| Desktop Node unit tests | 232 passed, 0 failed |
| Release/version/provenance/offline inventory tests | 61 passed, 0 failed |
| Website release-history build | Passed |
| Mobile unit tests | 185 passed, 0 failed |
| Mobile TypeScript and lint | Passed |
| Relay unit tests | 27 passed, 0 failed |
| Relay TypeScript | Passed |
| Complete assembled Rust library suite, locked | 1,436 passed, 0 failed, 42 ignored, 0 filtered |
| Complete assembled Clippy, all targets, warnings denied | Passed locally with `--no-deps` and in GitHub CI without it; affected storage tests: 52 passed, 1 ignored |
| Complete desktop browser collection, across frozen runs and correction reruns | **942 distinct scenarios covered: 941 passed, 1 existing simulator timing check failed; 0 unverified** |
| Cloud VM/deploy browser integration | 25 passed, 0 failed in the isolated cloud slice |
| Production dependency advisory check | `npm audit --omit=dev --json`: 0 advisories reported for the resolved production dependency graph |
| Slack credential-fixture scan and whitespace checks | Passed on the final tree |

The advisory result is a point-in-time registry lookup, not a security certification or evidence that every third-party/tarball dependency has complete advisory coverage. Document parser pins remain explicit in the lockfile. No native installer SBOM was generated: the local scanner helper was tested with controlled scanner responses, and its reports do not imply a real installer scan.

## Browser coverage and the unresolved timing check

The final Playwright collection contains 942 scenarios. Results were reconciled by filename and complete test title across frozen broad runs and focused reruns, with later correction evidence taking precedence. Every collected scenario has an executed result. The resulting 941 passes cover functional assertions; the remaining result is the simulator latency failure below.

The first broad run collected 924 cases and recorded 631 passes, 23 failures and two interrupted cases before it was stopped at the repeated macOS-only streaming-fixture setup failure. The remaining 268 cases had not started. The resumed run covered every subsequent file plus the new cloud cases: 275 passed and five failed. All setup/navigation/count failures were diagnosed and verified after correction. The full streaming file passed 27 cases, the support file passed seven, and targeted UI reruns retained menu hit testing, permission persistence, activity rendering and canceled-folder ownership. The final eight-file historical integration set has passing evidence for 125 distinct cases, including the new queued-close regression, plus two existing dialog checks. These phase counts overlap and must not be added together as a project total.

**Unresolved:** `tests/simulator.spec.ts` → `rapid pose changes keep the window responsive and end in the last pose` measured **511.7 ms** worst event-loop lag against its unchanged **150 ms** limit in this software-rendered environment. Its native-call queue, final Book pose and cleared-spinner assertions completed before the latency assertion. The simulator implementation and test remain unchanged from the frozen parent, whose verification report already records the same timing limitation (311.7 ms on its final isolated run). The threshold remains enforced. A suitable hardware-accelerated environment is still needed to resolve this performance result; the browser evidence is not an entirely green suite.

## Published CI and browser job boundaries

[The first PR CI run](https://github.com/ginmp8/pandacode-extended/actions/runs/40241947938) on `277423f77dd472abfe92c0210bf78687175e8962` passed the web, native, mobile and relay jobs. The desktop browser job passed all 54 repeated chat/thread-order executions, then passed 124 of 125 integration scenarios. The remaining Board retry-dialog scenario timed out at its initial visibility assertion, before its cancellation assertions ran. The historical browser command was not reached in that run.

The Board failure was reproduced locally with an 8× CPU slowdown: the original five-second dialog assertion included both application restoration and startup retry work. The corrected fixture waits for the enabled composer after its seeded reload, then runs the unchanged five-second dialog assertion and every existing cancellation/security check. Under the same slowdown, these separate readiness and dialog waits completed in 3.993 seconds and 3.636 seconds, and the case passed. The identical startup fixture in the adjacent retry case uses the same readiness check. No application logic, seeded intent, timeout, cancellation oracle or retry policy changed. This experiment supports a fixture synchronization correction; it is not a startup performance improvement claim.

The complete Board suite on CI-matching Chromium 153.0.8010.12 exposed one additional fixture race: the GitHub-disabled case clicked the Board shell before the preview helper had initialized. An enabled-composer check after that reload preserves the existing no-GitHub-read/write assertions. The final full Board run passed **25 of 25 cases**, with two workers, zero skips and unchanged timeouts. Both fixture corrections received independent review; production source fingerprints remain unchanged.

[The second PR CI run](https://github.com/ginmp8/pandacode-extended/actions/runs/40243697313) on `b63788e52fff8bd9aa78f54552f8b40ab5af1296` passed five jobs, including all **229 historical browser cases**, the complete native suite and strict Clippy. The desktop job again passed all 54 repeated chat/thread-order executions, then passed 123 of 125 integration cases. Every Board case passed. Retained traces identified two further fixture boundaries: DeepSeek's test clicked New task before projects loaded and therefore inspected the restored Claude task; the Actions Fix test counted dispatch immediately after an optimistic transcript update, before the asynchronous send reached IPC.

Controlled delays reproduced both failures with unchanged production code. Provider fixtures now establish boot readiness before New task and the intended provider before inspecting permissions; DeepSeek retains exactly four modes. The same prerequisite applies to the five provider cases with that identical setup sequence. Positive send/start counts in the two affected GitHub Hub cases use the existing bounded polling expectation for exactly one invocation. Their zero-automatic-start checks remain immediate, and edited prompt text, task ownership and selected harness assertions remain intact. The corrections change neither permission contracts nor assertion timeouts.

The final full-file runs on Chromium 153.0.8010.12 passed **33 provider-settings cases** and **24 GitHub Hub cases**, with two workers and zero failures, skips or flaky retries. Independent review checked the controlled original failures, corrected scenarios and retained assertions. An initial added Kilo identity predicate incorrectly required a decorative logo; the corrected predicate checks the actual selected provider container, while preserving the original catalog, permission and account checks. This local fixture iteration and its initial 32-of-33 result were retained separately from the final passing run.

[The third PR CI run](https://github.com/ginmp8/pandacode-extended/actions/runs/40245343254) on `cb66914ef87b1a639dadf6b866cec57cfaf9fd4d` passed web, native, mobile, relay and the complete desktop browser job (**54 repeated executions plus 125 integration cases**). The historical job passed 228 of 229 cases. Its launcher recovery fixture cleared a simulated connection failure before clicking Retry; an automatic snapshot refresh recovered the conversation first and hid the control. The recovery implementation and fixture block were unchanged from the frozen parent.

A controlled original run reproduced the hidden-control timeout. The corrected three recovery fixtures use a fixed clock paused after initial loading, retaining every explicit polling interval and the original 20-second test budget. The transcript variants also wait for their three initial Markdown bodies before starting the existing zero-mutation observer. All original refresh, mutation, streaming, status, coordination, error, actual-click and recovery assertions remain. **All 39 launcher cases passed** on Chromium 153 with two workers. Removing only the actual Retry listener in a diagnostic response route made all three recovery variants fail at their intended notice-hidden or empty-state-visible assertion; the transcript variants still recorded zero idle mutations. Independent review confirmed that automatic recovery cannot conceal the disabled manual handler in these corrected scenarios. No production source changed.

[The fourth PR CI run](https://github.com/ginmp8/pandacode-extended/actions/runs/40246933655) on `c65a8baba0f5ca29e5a5225d1c1ec14fa620c9a9` again passed web, native, mobile, relay and the complete desktop browser job. The historical group passed 228 of 229 cases, including every launcher case. The Agent usage trace showed its lazy dependencies arriving approximately four seconds after the dialog shell appeared. The Refresh control first mounted disabled near the end of the existing five-second enabled assertion; the final captured frame already showed it enabled, without `aria-busy`, with the expected completed usage data. No disposed or permanently stalled view was observed.

The shared usage fixture now waits for Refresh to be visible after the dialog shell appears. This establishes that the lazy view has mounted; it does not wait for data completion. Every caller retains its original enabled or disabled assertion and five-second limit. A matched experiment held the module response for four seconds and delayed each account response by 300 ms: the original case failed while three requests were active, and the corrected complete opening/reopening scenario passed with exactly 30 requests, all `refresh: false`, maximum concurrency three and the existing no-polling checks. The full Agent usage file passed **8 of 8 cases** in 18.766 seconds on Chromium 153 with two workers, zero skips and zero flaky retries. Independent review cleared the trace diagnosis, exact two-line fixture change and matched evidence. Production code, permission contracts, request counts and timeout values remain unchanged.

CI now runs the existing desktop integration commands and the historical parity command in independent jobs, each with the existing 20-minute budget. All three Playwright command strings, worker counts, repeats and test assertions are preserved. A desktop-group failure can no longer prevent the historical command from running. Both browser jobs retain failure traces and screenshots from `test-results/` for seven days using a commit-pinned artifact action; hidden files are excluded and repository permissions remain `contents: read`.

Current status is attached to [PR #25](https://github.com/ginmp8/pandacode-extended/pull/25). CI covers the selected integration groups; it does not replace the broader 942-scenario evidence or clear the separate simulator timing result.

## Focused feature checks

These overlapping checks supplement the assembled suites; their counts must not be added together as unique project totals.

| Slice | Evidence |
| --- | --- |
| Bot profiles, task binding, Connect key handling and scoped Skills | 149 targeted native cases passed; 2 existing tests ignored. 39 distinct browser cases passed across bots, web chat, profiles and existing Skills. |
| Providers, Jev, explicit approval review, native teams/remotes and platform wiring | 715 unique targeted native cases passed; 21 existing tests ignored. New approval-mode browser cases and existing remote cases passed, alongside Jev/provider settings scenarios. |
| Manual cloud adapters and VM tools | 100 cloud native cases plus 3 VM tool cases passed; 1 existing live VM test ignored. All-target Clippy passed. 25 VM/deploy browser scenarios passed under the desktop CSP. |
| File tabs, documents, drops and inspector ownership | 14 initial document/browser cases passed after correcting isolated-cache setup. A combined affected-area run exposed obsolete navigation assertions; 21 focused correction scenarios subsequently passed, including unchecked Markdown image rejection. |
| Release tooling | Original Extended version tests remain. New cases reject historical evidence drift, malformed/empty scanner output, updater/original asset names and inherited scanner network options. |

Cloud browser cases exercise each added backend's connect/create/lifecycle/command/delete flow, explicit leases, unsupported actions, Vercel project prerequisites, InstaBox image selection and delayed-account changes. They use deterministic mocks; no real account, paid machine or provider credential was used.

## Before/after and independent review findings

The feature loop recorded expected missing-feature failures at the frozen baseline: a project-file tab was absent, and Claude's explicit review mode was absent from the menu. The bot-detail race regression failed before its correction. Test intent and permission/resource boundaries were retained while adapting navigation assertions to the new file-tab surface.

Independent reviews found and corrected the following concrete issues:

| Finding | Correction and regression evidence |
| --- | --- |
| Windows ancestor-junction replacement could defeat a pathname recheck | Validate the opened file handle's normalized final path against the exact authorized target before reading. Refuse reparse points, alternate data streams and protected paths. Cross-platform path-identity tests exercise the validation contract; Windows runtime behavior remains unexecuted. |
| Markdown data images bypassed native dimension checks; workspace images could follow a protected alias | Render unchecked inline data images as alt text. Bound local image requests and route workspace image bytes through the document reader's handle/path/dimension checks. A real PNG replaces the old four-byte image fixture; protected aliases and invalid rasters are rejected. |
| boat command output accumulated across individually bounded response pages | Retain a 4 MiB UTF-8-safe tail across pages, verified with multiple pages crossing the aggregate limit. |
| CodeSandbox lost the explicit lease on later adapter instances | Send the default timeout only during creation. Later wake/reconnect calls omit it; command submission is not retried after an ambiguous failure. Provider-side restart semantics remain documented. |
| CodeSandbox used a page size above the API maximum | Request 50 items and bound page traversal; the regression returns multiple pages and rejects unexpected request shapes. |
| Delayed InstaBox image validation crossed an account change | Capture/recheck the current key and generation under shared settings serialization, plus UI generation/disposal guards. Native/browser tests reproduce the account-A/account-B sequence. |
| A delayed bot detail response could change another visible profile's save target | Reject stale detail loads before changing selection/detail state; the browser regression verifies the save remains attached to the visible bot. |
| A queued close event could dispose and hide a newly reopened Jev dialog | Ignore an earlier close while the shared dialog is open; keep the listener for a real close and remove it through the existing disposal path. The deterministic regression failed before the fix, then passed with the complete Jev assistance suite and existing dialog checks. |
| Empty component objects or unidentified scan output could look like clean SBOM coverage | Require identified CycloneDX components and Grype reports. Null/empty components and bare `{ "matches": [] }` no longer produce complete coverage. A valid identified zero-finding report remains supported. |

Reviewers re-read the committed cloud corrections, document/inventory fixes and dialog lifecycle guards and reported no remaining material finding in those scopes. This is a bounded code review, not a claim that the whole application or every dependency is vulnerability-free.

## Verification corrections and environmental limits

The first native archive test attempted to create hostile ZIP paths with a writer that normalizes them. Its fixture now patches the raw local/central filenames so the production reader is actually tested; the reader's rejection rule was retained. Initial browser cold-load failures exposed a shared Vite optimization cache and a late spreadsheet dependency reload. Separate caches and explicit document dependency optimization removed that interference.

The initial root mobile run had no mobile dependencies in that checkout. The run was repeated using dependencies whose lockfile matched the candidate byte for byte. Final results above count individual test cases, not file-only summaries from a restricted process runner.

The broad desktop browser run exposed fixture assumptions that needed explicit correction. The usage fixture must include the newly installed Kilo harness while retaining the three-request concurrency limit and no-polling assertions. File-navigation fixtures must select the new document tab, explicitly request an SVG browser preview, and return to Conversation before interacting with its inert transcript. The existing task/project ownership and external-attachment assertions remain.

Five other failing cases were reproduced unchanged on the frozen parent: two Jev scenarios and three launcher scenarios. The saved-diff fixture now declares its task attribution instead of weakening the renderer's ownership guard; the task-switch scenario explicitly opens that task's Browser and also checks that no previous frame is inherited. Launcher fixtures now expect the secured Ask default. A separate unavailable-current-provider case preserves disabled-selection and zero-discovery-call checks. No production permission default, performance threshold or test skip was changed to make these cases pass.

The streaming fixture's exact New task label assumed macOS; its portable label now accepts the explicit Mac/Linux forms, and its Settings key presses use `ControlOrMeta`. All 27 streaming cases passed with their ordering, snapshot, sanitization and task-switch assertions intact. The About fixture now uses the existing “Upstream releases” label and additionally verifies Extended's own repository and issue links. Exact menu expectations include the new Claude review choice and Kilo, while retaining every viewport/hit-target and persistence assertion. Two UI fixtures now repeat the existing task/history readiness checks after their own reloads, before testing activity or canceled folder selection. Neither change alters cancellation, task ownership or disclosure behavior.

The ignored native cases require installed/signed-in harnesses, platform facilities, live services or explicit local fixtures. They were not enabled or counted as passes. No live Kilo, Cline, Claude, Devin, OpenRouter, cloud or bot service session is certified. Actual provider plans, quotas and API deployment differences remain subject to live-account verification.

Windows handle validation, macOS single-instance behavior/Keychain prompts, native Linux WebView geometry, native mobile packaging, physical Connect devices and target-platform installers require their respective environments. Desktop browser mocks do not substitute for those checks. WebKit-specific execution is not claimed.

Existing Extended protections remain part of the validation target: OS protected secrets, pinned device trust, persistent replay, separate remote privilege grants, named-account isolation, scoped/hash-pinned Skills, scriptless preview origins, bot notification approval and disabled automatic updates/installers. The intentional exclusions in the feature map remain in force.

## Re-run commands

```sh
npm run build
npm test
npm run test:release
npm run site:build
npm test --prefix mobile
npm run typecheck --prefix mobile
EXPO_NO_TELEMETRY=1 npm run lint --prefix mobile
npm test --prefix relay
npm run typecheck --prefix relay
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --no-deps -- -D warnings
PANDACODE_TEST_CHROME=/path/to/chrome npx playwright test --workers=2
python security-review/hardened.3/check-slack-fixtures.py .
git diff --check
```

Use installed build/test dependencies and a fresh disposable browser context. These commands do not authorize a release publication, merge, tag, cloud resource creation or live account session.
