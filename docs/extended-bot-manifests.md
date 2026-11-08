# Extended: portable bot manifests and Git synchronization

Issue: [#123](https://github.com/ginmp8/pandacode-extended/issues/123). These features live on `extended/development`. The original PandaCode bot importer remains the single mutation engine; the Desktop and Git adapters invoke that engine rather than reimplementing bot management.

## Desktop workflow

Open **Bots → +**. The existing **New bot** and **New channel** actions remain available.

- **Import / Sync bots… → Local JSON file** reads a UTF-8 `{"bots":[...]}` manifest and imports it immediately after a verified local backup. No dry-run is required. A partial import reports errors and keeps the successfully imported entries.
- **Import / Sync bots… → Git sources** manages one or more `https://github.com/owner/repo` repositories, refs and explicit JSON file paths. Choose **Save sources**, **Check for updates** (read-only), or **Sync now** (validates every selected manifest, backs up locally, then applies).
- **Export bots…** selects one or many bots. Save a combined JSON file or choose a directory for individual per-bot JSON files. Existing destination files are not overwritten by Desktop exports. A combined file includes a shared channel only when all its members are selected; each individual bot file includes only channels whose full membership fits that file. Channels involving selected bots but omitted due to missing members are named in the export result and displayed in the dialog, including individual-file exports. Unrelated channels are not reported as omissions.

The existing Bots view refreshes after import or sync. The manifest contains configuration only, not conversation posts, runtime state, memories, workspace files, pending assignments, pairing decisions, credentials or application/database backups.

## Headless commands

The original commands are preserved:

```sh
pandacode serve --import-bots bots.json   # Imports, then starts serving
pandacode serve --export-bots bots.json   # Exports all configurations, then exits
```

Extended adds a one-shot Git sync that **exits after applying**; it does not start a persistent host:

```sh
pandacode serve --sync-bots bot-sources.json
```

A source registry accepted by both Desktop and headless:

```json
{
  "version": 1,
  "sources": [
    {
      "id": "team-bots",
      "repository": "https://github.com/example/bot-configurations",
      "ref": "main",
      "manifests": ["bots/developer.json", "bots/reviewer.json"],
      "autoSync": false
    },
    {
      "id": "release-bots",
      "repository": "https://github.com/example/release-configurations",
      "ref": "stable",
      "manifests": ["configs/bots.json"]
    }
  ]
}
```

Desktop stores the same registry JSON under its application-data directory, in `bot-manifest-sources.json`. The one-shot headless command reads the supplied source registry without modifying it or the Desktop registry. After each successful or partially applied sync, headless persists its checked/applied commit and content receipts under the private application-data directory `bot-manifest-headless-state/<source-path-sha256>.json`. The sidecar key is derived from the canonical source registry path, so reuse the same path and app-data directory across invocations to enable no-op retries. Changing a source's repository, ref or manifest paths invalidates its prior apply receipt. The source file remains suitable for Git synchronization and read-only environments.

Only direct GitHub-hosted repositories are supported by the current source transport. Private repositories may use an existing `GH_TOKEN` or `GITHUB_TOKEN` environment variable, scoped for read access; never put tokens into URLs or source JSON. Each source ref is resolved to an immutable commit SHA, all its files are read at that SHA and their SHA-256 content hashes are included in the sync result together with repository/ref provenance, bot change counts and start/end timestamps. When manifest content and exported local configuration match the previous successful sync, another manual sync reports no changes without importing, creating a backup or restarting channels; local drift still triggers reapplication after backup. Unsafe paths, oversized files, missing refs, invalid JSON, duplicate bot/channel definitions with incompatible settings and credentials embedded directly in remote manifests block the batch before mutation. Identical definitions are deduplicated. Git sync never pushes, commits or edits a remote repository.

**Automatic polling is not enabled in this implementation.** Source registry entries default to `autoSync: false`. Periodic application of security-sensitive configuration requires a separately reviewed opt-in and privilege gate; use explicit **Sync now** or the one-shot headless command in the meantime.

## Portable bot envelope

The original unversioned `{"bots":[...]}` format stays valid. One bot or many are represented by elements in the same `bots` array. Extensions are entirely optional:

```json
{
  "bots": [
    {
      "name": "Developer",
      "provider": "claude",
      "model": "example-model",
      "mode": "ask",
      "color": "#3366cc",
      "reasoningEffort": "high",
      "backupModels": [],
      "fallbackMode": "ask",
      "notify": false,
      "whileWorking": "queue",
      "showPlan": true,
      "extended": {
        "freshContextEachMessage": true,
        "includeSharedChannelHistory": false,
        "sharedChannelHistoryOverrides": [
          { "channel": "launch", "mode": "include" }
        ]
      }
    }
  ],
  "sharedChannels": [
    {
      "name": "launch",
      "members": ["Developer"],
      "lead": "Developer",
      "archived": false
    }
  ]
}
```

The examples use placeholder provider/model names. Import requires a provider, model and permission mode supported by the installed build. The newer optional native profile fields accept explicit values; omitted or `null` native additions preserve existing saved values on update and use the product defaults for new bots. Explicit `false` and empty arrays are meaningful. The legacy core settings retain their original import behavior. Missing or `null` `extended` values leave existing Extended preferences unchanged. Unknown Extended settings and malformed Extended preference values are rejected before changing a bot profile. A non-empty reasoning effort must also satisfy the provider's supported effort contract; dynamic per-model catalog checks still occur when the harness is available.

Local `sharedChannels` are distinct from external `bots[].channels`. Members and lead use portable bot names, resolved after bot imports; membership order is preserved. Repeated imports match a channel by case-insensitive name. Missing or `null` `sharedChannels` makes no changes, and `[]` never deletes channels. A missing required member produces an error; other bot imports can still succeed. History overrides are configured after the local channels have been imported. Channel messages and activity never enter manifests.

## External channel configurations

Existing `bots[].channels[]` are still matched by bot name plus kind and label. **Legacy imports skip existing channels**, so an old manifest does not unexpectedly change credentials or connectivity. An Extended export includes `reconcile: true`, enabling changes to supported non-secret settings and `enabled` on matching channels while retaining their channel ID, credentials, pairings and Web Chat token.

```json
{
  "bots": [{
    "name": "Developer",
    "provider": "claude", "model": "example-model", "mode": "ask",
    "channels": [{
      "kind": "web", "label": "local",
      "config": { "remote": false, "publicUrl": "" },
      "enabled": false,
      "reconcile": true,
      "secretsNeeded": []
    }]
  }]
}
```

Portable configuration is allowlisted. For Web Chat, `remote` and `publicUrl` are supported; for WhatsApp, `phoneNumberId` and `publicUrl` are supported. Export excludes runtime identity, WhatsApp verification tokens, Web tokens, provider tokens and other unknown/sensitive channel configuration keys. An unknown saved key blocks export/backup rather than silently leaking a secret. Immutable WhatsApp phone-number identity cannot be changed in-place by a manifest; reconfigure that connection through its native setup flow.

The `secretsNeeded` list is informational **only** and cannot recover credentials on another machine. To create a new external channel, supply the required `secrets` keys using a secure local reference, for example:

```json
{
  "kind": "telegram",
  "label": "team",
  "config": {},
  "secrets": { "token": "env:TELEGRAM_BOT_TOKEN" },
  "enabled": false
}
```

An existing channel never loses stored credentials because `secretsNeeded` is present. Newly restored Web links acquire local tokens; WhatsApp verification tokens and webhook endpoints require local reconfiguration; other external integrations may require destination-side credentials, endpoint registration or pairing. A manifest export is **not** a credential-complete machine migration.

## Verified local pre-import backups

Before a local or Git manifest can modify bots, the app exports **all** current non-secret bot and shared-channel configurations, writes a timestamped JSON snapshot under the dedicated `bot-backups/` application-data directory and reads it back to verify the bytes. On supported Unix filesystems, backups are private (`0700` directory, `0600` files). Windows protection relies on the current user's application-data directory access controls.

The snapshot has a unique UTC timestamp and ID. Up to 10 verified snapshots are retained; pruning happens only after a new verified backup was saved. An unavailable disk, invalid directory or failed write prevents the entire import from starting. Backup paths appear in headless output and Desktop import results; partial imports retain their snapshot.

To recover, import the backup JSON through the same import path. This restores supported configuration, **not** deleted data or secrets. Previously created bots are not deleted by a restoration file that omits them; live external actions, messages, runtime queues, pairings and conversation history cannot be undone through a manifest.

## Limits and regression safety

- Up to 1 MiB of normalized JSON and 250 bot entries per import batch; Git files are individually bounded to 512 KiB, with at most 16 sources and 32 manifest files.
- Git is read-only and manual by default. A source revision that fails partially is not recorded as fully applied; correct the manifest and retry.
- Native per-bot partial success, canonical name matching and non-deletion by omission are preserved.
- The original `--export-bots` path behavior is retained for compatibility. For protected, no-clobber desktop exports, choose a new destination in the Export dialog.
- This is not an application restore, a second bot CRUD engine, two-way Git sync, or a replacement for security reviews before applying untrusted configuration.
