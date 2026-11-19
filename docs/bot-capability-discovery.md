# Native bot capability discovery

Issue: [#159](https://github.com/ginmp8/pandacode-extended/issues/159). Base: stacked PR #154.

## Scope and trust boundaries

Profiles are read from current local bot configuration at query time. Results describe declared role and explicit published summary, assigned Skills and MCP references, native provider/model/permission mode, opaque Project ID, and paused status. Declared fields include provenance and a status; runtime eligibility and concurrency are not inferred. A Project ID is not a checkout or permission grant.

Discovery does not run a provider, send a work message, mutate settings, authorize a tool, or approve Council work. Native project permissions, Council and ToolHost gates remain authoritative. Assigned Skills and MCP references are not proof of successful installation, harness compatibility, execution or task completion.

Raw instructions, hidden prompts, private memories, unrelated chats, environment values and credentials are not returned. Only an explicitly user-published bounded summary can be shared with other bots. Each bot can opt out of peer discovery; the local owner can still inspect it. Declared summaries and capability questions are untrusted data. Questions are answered by a native deterministic reader instead of an LLM.

## Interfaces

- Desktop: Bots → Settings → Capabilities, with current fields, visibility settings, refresh and capability-only questions.
- Native IPC: `bot_capability_profile({botId})`, `bot_capability_question({botId,question})`, `bot_capability_preferences_set({botId,visibility,summary})`. Main-window authorization applies.
- Bot conversations: `pandacode_bot_inspect_capabilities({to})`, `pandacode_bot_ask_capability({to,question})`. Session identity is recovered natively and collaboration ancestry/three-hop limits apply.
- Headless: `pandacode serve --inspect-bot-capabilities <bot-id>` or `--ask-bot-capability <bot-id> <question>` prints read-only JSON without normal task startup/recovery.

## SuperAgent planning

The Team editor can compare discoverable candidates against an explicit objective. Ranking uses exact terms from declared role, the explicitly published summary and assigned Skill references. It does not infer abilities from bot names or prior conversation text. Results explain match terms, provenance, unknown execution eligibility and Project scope. Suggestions can be staged as Workers or Reviewers, but roles are not changed until the user saves through the existing revision-checked native roster API.

When a coordinated Demand is admitted, its Coordinator also receives a bounded, read-only snapshot of the current configured Team members. Opted-out bots contribute no profile details. The packet is labelled untrusted context; the existing read-only mandate remains authoritative. No ambient bot tools are enabled for SuperAgent Coordinator sessions.

## Persistence and audit

SQLite schema v70 stores per-bot visibility and an explicit published summary in a foreign-key sidecar; no new execution permission tier exists. Profile responses are generated fresh, not cached in native code. Peer logs contain target ID and operation type only, never the question text, private prompts or response body.

A discovery result is informational, never a permission grant, provider success evidence or Council verdict. Exact-HEAD Rust, TypeScript and browser tests are still required for acceptance.
