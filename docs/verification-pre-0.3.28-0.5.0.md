# Verification: compatibility before PandaCode 0.3.28

This report accompanies [issue #26](https://github.com/ginmp8/pandacode-extended/issues/26) and the [77-note feature map](upstream-parity-before-0.3.28.md). It records executed checks separately from source inspection and inherited validation history. The unpublished candidate stays at **0.5.0 / 0.5.0+pc.0.3.47**.

## Frozen inputs

| Input | Identity |
| --- | --- |
| Requested base branch | `feat/0.5.0-upstream-0.3.47` |
| Extended baseline | `68e4d0615ec864a2dd9e9630e0e3410432e27207` |
| Original 0.3.27 snapshot | `612ca79c8e7db74461040bc27ddf1a3dcebb32ab` |
| Earlier release-evidence SHA-256 | `4a9d40bf2b695b0c533fc15eaac026878603d24d712693fcc43b10f835d44720` |

All **77** source notes match the exact original snapshot and Extended baseline. All **28** currently published `0.3.0–0.3.27` bodies contain the complete source feature note followed by the generated installation appendix. The current public inventory does not establish release/tag records for the **49** `0.2.*` notes. The new metadata supplement retains the existing newer reviews, original ancestry and false full-parity/rebase flags.

## Reproduced failures and matching corrections

### Native initialization

The baseline proof uses a behavior-neutral extraction of the production instance initializer, leaving its original synchronous loading intact. It seeds a real settings file with a unique test-vault reference, enters the actual settings loader and holds its vault-read boundary with channels. The test requires initialization to return and Runtime to remain accessible before that read is released. A cleanup guard releases the read even on failure.

The baseline compiled successfully, then **failed the intended setup-return assertion**. The asynchronous candidate is checked against the same oracle. This demonstrates the blocking dependency in the initializer used by `jev::setup`; it is not a measurement of a launched native GUI or a macOS permission dialog.

The candidate passed **65/65** focused native Jev/decision tests, including **12** new cases. They cover remembered completion without subscribers, simultaneous and canceled waiters, fresh empty settings without vault access, queued Save ordering, independent key retention/clearing, failed reads without partial publication, byte-preserved recovery and retry, held/failed legacy migration, worker failure, saved Python selection, and lifecycle checks after a wait.

Installer tests execute the production admission/saved-path preparation method; browser tests execute the production reservation boundary. They do not launch an installer or browser runtime. Decision readiness placement received source review. After the focused run only formatting of new native hunks changed; the final formatted files passed all-targets project Clippy and independent review.

### Startup response after a newer Settings save

The browser regression holds the initial configuration snapshot, completes a real Jev Settings Save, verifies the exact save payload and newly available assistance, then releases the older snapshot. It closes and reopens the task menu so its final assertion observes current state.

On unchanged baseline `main.ts`, the final assistance-menu assertion **failed** at its existing five-second bound. With the small saved-settings guard, the **same test passed**. Three additional cases cover usable workspace navigation/drafts while configuration is pending, disabled settings controls, intact saved flags with empty password inputs, disposal before late completion, and rejected configuration IPC.

The full settings, assistance and live-browser suites passed **44/44**, with no skipped or flaky results, using Chromium 153 and an isolated preview server. One test title was subsequently corrected to distinguish rejected configuration IPC from native vault-read recovery; every test body and assertion remained byte-identical. Native vault-read failure returns disabled recovery state and retains native Save protection; it is not the rejected-IPC scenario.

### Release evidence cannot silently disappear

Both new metadata guard tests **failed with “Missing expected rejection”** against the previous validator. The updated validator passes all **10** guard tests. It checks the early baseline/snapshot, all note paths/digests in sequence, source-only publication restrictions and the unchanged older provenance. Mutation cases recompute the evidence digest before checking semantic omissions or invented dates, so they exercise more than checksum rejection. Windows CRLF normalization remains covered. The combined release guard now verifies **97 distinct upstream notes**.

## Executed local checks

| Check | Result |
| --- | --- |
| Node unit tests, `npm test` | **232 passed** |
| Release tests, `npm run test:release` | **63 passed** |
| Release metadata guard subset | **10 passed**, included in the 63 release tests |
| Desktop TypeScript and production build | Passed; existing chunk-size advisories remain |
| Website build | Passed; four static pages generated |
| Jev settings/assistance/live browser suites | **44 passed**, no failures/skips/flaky results |
| Native Jev/readiness/recovery tests | **65 passed**, including **12 new startup cases**; zero failed/ignored |
| Native project Clippy | Passed: `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --no-deps -- -D warnings`; full CI uses its existing command without `--no-deps` |
| Credential-shaped Slack fixture scan | Passed; **1,510 files checked**, no token-shaped fixture detected |
| Diff whitespace check | Passed |

The native and frontend changes are reviewed independently from their writer. The PR's CI checks are the authority for the final pushed commit; this table records the local executions rather than assuming CI or a platform result that has not run.

## Reviewed source identity

The integrated source fingerprint is **`6eb553a1b27281813425f8939f5945785d63b721bc897f3eef35529e3bcbd19e`**, computed from committed blob identities for application code, tests, release tooling, manifests and active provenance:

```sh
git ls-tree -r HEAD -- src src-tauri tests scripts mobile relay package.json package-lock.json tsconfig.json vite.config.ts EXTENDED-RELEASE.json | sha256sum
```

The integrated native files match the independently reviewed/tested slice byte-for-byte. Documentation-only commits do not change this fingerprint. The original note manifest has its own digest above.

## Preserved contracts and remaining limits

The change keeps OS-vault storage, immutable credential generations, JSON-as-commit-point persistence, failed-load recovery, omitted-key retention and explicit clearing. Initialization publishes a complete state before queued saves proceed. Pending capabilities remain disabled. Explicit operations wait before snapshot/admission, then recheck cancellation, shutdown and task ownership. The workspace does not wait for optional configuration during boot. No version, dependency, database schema, bundle/data identity, permission default or publication policy changes are introduced.

Native unit tests use controlled local files and the test credential store; browser tests exercise the real frontend with preview IPC fixtures. They do not certify real Keychain, Secret Service or Credential Manager prompts, macOS/Windows GUI startup, physical mobile devices, live Jev inference, cloud accounts, runtime installation, VMs/simulators or native installers. Signed-in Connect retains its separate inherited startup credential path. Worker-panic settlement is covered in the unwinding test profile; release `panic=abort` remains unchanged. Historical upstream performance claims are not re-measured here.

The earlier reports for [0.3.42–0.3.47](verification-0.5.0.md) and [0.3.28–0.3.42](verification-historical-0.5.0.md) remain distinct evidence. No original source note, published `0.4.0` history or immutable `HARDENED-REVISION.json` record is rewritten. This work does not merge the PR, create a release tag, publish installers or deploy services.
