# S2: Existing bots as native Council reviewers

## Purpose and defaults

Council continues to use the existing Architect review/scoring/round engine. **Automatic / native workers** remains the default. For one task or direct bot conversation, the user can choose **Selected bots** in the existing Council settings and select 2–8 distinct configured bots.

The selector lists a bot's name, role, provider, model and availability. Identity is persisted by bot ID, never by display name. The executor cannot review its own work. Missing, removed, paused, incompatible or unavailable bots fail explicitly; selected mode cannot silently substitute another bot or switch back to automatic mode. Selecting reviewers does not change any bot's ordinary permissions, stored profile, conversations, memories, Connections or settings.

## Native execution

- When the lead dispatches a Council reviewer, the native adapter chooses an unoccupied selected bot ID for the current cycle/round, resolves the real bot provider/model/reasoning effort, and starts a fresh independent Architect worker session under the S1 native reviewer policy.
- The S1 `ReviewerBinding` records the selected reviewer ID, per-round roster/quorum and a frozen, review-relevant bot persona plus assigned skill IDs (the S3 resolver is deliberately not yet invoked). A separately persisted native authority record authenticates the exact identity and snapshot at admission; a forged persisted identity is rejected.
- The selected persona informs the reviewer's lens and report. It does not grant the bot's ambient tools, shell, MCP access or ordinary workspace privileges. S1's native tool policy remains enforced. The reviewer receives no previous reviewer score or target.
- **Review tool access** is task-scoped and defaults to native read-only. An explicit user choice can authorize `pandacode_open_file` for one exact, task-relative path via S1's `Scoped` host-tool wrapper. Arbitrary app tools, external MCP writes, shell and assessed-checkout modifications remain unsupported in this scope; unsupported requests fail visibly rather than claiming broader permission. S5 owns post-acceptance GitHub issue creation.
- Original Council scoring, score parsing, round limits, correcting, final reports and exceptional acceptance stay unchanged. Automatic Council seats remain backward compatible.
- A round in progress cannot be restaffed by changing the selection. Existing completed reports and authority snapshots remain associated with their original seats. Reconfiguration affects later rounds, not prior completed results.

Skills provisioning (S3), Shared Channel delivery opt-in (S4), delivery approval/callback (S5), and recovery/failover expansion (S6) are **not part of S2**.

## Verification contract

Acceptance: AC-01, AC-02 and REQ-02 of #126.

1. Typecheck/build and existing Council/browser-preview regression tests.
2. Focused native validation for bot ID uniqueness, bounds, exclusion, immutable reviewer authority, read-only binding and ordinary Council behavior.
3. On an authenticated desktop build with at least two installed reviewer-capable harnesses or bots, configure two distinct eligible bots, run an actual direct Council task, and confirm two independently started seats and attributed scores. Repeat with automatic reviewers. Check that bot profiles and ordinary conversations are unchanged.
4. Test pause/remove/unsupported reviewers, executor exclusion, duplicates, and attempted mutation (S1 guard) using the real harness. Log exact SHA, provider version, OS, outcome and evidence. **Preview mocks and CI alone are not evidence of real provider execution.**

Do not check S2 complete in #126 until the real desktop scenario and regression evidence pass. S1 adversarial provider validation remains tracked separately.
