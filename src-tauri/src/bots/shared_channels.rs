//! Local group conversations shared by bots. Channel records stay in the bot database;
//! assignments use each member's ordinary desktop task and the existing gateway queue.

use super::{collaboration, gateway, Bot, DESKTOP};
use crate::{message_queue, storage::Store, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

const MAX_NAME: usize = 48;
const MAX_TEXT: usize = 16 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedChannel {
    pub id: String,
    pub name: String,
    pub member_ids: Vec<String>,
    pub lead_bot_id: Option<String>,
    pub archived: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedChannelMessage {
    pub id: String,
    pub kind: String,
    pub author_bot_id: Option<String>,
    pub text: String,
    pub status: Option<String>,
    pub created_at: i64,
    pub recipient_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub council: Option<ChannelCouncilView>,
}

/// A channel-visible, attributed verdict; private reviewer transcripts and
/// credentials are deliberately excluded from this value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelCouncilView {
    pub cycle_id: String,
    /// Bot-owned task for the scoped native Council report.
    pub session_id: String,
    pub state: String,
    pub round: usize,
    pub target: f64,
    pub lowest: Option<f64>,
    pub artifact_revision: String,
    pub notice: Option<String>,
    pub reviewers: Vec<ChannelCouncilReviewer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followup_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followup_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_by: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelCouncilReviewer {
    pub bot_id: Option<String>,
    pub name: String,
    pub lens: String,
    pub round: usize,
    pub score: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedChannelDetail {
    #[serde(flatten)]
    pub channel: SharedChannel,
    pub messages: Vec<SharedChannelMessage>,
    pub active_bot_ids: Vec<String>,
    /// Older rows exist beyond the `limit` returned.
    pub has_more: bool,
}

/// Channel history rows returned per page; the window asks for more as the user scrolls back.
pub(crate) const HISTORY_PAGE: usize = 100;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotMessageMarker {
    pub id: String,
    /// `sent`, `received`, or `update`.
    pub kind: String,
    pub bot_id: String,
    pub bot_name: String,
    pub avatar: String,
    pub color: String,
    pub text: String,
    pub result: Option<String>,
    pub status: String,
    pub created_at: i64,
}

fn clean_name(value: &str) -> Result<String> {
    let name = value.trim().trim_start_matches('#').trim();
    if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err("Use a channel name up to 48 characters.".into());
    }
    Ok(name.to_owned())
}

fn validate_members(store: &Store, members: &[String]) -> Result<Vec<String>> {
    if members.len() > 64 { return Err("A channel can have up to 64 bots.".into()); }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for id in members {
        if !seen.insert(id.as_str()) { continue; }
        super::get(store, id)?;
        out.push(id.clone());
    }
    Ok(out)
}

/// Deterministic member routing. Explicit mentions win; otherwise the lead receives the post.
pub(crate) fn route_recipients(members: &[String], lead: Option<&str>, requested: &[String]) -> Result<Vec<String>> {
    let chosen = if requested.is_empty() {
        vec![lead.ok_or("Add a bot in channel settings before posting.")?.to_owned()]
    } else {
        requested.to_vec()
    };
    if chosen.len() > 2 { return Err("Mention up to two bots in one post.".into()); }
    let mut seen = HashSet::new();
    let mut recipients = Vec::new();
    for id in chosen {
        if !members.contains(&id) { return Err("Choose bots that belong to this channel.".into()); }
        if seen.insert(id.clone()) { recipients.push(id); }
    }
    if recipients.is_empty() { return Err("Choose a channel member to assign this post.".into()); }
    Ok(recipients)
}

fn member_ids(store: &Store, channel_id: &str) -> Result<Vec<String>> {
    let mut statement = store.db.prepare("SELECT bot_id FROM bot_shared_channel_members WHERE channel_id=?1 ORDER BY position,bot_id").map_err(|e| e.to_string())?;
    let rows = statement.query_map([channel_id], |row| row.get(0)).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
}

fn shared_channel(store: &Store, id: &str) -> Result<SharedChannel> {
    let row: (String, String, Option<String>, bool, i64) = store.db.query_row(
        "SELECT id,name,lead_bot_id,archived,updated_at FROM bot_shared_channels WHERE id=?1", [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ).optional().map_err(|e| e.to_string())?.ok_or("This channel no longer exists.")?;
    Ok(SharedChannel { id: row.0.clone(), name: row.1, lead_bot_id: row.2, archived: row.3, updated_at: row.4, member_ids: member_ids(store, &row.0)? })
}

pub(crate) fn list(store: &Store) -> Result<Vec<SharedChannel>> {
    let ids = {
        let mut statement = store.db.prepare("SELECT id FROM bot_shared_channels ORDER BY archived,name COLLATE NOCASE").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?
    };
    ids.iter().map(|id| shared_channel(store, id)).collect()
}

pub(crate) fn create(store: &Store, raw_name: &str, requested: &[String]) -> Result<SharedChannel> {
    let name = clean_name(raw_name)?;
    let members = validate_members(store, requested)?;
    let id = crate::storage::id();
    let now = crate::storage::now();
    let lead = members.first().cloned();
    store.db.execute("INSERT INTO bot_shared_channels(id,name,lead_bot_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)", params![id,name,lead,now]).map_err(|e| e.to_string())?;
    for (position, bot_id) in members.iter().enumerate() {
        store.db.execute("INSERT INTO bot_shared_channel_members(channel_id,bot_id,position) VALUES(?1,?2,?3)", params![id,bot_id,position as i64]).map_err(|e| e.to_string())?;
    }
    shared_channel(store, &id)
}

pub(crate) fn update(store: &Store, id: &str, raw_name: &str, requested: &[String], requested_lead: Option<&str>) -> Result<SharedChannel> {
    if shared_channel(store, id)?.archived { return Err("This channel is deleted. Restore it to change it.".into()); }
    let name = clean_name(raw_name)?;
    let members = validate_members(store, requested)?;
    if requested_lead.is_some_and(|lead| !members.iter().any(|id| id == lead)) {
        return Err("Choose a lead from the channel members.".into());
    }
    let lead = requested_lead.map(str::to_owned).or_else(|| members.first().cloned());
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("UPDATE bot_shared_channels SET name=?2,lead_bot_id=?3,updated_at=?4 WHERE id=?1", params![id,name,lead,crate::storage::now()]).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM bot_shared_channel_members WHERE channel_id=?1", [id]).map_err(|e| e.to_string())?;
    for (position, bot_id) in members.iter().enumerate() {
        tx.execute("INSERT INTO bot_shared_channel_members(channel_id,bot_id,position) VALUES(?1,?2,?3)", params![id,bot_id,position as i64]).map_err(|e| e.to_string())?;
    }
    // Channel membership edits replace the join rows; preserve only continuing members' choices.
    tx.execute(
        "DELETE FROM extended_bot_channel_history_overrides
         WHERE channel_id=?1 AND bot_id NOT IN
         (SELECT bot_id FROM bot_shared_channel_members WHERE channel_id=?1)",
        [id],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    shared_channel(store, id)
}

pub(crate) fn set_archived(store: &Store, id: &str, archived: bool) -> Result<()> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let changed = tx.execute("UPDATE bot_shared_channels SET archived=?2,updated_at=?3 WHERE id=?1", params![id,archived,crate::storage::now()]).map_err(|e| e.to_string())?;
    if changed == 0 { return Err("This channel no longer exists.".into()); }
    if archived {
        tx.execute("DELETE FROM extended_bot_channel_history_overrides WHERE channel_id=?1", [id])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Clone)]
pub(super) struct ChannelCouncilRequest {
    pub(super) council: crate::architect::CouncilConfig,
    pub(super) executors: Vec<String>,
}

/// Council-off shares the existing desktop conversation, preserving its
/// routing, queue and history. Council-on uses a unique context per delivery.
pub(super) fn channel_delivery_thread(delivery_id: &str, enabled: bool) -> String {
    if enabled { format!("channel-council:{delivery_id}") } else { DESKTOP.to_owned() }
}

pub(super) fn council_request(store: &Store, message_id: &str) -> Result<Option<ChannelCouncilRequest>> {
    let row: Option<(String, String)> = store.db.query_row(
        "SELECT council_json,executor_ids FROM extended_channel_council_requests WHERE message_id=?1",
        [message_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    row.map(|(config, executors)| {
        Ok(ChannelCouncilRequest {
            council: serde_json::from_str(&config).map_err(|_| "channel_council_invalid: saved Council config is invalid.".to_string())?,
            executors: serde_json::from_str(&executors).map_err(|_| "channel_council_invalid: saved executor roster is invalid.".to_string())?,
        })
    }).transpose()
}

/// Bind the native Council's first cycle to the isolated channel delivery.
/// A later cycle in that session may never authorize the original assignment.
pub(crate) fn bind_delivery_cycle(store: &Store, session_id: &str, since: i64) -> Result<()> {
    let linked: Option<(String,String)> = store.db.query_row(
        "SELECT c.delivery_id,c.artifact_revision FROM extended_channel_council_deliveries c
         JOIN bot_shared_channel_deliveries d ON d.id=c.delivery_id
         WHERE d.session_id=?1 AND d.status IN ('queued','in_progress')",
        [session_id], |r| Ok((r.get(0)?,r.get(1)?)),
    ).optional().map_err(|e|e.to_string())?;
    let Some((id,existing)) = linked else { return Ok(()) };
    let frozen = format!("cycle:{since}");
    if existing == frozen { return Ok(()); }
    if existing != "0" {
        return Err("channel_council_scope_mismatch: a later Council cycle cannot approve an earlier delivery.".into());
    }
    let updated = store.db.execute(
        "UPDATE extended_channel_council_deliveries SET artifact_revision=?2
         WHERE delivery_id=?1 AND artifact_revision='0'",
        params![id,frozen],
    ).map_err(|e|e.to_string())?;
    if updated != 1 { return Err("channel_council_scope_mismatch: could not pin the original Council cycle.".into()); }
    Ok(())
}

/** Recheck frozen per-message authority against current channel members and
 * all message executors on every selected seat, including resumed rounds. */
pub(crate) fn validate_delivery_reviewers(store: &Store, session_id: &str, selected: &[String]) -> Result<()> {
    let thread: Option<String> = store.db.query_row(
        "SELECT external_id FROM bot_threads WHERE session_id=?1 AND external_id LIKE 'channel-council:%'",
        [session_id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let Some(thread) = thread else { return Ok(()) }; // Native/direct Council is unchanged.
    let delivery: Option<(String, String, String, String, String, String)> = store.db.query_row(
        "SELECT c.delivery_id,c.channel_id,c.source_message_id,c.executor_bot_id,d.bot_id,d.status
         FROM extended_channel_council_deliveries c
         JOIN bot_shared_channel_deliveries d ON d.id=c.delivery_id
         WHERE d.session_id=?1",
        [session_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((delivery_id, channel_id, message_id, executor, assigned_bot, status)) = delivery else {
        return Err("channel_council_scope_missing: isolated reviewer delivery has no durable authorization.".into());
    };
    if thread != format!("channel-council:{delivery_id}") || executor != assigned_bot {
        return Err("channel_council_scope_mismatch: reviewer delivery identity changed.".into());
    }
    if !matches!(status.as_str(), "queued" | "in_progress") {
        return Err("channel_council_unavailable: delivery is no longer active.".into());
    }
    let channel = shared_channel(store, &channel_id)?;
    if channel.archived || !channel.member_ids.contains(&executor) {
        return Err("reviewer_unavailable: originating channel or executor is no longer eligible.".into());
    }
    let request = council_request(store, &message_id)?
        .ok_or("channel_council_scope_missing: original Council opt-in is absent.")?;
    if request.council.reviewer_bot_ids.as_slice() != selected {
        return Err("channel_council_scope_mismatch: original message reviewer roster changed.".into());
    }
    let mut executors = request.executors;
    let mut stmt = store.db.prepare("SELECT bot_id FROM bot_shared_channel_deliveries WHERE message_id=?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([&message_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    executors.extend(rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?);
    for id in selected {
        if !channel.member_ids.contains(id) {
            return Err("reviewer_unavailable: selected reviewer is no longer a channel member.".into());
        }
        if executors.contains(id) {
            return Err("reviewer_unavailable: a message executor cannot review its channel delivery.".into());
        }
    }
    Ok(())
}

fn council_view(store: &Store, delivery_id: &str, session_id: &str, delivery_status: &str) -> Result<Option<ChannelCouncilView>> {
    let cycle: Option<(String, String, String)> = store.db.query_row(
        "SELECT cycle_id,artifact_revision,source_message_id FROM extended_channel_council_deliveries WHERE delivery_id=?1",
        [delivery_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((cycle_id, original_revision, source_message_id)) = cycle else { return Ok(None) };
    let original = council_request(store, &source_message_id)?.map(|request| request.council);
    let team = crate::architect::load(store, session_id)?;
    let proof: Option<(i64, i64, i64, String)> = store.db.query_row(
        "SELECT council_since,dispatch_revision,changed_at,reason
         FROM extended_channel_council_acceptances
         WHERE delivery_id=?1 AND cycle_id=?2 AND actor='local_user'",
        params![delivery_id,cycle_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
    ).optional().map_err(|e| e.to_string())?;
    let (state, round, target, lowest, revision, notice, reviewers, acceptance_reason) = if let Some(team) = team {
        let status = team.council_status.as_ref();
        let original_matches = original.as_ref() == team.config.council.as_ref()
            && original_revision == format!("cycle:{}",team.council_since);
        let authorized_exception = original_matches && proof.as_ref().is_some_and(|(since,dispatch,changed,_)| {
            team.council_accepted == Some(*since) && team.council_since == *since
                && team.dispatch_revision as i64 == *dispatch && team.changed_at == *changed
                && status.is_some_and(|s| s.state == "accepted" && !s.stale)
        });
        let state = match team.status.as_str() {
            "stopped" => "Stopped",
            "failed" => "Failed",
            _ => match status.map(|s| s.state.as_str()) {
                Some("passed") if original_matches => "Passed",
                Some("passed") => "Inconclusive",
                Some("accepted") if authorized_exception => "Accepted by user",
                Some("accepted") => "Inconclusive",
                Some("limit") if original_matches => "Limit reached",
                Some("limit") => "Inconclusive",
                Some("fixing" | "asking") => "Needs correction",
                Some("waiting" | "reviewing" | "dispatch") => "Reviewing",
                _ => "Inconclusive",
            },
        };
        let reviewers = team.workers.iter().filter_map(|worker| {
            worker.council.as_ref().map(|seat| ChannelCouncilReviewer {
                bot_id: worker.reviewer_binding.as_ref().and_then(|binding| binding.reviewer_bot_id.clone()),
                name: worker.name.clone(), lens: seat.lens.clone(),
                round: seat.round, score: seat.score,
            })
        }).collect();
        let note = (!original_matches)
            .then(|| "Council settings or cycle differ from this channel delivery; approval is unavailable.".to_string())
            .or_else(|| status.and_then(|s| s.notice.as_ref()).map(|s| s.chars().take(300).collect()))
            .or_else(|| (status.is_some_and(|s| s.state == "accepted") && !authorized_exception)
                .then(|| "Council acceptance requires a reasoned user action in this channel.".into()));
        (state.to_owned(), status.map_or(0, |s| s.round),
            status.map_or(9.0, |s| s.target), status.and_then(|s| s.lowest),
            format!("{}:{}", team.council_since, team.dispatch_revision),
            note, reviewers,
            if authorized_exception { proof.as_ref().map(|(_,_,_,reason)| reason.clone()) } else { None })
    } else {
        ("Inconclusive".into(), 0, 9.0, None, original_revision, None, Vec::new(), None)
    };
    // The native delivery is authoritative for Stop and failure even when an
    // interrupted Architect snapshot still says a reviewer is working.
    let state = match delivery_status {
        "stopped" => "Stopped".into(),
        "failed" if state != "Limit reached" => "Failed".into(),
        _ => state,
    };
    let followup: Option<(String, Option<String>)> = store.db.query_row(
        "SELECT state,issue_url FROM extended_channel_council_followups WHERE delivery_id=?1",
        [delivery_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(|e| e.to_string())?;
    let (followup_status, followup_url) = followup.map_or((None,None), |(s,u)| (Some(s),u));
    let notice = if delivery_status == "failed" && notice.is_none() {
        Some("Council delivery ended without approval. Review the report before retrying.".into())
    } else { notice };
    let accepted_by = acceptance_reason.as_ref().map(|_| "local_user".to_string());
    Ok(Some(ChannelCouncilView { cycle_id, session_id: session_id.to_owned(), state, round, target, lowest, artifact_revision: revision, notice, reviewers, followup_status, followup_url, acceptance_reason, accepted_by }))
}

/// The channel and its latest `limit` history rows (posts, replies and assignments), oldest first.
pub(crate) fn detail(store: &Store, id: &str, limit: usize) -> Result<SharedChannelDetail> {
    let limit = limit.clamp(1, 10_000);
    let channel = shared_channel(store, id)?;
    let mut messages = Vec::new();
    {
        let mut statement = store.db.prepare("SELECT m.id,CASE a.kind WHEN 'post' THEN 'bot_post' WHEN 'assignment' THEN 'bot_request' ELSE m.kind END,m.author_bot_id,m.text,m.status,m.created_at,COALESCE(a.recipients,'[]') FROM bot_shared_channel_messages m LEFT JOIN extended_bot_channel_actions a ON a.message_id=m.id WHERE m.channel_id=?1 ORDER BY m.created_at DESC,m.id DESC LIMIT ?2").map_err(|e| e.to_string())?;
        let rows = statement.query_map(params![id,limit as i64+1], |r| Ok(SharedChannelMessage { id:r.get(0)?, kind:r.get(1)?, author_bot_id:r.get(2)?, text:r.get(3)?, status:r.get::<_,Option<String>>(4)?.map(status_label), created_at:r.get(5)?, recipient_ids:serde_json::from_str(&r.get::<_,String>(6)?).unwrap_or_default(), council:None })).map_err(|e| e.to_string())?;
        messages.extend(rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?);
    }
    {
        let mut statement = store.db.prepare("SELECT d.id,d.bot_id,b.name,d.status,d.created_at,d.session_id FROM bot_shared_channel_deliveries d JOIN bots b ON b.id=d.bot_id WHERE d.channel_id=?1 ORDER BY d.created_at DESC,d.id DESC LIMIT ?2").map_err(|e| e.to_string())?;
        let rows = statement.query_map(params![id,limit as i64+1], |r| {
            let bot_id: String = r.get(1)?; let bot_name: String = r.get(2)?;
            let delivery_id: String = r.get(0)?;
            Ok((delivery_id, bot_id, bot_name, r.get::<_,String>(3)?, r.get::<_,i64>(4)?, r.get::<_,String>(5)?))
        }).map_err(|e| e.to_string())?;
        for row in rows {
            let (delivery_id, bot_id, bot_name, status, created_at, session_id) = row.map_err(|e| e.to_string())?;
            messages.push(SharedChannelMessage {
                council: council_view(store, &delivery_id, &session_id, &status)?,
                id: delivery_id, kind: "assigned".into(), author_bot_id: Some(bot_id),
                text: format!("Assigned to {bot_name}"), status: Some(status_label(status)),
                created_at, recipient_ids: Vec::new(),
            });
        }
    }
    // Direct delegations started by a channel assignee appear as compact status rows only;
    // the private bot-to-bot request body and response never enter the channel transcript.
    {
        let mut statement = store.db.prepare("SELECT m.id,m.source_bot_id,b.name,m.status,m.created_at,m.target_bot_id FROM bot_messages m JOIN extended_bot_direct_routes r ON r.message_id=m.id JOIN bot_shared_channel_deliveries d ON d.id=r.source_delivery_id JOIN bots b ON b.id=m.target_bot_id WHERE d.channel_id=?1 ORDER BY m.created_at DESC,m.id DESC LIMIT ?2").map_err(|e| e.to_string())?;
        let rows = statement.query_map(params![id,limit as i64+1], |r| Ok(SharedChannelMessage {
            id:r.get(0)?, kind:"direct_delegation".into(), author_bot_id:Some(r.get(1)?),
            text:format!("Direct request to {}",r.get::<_,String>(2)?),
            status:Some(status_label(r.get(3)?)),created_at:r.get(4)?,
            recipient_ids:vec![r.get(5)?], council:None,
        })).map_err(|e| e.to_string())?;
        messages.extend(rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?);
    }
    messages.sort_by(|a,b| a.created_at.cmp(&b.created_at).then_with(|| {
        let rank = |kind: &str| match kind { "user" | "bot_request" => 0, "assigned" => 1, "reply" | "bot_post" => 2, _ => 3 };
        rank(&a.kind).cmp(&rank(&b.kind))
    }).then_with(|| a.id.cmp(&b.id)));
    let has_more = messages.len() > limit;
    let messages = messages.split_off(messages.len().saturating_sub(limit));
    let active_bot_ids = {
        let mut statement=store.db.prepare("SELECT DISTINCT bot_id FROM bot_shared_channel_deliveries WHERE channel_id=?1 AND status='in_progress' ORDER BY bot_id").map_err(|e|e.to_string())?;
        let rows=statement.query_map([id],|r|r.get(0)).map_err(|e|e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?
    };
    Ok(SharedChannelDetail { channel, messages, active_bot_ids, has_more })
}

fn status_label(status: String) -> String {
    match status.as_str() { "queued" => "Queued", "in_progress" => "In progress", "completed" => "Completed", "failed" => "Failed", "stopped" => "Stopped", _ => "Failed" }.into()
}

/// A post's status from its deliveries. A stop or failure is superseded by any attempt made
/// after it (Continue or Reassign), and a post left with no deliveries (its bot was deleted) is
/// stopped, so it can be continued.
pub(crate) fn message_status(store: &Store, message_id: &str) -> Result<String> {
    let rows: Vec<(String, i64, i64)> = {
        let mut statement = store.db.prepare("SELECT status,created_at,updated_at FROM bot_shared_channel_deliveries WHERE message_id=?1 ORDER BY created_at,id").map_err(|e| e.to_string())?;
        let rows=statement.query_map([message_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?
    };
    if rows.is_empty() { return Ok("stopped".into()); }
    let statuses: Vec<&str> = rows.iter()
        .filter(|(status, _, ended_at)| !matches!(status.as_str(), "stopped" | "failed") || !rows.iter().any(|(_, created, _)| created > ended_at))
        .map(|(status, _, _)| status.as_str()).collect();
    let status = if statuses.contains(&"in_progress") { "in_progress" }
        else if statuses.contains(&"queued") { "queued" }
        else if statuses.contains(&"failed") { "failed" }
        else if statuses.contains(&"stopped") { "stopped" }
        else { "completed" };
    Ok(status.into())
}

pub(crate) fn update_message_status(store: &Store, message_id: &str) -> Result<()> {
    let status = message_status(store, message_id)?;
    store.db.execute("UPDATE bot_shared_channel_messages SET status=?2 WHERE id=?1", params![message_id,status]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Posts with unfinished work for `bot_id`, read before a reset or delete cascades its deliveries away.
pub(crate) fn open_posts(store: &Store, bot_id: &str) -> Result<Vec<String>> {
    let mut statement = store.db.prepare("SELECT DISTINCT message_id FROM bot_shared_channel_deliveries WHERE bot_id=?1 AND status IN ('queued','in_progress')").map_err(|e| e.to_string())?;
    let rows = statement.query_map([bot_id], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
}

/// Recomputes those posts once the cascade is done, so none stays In progress with nothing behind it.
pub(crate) fn settle_posts(store: &Store, message_ids: &[String]) -> Result<()> {
    for id in message_ids { update_message_status(store, id)?; }
    Ok(())
}

/// Runs are not restored for bot deliveries, so a process restart must not leave
/// old assignments permanently marked as active.
pub(crate) fn reconcile_in_progress(store: &Store) -> Result<()> {
    store.db.execute("UPDATE bot_messages SET status='failed',result='Interrupted when PandaCode closed.',updated_at=?1 WHERE status='in_progress'", [crate::storage::now()]).map_err(|e| e.to_string())?;
    store.db.execute("UPDATE bot_shared_channel_deliveries SET status='failed',updated_at=?1 WHERE status='in_progress'", [crate::storage::now()]).map_err(|e| e.to_string())?;
    // A queued assignment whose queue entry is gone was taken just before PandaCode closed.
    let queued: Vec<(String,String,String,String)> = {
        let mut statement = store.db.prepare("SELECT d.id,d.session_id,c.name,m.text FROM bot_shared_channel_deliveries d JOIN bot_shared_channel_messages m ON m.id=d.message_id JOIN bot_shared_channels c ON c.id=d.channel_id WHERE d.status='queued'").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?
    };
    for (id, session_id, name, text) in queued {
        let prefix = format!("Work requested in #{name}:");
        if !message_queue::list(store, Some(&session_id))?.iter().any(|item| prompt_matches(&item.text, &prefix, &text)) {
            store.db.execute("UPDATE bot_shared_channel_deliveries SET status='failed',updated_at=?2 WHERE id=?1", params![id,crate::storage::now()]).map_err(|e| e.to_string())?;
        }
    }
    let messages = {
        let mut statement = store.db.prepare("SELECT id FROM bot_shared_channel_messages WHERE kind='user' AND status IN ('queued','in_progress')").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?
    };
    for id in messages { update_message_status(store, &id)?; }
    Ok(())
}

/// The post, then the channel's latest other posts and replies, so the assigned bot can build on
/// what the user and other members already said.
fn text_for_assignment(channel: &SharedChannel, text: &str, history: &str) -> String {
    if history.is_empty() { return format!("Work requested in #{}:\n\n{}", channel.name, text); }
    format!("Work requested in #{}:\n\n{}\n\nRecent messages in #{} (context, oldest first):\n{}", channel.name, text, channel.name, history)
}

/// Apply the recipient's Extended-only effective history policy at the shared-channel boundary.
/// The channel transcript is never deleted; ordinary recipients keep upstream context.
pub(crate) fn assignment_text(
    store: &Store, channel: &SharedChannel, bot_id: &str, message_id: &str, text: &str,
) -> Result<String> {
    let history = if super::extended::channel_history_effective(store, bot_id, &channel.id)? {
        recent_history(store, &channel.id, message_id)?
    } else {
        String::new()
    };
    Ok(text_for_assignment(channel, text, &history))
}

const HISTORY_CONTEXT: usize = 8;
const HISTORY_CHARS: usize = 600;

/// The channel's latest posts and replies other than `message_id`, oldest first, one per line.
pub(crate) fn recent_history(store: &Store, channel_id: &str, message_id: &str) -> Result<String> {
    let mut statement = store.db.prepare("SELECT b.name,m.text FROM bot_shared_channel_messages m LEFT JOIN bots b ON b.id=m.author_bot_id WHERE m.channel_id=?1 AND m.id<>?2 AND m.kind IN ('user','reply') ORDER BY m.created_at DESC,m.id DESC LIMIT ?3").map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![channel_id,message_id,HISTORY_CONTEXT as i64], |r| Ok((r.get::<_,Option<String>>(0)?, r.get::<_,String>(1)?))).map_err(|e| e.to_string())?;
    let mut lines = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?.into_iter().map(|(author, text)| {
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let clipped: String = text.chars().take(HISTORY_CHARS).collect();
        format!("- {}: {}{}", author.unwrap_or_else(|| "You".into()), clipped, if clipped.len() < text.len() { "…" } else { "" })
    }).collect::<Vec<_>>();
    lines.reverse();
    Ok(lines.join("\n"))
}

fn prompt_matches(content: &str, sender_prefix: &str, text: &str) -> bool {
    let Some((header, body)) = content.split_once("\n\n") else { return false };
    if !header.starts_with(sender_prefix) { return false; }
    body == text || body.strip_prefix(text).is_some_and(|suffix| suffix.starts_with("\n\n"))
}

/// Durable admission for one channel delivery, shared by native dispatch and
/// isolated-context regression tests. No model/harness execution occurs here.
pub(super) struct ChannelDeliveryAdmission {
    pub(super) created_at: i64,
    pub(super) depth: u8,
}

pub(super) fn provision_channel_delivery(
    store: &Store, channel_id: &str, message_id: &str, bot_id: &str,
    text: &str, admission: ChannelDeliveryAdmission, delivery_id: &str,
) -> Result<(SharedChannel, Bot, String, String)> {
    let ChannelDeliveryAdmission { created_at, depth } = admission;
        let channel = shared_channel(store, channel_id)?;
        if channel.archived { return Err("This channel is deleted.".into()); }
        if !channel.member_ids.iter().any(|id| id == bot_id) { return Err("Choose a bot that belongs to this channel.".into()); }
        let origin: Option<String> = store.db.query_row(
            "SELECT origin_bot_id FROM extended_bot_channel_actions WHERE message_id=?1",
            [message_id], |r| r.get(0),
        ).optional().map_err(|e| e.to_string())?;
        if origin.is_some_and(|origin| !channel.member_ids.contains(&origin)) {
            return Err("The sending bot is no longer a channel member.".into());
        }
        let bot = super::get(store, bot_id)?;
        if bot.paused { return Err(format!("{} is paused.", bot.name)); }
        let requested = council_request(store, message_id)?;
        if let Some(request) = &requested {
            let mut executors = request.executors.clone();
            if !executors.iter().any(|id| id == bot_id) { executors.push(bot_id.to_owned()); }
            crate::architect::validate_channel_reviewers(store, &request.council, &channel.member_ids, &executors)?;
        }
        let thread = channel_delivery_thread(delivery_id, requested.is_some());
        let title = if requested.is_some() { format!("{} · Channel Council", bot.name) }
            else { format!("{} · You", bot.name) };
        // One SQLite transaction protects the new task, native Council team and
        // linked delivery. Failure must not leave a detached bot conversation.
        let transaction = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let (session_id, _) = super::thread_session(store, &bot, DESKTOP, &thread, &title)?;
        if let Some(request) = &requested {
            crate::architect::configure_channel_delivery(store, &session_id, request.council.clone())?;
        }
        let prompt = assignment_text(store, &channel, &bot.id, message_id, text)?;
        store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,depth,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'queued',?6,?7,?7)", params![delivery_id,channel_id,message_id,bot_id,session_id,depth as i64,created_at]).map_err(|e| e.to_string())?;
        if requested.is_some() {
            store.db.execute("INSERT INTO extended_channel_council_deliveries(delivery_id,channel_id,source_message_id,executor_bot_id,cycle_id,objective,artifact_revision,created_at) VALUES(?1,?2,?3,?4,?5,?6,'0',?7)", params![delivery_id,channel_id,message_id,bot_id,crate::storage::id(),text,created_at]).map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())?;
        Ok((channel, bot, prompt, thread))
}

/// `created_at` is shared by every delivery of one post or retry, so a failure early in a
/// post's fan-out never counts as superseded by its sibling (see `message_status`).
pub(crate) async fn send_channel_assignment(app: &tauri::AppHandle, channel_id: &str, message_id: &str, bot_id: &str, text: &str, created_at: i64, depth: u8) -> Result<()> {
    let state = app.state::<AppState>();
    // The delivery starts queued; the gateway marks it in progress when its turn launches
    // (`promote_taken`), so a finished run is credited only to the work it actually ran.
    let delivery_id = crate::storage::id();
    let (channel, bot, prompt, thread) = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        provision_channel_delivery(&store, channel_id, message_id, bot_id, text, ChannelDeliveryAdmission { created_at, depth }, &delivery_id)?
    };
    let inbound = gateway::Inbound { bot_id: bot.id.clone(), channel_id: DESKTOP.into(), thread, sender_id: format!("channel:{channel_id}"), sender_name: format!("#{}", channel.name), text: prompt, trusted: true, image_ids: None, moodboard_ids: None };
    if let Err(error) = gateway::deliver(app, inbound).await {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.db.execute("UPDATE bot_shared_channel_deliveries SET status='failed',updated_at=?2 WHERE id=?1 AND status IN ('queued','in_progress')", params![delivery_id,crate::storage::now()]).map_err(|e|e.to_string())?;
        update_message_status(&store, message_id)?;
        return Err(error);
    }
    // A Stop, archive or member removal that landed while this was being handed over found no
    // queue entry or run yet; stop what the hand-over just queued or started.
    // If the hand-over already launched the turn, stop that run directly: its queue entry is gone,
    // so re-queuing the row would let `stop_pending` miss the live session.
    let stopped_session: Option<String> = state.store.lock().map_err(|e| e.to_string())
        .and_then(|store| store.db.query_row("SELECT session_id FROM bot_shared_channel_deliveries WHERE id=?1 AND status='stopped'", [&delivery_id], |r| r.get(0)).optional().map_err(|e| e.to_string()))?;
    if let Some(session_id) = stopped_session {
        let running = state.agents.lock().map_err(|e| e.to_string())?.contains_key(&session_id);
        if running {
            let _ = crate::stop_session(app.clone(), app.state::<AppState>(), session_id).await;
        } else {
            state.store.lock().map_err(|e| e.to_string())
                .and_then(|store| store.db.execute("UPDATE bot_shared_channel_deliveries SET status='queued' WHERE id=?1 AND status='stopped'", [&delivery_id]).map_err(|e| e.to_string()))?;
            stop_pending(app, channel_id, Some(&[bot_id.to_owned()])).await?;
        }
    }
    let _ = app.emit("bot-shared-channel-changed", serde_json::json!({"channelId":channel_id}));
    Ok(())
}

/// An opt-in delegation gets its own bot conversation; the ordinary desktop task
/// (including its native harness history and durable queue) is never reset.
pub(crate) fn bot_message_target(store: &Store, source: &Bot, target: &Bot, message_id: &str, fresh_context: bool) -> Result<(String, String)> {
    let thread = if fresh_context { format!("delegation:{message_id}") } else { DESKTOP.to_owned() };
    let title = if fresh_context { format!("{} · From {}", target.name, source.name) } else { format!("{} · You", target.name) };
    let (session_id, _) = super::thread_session(store, target, DESKTOP, &thread, &title)?;
    Ok((session_id, thread))
}

pub(crate) async fn deliver_bot_message(app: &tauri::AppHandle, source: &Bot, source_thread: &super::Thread, target: &Bot, text: &str, fresh_context: bool, route: &collaboration::Route) -> Result<String> {
    let text = text.trim();
    if text.is_empty() { return Err("Write a message for the other bot.".into()); }
    if text.chars().count() > MAX_TEXT { return Err("Keep bot messages under 16,384 characters.".into()); }
    let state = app.state::<AppState>();
    let id = crate::storage::id();
    let (session_id, target_thread) = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        bot_message_target(&store, source, target, &id, fresh_context)?
    };
    // Queued until the gateway launches its turn (`promote_taken`).
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.db.execute("INSERT INTO bot_messages(id,source_bot_id,target_bot_id,source_session_id,target_session_id,source_channel_id,source_external_id,text,status,depth,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'queued',?9,?10,?10)", params![id,source.id,target.id,source_thread.session_id,session_id,source_thread.channel_id,source_thread.external_id,text,route.depth as i64,crate::storage::now()]).map_err(|e| e.to_string())?;
        collaboration::record_direct_route(&store, &id, &source_thread.session_id, route)?;
    }
    let inbound = gateway::Inbound { bot_id: target.id.clone(), channel_id: DESKTOP.into(), thread: target_thread, sender_id: format!("bot:{}", source.id), sender_name: format!("Message from {}", source.name), text: format!("Message from {}:\n\n{}", source.name, text), trusted: true, image_ids: None, moodboard_ids: None };
    if let Err(error) = gateway::deliver(app, inbound).await {
        if let Ok(store) = state.store.lock() { let _ = store.db.execute("UPDATE bot_messages SET status='failed',updated_at=?2 WHERE id=?1", params![id,crate::storage::now()]); }
        let _=app.emit("bot-messages-changed", ());
        return Err(error);
    }
    let _ = app.emit("bot-messages-changed", ());
    let _ = app.emit("bots-changed", ());
    Ok(id)
}

pub(crate) fn current_hop_depth(store: &Store, session_id: &str) -> Result<u8> {
    let direct: i64 = store.db.query_row("SELECT COALESCE(MAX(depth),0) FROM bot_messages WHERE target_session_id=?1 AND status='in_progress'", [session_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let channel: i64 = store.db.query_row("SELECT COALESCE(MAX(d.depth),0) FROM bot_shared_channel_deliveries d WHERE d.session_id=?1 AND d.status='in_progress'", [session_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    // A delegated bot's result becomes a new user message in the sender's ordinary task.
    // Recover that request's depth from the matching "Update from …" message so a queued
    // follow-up cannot reset the three-hop guard when a reply starts another turn.
    let latest_user: Option<String> = store.db.query_row("SELECT content FROM messages WHERE session_id=?1 AND role='user' ORDER BY id DESC LIMIT 1", [session_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    let update_depth = if let Some(content) = latest_user {
        let mut statement = store.db.prepare("SELECT b.name,m.result,m.depth FROM bot_messages m JOIN bots b ON b.id=m.target_bot_id WHERE m.source_session_id=?1 AND m.result IS NOT NULL ORDER BY m.updated_at DESC").map_err(|e| e.to_string())?;
        let rows = statement.query_map([session_id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?))).map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?.into_iter().find_map(|(name,result,depth)| {
            content.starts_with(&format!("Update from {name}:\n\n{result}")).then_some(depth)
        }).unwrap_or(0)
    } else { 0 };
    Ok(direct.max(channel).max(update_depth).clamp(0, 3) as u8)
}

pub(crate) fn markers(store: &Store, session_id: &str) -> Result<Vec<BotMessageMarker>> {
    let sql = "SELECT m.id,'sent',b.id,b.name,b.avatar,b.color,m.text,m.result,m.status,m.created_at FROM bot_messages m JOIN bots b ON b.id=m.target_bot_id WHERE m.source_session_id=?1 \
      UNION ALL SELECT m.id,'received',b.id,b.name,b.avatar,b.color,m.text,NULL,m.status,m.created_at FROM bot_messages m JOIN bots b ON b.id=m.source_bot_id WHERE m.target_session_id=?1 \
      UNION ALL SELECT m.id,'update',b.id,b.name,b.avatar,b.color,m.result,m.result,m.status,m.updated_at FROM bot_messages m JOIN bots b ON b.id=m.target_bot_id WHERE m.source_session_id=?1 AND m.result IS NOT NULL AND trim(m.result)<>'' \
      ORDER BY 10,1,2";
    let mut statement=store.db.prepare(sql).map_err(|e|e.to_string())?;
    let rows=statement.query_map([session_id],|r|Ok(BotMessageMarker{id:r.get(0)?,kind:r.get(1)?,bot_id:r.get(2)?,bot_name:r.get(3)?,avatar:r.get(4)?,color:r.get(5)?,text:r.get(6)?,result:r.get(7)?,status:status_label(r.get(8)?),created_at:r.get(9)?})).map_err(|e|e.to_string())?;
    rows.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())
}

pub(crate) fn latest_reply(store: &Store, session_id: &str) -> Result<String> {
    let mut statement=store.db.prepare("SELECT content FROM messages WHERE session_id=?1 AND role='assistant' AND id>COALESCE((SELECT MAX(id) FROM messages WHERE session_id=?1 AND role='user'),0) ORDER BY id").map_err(|e|e.to_string())?;
    let rows=statement.query_map([session_id],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?.join("\n\n").trim().to_owned())
}

/// Authorize callbacks at their final durable admission boundary, including
/// replay/restart/Stop callers that bypass the live run-finished gate.
/// Ordinary (Council-off) collaboration retains its original callback behavior.
pub(crate) fn council_callback_allowed(store: &Store, delivery_id: &str) -> Result<bool> {
    let linked: Option<(String,String,String,String)> = store.db.query_row(
        "SELECT d.session_id,d.status,c.artifact_revision,c.source_message_id
         FROM extended_channel_council_deliveries c
         JOIN bot_shared_channel_deliveries d ON d.id=c.delivery_id
         WHERE c.delivery_id=?1",
        [delivery_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((session_id,status,frozen_cycle,message_id)) = linked else { return Ok(true) };
    if status != "completed" { return Ok(false); }
    let Some(team) = crate::architect::load(store, &session_id)? else { return Ok(false) };
    if team.status != "completed" || team.config.council.is_none()
        || frozen_cycle != format!("cycle:{}",team.council_since)
    { return Ok(false); }
    let requested = council_request(store, &message_id)?;
    if requested.is_none_or(|original| team.config.council.as_ref() != Some(&original.council)) {
        return Ok(false);
    }
    let Some(verdict) = team.council_status.as_ref() else { return Ok(false) };
    if verdict.stale { return Ok(false); }
    if verdict.state == "passed" { return Ok(true); }
    if verdict.state != "accepted" { return Ok(false); }
    let proof: Option<(String,i64,i64,i64)> = store.db.query_row(
        "SELECT a.cycle_id,a.council_since,a.dispatch_revision,a.changed_at
         FROM extended_channel_council_acceptances a
         JOIN extended_channel_council_deliveries c ON c.delivery_id=a.delivery_id
         WHERE a.delivery_id=?1 AND a.cycle_id=c.cycle_id AND a.actor='local_user'",
        [delivery_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).optional().map_err(|e|e.to_string())?;
    Ok(proof.is_some_and(|(_,since,revision,changed)| {
        team.council_accepted == Some(since) && team.council_since == since
            && team.dispatch_revision as i64 == revision && team.changed_at == changed
    }))
}

/// Atomically claim one terminal delivery and its channel message. A duplicate
/// callback, stale Council verdict or Stop that already won cannot publish again.
/// The returned bool permits an ordinary collaboration callback; None means no
/// terminal transition was made.
pub(super) fn settle_channel_delivery(
    store: &Store, session_id: &str,
    delivery: (&str, &str, &str, &str),
    successful: bool, reply: &str,
) -> Result<Option<bool>> {
    let (delivery_id, channel_id, message_id, bot_id) = delivery;
    let transaction = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    // Re-read under the same SQLite transaction as the terminal claim:
    // a changed review/revision must not pass on a stale pre-gateway decision.
    let gate = super::channel_council_outcome(store, session_id)?;
    if gate == super::ChannelCouncilOutcome::Waiting { return Ok(None); }
    let denied = matches!(gate, super::ChannelCouncilOutcome::Denied(_));
    let approved = successful && !denied;
    let changed = store.db.execute(
        "UPDATE bot_shared_channel_deliveries SET status=?2,updated_at=?3
         WHERE id=?1 AND session_id=?4 AND status='in_progress'",
        params![delivery_id,if approved {"completed"} else {"failed"},crate::storage::now(),session_id],
    ).map_err(|e| e.to_string())?;
    if changed == 0 { return Ok(None); }
    // The legacy message schema accepts only user/reply, not synthetic
    // Council notices. A denied delivery is visible through its native
    // per-delivery status/notice, never as a fabricated bot response.
    if approved && !reply.trim().is_empty() {
        store.db.execute(
            "INSERT INTO bot_shared_channel_messages(id,channel_id,kind,author_bot_id,text,created_at)
             VALUES(?1,?2,'reply',?3,?4,?5)",
            params![crate::storage::id(),channel_id,bot_id,reply.trim(),crate::storage::now()],
        ).map_err(|e| e.to_string())?;
    }
    // Keep authorization, target, accepted report and revision immutable before
    // any external GitHub action; a user opt-in is scoped to this delivery only.
    if approved {
        if let Some(grant) = council_request(store, message_id)?
            .and_then(|request| request.council.post_acceptance_issue)
        {
            let cycle: String = store.db.query_row(
                "SELECT cycle_id FROM extended_channel_council_deliveries WHERE delivery_id=?1",
                [delivery_id], |r| r.get(0),
            ).map_err(|e| e.to_string())?;
            let team = crate::architect::load(store, session_id)?
                .ok_or("Council approval has no persisted team.")?;
            let revision = format!("{}:{}:{}", cycle,team.council_since,team.changed_at);
            let objective: String = store.db.query_row(
                "SELECT objective FROM extended_channel_council_deliveries WHERE delivery_id=?1",
                [delivery_id], |r| r.get(0),
            ).map_err(|e| e.to_string())?;
            let excerpt = objective.split_whitespace().collect::<Vec<_>>().join(" ");
            let title = format!("Council accepted: {}",excerpt.chars().take(100).collect::<String>());
            let findings = reply.trim();
            let ready = !findings.is_empty() && findings.chars().count() <= 16_384;
            let body = if ready {
                format!("Council-approved findings from delivery {} (revision {}):\n\n{}",delivery_id,revision,findings)
            } else { String::new() };
            store.db.execute(
                "INSERT OR IGNORE INTO extended_channel_council_followups
                 (delivery_id,authorization_id,repository,revision,title,body,state,error,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![delivery_id,grant.authorization_id,grant.repository,revision,title,body,
                    if ready {"pending"} else {"failed"},
                    if ready {None} else {Some("No complete accepted findings are available for GitHub publishing.")},
                    crate::storage::now()],
            ).map_err(|e| e.to_string())?;
        }
    }
    update_message_status(store, message_id)?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(Some(approved))
}

/// Claim one post-acceptance GitHub issue attempt. A crash after 'sending'
/// leaves an explicitly uncertain result; it must never silently create twice.
pub(super) fn claim_post_acceptance_issue(store: &Store, delivery_id: &str) -> Result<Option<(String,String,String)>> {
    let tx = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
    let item: Option<(String,String,String)> = store.db.query_row(
        "SELECT repository,title,body FROM extended_channel_council_followups
         WHERE delivery_id=?1 AND state='pending'",
        [delivery_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some(item) = item else { return Ok(None) };
    let changed = store.db.execute(
        "UPDATE extended_channel_council_followups SET state='sending',updated_at=?2
         WHERE delivery_id=?1 AND state='pending'",
        params![delivery_id,crate::storage::now()],
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok((changed==1).then_some(item))
}

/// Post on behalf of this delivery's configured executor, using the user's
/// existing gh CLI session. Only exact user-authorized owner/repo is allowed.
pub(super) async fn publish_post_acceptance_issue(app: &tauri::AppHandle, delivery_id: &str) {
    let state = app.state::<AppState>();
    let item = state.store.lock().map_err(|e|e.to_string())
        .and_then(|store| claim_post_acceptance_issue(&store,delivery_id));
    let Ok(Some((repo,title,body))) = item else { return };
    let repository = repo.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<String> {
        let args = [
            "issue".to_string(), "create".to_string(),
            format!("--repo={repo}"),format!("--title={title}"),format!("--body={body}"),
        ];
        let argv = args.iter().map(String::as_str).collect::<Vec<_>>();
        let captured = crate::github::run_gh(std::path::Path::new("."), &argv, crate::github::TIMEOUT)?;
        if captured.truncated { return Err("GitHub response was truncated; the result is uncertain.".into()); }
        let output = String::from_utf8_lossy(&captured.bytes);
        let expected = format!("https://github.com/{repository}/issues/");
        let url = output.trim();
        if url.strip_prefix(&expected).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) {
            Ok(url.to_string())
        } else {
            Err("GitHub issue creation result is uncertain; do not retry automatically.".into())
        }
    }).await.unwrap_or_else(|_| Err("GitHub issue creation result is uncertain; do not retry automatically.".into()));
    let (status,url,error) = match result {
        Ok(url) => ("succeeded",Some(url),None),
        Err(error) if error.contains("uncertain") => ("unknown",None,Some("The GitHub result is unknown; check the repository before trying again.".to_string())),
        Err(_) => ("failed",None,Some("The authorized GitHub issue could not be created; check the GitHub CLI and repository access.".to_string())),
    };
    if let Ok(store) = state.store.lock() {
        let _ = store.db.execute(
            "UPDATE extended_channel_council_followups SET state=?2,issue_url=?3,error=?4,updated_at=?5
             WHERE delivery_id=?1 AND state='sending'",
            params![delivery_id,status,url,error,crate::storage::now()],
        );
    }
    let _=app.emit("bot-shared-channel-changed",serde_json::json!({}));
}

/// Finalize the assignment whose turn just ended, copy its result into the channel or sender
/// thread, then promote the next queued collaboration request if the ordinary gateway started it.
pub(crate) async fn run_finished(app: &tauri::AppHandle, session_id: &str, successful: bool, reply: &str) {
    let state=app.state::<AppState>();
    // The request this run carried; with none recorded (an older run), the oldest active one.
    let tracked = take_running(session_id);
    let (message_id, delivery_id) = match &tracked { Some(Running::Message(id)) => (Some(id.as_str()),None), Some(Running::Delivery(id)) => (None,Some(id.as_str())), None => (None,None) };
    let direct = if delivery_id.is_some() { None } else {
        let Ok(store)=state.store.lock() else { return };
        store.db.query_row("SELECT id,source_bot_id,target_bot_id,source_channel_id,source_external_id FROM bot_messages WHERE target_session_id=?1 AND status='in_progress' AND (?2 IS NULL OR id=?2) ORDER BY created_at,id LIMIT 1", params![session_id,message_id], |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).optional().ok().flatten()
    };
    let channel = if message_id.is_some() { None } else {
        let Ok(store)=state.store.lock() else { return };
        store.db.query_row("SELECT id,channel_id,message_id,bot_id FROM bot_shared_channel_deliveries WHERE session_id=?1 AND status='in_progress' AND (?2 IS NULL OR id=?2) ORDER BY created_at,id LIMIT 1", params![session_id,delivery_id], |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).optional().ok().flatten()
    };
    if let Some((id,source_bot_id,target_bot_id,source_channel_id,source_external_id)) = direct {
        let result = if !successful { "I couldn't finish that message. Check PandaCode for details." } else if reply.trim().is_empty() { "The bot finished without a text reply." } else { reply.trim() };
        let source_bot = state.store.lock().ok().and_then(|store| super::get(&store,&source_bot_id).ok());
        let target_bot = state.store.lock().ok().and_then(|store| super::get(&store,&target_bot_id).ok());
        if let Ok(store)=state.store.lock() {
            let status=if successful {"completed"} else {"failed"};
            let _=store.db.execute("UPDATE bot_messages SET status=?2,result=?3,updated_at=?4 WHERE id=?1",params![id,status,result,crate::storage::now()]);
        }
        if let (Some(source),Some(target))=(source_bot,target_bot) {
            let text=format!("Update from {}:\n\n{}",target.name,result);
            let _=gateway::deliver(app,gateway::Inbound{bot_id:source.id,channel_id:source_channel_id,thread:source_external_id,sender_id:format!("bot-update:{id}"),sender_name:format!("Update from {}",target.name),text,trusted:true,image_ids:None,moodboard_ids:None}).await;
        }
        let _=app.emit("bot-messages-changed",());
    }
    if let Some((delivery_id,channel_id,message_id,bot_id)) = channel {
        let callback = {
            let store = match state.store.lock() { Ok(store)=>store, Err(_)=>return };
            match settle_channel_delivery(&store, session_id, (&delivery_id, &channel_id, &message_id, &bot_id), successful, reply) {
                Ok(Some(callback)) => callback,
                Ok(None) => return,
                Err(error) => {
                    eprintln!("Channel delivery settlement failed: {error}");
                    return;
                }
            }
        };
        let _=app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel_id}));
        if callback { collaboration::finish_delivery(app, &delivery_id, reply).await; }
        publish_post_acceptance_issue(app, &delivery_id).await;
    }
    // The window may have sent the next queued entry itself; credit it to its request.
    let running=state.agents.lock().map(|agents|agents.contains_key(session_id)).unwrap_or(false);
    if running {
        if let Ok(store)=state.store.lock() {
            let current:Option<String>=store.db.query_row("SELECT content FROM messages WHERE session_id=?1 AND role='user' ORDER BY id DESC LIMIT 1",[session_id],|r|r.get(0)).optional().ok().flatten();
            if let Some(current)=current { promote_taken(&store,session_id,&current); }
        }
    }
    let _=app.emit("bot-messages-changed",());
}

/// Accept below-target Council work only from an explicit local user action.
/// A model message, ordinary Council acceptance or a stale cycle cannot issue
/// the durable channel approval proof needed by the callback gate.
#[tauri::command]
pub async fn bot_shared_channel_council_accept(
    app: tauri::AppHandle, state: State<'_,AppState>, delivery_id: String, reason: String,
) -> Result<()> {
    let reason = reason.trim();
    if !(8..=1024).contains(&reason.chars().count()) || reason.chars().any(char::is_control) {
        return Err("Enter an acceptance reason between 8 and 1024 printable characters.".into());
    }
    let (channel_id, session_id, cycle_id, expected_since, expected_dispatch, expected_changed) = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let (channel_id, session_id, cycle_id): (String, String, String) = store.db.query_row(
            "SELECT c.channel_id,d.session_id,c.cycle_id
             FROM extended_channel_council_deliveries c
             JOIN bot_shared_channel_deliveries d ON d.id=c.delivery_id
             WHERE d.id=?1 AND d.status='in_progress'",
            [&delivery_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).map_err(|_| "Council delivery is not active or does not exist.".to_string())?;
        let channel = shared_channel(&store, &channel_id)?;
        if channel.archived { return Err("This channel is archived.".into()); }
        let team = crate::architect::load(&store, &session_id)?
            .ok_or("Council delivery has no native team.")?;
        let config = team.config.council.as_ref().ok_or("Council is disabled.")?;
        validate_delivery_reviewers(&store, &session_id, &config.reviewer_bot_ids)?;
        let status = team.council_status.as_ref().ok_or("Council has no review.")?;
        if status.round == 0 || status.lowest.is_none() || status.stale
            || !matches!(team.status.as_str(), "running" | "waiting" | "ready" | "completed")
        {
            return Err("A current Council review with recorded scores is required before exceptional acceptance.".into());
        }
        (channel_id, session_id, cycle_id, team.council_since, team.dispatch_revision, team.changed_at)
    };
    let team = crate::architect::accept_channel_delivery(&app, &state, &session_id)?;
    // Do not let a concurrent reassignment, stop or newer reviewer revision
    // rebind a local user's approval to a different delivery.
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let transaction = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let current_cycle: String = store.db.query_row(
            "SELECT c.cycle_id FROM extended_channel_council_deliveries c
             JOIN bot_shared_channel_deliveries d ON d.id=c.delivery_id
             WHERE d.id=?1 AND d.session_id=?2 AND d.status='in_progress'",
            params![delivery_id,session_id], |r| r.get(0),
        ).map_err(|_| "Council delivery changed during acceptance.".to_string())?;
        if current_cycle != cycle_id || team.council_accepted != Some(team.council_since)
            || team.council_since != expected_since || team.dispatch_revision != expected_dispatch
            || team.changed_at != expected_changed
            || team.council_status.as_ref().is_none_or(|s| s.state != "accepted" || s.stale)
        {
            return Err("Council approval no longer matches the current review.".into());
        }
        let selected = &team.config.council.as_ref()
            .ok_or("Council is disabled.")?.reviewer_bot_ids;
        validate_delivery_reviewers(&store, &session_id, selected)?;
        store.db.execute(
            "INSERT OR IGNORE INTO extended_channel_council_acceptances
             (delivery_id,cycle_id,council_since,dispatch_revision,changed_at,actor,reason,accepted_at)
             VALUES(?1,?2,?3,?4,?5,'local_user',?6,?7)",
            params![delivery_id,cycle_id,team.council_since,team.dispatch_revision as i64,
                team.changed_at,reason,crate::storage::now()],
        ).map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
    }
    let _ = app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel_id}));
    if team.status == "completed" {
        // A completed at-limit lead has no future harness turn to call back.
        super::run_finished(&app, &session_id, true);
    } else {
        crate::architect::wake(app.clone());
    }
    Ok(())
}

#[tauri::command]
pub async fn bot_shared_channels(state: State<'_,AppState>) -> Result<Vec<SharedChannel>> {
    let store = state.store.lock().map_err(|e|e.to_string())?;
    list(&store)
}

#[tauri::command]
pub async fn bot_shared_channel_detail(state: State<'_,AppState>, channel_id:String, limit:Option<usize>) -> Result<SharedChannelDetail> {
    let store = state.store.lock().map_err(|e|e.to_string())?;
    detail(&store,&channel_id,limit.unwrap_or(HISTORY_PAGE))
}

#[tauri::command]
pub async fn bot_shared_channel_create(app:tauri::AppHandle,state:State<'_,AppState>,name:String,member_ids:Vec<String>) -> Result<SharedChannel> {
    let store = state.store.lock().map_err(|e|e.to_string())?;
    let channel=create(&store,&name,&member_ids)?;
    drop(store);
    let _=app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel.id}));
    Ok(channel)
}

#[tauri::command]
pub async fn bot_shared_channel_update(app:tauri::AppHandle,state:State<'_,AppState>,channel_id:String,name:String,member_ids:Vec<String>,lead_bot_id:Option<String>) -> Result<SharedChannel> {
    let (channel,removed)={
        let store = state.store.lock().map_err(|e|e.to_string())?;
        let before=shared_channel(&store,&channel_id)?.member_ids;
        let channel=update(&store,&channel_id,&name,&member_ids,lead_bot_id.as_deref())?;
        let removed:Vec<String>=before.into_iter().filter(|id| !channel.member_ids.contains(id)).collect();
        (channel,removed)
    };
    // A removed member's pending work stops; its posts can be continued by the others.
    if !removed.is_empty() { stop_pending(&app,&channel_id,Some(&removed)).await?; }
    let _=app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel.id}));
    Ok(channel)
}

#[tauri::command]
pub async fn bot_shared_channel_archive(app:tauri::AppHandle,state:State<'_,AppState>,channel_id:String,archived:bool) -> Result<()> {
    // Archive first, so no post is admitted while its pending work stops.
    set_archived(&*state.store.lock().map_err(|e|e.to_string())?,&channel_id,archived)?;
    if archived { stop_pending(&app,&channel_id,None).await?; }
    let _=app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel_id}));
    Ok(())
}

#[tauri::command]
pub async fn bot_shared_channel_post(app:tauri::AppHandle,state:State<'_,AppState>,channel_id:String,text:String,recipients:Option<Vec<String>>,council:Option<crate::architect::CouncilConfig>) -> Result<()> {
    let text=text.trim().to_owned();
    if text.is_empty() || text.chars().count()>MAX_TEXT { return Err("Write a channel message under 16,384 characters.".into()); }
    let (channel,message_id,recipients)={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let channel=shared_channel(&store,&channel_id)?;
        if channel.archived { return Err("This channel is deleted.".into()); }
        let recipients=route_recipients(&channel.member_ids,channel.lead_bot_id.as_deref(),recipients.as_deref().unwrap_or_default())?;
        if let Some(config) = &council {
            crate::architect::validate_channel_reviewers(&store, config, &channel.member_ids, &recipients)?;
        }
        let message_id=crate::storage::id();
        let transaction=store.db.unchecked_transaction().map_err(|e|e.to_string())?;
        store.db.execute("INSERT INTO bot_shared_channel_messages(id,channel_id,kind,text,status,created_at) VALUES(?1,?2,'user',?3,'in_progress',?4)",params![message_id,channel_id,text,crate::storage::now()]).map_err(|e|e.to_string())?;
        if let Some(config) = &council {
            store.db.execute("INSERT INTO extended_channel_council_requests(message_id,council_json,executor_ids,created_at) VALUES(?1,?2,?3,?4)",params![message_id,serde_json::to_string(config).map_err(|e|e.to_string())?,serde_json::to_string(&recipients).map_err(|e|e.to_string())?,crate::storage::now()]).map_err(|e|e.to_string())?;
        }
        transaction.commit().map_err(|e|e.to_string())?;
        (channel,message_id,recipients)
    };
    let posted_at=crate::storage::now();
    for bot_id in recipients {
        if let Err(error)=send_channel_assignment(&app,&channel_id,&message_id,&bot_id,&text,posted_at,0).await {
            if let Ok(store)=state.store.lock() {
                // send_channel_assignment already recorded the failure once it had a delivery row.
                let _=store.db.execute("INSERT INTO bot_shared_channel_deliveries(id,channel_id,message_id,bot_id,session_id,status,created_at,updated_at) SELECT ?1,?2,?3,?4,session_id,'failed',?5,?5 FROM bot_threads WHERE bot_id=?4 AND channel_id=?6 AND NOT EXISTS (SELECT 1 FROM bot_shared_channel_deliveries WHERE message_id=?3 AND bot_id=?4) LIMIT 1",params![crate::storage::id(),channel_id,message_id,bot_id,posted_at,DESKTOP]);
                let _=update_message_status(&store,&message_id);
            }
            super::note(&app,&bot_id,"error",&format!("Channel assignment failed: {}",error.chars().take(160).collect::<String>()),serde_json::json!({"channelId":channel.id,"messageId":message_id}));
        }
    }
    {
        let store=state.store.lock().map_err(|e|e.to_string())?;
        update_message_status(&store,&message_id)?;
    }
    let _=app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel_id}));
    Ok(())
}

/// Stops a channel's unfinished deliveries (only `bots`' ones, when given). A queued assignment
/// leaves its bot's queue; one already taken from the queue or running stops that bot's turn
/// when the turn is this assignment. The records stay, marked stopped.
async fn stop_pending(app:&tauri::AppHandle,channel_id:&str,bots:Option<&[String]>) -> Result<()> {
    let state=app.state::<AppState>();
    let mut cancelled_holds=false;
    let (stop_sessions, stopped_deliveries): (HashSet<String>, Vec<String>)={
        let running: HashSet<String> = state.agents.lock().map_err(|e|e.to_string())?.keys().cloned().collect();
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let channel=shared_channel(&store,channel_id)?;
        let deliveries: Vec<(String,String,String,String,String,String)> = {
            let mut statement=store.db.prepare("SELECT d.id,d.message_id,d.session_id,d.status,m.text,d.bot_id FROM bot_shared_channel_deliveries d JOIN bot_shared_channel_messages m ON m.id=d.message_id WHERE d.channel_id=?1 AND d.status IN ('queued','in_progress') ORDER BY d.created_at,d.id").map_err(|e|e.to_string())?;
            let rows=statement.query_map([channel_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(|e|e.to_string())?;
            rows.collect::<rusqlite::Result<_>>().map_err(|e|e.to_string())?
        };
        let assignment_prefix=format!("Work requested in #{}:",channel.name);
        let mut stop_sessions=HashSet::new();
        let mut stopped_deliveries=Vec::new();
        let mut stopped_messages=HashSet::new();
        for (delivery_id,message_id,session_id,status,text,bot_id) in deliveries {
            if bots.is_some_and(|bots| !bots.contains(&bot_id)) { continue; }
            let queued = if status == "queued" { message_queue::list(&store,Some(&session_id))?.into_iter().find(|item| prompt_matches(&item.text,&assignment_prefix,&text)) } else { None };
            if let Some(queued) = queued {
                let _=message_queue::remove(&store,&queued.id);
            } else if status == "in_progress" {
                let latest: Option<String> = store.db.query_row("SELECT content FROM messages WHERE session_id=?1 AND role='user' ORDER BY id DESC LIMIT 1", [&session_id], |r|r.get(0)).optional().map_err(|e|e.to_string())?;
                if latest.as_deref().is_some_and(|content| prompt_matches(content,&assignment_prefix,&text)) {
                    if running.contains(&session_id) {
                        stop_sessions.insert(session_id.clone());
                    } else {
                        // Held by failover: no run will end for it, so drop the hold and its card.
                        forget(&session_id,&Running::Delivery(delivery_id.clone()));
                        super::failover::cancel(&session_id);
                        store.set_resume_at_reset(&session_id,false)?;
                        cancelled_holds=true;
                    }
                }
            }
            store.db.execute("UPDATE bot_shared_channel_deliveries SET status='stopped',updated_at=?2 WHERE id=?1",params![delivery_id,crate::storage::now()]).map_err(|e|e.to_string())?;
            stopped_messages.insert(message_id);
            stopped_deliveries.push(delivery_id);
        }
        for message_id in &stopped_messages { update_message_status(&store,message_id)?; }
        (stop_sessions, stopped_deliveries)
    };
    for id in stop_sessions { let _=crate::stop_session(app.clone(),app.state::<AppState>(),id).await; }
    for id in stopped_deliveries { collaboration::finish_delivery(app, &id, "").await; }
    if cancelled_holds { let _=app.emit("bot-messages-changed",()); let _=app.emit("sessions-changed",()); }
    Ok(())
}

/// Marks the queued bot message or channel delivery a turn about to launch (or a just-taken queue
/// entry) belongs to as in progress, in the same store lock as the launch or take, so Stop never
/// sees it queued with no queue row and a finished run is credited to the work it ran. Returns
/// the request, also recorded as the session's next running request for `run_finished`.
pub(crate) fn promote_taken(store: &Store, session_id: &str, text: &str) -> Option<Running> {
    let promoted = promote(store, session_id, text)?;
    if let Ok(mut map) = running().lock() { map.entry(session_id.to_owned()).or_default().push(promoted.clone()); }
    Some(promoted)
}

/// A bot request a turn carries: a `bot_messages` id or a channel delivery id.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Running { Message(String), Delivery(String) }

/// Requests promoted per session, oldest first; each finished run settles the first one.
fn running() -> &'static Mutex<HashMap<String, Vec<Running>>> {
    static RUNNING: std::sync::OnceLock<Mutex<HashMap<String, Vec<Running>>>> = std::sync::OnceLock::new();
    RUNNING.get_or_init(Default::default)
}

fn forget(session_id: &str, request: &Running) {
    if let Ok(mut map) = running().lock() {
        if let Some(list) = map.get_mut(session_id) { list.retain(|item| item != request); if list.is_empty() { map.remove(session_id); } }
    }
}

fn take_running(session_id: &str) -> Option<Running> {
    let mut map = running().lock().ok()?;
    let list = map.get_mut(session_id)?;
    let first = (!list.is_empty()).then(|| list.remove(0));
    if list.is_empty() { map.remove(session_id); }
    first
}

/// A promoted request whose turn did not launch goes back to queued (its queue entry is
/// restored or its sender settles it), so Stop finds it and no later run is credited with it.
pub(crate) fn launch_failed(store: &Store, session_id: &str, request: &Running) {
    forget(session_id, request);
    let now = crate::storage::now();
    match request {
        Running::Message(id) => { let _ = store.db.execute("UPDATE bot_messages SET status='queued',updated_at=?2 WHERE id=?1 AND status='in_progress'", params![id,now]); }
        Running::Delivery(id) => {
            let _ = store.db.execute("UPDATE bot_shared_channel_deliveries SET status='queued',updated_at=?2 WHERE id=?1 AND status='in_progress'", params![id,now]);
            let message_id: Option<String> = store.db.query_row("SELECT message_id FROM bot_shared_channel_deliveries WHERE id=?1",[id],|r|r.get(0)).ok();
            if let Some(message_id) = message_id { let _ = update_message_status(store,&message_id); }
        }
    }
}

fn promote(store: &Store, session_id: &str, text: &str) -> Option<Running> {
    let direct: Vec<(String,String)> = store.db.prepare("SELECT id,text FROM bot_messages WHERE target_session_id=?1 AND status='queued' ORDER BY created_at,id").ok()
        .and_then(|mut statement| statement.query_map([session_id],|r|Ok((r.get(0)?,r.get(1)?))).ok().and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>().ok())).unwrap_or_default();
    if let Some(id) = direct.into_iter().find_map(|(id,body)| prompt_matches(text,"Message from ",&body).then_some(id)) {
        let _ = store.db.execute("UPDATE bot_messages SET status='in_progress',updated_at=?2 WHERE id=?1 AND status='queued'",params![id,crate::storage::now()]);
        return Some(Running::Message(id));
    }
    let rows: Vec<(String,String,String)> = store.db.prepare("SELECT d.id,c.name,m.text FROM bot_shared_channel_deliveries d JOIN bot_shared_channel_messages m ON m.id=d.message_id JOIN bot_shared_channels c ON c.id=d.channel_id WHERE d.session_id=?1 AND d.status='queued' ORDER BY d.created_at,d.id").ok()
        .and_then(|mut statement| statement.query_map([session_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).ok().and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>().ok())).unwrap_or_default();
    let id = rows.into_iter().find_map(|(id,name,body)| prompt_matches(text,&format!("Work requested in #{name}:"),&body).then_some(id))?;
    store.db.execute("UPDATE bot_shared_channel_deliveries SET status='in_progress',updated_at=?2 WHERE id=?1 AND status='queued'",params![id,crate::storage::now()]).ok()?;
    let message_id: Option<String> = store.db.query_row("SELECT message_id FROM bot_shared_channel_deliveries WHERE id=?1",[&id],|r|r.get(0)).ok();
    if let Some(message_id) = message_id { let _ = update_message_status(store,&message_id); }
    Some(Running::Delivery(id))
}

/// Whether a channel delivery was stopped, e.g. between its queue entry being taken and its turn starting.
pub(crate) fn delivery_stopped(store: &Store, request: &Running) -> bool {
    let Running::Delivery(id) = request else { return false };
    store.db.query_row("SELECT status='stopped' FROM bot_shared_channel_deliveries WHERE id=?1",[id],|r|r.get(0)).unwrap_or(false)
}

#[tauri::command]
pub async fn bot_shared_channel_stop(app:tauri::AppHandle,channel_id:String) -> Result<()> {
    stop_pending(&app,&channel_id,None).await?;
    let _=app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel_id}));
    Ok(())
}

async fn continue_message(app:&tauri::AppHandle,state:&tauri::State<'_,AppState>,channel_id:&str,message_id:&str,requested_bot:Option<&str>) -> Result<()> {
    let (channel,text,bot_id)={
        let store=state.store.lock().map_err(|e|e.to_string())?;
        let channel=shared_channel(&store,channel_id)?;
        if channel.archived { return Err("This channel is deleted.".into()); }
        let (text,status):(String,String)=store.db.query_row("SELECT text,status FROM bot_shared_channel_messages WHERE id=?1 AND channel_id=?2 AND kind='user'",params![message_id,channel_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?.ok_or("This channel message no longer exists.")?;
        if status!="stopped" && status!="failed" { return Err("This message is no longer stopped.".into()); }
        let bot_id=requested_bot.map(str::to_owned).or(channel.lead_bot_id.clone()).or_else(||channel.member_ids.first().cloned()).ok_or("Add a bot in channel settings to continue.")?;
        if !channel.member_ids.contains(&bot_id) { return Err("Choose a bot that belongs to this channel.".into()); }
        store.db.execute("UPDATE bot_shared_channel_messages SET status='in_progress' WHERE id=?1",[message_id]).map_err(|e|e.to_string())?;
        (channel,text,bot_id)
    };
    if let Err(error)=send_channel_assignment(app,channel_id,message_id,&bot_id,&text,crate::storage::now(),0).await {
        // Recompute, so a retry that failed before its delivery existed keeps the post recoverable.
        if let Ok(store)=state.store.lock() { let _=update_message_status(&store,message_id); }
        let _=app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel_id}));
        return Err(error);
    }
    update_message_status(&*state.store.lock().map_err(|e|e.to_string())?,message_id)?;
    let _=app.emit("bot-shared-channel-changed",serde_json::json!({"channelId":channel.id}));
    Ok(())
}

#[tauri::command]
pub async fn bot_shared_channel_continue(app:tauri::AppHandle,state:State<'_,AppState>,channel_id:String,message_id:String) -> Result<()> {
    continue_message(&app,&state,&channel_id,&message_id,None).await
}

#[tauri::command]
pub async fn bot_shared_channel_reassign(app:tauri::AppHandle,state:State<'_,AppState>,channel_id:String,message_id:String,bot_id:String) -> Result<()> {
    continue_message(&app,&state,&channel_id,&message_id,Some(&bot_id)).await
}

#[tauri::command]
pub async fn bot_message_markers(state:State<'_,AppState>,session_id:String) -> Result<Vec<BotMessageMarker>> {
    let store = state.store.lock().map_err(|e|e.to_string())?;
    markers(&store,&session_id)
}
