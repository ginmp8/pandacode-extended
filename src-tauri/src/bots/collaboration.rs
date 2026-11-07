//! Extended-owned, explicit local Shared Channel collaboration for existing bots.
//! The upstream channel timeline and gateway remain authoritative for visible work.

use super::{gateway, shared_channels, Bot, Thread, DESKTOP};
use crate::{storage::Store, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tauri::{Emitter, Manager};

const MAX_TEXT: usize = 16 * 1024;

/// Separate metadata keeps upstream message, delivery, bot and profile schemas intact.
pub(crate) fn upgrade(db: &rusqlite::Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS extended_bot_channel_actions(
            message_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL CHECK(kind IN ('post','assignment')),
            origin_bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
            origin_session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            origin_channel_id TEXT NOT NULL,
            origin_external_id TEXT NOT NULL,
            recipients TEXT NOT NULL CHECK(json_valid(recipients)),
            parent_request_id TEXT,
            depth INTEGER NOT NULL CHECK(depth BETWEEN 0 AND 3),
            route TEXT NOT NULL CHECK(json_valid(route)),
            idempotency_key TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS extended_bot_channel_origin
            ON extended_bot_channel_actions(origin_session_id,created_at,message_id);
        CREATE TABLE IF NOT EXISTS extended_bot_channel_results(
            delivery_id TEXT PRIMARY KEY,
            result TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('sending','sent','failed')),
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS extended_bot_direct_routes(
            message_id TEXT PRIMARY KEY,
            source_delivery_id TEXT,
            parent_request_id TEXT,
            route TEXT NOT NULL CHECK(json_valid(route))
        );",
    ).map_err(|e| e.to_string())?;
    // Some legacy bot-profile upgrade tests construct only the bots table, without
    // a sessions table. Preserve the session FK and defer sidecar cleanup until
    // the complete application database is available; never weaken that fixture.
    let has_sessions: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sessions')",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if !has_sessions { return Ok(()); }
    // The app can reconstruct upstream channel tables while migrating older databases.
    // Avoid cross-table foreign keys to replaceable upstream tables, then prune orphans.
    db.execute_batch(
        "DELETE FROM extended_bot_channel_results WHERE delivery_id NOT IN
           (SELECT id FROM bot_shared_channel_deliveries);
         DELETE FROM extended_bot_direct_routes WHERE message_id NOT IN
           (SELECT id FROM bot_messages);
         DELETE FROM extended_bot_channel_actions WHERE message_id NOT IN
           (SELECT id FROM bot_shared_channel_messages);",
    ).map_err(|e| e.to_string())
}

type CallbackInfo = (String, String, String, String, String, String);
type CallbackRow = (String, String, String, String, String, String, String, String);

#[derive(Clone, Debug)]
pub(crate) struct Route {
    pub trace: Vec<String>,
    pub depth: u8,
    pub parent: Option<String>,
}

impl Route {
    fn initial(bot_id: &str, depth: u8) -> Self {
        Self { trace: vec![bot_id.to_owned()], depth, parent: None }
    }

    pub(crate) fn to(&self, source: &str, recipient: &str) -> Result<Self> {
        if source == recipient {
            return Err("A bot cannot assign work to itself.".into());
        }
        if self.depth >= 3 {
            return Err("Bot collaboration is limited to three hops across direct messages and channels.".into());
        }
        if self.trace.iter().any(|id| id == recipient) {
            return Err("This bot was already visited by the current collaboration route.".into());
        }
        let mut trace = self.trace.clone();
        trace.push(recipient.to_owned());
        Ok(Self { trace, depth: self.depth + 1, parent: self.parent.clone() })
    }
}

fn parse_trace(text: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(text).unwrap_or_default()
}

/// Route ancestry is recovered from a currently executing delivery or a correlated callback.
/// A new, unrelated user turn starts a new route; a callback cannot silently reset the hop budget.
pub(crate) fn route(store: &Store, session_id: &str, bot_id: &str) -> Result<Route> {
    let legacy_depth = shared_channels::current_hop_depth(store, session_id)?;
    let direct: Option<(String, i64, String)> = store.db.query_row(
        "SELECT r.route,m.depth,m.id FROM extended_bot_direct_routes r
         JOIN bot_messages m ON m.id=r.message_id
         WHERE m.target_session_id=?1 AND m.status='in_progress'
         ORDER BY m.updated_at DESC,m.id DESC LIMIT 1",
        [session_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e| e.to_string())?;
    if let Some((trace, depth, parent)) = direct {
        return Ok(Route { trace: parse_trace(&trace), depth: legacy_depth.max(depth.clamp(0,3) as u8), parent: Some(parent) });
    }
    let channel: Option<(String, i64, String)> = store.db.query_row(
        "SELECT a.route,d.depth,a.message_id FROM bot_shared_channel_deliveries d
         JOIN extended_bot_channel_actions a ON a.message_id=d.message_id
         WHERE d.session_id=?1 AND d.status='in_progress'
         ORDER BY d.updated_at DESC,d.id DESC LIMIT 1",
        [session_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e| e.to_string())?;
    if let Some((trace, depth, parent)) = channel {
        let mut trace = parse_trace(&trace);
        if !trace.iter().any(|id| id == bot_id) { trace.push(bot_id.to_owned()); }
        return Ok(Route { trace, depth: legacy_depth.max(depth.clamp(0,3) as u8), parent: Some(parent) });
    }

    let latest_user: Option<String> = store.db.query_row(
        "SELECT content FROM messages WHERE session_id=?1 AND role='user' ORDER BY id DESC LIMIT 1",
        [session_id], |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if let Some(content) = latest_user {
        let mut statement = store.db.prepare(
            "SELECT r.route,m.depth,m.result,b.name,m.id FROM extended_bot_direct_routes r
             JOIN bot_messages m ON m.id=r.message_id
             JOIN bots b ON b.id=m.target_bot_id
             WHERE m.source_session_id=?1 AND m.result IS NOT NULL ORDER BY m.updated_at DESC",
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([session_id], |r| Ok((
            r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,
            r.get::<_,String>(3)?,r.get::<_,String>(4)?,
        ))).map_err(|e| e.to_string())?;
        for row in rows {
            let (trace, depth, result, name, parent) = row.map_err(|e| e.to_string())?;
            if content.starts_with(&format!("Update from {name}:\n\n{result}")) {
                return Ok(Route { trace: parse_trace(&trace), depth: legacy_depth.max(depth.clamp(0,3) as u8), parent: Some(parent) });
            }
        }
        let mut statement = store.db.prepare(
            "SELECT a.route,d.depth,d.bot_id,r.result,b.name,c.name,a.message_id
             FROM extended_bot_channel_results r
             JOIN bot_shared_channel_deliveries d ON d.id=r.delivery_id
             JOIN extended_bot_channel_actions a ON a.message_id=d.message_id
             JOIN bots b ON b.id=d.bot_id
             JOIN bot_shared_channels c ON c.id=d.channel_id
             WHERE a.origin_session_id=?1 AND r.state='sent' ORDER BY r.updated_at DESC",
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([session_id], |r| Ok((
            r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,
            r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,
            r.get::<_,String>(6)?,
        ))).map_err(|e| e.to_string())?;
        for row in rows {
            let (trace, depth, target, result, name, channel, parent) = row.map_err(|e| e.to_string())?;
            if content.starts_with(&callback_text(&name, &channel, &parent, &result)) {
                let mut trace = parse_trace(&trace);
                trace.push(target);
                return Ok(Route { trace, depth: legacy_depth.max(depth.clamp(0,3) as u8), parent: Some(parent) });
            }
        }
    }
    Ok(Route::initial(bot_id, legacy_depth))
}

/// Persist before forwarding the direct request, including a source-channel marker without
/// exposing the private request body in that channel.
pub(crate) fn record_direct_route(store: &Store, message_id: &str, source_session_id: &str, route: &Route) -> Result<()> {
    let source_delivery: Option<String> = store.db.query_row(
        "SELECT id FROM bot_shared_channel_deliveries
         WHERE session_id=?1 AND status='in_progress' ORDER BY updated_at DESC,id DESC LIMIT 1",
        [source_session_id], |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?;
    store.db.execute(
        "INSERT INTO extended_bot_direct_routes(message_id,source_delivery_id,parent_request_id,route)
         VALUES(?1,?2,?3,?4)",
        params![message_id, source_delivery, route.parent, json!(route.trace).to_string()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

fn resolve_channel(store: &Store, raw: &str) -> Result<shared_channels::SharedChannel> {
    let requested = raw.trim();
    let channels = shared_channels::list(store)?;
    let found = channels.iter().find(|channel| channel.id == requested);
    let channel = if let Some(found) = found {
        found.clone()
    } else {
        let name = requested.trim_start_matches('#');
        let matches: Vec<_> = channels.into_iter()
            .filter(|channel| channel.name.eq_ignore_ascii_case(name)).collect();
        if matches.len() != 1 {
            return Err(if matches.is_empty() {
                "Choose an existing local Shared Channel by its ID or exact name."
            } else {
                "Several channels share that name. Use the channel ID."
            }.into());
        }
        matches[0].clone()
    };
    if channel.archived { return Err("This channel is archived.".into()); }
    Ok(channel)
}

fn require_member(channel: &shared_channels::SharedChannel, bot_id: &str) -> Result<()> {
    if !channel.member_ids.iter().any(|id| id == bot_id) {
        return Err("The sending bot must belong to the selected active channel.".into());
    }
    Ok(())
}

fn clean_text(raw: &str) -> Result<String> {
    let text = raw.trim();
    if text.is_empty() || text.chars().count() > MAX_TEXT {
        return Err("Write a channel message between 1 and 16,384 characters.".into());
    }
    Ok(text.to_owned())
}

fn resolve_recipients(store: &Store, channel: &shared_channels::SharedChannel, sender: &Bot, requested: &[String], path: &Route) -> Result<Vec<String>> {
    if requested.is_empty() || requested.len() > 2 {
        return Err("Explicitly select one or two channel members.".into());
    }
    let mut recipients = Vec::new();
    let mut seen = HashSet::new();
    for raw in requested {
        let text = raw.trim();
        let members = channel.member_ids.iter()
            .map(|id| super::get(store, id))
            .collect::<Result<Vec<_>>>()?;
        let exact = members.iter().find(|bot| bot.id == text);
        let matches: Vec<_> = members.iter().filter(|bot| bot.name.eq_ignore_ascii_case(text)).collect();
        let target = if let Some(bot) = exact {
            bot
        } else if matches.len() == 1 {
            matches[0]
        } else {
            return Err(if matches.is_empty() {
                "Recipients must be members of the destination channel."
            } else {
                "Multiple channel members have that name. Use a bot ID."
            }.into());
        };
        if !seen.insert(target.id.clone()) {
            return Err("Choose one or two distinct recipients.".into());
        }
        path.to(&sender.id, &target.id)?;
        recipients.push(target.id.clone());
    }
    Ok(recipients)
}

fn identity(store: &Store, source: &Thread, bot: &Bot, target: (&str, &str),
    text: &str, recipients: &[String], explicit: Option<&str>) -> Result<String> {
    let (kind, channel_id) = target;
    if let Some(key) = explicit {
        if key.is_empty() || key.len() > 128 || key.chars().any(char::is_control) {
            return Err("The idempotency key must contain 1 to 128 printable characters.".into());
        }
        return Ok(format!("provided:{}:{}:{}", bot.id, source.session_id, key));
    }
    let turn: i64 = store.db.query_row(
        "SELECT COALESCE(MAX(id),0) FROM messages WHERE session_id=?1 AND role='user'",
        [&source.session_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    let body = json!([bot.id,source.session_id,turn,kind,channel_id,text,recipients]).to_string();
    let hash = Sha256::digest(body.as_bytes());
    Ok(format!("turn:{hash:x}"))
}

fn existing(store: &Store, key: &str, kind: &str, channel: &str, sender: &str,
    text: &str, recipients: &[String]) -> Result<Option<String>> {
    let found: Option<(String,String,String,String,String,String)> = store.db.query_row(
        "SELECT a.message_id,a.kind,m.channel_id,a.origin_bot_id,m.text,a.recipients
         FROM extended_bot_channel_actions a JOIN bot_shared_channel_messages m ON m.id=a.message_id
         WHERE a.idempotency_key=?1",
        [key], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)),
    ).optional().map_err(|e| e.to_string())?;
    match found {
        None => Ok(None),
        Some((id, saved_kind, saved_channel, saved_bot, saved_text, saved_recipients))
            if saved_kind == kind && saved_channel == channel && saved_bot == sender
                && saved_text == text && parse_trace(&saved_recipients) == recipients => Ok(Some(id)),
        _ => Err("This idempotency key was already used with different arguments.".into()),
    }
}

pub(crate) fn post(store: &Store, bot: &Bot, source: &Thread, target: &str,
    raw: &str, key: Option<&str>) -> Result<Value> {
    let channel = resolve_channel(store, target)?;
    require_member(&channel, &bot.id)?;
    let text = clean_text(raw)?;
    let route = route(store, &source.session_id, &bot.id)?;
    let key = identity(store, source, bot, ("post", &channel.id), &text, &[], key)?;
    if let Some(id) = existing(store, &key, "post", &channel.id, &bot.id, &text, &[])? {
        return Ok(json!({"postId":id,"channelId":channel.id,"status":"posted","replayed":true}));
    }
    let id = crate::storage::id();
    let now = crate::storage::now();
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,author_bot_id,text,status,created_at)
         VALUES(?1,?2,'reply',?3,?4,'completed',?5)",
        params![id, channel.id, bot.id, text, now],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO extended_bot_channel_actions(message_id,kind,origin_bot_id,origin_session_id,
         origin_channel_id,origin_external_id,recipients,parent_request_id,depth,route,idempotency_key,created_at)
         VALUES(?1,'post',?2,?3,?4,?5,'[]',?6,?7,?8,?9,?10)",
        params![id, bot.id, source.session_id, source.channel_id, source.external_id, route.parent,
            route.depth as i64, json!(route.trace).to_string(), key, now],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(json!({"postId":id,"channelId":channel.id,"status":"posted"}))
}

/// Bot assignments never use the channel lead as an implicit recipient.
pub(crate) async fn request(app: &tauri::AppHandle, bot: &Bot, source: &Thread,
    target: &str, names: &[String], raw: &str, key: Option<&str>) -> Result<Value> {
    let state = app.state::<AppState>();
    let (channel_id, message_id, recipients, depth, created, is_new) = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let channel = resolve_channel(&store, target)?;
        require_member(&channel, &bot.id)?;
        let text = clean_text(raw)?;
        let route = route(&store, &source.session_id, &bot.id)?;
        let recipients = resolve_recipients(&store, &channel, bot, names, &route)?;
        let key = identity(&store, source, bot, ("assignment", &channel.id), &text, &recipients, key)?;
        if let Some(id) = existing(&store, &key, "assignment", &channel.id, &bot.id, &text, &recipients)? {
            (channel.id, id, recipients, route.depth+1, crate::storage::now(), false)
        } else {
            let id = crate::storage::id();
            let now = crate::storage::now();
            let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,author_bot_id,text,status,created_at)
                 VALUES(?1,?2,'user',?3,?4,'queued',?5)",
                params![id, channel.id, bot.id, text, now],
            ).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO extended_bot_channel_actions(message_id,kind,origin_bot_id,origin_session_id,
                 origin_channel_id,origin_external_id,recipients,parent_request_id,depth,route,idempotency_key,created_at)
                 VALUES(?1,'assignment',?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![id, bot.id, source.session_id, source.channel_id, source.external_id,
                    json!(recipients).to_string(),route.parent,route.depth as i64,
                    json!(route.trace).to_string(),key,now],
            ).map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            (channel.id,id,recipients,route.depth+1,now,true)
        }
    };
    if is_new {
        for recipient in &recipients {
            let text: String = {
                let store = state.store.lock().map_err(|e| e.to_string())?;
                store.db.query_row("SELECT text FROM bot_shared_channel_messages WHERE id=?1",
                    [&message_id], |r| r.get(0)).map_err(|e| e.to_string())?
            };
            if let Err(error) = shared_channels::send_channel_assignment(
                app, &channel_id, &message_id, recipient, &text, created, depth,
            ).await {
                let failed_id = {
                    let store = state.store.lock().map_err(|e| e.to_string())?;
                    let id = record_failed_delivery(&store, &channel_id, &message_id, recipient, depth, created).ok();
                    let _ = shared_channels::update_message_status(&store, &message_id);
                    id
                };
                if let Some(id) = failed_id {
                    finish_delivery(app, &id, &format!("The assignment could not start: {error}")).await;
                }
            }
        }
    }
    let status = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let count: i64 = store.db.query_row(
            "SELECT COUNT(*) FROM bot_shared_channel_deliveries WHERE message_id=?1",
            [&message_id], |r| r.get(0),
        ).map_err(|e| e.to_string())?;
        // Another identical invocation may observe a persisted request before its first
        // delivery is enqueued. Do not report that accepted work was stopped.
        if count == 0 { "queued".to_owned() } else {
            shared_channels::update_message_status(&store, &message_id)?;
            shared_channels::message_status(&store, &message_id)?
        }
    };
    let _ = app.emit("bot-shared-channel-changed", json!({"channelId":channel_id}));
    Ok(json!({"requestId":message_id,"channelId":channel_id,
        "recipientIds":recipients,"status":status,"replayed":!is_new}))
}

fn record_failed_delivery(store: &Store, channel_id: &str, message_id: &str,
    recipient: &str, depth: u8, now: i64) -> Result<String> {
    // The gateway may already have persisted a failed delivery before returning an error.
    // Reuse it instead of creating a second result/callback for the same recipient.
    let prior: Option<String> = store.db.query_row(
        "SELECT id FROM bot_shared_channel_deliveries WHERE message_id=?1 AND bot_id=?2
         ORDER BY created_at DESC,id DESC LIMIT 1",
        params![message_id,recipient], |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if let Some(id) = prior { return Ok(id); }
    let target = super::get(store, recipient)?;
    let (session, _) = super::thread_session(store, &target, DESKTOP, DESKTOP, &format!("{} · You",target.name))?;
    let id = crate::storage::id();
    store.db.execute(
        "INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,depth,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,'failed',?6,?7,?7)",
        params![id,channel_id,message_id,recipient,session,depth as i64,now],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

fn callback_text(target: &str, channel: &str, request: &str, result: &str) -> String {
    format!("Update from {target} in #{channel} (request {request}):\n\n{result}")
}

/// Native admission is durable: a callback accepted for execution appears in the
/// destination bot's queued messages or its recorded user turn. Briefs may follow
/// the unique callback prefix; a previously admitted callback is never replayed.
fn callback_admitted(store: &Store, session_id: &str, text: &str) -> Result<bool> {
    store.db.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM queued_messages WHERE session_id=?1 AND substr(text,1,length(?2))=?2
           UNION ALL
           SELECT 1 FROM messages WHERE session_id=?1 AND role='user'
             AND substr(content,1,length(?2))=?2
         )",
        params![session_id, text], |row| row.get(0),
    ).map_err(|error| error.to_string())
}

/// A normal completion may claim one send. Only startup recovery turns an
/// interrupted 'sending' claim into 'failed' so it can be retried. The original
/// callback result is preserved, even when the Rust task finished before restart.
fn claim_callback(store: &Store, delivery_id: &str, reply: &str) -> Result<Option<CallbackInfo>> {
    let item: Option<(String,String,String,String,String,String,String,String,String)> = store.db.query_row(
        "SELECT a.origin_bot_id,a.origin_channel_id,a.origin_external_id,d.bot_id,
                b.name,c.name,a.message_id,d.status,a.origin_session_id
         FROM extended_bot_channel_actions a
         JOIN bot_shared_channel_deliveries d ON d.message_id=a.message_id
         JOIN bots b ON b.id=d.bot_id
         JOIN bot_shared_channels c ON c.id=d.channel_id
         WHERE d.id=?1 AND a.kind='assignment'",
        [delivery_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
            r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?)),
    ).optional().map_err(|error| error.to_string())?;
    let Some((origin, source_channel, external, _, name, channel, message_id, status, session_id)) = item else {
        return Ok(None);
    };
    if !matches!(status.as_str(), "completed" | "failed" | "stopped") { return Ok(None); }
    let fresh = match status.as_str() {
        "completed" if !reply.trim().is_empty() => reply.trim().to_owned(),
        "completed" => "The assigned bot finished without a text reply.".to_owned(),
        "stopped" => "The assignment was stopped.".to_owned(),
        _ if !reply.trim().is_empty() => reply.trim().to_owned(),
        _ => "The assignment failed or was interrupted.".to_owned(),
    };
    let saved: Option<(String,String)> = store.db.query_row(
        "SELECT result,state FROM extended_bot_channel_results WHERE delivery_id=?1",
        [delivery_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|error| error.to_string())?;
    let (result, prior) = saved.unwrap_or((fresh, String::new()));
    if prior == "sent" { return Ok(None); }
    let text = callback_text(&name, &channel, &message_id, &result);
    if !prior.is_empty() && callback_admitted(store, &session_id, &text)? {
        store.db.execute(
            "UPDATE extended_bot_channel_results SET state='sent',updated_at=?2 WHERE delivery_id=?1",
            params![delivery_id,crate::storage::now()],
        ).map_err(|error| error.to_string())?;
        return Ok(None);
    }
    let claimed = if prior.is_empty() {
        store.db.execute(
            "INSERT OR IGNORE INTO extended_bot_channel_results(delivery_id,result,state,updated_at)
             VALUES(?1,?2,'sending',?3)",
            params![delivery_id,result,crate::storage::now()],
        ).map_err(|error| error.to_string())?
    } else if prior == "failed" {
        store.db.execute(
            "UPDATE extended_bot_channel_results SET state='sending',updated_at=?2
             WHERE delivery_id=?1 AND state='failed'",
            params![delivery_id,crate::storage::now()],
        ).map_err(|error| error.to_string())?
    } else { 0 };
    if claimed == 0 { return Ok(None); }
    Ok(Some((origin,source_channel,external,message_id,text,delivery_id.to_owned())))
}

/// Resume only callbacks not already admitted by the native bot gateway.
/// A failed gateway admission is recoverable after restart; completed work is
/// never re-executed simply because its callback confirmation was interrupted.
pub(crate) async fn finish_delivery(app: &tauri::AppHandle, delivery_id: &str, reply: &str) {
    let state = app.state::<AppState>();
    let pending = state.store.lock().map_err(|error| error.to_string())
        .and_then(|store| claim_callback(&store, delivery_id, reply));
    let Ok(Some((origin,source_channel,external,message_id,text,id))) = pending else { return };
    let delivered = gateway::deliver_with_queue(app, gateway::Inbound {
        bot_id: origin.clone(), channel_id: source_channel, thread: external,
        sender_id: format!("bot-channel-update:{message_id}:{id}"),
        sender_name: "Bot channel update".into(), text,
        trusted: true, image_ids: None, moodboard_ids: None,
    }, true).await;
    if let Ok(store) = state.store.lock() {
        let next = if delivered.is_ok() { "sent" } else { "failed" };
        let _ = store.db.execute(
            "UPDATE extended_bot_channel_results SET state=?2,updated_at=?3 WHERE delivery_id=?1",
            params![id,next,crate::storage::now()],
        );
    }
    if let Err(error) = delivered {
        super::note(app, &origin, "error", &format!("Channel completion callback failed: {error}"),
            json!({"requestId":message_id,"deliveryId":id}));
    }
    let _ = app.emit("bot-messages-changed", ());
}

/// A process can stop between committing a request and creating its first delivery.
/// Record missing recipients as failed, then let normal callback recovery report them.
/// Never auto-run work after an uncertain crash, or duplicate an existing recipient.
pub(crate) fn reconcile_unsent_requests(store: &Store) -> Result<()> {
    let requests: Vec<(String,String,String,i64,i64)> = {
        let mut statement = store.db.prepare(
            "SELECT a.message_id,m.channel_id,a.recipients,a.depth,m.created_at
             FROM extended_bot_channel_actions a
             JOIN bot_shared_channel_messages m ON m.id=a.message_id
             WHERE a.kind='assignment' AND EXISTS (
               SELECT 1 FROM json_each(a.recipients) r
               WHERE NOT EXISTS (
                 SELECT 1 FROM bot_shared_channel_deliveries d
                 WHERE d.message_id=a.message_id AND d.bot_id=r.value
               )
             )",
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?
    };
    for (message_id,channel_id,recipients,depth,created) in requests {
        let mut lost_recipient = false;
        for recipient in parse_trace(&recipients) {
            if let Err(error) = record_failed_delivery(store, &channel_id, &message_id,
                &recipient, (depth + 1).clamp(1,3) as u8, created) {
                lost_recipient = true;
                eprintln!("Could not recover a missing channel recipient for {message_id}: {error}");
            }
        }
        if lost_recipient {
            store.db.execute("UPDATE bot_shared_channel_messages SET status='failed' WHERE id=?1",
                [&message_id]).map_err(|e| e.to_string())?;
        } else {
            shared_channels::update_message_status(store, &message_id)?;
        }
    }
    Ok(())
}

/// Startup alone reclaims interrupted sends. The admission check above distinguishes
/// a callback already queued/launched from one lost before gateway admission.
pub(crate) async fn resume_callbacks(app: &tauri::AppHandle) {
    let ids: Vec<String> = app.state::<AppState>().store.lock().ok().and_then(|store| {
        store.db.execute(
            "UPDATE extended_bot_channel_results SET state='failed',updated_at=?1 WHERE state='sending'",
            [crate::storage::now()],
        ).ok()?;
        let mut query = store.db.prepare(
            "SELECT d.id FROM bot_shared_channel_deliveries d
             JOIN extended_bot_channel_actions a ON a.message_id=d.message_id
             LEFT JOIN extended_bot_channel_results r ON r.delivery_id=d.id
             WHERE a.kind='assignment' AND d.status IN ('completed','failed','stopped')
               AND (r.delivery_id IS NULL OR r.state='failed')
             ORDER BY d.created_at,d.id",
        ).ok()?;
        let rows = query.query_map([], |r| r.get::<_,String>(0)).ok()?;
        rows.collect::<rusqlite::Result<Vec<_>>>().ok()
    }).unwrap_or_default();
    for id in ids { finish_delivery(app, &id, "").await; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{create, BotInput};
    struct Fixture { store: Store, root: std::path::PathBuf }
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.root); } }
    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!("pandacode-collaboration-{}", crate::storage::id()));
        std::fs::create_dir_all(&root).unwrap();
        Fixture { store: Store::open(std::path::Path::new(":memory:")).unwrap(), root }
    }
    fn bot(f: &Fixture, name: &str) -> Bot {
        create(&f.store, &BotInput { name:name.into(), instructions:"Stay focused.".into(),
            provider:"claude".into(),model:"default".into(),mode:"ask".into(),..Default::default() }, &f.root).unwrap()
    }

    #[test]
    fn posting_is_terminal_and_never_creates_deliveries_even_with_mentions() {
        let f=fixture();
        let author=bot(&f,"Author");
        let reviewer=bot(&f,"Reviewer");
        let launch=shared_channels::create(&f.store,"launch",&[author.id.clone(),reviewer.id.clone()]).unwrap();
        let other=shared_channels::create(&f.store,"other",std::slice::from_ref(&author.id)).unwrap();
        let (_,session)=("unused",super::super::thread_session(&f.store,&author,DESKTOP,DESKTOP,"Author").unwrap().0);
        let thread=super::super::owner_of(&f.store,&session).unwrap().unwrap().1;
        let sent=post(&f.store,&author,&thread,&launch.id,"@Reviewer Build is ready.",Some("milestone")).unwrap();
        let replay=post(&f.store,&author,&thread,"launch","@Reviewer Build is ready.",Some("milestone")).unwrap();
        assert_eq!(sent["postId"],replay["postId"]);
        assert_eq!(replay["replayed"],true);
        let count:i64=f.store.db.query_row("SELECT count(*) FROM bot_shared_channel_deliveries",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0);
        assert_eq!(shared_channels::detail(&f.store,&other.id,100).unwrap().messages.len(),0);
        let displayed=shared_channels::detail(&f.store,&launch.id,100).unwrap();
        assert_eq!(displayed.messages.len(),1);
        assert_eq!(displayed.messages[0].author_bot_id.as_deref(),Some(author.id.as_str()));
        assert!(post(&f.store,&reviewer,&thread,&other.id,"Private",None).is_err());
    }

    #[test]
    fn explicit_recipients_and_shared_depth_reject_self_ambiguity_and_cycles() {
        let f=fixture();
        let author=bot(&f,"Author");
        let reviewer=bot(&f,"Reviewer");
        let duplicate=bot(&f,"REVIEWER");
        let channel=shared_channels::create(&f.store,"quality",
            &[author.id.clone(),reviewer.id.clone(),duplicate.id.clone()]).unwrap();
        let route=Route::initial(&author.id,0);
        assert!(resolve_recipients(&f.store,&channel,&author,&[],&route).is_err());
        assert!(resolve_recipients(&f.store,&channel,&author,std::slice::from_ref(&author.id),&route).is_err());
        assert!(resolve_recipients(&f.store,&channel,&author,&["reviewer".into()],&route).is_err());
        assert!(resolve_recipients(&f.store,&channel,&author,&[reviewer.id.clone(),reviewer.id.clone()],&route).is_err());
        assert_eq!(resolve_recipients(&f.store,&channel,&author,std::slice::from_ref(&reviewer.id),&route).unwrap(),vec![reviewer.id.clone()]);
        let one=route.to(&author.id,&reviewer.id).unwrap();
        assert!(one.to(&reviewer.id,&author.id).is_err());
        let two=one.to(&reviewer.id,&duplicate.id).unwrap();
        let three=two.to(&duplicate.id,"fourth").unwrap();
        assert!(three.to("fourth","fifth").is_err());
    }

    #[test]
    fn failed_dispatch_reuses_the_existing_delivery_and_startup_records_missing_work() {
        let f = fixture();
        let author = bot(&f, "Author");
        let reviewer = bot(&f, "Reviewer");
        let channel = shared_channels::create(&f.store, "launch", &[author.id.clone(), reviewer.id.clone()]).unwrap();
        let (session, _) = super::super::thread_session(&f.store, &author, DESKTOP, DESKTOP, "Author").unwrap();
        let id = crate::storage::id();
        let now = crate::storage::now();
        f.store.db.execute(
            "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,author_bot_id,text,status,created_at)
             VALUES(?1,?2,'user',?3,'Review this','queued',?4)",
            params![id,channel.id,author.id,now],
        ).unwrap();
        f.store.db.execute(
            "INSERT INTO extended_bot_channel_actions(message_id,kind,origin_bot_id,origin_session_id,origin_channel_id,
             origin_external_id,recipients,depth,route,idempotency_key,created_at)
             VALUES(?1,'assignment',?2,?3,'desktop','desktop',?4,0,?5,'test-recovery',?6)",
            params![id,author.id,session,json!([reviewer.id]).to_string(),json!([author.id]).to_string(),now],
        ).unwrap();
        reconcile_unsent_requests(&f.store).unwrap();
        let first = record_failed_delivery(&f.store, &channel.id, &id, &reviewer.id, 1, now).unwrap();
        let again = record_failed_delivery(&f.store, &channel.id, &id, &reviewer.id, 1, now).unwrap();
        assert_eq!(first, again);
        reconcile_unsent_requests(&f.store).unwrap();
        let count: i64 = f.store.db.query_row(
            "SELECT COUNT(*) FROM bot_shared_channel_deliveries WHERE message_id=?1",
            [&id], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
        assert_eq!(shared_channels::message_status(&f.store, &id).unwrap(), "failed");
    }

    #[test]
    fn interrupted_callbacks_recover_only_when_gateway_did_not_admit_them() {
        let f=fixture();
        let author=bot(&f,"Author");
        let reviewer=bot(&f,"Reviewer");
        let channel=shared_channels::create(&f.store,"quality",
            &[author.id.clone(),reviewer.id.clone()]).unwrap();
        let (session,_)=super::super::thread_session(&f.store,&author,DESKTOP,DESKTOP,"Author").unwrap();
        let request_id=crate::storage::id();
        let delivery_id=crate::storage::id();
        let now=crate::storage::now();
        f.store.db.execute(
            "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,author_bot_id,text,status,created_at)
             VALUES(?1,?2,'user',?3,'Review','completed',?4)",
            params![request_id,channel.id,author.id,now],
        ).unwrap();
        f.store.db.execute(
            "INSERT INTO extended_bot_channel_actions(message_id,kind,origin_bot_id,origin_session_id,
             origin_channel_id,origin_external_id,recipients,depth,route,idempotency_key,created_at)
             VALUES(?1,'assignment',?2,?3,'desktop','desktop',?4,0,?5,'callback-test',?6)",
            params![request_id,author.id,session,json!([reviewer.id]).to_string(),
                json!([author.id]).to_string(),now],
        ).unwrap();
        f.store.db.execute(
            "INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,depth,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,'completed',1,?6,?6)",
            params![delivery_id,channel.id,request_id,reviewer.id,session,now],
        ).unwrap();
        let first=claim_callback(&f.store,&delivery_id,"Review completed").unwrap().unwrap();
        assert!(claim_callback(&f.store,&delivery_id,"Review completed").unwrap().is_none(),
            "two overlapping callbacks must not execute");
        f.store.db.execute(
            "UPDATE extended_bot_channel_results SET state='failed' WHERE delivery_id=?1",
            [&delivery_id],
        ).unwrap();
        let retry=claim_callback(&f.store,&delivery_id,"").unwrap().unwrap();
        assert_eq!(first.4,retry.4,"restart retains the original result");
        f.store.db.execute(
            "UPDATE extended_bot_channel_results SET state='failed' WHERE delivery_id=?1",
            [&delivery_id],
        ).unwrap();
        crate::message_queue::add(&f.store,&session,&retry.4,&json!([])).unwrap();
        assert!(claim_callback(&f.store,&delivery_id,"").unwrap().is_none(),
            "a queued callback must not execute a second time");
        let state:String=f.store.db.query_row(
            "SELECT state FROM extended_bot_channel_results WHERE delivery_id=?1",
            [&delivery_id],|row|row.get(0),
        ).unwrap();
        assert_eq!(state,"sent");
        assert_eq!(crate::message_queue::list(&f.store,Some(&session)).unwrap().len(),1);
    }

    #[test]
    fn idempotency_keys_are_scoped_to_source_and_payload() {
        let f=fixture();
        let author=bot(&f,"Author");
        let channel=shared_channels::create(&f.store,"release",std::slice::from_ref(&author.id)).unwrap();
        let (session,_)=super::super::thread_session(&f.store,&author,DESKTOP,DESKTOP,"Author").unwrap();
        let thread=super::super::owner_of(&f.store,&session).unwrap().unwrap().1;
        let saved=post(&f.store,&author,&thread,&channel.id,"One update",Some("run-123")).unwrap();
        assert!(post(&f.store,&author,&thread,&channel.id,"Different update",Some("run-123")).is_err());
        assert_eq!(post(&f.store,&author,&thread,&channel.id,"One update",Some("run-123")).unwrap()["postId"],saved["postId"]);
        assert_eq!(shared_channels::detail(&f.store,&channel.id,50).unwrap().messages.len(),1);
    }
}
