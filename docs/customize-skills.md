# Customize → Skills (Extended)

Customize is a standalone, scrollable workspace opened from the main sidebar's top action card, immediately below Search. Its plug icon identifies workspace customization. **Skills** is the only active category. The existing **Settings → Tools → Skills & rules** page remains available for the original rules, catalogs, harness activation and drift matrix; both surfaces share the **same central native skills library**.

## Browse and preview

Open **Customize → Installed** to search by skill name, ID, description or origin. Selecting a row shows the installed package with a sanitized **Rendered** Markdown preview, **Raw** source, and a read-only list of package files. Binary or unsupported files show metadata instead of being executed. A GitHub discovery preview is clearly labeled **not installed** and reads the immutable commit selected by the scan.

Imported skills remain inactive until explicitly activated for a supported harness. The installed skill preview now provides user-scope enable/disable switches per installed harness, explicit safety-finding acknowledgement, an uninstall confirmation, and a Try in chat action. Project-scope activations and drift details remain in Settings → Tools → Skills & rules. Activated, current skills appear as `/skillname` suggestions alongside harness commands in regular chats, tasks and bot conversations, without replacing native harness commands. Native harness copies, bot permissions, approval policies and running instructions are not modified by importing or synchronizing.

Installed skill rows use a safe, inert SVG thumbnail when the package provides `assets/icon.svg`; otherwise a cube symbol is shown. Safety finding counts appear alongside skill names, not in the right action group.

### Activation, uninstall and trying a skill

Customize shows an immediate loading message while the Skills UI module loads, then a loading indicator while initial library, activation and source information is read; it never displays an empty-library message before that read finishes. Refresh preserves existing items while rechecking. Successful activation and deactivation display a brief accessible notification; failures retain the existing activation and show an actionable error.

Open an installed skill to **Enable** or **Disable** it for any available harness at user scope. Risk findings must be reviewed and explicitly acknowledged before enablement. Project-scope activations remain available in Settings. A stale or edited file may require a separate reactivation; PandaCode does not silently overwrite changed harness files.

Choose **Try in chat** to open a **new projectless chat** using one of the harnesses for which the skill is enabled at user scope. The composer is prefilled with `/skillname ` and receives focus. Nothing is submitted or executed automatically.

**Uninstall** requires a second confirmation and every activation must first be disabled, including project-scoped ones. This removes the central library package, not saved GitHub source configuration. Previously removed packages are not silently reinstalled by startup synchronization.

## Add a ZIP skill

Choose **Add → ZIP archive** and select one local archive with the system file picker. It must contain one skill package, optionally within one wrapper directory, with a valid `SKILL.md` declaring frontmatter `name` and `description`.

The reader enforces a 32 MiB archive limit, the central library's per-skill 256-file/24 MiB uncompressed limits, bounded entry counts and decompression ratios, safe relative paths, Windows reserved and case-colliding names, and rejection of symlinks/special files. Hidden supporting paths are retained, except for the reserved `.git` directory. A ZIP containing multiple skill roots is rejected. Archive scripts are never executed; no clone or accumulated temporary archive is retained.

Installing a ZIP with an existing canonical skill ID first **requires a separate explicit confirmation**. Only after confirmation may PandaCode **replace the central library package atomically**. The persistent result shows the previous origin and existing harness activations that may require reconciliation. ZIP replacement detaches the installed entry from automatic GitHub updates until the user deliberately reimports the GitHub origin. No native harness file is silently overwritten.

## Add skills from GitHub

