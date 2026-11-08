# Skills Library and rules registry

PandaCode Extended's **Settings → Tools → Skills & rules** contains a file-backed library and import, source configuration and synchronization views. The new configuration-based import is developed on `feat/125-configured-github-skill-sources`, based on [PR #127](https://github.com/ginmp8/pandacode-extended/pull/127). It has not been released.

## Settings views

- **Library & activation:** import local skills and rules, inspect the library and safety findings, manually enable or disable installed items for supported harnesses and user/project scopes, and check native-harness drift. Legacy single-package GitHub import and repository catalogs remain supported.
- **Import from GitHub:** supply a repository (`owner/repo` or `https://github.com/owner/repo`), optional skills folder path (for example `skills`), and optional branch, tag or full commit SHA. Choose **List skills**. The signed-in GitHub CLI reads the repository's Git tree and matching `SKILL.md` metadata without a local clone. Use checkboxes to select any subset, then **Import selected and save source**. No packages are imported by merely listing them.
- **Synchronization:** sync all installed GitHub skills, one installed skill, or the selected paths in an individual saved source. Turn on **Automatically synchronize imported skills when PandaCode starts** to opt into nonblocking startup checks. This does not discover or install new packages.

No recommendation or bundled skill list is provided. Any accessible GitHub repository with valid skill packages can be used; RhapsodIA is one possible source, not a required or privileged source.

## Source configuration lifecycle

Each saved configuration identifies a **repository + folder path**, the Git ref, the scanned commit and only the successfully selected/imported skill paths. Separate folders in one repository may be stored independently. **Edit selection** rescans that configuration and permits a different selection; it does not silently remove previously imported copies. **Remove source config** only removes its metadata, after confirmation. Previously imported skills remain in the library until removed through normal library controls.

Only explicitly confirmed configurations are written to `<app data>/skills/.skill-sources.json`. Scanning a source produces a transient index, not a persistent catalog, clone, Git checkout or package cache. The system limits this configuration store to **64 sources**. It does not create disposable Git repositories or retain unchecked/unused skill packages.

The legacy Catalogs panel (from upstream functionality) retains its own independent `.catalogs.json` metadata and is not converted automatically. Existing imports, configurations and native harness folders are preserved.

## GitHub CLI, provenance and trust

Install and authenticate the [GitHub CLI](https://cli.github.com/): `gh auth login` and `gh auth status`. PandaCode invokes `gh api` to resolve commits and read tree/blob data at an immutable commit SHA. An accessible private repository requires authentication. GitHub tokens are not persisted in source configurations. **No imported script is executed during listing, download or installation.** Import never activates a skill automatically.

The selected path must be inside the scanned source folder, present at the selected commit, and contain valid skill frontmatter. A selection with two paths having the same effective skill ID is rejected; a skill already installed from another source is not silently overwritten. Imports preserve package supporting files and reject unsafe names, filesystem collisions, oversized files and invalid content. The per-package bounds are **256 files and 24 MiB**; source enumeration is bounded at **20,000 tree entries** and **300 indexed skills**. GitHub tree results marked truncated are rejected. If the source is larger, choose a smaller repository or reduce its indexed content; there is no hidden full-clone fallback.

A SHA-locked ref will remain fixed on automatic sync. The source records the branch/tag (if selected) and resolved commit; library metadata contains per-file hashes. Importing a skill doesn't grant additional permissions or guarantee that its declared dependencies are present.

## Storage and backward compatibility

```text
<app data>/skills/<id>/SKILL.md          installed skill and supporting files
<app data>/skills/<id>.json              source, hash, activations, safety and sync metadata
<app data>/skills/.skill-sources.json    explicitly saved GitHub repo/path/selection configs
<app data>/skills/.catalogs.json         pre-existing legacy repository catalog
<app data>/skills/.sync-policy.json      global opt-in, legacy trust fields, per-skill overrides elsewhere
<app data>/skills/.sync-report.json      last manual/automatic sync report
```

Existing metadata is read without a migration. Missing sync state means **disabled**. Old trusted-source entries may remain in `.sync-policy.json` for backward compatibility, but **do not authorize startup discovery or installation** in this workflow.

Synchronization updates the central library only and preserves the previous complete package on a fetch/validation failure. When successful, it **can replace locally edited library files without keeping a backup**; native harness copies are not silently overwritten. Sync errors are isolated per skill and reported. Only one synchronization is allowed to write at a time; overlapping requests may return a busy report.

The global automatic-sync toggle is the master switch. Per-skill **Disabled** suppresses startup updates for that skill, regardless of the global setting. When the global switch is off, a per-skill **Enabled** override does not initiate network requests. Manual synchronization remains available. Newly added skills in a repository require **another explicit scan, selection and import**.

The pre-existing skill adapters and per-bot prompt behavior are not redesigned by this change. Native harnesses can expose shared skill folders across bot sessions: the library does not claim per-bot security isolation. Some issue #125 acceptance criteria, including independent active-source choice for duplicate IDs and one-time native harness overwrites, remain outside this feature branch.

See [source notes](extended/rhapsodia-skills.md) and [attribution and licensing](extended/attribution-and-licensing.md).
