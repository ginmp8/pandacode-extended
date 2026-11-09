# Customize → Skills (Extended)

Customize is a standalone, scrollable workspace immediately above Bots in the PandaCode left sidebar. **Skills** is the only active category. The existing **Settings → Tools → Skills & rules** page remains available for the original rules, catalogs, harness activation and drift matrix; both surfaces share the **same central native skills library**.

## Browse and preview

Open **Customize → Installed** to search by skill name, ID, description or origin. Selecting a row shows the installed package with a sanitized **Rendered** Markdown preview, **Raw** source, and a read-only list of package files. Binary or unsupported files show metadata instead of being executed. A GitHub discovery preview is clearly labeled **not installed** and reads the immutable commit selected by the scan.

Imported skills remain inactive until explicitly activated for a supported harness through Settings. Native harness copies, bot permissions, approval policies and running instructions are not modified by importing or synchronizing.

## Add a ZIP skill

Choose **Add → ZIP archive** and select one local archive with the system file picker. It must contain one skill package, optionally within one wrapper directory, with a valid `SKILL.md` declaring frontmatter `name` and `description`.

The reader enforces a 32 MiB archive limit, the central library's per-skill 256-file/24 MiB uncompressed limits, bounded entry counts and decompression ratios, safe relative paths, Windows reserved and case-colliding names, and rejection of symlinks/special files. A ZIP containing multiple skill roots is rejected. Archive scripts are never executed; no clone or accumulated temporary archive is retained.

Installing a ZIP with an existing canonical skill ID first **requires a separate explicit confirmation**. Only after confirmation may PandaCode **replace the central library package atomically**. The persistent result shows the previous origin and existing harness activations that may require reconciliation. ZIP replacement detaches the installed entry from automatic GitHub updates until the user deliberately reimports the GitHub origin. No native harness file is silently overwritten.

## Add skills from GitHub

1. Install [GitHub CLI](https://cli.github.com/) and sign in with `gh auth login`. PandaCode uses that existing authenticated `gh api` access for both public and authorized private repositories; no app-specific token field is added.
2. Choose **Add → GitHub link**. Enter only the required HTTPS repository URL and, optionally, a branch or full 40-character commit SHA. A supported GitHub `/tree/<ref>/<directory>` URL scopes discovery without an extra path input.
3. Click **Scan repository** to resolve an immutable commit and recursively find valid `SKILL.md` packages at any tracked directory level. No package is installed during scanning.
4. Filter by name/path, **Select shown**, clear or select individual checkboxes, and review a package with **Preview**.
5. Click **Import selected (N)**. PandaCode fetches only the selected package paths and their valid supporting files, verifies the pinned commit and installs into the central library. An installed ID from another source requires explicit replacement approval.
6. The persistent operation panel shows actual terminal-count progress, the current package, stages, per-package failures, a final installed/unchanged/skipped/failed summary, and **Retry failed** for unsuccessful paths only.

Repository enumeration resolves explicitly scoped subtrees before scanning. When the recursive Git API is truncated, a bounded non-cloning tree walk is attempted; limits and incomplete results are reported as errors, not quietly omitted.

## Saved sources and synchronization

**GitHub sources** lists saved repository, requested ref, resolved commit, selected paths and update configuration. Different refs or subtrees can have separate entries.

- **Edit selection** rescans the source. **Save tracked selection** changes tracked installed packages without deleting them. First import any new package that should be tracked. **Remove source** deletes only the source configuration, not library files.
- **Sync** updates an individual GitHub skill, **Sync source** updates installed packages in one saved selection, and **Sync all GitHub skills** covers installed GitHub-backed skills. Results describe old/new revisions, unchanged/failed packages and affected native harness activations.
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
authoritative terminal per-package outcomes and explicitly reports missing incremental events,
rather than simulating progress. Only failed items are eligible for Retry failed. ZIP replacement
suspends old GitHub automatic tracking and never rewrites native harness copies. A source ref
change requires a new scan and explicit selected-package import.

SKILL.md previews show untrusted Markdown without executing interactive message widgets or
loading remote images. ZIP frontmatter validation rejects malformed required fields and
duplicate name/description keys before staging files.