1. Install [GitHub CLI](https://cli.github.com/) and sign in with `gh auth login`. PandaCode uses that existing authenticated `gh api` access for both public and authorized private repositories; no app-specific token field is added.
2. Choose **Add → GitHub link**. Enter only the required HTTPS repository URL and, optionally, a branch or full 40-character commit SHA. A supported GitHub `/tree/<ref>/<directory>` URL scopes discovery without an extra path input.
3. Click **Scan repository** to resolve an immutable commit and recursively find valid `SKILL.md` packages at any tracked directory level. No package is installed during scanning.
4. Filter by name/path, **Select shown**, clear or select individual checkboxes, and review a package with **Preview**.
5. Click **Import selected (N)**. PandaCode fetches only the selected package paths and their valid supporting files, verifies the pinned commit and installs into the central library. An installed ID from another source requires explicit replacement approval.
6. The persistent operation panel shows actual terminal-count progress, the current package, stages, per-package failures, a final installed/unchanged/skipped/failed summary, and **Retry failed** for unsuccessful paths only.

Repository enumeration resolves explicitly scoped subtrees before scanning, including `.github/skills` and `.agents/skills`. Hidden supporting files are preserved in selected packages, except the reserved `.git` directory. When the recursive Git API is truncated, a bounded non-cloning tree walk is attempted; limits and incomplete results are reported as errors, not quietly omitted. Each selected package download is capped at 512 API requests, 512 queued directories and five minutes; the previous installed copy remains intact on a rejected import.

## Saved sources and synchronization

**GitHub sources** lists saved repository, requested ref, resolved commit, selected paths and update configuration. Different refs or subtrees can have separate entries.

- **Edit selection** rescans the source. **Save tracked selection** changes tracked installed packages without deleting them. First import any new package that should be tracked. **Remove source** deletes only the source configuration, not library files. Removing a source with an active import is blocked until the operation completes. Import exclusion is scoped to the central library directory, so unrelated library profiles do not block each other.
- **Sync** updates an individual GitHub skill, **Sync source** updates installed packages in one saved selection, and **Sync all GitHub skills** covers installed GitHub-backed skills. Each terminal result shows its old/new revision (when known), error or skip reason, and affected native harness activations. Concurrent synchronization attempts show an explicit failure instead of a false completion.
- **On startup** is **off by default**. When explicitly enabled it checks *only previously imported skills still selected in saved sources*, respecting per-skill opt-outs. There is no polling, newly discovered packages are never auto-installed, and a full SHA remains pinned. Removing a source stops that source's automatic tracking.
- Manual synchronization is available even with automatic startup synchronization off. Updates affect the central library only; existing harness copies must be reconciled using the original Settings screen.

## Errors and recovery

| Problem | Resolution |
|---|---|
| GitHub CLI unavailable or not authenticated | Install `gh`, run `gh auth login` and check `gh auth status`. |
| Invalid repo, ref, missing private access | Verify the URL, SHA and access through your GitHub CLI account. |
| Truncated or over-budget tree | Scan a narrower GitHub tree URL. |
| ZIP has unsafe paths, too many files, special entries or multiple packages | Repackage as one valid skill directory within limits. |
| Another origin already owns the installed skill ID | Review the source and explicitly approve a replacement. |
| Import partially succeeded | Read the preserved per-package errors and retry only unsuccessful paths. |
| Central library changed but harness is outdated | Reconcile or reactivate through Settings; installed harness files were not rewritten. |

## Architecture and non-goals

The native GitHub discovery/source persistence/sync engine and regression tests are selectively reused from [PR #128](https://github.com/ginmp8/pandacode-extended/pull/128); its Settings-based prototype UI is not copied. Tauri skills commands and their original shared library remain authoritative. This feature does **not** implement Agent/Plugin management, a plugin runtime, a Git working-tree clone, automatic activation, or changes to existing bot permission boundaries.

Optional skill packages retain their own copyright and licenses separately from PandaCode Extended's application attribution.

## Reliability and verification

The GitHub import operation uses one immutable commit. During manual or startup synchronization,
a moving repository/ref is resolved once per batch, preventing mixed revisions within that source.
An interrupted import leaves a bounded recovery intent; the next read reconciles only committed
packages that match its reviewed selection, source and commit.

The UI distinguishes Installed, Replaced, Updated, Unchanged, Skipped and Failed. It uses
authoritative terminal per-package outcomes and flags multi-item operations as **Completed with errors** when native incremental events are missing, rather than simulating progress or announcing unconditional success. Only failed items are eligible for Retry failed. ZIP replacement
suspends old GitHub automatic tracking and never rewrites native harness copies. A source ref
change requires a new scan and explicit selected-package import.

SKILL.md previews show untrusted Markdown without executing interactive message widgets or
loading remote images. ZIP frontmatter checks required string scalars, multiline descriptions,
and optional nested metadata/maps/lists with bounded inline-flow validation; malformed nested
collections and duplicate required keys are rejected before staging files. Arbitrary YAML
objects cannot be used as skill names or descriptions.

The CI browser job publishes the `customize-issue-129-ui-review` artifact, with
before/after navigation and Customize GitHub/ZIP screenshots from the preview
harness. Those screenshots are design review evidence, **not** native packaged
Windows/macOS/Linux desktop acceptance. Focused Rust tests exercise native
package import and the shared Tauri progress event payload contract. The
Windows CI job runs the dedicated native catalog, ZIP and GitHub acquisition
regression tests, alongside the Linux and macOS jobs.

## Visual review observations

The browser preview CI job captures both the workspace before opening Customize
and the standalone Customize view after navigation, plus GitHub discovery and
ZIP-install states. The Customize hub hides the underlying workspace content
visually and from keyboard focus without disposing of chats or running tasks.
Checkboxes in skill discovery explicitly use control-sized geometry rather
than inheriting the application's full-width generic input rule; automated
layout assertions verify readable skill rows and keyboard focus retention.

These captures are preview-harness evidence; packaged desktop acceptance is
separate. The CI artifact is named `customize-issue-129-ui-review`.
