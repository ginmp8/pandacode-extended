> **Historical report for hardened.2.** The hardened.3 fixture adjustments are documented in `security-review/hardened.3/README.md`.

# PandaCode 0.3.28 / hardened.2: Installation Review

**Result: installation and packaging gaps corrected; the cause of the Avast alert remains undetermined.**

Date: October 23, 2026. This work modifies the existing project; it is not a reimplementation. The package contains source code, not a compiled or signed EXE/MSI. It was not run on Windows or submitted for Avast validation.

## 1. Identity, Context, and Limits

Examined base: `pandacode-0.3.28-hardened.1.zip`, SHA-256:

```text
4100f1ec4ac1c1629fd1d4c01137071cc41d2436d5b2cea6f0fb222a69381b17
```

Extraction was checked for path traversal, duplicates, and symbolic links; the base was preserved separately. The user reported that `IDP.HELU.PSE95 - Command line detection` appeared during installation. The image identifies PowerShell, Behavior Shield, and a blocked state, but does not show the full command line, parent process, or installer that ran. The previously supplied file contains no EXE/MSI, DLL, PS1, BAT, or CMD. Therefore, the identity of the local binary that triggered the alert cannot be verified.

This review covered Tauri configuration, installers/packaging, release scripts, npm installation, the npx bridge, installation instructions, and the main PowerShell invocations in source. It is not a new exhaustive audit of every provider, dependency, prompt, or integration. No independent verifier was used in this round. The findings below describe source/configuration behavior and local test results, not confirmed CVEs or exploits. No hidden exfiltration instruction was identified in the inspected installation paths; this does not certify that malware is absent.

## 2. Gaps and Corrections

### INST-01: Automatic WebView2 Download and Execution

In the base, `scripts/release/build.mjs` configured `downloadBootstrapper` for silent execution. The main Tauri configuration also did not establish an alternative Windows policy. An installer produced through these paths could therefore fetch and run the external prerequisite on the user's machine. This is legitimate installation behavior for a Microsoft component, **not evidence of malware or that it triggered Avast**.

Correction: `webviewInstallMode: { type: "skip" }` in the release plan and main configuration; NSIS explicitly set to `currentUser`. The new overlay `src-tauri/tauri.windows.conf.json` selects NSIS for direct Windows builds. Files: `scripts/release/build.mjs:41-59`, `src-tauri/tauri.conf.json:66-74`, and `src-tauri/tauri.windows.conf.json`. The user must provide WebView2 separately; the application will not run without the runtime. Security updates for Windows/Edge/WebView2 managed outside the application were not disabled.

### INST-02: Residual Updater Build Infrastructure

The base release script could enable `createUpdaterArtifacts`, pass inherited signing keys, and sign/copy update artifacts; the `updater-manifest.mjs` CLI still generated `latest.json`. The application updater plugin had already been removed in hardened.1. **Generating these files does not prove that the application automatically updated at runtime.**

Correction: artifact generation is always disabled; signing/copy steps were removed; inherited updater keys are filtered; and the manifest CLI explicitly refuses to run, even with valid inputs. References: `scripts/release/build.mjs:19-59` and `scripts/release/updater-manifest.mjs:16-21`. Pure/offline helpers and their tests were preserved; they do not publish anything automatically.

### INST-03: npm Scripts Relied on a Manually Typed Option

Without a project `.npmrc`, the previous protection depended on remembering `--ignore-scripts` for each installation. Dependency install hooks execute code with the npm process's permissions and environment. No malicious dependency in this package was demonstrated.

Correction: `.npmrc` sets `ignore-scripts=true`; instructions retain `npm ci --ignore-scripts`. A test ran `npm install --offline` on a local fixture with a harmless postinstall hook that creates a sentinel file: the base ran the hook; the revision did not. The test clears inherited npm configuration to verify the project file's policy rather than an environment-masked result. This option is not a sandbox and can be intentionally overridden by higher-precedence configuration. Explicit `npm run` commands continue to work. Rust `build.rs` scripts are not covered.

### INST-04: Rust Dependencies Were Not Locked During Packaging

Release plans did not pass `--locked` to Cargo. If resolution changed, a build could modify the lockfile instead of failing. Correction in `scripts/release/build.mjs:59` and the `package`/`package:fast` scripts in `package.json`: pass `--locked`. This fixes dependency resolution identity; it does not block downloads or build executables, nor prove that dependencies are safe. Lockfiles and dependency versions were not changed in this round.

### INST-05: Remote Installer Piped Directly to a Shell

The Grok section of the UI (`src/provider-settings.ts:71`) and `docs/providers.md` suggested downloading a remote script and piping it directly to bash. This was displayed guidance, not automatic execution or evidence of malicious provider behavior. It was replaced with manual installation of a reviewed/pinned version. The Grok integration remains available.

### INST-06: Upstream Publishing and Artifact Confusion

The `.github/workflows/release.yml` workflow published releases/feed and could publish the npm package when configured. It does not run merely because the application is installed on a PC; it was repository infrastructure incompatible with this locally reviewed distribution. It was removed. `packages/npx/package.json` now sets `private: true`; the npx installer, already blocked in the previous review, remains blocked.

The local build now separates outputs under `release/0.3.28/hardened.2/` with names `PandaCode-hardened.2-...` and a SHA-256 sidecar. README, CONTRIBUTING, and guides do not direct readers to obtain this revision through upstream clone/installers. Instructions to bypass alerts were removed from the platform guide. **The only product file removed was `.github/workflows/release.yml`.** Previous evidence summaries were relocated, not discarded.

