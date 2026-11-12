# S3: Task-scoped reviewer skills

S3 code and automated verification are recorded in issue #126 at `cfad39aeb26cc6bf1fbe0c0f81e566772b1db59f` (CI #42881606678). Live provider-credential and interactive Desktop checks remain unexecuted and are not implied by the automated result.

## Ownership and scope

S3 reuses the existing central Skills Library and the S2 native reviewer seat.
It does not create a second skills catalog, install skills into the assessed
checkout, activate a harness-global skill, change a bot's ordinary configuration,
or grant new shell, MCP, filesystem write or network permissions.

A **library import** is not an assignment. Only the explicit skill IDs on the
selected bot and their required skill dependencies are candidates for a reviewer.
Project/global activation is not required for those bot assignments. Council
without selected bots, or a selected bot with no assigned skills, retains the
existing native review behavior.

## Frozen input and provenance

At the first selected-bot dispatch for a Council round, the native adapter
resolves the complete selected roster, including skill content. It records
this before issuing a seat, in the same persisted round snapshot as reviewer
identity, model, instructions and tool policy.

The per-skill receipt contains:

- Central library ID, source kind/location, source ref and resolved GitHub commit
  (when GitHub-backed), whole-package SHA-256, and the digest of every file
- Required skill dependencies, whether explicitly assigned to the bot, and
  the effective delivery mode
- Distinct `imported`, `botAssigned`, `eligible` and `supplied` states
- Explicit `invoked`, `checkExecuted` and `evidenceVerified` booleans,
  initially **false**, never inferred from a model report

A later bot profile or library import replacement does not rewrite an already
issued round. Restarts revalidate the frozen native authority and any staged
supporting files rather than silently replacing them with a current library
version. Legacy selected rounds that name assigned skills without a frozen
receipt fail closed and require explicit new-round recovery.

## Trust, dependency and size gates

The resolver rejects a missing or invalid package, file/hash drift, untrusted
safety findings, secret-shaped content, missing required dependency, dependency
cycle, mutable GitHub source with no 40-character resolved commit, unsafe
supporting-file path, or incomplete/non-UTF-8 mandatory prompt material.

Skill text is bounded to **16,384 characters per skill** and the combined
reviewer skill prompt to **65,536 characters**. Oversized inputs fail visibly
with `reviewer_skill_truncated`; content is not silently truncated.

For Claude and Codex, package files are copied to content-addressed, read-only
directories under the app-managed central Skills Library, outside the assessed
checkout. The exact staged path and the complete bounded `SKILL.md` text are
included in the reviewer briefing. Other recognized harnesses receive a
bounded lower-trust prompt including UTF-8 supporting-file content. Binary
supporting files that cannot be safely represented by these harnesses fail
closed. Skill scripts are never executed as part of provisioning.

Provisioning errors use `reviewer_skill_unavailable`,
`reviewer_skill_untrusted`, `reviewer_skill_drift`,
`reviewer_skill_cycle`, `reviewer_skill_source_unpinned` and
`reviewer_skill_truncated`. Native tool permissions remain governed
independently by the task-scoped reviewer policy, regardless of what a skill
claims or requests.

## Automated validation

Run against the exact candidate commit:

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib reviewer
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib selected_reviewer_skills
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
npm test
npx playwright test tests/council.spec.ts --workers=1
```

Additionally require the complete PR CI matrix at the same HEAD. The focused
unit cases cover assigned-only resolution, dependency closure and cycles,
content drift, prompt overrun, read-only staging, immutable native round
snapshots, and forged authority records.

## Evidence and unfinished acceptance

This document describes the implementation under review, **not a completed
S3 acceptance certificate**. Source/automated tests and CI must pass at the
exact HEAD before code sign-off. A model claim, displayed slash command or score cannot establish actual
skill invocation or executed checks. The original immutable skill input flags
stay false. Instead, the native reviewer ToolHost now exposes two independent,
task-scoped tools: `pandacode_reviewer_skill_open` returns the frozen lower-trust
skill instructions and records native retrieval; `pandacode_reviewer_skill_verify`
checks the pinned content integrity and records an independently observed
native check. The mutable receipt is bound to the seat, reviewed artifact
revision and package SHA-256, separate from the frozen native authority.
Model-authored claims or slash-command availability do not create such events.

Mandatory *supply* of skill methodology is independent from an explicitly
required native integrity check. The former blocks on unavailable skill inputs;
the latter blocks until an authentic native check is observed. A successful
package-integrity check does NOT demonstrate that arbitrary skill scripts,
project tests, or methodology-dependent behavior actually executed. Such
additional checks require their own independent evidence.

The current implementation has not yet demonstrated live harness discovery of
staged paths or a complete lifecycle cleanup of old staged packages. These
are code/runtime follow-up items, not grounds to claim a passed AC-11,
AC-15, AC-16, AC-17 or AC-22. Live authenticated Desktop validation is
`not_run — user-deferred` under issue #126's owner-approved stage gate.
