# Windows: Build and Install 0.3.34.hardened

This package contains source code, not a prebuilt Windows installer. The executable must be built on Windows with the Tauri prerequisites, Node.js, Rust/MSVC, and the Microsoft WebView2 runtime already installed. The installer does not download WebView2. The application will not run without the runtime.

## Local Commands

From this project directory, without elevating the terminal to administrator:

```text
npm ci --ignore-scripts
npm run test:install-security
npm run release:build -- windows-x86_64
```

For Windows ARM64, use `windows-aarch64` on a compatible ARM64 host. The build explicitly runs TypeScript/Vite, Cargo, and the Tauri packager. `--ignore-scripts` blocks npm install scripts; it does not block these deliberate commands or Rust dependencies' `build.rs` scripts. The build may download public dependencies and tools; it is not an offline build.

Expected x64 output from the script, not yet produced in this Linux review:

```text
release/0.3.34-hardened/hardened/PandaCode-0.3.34.hardened-Windows-x64-setup.exe
release/0.3.34-hardened/hardened/PandaCode-0.3.34.hardened-Windows-x64-setup.exe.sha256
```

Review the produced binary, tool origins, and antivirus alerts before running it. Do not disable Avast, add a PowerShell exception, or ignore a new detection to continue installation. The hash verifies identity, not safety. The absence of an Authenticode signature cannot be fixed with an invented certificate.

## Do Not Use

Do not use `npx pandacode`, Homebrew, upstream release links, or a previously downloaded installer to obtain these fixes. They are different artifacts. Do not use `ExecutionPolicy Bypass`, antivirus exclusions, or remote commands piped into an interpreter as installation shortcuts.

## Limitations

The native identifier was preserved and the internal version is now `0.3.34-hardened`. Back up existing data before replacing an installation. The About screen displays 0.3.34.hardened. The application still runs authorized AI commands and may open PowerShell; this configuration does not impose a firewall on agent processes. Microsoft-managed Windows/WebView2 updates were not changed.

References: [Tauri Windows installation](https://v2.tauri.app/distribute/windows-installer/), [prerequisites](https://v2.tauri.app/start/prerequisites/), [npm configuration](https://docs.npmjs.com/cli/v10/using-npm/config/).
