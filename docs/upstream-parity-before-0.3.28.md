# Historical feature coverage before PandaCode 0.3.28

This supplement reviews **77 original source notes** before `0.3.28` for the unpublished **PandaCode Extended 0.5.0** candidate. At the audited baseline it identifies one compatible responsiveness gap: Jev's protected-store hydration runs synchronously during native setup. [Issue #26](https://github.com/ginmp8/pandacode-extended/issues/26) defines the bounded correction and its frozen acceptance criteria. The remaining reviewed feature families are inherited, have later replacements, or have explicit security exclusions.

Technical version **0.5.0**, display identity **0.5.0+pc.0.3.47**, historical source ancestry **0.3.28**, and the newer [0.3.28–0.3.42](upstream-parity-0.3.28-0.3.42.md) and [0.3.42–0.3.47](upstream-integration-0.5.0.md) evidence remain unchanged. This is a compatibility review with deliberate differences, not a complete rebase or unrestricted parity claim.

## Source identities and evidence limits

| Input | Verified identity |
| --- | --- |
| Requested base branch | `feat/0.5.0-upstream-0.3.47` |
| Frozen Extended baseline | `68e4d0615ec864a2dd9e9630e0e3410432e27207` |
| Exact original 0.3.27 snapshot | `612ca79c8e7db74461040bc27ddf1a3dcebb32ab` |
| Existing original 0.3.28 ancestry | `70bdc708303c8681d8eb0b46f2cdb0ff087aa6eb` |
| Source-note range | 49 notes `0.2.0–0.2.48`; 28 notes `0.3.0–0.3.27` |
| Current published releases in this range | 28; all complete source notes match the published body followed by its generated installation appendix |
| Evidence manifest | [upstream-release-evidence-before-0.3.28.json](upstream-release-evidence-before-0.3.28.json) |
| Manifest SHA-256 (LF) | `4a9d40bf2b695b0c533fc15eaac026878603d24d712693fcc43b10f835d44720` |

The official public release inventory was exhausted (47 records at observation), and official tag refs were checked. No current public `0.2.*` release/tag records were returned. The 49 older notes are source evidence retained in the exact original snapshot; this does not establish their original change commits or publication dates. The audit does not invent releases before the earliest available `0.2.0` note.

All 77 source notes match the Extended baseline byte-for-byte. Each feature was additionally checked against an active implementation/caller, later source, policy and available regression contracts. Unchanged notes or helper-only tests do not by themselves establish working user-facing behavior. The map explicitly records the colocated Jujutsu badge's later upstream limitation rather than treating its unused helper as UI evidence.

**Path and test convention:** source paths and line references in the item maps refer to the frozen baseline above, unless another upstream version is named. Existing tests identify inspected contracts; they are not claims of new execution. New execution results belong in [the verification report](verification-pre-0.3.28-0.5.0.md). Historical speed measurements, native credential dialogs, live cloud/provider services, VMs/simulators and physical mobile devices are not recertified by source inspection.

## Included correction and preserved behavior

