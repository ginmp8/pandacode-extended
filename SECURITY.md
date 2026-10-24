# Security policy

## Supported versions

**PandaCode 0.3.34.1 hardened** is a separately maintained source distribution.
It does not automatically receive upstream security fixes and cannot safely be
upgraded by installing a public upstream release or running `npx pandacode`.
No signed desktop installers or guaranteed security-maintenance SLA are provided
by this source archive. The 0.3.34 hardened package is superseded by 0.3.34.1
for the issues addressed in this revision; other versions are not promised support.

For upgrades, obtain a separately reviewed hardened source ZIP, verify its published
SHA-256 against an independently delivered expected value, review the diff and
its dependency locks, build on a trusted isolated host, and back up local data
before replacing an installation. Never treat the latest upstream release as an
automatic hardened update.

## Reporting a vulnerability

Please do **not** open a public issue for security problems.

For an issue in the **original upstream project**, use the upstream
[GitHub Security Advisories](https://github.com/aicodeking/pandacode/security).
For an issue specific to this **hardened fork**, report privately to the person
or organization that supplied this particular ZIP; the upstream maintainers do
not maintain or warrant these fork-only changes. This source package has no
verified public fork-specific vulnerability intake address. Include:

- the PandaCode version, OS and architecture (Settings → About can copy these),
- what an attacker can do and the steps to reproduce,
- any proof of concept, logs or screenshots (remove secrets and personal data).

We aim to acknowledge reports within a few days, keep you updated while we
investigate, and credit you in the advisory unless you prefer otherwise. Please
give us a reasonable time to release a fix before disclosing publicly.

## Scope

In scope: the desktop app in this repository (Rust core, webview frontend,
IPC boundary, local servers such as the bots web chat, credential storage) and
the release workflow.

Out of scope: third-party coding CLIs and provider services; report their
own vulnerabilities to their maintainers. An explicitly approved Full-access
action may have substantial side effects, but authorization and approval-binding
bugs in PandaCode itself remain in scope.

**Hardened differences:** new tasks default to Ask; external bot approvals require
an exact one-time request ID; runtime npm installation and npx-based Wrangler
fetch are disabled; source-only manual builds and disabled application auto-updates
remain the distribution policy. See `security-review/0.3.34.1/README.md`
for implementation evidence and outstanding runtime verification.
