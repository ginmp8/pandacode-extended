# PR #40 audit corrections

This record tracks implementation work for [issue #42](https://github.com/ginmp8/pandacode-extended/issues/42) and the additional concrete findings from the independent audit of `d769cba4c5444fb6e7faced83e19a567a5d74734`. The related contracts remain [#38](https://github.com/ginmp8/pandacode-extended/issues/38), [#39](https://github.com/ginmp8/pandacode-extended/issues/39), and [#41](https://github.com/ginmp8/pandacode-extended/issues/41).

The technical version remains **0.6.0**, with display reference **0.6.0+pc.0.3.50**. This is corrective work in the existing integration PR. It does not authorize a merge, release, tag, installer publication, or issue closure.

## Baseline and scope

- Extended base: `479f2e6ac700d216d62ae08681e36aaaa20fa074`.
- Audited PR head: `d769cba4c5444fb6e7faced83e19a567a5d74734`.
- Upstream 0.3.49: `e71acdda9eb5d488122081c4dbc49ee1a5b4f9e2`.
- Upstream 0.3.50: `15541a51f3f5d8ee32d91349f9d5035a53c4cd19`.
- All changes are on the existing PR branch, `feat/0.6.0-upstream-0.3.49-0.3.50`, targeting `main`.
- The old green CI run is baseline evidence only. Acceptance of these corrections requires checks on the final corrected source revision.

## Correction map

The table identifies implementation and regression paths. A row describes a correction, not a substitute for the execution results or remaining platform gates below.

| Finding | Correction | Regression evidence |
| --- | --- | --- |
| SEC-01 | `remote/grants.rs` computes effective method authority from the authenticated key, current presence, legacy pin, scoped grant, local Full switch, and memory/disk denial fences. Persistence failures immediately deny the running process. | Native grant tests inject a scoped-file failure during Full OFF, assert every method denied, reopen persisted state, and require complete sign-out before recovery. |
| SEC-02 | Local grants and pairing redemption share a fenced cross-store transition under Remote → GRANTS lock ordering. A valid code is consumed before attempted commit; authority is published only after all required writes and fence removal succeed. | Native tests fail each settings write, the scoped write, and fence removal; test current-key binding and concurrent single-use redemption. |
| SEC-03 | `github_pull::merge_checked` checks current method authority after GitHub lookup and immediately before the merge command. Remote task creation carries request provenance through its spawned task; clone and other effects recheck current authority. | A channel-controlled delayed lookup tests Full→Control/Pending/Blocked, Full OFF, Host OFF, key rotation and lockdown, asserting zero merge effects. Clone supervision tests denial before spawn and revocation during progress. |
| SEC-04 | Settings and approval prompts display the complete grouped fingerprint used by native approval. | `remote-security.spec.ts` asserts the displayed value and exact approval argument; `remote-pairing.spec.ts` checks the complete local fingerprint. Existing native stale/malformed/missing-key tests remain required. |
| SEC-05 | Diagnostic redaction removes complete filesystem paths before token replacement can fragment them. Quoted paths retain no private suffix; unquoted paths conservatively redact through the line end. | `complete_home_paths_are_removed_from_logs_ipc_and_export` covers HOME, tilde, Windows, UNC, file URLs, spaces, Unicode, commas and semicolons across logs, IPC and export. |
| SCHED-AUTH-01 | Native admission resolves the occurrence's current policy under the same agents/store locks that capture `RunConfig` and claim the session. A shared session mode cannot supply another occurrence's authority. | Native tests use the real admission and concurrent barriers to bind each initiating prompt to its own effective mode. |
| SCHED-AUTH-02 | Schema 32 adds schedule-owned authorization outside the bounded activity log. Known original ceilings are migrated; bot-created, unknown/malformed legacy and unexpectedly missing policies use Plan. | Native tests prune the real activity log, migrate/reopen storage, and challenge a formerly Plan schedule after the bot becomes Full and has a broad allow rule. |
| FUNC-01 | The session claim, initiating message, occurrence receipt, advancement and delivered brief commit atomically. Busy or failed admission leaves work due. Run Now has a separate explicit request identity. | Native tests cover once, recurring, heartbeat, Run Now, busy/retry, duplicate admission, restart and write failures at each transaction boundary. |
| FUNC-02 | A selected unavailable remote computer remains selected and visible; sending remains denied until the user chooses a valid target. | `remote-connect-ux.spec.ts` covers disappearance/revocation and prevents silent local fallback. |
| FUNC-03 | Presence refresh updates signed-in/readiness state and rejects stale refresh results. Offline recovery is reachable from the composer. | Remote connection suites cover offline/retry/reconnect while preserving target and composer state. |
| FUNC-04 | A failed device-list request displays its error and retry state without simultaneously claiming there are no devices. | Device-list failure/retry browser case uses one controlled failed request. |
| UI-01 | Search-result highlighting treats supported patterns as literals; arbitrary regex is never run against result text in the renderer. Unsupported regex syntax still displays escaped results without highlighting. | A subprocess with a five-second timeout challenges pathological backtracking patterns; literal and escaped-metacharacter cases remain covered. |
| UI-02 | Raw tool input/output starts closed, including tools without a structured card. Manual disclosure remains the reader's choice across updates. | Browser cases exercise Skill/unknown tools by keyboard and preserve manually opened raw details after later results. |
| UI-03 | Active trace animations respond immediately when reduced motion is enabled or the document becomes hidden. Cleanup applies the final open/closed state. | Browser assertions capture the first preference-change frame, preventing natural animation completion from satisfying the test accidentally. |
| UI-04 | Unchanged historical content is retained during handoff, and composer refresh does not steal a non-collapsed transcript selection. | Browser cases check selected text, node identity and transient disconnection with MutationObserver, including historical content and a delayed history fetch. |
| TEST-01 | The canonical Playwright configuration collects all required Connect and agent-trace suites already named by CI. | A unit test runs Playwright's real discovery and requires tests from each previously omitted file. |
| TEST-02 | Mobile replay includes the missing cursor and replay-following regressions. | `mobile/tests/replay-pagination.test.ts` exercises four upstream scenarios using the real replay helpers. |
| TEST-03 | Preview hooks now model device fetch failures, held/rejected remote sends, and resubscription. The stale-identity browser action is a single submission, without checkbox retries silently approving a newly rendered identity. | Existing refusal/recovery assertions remain; failure counts and accessibility selectors match the actual invoked request and dialog. |
| DOC-01 | Current version guidance, README and upstream history identify the 0.6.0 candidate; historical release evidence remains intact. | Build/release-contract checks must continue to validate manifest and display identity consistency. |
| GOV-01 | Ready for review reflects the owner's explicit instruction and opens review only. It is separate from technical acceptance, issue completion and release authorization. | The integration record and release note retain the unresolved physical-device/platform gates. No merge, tag or release is performed. |

## Additional challenge-pass corrections

Independent review of the first implementation commits identified additional cases that belong to the same requirements:

- **Pause versus admission:** pause/reset/delete must close new admission before awaiting shutdown of existing runs. Checking only before a pause is insufficient. Both local and remote callers use the same native boundary.
- **Already-pruned legacy policy:** migration must not convert an unknown former Plan ceiling to Ask and then silently apply an existing allow rule. Unknown authorization is migrated conservatively to Plan.
- **Path punctuation:** commas and semicolons may be part of a quoted filename. Treating them as unconditional field separators leaked the remainder of the path.
- **Cached pairing responses:** a successful cached response must not advertise a previous Full grant after downgrade. The response's declared scope is checked against current authority before transmission.
- **Reader state in nested history:** stable top-level articles alone are insufficient if an expanded historical trace ancestor is removed. Historical narrative/raw details need the same reader-state guarantee.
- **Owned harness consoles:** pausing a bot also closes consoles belonging to its persisted bot conversations. Free shells and unrelated tasks remain available. Registry removal is authorized and locked; process teardown happens after releasing the registry lock. A Unix regression uses real PTYs and independent sentinel sessions.
- **Live tool interaction:** updating one tool must not detach an unchanged sibling between pointer down and pointer up. A browser regression checks node identity, transient removal and the completed click. Replay tests pause simulated time between explicit advances so interaction phases do not race uncontrolled fixture events.

## Persistence and recovery semantics

Schema 32 adds `bot_schedule_authorizations` and `bot_scheduled_runs` through the existing transactional migration path. Activity retention cannot remove a schedule's original ceiling. Existing known user authorizations remain usable within the current bot policy. Unknown authorization does not become more permissive during migration.

Scheduled work has **atomic, idempotent acceptance**, not a claim of exactly-once effects in external services. A process interrupted after acceptance leaves an interrupted persisted turn. It is not automatically submitted again, because the process cannot know whether an external effect already happened. Work rejected before acceptance remains eligible for retry. Run Now does not consume the next timer occurrence or reactivate a completed once schedule.

Connect permission writes report errors. If a durable fence was written, restart remains denied even when later writes fail. If all persistent storage is unavailable and even the fence cannot be written, the process denies access in memory and returns failure; it must not report durable revocation as successful. Recovery requires restoring writable storage and completing local sign-out. This condition is distinct from a successful persistent revoke.

## Verification

Validation rounds are separated by at least 120 seconds. Failed runs are retained as diagnostic evidence, then the relevant checks are repeated after correction. Tests are not disabled, skipped, or weakened to manufacture success.

Authoritative commands come from [CI](../.github/workflows/ci.yml):

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
npx playwright test
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
python security-review/hardened.3/check-slack-fixtures.py .
```

The browser CI separates chat repetition, desktop integration and historical regressions. The canonical configuration now includes the required security, pairing, diagnostics, Connect UX and agent-trace suites; collecting a filename is not treated as a passing browser execution.

## Remaining acceptance boundaries

Automated tests and Linux native checks do not demonstrate physical two-device behavior. Real LAN/Tailscale and relay transitions, OS-specific key storage, active-session revocation across separate machines, and supported macOS/Windows native behavior require their own evidence. These gaps remain open even when all GitHub Actions jobs are green. Full upstream parity, release readiness and closure of the original issues are not claimed by this correction record.
