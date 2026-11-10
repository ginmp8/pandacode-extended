# Reconcile PandaCode upstream 0.3.107 into Extended 0.12.0

> **Status: draft / not implemented.** This document records the frozen branch
> comparison and required acceptance gates. It does not constitute an integration
> commit or evidence of passing tests.

## Branches and verified checkpoints

- Destination: `extended/development`, PR #120, at
  `6467995ea3b533b039bcd393e4e6a15e847f5464`.
- Source: `base/development`, currently at
  `50b2d8bfd25ba2f885a9f060bfb93fcc4111cc34`.
- Feature-bearing source commit: `e4e5c0c51daa8a0149015bb26a00c8fb09bb2050`
  (native upstream 0.3.107 Skills manager). The later source commit updates documentation.
- Last previously imported source checkpoint for this comparison:
  `4f777ec3414a9c46d07ea58debb99c87fcf4e74f`.
- This PR's branch was created from the destination checkpoint. Neither
  `extended/development` nor `base/development` is a publication target
  for intermediate reconciliation work.

Refresh the source and destination identities before implementation or
publication if either ref advances.

## Required behavior

Integrate **every new upstream feature** from `base/development`, not merely
source-only files. When upstream changes an interface already customized by
Extended, adapt the Extended integration to the new upstream contract. Do not
silently prefer the Extended version of a conflict and omit new upstream behavior.

In particular, retain the upstream native Skills manager (Settings, supported
harness inventory, switches, installation, quarantine, recovery, native command
integration, and tests), while adapting the separate Extended Customize Skills
workspace, its GitHub/ZIP sources, install/replace/sync progress, and other
Extended-specific behavior to coexist without duplicate or conflicting
authority. Preserve Council reviewer isolation and existing bot, channel,
feature-control, remote, and credential boundaries.

## Conflict inventory

The diagnostic three-way integration against the last imported source tree
reported **29 unresolved paths**, including:

- Rust/IPC: `src-tauri/src/lib.rs`, `src-tauri/src/skills.rs`,
  `src-tauri/src/providers.rs`, `src-tauri/src/architect.rs`,
  `src-tauri/src/bots.rs`, and `src-tauri/src/agent_tools.rs`.
- Frontend/preview: `src/main.ts`, `src/skills-library.ts`,
  `src/preview-skills.ts`, and `src/preview.ts`.
- Manifests and tests: `package.json`, `package-lock.json`,
  `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and Playwright configuration.
- Two formerly shared files, `src-tauri/src/skills/catalog.rs` and
  `src-tauri/src/skills/catalog_tests.rs`, were deleted upstream but changed
  in Extended; their replacement contracts must be reviewed explicitly.
- Documentation, release metadata, and the upstream baseline manifest.

The diagnostic merge was performed in a disposable CI checkout. It was
**not committed** to this branch.

## Acceptance gates before marking ready

- [ ] Re-freeze source/destination refs and review every source-only,
      destination-only, and concurrently changed path, including deletions
      and file modes.
- [ ] Import all new upstream behavior and adapt Extended call sites, data
      contracts, UI navigation, settings, and persistent-state transitions.
- [ ] Preserve Extended behavior without retaining obsolete upstream
      interfaces or bypassing native security controls.
- [ ] Resolve every concurrent modification semantically; no unresolved
      conflict markers, automatic ours/theirs conflict policy, placeholders,
      or unexplained omitted source behavior.
- [ ] Publish a single-parent reconciliation commit on this topic branch
      without merging, squashing, rebasing, force-pushing, or modifying either
      original branch.
- [ ] Verify the committed tree, protected source/target invariants, and
      parent count at the exact candidate SHA.
- [ ] Run TypeScript build, Node and release tests, relevant Playwright
      Skills/Council/bot UI tests, Rust tests and Clippy, and available platform
      CI jobs. Repair real integration regressions without weakening tests.
- [ ] Inspect all exact-SHA CI results; record any native Windows/macOS/Linux
      acceptance still not executed.

**Do not merge or mark this draft ready until implementation and
verification have actually passed.**
