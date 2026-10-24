# Desktop platform support

The retained build configuration describes six architecture targets across three
operating systems. This source-only 0.3.34.hardened archive includes no installers. Artifact filenames below are build targets, not proof of completed builds.
Use the local packaging commands in [releasing](releasing.md); upstream releases
do not include this fork's changes. Native platform support requires its own
build and desktop checks and is not established by this installation review.

| Target | Platform requirements | Release asset |
| --- | --- | --- |
| `darwin-aarch64` | Apple silicon, macOS 12 or later | `PandaCode-0.3.34.hardened-macOS-arm64.dmg` containing `PandaCode.app` |
| `darwin-x86_64` | Intel Mac, macOS 12 or later | `PandaCode-0.3.34.hardened-macOS-x64.dmg` containing `PandaCode.app` |
| `windows-x86_64` | x64 Windows 10 version 1809 or later, or Windows 11 | `PandaCode-0.3.34.hardened-Windows-x64-setup.exe` (current-user NSIS installer) |
| `windows-aarch64` | Windows 11 on ARM (arm64) | `PandaCode-0.3.34.hardened-Windows-arm64-setup.exe` (current-user NSIS installer, native arm64) |
| `linux-x86_64` | x64 Ubuntu 22.04 or a compatible newer distribution, glibc 2.35+, graphical X11 or XWayland session | `PandaCode-0.3.34.hardened-Linux-x86_64.AppImage` built on Ubuntu 22.04 |
| `linux-aarch64` | arm64 Ubuntu 24.04 or a compatible newer distribution, glibc 2.39+, graphical X11 or XWayland session | `PandaCode-0.3.34.hardened-Linux-aarch64.AppImage` built on Ubuntu 24.04 |

Other architectures are not release targets, but you can try building them
from source with `npm run package`.

## Unsigned builds

Windows and Linux local builds are unsigned unless an independently reviewed
signing process is added. macOS supports the explicit signing choices documented
in [releasing](releasing.md). An unsigned build or reputation warning is not proof
of malware or safety. Do not disable antivirus, add exclusions, or override a
security block to use this distribution; investigate the exact detection first.

Each asset has a `.sha256` file beside it so you can check the download.

## Native runtimes and dependencies

The interface uses the platform webview: WKWebView on macOS, WebView2 on Windows,
and WebKitGTK on Linux. PandaCode adds no Electron, resident Node server, or bundled
Chromium runtime. Windows and Linux integration dependencies are conditional
Cargo dependencies; they do not enter the Mac build. The AppImage includes core
GTK and WebKitGTK 4.1 libraries, so its package size should be assessed separately.
It also bundles `xdg-open` to open external links in the user's configured
desktop browser.

AppKit Liquid Glass and vibrancy are Mac-only window materials. Windows and
Linux retain their native window controls; the app's wallpaper opacity and blur
settings are shared UI features, not a promise of equivalent native materials.

Windows requires an existing Microsoft WebView2 runtime. This revision sets
`webviewInstallMode.type` to `skip`: the installer neither downloads nor installs
the runtime and does not bundle a fixed copy. Provision it separately under your
Windows administration policy; without it the app cannot run. See
[Windows installation](install-windows-hardened.md). The integrated terminal uses ConPTY,
which sets the Windows 10 version 1809 minimum.

The Linux AppImage's GTK launch hook selects the X11 backend. A graphical X11
session or XWayland within a Wayland desktop is required; native Wayland without
XWayland is unsupported. Core GTK/WebKit libraries are bundled and do not require
a separate manual install for the AppImage.

Mark the downloaded AppImage executable before opening it. Some distributions
also require FUSE 2 (`libfuse2` on Ubuntu 22.04). Where FUSE is unavailable,
AppImage supports extract-and-run:

