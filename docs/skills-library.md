# Skills library, GitHub catalogs and scoped selections

The existing **Settings → Skills & rules** page owns all three flows. Imports are inert.
A skill reaches a harness only after the user activates it or saves a bot/project selection.
No discovery step runs a repository script, installs a package, changes permissions, or starts an agent.

## Where to use it

1. **Library** imports a local skill/rule or a specific GitHub path and retains the existing
   user/project activation matrix for each harness.
2. **GitHub catalogs** scans an explicitly named `owner/repository`, including nested
   `SKILL.md` folders. Select one or several packages and choose **Import selected**.
3. **Bot & project skills** selects a bot or project and pins the current versions with
   **Save skill selection**. Review any findings before accepting them.
4. **Current task: effective skills** shows the union used by the current task, with
   `bot`, `project`, `both`, or existing `user` provenance and `native`, `prompt`, or
   `skipped` delivery. Refresh the preview after changing a file-based activation.

Bot selections apply to that bot's conversations and other tasks in its own workspace.
Project selections apply to tasks whose current project id matches the selection. A task's
actual bot/project is resolved natively during execution admission; frontend-provided
context does not choose another task's skills. Unrelated projects receive no bot additions.

## Catalogs and provenance

Catalog registration and refresh use the already installed GitHub CLI and its existing
GitHub.com login. The user initiates each read. A catalog records repository, optional ref,
immutable 40-hex commit, scan time, package paths, frontmatter, file counts and byte counts.
An import reads the exact indexed commit, records `owner/repository/path`, ref, commit and
the library's full SHA-256 content hash, and runs the ordinary safety scan.

A repository tree may have at most 20,000 entries and 128 skills. A package may have at
most 64 files / 1 MiB. Description reads and one import batch have an 8 MiB aggregate
budget. Each underlying GitHub call is bounded by the existing runner; operations check
a 60-second budget between calls. Truncated trees, invalid object ids, invalid paths,
symlinks, submodules, unsupported encodings and inconsistent blob lengths are rejected or
excluded before packaging. A nested skill is a separate package: its files do not leak
into its parent skill's import.

**Refresh catalog** only updates the index. A different imported commit appears as
**Update available**; no library copy, native activation or scope selection is updated
in the background. **Disabled** and **Ignored** catalogs cannot refresh or import.
**Remove catalog** removes only the index; imported copies remain in the library.

Name conflicts are shown per package. Importing a different source under an existing id
is refused. The library also refuses to overwrite a changed local hash, an unmanaged
folder, a symlink or added hidden content. Batch import reports each imported/skipped
package separately; partial success is visible. Ordinary same-source reimports retain
activations but leave them outdated until the user explicitly activates the new version.

## Native folders and scoped fallback

| Harness | Skill, user | Skill, project | Rule, user | Rule, project |
|---|---|---|---|---|
| Claude Code | `~/.claude/skills/<id>/` | `<checkout>/.claude/skills/<id>/` | block in `~/.claude/CLAUDE.md` | block in `<checkout>/CLAUDE.md` |
| Codex | `~/.codex/skills/<id>/` | `<checkout>/.codex/skills/<id>/` | block in `~/.codex/AGENTS.md` | block in `<checkout>/AGENTS.md` |
| Other registered harnesses | bounded prompt | bounded prompt | bounded prompt | bounded prompt |

The existing adapters retain their protections against overwriting unmanaged/edited
files. Project activations still use the explicit checkout/worktree supplied to the
existing activation command, and managed Git exclusions remain unchanged.

Bot/project selections are an additional, harness-independent choice. They carry only
approved main-file text in the current turn's prompt, without writing files into shared
checkouts. If the same current hash already has a healthy native activation for this
harness and checkout, the native folder is used and no duplicate text is injected. A
native activation with another version or changed files is reported for review instead
of injecting a competing version. Its files are never silently replaced.

The fallback has an 8 KiB per-item budget and a 32 KiB aggregate context budget (including its
short heading). Truncated text says it was truncated. Items that do not fit are reported
as skipped. Supporting files remain available in the library/native activation but are
not automatically read outside the project or copied for a bot-specific selection.
This means a skill that depends on supporting files may require explicit native project
activation; the fallback does not claim full native skill-directory behavior.

The final prompt is assembled under the native task-admission lock before saving the
new user turn. User history retains the user's original text. A per-task fingerprint invalidates the native resume id when effective skill versions
change, disappear or become unavailable. The existing history recovery then restores
user/assistant messages without restoring the old synthetic skill block. Changes during
a running turn apply at the next admission. Tasks using the text fallback must continue
through chat: the interactive console cannot deliver that context, and even a native
resume id does not prove a previous prompt was delivered. Healthy native file activations
can use the console after an ordinary chat turn has refreshed any changed skill context. Permission mode, tool
approval, remote authorization and the execution backend remain unchanged. Maintenance
and goal-control operations do not receive these additions.

