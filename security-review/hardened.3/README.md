# PandaCode hardened.3 - Slack test fixture correction

The hardened.2 source archive contained two Slack credential-shaped **test values** in
`src-tauri/src/bots/tests.rs`, inside the ignored live-service integration test.
The origin is a test fixture; neither credential validity nor usage was established.

This revision changes those two strings to bare, deliberately invalid `xoxb-` and
`xapp-` prefixes. `channels::check_format` still accepts the right prefixes and
`slack::verify` still calls the provider in the explicitly ignored live test.
No production Slack code or network behavior was modified.

Changes also update source-distribution, About, release-asset and installation
labeling to `hardened.3`. The numeric upstream version remains `0.3.28`.
No historical evidence or changes.patch is edited; notably, a patch containing
removed literal tokens must never be redistributed.

## Reproduction / validation

- `python security-review/hardened.3/check-slack-fixtures.py .` scans the source
  distribution for Slack-token-shaped values and emits **locations only**.
- The same scanner found **two candidates** in the previous hardened.2 source
  before the fix and **none** in the current release source.
- Synthetic positive and negative scanner cases were executed with no token-byte
  disclosure. Rust test source assertions and release metadata consistency passed.
- Rust/Cargo integration tests, Windows install, antivirus and authenticated
  Slack integration were **not run** in this revision; see `verification.json`.

## Local Git history

Repacking source archives does **not** rewrite local Git history. If GitHub push
protection reports the original commit, replace its fixtures and rewrite every
commit that contains the token-shaped strings before pushing. Do not bypass
push protection based solely on the assumption that values are synthetic.
If there is any chance a token was real, revoke it with the provider.

Files in this folder:
- `changes.json`: redaction-safe file/hash delta from hardened.2.
- `verification.json`: checks and limitations for this narrow change.
- `manifest.json`: SHA-256 inventory of the final archive files, excluding itself.
- `check-slack-fixtures.py`: data-only source scanner.