At the audited baseline, Jev enters `lib.rs` setup → `jev::setup` → `load_saved` → OS vault reads or legacy migration before the main Store opens. This source-confirmed dependency conflicts with the responsiveness objective in [0.2.4](https://github.com/aicodeking/pandacode/blob/612ca79c8e7db74461040bc27ddf1a3dcebb32ab/releases/0.2.4.md), even when both optional Jev switches are disabled but saved credentials exist. It is not a reproduced macOS permission-dialog measurement.

The correction registers disabled pending state, runs one detached initializer without holding Runtime across vault I/O, and publishes a complete result before settling a remembered asynchronous readiness latch. Configuration, Save, Doctor, explicit installation and execution wait before reading saved settings or taking admission locks. Ordinary boot, task interaction, history, cancellation and shutdown remain independent. A late startup configuration response must not override a newer successful settings save.

Credential storage stays protected. Omitted keys retain their existing values; explicit empty strings clear only the selected key. A failed read/migration preserves the recovery file, publishes no partial credentials and refuses destructive Save until recovery succeeds. Existing immutable vault generations keep the JSON file as their commit point. The change introduces no database migration, dependency, permission mode, background network call or new secret-bearing IPC.

Signed-in Connect has a separate inherited startup credential path. This bounded fix does not claim to eliminate every OS prompt or every application startup dependency. The old `0.2.5` blanket no-Keychain statement does not authorize plaintext storage.

## Exclusions and later replacements

| Behavior | Decision and retained alternative |
| --- | --- |
| Automatic app updates; agent update checks/native installers | Keep disabled under `AGENTS.md` and `SECURITY-HARDENING.md`; retain installed-tool discovery and local packaging |
| Daily network model-price refresh | Keep saved/bundled estimates and explicit unavailable states |
| Full-access-by-default from `0.2.16` | Keep Standard default, explicit elevated modes and separate remote Full-access authorization |
| Plaintext Jev credentials or disabling vault protection to avoid prompts | Keep OS-vault storage, atomic migration, recovery and explicit key-update semantics |
| Bare/unbound bot approvals or unsolicited scheduled external output | Keep request-bound approvals, explicit outbound consent, local summaries and revocation checks |
| Automatic VM desktop setup | Keep explicit Add a desktop |
| Credential-bearing deployment downloads and `npx` network bootstrap | Require installed tools, normal approvals and the existing disabled/private installer package |
| Automatic cloud-agent runtime/login/checkout upload | Keep excluded; manual approval-gated cloud adapters remain |
| Older spring/glass/sidebar/bot layouts | Retain later source and the current Extended interface |
| Colocated Git/Jujutsu badge from `0.3.24` | Record upstream `0.3.45`'s declared absence in redesigned Changes; do not restore obsolete UI |
| Local macOS ad-hoc self-signing and explicit RTK/Android/Tart setup | Preserve; these are not covered by the blanket agent-updater exclusion |

Preserve Extended review/coordination, Advisor/Committee, QA, issue-action confirmation, Board controls, named connections, scoped hash-pinned Skills, task-owned attachments/history, scriptless viewers, Connect current-key pinning/revocation/replay controls, independent remote privileges and bot notification consent. The historical notes do not override these contracts.

## Classification

- **Preserved / P:** an active user/tool entry point reaches the implementation or its functional equivalent.
- **Superseded / S:** later source replaces the old surface or explicitly declares a limitation; the map names that replacement/limit.
- **Intentional exclusion / X:** a current policy requires the difference and retains a documented alternative.
- **Gap / G:** a specific compatible behavior needs correction; the Jev startup dependency is the sole identified item.
- **Uncertain / U:** evidence does not establish an original bug mechanism, measurement or live platform outcome; uncertainty alone is not a reason to invent a port.

## PandaCode 0.2.0–0.2.24

### 0.2.0

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Notification, installation and restart for new application versions when idle | **Policy exclusion** | `src/app-updates.ts` is an inert controller with `supported: false`, no preference reactivation, scheduling, download or restart. `src-tauri/src/lib.rs:3245` omits updater registration; `src-tauri/tauri.conf.json` has `createUpdaterArtifacts: false`. `AGENTS.md` and `SECURITY-HARDENING.md` explicitly forbid restoring this behavior. |
| Task menus remain stable when responses finish streaming | **Preserved** | `src/main.ts:835` preserves the task-action trigger/connected task-row menu in `closeContentMenus`; `renderTranscript` and `renderComposer` use it. `renderNavigation` retains menu-anchor rows around DOM morphing at `1055–1084`. Existing keyboard/task-menu scenarios are in `tests/ui.spec.ts:1243` and retained-row/streaming scenarios in `tests/thread-order.spec.ts`. |

### 0.2.1

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Improved release verification | **Preserved, adapted** | `scripts/release/build.mjs` verifies native macOS app signatures and DMG contents and writes SHA-256 sidecars; version/provenance checks remain in `scripts/release/check-hardened-sync.mjs`. Existing `desktop-build.test.mjs`, `common.test.mjs`, `extended-version.test.mjs` and sync corruption guards cover local packaging contracts. This is not a claim that a native installer was built or verified in this audit. |
| Automated macOS preview delivery | **Policy exclusion** | `docs/releasing.md` explicitly removes automatic GitHub/npm publication and retains only operator-invoked local packaging. The updater feed and npx bootstrap remain disabled. Do not restore an upstream publishing workflow to satisfy an old maintenance note. |

### 0.2.2

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Fix a crash on a fresh launch | **Uncertainty about the historical mechanism; not evidence of a gap** | The entire note is one sentence and no exact 0.2.2 change commit/public tag is available in the verified inventory. Extended descends from later 0.3.28 source and has current startup/storage code plus `scripts/release/ci-startup-smoke.mjs` and its existing unit test. It would be unsupported to invent the old cause or claim a fresh-launch regression without a reproducer. |

### 0.2.3

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Search installed OpenCode models in Settings → Agents, save choices and use them in conversation, Board, launcher and team setup | **Preserved** | `src/opencode-model-settings.ts` mounts searchable/paged choices and calls `list_opencode_models`/`save_opencode_models`; `src/main.ts:4071,4124` wires Settings. `provider_settings.rs:436–510` persists validated IDs. Main and `src/overlay-model-picker.ts` invalidate saved catalogs; team setup uses the shared picker. `tests/opencode-models.spec.ts` exercises selection, errors, restart, main/launcher cache invalidation and a 2,012-model fixture. |
| Large installed catalogs must not fall back to Default; preserve existing task models and selections after restart | **Preserved** | `src-tauri/src/providers.rs:66` defines `MODEL_CATALOG_LIMIT = 10_000`; discovery and selection flattening share it in `providers/acp.rs`. The picker retains unavailable selected IDs instead of deleting them. Existing native tests include `opencode_selects_later_models_from_large_flat_and_grouped_catalogs`, `acp_catalog_limit_bounds_the_flattened_total` and `legacy_acp_catalog_keeps_later_models`; browser cases cover the saved current model. |

### 0.2.4

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Remain responsive while macOS checks Keychain permissions instead of waiting to open | **Preserved for ordinary provider configuration; compatible gap for configured Jev startup** | `provider_settings::setup` reads only nonsecret JSON; settings credential inspection is the explicit async `provider_configuration` command, using `crate::blocking`. In contrast, `lib.rs:3261 → jev::setup → load_saved → secret_store::get` is synchronous at the frozen baseline before `AppState` and normal startup finish. See the included correction above and the verification report; a real macOS permission-dialog result is not claimed. |
| Explain the macOS password prompt; ad-hoc previews can still trigger permission prompts | **Superseded/platform limitation** | Current provider errors advise unlocking the login Keychain, while `docs/upstream-parity-0.3.28-0.3.42.md:34` explicitly does not promise elimination of OS credential prompts. Current protected-storage policy must remain. Exact old explanatory UI cannot be attributed to a known 0.2.4 source patch, and the old automatic-preview update path is disabled. |

### 0.2.5

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| No macOS Keychain access at startup, including after updates | **Superseded blanket promise; protected storage is a policy requirement** | Fresh nonsecret provider setup is lazy, but exact original 0.3.27 already calls `remote::setup`, which reads Keychain only for a signed-in Connect account. Current `docs/remote-access.md:89` documents that startup behavior. Extended additionally secures Jev secrets in the vault; reverting them to upstream plaintext is expressly excluded. The separate Jev startup-responsiveness seam is not resolved by this classification. |
| Preserve tasks, provider settings and local history | **Preserved** | `provider_settings::setup`, the retained bundle/database identity in `lib.rs`, transactional storage migrations and `storage.rs` reopen/migration tests preserve data. Current schema 29 adds to the existing history; it does not reset it. |

### 0.2.6

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Compact composer popover with saved profile, architect/worker models and tucked run limits | **Preserved; newer automatic and named-team modes coexist** | `src/architect.ts:50,144–225` provides the anchored setup, model choices, profiles and expandable limits. `src/team-mode.ts` and `main.ts` retain the classic setup while exposing newer team modes. Existing `tests/architect.spec.ts` and native profile tests cover retained setup/defaults. |
| Cross-harness direct messages, Workers → Team messages, durable checkpoints with work/next steps/blocker | **Preserved** | `architect/coordination.rs` exposes `team_directory`, `send_team_message`, `read_team_mailbox` and `checkpoint_worker`, with ownership/attempt checks and durable storage. `src/architect.ts` renders messages and checkpoints. Existing UI cases at `tests/architect.spec.ts:221,251` and native tests at `architect/tests.rs:456,539,620,663` check scope, boundedness and restoration. |
| Messages/blockers wake an idle architect within the round budget without weakening concurrency, file scopes, approvals, cancellation or retries | **Preserved** | The coordination revision/pending-turn path in `architect.rs:1709` performs event-driven wakeups; scheduling and generation checks remain. Existing tests include `checkpoints_notify_idle_architect_once_without_interrupting_workers_or_exhausting_review`, `workers_cannot_control_teams_spoof_senders_or_write_after_cancel_pause_or_retry`, and `round_and_attempt_limits_stop_automatic_spending_until_explicit_action`. |
| Preserve saved profiles; transfer Codex context once on the next explicit run; updates do not auto-start agents | **Preserved** | `architect.rs:375–385` handles the tool-catalog version and Codex recovery marker; start/resume controls perform the transfer. Existing tests include `collaboration_migration_refreshes_only_codex_team_catalogs_and_retains_private_history`, `persisted_messages_and_checkpoint_survive_restart_without_auto_resuming`, and `architect/profiles/tests.rs:94,631,716`. |

### 0.2.7

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Show architect name plus saved worker-model count across conversation, Board and launcher, keeping the count visible with long names and removing it for single-agent mode | **Preserved; extended for automatic/named teams** | `main.ts:756–774` emits `+N model(s)` for classic teams, a model count/name for roster teams and the Team toggle state for automatic teams. Shared composer is used by conversation/Board; launcher has its own count. `tests/overlay.spec.ts:84` explicitly checks +2/+1 counts, long labels and selection changes. Automatic-team suppression is deliberate in `tests/architect.spec.ts:355`. |

### 0.2.8

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Compact live worker summary with assignment, role, harness/model, state and opening the worker's live thread/saved changes in conversation and Board | **Preserved** | `main.ts` places worker progress into turn slots through `updateTeamProgress`; `src/architect.ts` and team-progress rendering retain worker identity/status/details. Existing cases `tests/architect.spec.ts:408,429,443,758,857` and `tests/turn-changes.spec.ts:265` cover ordering, narrow layout, history and worker ownership. |
| Actual file changes/capture errors remain visible | **Preserved for attributed changes; empty-warning presentation superseded by 0.2.24** | `src/turn-changes.ts` keeps partial/error notices on real attributed changes; empty/unowned cards are deliberately omitted. `tests/turn-changes.spec.ts:165,186,197,227` distinguishes empty captures, incomplete real diffs and unavailable states. Do not recreate empty warnings that the later release explicitly removed. |

### 0.2.9

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Render internal coordination as compact Team status while retaining full agent context; preserve user JSON in existing/forked conversation, Board, worker and launcher views | **Preserved** | `architect/internal_turns.rs` encodes/identifies app-origin coordination. `src/coordination-message.ts` only renders typed coordination rows. Native tests `generated_review_keeps_model_input_but_projects_a_timed_coordination_turn`, `generated_worker_notification_is_typed_but_user_json_and_copies_are_unchanged` and legacy migration tests guard ownership. Browser coverage is `tests/architect.spec.ts:460,696` and `tests/overlay.spec.ts:470`. |

### 0.2.10

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Less memory for long saved replies without altering stored text or provider context | **Preserved** | `storage.rs` uses bounded assistant previews and bounded transcript pages; `assistant_previews_bound_allocation_without_mutating_history` at 3438 verifies byte capacity, Unicode/NUL handling and unchanged full database content; `transcript_pages_have_a_byte_budget_and_do_not_skip_history` covers paging. Full message copying remains separate. |
| Avoid redrawing unchanged launcher conversations while updating text, status and recovery | **Preserved** | `src/overlay-thread.ts:109–150` uses history revisions plus live text/status/provider/notice checks and repaints recovery. `tests/overlay.spec.ts:615,676` retain zero-DOM-mutation, live/status/coordination and real Retry recovery oracles. These are existing tests, not a new run for this audit. |

### 0.2.11

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Optional Jev settings, independent assistance/browser switches off by default, app usable without keys | **Preserved** | `jev.rs:52–53` defaults both off; `src/jev.ts:148` renders separate switches; `main.ts:176` catches optional metadata failure. `tests/jev-assist.spec.ts:63` and `tests/jev.spec.ts:88` cover opt-in/no-key behavior. The configured-vault startup correction is listed separately above. |
| Suggest configured workers, find recent context, review saved work and project rules; preview evidence, store local reports, no automatic model/assignment/completion changes | **Preserved, enhanced with explicit OpenRouter selection** | `src/jev-assist.ts` exposes route/context/review/rules; `jev_decisions.rs:977–1009` supplies prepare/run/list/save-rules. Preparation fingerprints, bounded evidence, task/project rechecks and local reports remain. Existing assistance tests at 82,113,133,153,183,199 plus native bounded-input/report tests cover the contract. |
| Isolated local browser tests, actions/screenshots/final checks; separate runtime and own credentials | **Preserved** | `jev.rs` owns bounded isolated runs, reports, screenshots and cancellation; `src/jev.ts` renders reports and test controls. Installation is explicit through a user action, not startup installation; the existing pinned runtime installer sanitizes its environment. `tests/jev.spec.ts:442,468,493,530` cover observed evidence, cancellation and errors. Paid live-provider outcomes remain unverified here. |
| Project-file @ mentions; copy original messages/responses and fork a saved prefix, including Board/launcher | **Preserved** | `src/file-mentions.ts`, `src/message-actions.ts`, `task_controls.rs` and overlay IPC provide the operations. Existing `tests/file-mentions.spec.ts`, `tests/message-actions.spec.ts:138–274`, and `tests/overlay.spec.ts:27,138–198` retain exact text, ownership, keyboard, stale-result and explicit-send boundaries. |
| General desktop compatibility/process improvements | **Uncertainty about the unspecific historical change; current mechanisms present** | The note does not identify the old patch. Current native process groups, `providers/windows_job.rs`, bounded cancellation/output and launch resolution remain; existing concurrent-process tests cover isolation. No platform regression follows merely from this vague line. |
| Apple-silicon-only downloads and advisory preview/billing caveats | **Distribution note / retained scope, not a missing feature** | The current Extended local packaging policy replaces upstream publication claims. Jev remains optional and advisory; no live provider/billing/native platform validation is claimed by this read-only audit. |

### 0.2.12

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Allow valid models late in large OpenCode flat/grouped catalogs; discovery and launch share a bound; reject unsupported model/effort | **Preserved** | `providers.rs:66` and `providers/acp.rs:152–159,756,853` use the shared 10,000-model bound. Native tests at `acp.rs:1471,1512,1528` explicitly exercise later/grouped entries and total bounds. Current task/saved-model persistence remains in `provider_settings.rs`; existing OpenCode browser scenarios cover selections. |
| Preserve saved settings; Apple-silicon release scope | **Preserved data; historical distribution scope** | No settings reset was found. Do not infer platform downloads for Extended from the original note. |

### 0.2.13

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Live isolated Chrome view/action history in sidebar; goal/preset popover; stop, return to browser, reopen reports | **Preserved** | `src/jev-live.ts` implements `watch`, `refresh`, `leave`, `close` and selected-run cancellation; `src/jev.ts` supplies anchored test/report controls. `tests/jev-live.spec.ts:89,174,215,229` and `tests/jev.spec.ts:175,480,493` cover those flows and exact task/run selection. |
| Private bounded local frames, capture only while watched, cleanup/expiry, no extra inference merely from watching | **Preserved** | `jev.rs` caps frames at 256 KiB, implements task-bound `jev_watch_run`, watch generation/TTL and cleanup; `jev-live.ts` releases/renews leases and ignores stale frames. Native `live_watch_is_scoped_expires_and_stops_without_persisting_frames` and `live_frames_are_bounded_digest_checked_and_have_fixed_dimensions`, plus browser cases at 244,259,286, cover the contract. Watch requests do not call the inference runner. |
| Optional runtime/keys; signed updater/Apple preview boilerplate | **Preserved optionality; updater publication excluded** | The live view does not remove opt-in or introduce an automatic runtime download. Updater claims do not apply to Extended. |

### 0.2.14

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Fit large text/long servers; quiet controls/info explanations; Run remains visible while options scroll; expandable servers, hidden empty history | **Preserved** | `src/jev.ts` popover and `src/jev.css` use bounded sections/footer/overflow controls. Existing `tests/jev.spec.ts:255,328,359` exercise long lists, enlarged short windows and accessible explanations. |
| Translucent blur material | **Superseded** | Both current source and exact original 0.3.27 have final overrides in `src/styles.css:881–889` that intentionally make popovers and modal surfaces solid. Reinstating the earlier material would undo a later design. `tests/compact-menus.spec.ts:113` explicitly expects the solid project menu. |
| Keyboard, validation and reduced-transparency/contrast behavior | **Preserved** | Jev field validation, focus/dismissal handling and accessibility CSS remain; `tests/jev.spec.ts:198,233,359,411,530` retain relevant scenarios. The presence of older visual assertions is not a claim that all were rerun now. |
| Signing/update preview boilerplate | **Historical platform note / policy exclusion for updates** | Apply current Extended packaging and updater policy. |

### 0.2.15

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Translucent worker/subagent panels with accessible solid alternatives | **Superseded visual material; accessible panels preserved** | `src/architect.css:80,134` and `src/subagents.css:9` use the same solid surfaces as exact original 0.3.27. Existing contrast rules and readable bounded panel layouts remain. This is not an unported historical feature. |
| Opening Browser with the left sidebar hidden must not collapse the conversation; retain resized widths across view/text changes | **Preserved** | Current sidebar/inspector state/layout logic separates visibility and width; browser guest bounds follow actual inspector geometry. `tests/layout-preferences.spec.ts:384,425,484,559,651` and `tests/browser.spec.ts:300` exercise collapse/Board/text-scale/resizing and native guest bounds. |
| Signing/update preview boilerplate | **Historical platform note / policy exclusion for updates** | No Extended installer or updater behavior is certified by the old note. |

### 0.2.16

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Full access is the default for new tasks | **Policy exclusion** | `docs/design.md:48` explicitly requires Standard approvals and deliberate elevated-mode selection. `storage.rs:23` is `DEFAULT_PERMISSION_MODE = "ask"`; main create paths and launcher preserve it. `tests/ui.spec.ts:1365`, `architect/profiles/tests.rs:132` and `overlay.rs:1742` assert the safer default. Do not change it to claim historical parity. |
| Preserve chosen mode on harness/profile changes; workers/forks inherit parent choice | **Preserved** | Native profile/task configuration and fork/worker creation carry the selected mode; later explicit review-mode support refuses unsupported providers rather than silently widening it. Profile tests, UI permission-persistence cases and worker revocation tests cover this boundary. |
| macOS drag-to-Applications DMG | **Preserved local build capability** | `src-tauri/tauri.conf.json` retains DMG window/app/Application-folder positions; `scripts/release/common.mjs` uses `app,dmg` for native Darwin builds. `build.mjs` verifies the resulting bundle when explicitly run. No actual macOS installer was produced in this audit. |
| Cryptographically verified automatic updates; original ad-hoc/not-notarized status | **Policy exclusion / historical publication detail** | Updates remain disabled. Current signing/notarization claims require an actual explicit native build; do not copy original release claims to Extended. |

### 0.2.17

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Left-side support button remains available with hidden sidebar or Board | **Preserved** | `main.ts:304` defines one button; `renderSidebarVisibility` at 1824–1848 moves the same element into the header for hidden/full-width views. `tests/support.spec.ts` covers keyboard-only request, DOM identity, duplicate-open prevention, failure and narrow large-text layouts. |
| Help & support settings; explicitly copy version/platform/architecture | **Preserved** | `src/settings.ts:111`, `src-tauri/src/support.rs:20` expose the allowlisted three fields, without automatic transmission. `tests/settings.spec.ts:123,146` and native `support_has_no_identity_or_workspace_payload` retain the contract. Extended About links coexist with upstream attribution. |

### 0.2.18

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Warm glass and spring movement, reduced-motion/contrast fallbacks | **Superseded by later upstream/current design; accessibility preserved** | Exact original 0.3.27 and current source both set `MOTION = false` and make several surfaces solid. Current `AGENTS.md` also mandates instant UI, and global CSS disables transitions. Existing accessibility/contrast handling stays. Restoring the older springs/material would undo the later design. |
| Project/checkout/branch bar; attach worktree to an unstarted draft or create sibling task | **Preserved** | `src/worktrees.ts:68–117` provides context bar/menu and attach/create paths; `main.ts` connects draft/task ownership. `tests/worktrees.spec.ts:29,41` covers the context and retaining draft/title/selection. |
| Choose/reuse branch, recover missing checkout, safe cleanup of dirty/unmerged work | **Preserved** | Native `workspace.rs` implements branch/worktree lifecycle and refusal conditions. UI tests at 71,96,119,141,182 and native tests at 1682,1693,1728,1748,1803 exercise existing branches, conflicts, unsafe cleanup and recovery. |
| Optional signed-in gh issue import to Pending and push/open linked PR | **Preserved, extended** | `src/github.ts:173,250` wires imports/creation; native `github.rs` validates repositories/arguments. `tests/github.spec.ts:94,140` covers duplicate imports, dirty-tree refusal and linking. Extended issue-action approvals are additional behavior and must remain. |
| Linked issue/PR state on task/Board; disabling GitHub hides controls and stops CLI calls | **Preserved** | `githubEnabled` guards status, mentions, links, imports and Hub. `tests/github.spec.ts:57,174` and `tests/board.spec.ts:182,263` assert no GitHub calls while off. |
| Known issue-mention lookup limitation | **Superseded/fixed** | Current native issue/PR paths request their own appropriate fields; existing `tests/github.spec.ts:28` exercises mentions/context. An acknowledged old bug is not behavior to restore. |

### 0.2.19

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Repair missing worktree_branch on upgrade; preserve prior data; migration guard follows latest schema | **Preserved** | `storage.rs:168,210,329–331` uses current schema 29 with a conditional column repair. Existing regression at `storage.rs:2972` simulates the old version-17 missing-column database and retains tasks/messages; `schema_version_constant_matches_the_newest_migration_step` at 3039 guards the version. Later schema 28/29 migrations retain Extended tables and history. |

### 0.2.20

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Images/documents/folders from composer, image paste/drop | **Preserved, extended with durable history/drop fallback** | `src/attachments.ts` owns task-scoped chips, caps and restore-on-failure; native `prompt_attachments.rs` reads bounded data; `lib.rs:2630–2668` wires image/document/folder commands. `tests/attachments.spec.ts:40,83,102,118,143,168` covers selection, paste/drop, cap, sends, restoration and ownership. |
| Create/edit/send sketches | **Preserved** | `src/sketch.ts` and `attachments.ts:184` keep editable sketch IDs and replacement-in-place. `tests/sketch.spec.ts:40,84,119` covers drawing/editing/export/cancelled-draft behavior. |
| @ folders as well as files | **Preserved** | `src/file-mentions.ts:125–134` renders directory results and inserts paths; `tests/attachments.spec.ts:156` checks the trailing slash; mention tests preserve project/task scope. |
| Discover tasks hidden by project filter | **Preserved** | `main.ts:1060–1065` shows the count/Show all affordance outside the list. `tests/ui.spec.ts:1139` covers discovery and clearing the filter. |
| Stable Team button appearance before opening its selector | **Preserved/superseded by the current Team control** | Composer styling is eagerly imported by `main.ts` and current Team controls come from `team-mode.ts`; they do not depend on lazy loading of the setup panel. Automatic/named/classic modes coexist rather than removing teams. |
| macOS PDF/Office extraction; cross-platform images/text/folders/sketches | **Preserved with the same platform limit** | `prompt_attachments.rs:389–432` uses textutil/Spotlight on macOS and an explicit text-export error elsewhere; native extraction tests are target-gated. Later cross-platform document **viewers** are a separate feature, not evidence that attachment extraction became universal. No missing universal extraction should be invented from this note. |

### 0.2.21

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Compact single-line model/project/attachment/command rows; long help in accessible info tooltips | **Preserved** | `src/choice-menu.ts:130–136` retains inline brief help or screen-reader descriptions with info marks; model/mentions/slash components use compact rows. `tests/compact-menus.spec.ts:13,28,60` checks no wrapping, hover/keyboard tooltips and readable hidden hints. |
| Simpler Settings/headings/launcher shortcut controls; retain live GitHub state/worktree previews | **Preserved and evolved** | Current Settings organization and overlay settings remain; `tests/settings.spec.ts`, `tests/compact-menus.spec.ts:87`, `tests/github.spec.ts` and `tests/worktrees.spec.ts` cover headings, status and checkout preview flows. Later tab organization is not a gap. |
| Nested menus/tooltips do not replay dialog entrance | **Preserved; instant motion policy is stronger** | `src/motion.ts:392–401` excludes `.floating-menu`/`.tip-bubble` from content-arrival changes, and `MOTION = false` disables entrance motion globally. The queued-close/reopen lifecycle correction remains intact. |

### 0.2.22

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Grok Build discovery/effort/context usage/streaming/approvals/resume | **Preserved** | `providers.rs` registers Grok; `providers/acp.rs` configures cached-token auth, native permission flags, model/effort, resumed context and usage frames. Existing tests at `acp.rs:1614–1738` and `tests/grok.spec.ts:32,56` cover it. The note promises context usage, not subscription-limit reporting; current account-usage UI correctly reports unsupported when the CLI exposes none. |
| MiMo via Claude, Token Plan regions/separate API endpoint/key settings | **Preserved** | `providers/mimo.rs` exposes the documented cluster allowlist and isolated environment-only credentials; `provider_settings.rs` stores keys in the native vault. `tests/provider-settings.spec.ts:128,150` and native endpoint/key/isolation tests cover it. |
| Both across teams/profiles/launcher/terminal/forks/slash commands | **Preserved** | Central provider/architect/profile allowlists, launcher choices, task controls, native console adapters and slash registry include Grok/MiMo. Existing provider launch, team/profile and console tests retain selected permissions and connection scope. Later named-account restrictions are intentional safety boundaries rather than missing default-provider support. |
| Refuse Grok images/sketches until supported | **Preserved** | `attachments.ts` hides/refuses image/sketch paths for imageless harnesses; `acp.rs` refuses image input before prompting when the server lacks capability. `tests/grok.spec.ts:75` and `unsupported_images_mcp_effort_or_plan_fail_before_any_prompt` preserve rejection. |

### 0.2.23

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Remove four-agent task cap; unlimited parallel team option | **Preserved** | `architect.ts:216–225` represents unlimited as zero; native scheduler checks only positive `max_parallel` limits. `architect/tests.rs:1640` claims eight independent workers plus architect (nine agents) while keeping overlapping scopes queued; `explicit_team_parallel_limit_still_queues_independent_workers` preserves finite limits. No global four-agent admission guard was found. |
| Mark sent prompt Active immediately and retain after restart | **Preserved** | `storage.rs:337–345` stores/backfills prompt timestamps and maintains them by trigger; task admission persists the initiating message atomically. `tests/ui.spec.ts:2348,2379` and the schema-19 restart regression cover immediate/persisted activity. |
| Hide empty cards/blank space without hiding worker progress | **Preserved** | `src/turn-changes.ts:35–41` returns no empty/unowned card; `main.ts:1168` emits no wrapper when no card; worker slots are independent. `tests/turn-changes.spec.ts:165` and team progress/order tests cover it. |
| Explanations for failed/incomplete capture | **Preserved on real attributed changes; empty-warning behavior superseded by 0.2.24** | Notices remain in saved summaries and on real partial cards. `tests/turn-changes.spec.ts:186,197,227` deliberately distinguishes tracking failure from agent failure and omits empty warnings. |

### 0.2.24

| Release item | Classification | Current evidence and existing coverage |
| --- | --- | --- |
| Idle harness switch preserves conversation/project, bounded handoff, original historical provider labels | **Preserved, extended for named-connection safety** | `main.ts:3642 → configure_session`; `storage.rs:1271–1324` atomically queues context recovery, clears native context and writes handoff notice. `provider-handoff.ts`/message author resolution retain earlier labels. `docs/connection-session-safety.md` specifies 200 entries/64 KiB, no reasoning/tool state/image bytes. `tests/ui.spec.ts:3047`, message-action/launcher history tests and native rollback/author tests cover it. |
| Preview HTML/SVG/images/project references in built-in browser with source option | **Preserved, expanded with safe file tabs** | `main.ts`, `src/file-tabs.ts:88,174` and `src/browser.ts` retain explicit Preview in browser/source flows; newer file tabs handle many images/documents directly. `tests/browser.spec.ts` retains prepared-preview task/project calls. `file_preview.rs:23` uses scriptless/no-network CSP; restoring workspace-script execution is expressly excluded. |
| Local/cached slash commands immediately, provider discovery in background, loading/retry | **Preserved** | `slash_commands.rs:302` returns local/cached snapshots without launching a harness; discovery is separate and stale results cannot overwrite invalidation. `tests/slash-commands.spec.ts:142,153` covers delayed discovery, timeout, retry and preserved cached commands; native cache tests cover invalidation. |
| Windows Codex npm launcher and Antigravity path compatibility | **Preserved in source; native platform execution unverified here** | `providers/launch.rs` resolves a package-declared Node entry directly without a cmd shell and normalizes safe Windows namespace paths. Tests include `windows_npm_launches_node_entry_without_a_batch_shell`, unknown/outside-package rejection and `antigravity_new_and_resumed_sessions_use_ordinary_windows_cwd`. No affected Windows machine was used in this audit. |
| Longer Antigravity cold discovery, shared successful metadata and stale-list refresh | **Preserved** | `providers/acp.rs:6` has a 60-second discovery startup budget; `acp_catalog.rs` shares successful metadata with keyed locks and generation-safe invalidation. Native tests include `metadata_survives_missing_commands_after_slow_startup` and `refresh_cannot_be_repopulated_by_discovery_started_before_it`. |
| Distinguish installation from connection readiness; startup stages/actionable failure details | **Preserved** | `acp.rs` has an explicit connection check and stage status, `providers/diagnostics.rs` retains useful bounded/sanitized failure tails. `tests/settings.spec.ts:424` plus `readiness_distinguishes_startup_authentication_and_session_failures`, `startup_status_identifies_each_acknowledged_acp_stage` and diagnostics tests cover it. |
| Show Claude authentication source; prefer saved login over inherited credentials when explicitly chosen | **Preserved** | `providers/claude_auth.rs` implements the stored mode, sanitized source description and environment override removal for Claude only; `provider_settings.rs` persists the choice. `tests/provider-settings.spec.ts:3,61,89` and native source/preference tests preserve it without touching other providers. |
| Bound snapshot startup delay; no misleading warning without usable diff | **Preserved, hardened for task attribution** | `turn_changes.rs:34–35,174` bounds scanning/startup wait; `src/turn-changes.ts` shows only actual attributed changes. `tests/turn-changes.spec.ts:165,186,197` separates empty/partial captures and keeps real diagnostics. Current ownership checks must remain. |
| Explicit Windows/Antigravity uncertainty and bounded transfer caveat | **Retained validation limit** | Source coverage is not affected-machine verification. No paid/authenticated provider test or native Windows/macOS smoke test ran in this audit. |

## PandaCode 0.2.25–0.2.48

| Source note | Features and disposition | Active implementation and inspected regression contracts |
| --- | --- | --- |
| **0.2.25** | **Preserved:** on-demand account usage/reset windows, explicit unavailable states, evolving automatic titles that never override manual titles, groups of at most five tools with narrative boundaries, preserved expanded details during streaming, current-activity status, and the non-overlapping slash-menu browser visibility fix. | `main.ts` opens `mountAgentUsage()` and wraps task admission with `session_titles::with_host`; `agent-usage.ts` loads account limits only on demand; `session_titles.rs` stages eligible suggestions and commits them only after a successful matching turn; `tool-activity.ts::groupToolRuns` caps groups at five and is used by main and overlay transcripts. `browser.ts` checks actual menu overlap. Contracts: `tests/agent-usage.spec.ts`, `tests/ui.spec.ts` tool expansion/status cases, `tests/overlay.spec.ts` interleaved narration, `tests/browser.spec.ts` slash-menu visibility, and native title tests. |
| **0.2.26** | **Preserved:** per-task inspector state and browser URL, safe task/worker-history deletion, and stale file/browser/screenshot ownership guards. **Intentional exclusion:** agent update notifications, release polling and in-app agent installation. Read-only installed-harness discovery and manual external installation guidance remain. | `main.ts` captures/restores task panel state and carries screenshot ownership; `task_deletion.rs` serializes deletion against agents, consoles and team activity, deletes only the owned session set and does not delete project/worktree files. `agent-updates.ts` is an inert controller; native update commands call `require_updates_enabled()` and refuse. Contracts: `tests/thread-panels.spec.ts`, `tests/thread-deletion.spec.ts`, `tests/browser.spec.ts`, `tests/agent-updates.spec.ts`, and native `task_deletion` tests. Policy: `AGENTS.md` and `SECURITY-HARDENING.md`, “Agent updates.” |
| **0.2.27** | **Preserved:** durable pins/order and keyboard reordering, task-owned multiple terminal tabs/processes, safe minimize/close choices, static appearance import from the newest supported configuration, history paging without losing reading position, restored browser visibility, retained terminal startup output, and restart of exited harness consoles. **Superseded:** the original Active/Inactive organization now uses the newer project/status sidebar while retaining pins, activity and order. | `thread-order.ts`, `storage.rs::pin_session/reorder_sessions`, `TerminalPanel` groups keyed by task, terminal event buffering, and `terminal_preferences.rs` parsers are active. The preference loader chooses the latest usable bounded file and never evaluates Lua/configuration code; app zoom owns font size. Main, worker and overlay history use bounded pages and transcript anchors. Contracts: `tests/thread-order.spec.ts`, `tests/terminal.spec.ts`, `tests/ui.spec.ts` paging, `tests/overlay.spec.ts` paging, `tests/thread-panels.spec.ts`, and native terminal preference tests. |
| **0.2.28** | **Preserved:** stable Claude title-tool identity across review/resume, including a safe inactive no-op, and restoration of prepared HTML/SVG previews with query/fragment/directory navigation. Private preview authorization remains intact. | `session_titles.rs::with_host` always includes `pandacode_title`; eligibility affects only the suggestion handler. Task/team turns use the same admitted run path in `lib.rs`. `browser.rs::browser_open` calls `FilePreviews::navigation_url`, which restores access only for the owned preview origin. Native contracts include inactive title host behavior, `restored_navigation_only_bootstraps_owned_preview_origins`, and `restored_directory_navigation_bootstraps_without_exposing_private_files`; browser task-restoration tests cover callers. |
| **0.2.29** | **Preserved:** changing saved team setups during a run, replacing queued choices, keeping workers' own selections/conversations, changing the lead with a saved-history handoff, durable queued setup, and applying it at the architect-turn boundary without losing pending work. The repeated 0.2.28 fixes are also preserved. | `architect.rs` serializes `pending_setup`, queues changes while the architect is admitted, applies them at controlled turn/resume/scheduling boundaries, and leaves immutable worker selections intact. `profiles.rs` applies the lead selection and context handoff transactionally. Contracts: `tests/team-profiles.spec.ts`; native `used_teams_switch_setup_in_place_and_composer_can_change_the_architect_model`, `pending_team_setup_survives_restart_and_applies_before_explicit_resume`, and configure/review race cases in `architect/tests.rs`. |
| **0.2.30** | **Preserved:** Codex daily activity calendar, totals/active days, exact day readout, single keyboard stop with day/week arrows and Home/End, `/usage daily` focus, and narrow-window scrolling to recent weeks. | `main.ts` maps `/usage daily` to `initialSection: 'daily'`; `command-center.ts` mounts `usageHeatmap` and focuses it. `usage-heatmap.ts` provides the listbox, exact count labels, keyboard increments and responsive scroller. The newer Agent usage panel also reuses the calendar. Contracts: `tests/command-center.spec.ts` checks calendar totals, day selection and keyboard movement; `tests/usage-ledger.test.mjs` checks calendar data sources. The heatmap and command-center implementations are byte-identical to the original 0.3.27 comparison source. |
| **0.2.31** | **Preserved:** header/turn-card project Run, task-owned Run terminal and declared local preview, task/worker-attributed immutable diffs, resizable remembered right sidebar, bounded imported terminal appearance, parallel/dependency/file-conflict-aware worker scheduling, read-only worker coexistence, Codex team-tool catalog refresh with history handoff, and the daily calendar. | `ProjectRunner` is invoked by header, change cards and agent UI actions; `lib.rs` registers inspect/start/preview commands. `turn_changes::Tracker` is wrapped into the admitted tool host and persists attributed snapshots. `sidebar-resize.ts` owns saved dimensions. `architect.rs` performs eligibility/admission and refreshes stale Codex catalogs using `context_recovery`. Contracts: `tests/run-project.spec.ts`, `tests/turn-changes.spec.ts`, `tests/layout-preferences.spec.ts`, native turn-change ownership/immutability tests, and scheduler/catalog migration tests. |
| **0.2.32** | **Preserved:** all repeated 0.2.31 features plus consistent run-script preview validation and native Windows Bash discovery. | `run_script.rs` validates the declared preview and regular bounded script against the authorized checkout, rejects unsafe paths, and discovers native Windows Bash explicitly. Contracts include `windows_uses_native_bash_with_literal_script_arguments_or_explains_requirement`, `inspection_rejects_unsafe_or_ambiguous_preview_destinations`, and `scripts_and_previews_cannot_follow_symlinks_outside_the_workspace`. The implementation is byte-identical to original 0.3.27. |
| **0.2.33** | **Preserved:** usage Overview, week/month/year ranges, average peak account limit, per-agent/model/day cost breakdowns, daily cost graph, token/cache totals, per-agent calendars, local Claude/Codex token-log reading and incremental caching, and explicit unpriced/unavailable states. **Intentional exclusion:** daily network refresh of LiteLLM pricing; saved or bundled prices remain. | `agent-usage.ts`, `usage-overview.ts`, `usage-cost-chart.ts` and `usage-ledger.ts` render the overview and tabs. Native `usage_ledger.rs` reads only complete appended log lines, caches offsets/counts and is called on opening/refresh. `usage_ledger/pricing.rs` explicitly performs no network refresh. Contracts: `tests/agent-usage.spec.ts`, `tests/usage-ledger.test.mjs`, native ledger parser/cache tests and `missing_or_stale_prices_never_trigger_a_network_refresh`. Policy: `SECURITY-HARDENING.md`, “Prices and DNS.” |
| **0.2.34** | **Preserved/equivalent:** compact one-line project task rows, most-recent project ordering, six-row folder limit, remembered collapse with selected-task visibility, pins, activity styling, and within-project keyboard/drag order. **Superseded:** later review/status grouping supplements the old purely time-based organization; it must remain. | `main.ts` uses `folderGroups`, `shownTasks`, `FOLDER_LIMIT = 6`, activity metadata and the newer `sidebarSections`. `sidebar-tree.ts` preserves recent-first grouping and saved collapsed IDs; `thread-order.ts` confines reordering to the visible project/pin group. Contracts: `tests/sidebar-tree.test.mjs`, `tests/sidebar-sections.spec.ts`, `tests/thread-order.spec.ts`, and the project-folder/activity cases in `tests/ui.spec.ts`. |
| **0.2.35** | **Preserved:** Claude/Codex conversation search/import, project resolution, original timestamps/native resume identity, direct links to already-imported tasks, lazy task publication, discard of untouched drafts, retention/reuse of drafts with text/attachments/worktrees, and the single-project twenty-row sidebar. | `conversation-import.ts` is reachable from sidebar and Go to anything; native imports validate the recorded folder, reuse/create its project, deduplicate imported identity and call `Store::import_task` with original history times and resume ID. `main.ts::discardEmptyDraft` checks typed text, all attachment kinds and worktree state; first saved user turn publishes the draft. `PROJECT_LIMIT = 20` drives single-project lists. Contracts: `tests/conversation-import.spec.ts`, native import tests, and new-task/draft/project-picker cases in `tests/ui.spec.ts`. |
| **0.2.36** | **Preserved:** bot templates/profile/goals, conversations, memories, schedules/check-ins, guided web/Telegram/Slack/Discord/WhatsApp connections, pairing, stop/new commands, local rules, pause/reset/delete/activity; durable queue/edit/reorder/remove and steering; usage-reset controls; same-project task search/queue/steer/stop/pin tools. **Superseded:** dedicated Bots-window chat is now the ordinary task conversation. **Intentional policy differences:** external approvals require a matching expiring request ID, proactive notifications always ask, scheduled results remain local absent explicit export approval, and channel revocation/ownership are rechecked. Bare yes/no cannot authorize another request. | Active paths include `bots.rs`, `bots/gateway.rs`, `bots/tools.rs`, channel adapters, `message_queue.rs`, provider steering/limits, `agent_tools.rs` and corresponding main handlers. Task tools enforce project ownership; bot notification is an `ALWAYS_ASK` tool. Contracts: `tests/bots.spec.ts`, `tests/bots-web.spec.ts`, `tests/message-queue.spec.ts`, `tests/steering.spec.ts`, `tests/usage-limit.spec.ts`, native bot rules/pairing/admission/approval-binding tests, and `storage.rs::task_search_matches_titles_and_messages_within_the_project`. Policy: `SECURITY-HARDENING.md` bot outbound/approval/revocation findings and `docs/bots.md`. |
| **0.2.37** | **Preserved:** compact controls/fields, round close button, single search fields and accessibility fallbacks. **Superseded:** the glass material was removed by original 0.3.4; focus color follows the chosen palette. | Higher-specificity rules in `src/styles.css:881–889` make floating surfaces solid and disable backdrop blur. Shared tokens remain, but their presence does not establish rendered glass. The later source note `releases/0.3.4.md` explicitly describes dropping glass. Existing appearance/accessibility cases in `tests/ui.spec.ts` and compact-menu/modal suites remain relevant to controls and accessibility, not a claim of preserved translucency. |
| **0.2.38** | **Preserved/equivalent:** bot navigation/status, full ordinary composer, add/configure actions and exclusion of bot workspaces from project pickers; slimmer modal controls. **Superseded:** the exact historical bottom-of-sidebar arrangement is the newer Bots sidebar mode/list. The old image/moodboard limitation is not an entitlement to bypass current attachment ownership or bot admission. | `sidebar-bots.ts` and `main.ts::renderBots` route a bot to `bot_desktop_thread`; the ordinary task view retains streaming, approvals and model selection. `userProjects()` filters bot workspaces. Settings still expose memories, schedules, channels and rules. Contracts: `tests/bots.spec.ts` sidebar, creation, model, pause, settings and navigation cases. |
| **0.2.39** | **Preserved:** Enriched Markdown in normal/streaming/worker/subagent/launcher/bot output, task lists, callouts, table alignment, ordered-list starts, math with plain currency preserved, and inert raw HTML; bot web chat uses the same parser. **Superseded:** the old dedicated bot page and adjacent persistent settings panel are the ordinary task conversation and explicit modal settings. All management features remain reachable. | Main and child renderers call the shared `markdown`/`streamMarkdown` path; `dom.ts` sanitizes rendered output, `enriched-markdown.ts` handles the documented nodes, and KaTeX uses untrusted-safe settings. The web chat serves the same md4c parser and keeps math as code as the note states. Contracts: `tests/markdown.spec.ts`, `tests/bots-web.spec.ts` task/callout/table/math checks, `tests/overlay.spec.ts`, and `tests/bots.spec.ts`. |
| **0.2.40** | **Preserved:** interactive simulator/device finishes and agent tools, explicit Android setup where supported, PR auto-link/manual linking/inspector, per-thread token saver packs and explicit missing-RTK installation, wallpaper/custom accents, VSIX/Open VSX color-only themes, automatic team model choice, bot setup/management, cleaner panel layout and right-aligned user bubbles with hover actions. No installation was performed during this audit. | `syncSimulatorPanel()` mounts the native-backed simulator; simulator tools/commands are registered in `lib.rs`. `github.ts` run-end detection and `github-pull.ts` expose PRs; newer detection uses the correct latest turn. Token saver toggles call `token_saver_install` only explicitly. `theme-settings.ts` calls bounded color-only native import/install; `palette.ts` holds imported colors across wallpapers. Auto teams use the selected task model and installed worker catalog. Contracts: `tests/simulator.spec.ts`, GitHub/PR suites, `tests/token-saver.spec.ts`, `tests/colors.spec.ts`, palette tests, `tests/team-profiles.spec.ts`, and `tests/message-actions.spec.ts`. |
| **0.2.41** | **Preserved:** user bubbles align with the conversation column's right edge, their text remains left-aligned, and completed activity/tool/change rows start at the reply edge. | `smooth.css` is actively imported after `styles.css`; its right-margin calculation aligns the bubble with the centered 830-pixel column and removes activity indentation. `tests/message-actions.spec.ts` compares bubble/reply right edges and activity left edges at 1,600 and 1,000 pixels. Source is byte-identical to original 0.3.27. |
| **0.2.42** | **Preserved:** live Working/tool/response activity aligns with completed replies without an end-of-turn indent shift. | The active `smooth.css` rule covers direct children of both `.transcript` and `#stream-region`; shared message content has zero left padding. The same message-action regression asserts the live activity summary aligns with its own box and the completed column. The source remains inherited unchanged. |
| **0.2.43** | **Preserved:** named team profiles, lead/model selection, model-picker integration, strict native roster enforcement, return to ordinary single-agent selection, and optional Classic team setup with explicit workers/run limits. | `agent-teams.ts` and `team-mode.ts` manage the saved library; `main.ts` mounts settings and uses the profiles in the model picker. `architect.rs::staff_from_roster` refuses provider/model pairs outside the configured roster instead of trusting the prompt. Contracts: `tests/agent-teams.spec.ts`, `tests/team-profiles.spec.ts`, and native roster/profile tests. |
| **0.2.44** | **Preserved:** local Apple-silicon Tart machines and catalog, user/agent screen input and commands, explicit signed/notarized Tart setup, headless operation, 2 GiB Linux defaults, ten-minute idle stop/suspend, quit cleanup, explicit desktop installation, and read-only project sharing. | `vm.rs` owns the catalog, processes and idle policy; `vm/tart.rs` sets headless/VNC/read-only options; `vm/setup.rs` checks the developer team, signature and notarization. `vm::watch_idle` is installed at app setup and shutdown releases app-owned VMs. `vm-panel.ts`, `vm/commands.rs` and approval-gated `vm/tools.rs` are connected. Contracts: `tests/vm.spec.ts`, native catalog/Tart/screen tests and ignored real-VM scenarios. No real-Mac certification is claimed. |
| **0.2.45** | **Preserved:** the repeated local-VM capabilities and non-Mac native availability checks. **Superseded:** hiding the entire VM tab on Windows/Linux would now hide supported cloud machines; the current panel is intentionally available there and explains that local VMs require an Apple-silicon Mac. | `vm::commands::status` uses `cfg!(all(target_os = "macos", target_arch = "aarch64"))`; local setup/options remain capability-dependent. `main.ts` permits the cross-platform panel, and manual cloud capabilities are independent of Tart. Existing Linux provider cases in `tests/vm.spec.ts` require no local setup/create/install calls. |
| **0.2.46** | **Preserved:** Daytona desktop/commands and idle timeout, Cloudflare authenticated container Worker, EC2 Linux/Windows desktops through SSM, AgentCore, explicit cloud connections, protected-store credentials, Deploy briefs and approval-gated CLIs. **Intentional policy difference:** missing deployment CLIs are not downloaded automatically with credentials; users must install them separately. Endpoint/credential restrictions and bounded redaction remain. | `cloud/commands.rs` routes the original backends and `vm/tools.rs` exposes their operations; `cloud/daytona.rs`, `cloud/cloudflare.rs`, `cloud/aws.rs`, `cloud/aws_screen.rs` and `cloud/agentcore.rs` implement their contracts. `deploy.ts` starts the platform brief and `cloud/deploy.rs` marks `pandacode_cloud_cli` always-ask. Keys are stored through `secret_store.rs`. Contracts: `tests/vm.spec.ts`, `tests/deploy.spec.ts`, loopback API/provider tests, AWS no-open-VNC-port checks and deploy refusal/scrubbing tests. Live-account behavior is not newly tested. |
| **0.2.47** | **Preserved:** canvas-drawn VM/cloud frames under the app CSP, immediate startup of a newly created VM, and image-to-screen coordinate mapping. **Intentional policy exclusion:** automatic background Xfce/package installation on first start. The explicit **Add a desktop** action remains available instead. | `vm-panel.ts` decodes bytes and draws the canvas; `vm/screen.rs` and frontend coordinate conversion preserve screen coordinates. `vm/commands.rs::vm_create` creates/starts without installing; `vm_desktop` is a separate explicit command. `tests/vm.spec.ts` asserts zero desktop-install calls on create/restart, then exactly one after the explicit action, and tests pixel mapping. Policy: `SECURITY-HARDENING.md`, “VM desktop.” The recently stabilized JPEG fixture still exercises the actual panel decoder/drawing/CSP path. |
| **0.2.48** | **Preserved/equivalent:** bot messages use the task column and common message presentation, full model/permission composer, and complete management settings. **Superseded:** the historical dedicated full-height settings side panel and check-in composer chip were replaced by the ordinary bot conversation with explicit header Settings; check-ins remain configurable in the bot profile/settings. Do not restore the obsolete parallel chat implementation. | `main.ts` uses ordinary conversation rendering for bot sessions, `botCrumb()` supplies header controls, and `showBotSettings()` mounts `bots.ts`/`bots-panels.ts` in settings layout. Profile exposes check-ins; memory/schedules/channels/rules/activity remain separate settings sections. Contracts: `tests/bots.spec.ts` common composer/model, header pause, settings, schedules and Open chat behavior. The later architecture is also explicitly documented in `AGENTS.md` and `docs/bots.md`. |

## PandaCode 0.3.0–0.3.27

### 0.3.0 — Open source

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| MIT-based license with fork credit; no activation or paid-license gate; contributor and security-reporting route | P | `LICENSE`, `README.md`, `CONTRIBUTING.md`, and `AGENTS.md` retain fork attribution and local build/contribution instructions. No account is required for ordinary local features; optional service sign-in remains service-specific. |
| App no-telemetry policy and documented child-tool opt-outs | P, U | `AGENTS.md` retains the app no-telemetry policy. `src-tauri/src/child_env.rs` configures supported CLI opt-outs and removes inherited app-bundle interference; its comments explicitly state that external CLIs can override these settings and this is not an egress sandbox. No universal third-party traffic-enforcement claim is made. |
| Automatic installed-CLI version checks and daily public model prices mentioned as exceptions | X | `src-tauri/src/agent_updates.rs` rejects external update operations; `src/app-updates.ts` is inert. The current no-update/no-price-refresh policy expressly excludes these requests. |
| Manual replacement builds retain data/settings; build from source | P | Stable application/data identifiers, storage migrations through schema 29, `scripts/release/build.mjs`, and `docs/releasing.md` preserve the local build/update path. Publication is a separate authorized release action, not something to infer from source support. |
| Original unsigned-build / Gatekeeper / SmartScreen warning | S/U | Later 0.3.18 added local macOS identity signing, but it does not certify Gatekeeper or Windows reputation. No fresh packaged-platform claim is made here. |

### 0.3.1 — Updates and GitHub view

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Automatic launch/six-hour checks, signed background download, idle restart, Restart to update, Settings updater controls | X | Current `src/app-updates.ts` no-op surface and absent updater capability intentionally disable this entire lifecycle. Old stored updater preferences do not reactivate it. |
| GitHub repositories, related threads, PRs/issues, review/check-fix/comment/merge actions, clone and thread linkage | P | `src/github-hub.ts` → `main.ts::startHubTask` → `src-tauri/src/github_hub.rs`; `src/github-pull.ts`/`github_pull.rs`; `tests/github-hub.spec.ts`, `tests/github.spec.ts`. Extended adds explicit issue-action confirmation/persistence instead of removing GitHub workflows. |

### 0.3.2 — GitHub launched from Finder/Dock

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Find an installed `gh` despite a minimal GUI PATH | P | `src-tauri/src/github.rs::gh_base`/`gh_command` use `providers/launch.rs` discovery, including Homebrew, `/usr/local/bin`, and user-local locations. Native `github.rs` minimal-PATH fixture exercises this path. |
| Show the CLI's actionable connection error | P | `github_hub.rs` status/error result reaches the GitHub connect screen in `src/github-hub.ts`; bounded external process output is retained. Existing GitHub browser fixtures cover connection failures. |

### 0.3.3 — Installed tools available to agents

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Give agents the installed tool locations available to a terminal | P | `src-tauri/src/lib.rs::prepare_environment` invokes provider launch environment setup; `providers/launch.rs::search_directories` and child setup cover `gh`, Node, Homebrew and local tools. Later AppImage scrubbing preserves this discovery while removing incompatible inherited bundle libraries. Native launch tests exercise minimal GUI environments. |

### 0.3.4 — Agent speed, UI, approvals, teams and Git

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Warm Claude follow-ups, maximum two processes, ten-minute idle lifetime | P/U | `providers/warm.rs` retains the limit/lifetime and matches model, mode, effort, cwd, resume ID, instructions and tool catalog; `lib.rs` prewarms after a clean turn. Current named-account revisions strengthen reuse isolation. Historical latency numbers were not remeasured. |
| Stable Claude/GLM/MiMo system instructions; first/eighth-turn reminders for other harnesses | P/U | `providers::takes_system_instructions`, `lib.rs` admission/instruction construction, and `capabilities.rs` cache explanation use the same policy. The historic token-saving estimate is not a new measurement. |
| Instant menus/dialogs/settings, retained status spinners; formatted remembered-thread view; stable composer; whole-interface zoom | P | `src/motion.ts` has `MOTION = false`; final CSS disables transitions; `main.ts::selectSession` uses task-owned `threadPages`; `dom.ts` caches formatted Markdown; `src/ui-size.ts` applies native zoom or preview scale. UI/streaming tests retain thread identity, scrolling and formatting oracles. |
| Compact one-line approval bar, grouped Approve all, bounded long commands | P | Approval rendering in `src/main.ts`/`src/agent-input.ts`, styles in `src/styles.css`; browser approval cases retain Deny/Approve and grouped actions. Existing native permission checks remain authoritative. |
| Compact spawned-worker rows at their spawn point; worker panel | P | `main.ts::workerSlot`/`renderTeamProgress`, `src/architect.ts`, `architect.css`; `tests/architect.spec.ts` chronology and worker status cases. Later details enhance the same panel. |
| Find-in-thread shortcuts and stepping | P | `src/thread-find.ts`, main shortcut/click wiring, `tests/ui.spec.ts`; Custom Highlight API avoids transcript DOM rewriting. |
| GitHub thread side panel; safe local merge into main and cleanup offer; branch switching; linked PR merge; inactive repositories under Other | P | `main.ts` GitHub panel host, `src/worktrees.ts`, `src-tauri/src/workspace.rs::merge_worktree`, `src/github-pull.ts`, `github_pull.rs`, and `github-hub.ts` repository grouping. `tests/worktrees.spec.ts` covers switch/merge/conflict refusal. |
| Grouped full-page Settings and resource display | P | `src/settings.ts` sections and usage/resource mounts, native usage commands; Settings browser coverage. |
| Sidebar one-/two-line project layouts, accent selection; Bots view with latest messages | P/S | Basic project/Bot rendering remains in `main.ts`/`src/bots.ts` and styles. The newer 0.3.45 Active/Needs review/Completed grouping legitimately supplements the old sidebar organization. |
| Solid borderless menus/modals/prompt, tighter corners; hide bot brief attachment context; stable bot gear, contained Team status, one-line moodboards | P | Final `styles.css` surface overrides, `main.ts` filtering `Bot brief:\n`, bot row markup, `architect.css`, and `src/moodboards.ts`/CSS. These behaviors were retained rather than inferred from obsolete base CSS rules. |

### 0.3.5 — Robust teams and rich questions

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Claude architect cannot wait on background monitor/scheduled wakeups; one continuation prompt before pausing | P | `providers.rs::CLAUDE_SESSION_BOUND_TOOLS`, launch configuration, and `architect.rs` continuation handling; native architect continuation tests. |
| Twenty-minute silent-turn watchdog, excluding approvals/input; silent worker fails, silent lead notices while other workers continue | P | `src-tauri/src/architect/watchdog.rs`; pending-approval/waiting checks precede cancellation and failure handling. Native watchdog/architect cases preserve these distinctions. |
| Immediate failed-worker handling; approval-free internal team tools; warm review; initial rules plus later delta/reminder | P | `architect.rs` failure dispatch and briefing, provider warm path and app-tool permission classification. The exemptions remain limited to internal team operations. |
| Questions do not start unsolicited work; reused assignment keys work for new requests; limits scoped to current objective | P | `architect.rs`/`architect/tests.rs` question, reused-key and assignment tests. |
| Missing folders or failed launches do not stop unrelated teams; pauses/stops/restarts do not spend retries; immediate Resume and four extra rounds up to twenty; failed retry cannot show old result | P | Architect scheduler/recovery implementation and native tests for limits, interruption and result reset. Current historical additions only extend provider/mode admission around this retained scheduler. |
| Busy worker rings, closed-panel Needs input, unsolicited pause/failure notices | P | `src/architect.ts`, `main.ts::renderTeamProgress`, `tests/architect.spec.ts`. |
| Rich Codex questions with Markdown, local images, charts/SVG/static HTML, no remote-image fetch or scripts | P | `src/agent-input.ts` → `src/rich-markdown.ts`; authorized `workspace_image` native access; `tests/agent-input.spec.ts`. Sanitization and task ownership are retained. |
| New panda icon | P | Application icons/assets and Tauri icon references persist in the inherited source. No replacement is required. |

### 0.3.6 — Queue reliability and safe cloud operations

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Failed queue delivery restored to the front and held | P | `main.ts::deliverQueued` failure path → `src-tauri/src/message_queue.rs::restore`; queue tests assert ordering and pause/retry behavior. |
| Bot queued documents/folders/images survive delivery; failed bot send restores message | P | `src-tauri/src/bots/gateway.rs` builds queued context/images and restores failed launches; native gateway and queue regression cases. |
| Cloud CLI always requires approval even under Full access | P | `src-tauri/src/cloud/deploy.rs` registers `pandacode_cloud_cli` in approval-required tools; provider/native permission pipeline retains the gate. |
| Refuse AWS credential operations and debug; redact access keys/secrets/session tokens | P | `cloud/deploy.rs`/AWS guard helpers; deploy unit/browser fixtures. Current manual adapters preserve and extend redaction. |
| Drain large Wrangler output; stop child process groups on timeout | P | Bounded `workspace::run_process_cancelable` and deploy runner cleanup remain active. Installed-CLI-only policy removes the separate bootstrap path, not bounded process handling. |
| VM idle watcher does not interrupt explicit Linux desktop installation | P | `vm/desktop.rs` installation state and `vm.rs` idle exclusion. `tests/vm.spec.ts` keeps explicit Add a desktop and no implicit install assertions. |

### 0.3.7 — Worker chronology

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Spawned workers precede later tools and resumed architect turns | P | `main.ts::workerSlot`/`renderTeamProgress` use spawn/turn ordering; `tests/architect.spec.ts` has both chronology cases. |
| Second spawn batch in one turn may group with the first | U | This is a documented upstream limitation, not a requested missing feature. No new claim that multiple batches are independently placed is made. |

### 0.3.8 — Worker panel and contributor build improvements

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Working header/count; per-worker harness/name/status/model; faded long list | P | `src/architect.ts`/`architect.css`; worker-panel fixtures. |
| No clipboard-image toast; file-drop confirmation remains | P | `src/attachments.ts` and main attachment callbacks distinguish paste from drop; attachment browser fixtures. |
| Rust library builds only linked output; fast-release developer package profile | P/U | `src-tauri/Cargo.toml` uses `rlib`; `package.json` exposes locked `package:fast` with `fast-release`. Original clean-build timings were not rerun. |
| Rust CI caching without redundant pre-Clippy check; four default browser workers with override | P | `.github/workflows/ci.yml` and Playwright configuration retain caching and configurable concurrency. Current split CI adds explicit feature gates rather than removing validation. |
| Eight historical failing browser cases | S | This is release-era test debt, not an additional product feature. The current candidate contains subsequent fixture and lifecycle corrections; this audit does not reproduce those old failures. |

### 0.3.9 — Scroll following and Send now

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Stay at bottom during send/stream; preserve reader position when scrolled up | P | `src/transcript-scroll.ts`, main render/stream integration; UI and streaming regression cases. |
| Send now immediately when idle/held; preserve remaining queue order | P | `main.ts::sendQueuedNow` and `src/message-queue.ts`; message-queue tests. |
| Stop-running Send now behavior; Claude/Codex labeled Steer | S | 0.3.27 expanded steering: current non-team text uses `steer_task` without Stop; attachment/team paths still stop before prioritized send. This is the intended later contract, not lost functionality. |
| No Send now for bot-owned queues | P | Bot queue rendering/dispatch omits the manual action; bots keep gateway-owned scheduling. |
| Repeated old browser-failure warning | S | Historical test debt as above. |

### 0.3.10 — Changed-file tracking disclosure

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Tracking limitation behind an info mark beside file and +/- counts | P | `src/turn-changes.ts` uses `infoMark` and accessible hidden description; existing UI changed-file cases exercise the card. |
| Repeated old browser-failure warning | S | Historical test debt, not a missing capability. |

### 0.3.11 — Pi

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Installed Pi discovery/configured executable, own accounts/extensions, provider-grouped models and thinking choices | P | `providers/pi.rs`, `providers/launch.rs`, provider catalog/settings and frontend picker. Installation remains an explicit user prerequisite. |
| Streaming, tool/context usage, images, persistent session, tasks/teams/bots | P | Pi RPC reader/launch/resume path and all provider allowlists; recorded native Pi frames. |
| Live Pi steering | P | `providers/pi.rs` steer handling, `providers/steering.rs`, and main `steer_task` dispatch. |
| Commands/skills/templates in slash menu | P | Pi command discovery through slash-command native/provider code and frontend menu. |
| Standard/Accept edits/Plan/Full permissions and authenticated app bridge | P | `providers/pi-extension.mjs` enforces read-only Plan, ask-before-mutation Standard and explicit modes; bridge token is removed from inherited child environment. `tests/pi-extension.test.mjs` covers these boundaries. |
| Retry/compaction continue instead of ending early | P | Pi event reader recognizes retry/compaction and final-state transitions; recorded tests preserve this behavior. |
| Full-only interactive console; unsupported rich extension prompts declined; no subscription usage | P/U | Console admission and Pi extension response types retain the limits. No live paid call was made to certify an external CLI installation or subscription endpoint. |

### 0.3.12 — DeepSeek, document reader and generative UI

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| DSH ACP discovery, model list and Off/Low/High/Max effort | P | `providers/deepseek.rs` and catalog/settings maps; no-prompt model metadata and recorded ACP tests. This provider module is unchanged from inherited 0.3.28. |
| Optional protected saved key, process environment or DSH's own account | P | Provider credential configuration only passes the authorized key to DSH; named-account isolation is retained. |
| Workspace-write Standard/Accept edits, read-only Plan, explicit Full; teams can change lead model/effort | P | DeepSeek sandbox mapping; resume ID carries sandbox and changes conversation when it changes. Main model picker updates architect configuration; native/team tests. |
| Rendered/raw text/Markdown reader, Copy, wrap, word count/reading time; file links and agent-open routes | P/S | `src/document-viewer.ts` retains controls. `main.ts::openFile`, `src/file-tabs.ts` and the later task-owned document surface replace the original sidebar location; authorized file access remains native. `tests/document-viewer.spec.ts` covers rendering/raw/copy/wrap. |
| Fixed code line numbers excluded from copied text; optional wrap | P | `document-viewer.ts` code rendering/copy behavior and browser tests. |
| Structured table/chart/stat/card/callout/tab/accordion/step/timeline/checklist/plan/tree/diff/code/terminal/pro-con/gallery/form/action/grid views | P | `src/generative-ui-components.ts::VIEW_TYPES` and renderer cases, reached from `src/dom.ts`; generative UI unit/browser tests. |
| Scriptless custom HTML/CSS, no frames or remote resources; actions only populate composer | P | `src/generative-ui.ts::fillComposer` and custom sanitization in components; existing negative sanitization/action tests. |
| Copy code, collapsible sections and follow-up chips; smart-quote file mention fix; no conversation focus outline | P | Generative/Markdown renderer, attachment mention parsing, and conversation CSS; corresponding UI/generative tests. |
| Mermaid source-only, HTML/SVG source disclosure, no DeepSeek limits, recorded/live limitations | P/U | These are retained limits rather than missing eligible features. Real DSH account/hardware behavior and old timing/test-debt claims were not re-certified. |

### 0.3.13 — Vertical prompt overflow

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Empty prompt has no vertical scrollbar; restored drafts resize; overflow only beyond maximum height | P | `styles.css` composer textarea rules and `main.ts::resizePrompt` cap at 180 pixels and switch `overflowY` only when needed; called on draft restoration/input/scale. |
| Historic failing-test warning | S | Release-era test debt, not a feature gap. |

### 0.3.14 — Horizontal prompt overflow

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| No horizontal scrollbar; wrap long words/URLs; preserve long-message vertical scrolling | P | Same textarea rules use hidden horizontal overflow and `overflow-wrap:anywhere`; `resizePrompt` controls vertical overflow. |
| Historic failing-test warning | S | Release-era test debt. |

### 0.3.15 — OpenCode, Duo, Android, Actions, deploys and packaging

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Whole OpenCode catalog when no settings selection; explicit narrowing; verbose-list fallback | P/S | `providers.rs::list_models` and `providers/extra_models.rs` retain discovery/fallback; 0.3.16 intentionally shortens the initial picker but still searches the complete catalog. `tests/opencode-models.spec.ts`. |
| Safe Windows npm/pnpm shim resolution for OpenCode/Pi | P | `providers/launch.rs` follows the package-declared Node entry without a command shell; Windows shim fixtures. |
| Duo cover/inner display, three poses, agent fold/tap/type with Xcode 27.1+ | P/U | `src/simulator.ts`, `simulator-3d.ts`, `simulator-duo.ts`, native `simulator/duo.rs`/`duo_helper.m`, stream/input integration. These files are unchanged from inherited 0.3.28. Browser/recorded native oracles exist; actual Xcode hardware was not exercised. |
| Windows installed Android SDK listing, USERPROFILE fallback, no console window, dotted API parsing; no Windows SDK autoinstall | P | `simulator/android.rs` path/parser/spawn flags and Windows UI projection; native recorded tests. Current native installer exclusion must remain. |
| PR checks badge; workflow list/status/branch/age; failure-log fix task | P | `github_hub.rs`, `src/github-hub.ts`, `src/github-hub-prompts.ts`; Actions fixtures. Extended confirmation and draft/issue ownership safeguards remain. |
| Railway installed CLI, protected process-only token, redaction, deploy-only allowlist | P | `cloud/deploy.rs` installed CLI resolution, Railway arguments and secret scrubbing. |
| Automatic pinned npx Railway fallback | X | Current policy explicitly forbids automatic download-and-execute fallback; installed Railway remains usable. |
| Installed Google Cloud login/project/region, credential-command refusal, token scrubbing, reporting disabled | P | Gcloud/deploy guards and child environment; current manual GCP adapter adds safe operations without cloud-agent credential transport. |
| Completed tasks hidden from ordinary sidebar, pinned or not, reopenable | S | Upstream 0.3.45 introduced a collapsed persistent Completed section. `src/sidebar-sections.ts` and main sidebar rendering follow that newer behavior. |
| Mac header drag; remove conflicting AppImage Wayland libraries; fork URL/CRLF helpers; hooks test independent of global hooksPath | P/X | Header drag regions; `scripts/release/appimage.mjs` and tests; offline updater-manifest/version helper tests; native Git hook fixture isolation. Automatic publication/updater use remains X, not the compatible helper logic. |
| Original real-device/packaged-platform and refresh-only Actions limitations | U/P | No hardware or packaging verification was attempted. Actions refresh remains explicit; no push notification feature was promised. |

### 0.3.16 — OpenCode recents and Duo rendering

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Default plus five current/recent OpenCode choices; full search; settings selection still wins | P | `src/model-picker.ts` recent-limit/catalog logic and `tests/opencode-models.spec.ts` exact Default+5/search/selection cases. |
| Duo rear camera/material/hinge geometry | P | `src/simulator-duo.ts` retained geometry/material definitions. |
| Optimistic pose, loading spinner, error rollback | P | `src/simulator.ts` pose state and request epoch; simulator browser cases. |
| Compressed inner display frames/faster switching | P/U | Native `simulator/stream.rs` and Duo display path retain binary frame delivery. Historic speed/size ratios are not a fresh benchmark. |

### 0.3.17 — Responsive and reliable Duo folding

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Cheap spinner and rendering only while motion/updates require it | P | `src/simulator-3d.ts` render scheduling/spinner path; unchanged inherited implementation. |
| Immediate 3D pose, coalesced latest selection, 0.2-second close sweep, settle wait and half-open recovery | P/U | `src/simulator.ts` poseWanted/epoch; `simulator/duo.rs::fold` serialized latest-wins pipeline and recovery. Native coalescing fixture and browser rapid-pose case exist; cycle timings were not rerun. |
| Inner-display accessibility positions/content used for agent taps | P | `simulator/duo.rs` element parsing/active display selection, `duo_helper.m` and `simulator/tree.rs`; recorded AX and ignored live cases. |
| Xcode/SpringBoard and inner-description latency limits | U | Retained platform constraints, not missing source features. |

### 0.3.18 — Installer and local signing

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| npx latest-release download, signature check, platform install and download/dry-run | X | `packages/npx/bin/pandacode.mjs` exposes safe help/version and rejects install; current release policy excludes automatic bootstrap/install. |
| Stable local macOS signing identity in own Keychain for eligible app bundles | P | `src-tauri/src/local_signing.rs` handles the running own ad-hoc bundle; `lib.rs` startup calls `sign_running_app`. Real Developer ID signatures are left intact. This must not be misclassified as removed. |
| Re-sign after an automatic updater operation | X | The updater lifecycle is absent; local startup signing remains available. |
| First identity-change prompt, no Gatekeeper certification, npx quarantine avoidance/platform limitations | S/U/X | Local signing's identity caveat remains; no Gatekeeper guarantee is made. The npx installation effects belong to the excluded path, and no real platform installation was attempted. |

### 0.3.19 — PandaCode Connect

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Optional GitHub sign-in, allow-host switch, mutual fingerprint comparison/trust, task read/send/start/stop/approval | P | `src/remote*.ts`, `src-tauri/src/remote.rs`, host/router/trust code; current explicit key pinning, replay and revocation checks strengthen the same workflow. |
| Separate Computers sidebar and remote view | S | Explicitly removed upstream in 0.3.20 in favor of ordinary mirrored tasks. No old view should be restored. |
| No pre-sign-in traffic; E2E X25519/XChaCha20-Poly1305 relay forwarding; host permission policy | P | Native remote socket/crypto/session handling and `relay/`; current separate Full access grant further constrains privileged remote actions. |
| Up to ten devices per account | P | `relay/src/limits.ts::MAX_DEVICES = 10`, enforced by `relay/src/account.ts` registration before adding a new row. |
| Awake/online requirement; paged/bounded transcripts; original automated-only validation | P/U | Online/paging/bounds remain, later attachments/pagination improve capabilities. No two-real-device run was performed for this audit. |

### 0.3.20 — Ordinary remote tasks

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Computer picker and ordinary row/transcript/queue/Stop/approval/model/mode | P | Main computer/model picker → `remote_start_task`; `remote/mirror.rs`, native `remote::route` and local durable queue. Task-owned mirror IDs remain separate from host IDs. |
| Find or clone same GitHub repository under ~/PandaCode, use matching branch or new worktree; refuse dirty/busy checkout switching | P | `remote/start.rs` repository resolution, branch validation, checkout guards; native recorded tests. |
| Confirm dirty/unpushed branch or commits before starting from GitHub version | P | `remote/mirror.rs::repo_summary` and remote start confirmation UI. |
| Throttled pushed events and one transcript copy per run | P | Native remote event batching/forwarding and mirror synchronization retain this architecture; later response/task ownership and cancellation guards strengthen it. |
| Install-specific key records avoid shared-build keychain conflict | P | `remote/config.rs` scoped record identity and guarded legacy adoption; protected store remains authoritative. |
| Remote image/browser captures unsupported | S | 0.3.44 added durable remote attachments. Current attachment authorization, replay and ownership checks implement the newer contract. |
| Remote files/diffs remain hosted; no fresh real-device validation | P/U | `Store::workspace_root` refuses mirrors; no unsafe local workspace access is substituted. Hardware validation was not performed here. |

### 0.3.21 — Command Code, omp, team settings and GitHub improvements

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Installed Command Code ACP with catalog, effort, slash commands, native permission modes and protected key/own CLI login | P | `providers/commandcode.rs`, ACP discovery, settings/catalog/slash surfaces. Recorded tests cover mode, effort, auth and resume. The module is unchanged from inherited 0.3.28. |
| Installed Oh My Pi with its existing login/config | P/S | Pi/omp shared integration and all provider allowlists are present. Later reviewed RPC v2 support/chunk handling extends the initial implementation. |
| Team-specific lead guidance and reset; independently configurable lead/worker efforts | P | `architect.rs` briefing, `architect/profiles.rs`, `src/architect.ts`, `src/agent-teams.ts`, and native/browser guidance/effort tests. |
| Actions event/PR/attempt/duration metadata, filters/latest-per-PR, PR Runs link | P | `github_hub.rs::runs_of`/`runs`, `src/github-hub.ts`; Actions filter fixtures. |
| Job duration/artifact size details; Didn't start classification and workflow fix | P | Native `run_state`/`run_details`; frontend actions renderer and draft prompts; job/artifact/non-start tests. |
| Preserve GitHub typed input during refresh | P | GitHub view state/render handling and browser stale-refresh tests retain the value rather than replacing it. |
| Remove project including missing-folder record without deleting folder | P | Settings project list → native `remove_project`; project/task storage cleanup is separate from disk deletion. |
| Mac header drag; Markdown/provider/launcher links use default browser | P | Header drag regions; `src/main.ts`/`src/overlay.ts::mountExternalLinks` invoke authorized `open_external_url`. |
| Lazy Material Icon Theme file icons for Files, mentions and changed files | P | File-icon loader and callers retain lazy imports; no startup bulk icon loading was added. |
| Task elapsed time, new-task default model, past-tool expansion preference with running activity open | P | Main runtime label, remembered model/default preference and `activityExpanded` handling; Settings/UI regression coverage. |
| Native Linux ARM64 build | P/X/U | `scripts/release/common.mjs` and `build.mjs` support `linux-aarch64`; release tests require matching native host/architecture and Extended artifact names. Auto updater/npx distribution is X. A real ARM64 build was not run. |
| Windows installed Android devices / SDK instructions | P | Platform-aware simulator projection and native Android discovery retain the supported installed-tool path. |
| Scrub AppImage library/Python paths from child tools; find ~/.grok/bin; log warm misses | P | `child_env.rs`, `providers/launch.rs`, `providers/warm.rs`; minimal environment and warm-key tests. |
| Command Code cannot take mid-turn steering | S | 0.3.27 supports next-turn delivery within the same ACP process/session; current steering matches that later contract. |

### 0.3.22 — Stable Connect identity and host catalogs

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Preserve/adopt matching older sign-in, keep device identity/trust, retry transient Keychain read, refresh expiring token | P | `remote/config.rs` legacy-adoption and scoped identity validation; `remote.rs` startup retry; socket token renewal. Malformed/unreadable records fail closed and are not overwritten. |
| Host-specific installed providers/models, fallback with notice, restore local selection | P | `main.ts::adaptPickerToComputer` and per-device catalogs → native `remote/catalog.rs`; provider/model browser cases. |
| Missing legacy record requires sign-in; real-machine behavior | P/U | Correctly remains an actionable sign-in state, with no fabricated replacement identity after protected-store failure. No hardware run was attempted. |

### 0.3.23 — Readable tool traces

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Sentence-like tool summaries and MCP server/tool names | P | `src/tool-trace.ts` normalization/rendering and unit fixtures. |
| Group summary counts/icons, active progress and distinct failed counts | P | `src/tool-activity.ts` shared grouping and unit fixtures. |
| Expanded command/file/search/diff details with raw input/output disclosure | P | Shared tool renderers; browser activity/diff/disclosure cases. |
| Conversation, Board, launcher, subagent and worker reuse; raw subagent detail stays open during stream | P | Main transcript renderer, `src/overlay-thread.ts`, `src/subagents.ts`, `src/architect.ts` use the shared trace path; subagent update preserves raw disclosure state. |
| Original Chrome-only validation warning | U | This source audit did not validate native WebKit rendering. |

### 0.3.24 — Chats, connections, design, Skills and developer tools

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Projectless Chats, private folder, sidebar section, hidden project controls, Move to project before start | P | Main New chat/move/delete wiring → `src-tauri/src/chats.rs`; `src/chats.ts`; chat/browser fixtures. Extended cleanup and task attachment ownership are retained. |
| Named per-harness executable/arguments/environment/config profiles, protected secrets and task-bound choice | P | `src/connections.ts`, `src-tauri/src/connections.rs`, picker/task storage and launch snapshot. Current account revisions and native task policy prevent stale/broadened connection overrides. |
| File-backed scriptless artboards, pan/zoom, element comments and return to agent | P | Main Design tab mount → `src/design-canvas.ts` → native `design.rs`; design browser fixtures. No scripts execute inside artboards. |
| Skills/rules folder/GitHub imports, global/project activation per harness, drift/unmanaged detection, secret/bypass scan, never overwrite unowned files | P | `src/skills*.ts`, native `skills` modules and actual activation commands. Extended scoped hash-pinned authorization deliberately strengthens the inherited contract; raw upstream activation must not replace it. |
| Non-colocated Jujutsu Changes panel, metadata/diffs/Describe/New/Commit/log | P | Main `jj_status` refresh and non-colocated renderer → `src/jj.ts` and native `jj.rs`; recorded jj/browser cases. |
| Colocated Git+jj chip | S | **Absent, as upstream 0.3.45 explicitly documents.** Main imports `jjChip` but does not render it in the newer Git Changes view; `tests/jj.spec.ts` tests the helper output rather than actual placement. This is a declared later limitation, not a pre-0.3.28 port gap. Do not restore old UI or clean the import as part of this audit. |
| Read-only capability matrix and instruction/cache token explanation | P | Settings mount → `src/capabilities-panel.ts` → native `capabilities.rs` inventory/cache calls. Scanner returns names/metadata without secret values; instruction policy is derived from provider behavior. Capability tests exist. |
| Saved JSON workflow recipes, dry-run native/fallback explanation, execution as normal queued messages | P | Main + menu → `src/recipes.ts` → native recipe/queue calls; recipe tests. No hidden action executor replaces normal approval/task flow. |
| Installed GitHub Copilot CLI harness | P | `providers/github_copilot.rs`, discovery/allowlists/catalog/settings; recorded ACP tests. |
| Workspace text search / inspector navigation | P/S | `src/search-panel.ts`, authorized native workspace search and main tab wiring. Later inspector tabs replace the older activity-bar presentation while retaining Search. |
| Off-by-default clean-turn project checks, one editable fix, never loop | P | Main clean-finish hook → `src/turn-checks.ts` → native `checks.rs`; `tests/turn-checks.test.mjs`. Plan/read-only and stale task guards remain. |
| Installed codedb read-only tools with telemetry/daemon disabled | P | `src-tauri/src/code_search.rs` installed resolver, constrained search/find arguments, bounded output and MCP tool registration. No installer is introduced. |
| New task from project heading | P | Main project row action and New task handler; sidebar/UI fixtures. |
| Intel macOS native target | P/X/U | Local `darwin-x86_64` target/build/artifact tests are present. Automatic updater/npx installation is X; no physical Intel Mac certification was attempted. |
| GitHub keyboard run navigation; deeper steps/summary/attempt history; Check workflow files action | P | `src/github-hub.ts` keyboard handling, `github_hub.rs::run_details`, actions renderer and `github-hub-prompts.ts`; browser fixtures cover these exact surfaces. |
| Node version-manager discovery, pi.dev Windows installed entry | P | `providers/launch.rs::node_manager_dirs` handles fnm/nvm/mise/asdf/NVM Windows; declared-entry resolution handles pi.dev. Native GUI-PATH/shim fixtures cover both. |
| Newly opened project picker remains open; Board task opened during boot stays selected | P | Main picker/startup task-ownership guards and current Board/UI cases retain the intended selection. No source removal of those guards was found. |
| Recorded-only jj/codedb/Copilot, static artboards, untested Intel hardware | P/U | Scriptless artboard behavior is intentional. Source presence and recorded tests do not certify live CLI/hardware behavior. |

### 0.3.25 — Windows ARM64 and app-wide lead guidance

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Native Windows ARM64 artifact | P/X/U | `windows-aarch64` target and matching artifact/native-host checks in `scripts/release/build.mjs` and tests. Automatic updater/npx installation remains X; real ARM64 hardware was not tested here. |
| App-wide lead guidance; per-team override; reset falls back to app-wide value | P | `architect/profiles.rs` persistence, `architect.rs` own-guidance/fallback selection, `src/agent-teams.ts` and `src/architect.ts`; explicit native/browser guidance tests. |
| Full-height filter and readable model name next to effort control | P | `src/agent-teams.ts`/CSS and `tests/agent-teams.spec.ts` layout/effort cases; inherited layout remains. |
| Old-version downgrade cannot read newer guidance library | U | No backward-format rewrite or downgrade guarantee is introduced. This is a documented migration constraint rather than missing functionality. |

### 0.3.26 — Current GLM catalog

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| GLM-5.3 default, Flash, both 1M variants, Turbo, Flash background model | P | `providers/glm.rs` catalog/environment constants, frontend fallback/default maps; module unchanged from inherited 0.3.28. |
| Keep old saved model IDs until user changes them | P | Native catalog/storage preserve unknown/old IDs instead of silently overwriting task selection. Current provider-picker tests also preserve unavailable saved values. Server-side alias routing is external and not re-verified by this source audit. |

### 0.3.27 — Cline and universal steering

| Item | Class | Evidence and assessment |
| --- | --- | --- |
| Installed Cline ACP, no-prompt account catalog, effort, permission modes and same-session follow-up | P | `providers/cline.rs` and ACP transport; settings/discovery/console/capability maps and recorded mode/effort/model/resume tests. Later safe console/capability additions are already integrated. Unrelated credential guessing remains intentionally excluded. |
| Every harness accepts Steer without stopping the run | P | `message_queue.rs::steer_task` → `RunInput::Steer` → `providers/steering.rs`; native remote route uses the same admission semantics. Claude-family stream input, Codex turn/steer, Pi/omp RPC, OpenCode second ACP prompt, and other ACP next-turn holding are implemented. |
| Startup/between-turn/refused steers held for same process/session; pending live-vs-next-turn status; failed steer requeued | P | ACP `acp_steer`/`next_steered_turn`, late Claude replay acknowledgements, Codex refusal fallback, end-of-run failed events; main pending map handles event-before-IPC-return and queue/composer recovery. Native steering/ACP and browser steering/message-queue tests cover these distinctions. |
| Steer beside Stop and Ctrl/Meta+Enter | P | Main composer action and shortcut, `src/message-queue.ts` action labels; no destructive Stop is used for eligible text steering. |
| Cline images as authorized file paths, unavailable app tools/slash/context/limits | P/U | Cline configuration retains the advertised limits; current later console/capability work does not falsely claim unsupported tools. No live Cline account call was made. |
| Accepted-then-failed-and-resent steer can appear twice historically | U | Documented upstream limitation, not evidence of an unported feature. This audit does not expand into unrelated transcript redesign. |
