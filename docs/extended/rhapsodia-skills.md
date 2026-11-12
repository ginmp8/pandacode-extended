# Public RhapsodIA skills in PandaCode Extended

[Issue #125](https://github.com/ginmp8/pandacode-extended/issues/125) defines two complementary capabilities: manual GitHub-based skill import and a deterministic skill bundle for future Extended release installers.

## Public repository and scope

- **Source:** [ginmp8/rhapsodia](https://github.com/ginmp8/rhapsodia), [`skills/` directory](https://github.com/ginmp8/rhapsodia/tree/main/skills).
- **Licensing:** The source repository includes Apache-2.0 at the root. Each selected package must be checked for its own license and third-party notices.
- **Release reproducibility:** The release pack must lock a full Git commit SHA and validate individual package bytes, independent of any future changes to `main`.

Examples of public directories available in RhapsodIA include `skills/software-improvement-loop/`, `skills/systematic-debugging/`, `skills/archify/`, `skills/archify-review/`, `skills/reproducibility-engineer/`, `skills/audit-spec-implementation/` and the five stage packages used by the spec-conformance coordinator.

## Today: existing GitHub import and catalog selection

The current Skills Library already uses the locally authenticated **GitHub CLI** (`gh api`), records source revisions, and supports catalog-level **Import selected** and **Import all**. Imported skills are not automatically activated or executed.

1. Run `gh auth status` in the user account that launches PandaCode. Follow `gh auth login` if authentication is missing.
2. In **Settings → Skills & rules**, add `ginmp8/rhapsodia` as a catalog source and select an approved ref.
3. Import the selected packages, or import one exact path using the existing GitHub source form: repository `ginmp8/rhapsodia`, path `skills/archify`, and an optional immutable ref.
4. Review safety findings and explicitly select harness and scope before activating any package.

Current import limits still apply to each individual package: 64 files and 1 MiB. The catalog is an index, not a live synchronization of installed files.

## Planned: one GitHub URL for a single package or collection

[Issue #125](https://github.com/ginmp8/pandacode-extended/issues/125) specifies a URL-aware extension to the existing importer:

| Input | Expected future interpretation |
| --- | --- |
| `https://github.com/OWNER/REPO` | Index valid skill packages in the repository; preview and import selected/all |
| `https://github.com/OWNER/REPO/tree/REV/skills` | Index packages under `skills/`; preview and import selected/all |
| `https://github.com/OWNER/REPO/tree/REV/skills/SKILL_ID` | Import the single validated package at that path |

A path containing an ambiguous ref must be resolved explicitly instead of guessing. Repository roots must not be copied indiscriminately. Acquisition must continue through `gh api`, preserving identity, conflict and permission checks; existing catalog and bot JSON import behavior must remain unchanged.

## Planned: offline release pack

A reviewed manifest will select exact RhapsodIA skill IDs and an immutable source commit. CI will package the complete selected directories, verify their hashes and notices, and embed the same bundle in each supported installer. The future installed package must remain offline-readable and distinguish **available** from **activated**.

**Status:** Only the existing manual GitHub import/catalog functionality is presently documented as implemented. URL-aware collection input, source-specific enhancements and SHA-pinned installer bundling remain pending issue #125 acceptance and release validation.
