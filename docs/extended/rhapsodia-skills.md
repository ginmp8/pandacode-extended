# GitHub-based skill sources (RhapsodIA example)

The **Import from GitHub** view is source-neutral. Users enter their own repository, folder and optional Git ref; PandaCode does not bundle or suggest a fixed list of skills.

For example, the public [RhapsodIA repository](https://github.com/ginmp8/rhapsodia) may be scanned with repository `ginmp8/rhapsodia`, path `skills` and ref `main`. The application uses the authenticated GitHub CLI (`gh api`) to list valid `SKILL.md` packages. Users select the desired entries in a multi-select list and confirm installation. One source configuration records that repository, path, ref, imported commit and chosen paths, without cloning the repository.

The same workflow works with other authorized GitHub repositories. Multiple folder paths within the same repository may be saved separately. Removing a saved configuration does not remove skills already installed locally.

**No RhapsodIA skills are embedded in release artifacts, sourced during CI builds, auto-selected, or installed without confirmation.** Skills remain governed by their own licenses, notices and runtime dependencies. RhapsodIA is not the original author of PandaCode Extended and its licenses do not supersede the project's [LICENSE](../../LICENSE).

Automatic synchronization is disabled by default. Enabling it updates **only already imported skills**; a newly published skill must be explicitly selected and imported. Successful source sync changes the central library copy, not an edited native harness folder.

See [Skills Library](../skills-library.md), [attribution](attribution-and-licensing.md), [issue #125](https://github.com/ginmp8/pandacode-extended/issues/125) and the parent [PR #127](https://github.com/ginmp8/pandacode-extended/pull/127).
