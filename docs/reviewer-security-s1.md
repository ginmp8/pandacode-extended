# Council S1: task-scoped native reviewer authority

## Product contract

A PandaCode bot keeps its configured harness, Skills, MCP references, Connections, and permissions. Council does not edit the bot profile. A reviewer seat runs with an **effective task- and phase-scoped policy** instead of inheriting every ambient capability. Native reviewers default to `NativeReadOnly`. A future user-facing S2/S4 flow can provide a frozen, explicitly authorized intersection of the bot's configured tools and the user's exact task grants.

A grant names an existing **host-managed function tool**, an exact operation (`action` argument, or atomic `call`), and an exact string resource argument/value. The native wrapper both advertises only granted tools and checks every invocation before dispatch, including MCP transport. Unknown, composite, absent, unscoped, or unsupported tools do not silently gain access. Full-access source bot permission alone never authorizes a reviewer write. A grant must be explicitly sourced from the user's selection, not a model prompt, skill description, or other untrusted artifact. A user-authorized operation that changes the assessed artifact must invalidate the approval; S4-S6 own revision and delivery integration.

The same bot can conduct a review and later publish GitHub issues based on accepted findings. **Issue publishing is S5**, after an approved verdict and the user's repository/action opt-in; no extra confirmation or second bot is required for an already authorized action. S1 implements the underlying grants and native execution boundary only. The current Council UI does not yet select these grants and no new GitHub publishing workflow is claimed here.

## Frozen identity and compatibility

`ReviewerBinding` contains Council cycle, optional delivery ID, round, seat, optional reviewer bot ID, artifact revision token, and immutable provider/model/lens/tool policy. Native automatically scheduled reviewers use no delivery or external bot identity in S1; S2 and S4 must bind their IDs to authoritative records. An S1-issued native artifact token is anchored to Council start, source dispatch revision, worker creation time, and worker ID; it is **not** a content hash or proof that an artifact was checked. Legacy workers without a binding retain read-only admission. Existing WorkerProfile and native Council scoring/rounds are not replaced.

The reviewer tool catalog version is bumped so Codex's persisted native worker contexts recover without retaining an older writable tool inventory. New grants must be derived from actual configured bot capabilities by trusted native code; S2 will own collecting and resolving user grants. S6 will revalidate them on resume and failover.

## Enforcement boundary

- Codex review always retains its native read-only filesystem sandbox, denied network through sandboxed shell, and `approvalPolicy=never`; the bot's ordinary `Full access` mode remains unchanged.
- A scoped Council reviewer may receive only individually permitted **PandaCode-owned host tools**. The wrapper checks tool name, operation and resource argument again at invocation time. User-granted actions are preauthorized for exactly that scope; unauthorized actions are rejected. Generic tool and shell privileges are not conferred.
- Direct external Codex MCP servers are **not** a managed host-tool boundary. The native `codex mcp list --json` preflight requires an empty external server inventory and project-local MCP/plugin/app configuration is rejected. `-c mcp_servers={}` does not erase inherited TOML servers. Direct third-party MCP access remains unsupported until a future adapter can enforce scoped calls; never silently allow it because the bot already has an MCP reference.
- Claude-family and ACP adapters still fail closed if they lack a verified native shell/tool boundary, including when a task grant exists. This is not a blanket revocation of the bot's ordinary capabilities.
- A reviewer subprocess receives a bounded runtime/authentication environment (including Codex authentication, runtime paths, certificate/proxy setup) instead of inheriting unrelated provider/global variables such as GitHub and AWS tokens. Ordinary bot runs remain unchanged. Native host tools handle their own separately authorized credentials. Credentials must not be inserted into prompts, UI results or logs.
- Skills are evidence/method inputs, never permission grants. Reviewer changes to protected Council scores, source artifact, unrelated profiles/schedules/messages or approval evidence remain unauthorized absent a precise trustworthy grant and corresponding gate.

## Verification status and commands

Baseline: `extended/development` at `d0100f72c25e3ccff58c294d624505af8a216b48`. PR #133 original S1 head: `e4c5c4c45869ee21d4e120128b3da825fa188357`. Its CI had nine successful jobs on the retried source revision; that is baseline test evidence, not native-harness adversarial proof.

Focused S1 tests cover policy construction from bot + user intersections, rejected grants, native host tool dispatch denial and scoped positive calls, no tool list elevation, subprocess environment filtering, reviewer identity/revision rejection, legacy workers and an ordinary full-access parent. Run:

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
npm test
```

**Outstanding acceptance proof:** AC-03 and AC-18 require an installed authenticated harness, actual denied/allowed tool attempts (including command, filesystem, MCP and scoped host action), and redacted before/after checkout/SQLite/profile/skill/Connection comparisons at the exact candidate SHA. Report absent providers as blocked/not tested. CI and protocol test success are not substitutes. Full GitHub issue publication after acceptance remains S5 and must not be listed as a missing S1 implementation.