## Versions, safety and storage

Storage stays under the existing app data directory; no SQLite migration is required:

- `skills/<id>/SKILL.md` or `RULE.md`, plus supporting regular files.
- `skills/<id>.json`: source, content SHA-256, findings acknowledgement and activations.
- `skills/.catalogs.json`: explicit catalog sources and scanned indexes.
- `skills/.scopes.json`: bot/project selections as `{id, sha256}` pins.
- `skills/.task-contexts/<hashed-session-id>`: effective-skill fingerprint for resume safety.

A new import never silently changes a selected pin. Missing items, modified library
content, unacknowledged findings, mismatched selected hashes or conflicting bot/project
versions are reported as skipped. Save the reviewed selection again to approve a new
version. Removal is refused while any native/prompt activation or existing bot/project
selection still refers to that skill. An explicit library removal first clears selections
whose bot/project ids are confirmed absent from the database. Existing targets keep their
selections even if a workspace folder is unavailable; database/read/write errors abort
cleanup and removal. Scope writes and cleanup hold the Store lock before the library lock,
so a target cannot be deleted between validation and saving a selection.

The safety scan flags secret-shaped values, attempts to disable approvals/sandboxing,
and downloads piped into a shell. Acknowledgement is explicit and specific to the
content hash. It is an inspection aid, not proof that a skill is safe.

## Repository catalogs and discovery (#114, #115)

`skills/catalog.rs`. A catalog source is a GitHub repository indexed for skills, kept apart from the library and from activation.

- Scanning reads the commit for the ref, then one recursive Git tree. Every `SKILL.md` (at the root, `skills/<name>/`, or deeper in nested collections) counts only when its frontmatter has both `name` and `description` (YAML `>`/`|` block values work). A repository named "skills" without such a file is never registered. Hidden paths are skipped. A skill's files are its folder minus any nested skill folder.
- Importing copies the chosen skills (or all of them) at the indexed commit, each with its supporting files and `owner/name/path`, ref and commit as its source. Nothing is activated. The 64-file and 1 MB limits apply per skill, and a skill over them is skipped with a reason. Per-catalog limits: 20,000 tree entries, 300 `SKILL.md` files, and a truncated GitHub tree, each refused with a clear error.
- Same names: each indexed skill lists other catalog paths and library items with the same id (`conflicts`). An import whose id the library already holds from another source is skipped, never overwritten.
- Refresh re-reads the index only. A library copy keeps its commit (`imported`), and the UI says when a newer version is upstream. Importing again updates the library, and harnesses then show `outdated`, as before.
- Discovery (`skills_discover`) uses the same `gh` login as GitHub import, with no new scopes. It lists `user/repos?affiliation=owner` 100 at a time, up to 10 pages; organizations are not scanned. Matching repositories are registered as `discovered` sources. Ignored and disabled sources are left alone, unchanged repositories (same `pushed_at`) are not read again, and empty ones are skipped, so repeating discovery is idempotent. A rate limit or sign-in failure stops discovery with a clear message after saving progress; other per-repository failures are listed in the report.
- Settings shows the sources with Refresh, Disable/Enable and Ignore (ignored sources are listed with Stop ignoring), each source's skills with checkboxes, and Import selected / Import all.

## Commands

Existing commands remain available: `skills_list`, `skills_import_folder`,
`skills_import_github`, `skills_activate`, `skills_deactivate`, `skills_matrix`,
`skills_scan` and `skills_remove`.

New commands:

| Command | Purpose |
|---|---|
| `skills_catalogs()` | Read catalog indexes without network access |
| `skills_catalog_add({repo, gitRef?})` | Explicit discovery/registration |
| `skills_catalog_refresh({repo})` | Refresh index only |
| `skills_catalog_state({repo, state})` | Enable/disable/ignore; `null` removes index |
| `skills_catalog_import({repo, paths})` | Import exactly the selected paths |
| `skills_scope_get({target, targetId})` | Read bot/project hash pins |
| `skills_scope_set({target, targetId, ids, acknowledge?})` | Explicitly save reviewed versions |
| `skills_task_resolve({sessionId})` | Read effective provenance/delivery and skip reasons |

Catalog mutation/network reads and scoped selection writes require the main local
window. New commands are not exposed through remote control, bot channels or the tray.

## Validation and limits

Fixture tests exercise nested discovery, pinned imports, disabled/ignored sources,
source collisions, library edits, unsafe paths, truncated trees, size limits, scoped
provenance, stale versions, explicit safety consent, bounded context and real native
execution admission. Browser tests exercise the existing library and new catalog/scope
controls through the preview IPC layer. Live GitHub and real provider execution require
an installed authenticated CLI and are separate integration checks.
