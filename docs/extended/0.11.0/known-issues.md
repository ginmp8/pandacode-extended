# Extended 0.11.0 — known issues and acceptance gaps

**Status:** Initial register. An untested flow is not automatically a confirmed defect.

The previous [0.10.0 known-issues register](../0.10.0/known-issues.md) and [release checklist](../0.10.0/release-checklist.md) remain evidence sources. Validate each open concern anew against the actual 0.11.0 candidate; do not silently inherit a claim of failure or closure.

| Area | Current classification | Required evidence |
| --- | --- | --- |
| Full upstream parity under Extended controls | Not fully demonstrated for the new cycle | Whole-tree SHA/mode inventory plus shared-hunk and On/Off behavior checks |
| Native/remote integrations (bots, Connect, MCP, cloud/VM, credentials) | Acceptance pending | Supported-platform functional tests and exact-SHA links |
| Existing installations, SQLite data and upgrades | Acceptance pending | Representative persisted-data migration, rollback/recovery and native-install checks |
| Extended packaging/version identity | Previous 0.10.0 distribution gate remains unresolved unless independently fixed | Release-workflow/tag and artifact identity checks without rewriting the upstream application version |
| 0.11.0-specific defects | None confirmed by opening the documentation cycle | Minimal reproduction, affected commit and verified correction |

Do not remove upstream features or tighten Extended defaults solely because an area remains untested.
