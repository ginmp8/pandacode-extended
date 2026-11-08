# Optional public RhapsodIA skills for PandaCode Extended

[Issue #125](https://github.com/ginmp8/pandacode-extended/issues/125) is the source specification. [PR #127](https://github.com/ginmp8/pandacode-extended/pull/127) contains an in-development implementation against `extended/development`.

## Distribution and licensing

[RhapsodIA](https://github.com/ginmp8/rhapsodia/tree/main/skills) is an optional external repository, **not** the upstream PandaCode author. PandaCode Extended is a derivative of [the original PandaCode project](https://github.com/aicodeking/pandacode) and preserves its inherited [LICENSE](../../LICENSE). RhapsodIA's root Apache-2.0 notice does not override individual skill licenses, asset rights or required third-party notices.

**Installers never embed RhapsodIA skill packages.** The app includes only a versioned JSON recommendation list. Prior plans for release-time SHA-locked downloads and offline bundled packages were superseded by the final #125 specification. Do not add CI retrieval, release packaging or skill payload export for this capability.

## Recommendations

`src/recommended-skills.v1.json` provides 15 primary references, grouped into core engineering, diagrams, spec conformance and audit specialists, plus four optional entries. Paths, not personal ChatGPT plugins or local runtime URIs, identify each skill. The initial catalog explicitly excludes `mago`, `magia` and `nomia` from recommendations. Source availability is checked against the public Git tree: `agent-systems-best-practices` was absent when this branch was authored, so its optional recommendation is displayed as unavailable instead of claiming that it can be imported.

Install the [GitHub CLI](https://cli.github.com/) and authenticate with `gh auth login`. In **Settings → Skills & rules → Recommended skills**, choose individual available packages or **Install all primary skills**. The catalog is an opt-in import, **not** automatic activation. The importer stores the full package (within bounded limits), source path, selected ref, resolved commit and file hashes. A user may select a full commit SHA instead of the moving `main` branch.

## Synchronization

In **Synchronization**, manual buttons act on one installed skill, one installed source or all installed GitHub skills. The optional **on startup** policy is off by default and requires source trust. A full-SHA pin never auto-moves. Updates are atomic at package level and can overwrite locally edited **library** files without a backup. Native harness copies remain separate and are not silently overwritten. Source errors and changes are reported to the user.

Explicitly trusted root/collection discovery and installation of newly added, safe packages are implemented behind the off-by-default startup setting. Duplicate-ID source choices, persistent per-bot auto-inheritance exclusions, coalesced overlapping synchronizations, and explicit one-time native harness replacement remain acceptance gaps on this draft PR. The current application does **not** guarantee per-bot filesystem isolation. No imported content is executed merely by installation.

See [Skills Library](../skills-library.md), [Extended attribution](attribution-and-licensing.md), and [the current issue acceptance checklist](https://github.com/ginmp8/pandacode-extended/issues/125).
