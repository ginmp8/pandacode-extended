# Skills Library and rules registry

PandaCode Extended's **Settings → Tools → Skills & rules** reuses the original file-backed Skills Library (issues #15, #45, #114 and #115). The additions tracked in [issue #125](https://github.com/ginmp8/pandacode-extended/issues/125) are under development in [PR #127](https://github.com/ginmp8/pandacode-extended/pull/127); do not assume they are available in a released installer.

## The three Settings views

- **Library & activation:** inspect imports and source commits, review safety findings, manually activate/deactivate each skill for supported harness and user/project scopes, and inspect installed-versus-actual harness drift.
- **Recommended skills:** a versioned, bundled **JSON metadata catalog** lists 15 primary and four optional references from [RhapsodIA](https://github.com/ginmp8/rhapsodia/tree/main/skills). No skill content is bundled or automatically installed by a recommendation. **Install selected** chooses checked packages; **Install all primary skills** excludes optional entries. A listed but unavailable source package is disabled.
- **Synchronization:** manually sync a library skill, installed source repository or all installed GitHub skills. Edit an installed skill's branch, tag or immutable full SHA; choose inherited, enabled or disabled startup updates. Global startup checking starts **off**. Trusting a source is separate from enabling startup checks. Each result reports changes or skipped/errors; prior source commits and impacted harnesses are recorded.

The available implementation is **not a guarantee of per-bot isolation**: native harnesses can load the same shared filesystem folders for multiple bot sessions. An excluded skill may remain visible through the underlying harness.

## GitHub import and source provenance

Install and authenticate the [GitHub CLI](https://cli.github.com/) in the OS account running PandaCode: `gh auth login`, `gh auth status`. The desktop importer uses `gh api`; importing private sources requires existing read authorization. It does not store GitHub tokens or install executable dependencies. **No skill script executes on import.**

Single-skill import accepts `owner/repo` with an exact path and optional ref. The Extended parser also accepts `https://github.com/owner/repo`, `/tree/main/skills/id`, or `/blob/<full-40-character-SHA>/skills/id/SKILL.md` URLs. When branch names contain slashes, provide the exact ref explicitly; the parser refuses ambiguous tree URLs rather than guessing. A repository root or collection must be indexed and packages chosen through the catalog; the individual importer does not recursively treat an arbitrary repository as one skill.

Each import resolves the chosen ref to a commit and acquires source files at that immutable revision. Source metadata includes the original ref, resolved commit and SHA-256 content hashes. Paths must be structurally safe and unique on case-insensitive filesystems. Package limits on this branch are **256 files / 24 MiB per skill**, raised to accommodate real RhapsodIA packages such as Archify, but still bounded. A GitHub repository may have up to 20,000 tree entries and 300 detected skills for catalog indexing; a truncated tree is rejected.

GitHub skill imports do not fetch executable dependencies, change application permissions or automatically enable an imported skill. An imported package may be incomplete if its declared peer skills or runtime dependencies are not installed; inspect those requirements before activation.

## Library storage and legacy compatibility

Everything remains in the app data directory, with no new database migration:

```text
<app data>/skills/<id>/SKILL.md         skill and its supporting files
<app data>/skills/<id>.json             source, immutable commit, content hashes,
                                        safety acceptance, activation and sync override
<app data>/skills/.catalogs.json        discovered and configured repository indexes
<app data>/skills/.sync-policy.json     opt-in startup and trusted repository/path scopes
<app data>/skills/.sync-report.json     previous sync outcome by skill
```

Missing sync metadata in older installs means startup sync **disabled** and per-skill policy **inherited**; existing copies, activations, edited harness files and source receipts remain in place. A sync replaces the **central library copy** including local edits, without keeping a backup (a deliberate behavior). Package writes use staging and a rollback folder to recover an interruption. Successful library imports do **not** overwrite a manually edited or external native harness file; an older harness copy can remain active until user action. Sync cannot undo changes made by executing a skill.

Native adapter surfaces are:

| Harness | User skill folder | Project skill folder |
| --- | --- | --- |
| Claude Code | `~/.claude/skills/<id>/` | `<checkout>/.claude/skills/<id>/` |
| Codex | `~/.codex/skills/<id>/` | `<checkout>/.agents/skills/<id>/` |
| Other supported harnesses | Managed prompt surface | Managed prompt surface |

PandaCode tracks the hash of its managed files and refuses accidental replacement of drifted native files. Project writes may be excluded in `.git/info/exclude`.

## Manual and startup synchronization

Manual buttons are explicit network requests. On application startup, checks run on a separate thread without blocking the UI. A startup check requires both the applicable global/per-skill policy **and** a trusted repository/path scope. Full SHA refs are immutable and are never silently moved by startup synchronization. Other refs can resolve to a newer commit.

An update always targets the library copy; already-running bots keep their existing execution context. Newly imported content may change subsequent prompts or require a user to refresh an existing harness activation. Acknowledged safety findings from the same source are not repeatedly demanded for later versions, although new warnings are still visible. Source read errors, path validation errors, size violations and renamed packages are reported per item rather than deleting the previous installed files. The current implementation serializes sync operations; a concurrent request can report that another sync is running rather than running an overlapping write.

## Current limitations on this feature branch

The source-index/catalog importer still refuses a second installed package with an identical effective ID instead of storing multiple active library candidates and offering explicit source precedence. The new one-time **overwrite harness copy** action is not yet available. The UI does not currently enforce required peer-skill dependencies, offer per-bot inherited/excluded library candidate controls, or automatically install newly discovered packages from an authorized scope. Those parts of #125 remain acceptance gaps and must not be represented as completed. The original bot manifest stores portable skill **IDs** without serializing contents; a missing ID on another machine must be installed separately.

For the scope and validation gates, see [issue #125](https://github.com/ginmp8/pandacode-extended/issues/125) and [the RhapsodIA integration notes](extended/rhapsodia-skills.md).
