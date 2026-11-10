# PandaCode Extended — independent release-tag identity

Extended releases and upstream PandaCode application versions are **different identities**.

- **Application/source version:** `src-tauri/tauri.conf.json`, `package.json`, Cargo and lockfiles remain aligned with the latest verified `ginmp8/pandacode:main` source (for example, application `0.3.99`). Do not run `npm run release:version` to relabel the app as Extended `0.10.0`.
- **Extended release tag:** a tag such as `v0.10.0` or `v0.11.0` selects matching nonempty `releases/extended/X.Y.Z.md`. This also takes precedence when an Extended version happens to equal the current application version. The source application version remains the native build version and determines the `release/<app-version>/` artifact directory.
- **Upstream tag:** `v<app-version>` selects original `releases/<app-version>.md` when no same-version Extended note exists. If both notes exist for the same numeric tag, the Extended release takes precedence; one Git tag cannot represent two distinct releases. Manual branch runs continue to select original source notes. New upstream changes must still be imported from the fork's current full HEAD, with no fixed version ceiling.
- **Manual branch run:** absent a tag ref, the workflow uses the source application version and original source release notes. Do not assume a branch-based manual run selects the planned Extended documentation version.

## Publishing behavior

Pushing a valid Extended or source tag runs the six-platform build and uploads workflow artifacts. **It does not automatically create a GitHub Release and cannot publish a package to npm.** `.github/workflows/release.yml` has no `npm` publishing job, even for manually approved runs.

Creating a GitHub Release still requires an explicitly initiated `workflow_dispatch` with `publish=true`. The release tag/title and notes use the release identity, while the installer and Tauri updater manifest keep the actual application version. Where signed updates exist, their manifest download URLs use the release **tag** (not a fabricated `v<app-version>` URL). SBOM and build paths continue to describe the application version.

## Limitations and verification

- A `v0.10.0` tag already exists at [main commit `c8ca58e`](https://github.com/ginmp8/pandacode-extended/commit/c8ca58e6baa791afe03b3b94748570222657e23f), where [Release run #42220490913](https://github.com/ginmp8/pandacode-extended/actions/runs/42220490913) failed before build. This patch, committed only to `base/development`, **does not change that immutable tag**; rerunning the old commit still uses its old workflow. Future tags must point to a commit containing this fix after normal promotion, without moving existing tags.
- Tauri's updater compares **application versions**, not Extended documentation tags. An existing installation already reporting the same upstream app version will not necessarily auto-update to a new Extended-only build, even with valid assets. An independent Extended in-app update/version strategy is a separate future change; do not mask this limitation by changing the upstream version fields.
- After merging, validate release resolution using `npm run test:release`, inspect exact-SHA CI, and test installers and upgrades on real supported platforms before approving public artifacts. A green CI build alone is not native upgrade acceptance.

See the [Extended readiness checklist](0.11.0/release-checklist.md) and [upstream parity contract](upstream/parity-contract.md).

## Manual publication requires main-push CI (2026-11-10)

A successful `ci.yml` run must match **both** the exact source commit SHA being released and a **push event on `main`**. Successful PR and other-branch runs do not satisfy the release gate. Only an explicit `workflow_dispatch` with `publish=true` checks this gate and may create a GitHub Release; tag pushes still build workflow artifacts and never publish. Pending/failed/cancelled CI blocks manual publication, and cancelled CI must be rerun for that commit before retrying the release. The app version, Extended release identity and no-npm policy are unchanged.
