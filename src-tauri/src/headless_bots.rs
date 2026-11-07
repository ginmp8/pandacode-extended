//! Headless bot configuration (docs/headless.md): `pandacode serve --import-bots bots.json`
//! applies one declarative file through the same constructors the Settings UI uses, so a
//! server needs no window to set up bots; `--export-bots` writes the secret-free shape
//! back. Applying is pure (Store in, report out) — only starting the added channels needs
//! the AppHandle, which is the caller's job in the serve process.

use crate::bots::{self, channels, guard, schedule, shared_channels};
use crate::storage::Store;
use serde_json::{json, Value};
use std::path::Path;

/// A channel the import added; the serve wiring starts it live.
pub(crate) struct AddedChannel {
    pub id: String,
}

pub(crate) struct Report {
    pub lines: Vec<String>,
    pub added_channels: Vec<AddedChannel>,
    pub failed: usize,
}

/// Applies the import document. Bots are matched by exact name (existing = profile update,
/// missing = created with their own workspace folder); channels match on kind+label,
/// schedules on title (or prompt for untitled ones), rules on pattern+decision — so
/// re-applying the same file converges instead of duplicating. A bot that fails is
/// reported and skipped; the rest still import.
pub(crate) fn apply(store: &Store, root: &Path, doc: &Value, now: i64) -> Report {
    let mut report = Report { lines: Vec::new(), added_channels: Vec::new(), failed: 0 };
    let Some(entries) = doc.get("bots").and_then(Value::as_array) else {
        report.failed += 1;
        report.lines.push("The import file needs a \"bots\" array.".into());
        return report;
    };
    for entry in entries {
        let name = entry.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
        match apply_bot(store, root, entry, &mut report.added_channels, now) {
            Ok(lines) => report.lines.extend(lines),
            Err(error) => {
                report.failed += 1;
                report.lines.push(format!("bot \"{name}\": {error}"));
            }
        }
    }
    if let Some(entries) = doc.get("sharedChannels").filter(|value| !value.is_null()) {
        if let Some(entries) = entries.as_array() {
            for entry in entries {
                match apply_shared_channel(store, entry) {
                    Ok(line) => report.lines.push(line),
                    Err(error) => {
                        report.failed += 1;
                        report.lines.push(format!("shared channel: {error}"));
                    }
                }
            }
        } else {
            report.failed += 1;
            report.lines.push("sharedChannels must be an array or null.".into());
        }
    }
    // Restore channel-specific Extended settings after their channels have been imported.
    for entry in entries {
        if let Some(overrides) = entry.get("extended")
            .and_then(|value| value.get("sharedChannelHistoryOverrides"))
            .filter(|value| !value.is_null()) {
            let name = entry.get("name").and_then(Value::as_str).unwrap_or("");
            if let Err(error) = apply_history_overrides(store, name, overrides) {
                report.failed += 1;
                report.lines.push(format!("bot \"{name}\" shared-channel preferences: {error}"));
            }
        }
    }
    report
}

