# Initial synthetic main-to-main integration candidate — not approved

Extended main: `f0105e7db85a0d7bc2e70726716fcfa939fac182`  
Fork main: `0336a2fb5e3abaea35c095fcc63805652f49ea98`  
Original content ancestor: `70bdc708303c8681d8eb0b46f2cdb0ff087aa6eb`

Source merge tree `e49ca180c470fb7bae5e0e7f29049d914cc3a973`. For the first automated code import, conflicting content favors Extended. **This does not prove the new upstream behavior is present in conflicting functions.**

- Paths modified from Extended main: **561**
- Paths still different from fork main: **771** (includes legitimate hardening, branding and exclusive features)
- Paths with unresolved Git conflicts explicitly retained from Extended: **1**.
- Protected paths were restored byte-for-byte from Extended main. Upstream-only workflow files were not imported.
- Full acceptance, functional parity and security validation: **NOT EXECUTED**.

The list of unresolved paths is in `docs/integration/main-to-main/unresolved-paths.txt`. A full assessment artifact is attached to the GitHub Actions importer run. The report is **a worklist**, not a resolution certificate.

## Next gates

1. Audit each auto-merged source path touching network access, credentials, remote execution, providers, agent tool grants, Tauri IPC, storage, cloud connections and mobile push.
2. Reconcile all retained Extended conflict paths against upstream source and tests. Keep both sets of user-facing features or document an explicit verified incompatibility.
3. Reconcile and lock new dependency requirements with no updater, unattended installer, untrusted `npx` bootstrap or non-consensual downloads.
4. Verify tests/builds under a **separate read-only CI token**, then execute native/physical remote safety checks.
5. Never merge to main or publish this candidate solely based on Git's merge result.
