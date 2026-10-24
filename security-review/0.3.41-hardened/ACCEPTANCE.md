# PandaCode Hardened 0.3.41 — frozen acceptance

Mode: feature; baseline: supplied Hardened 0.3.35.4, isolated immutable source copy and initial Git commit. Reference: supplied upstream 0.3.41. No dependency version upgrades, updater, automatic installation, plaintext secret-store migration, new cloud provider, or SQLite migration authorized by this implementation.

1. Connection reassignment/edit/delete cannot resume the previous account's session. History is preserved and context recovery remains available. Active runs reject credential changes.
2. Remote Full access defaults off, is controlled locally, and is checked on native start/send/configure/steer/input/approval admission. Trust/revocation/replay remain enforced. Public-key identity is unchanged by 128-bit fingerprint presentation.
3. GitHub catalogs discover nested SKILL.md at pinned commits, never execute repository content, import explicitly without activation, preserve provenance/hash, reject unsafe paths and local drift, report name/source conflicts, and allow disabled catalogs.
4. Bot/project Skills are explicit, deduplicated, origin-labelled and bounded in each admitted task. They cannot mutate an unrelated/shared checkout. Existing per-agent library management remains available.
5. Edit/Rewind creates a fresh task containing only messages before the selected user message, restores editable text/attachments, never reuses resume ID, rejects running tasks and does not revert files.
6. Historical attachments render as individual usable chips with message-bound native path/URL validation. Old messages and send contracts remain supported.
7. Model picker hides unavailable noncurrent providers; unavailable current provider remains visible but non-executable.
8. Usage by connection reports only data read using that connection. Unsupported/custom wrappers never receive another account's values. Switching at a limit is explicit and starts fresh native context.

Verification: same baseline/candidate npm build and Node suites; targeted browser scenarios for real UI routes; locked Rust compilation and relevant backend unit tests when runtime supports them; final diff and package inspection. Preview mocks prove UI behavior only; no live-account/remote-device/Windows/macOS result is inferred from mocks.

Budget: at most three candidate repair iterations per feature; preserve failed evidence; no weakening test oracles to make a change pass. Parallel workers have independent branches/worktrees. Root merges and evaluates the integrated candidate.