fn apply_bot(store: &Store, root: &Path, entry: &Value, added: &mut Vec<AddedChannel>, now: i64) -> crate::Result<Vec<String>> {
    let raw_name = entry.get("name").and_then(Value::as_str).unwrap_or("");
    if raw_name.trim().is_empty() {
        return Err("Every bot needs a name.".into());
    }
    // Keep native BotInput defaults for original manifests. Only the newly exported optional
    // preferences preserve their previous value when omitted or explicitly null.
    let canonical = bots::clean(raw_name, 60);
    let existing = bots::list(store)?.into_iter().find(|bot| bot.name == canonical);
    let mut prepared = entry.clone();
    let fields = ["reasoningEffort", "backupModels", "fallbackMode", "color", "notify", "whileWorking", "showPlan"];
    let object = prepared.as_object_mut().ok_or("unreadable bot entry: expected an object")?;
    if let Some(saved) = &existing {
        let saved = serde_json::to_value(saved).map_err(|error| error.to_string())?;
        for key in fields {
            if object.get(key).is_none_or(Value::is_null) {
                if let Some(value) = saved.get(key) {
                    object.insert(key.to_owned(), value.clone());
                }
            }
        }
    } else {
        for key in fields {
            if object.get(key).is_some_and(Value::is_null) {
                object.remove(key);
            }
        }
    }
    let input: bots::BotInput =
        serde_json::from_value(prepared).map_err(|error| format!("unreadable bot entry: {error}"))?;
    let (name, mut lines) = match &existing {
        Some(bot) => {
            bots::update(store, &bot.id, &input)?;
            (bot.id.clone(), vec![format!("\"{}\" updated", bot.name)])
        }
        None => {
            let bot = bots::create(store, &input, root)?;
            (bot.id.clone(), vec![format!("\"{}\" imported", bot.name)])
        }
    };
    if let Some(extended) = entry.get("extended").filter(|value| !value.is_null()) {
        let values = extended.as_object().ok_or("extended must be an object or null")?;
        for (key, value) in values {
            if value.is_null() { continue; }
            if key == "sharedChannelHistoryOverrides" {
                if !value.is_array() { return Err("extended.sharedChannelHistoryOverrides must be an array or null.".into()); }
                continue;
            }
            let enabled = value.as_bool().ok_or_else(|| format!("extended.{key} must be boolean or null"))?;
            match key.as_str() {
                "freshContextEachMessage" => { bots::extended::set(store, &name, enabled)?; }
                "includeSharedChannelHistory" => { bots::extended::set_channel_history(store, &name, enabled)?; }
                _ => return Err(format!("Unsupported extended preference: {key}")),
            }
        }
    }
    if entry.get("skills").is_some() || entry.get("mcp").is_some() {
        let skills = canonical_capabilities(entry.get("skills"))?;
        let mcp = canonical_capabilities(entry.get("mcp"))?;
        // set_capabilities bumps the brief version unconditionally; skip when the saved
        // lists already match, so re-applying a file does not re-broadcast briefs.
        let unchanged = existing.is_some_and(|bot| {
            same_list(&skills, &bot.skills) && same_list(&mcp, &bot.mcp)
        });
        if !unchanged {
            bots::set_capabilities(store, &name, &skills, &mcp)?;
            lines.push(format!("  capabilities: {} skill(s), {} MCP server(s)", skills.len(), mcp.len()));
        }
    }
    if let Some(entries) = entry.get("channels").and_then(Value::as_array) {
        for channel_entry in entries {
            let mut input: channels::ChannelInput = serde_json::from_value(channel_entry.clone())
                .map_err(|error| format!("unreadable channel entry: {error}"))?;
            let label = input.label.trim().chars().take(60).collect::<String>();
            let enabled = channel_entry.get("enabled").filter(|value| !value.is_null())
                .map(|value| value.as_bool().ok_or("Channel enabled must be boolean or null."))
                .transpose()?;
            let reconcile = channel_entry.get("reconcile").filter(|value| !value.is_null())
                .map(|value| value.as_bool().ok_or("Channel reconcile must be boolean or null."))
                .transpose()?.unwrap_or(false);
            let matches: Vec<_> = channels::list(store, &name)?.into_iter()
                .filter(|channel| channel.kind == input.kind && channel.label == label).collect();
            if matches.len() > 1 {
                return Err(format!("Ambiguous {} channel \"{label}\".", input.kind));
            }
            if let Some(saved) = matches.first() {
                if !reconcile {
                    lines.push(format!("  channel {} \"{label}\" already configured", input.kind));
                    continue;
                }
                if !input.secrets.is_empty() {
                    return Err(format!("  channel {} \"{label}\": existing credentials cannot be replaced by a manifest", input.kind));
                }
                let mut patch = serde_json::Map::new();
                if let Some(config) = channel_entry.get("config").filter(|value| !value.is_null()) {
                    for (key, value) in config.as_object().ok_or("Channel config must be an object.")? {
                        if !matches!((input.kind.as_str(), key.as_str()), ("web", "remote" | "publicUrl") | ("whatsapp", "publicUrl")) {
                            if key == "phoneNumberId" && input.kind == "whatsapp" && saved.config.get(key) == Some(value) {
                                continue;
                            }
                            return Err(format!("  channel {} \"{label}\": {key} cannot be reconciled", input.kind));
                        }
                        if saved.config.get(key) != Some(value) {
                            patch.insert(key.clone(), value.clone());
                        }
                    }
                }
                // Validate and persist channel settings before toggling availability.
                // An invalid endpoint must not unintentionally disable a working channel.
                if !patch.is_empty() {
                    channels::configure(store, &saved.id, &Value::Object(patch))?;
                }
                if let Some(value) = enabled {
                    if value != saved.enabled {
                        store.db.execute("UPDATE bot_channels SET enabled=?2 WHERE id=?1",
                            rusqlite::params![saved.id, value]).map_err(|error| error.to_string())?;
                    }
                }
                if enabled.is_some_and(|value| value != saved.enabled) || channels::get(store, &saved.id)?.config != saved.config {
                    added.push(AddedChannel { id: saved.id.clone() });
                    lines.push(format!("  channel {} \"{label}\" updated", input.kind));
                } else {
                    lines.push(format!("  channel {} \"{label}\" already configured", input.kind));
                }
                continue;
            }
            for (secret_name, value) in input.secrets.iter_mut() {
                *value = resolve_secret(&input.kind, secret_name, value, &|variable| std::env::var(variable).ok())?;
            }
            channels::check_format(&input)?;
            let channel = channels::add(store, &name, &input)?;
            if enabled == Some(false) {
                store.db.execute("UPDATE bot_channels SET enabled=0 WHERE id=?1", [&channel.id])
                    .map_err(|error| error.to_string())?;
            }
            added.push(AddedChannel { id: channel.id.clone() });
            lines.push(format!("  channel {} \"{label}\" added{}", channel.kind,
                if enabled == Some(false) { " (disabled)" } else { "" }));
        }
    }
    if let Some(entries) = entry.get("schedules").and_then(Value::as_array) {
        for schedule_entry in entries {
            let input: schedule::ScheduleInput = serde_json::from_value(schedule_entry.clone())
                .map_err(|error| format!("unreadable schedule entry: {error}"))?;
            let title = input.title.trim().chars().take(120).collect::<String>();
            let prompt = input.prompt.trim().chars().take(8000).collect::<String>();
            let exists = schedule::list(store, &name)?.iter().any(|saved| {
                if title.is_empty() {
                    saved.prompt == prompt && saved.kind == input.kind
                } else {
                    saved.title == title
                }
            });
            if exists {
                lines.push(format!("  schedule \"{}\" already configured", if title.is_empty() { &input.prompt } else { &title }));
                continue;
            }
            let created = schedule::create(store, &name, &input, "user", now)?;
            lines.push(format!("  schedule \"{}\" added", created.title));
        }
    }
    if let Some(entries) = entry.get("rules").and_then(Value::as_array) {
        for rule_entry in entries {
            let pattern = rule_entry.get("pattern").and_then(Value::as_str).ok_or("Every rule needs a pattern.")?;
            let decision = rule_entry.get("decision").and_then(Value::as_str).ok_or("Every rule needs a decision (allow, ask or deny).")?;
            // Validate what the rule will store before touching what is already there:
            // a typo'd decision must not delete a working rule.
            let pattern = pattern.trim().chars().take(200).collect::<String>();
            if pattern.trim_matches('*').trim().chars().count() < 2 {
                return Err("A rule needs a pattern of at least two characters.".into());
            }
            if !matches!(decision, "allow" | "ask" | "deny") {
                return Err("A rule allows, asks or denies.".into());
            }
            // The file is the source of truth for a pattern: an edited decision replaces
            // the old rule instead of stacking one that deny-wins would shadow.
            let mut replaced = false;
            for rule in guard::rules(store, &name)?.iter().filter(|rule| rule.pattern == pattern && rule.decision != decision).collect::<Vec<_>>() {
                guard::delete_rule(store, &name, &rule.id)?;
                replaced = true;
            }
            if guard::rules(store, &name)?.iter().any(|rule| rule.pattern == pattern && rule.decision == decision) {
                lines.push(format!("  rule \"{pattern}\" already configured{}", if replaced { " (replaced a different decision)" } else { "" }));
            } else {
                guard::add_rule(store, &name, &pattern, decision)?;
                lines.push(format!("  rule \"{pattern}\" {decision}{}", if replaced { " (replaced the old rule)" } else { "" }));
            }
        }
    }
    Ok(lines)
}

