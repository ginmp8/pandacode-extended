# Extended 0.5.0 verification

This report records the executed checks for the source integration in [issue #22](https://github.com/ginmp8/pandacode-extended/issues/22), prepared on **2026-10-25**. The product version is **0.5.0** and the display identity is **0.5.0+pc.0.3.47**. The [integration map](upstream-integration-0.5.0.md) records the upstream release-by-release behavior, implementation surfaces, preserved Extended contracts and immutable source identities.

## Source and environment

The branch starts at Extended `main`, `5d4cb69b0a97c46eef544303137c73a81f4c5b59`. The reference is original PandaCode `v0.3.47`, `4187553cb48db96657609948f6b0904556845d45`. All 1,146 regular files in the supplied archive match that tag. The archive SHA-256 and historical evidence digest are recorded in the integration map and validated independently from the product version.

Checks ran on Linux x86_64 with Node **24.22.1**, npm **11.9.0**, Rust **1.99.1**, GTK/WebKit development dependencies, and official Chrome **156.0.8078.47** in disposable browser contexts. Dependency lockfiles were retained. Mobile dependencies match the upstream 0.3.47 lock after normalizing the root package name and version. Mobile and relay installations used `npm ci --ignore-scripts`.

## Executed results

| Gate | Result |
| --- | --- |
| Desktop build and TypeScript | Passed: `npm run build` |
| Desktop Node unit tests | **227 passed**, 0 failed |
| Release/version/history tests | **53 passed**, 0 failed |
| Website | Passed: 4 generated pages |
| Rust library tests | **1,334 passed**, 0 failed, **41 ignored** |
| Rust Clippy, all targets, warnings denied for project code | Passed |
| Desktop browser regressions | **424 distinct scenarios passed; 1 performance check failed** in the software-rendered environment. |
| Mobile unit/protocol/state tests | **179 passed**, 0 failed |
| Relay tests | **22 passed**, 0 failed |
| Mobile and relay TypeScript | Passed |
| Expo lint | Passed |
| Android JavaScript/Hermes export | Passed: 2,189 modules, approximately 5.5 MB |
| iOS JavaScript/Hermes export | Passed: 2,107 modules, approximately 5.3 MB |
| Mobile assets exported | 95 |
| Slack credential-shaped fixture scanner | Passed: 3,066 files checked |
| Whitespace | Passed: `git diff --check` |

Vite reports existing large syntax-highlighter chunks. Clippy reports warnings in the vendored `glib` dependency, while the project passes `-D warnings`. Neither warning was converted into a claim of a clean dependency audit.

## Browser evidence

The broad desktop runs covered the changed inspector/sidebar/remote/provider flows and retained GitHub, Board, chat, connection, search, thread, simulator and VM behavior. Failures were investigated rather than suppressed. Focused reruns verified the resulting source or fixture corrections; this is cumulative scenario coverage, not a claim that one uninterrupted full browser run was green.

| Browser area | Distinct passing scenarios |
| --- | ---: |
| Changes, inspector tabs, sidebar sections, remote startup/images and provider settings | 66 |
| Chats and thread ordering | 27 |
| GitHub, GitHub hub and Board | 57 |
| Existing remote task controls | 10 |
| Layout preferences | 35 |
| General desktop UI | 126 |
| Simulator | 13 |
| Retained browser, concurrency, connection, document, Jujutsu, search, thread, turn-change, agent-usage and VM flows | 90 |
| **Total across 27 suites** | **424** |

**Unresolved performance check:** `tests/simulator.spec.ts` → `rapid pose changes keep the window responsive and end in the last pose` measured **311.7 ms** worst event-loop lag on its final isolated rerun, above its unchanged **150 ms** limit. The preceding run measured 377.1 ms. The final rerun verified the exact `['open', 'book']` native-call queue, final Book pose and cleared spinner before evaluating latency. The simulator/native implementation is unchanged from `main`; the timing result still requires a check in a suitable hardware-accelerated environment. The test is retained with the original threshold and is not marked skipped or passed. These browser results must not be represented as an entirely green suite.

Two real GitHub workflow defects were fixed: returning to an existing issue draft now reveals its Board, and a delayed close event from an earlier shared dialog cannot cancel a newly opened issue retry approval. Actual dismissal still cancels the pending action. Existing draft text and attachments are retained, with no implicit send.

Other source corrections covered remote retry-attempt correlation, remembering a background mirror prompt after asynchronous context preparation, and cancelling removed inline-diff loads before they can mount an orphaned viewer. Regression tests exercise delayed results, retry races and stale identities.

Fixture corrections preserve the product contracts:

- New tasks and worktrees start in **Standard** mode. Full access remains a deliberate choice and persists only where requested.
- Branding checks expect **PandaCode Extended**. Shortcut assertions accept the host platform's Ctrl or Command label.
- After fixture reloads, tests wait for the selected task/composer before opening filters or starting work. They no longer race application startup.
- GitHub draft assertions respect unlisted, unsent drafts, actual linked-item titles, portal menus and persisted link refreshes.
- Simulator tests explicitly select their emulated macOS/Windows platform and wait for the lazy 3D module. The drag test uses the same gesture endpoints without redundant intermediate pointer steps; exact text and key order permit valid debounce batching. The simulator implementation remains unchanged from `main`.

### Reproduction

Run the new feature gates with an installed Chrome/Chromium executable:

```bash
export PANDACODE_TEST_CHROME=/path/to/chrome
npx playwright test \
  tests/changes-view.spec.ts tests/inspector-tabs.spec.ts \
  tests/sidebar-sections.spec.ts tests/remote-progress.spec.ts \
  tests/remote-first-message.spec.ts tests/remote-images.spec.ts \
  tests/provider-settings.spec.ts --workers=2
npx playwright test tests/chats.spec.ts tests/thread-order.spec.ts --workers=1
npx playwright test tests/board.spec.ts tests/github-hub.spec.ts tests/github.spec.ts --workers=2
```

The remaining covered suites are `agent-usage`, `browser`, `concurrency`, `connections`, `document-viewer`, `hardened-041-connections`, `jj`, `layout-preferences`, `remote`, `search-panel`, `simulator`, `thread-panels`, `turn-changes`, `ui`, and `vm` under `tests/`. Run simulator cases with one worker when using software-rendered 3D. Historical screenshot fixtures expect a writable `/private/tmp`; CI creates that temporary directory explicitly.

The CI change keeps repeated chat/thread-order checks, adds the upstream integration suites, and adds separate Node 24 mobile/relay jobs. CI performs validation only.

## Native, mobile and release reproduction

```bash
npm run build
npm test
npm run test:release
npm run site:build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
python security-review/hardened.3/check-slack-fixtures.py .
git diff --check

npm ci --ignore-scripts --prefix mobile
npm test --prefix mobile
npm run typecheck --prefix mobile
EXPO_NO_TELEMETRY=1 npm run lint --prefix mobile
npm ci --ignore-scripts --prefix relay
npm test --prefix relay
npm run typecheck --prefix relay
```

From `mobile/`, compile the configured native JavaScript bundles without signing or publishing:

```bash
EXPO_NO_TELEMETRY=1 EXPO_OFFLINE=1 \
  node node_modules/expo/bin/cli export --platform all --max-workers 2 \
  --output-dir /tmp/pandacode-mobile-native-bundles
```

The 41 ignored Rust tests require installed/signed-in providers, live GitHub or bot services, a real simulator/VM, platform facilities or other explicit local fixtures. The normal test command leaves them ignored. Both installed-Devin tests are among those unexecuted checks; no live provider response is implied by the mocked ACP coverage.

Release tests validate desktop/mobile manifest agreement, source identities, preserved historical evidence, original/Extended note separation, bundle identity, disabled publication policy, and a future product-only patch through the documented version command. A patch retains the existing integration coverage reference; it does not rewrite the upstream review merely to match a new product version.

## Preserved contracts and migration

Database migration **28** adds durable message attachments after Extended's existing migration **27** for GitHub issue actions. Tests cover preserved rows and reopening/migration behavior. The desktop application identifier, credential paths and data paths remain stable. Existing historical review bytes and published Extended 0.4.0 notes remain unchanged.

New Connect paths retain exact current-key device trust, durable replay admission, bounded data and image reads, ordered task mutations, and the separate disabled-by-default remote Full access setting. Long repository starts have their own bounded scheduling lane so they cannot block Stop and approval controls for existing tasks.

Mobile tests cover key changes, trust revocation during asynchronous work, replay/storage failures, delayed security writes, account deletion and sign-out, stale cache writes, credential-origin binding, and account-bound outbox delivery. The relay tests include account deletion races and registration generations that prevent old tokens from becoming valid after immediate re-registration. See [mobile setup and trust](../mobile/README.md) for the conservative replay-checkpoint recovery behavior and local data semantics.

## Explicit limits

These results validate the source candidate and its automated fixtures. They do not certify signed native desktop installers, Windows/Avast behavior, packaged macOS/WKWebView rendering, real iOS/Android device behavior, native signing, OS credential-store behavior on every platform, live OAuth, an operational relay deployment, or real phone-to-computer latency. The Expo exports compile JavaScript/Hermes bundles; they are not store binaries or physical-device tests.

The requested 0.3.42–0.3.47 release range is mapped and integrated while intentional Extended restrictions remain. Historical source ancestry is still 0.3.28; this report does not claim a full upstream rebase or retroactive proof of every older upstream feature.

This work prepares the source branch and PR. It does not merge, create `v0.5.0`, publish installers, deploy the relay, enable an updater feed, or publish an npm/mobile package.