## 3. Preserved PowerShell Uses and Unattributed Alert

| Path | Source use | Demonstrated relation to alert |
| --- | --- | --- |
| `src-tauri/src/local_servers.rs:43-62` | Local listening-port/process query through `Get-NetTCPConnection`/`Get-Process`; fixed arguments, no profile | None |
| `src-tauri/src/terminal.rs:622-641` | Local PID, parent PID, and name query through `Get-CimInstance Win32_Process` | None |
| Interactive terminal | Shell selected for user/agent-authorized commands | None; executed command not supplied |
| `src-tauri/src/cloud/aws.rs:119-153` | TightVNC installation script inside an AWS Windows VM, during explicit provisioning, with inherited Authenticode verification | Not the local PC installer |

None of these uses was renamed, obfuscated, or replaced to evade antivirus. No Rust file was modified in this round. The product continues to run commands and use selected providers as requested. Without the full blocked command line, parent process, and hash/origin of the EXE used, the alert cannot be attributed to packaging, an external dependency/tool, environment commands, or a false positive. The corrections are justified by verifiable code paths, **not by reproducing the detection**.

## 4. Verification Actually Run in This Round

| Verification | Result |
| --- | --- |
| Eleven new policy tests against hardened.1 | 11 expected failures; the tests detect the original gaps |
| Same tests against hardened.2 | 11 passed |
| Existing release/npx suite | 39 passed; no failures |
| Full Node suite attempted | 134 TAP cases: 130 passed, 4 failed because inputs were missing |
| Explicitly runnable Node suite subset | 130 passed; includes the 11 new tests, do not count them again |
| Syntax/transpilation of the two changed TypeScript files | Passed; not equivalent to full typecheck |
| Syntax of six changed JavaScript files | Passed |
| Full TypeScript/Vite/Tauri/Rust build | Not run |
| Windows installer/signing/execution and Avast check | Not run |

The four full-suite failures were three bundle tests without `dist/index.html` and loading `tests/file-icons.test.mjs` without `material-icon-theme` installed. Test files remain in the project; none were deleted, weakened, or marked as passing. The reduced selection is identified in the log and external runner. There were 169 distinct passes across available suites and release tests; this does not mean full coverage.

Existing policy tests were adjusted for the new explicit requirement (`false` instead of a missing field; no bootstrap/updater), without removing unrelated security assertions. Early attempts hit test-environment incompatibilities (TypeScript loading, WebAssembly, and loopback); logs were preserved, and the final run used a corrected environment, not reduced assertions.

Isolated Linux execution used network/PID/mount/user namespaces, chroot, removed capabilities, an allowlisted environment without credentials, read-only source/toolchain, and a 64 MiB tmpfs scratch space. These protections were tested beforehand; limits covered time, CPU, virtual memory, processes, files, and descriptors. Loopback was allowed for fixtures; external egress was denied. No suspicious script was run on the user's PC. Logs and runners are under `security-review/hardened.2/`. The runner records paths from this review environment and is not an installer to run on Windows.

The hardened.1 results, including its older Rust/browser suites, are historical and do not count as validation for this round. Patch equivalence and package integrity are recorded separately in `security-review/hardened.2/patch-verification.json` and the manifest.

## 5. Use and Residual Risk

Follow [the Windows guide](docs/install-windows-hardened.md) using this source code. Do not use `npx pandacode`, Homebrew, or the public upstream installer to obtain hardened.2. Keep Avast enabled and do not add PowerShell/application exclusions to bypass a block. The new policy reduces identified automatic paths; it does not guarantee the alert will not recur.

The build may download dependencies/tools and execute their code, including `build.rs`; this version is not inherently offline. This round did not run a new vulnerability scan of every dependency. After installation, prompts/code and results may be sent to the selected AI provider; authorized commands, bots, cloud, and preserved integrations may also use the network. This hardening does not impose a firewall on agents or prevent every malicious command generated through prompt injection. Internal version 0.3.28 and the app identifier were retained; back up data before replacing an existing installation.

No source/code/credentials were sent to a public scanner. External queries in this review concerned public documentation. Attributing the detection requires preserving the original event, blocked command line and parent process (with secrets redacted), and the hash of the installer used. Do not reproduce the installation by bypassing the block.

## 6. Sources and Evidence Trail

- [Tauri: Windows installation and WebView2 modes](https://v2.tauri.app/distribute/windows-installer/): `skip` does not install the runtime; the application depends on it.
- [npm v10: ignore-scripts](https://docs.npmjs.com/cli/v10/using-npm/config/#ignore-scripts): hooks and explicitly requested commands behave differently.
- [Cargo build: locked](https://doc.rust-lang.org/cargo/commands/cargo-build.html): locked resolution is not equivalent to `offline` or a sandbox.
- `security-review/hardened.2/findings.json`: findings, scope, and attribution status.
- `security-review/hardened.2/verification.json`: current results and validations not run.
- `security-review/hardened.2/changes.patch`: product/docs/test diff; excludes only the `security-review/` evidence tree to prevent recursion.
- `security-review/hardened.2/changes.json`: detailed delta inventory.
- `security-review/hardened.2/manifest.json`: hashes of final files, excluding the manifest itself.

**Conclusion:** installation corrections delivered with local tests and documented limitations. No malware-free attestation, Windows/Avast approval, or confirmed cause for the original alert is claimed.