/// Restore local group-channel configuration only. Membership references are portable names;
/// assignments, messages and channel history never enter the document.
fn apply_shared_channel(store: &Store, entry: &Value) -> crate::Result<String> {
    let object = entry.as_object().ok_or("Expected a shared channel object.")?;
    let name = object.get("name").and_then(Value::as_str).ok_or("A shared channel needs a name.")?;
    let normalized = name.trim().trim_start_matches('#').trim();
    if normalized.is_empty() { return Err("A shared channel needs a name.".into()); }
    let matches: Vec<_> = shared_channels::list(store)?.into_iter()
        .filter(|channel| channel.name.eq_ignore_ascii_case(normalized)).collect();
    if matches.len() > 1 { return Err(format!("Ambiguous shared channel: {name}")); }
    let existing = matches.first();
    let all_bots = bots::list(store)?;
    let resolve = |raw: &str| -> crate::Result<String> {
        let canonical = bots::clean(raw, 60);
        let found: Vec<_> = all_bots.iter().filter(|bot| bot.name == canonical).collect();
        if found.len() != 1 { return Err(format!("Shared channel {name}: bot {raw:?} is missing or ambiguous.")); }
        Ok(found[0].id.clone())
    };
    let members = match object.get("members").filter(|value| !value.is_null()) {
        Some(value) => Some(value.as_array().ok_or("Shared channel members must be an array.")?
            .iter().map(|value| resolve(value.as_str().ok_or("Member names must be strings.")?))
            .collect::<crate::Result<Vec<_>>>()?),
        None => None,
    };
    let lead = object.get("lead").filter(|value| !value.is_null())
        .map(|value| resolve(value.as_str().ok_or("Shared channel lead must be a name.")?))
        .transpose()?;
    let archived = object.get("archived").filter(|value| !value.is_null())
        .map(|value| value.as_bool().ok_or("Shared channel archived must be boolean."))
        .transpose()?;
    let channel = match existing {
        Some(channel) => {
            let desired = members.as_deref().unwrap_or(&channel.member_ids);
            let lead = lead.as_deref().or(channel.lead_bot_id.as_deref());
            if lead.is_some_and(|id| !desired.iter().any(|member| member == id)) {
                return Err(format!("Shared channel {name}: lead must be a member."));
            }
            if channel.archived && (desired != channel.member_ids.as_slice() || lead != channel.lead_bot_id.as_deref()) {
                return Err(format!("Shared channel {name}: restore the archived channel before changing its members."));
            }
            if desired != channel.member_ids.as_slice() || lead != channel.lead_bot_id.as_deref() {
                shared_channels::update(store, &channel.id, name, desired, lead)?;
            }
            channel.clone()
        }
        None => {
            let desired = members.ok_or("A new shared channel requires members.")?;
            if desired.is_empty() { return Err("A new shared channel requires at least one member.".into()); }
            if lead.as_ref().is_some_and(|id| !desired.contains(id)) {
                return Err(format!("Shared channel {name}: lead must be a member."));
            }
            let created = shared_channels::create(store, name, &desired)?;
            if let Some(lead) = lead.as_deref() {
                if created.lead_bot_id.as_deref() != Some(lead) {
                    shared_channels::update(store, &created.id, name, &desired, Some(lead))?;
                }
            }
            created
        }
    };
    if let Some(archived) = archived {
        if channel.archived != archived {
            let active: i64 = store.db.query_row(
                "SELECT COUNT(*) FROM bot_shared_channel_deliveries WHERE channel_id=?1 AND status IN ('queued','in_progress')",
                [&channel.id], |row| row.get(0)).map_err(|error| error.to_string())?;
            if active > 0 { return Err(format!("Shared channel {name}: active work prevents archive changes.")); }
            shared_channels::set_archived(store, &channel.id, archived)?;
        }
    }
    Ok(format!("shared channel \"{name}\" configured"))
}

fn exported_history_overrides(store: &Store, bot_id: &str) -> crate::Result<Value> {
    let mut statement = store.db.prepare(
        "SELECT c.name,o.mode FROM extended_bot_channel_history_overrides o
         JOIN bot_shared_channels c ON c.id=o.channel_id
         WHERE o.bot_id=?1 ORDER BY c.name COLLATE NOCASE,c.id",
    ).map_err(|error| error.to_string())?;
    let rows = statement.query_map([bot_id], |row| {
        Ok(json!({"channel": row.get::<_, String>(0)?, "mode": row.get::<_, String>(1)?}))
    }).map_err(|error| error.to_string())?;
    let values = rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    Ok(Value::Array(values))
}

