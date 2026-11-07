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

## Additional 0.3.100 integration validation gaps

- **Account pools and sign-in:** The upstream implementation, tests, original release notes and UI were imported at [`a28ce807`](https://github.com/ginmp8/pandacode-extended/commit/a28ce807a9bcc3a8583440ff6035deb1fd0854da). No independent live two-account Claude/Codex switching, authentication, restart/hold recovery, input-queue preservation, or interaction with optional Extended controls has yet been recorded for this candidate. These are **unverified acceptance criteria**, not established failures.
- **Inherited upstream limitations:** Only Claude Code and Codex subscription logins can join pools; cloud runs and goal operations are not pooled. The source reports incomplete live two-account Codex and complete one-click sign-in testing. See [original 0.3.100 notes](../../../releases/0.3.100.md).

## PandaCode 0.3.101 acceptance gaps

- **Unverified, not confirmed defects:** native multi-account Claude/Codex sign-in, identity reads, cache invalidation after login change, account usage refresh and cross-panel state consistency after the [upstream import](https://github.com/ginmp8/pandacode-extended/commit/d7cf15467bf8f25563db253e7b35a1efeb2e5b5e).
- **Unverified, not confirmed defects:** operation of Extended-specific Settings controls in both states alongside the new account-provider screens; comparison of all 80 shared divergent files by hunk.
- Original upstream [0.3.101 notes](../../../releases/0.3.101.md) document incomplete installed-app testing with several real accounts. No feature is disabled on this basis.
