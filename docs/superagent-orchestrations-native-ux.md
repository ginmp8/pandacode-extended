# Orchestrations workspace: PandaCode-native UX

This page documents the focused user-experience redesign built on top of the existing native SuperAgent work in PR #154. It does not introduce a second execution engine, bot registry, or authority source.

## User journey

1. Open **Bots → Orchestrations** in the existing main-content workspace.
2. With no Teams, read the one-screen explanation and select **New Team** (guided setup).
3. Configure a name, one Coordinator, one or more configured Workers, optional Council Reviewers, Skills/policy visibility, and save the Team **Off**.
4. Select a saved Team to inspect its configured member roster. Selecting a member shows the current bot identity, configured provider/model/mode/project and assigned Skill IDs. These fields are read-only observations, not live provider proof or authorization.
5. Use **Members and Skills** and **Execution policy** for future-Team configuration. Native revision, project, bot and Council checks remain authoritative.
6. Select **Assign demand / Open runs** to use the existing native admission and preflight flow. Inspect Tickets, WorkItems, evidence, grants, source receipts and review status through their existing native calls.
7. The **Review inbox** remains a native Ticket projection; **Sources** and **Portable configuration** are secondary navigation rather than competing first-use actions.

The previous one-form manual creation path is retained in a collapsed **Advanced: legacy manual setup** section for compatibility, with the guided builder as the recommended path. Expansion state is local presentation state and cannot authorize any work.

## Visual integration

The implementation uses the existing PandaCode sidebar and persistent main pane. Styles are scoped to `.sa-native-ui` and reuse `--ds-*` tokens from `src/design-system.css` and surface geometry from `src/inset-shell.css`.

- A quiet eyebrow/title/description and a single primary **New Team** action reproduce the page hierarchy used by Customize.
- A 196px secondary navigation column separates workspace navigation from Team configuration, as in Customize.
- Bot members are displayed as compact rows with an adjacent read-only inspector. No additional bot identity, model availability or private conversation is inferred.
- The Demand board retains the existing native paginated Ticket index and state projections; UI styling borrows compact Board card spacing.
- CSS adapts the navigation and member inspector to narrow viewports without a second application shell.

Source visual references: `docs/screenshots/settings.webp`, `board.webp`, `bot-settings.webp`, `agents-flyout.webp`, `github.webp`. Code references: `src/styles.css`, `src/customize-skills.css`, `src/design-system.css`, `src/superagent.css`.

## Native security invariants preserved

- All Team creation and updates still invoke `superagent_create`, `superagent_update` and revision-checked native commands.
- No frontend action issues scoped file writes, GitHub effects, Council verdicts or task grants without the existing native guard.
- The saved Team is Off by default. Configured Workers are not active Assignments.
- A member's presence in the Skills Library is not proof of harness eligibility or actual invocation.
- Council waiting, Council passed, human acceptance, and Ticket completion remain distinct.
- No synthetic workload percentages, timelines, active sessions, tool actions, review verdicts or repository outcomes are added to production UI.

## Verification

Focused browser tests: `tests/superagent-native-ux.spec.ts`. They exercise the first-use flow, guided creation, Team overview/member inspector, section navigation and secondary views. Existing `tests/superagent.spec.ts` remains the source of regression evidence for native orchestration actions.

### Remaining acceptance work

This is a PandaCode-native workspace UX slice, **not** the full completion of issue #148 UX-01..24, S1-S7 or original #131. Remaining work includes integration coverage across all current native commands, comprehensive WorkItem graph/inspector layouts, responsive/light/compact/200% zoom screenshots from a running PandaCode instance, and complete end-to-end tests with native Council and scoped-write fixtures. A standalone HTML prototype or synthetic screenshot is not proof of a production implementation.