fn apply_history_overrides(store: &Store, name: &str, overrides: &Value) -> crate::Result<()> {
    let bot_name = bots::clean(name, 60);
    let bot = bots::list(store)?.into_iter().find(|bot| bot.name == bot_name)
        .ok_or_else(|| format!("Bot {bot_name:?} is missing."))?;
    let entries = overrides.as_array().ok_or("Shared channel history overrides must be an array.")?;
    let channels = shared_channels::list(store)?;
    for entry in entries {
        let channel_name = entry.get("channel").and_then(Value::as_str)
            .ok_or("Every shared channel history override needs a channel name.")?;
        let mode = entry.get("mode").and_then(Value::as_str)
            .ok_or("Every shared channel history override needs a mode.")?;
        if !matches!(mode, "default" | "include" | "exclude") {
            return Err(format!("Invalid history preference for {channel_name}."));
        }
        let found: Vec<_> = channels.iter().filter(|channel| channel.name.eq_ignore_ascii_case(channel_name)).collect();
        if found.len() != 1 { return Err(format!("Missing or ambiguous shared channel {channel_name:?}.")); }
        bots::extended::set_channel_history_override(store, &bot.id, &found[0].id, mode)?;
    }
    Ok(())
}

/// Portable channel configuration is allowlisted: the native record also contains
/// machine identity, connection state and WhatsApp verification tokens.
fn portable_channel_config(kind: &str, source: &Value) -> crate::Result<Value> {
    let object = source.as_object().ok_or("Saved channel config must be an object.")?;
    let mut config = serde_json::Map::new();
    for (key, value) in object {
        if matches!(key.as_str(), "identity" | "verifyToken") { continue; }
        match (kind, key.as_str()) {
            ("web", "remote") if value.is_boolean() => {},
            ("web" | "whatsapp", "publicUrl") if value.is_string() => {},
            ("whatsapp", "phoneNumberId") if value.as_str().is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())) => {},
            _ => return Err(format!("Unsupported or sensitive {kind} channel config property: {key}")),
        }
        config.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(config))
}

/// Capabilities as `references()` stores them: strings, trimmed, empties dropped — the
/// same canonical form the unchanged-skip compares against.
fn canonical_capabilities(value: Option<&Value>) -> crate::Result<Vec<String>> {
    Ok(string_list(value)?
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect())
}

fn same_list(doc: &[String], saved: &[String]) -> bool {
    doc.iter().collect::<std::collections::BTreeSet<_>>() == saved.iter().collect::<std::collections::BTreeSet<_>>()
}

/// A secret is either its value or `env:NAME`, read from the environment at import
/// (the lookup is a parameter so tests never touch the process environment).
fn resolve_secret(kind: &str, name: &str, value: &str, lookup: &dyn Fn(&str) -> Option<String>) -> crate::Result<String> {
    let Some(variable) = value.strip_prefix("env:") else {
        return Ok(value.to_owned());
    };
    let resolved = lookup(variable).ok_or_else(|| format!("{kind} channel ({name}): environment variable {variable} is not set"))?;
    if resolved.trim().is_empty() {
        return Err(format!("{kind} channel ({name}): environment variable {variable} is empty"));
    }
    Ok(resolved)
}

fn string_list(value: Option<&Value>) -> crate::Result<Vec<String>> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned).ok_or_else(|| "Capability lists hold strings.".to_string()))
            .collect(),
        Some(_) => Err("Capability lists are arrays of strings.".into()),
    }
}

/// The importable shape of everything configured, minus secrets: channels list the
/// secret *names* under `secretsNeeded`, web tokens are left out entirely (a re-import
/// mints a fresh one and prints the new link), and fired once-schedules are omitted
/// (their run time is gone; exporting them would fail `schedule::create` on import).
pub(crate) fn export(store: &Store) -> crate::Result<Value> {
    let mut bots_json = Vec::new();
    for bot in bots::list(store)? {
        let mut channels_json = Vec::new();
        for channel in channels::list(store, &bot.id)? {
            let config = portable_channel_config(&channel.kind, &channel.config)?;
            channels_json.push(json!({
                "kind": channel.kind, "label": channel.label, "config": config,
                "enabled": channel.enabled, "reconcile": true,
                "secretsNeeded": channel.secrets,
            }));
        }
        let schedules_json = schedule::list(store, &bot.id)?
            .iter()
            // A once-schedule whose run time is gone (fired, or drifted past while
            // disabled) cannot re-import: create rejects a stale or missing runAt.
            .filter(|saved| !(saved.kind == "once" && saved.next_run_at.is_none_or(|at| at <= crate::storage::now())))
            .map(|saved| {
                json!({
                    "title": saved.title, "prompt": saved.prompt, "kind": saved.kind,
                    "intervalMinutes": saved.interval_minutes, "timeOfDay": saved.time_of_day,
                    "runAt": (saved.kind == "once").then_some(saved.next_run_at).flatten(),
                })
            })
            .collect::<Vec<_>>();
        let rules_json = guard::rules(store, &bot.id)?
            .iter()
            .map(|rule| json!({"pattern": rule.pattern, "decision": rule.decision}))
            .collect::<Vec<_>>();
        bots_json.push(json!({
            "name": bot.name, "role": bot.role, "instructions": bot.instructions,
            "goals": bot.goals, "soul": bot.soul, "avatar": bot.avatar,
            "provider": bot.provider, "model": bot.model,
            "mode": bot.mode, "heartbeatMinutes": bot.heartbeat_minutes,
            "color": bot.color, "reasoningEffort": bot.reasoning_effort,
            "backupModels": bot.backup_models, "fallbackMode": bot.fallback_mode,
            "notify": bot.notify, "whileWorking": bot.while_working, "showPlan": bot.show_plan,
            "extended": {
                "freshContextEachMessage": bots::extended::enabled(store, &bot.id)?,
                "includeSharedChannelHistory": bots::extended::include_shared_channel_history(store, &bot.id)?,
                "sharedChannelHistoryOverrides": exported_history_overrides(store, &bot.id)?,
            },
            "skills": bot.skills, "mcp": bot.mcp,
            "channels": channels_json, "schedules": schedules_json, "rules": rules_json,
        }));
    }
    let all_bots = bots::list(store)?;
    let mut local_channels = Vec::new();
    for channel in shared_channels::list(store)? {
        let names = channel.member_ids.iter()
            .map(|id| all_bots.iter().find(|bot| bot.id == *id)
                .map(|bot| bot.name.clone()).ok_or_else(|| format!("Missing member in shared channel {}", channel.name)))
            .collect::<crate::Result<Vec<_>>>()?;
        let lead = channel.lead_bot_id.as_ref()
            .map(|id| all_bots.iter().find(|bot| bot.id == *id)
                .map(|bot| bot.name.clone()).ok_or_else(|| format!("Missing lead in shared channel {}", channel.name)))
            .transpose()?;
        local_channels.push(json!({ "name": channel.name, "members": names,
            "lead": lead, "archived": channel.archived }));
    }
    Ok(json!({ "bots": bots_json, "sharedChannels": local_channels }))
}

