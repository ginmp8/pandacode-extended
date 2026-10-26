# Optional chatgpt-cli bridge

PandaCode Extended already supports the official Codex CLI via `codex app-server`. This addition exposes a controlled executable override for the existing Codex harness in **Settings > Providers > Codex > Executable location**.

The companion `ginmp8/chatgpt-cli` package is only a thin process-replacing passthrough to an independently installed official Codex CLI. It **cannot** access a ChatGPT website session or extract the assistant's internal runtime.

## Setup

1. Install and authenticate the official Codex CLI using its supported flow.
2. Install the [companion bridge](https://github.com/ginmp8/chatgpt-cli) and configure `CHATGPT_CLI_CODEX_BIN` as an absolute path to the official executable.
3. In Settings > Providers > Codex > Executable location, choose the installed `chatgpt-cli` entry point and apply.
4. In Settings > Agents, select Refresh agents. A positive installed indicator means an executable was found, **not** that its account or models are authorized.
5. Test an actual `codex app-server` handshake. A screenshot is only evidence of UI and local executable status, not remote authentication.

No API keys, browser tokens, model-provider credentials or authentication assertions are stored by this integration.

## Required acceptance gates

- Existing executable validation rejects relative paths, non-files and non-executable paths.
- Existing provider list and Codex execution path work without the wrapper.
- New UI shows and saves the override without contaminating other provider settings.
- Native build + real authenticated CLI handshake + actual Settings > Agents screenshot, not a browser preview.
- If the runtime lacks the official Codex CLI or GUI dependencies, report `not verified` instead of fabricating screenshots.

Companion issue: https://github.com/ginmp8/chatgpt-cli/issues/1
Integration issue: https://github.com/ginmp8/pandacode-extended/issues/36