```sh
chmod +x ./PandaCode-0.3.34.hardened-Linux-x86_64.AppImage   # or PandaCode-0.3.34.hardened-Linux-aarch64.AppImage
APPIMAGE_EXTRACT_AND_RUN=1 ./PandaCode-0.3.34.hardened-Linux-x86_64.AppImage
```

Linux GLM key storage requires an unlocked desktop Secret Service. If it is
unavailable, configure `ZAI_API_KEY` in the app's environment.

## Coding tools and shortcuts

Install Git and at least one supported provider CLI on the machine, and sign in
using that CLI's normal flow. PandaCode does not include provider plans,
provider binaries, or their optional Node/runtime dependencies. Each CLI's own
platform requirements still apply. Build contributors also need Node.js 22+, Rust,
and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for their
host, including the Windows C++ build tools or Linux development libraries
(see [CONTRIBUTING.md](../CONTRIBUTING.md)).

The interface uses Command shortcuts on Mac and Control shortcuts on Windows
and Linux. The quick launcher's default is Command/Control + Shift + Space and
can be changed in Settings.

Linux global shortcuts use an X11 backend. In a Wayland session, XWayland may
allow registration without making the shortcut work outside XWayland apps;
behavior depends on the desktop. If only the global-shortcut backend cannot
initialize, PandaCode still opens and explains the limitation in launcher settings;
this does not remove the graphical X11/XWayland requirement. Use
**Settings → Quick launcher → Open** to open the panel manually. This is not a
Wayland portal implementation.

## Validation

CI (`.github/workflows/ci.yml`) typechecks, runs the unit tests, builds the
website and runs `cargo check`, `cargo test --lib` and strict Clippy on Linux.
The previous automatic release/publication workflow is removed in this revision.
The retained `scripts/release/ci-startup-smoke.mjs` is an optional local probe;
WebView2 must already be provisioned on Windows. Staying alive alone would not
prove that a webview rendered, a window accepted input, or a provider turn
completed. Browser fixtures on a different OS do not establish native platform
compatibility. Current executed checks and limitations are in
[the installation review](../SECURITY-INSTALLATION-REVIEW.md).

### Recording validation results

Keep build results separate from interactive checks. Mark unrun checks as
**Not run**:

```text
Commit / artifact SHA-256 / target:
CI run / native tests / Clippy / package build:
Startup probe: command, environment, duration, exit/log result:
Interactive OS / display session / webview version:
Manual checks: passed items, failures, and items not run:
```

### Manual native smoke checks

Use the packaged artifact on a real desktop session or an interactive VM. Record
the exact OS and artifact for each result; do not infer Windows 10 behavior from
a Windows Server runner, or Linux desktop behavior from Xvfb.

| Check | Evidence to record |
| --- | --- |
| Install and launch | Window visibly renders and accepts input; close/reopen preserves settings. Windows does not install/download WebView2; test with the runtime present and record the failure when absent. Linux launches with FUSE and with extract-and-run; record X11 or XWayland. |
| Workspace and terminal | Open a Git project, inspect files/diffs, create a worktree, run and resize the terminal, then close it. Include a path with spaces. |
| Providers and persistence | Discover each provider being advertised on that OS. With a signed-in CLI, stream a small turn, handle approval, stop, restart the app, and resume the same task. Note any provider not tested. |
| Concurrent work | Run separate tasks and a team, inspect worker output, cancel them, and confirm their child processes exit. |
| Embedded browser | Open a local port, navigate back/forward/reload, select an element, and attach a readable screenshot. |
| Launcher and input | Open manually and with the configured shortcut, change the shortcut, inspect an active task, dismiss/refocus, and use platform keyboard shortcuts. Record Wayland limits instead of claiming universal global access. |
| Credentials | Check GLM store-unavailable messaging and environment fallback if GLM is included in the test. |

Record unrun native checks and known limitations alongside each preview's
results, even if CI packaging succeeds. Complete these checks before claiming
stable support. Reports from real machines are welcome as
[GitHub issues](https://github.com/aicodeking/pandacode/issues).