/// Reads and applies the import file inside the serve process, starts the added channels
/// live, prints web chat links. Exits non-zero on any failure, after the per-bot report
/// (systemd shows it); what imported before the failure stays, so fixing the file and
/// restarting converges.
pub(crate) fn run_import(app: &tauri::AppHandle, path: &Path) {
    use tauri::Manager;
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("Could not read the bot import file {}: {error}", path.display());
            std::process::exit(1)
        }
    };
    let doc: Value = match serde_json::from_str(&text) {
        Ok(doc) => doc,
        Err(error) => {
            eprintln!("The bot import file is not valid JSON: {error}");
            std::process::exit(1)
        }
    };
    let state = app.state::<crate::AppState>();
    let data_dir = app.path().app_data_dir().unwrap_or_else(|error| {
        eprintln!("Could not find the bot backup directory: {error}");
        std::process::exit(1)
    });
    let (report, backup_path) = {
        let Ok(store) = state.store.lock() else {
            eprintln!("Could not open the task store for the bot import.");
            std::process::exit(1)
        };
        crate::extended_bot_manifest::apply_with_backup(&store, &data_dir, &doc, crate::storage::now())
            .unwrap_or_else(|error| {
                eprintln!("Bot import blocked before mutation: {error}");
                std::process::exit(1)
            })
    };
    println!("Bot configuration backup: {}", backup_path.display());
    for line in &report.lines {
        println!("{line}");
    }
    if report.failed > 0 {
        eprintln!("The bot import had errors; fix the file and restart — what imported stays.");
        std::process::exit(1)
    }
    let mut links = Vec::new();
    for added in &report.added_channels {
        let channel = {
            let Ok(store) = state.store.lock() else {
                eprintln!("serve: could not re-read an imported channel; it will start on the next restart.");
                continue;
            };
            match channels::get(&store, &added.id) {
                Ok(channel) => channel,
                Err(error) => {
                    eprintln!("serve: an imported channel did not come back: {error}");
                    continue;
                }
            }
        };
        channels::start(app, channel.clone());
        if channel.kind == "web" {
            let port = channels::http::ensure_server(app.clone());
            if port == 0 {
                eprintln!("serve: the local chat server could not bind a port; web chat links are unavailable.");
                continue;
            }
            let token = {
                let Ok(store) = state.store.lock() else { continue };
                channels::web_token(&store, &channel.id)
            };
            if let Some(token) = token {
                links.push(format!("http://127.0.0.1:{port}/chat/{}#{token}", channel.id));
            }
        }
    }
    for link in links {
        println!("Web chat: {link}");
    }
}

