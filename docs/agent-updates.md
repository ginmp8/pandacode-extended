# Agent updates in hardened.1

Automatic version checks, update popups and in-app agent installation are disabled.
The frontend controller has no polling timer, and native `check_agent_updates`,
`update_agent` and the internal installation entry point reject before contacting
an external server or starting an updater. Old preferences cannot re-enable them.

Install or update the specific agent version you have reviewed using its own
package manager outside PandaCode. Keep the existing provider selection and
login; normal AI execution remains available. The metadata parsers and offline
installer tests retained in the source are not active update routes.

Child processes receive supported update/telemetry opt-outs for Claude, Kimi
and OpenCode; Copilot and Grok retain their no-auto-update launch arguments.
These settings reduce vendor background traffic but are not an OS firewall:
a provider binary, its own settings, plugins or an explicitly requested shell
command can still perform network access or installation.

See [SECURITY-HARDENING.md](../SECURITY-HARDENING.md) for exact changes and test
coverage. Application updates and the included npx installer are disabled too.