/// Writes the export document to `path` (pretty-printed, newline-terminated).
pub(crate) fn run_export(app: &tauri::AppHandle, path: &Path) -> crate::Result<()> {
    use tauri::Manager;
    let state = app.state::<crate::AppState>();
    let doc = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        export(&store)?
    };
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| format!("Could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_store;

    fn store() -> Store {
        Store::open(Path::new(":memory:")).unwrap()
    }

    fn root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("pandacode-bot-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// A per-test variable name that is never set: reading a missing variable is safe
    /// in parallel tests; writing the environment is not.
    fn token_doc(var: &str) -> Value {
        serde_json::from_str(&format!(
            r#"{{"bots": [{{
                "name": "build-bot", "role": "builder", "instructions": "Build things.",
                "goals": "Ship it", "provider": "claude", "model": "sonnet", "mode": "auto",
                "avatar": "panda:3",
                "heartbeatMinutes": 30, "skills": ["skill-a"], "mcp": ["server-a"],
                "channels": [
                    {{"kind": "telegram", "label": "main", "config": {{}}, "secrets": {{"token": "{var}"}}}},
                    {{"kind": "web"}}
                ],
                "schedules": [{{"title": "Standup", "prompt": "What is next?", "kind": "daily", "timeOfDay": "09:00"}}],
                "rules": [{{"pattern": "npm test*", "decision": "allow"}}]
            }}]}}"#
        ))
        .unwrap()
    }

    #[test]
    fn import_creates_bot_channel_secrets_schedule_and_rule() {
        let store = store();
        let root = root();
        let report = apply(&store, &root, &token_doc("123456:AAtesttokenvalueaaaaaaaaaaaaaaaaa"), 1_000_000);
        assert_eq!(report.failed, 0, "{:?}", report.lines);
        assert_eq!(report.added_channels.len(), 2);
        let list = bots::list(&store).unwrap();
        assert_eq!(list.len(), 1);
        let bot = &list[0];
        assert_eq!((bot.provider.as_str(), bot.mode.as_str(), bot.heartbeat_minutes), ("claude", "auto", 30));
        assert_eq!(bot.skills, vec!["skill-a".to_owned()]);
        assert_eq!(bot.goals, "Ship it");
        assert_eq!(bot.avatar, "panda:3");
        // The telegram channel saved the token under the file backend's key.
        let channel = channels::list(&store, &bot.id).unwrap().into_iter().find(|c| c.kind == "telegram").unwrap();
        assert_eq!(channel.secrets, vec!["token".to_owned()]);
        assert_eq!(secret_store::get("app.pandacode.bots", &format!("{}:token", channel.id)).as_deref(), Some("123456:AAtesttokenvalueaaaaaaaaaaaaaaaaa"));
        // The web channel minted a link token.
        let web = channels::list(&store, &bot.id).unwrap().into_iter().find(|c| c.kind == "web").unwrap();
        let token = channels::web_token(&store, &web.id).unwrap();
        assert_eq!(token.len(), 64);
        // The schedule got a computed next run and the rule stored its decision.
        let schedules = schedule::list(&store, &bot.id).unwrap();
        assert_eq!(schedules.len(), 1);
        assert!(schedules[0].next_run_at.is_some());
        assert_eq!(schedules[0].created_by, "user");
        assert_eq!(guard::rules(&store, &bot.id).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reapplying_the_same_file_converges() {
        let store = store();
        let root = root();
        let doc = token_doc("123456:AAtesttokenvalueaaaaaaaaaaaaaaaaa");
        apply(&store, &root, &doc, 1_000_000);
        let report = apply(&store, &root, &doc, 2_000_000);
        assert_eq!(report.failed, 0, "{:?}", report.lines);
        assert_eq!(report.added_channels.len(), 0, "channels are matched, not re-added");
        let bots = bots::list(&store).unwrap();
        assert_eq!(bots.len(), 1);
        // Unchanged capabilities skip the write: the brief is not re-broadcast.
        assert_eq!(bots[0].memory_version, 1);
        assert_eq!(channels::list(&store, &bots[0].id).unwrap().len(), 2);
        assert_eq!(schedule::list(&store, &bots[0].id).unwrap().len(), 1);
        assert_eq!(guard::rules(&store, &bots[0].id).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_failing_bot_does_not_block_the_others() {
        let store = store();
        let root = root();
        let doc: Value = serde_json::from_str(
            r#"{"bots": [
                {"name": "good", "provider": "claude", "model": "sonnet", "mode": "auto"},
                {"name": "bad", "provider": "nope", "model": "x", "mode": "auto"}
            ]}"#,
        )
        .unwrap();
        let report = apply(&store, &root, &doc, 1_000_000);
        assert_eq!(report.failed, 1);
        assert!(report.lines.iter().any(|line| line.contains("bad")));
        let bots = bots::list(&store).unwrap();
        assert_eq!(bots.len(), 1);
        assert_eq!(bots[0].name, "good");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn export_is_config_only_and_secret_free() {
        let store = store();
        let root = root();
        apply(&store, &root, &token_doc("123456:AAtesttokenvalueaaaaaaaaaaaaaaaaa"), 1_000_000);
        let exported = export(&store).unwrap();
        let text = serde_json::to_string(&exported).unwrap();
        assert!(!text.contains("AAtesttokenvalue"), "secret values never export");
        assert!(!text.to_lowercase().contains("webtoken"), "web tokens never export");
        let bot = &exported["bots"][0];
        assert_eq!(bot["name"], "build-bot");
        assert_eq!(bot["avatar"], "panda:3", "the avatar round-trips instead of being wiped");
        assert_eq!(bot["channels"][0]["secretsNeeded"], serde_json::json!(["token"]));
        assert_eq!(bot["rules"][0]["decision"], "allow");
        // An export re-imports cleanly on the same server: the profile updates and the
        // existing channels match by kind+label, so no secret is needed again. (On a
        // fresh server the missing secret is the named per-bot error instead.)
        let report = apply(&store, &root, &exported, 3_000_000);
        assert_eq!(report.failed, 0, "{:?}", report.lines);
        assert!(report.lines.iter().any(|line| line.contains("already configured")));
        assert_eq!(bots::list(&store).unwrap().len(), 1);
        assert_eq!(bots::list(&store).unwrap()[0].avatar, "panda:3", "the avatar survives the round trip");
        assert_eq!(channels::list(&store, &bots::list(&store).unwrap()[0].id).unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(root);
    }


    #[test]
    fn optional_preferences_preserve_absent_null_and_explicit_false() {
        let store = store();
        let root = root();
        let original = json!({"bots":[{"name":"tester","provider":"claude","model":"sonnet","mode":"auto",
            "reasoningEffort":"high","fallbackMode":"auto","notify":false,
            "whileWorking":"steer","showPlan":true,
            "extended":{"freshContextEachMessage":true,"includeSharedChannelHistory":false}}]});
        assert_eq!(apply(&store, &root, &original, 1_000_000).failed, 0);
        let legacy = json!({"bots":[{"name":"tester","provider":"claude","model":"sonnet","mode":"auto",
            "notify":null,"reasoningEffort":null,"extended":null}]});
        assert_eq!(apply(&store, &root, &legacy, 1_000_000).failed, 0);
        let saved = bots::list(&store).unwrap().remove(0);
        assert_eq!(saved.reasoning_effort, "high");
        assert_eq!(saved.fallback_mode, "auto");
        assert!(!saved.notify);
        assert_eq!(saved.while_working, "steer");
        assert!(saved.show_plan);
        assert!(bots::extended::enabled(&store, &saved.id).unwrap());
        assert!(!bots::extended::include_shared_channel_history(&store, &saved.id).unwrap());
        let exported = export(&store).unwrap();
        assert_eq!(exported["bots"][0]["notify"], false);
        assert_eq!(exported["bots"][0]["extended"]["freshContextEachMessage"], true);
        assert_eq!(apply(&store, &root, &exported, 1_000_000).failed, 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn local_shared_channels_use_portable_bot_names_and_remain_idempotent() {
        let store = store();
        let root = root();
        let doc = json!({"bots":[
            {"name":"dev","provider":"claude","model":"sonnet","mode":"auto"},
            {"name":"review","provider":"claude","model":"sonnet","mode":"auto"}],
            "sharedChannels":[{"name":"launch","members":["dev","review"],"lead":"review","archived":false}]});
        assert_eq!(apply(&store, &root, &doc, 1_000_000).failed, 0);
        assert_eq!(apply(&store, &root, &doc, 1_000_000).failed, 0);
        assert_eq!(shared_channels::list(&store).unwrap().len(), 1);
        let output = export(&store).unwrap();
        assert_eq!(output["sharedChannels"][0]["members"], json!(["dev","review"]));
        assert_eq!(output["sharedChannels"][0]["lead"], "review");
        assert_eq!(apply(&store, &root, &json!({"bots":[],"sharedChannels":null}), 1_000_000).failed, 0);
        assert_eq!(shared_channels::list(&store).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn external_channel_reconciliation_is_opt_in_and_preserves_identity() {
        let store = store();
        let root = root();
        let doc = json!({"bots":[{"name":"web","provider":"claude","model":"sonnet","mode":"auto",
            "channels":[{"kind":"web","label":"local","config":{"remote":false}}]}]});
        assert_eq!(apply(&store, &root, &doc, 1_000_000).failed, 0);
        let bot = bots::list(&store).unwrap().remove(0);
        let before = channels::list(&store, &bot.id).unwrap().remove(0);
        let token = channels::web_token(&store, &before.id).unwrap();
        let legacy = json!({"bots":[{"name":"web","provider":"claude","model":"sonnet","mode":"auto",
            "channels":[{"kind":"web","label":"local","config":{"remote":true},"enabled":false}]}]});
        assert_eq!(apply(&store, &root, &legacy, 1_000_000).failed, 0);
        assert_eq!(channels::get(&store, &before.id).unwrap().enabled, true);
        let modern = json!({"bots":[{"name":"web","provider":"claude","model":"sonnet","mode":"auto",
            "channels":[{"kind":"web","label":"local","reconcile":true,
                "config":{"remote":true},"enabled":false}]}]});
        assert_eq!(apply(&store, &root, &modern, 1_000_000).failed, 0);
        let after = channels::get(&store, &before.id).unwrap();
        assert!(!after.enabled);
        assert_eq!(after.config["remote"], true);
        assert_eq!(channels::web_token(&store, &before.id).unwrap(), token);
        let exported = export(&store).unwrap();
        assert_eq!(exported["bots"][0]["channels"][0]["enabled"], false);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_env_and_malformed_entries_report_per_bot() {
        let store = store();
        let root = root();
        let var = format!("TEST_TOKEN_{}", uuid::Uuid::new_v4().simple());
        let doc: Value = serde_json::from_str(&format!(
            r#"{{"bots": [
                {{"name": "tg", "provider": "claude", "model": "sonnet", "mode": "auto",
                 "channels": [{{"kind": "telegram", "secrets": {{"token": "env:{var}"}}}}]}},
                {{"name": 42}}
            ]}}"#
        ))
        .unwrap();
        let report = apply(&store, &root, &doc, 1_000_000);
        assert_eq!(report.failed, 2);
        assert!(report.lines.iter().any(|line| line.contains(&var)));
        assert!(report.lines.iter().any(|line| line.contains("unreadable bot entry")));
        // Not a document: no bots array.
        let report = apply(&store, &root, &serde_json::json!({"nope": true}), 1_000_000);
        assert_eq!(report.failed, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn non_canonical_names_and_truncations_converge() {
        let store = store();
        let root = root();
        // Constructors store the cleaned name; the match must use the same form.
        let doc: Value = serde_json::from_str(
            r#"{"bots": [{"name": "  build-bot ", "provider": "claude", "model": "sonnet", "mode": "auto"}]}"#,
        )
        .unwrap();
        apply(&store, &root, &doc, 1_000_000);
        let report = apply(&store, &root, &doc, 2_000_000);
        assert_eq!(report.failed, 0, "{:?}", report.lines);
        assert!(report.lines.iter().any(|line| line.contains("updated")), "{:?}", report.lines);
        assert_eq!(bots::list(&store).unwrap().len(), 1);
        // A >60-char name is capped by the constructor and must still converge.
        let long: Value = serde_json::from_str(&format!(
            r#"{{"bots": [{{"name": "{}", "provider": "claude", "model": "sonnet", "mode": "auto"}}]}}"#,
            "x".repeat(80)
        ))
        .unwrap();
        apply(&store, &root, &long, 3_000_000);
        let report = apply(&store, &root, &long, 4_000_000);
        assert_eq!(report.failed, 0);
        assert_eq!(bots::list(&store).unwrap().len(), 2, "the long-named bot updates instead of duplicating");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rule_decision_edits_replace_instead_of_stack() {
        let store = store();
        let root = root();
        let v1: Value = serde_json::from_str(
            r#"{"bots": [{"name": "bot", "provider": "claude", "model": "sonnet", "mode": "auto",
                "rules": [{"pattern": "npm test*", "decision": "deny"}]}]}"#,
        )
        .unwrap();
        let v2: Value = serde_json::from_str(
            r#"{"bots": [{"name": "bot", "provider": "claude", "model": "sonnet", "mode": "auto",
                "rules": [{"pattern": "npm test*", "decision": "allow"}]}]}"#,
        )
        .unwrap();
        let bot = || bots::list(&store).unwrap().remove(0);
        apply(&store, &root, &v1, 1_000_000);
        let report = apply(&store, &root, &v2, 2_000_000);
        assert_eq!(report.failed, 0, "{:?}", report.lines);
        let rules = guard::rules(&store, &bot().id).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].decision, "allow", "the edited decision must take effect");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_once_schedules_do_not_export() {
        let store = store();
        let root = root();
        // Three once-schedules: "Deploy check" stays pending (exports); "Fired" gets its
        // run time cleared by advance (next_run_at NULL); "Old" drifts into the past
        // while disabled. The two stale ones cannot re-import and must not export.
        // Real clock throughout: export filters stale runAt against it.
        let now = crate::storage::now();
        let future = now + 30 * 86400 * 1000;
        let doc: Value = serde_json::from_str(&format!(
            r#"{{"bots": [{{"name": "bot", "provider": "claude", "model": "sonnet", "mode": "auto",
                "schedules": [
                    {{"title": "Deploy check", "prompt": "go", "kind": "once", "runAt": {future}}},
                    {{"title": "Fired", "prompt": "done", "kind": "once", "runAt": {future}}},
                    {{"title": "Old", "prompt": "gone", "kind": "once", "runAt": {future}}}
                ]}}]}}"#
        ))
        .unwrap();
        apply(&store, &root, &doc, now);
        let bot = bots::list(&store).unwrap().remove(0);
        store.db.execute("UPDATE bot_schedules SET next_run_at=NULL,enabled=0 WHERE title='Fired'", []).unwrap();
        store.db.execute("UPDATE bot_schedules SET next_run_at=1000,enabled=0 WHERE title='Old'", []).unwrap();
        let exported = export(&store).unwrap();
        let schedules = exported["bots"][0]["schedules"].as_array().unwrap();
        assert_eq!(schedules.len(), 1, "{schedules:?}");
        assert_eq!(schedules[0]["title"], "Deploy check");
        // Re-importing the export converges (the stale rows stay history, not config).
        let report = apply(&store, &root, &exported, 2_000_000);
        assert_eq!(report.failed, 0, "{:?}", report.lines);
        assert_eq!(schedule::list(&store, &bot.id).unwrap().len(), 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_typoed_decision_does_not_destroy_the_stored_rule() {
        let store = store();
        let root = root();
        let good: Value = serde_json::from_str(
            r#"{"bots": [{"name": "bot", "provider": "claude", "model": "sonnet", "mode": "auto",
                "rules": [{"pattern": "npm test*", "decision": "allow"}]}]}"#,
        )
        .unwrap();
        let typo: Value = serde_json::from_str(
            r#"{"bots": [{"name": "bot", "provider": "claude", "model": "sonnet", "mode": "auto",
                "rules": [{"pattern": "npm test*", "decision": "alow"}]}]}"#,
        )
        .unwrap();
        apply(&store, &root, &good, 1_000_000);
        let report = apply(&store, &root, &typo, 2_000_000);
        assert_eq!(report.failed, 1);
        let rules = guard::rules(&store, &bots::list(&store).unwrap().remove(0).id).unwrap();
        assert_eq!(rules.len(), 1, "the invalid file must not delete the working rule");
        assert_eq!(rules[0].decision, "allow");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bad_token_shapes_fail_the_import() {
        let store = store();
        let root = root();
        // Slack's two tokens pasted in the wrong boxes: exactly what check_format exists to catch.
        let bad: Value = serde_json::from_str(
            r#"{"bots": [{"name": "bot", "provider": "claude", "model": "sonnet", "mode": "auto",
                "channels": [{"kind": "slack", "secrets": {"token": "xapp-notabottoken0123456789", "app_token": "xoxb-notanapptoken0123456789"}}]}]}"#,
        )
        .unwrap();
        let report = apply(&store, &root, &bad, 1_000_000);
        assert_eq!(report.failed, 1, "{:?}", report.lines);
        assert!(report.lines.iter().any(|line| line.to_lowercase().contains("swap")), "{:?}", report.lines);
        // The bot itself was created before its channel failed: fixing the file and
        // re-applying converges (update the profile, add the channel).
        let good: Value = serde_json::from_str(
            r#"{"bots": [{"name": "bot", "provider": "claude", "model": "sonnet", "mode": "auto",
                "channels": [{"kind": "slack", "label": "main", "secrets": {"token": "xoxb-realbottokenvalue012345678", "app_token": "xapp-realtokenvalue0123456789"}}]}]}"#,
        )
        .unwrap();
        let report = apply(&store, &root, &good, 2_000_000);
        assert_eq!(report.failed, 0, "{:?}", report.lines);
        let bot = bots::list(&store).unwrap().remove(0);
        assert_eq!(channels::list(&store, &bot.id).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn secrets_resolve_from_the_environment_or_pass_through() {
        let lookup = |name: &str| Some(format!("value-for-{name}"));
        assert_eq!(resolve_secret("telegram", "token", "plain", &lookup).unwrap(), "plain");
        assert_eq!(resolve_secret("telegram", "token", "env:TOKEN", &lookup).unwrap(), "value-for-TOKEN");
        assert!(resolve_secret("telegram", "token", "env:MISSING", &|_| None).unwrap_err().contains("MISSING"));
        assert!(resolve_secret("telegram", "token", "env:EMPTY", &|_| Some("  ".into())).unwrap_err().contains("EMPTY"));
    }
}
