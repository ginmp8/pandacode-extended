//! Team-scoped, read-only assignment activity for the persistent Orchestrations workspace.
//! Activity is not a provider completion, a Council verdict, or a tool grant.

use super::{configuration, require_main_window};
use crate::{storage::Store, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemberActivityCursor {
    pub created_at: i64,
    pub assignment_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberActivityItem {
    pub assignment_id: String,
    pub ticket_id: String,
    pub orchestration_id: String,
    pub bot_id: String,
    pub session_id: String,
    pub assignment_state: String,
    pub ticket_state: String,
    pub role: String,
    pub attempt: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub has_result: bool,
    pub chat_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberActivityPage {
    pub orchestration_id: String,
    pub bot_id: String,
    pub items: Vec<MemberActivityItem>,
    /// Counts cover this bot's entire Team history, not just the loaded page.
    /// Demand counts are distinct Ticket identities across retry attempts.
    pub assignment_count: i64,
    pub demand_count: i64,
    pub attention_demand_count: i64,
    pub has_more: bool,
    pub next_cursor: Option<MemberActivityCursor>,
}

/// Query by BOTH immutable Ticket Team and Assignment bot identity before
/// limiting. A bot reused across Teams cannot expose foreign Ticket history.
/// Missing/removed sessions remain visible as history, but never as open chats.
pub(super) fn page(
    store: &Store,
    orchestration_id: &str,
    bot_id: &str,
    before: Option<MemberActivityCursor>,
    limit: i64,
) -> Result<MemberActivityPage> {
    page_for_demand(store, orchestration_id, bot_id, before, limit, None)
}

/// Read-only native Demand filter applied before keyset pagination.
pub(super) fn page_for_demand(
    store: &Store, orchestration_id: &str, bot_id: &str,
    before: Option<MemberActivityCursor>, limit: i64, demand_id: Option<&str>,
) -> Result<MemberActivityPage> {
    if !(1..=100).contains(&limit) {
        return Err("Team member activity page size must be between 1 and 100.".into());
    }
    if bot_id.trim().is_empty() || bot_id.len() > 128 || bot_id.chars().any(char::is_control) {
        return Err("A valid existing or historical bot identity is required.".into());
    }
    if let Some(cursor) = &before {
        if cursor.created_at <= 0 || cursor.assignment_id.is_empty()
            || cursor.assignment_id.len() > 128
            || !cursor.assignment_id.bytes().all(|b| b.is_ascii_alphanumeric()
                || b == b'-' || b == b'_')
        {
            return Err("Invalid Team member activity cursor.".into());
        }
    }
    configuration(store, orchestration_id)?;
    if let Some(demand_id) = demand_id {
        if demand_id.is_empty() || demand_id.len() > 128 ||
            !demand_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Invalid Team member Demand filter.".into());
        }
        let owned: bool = store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_assignments a
              JOIN sa_tickets t ON t.id=a.ticket_id
              WHERE t.id=?1 AND t.orchestration_id=?2 AND a.bot_id=?3)",
            params![demand_id, orchestration_id, bot_id], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if !owned { return Err("Demand filter does not belong to this Team and bot.".into()); }
    }
    let (assignment_count, demand_count, attention_demand_count): (i64, i64, i64) =
        store.db.query_row(
            "SELECT COUNT(*), COUNT(DISTINCT t.id),
                    COUNT(DISTINCT CASE WHEN t.state IN
                        ('blocked','failed','interrupted','inconclusive')
                        THEN t.id END)
             FROM sa_assignments a JOIN sa_tickets t ON t.id=a.ticket_id
             WHERE t.orchestration_id=?1 AND a.bot_id=?2
               AND (?3 IS NULL OR t.id=?3)",
            params![orchestration_id, bot_id, demand_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).map_err(|e| e.to_string())?;
    if let Some(cursor) = &before {
        // A syntactically valid keyset boundary is not authority to skip
        // arbitrary history. Tie it to the exact Team, bot and immutable
        // assignment identity, including after a process restart.
        let owned: bool = store.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sa_assignments a
             JOIN sa_tickets t ON t.id=a.ticket_id
             WHERE t.orchestration_id=?1 AND a.bot_id=?2
               AND a.id=?3 AND a.created_at=?4
               AND (?5 IS NULL OR t.id=?5))",
            params![orchestration_id, bot_id, &cursor.assignment_id, cursor.created_at, demand_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if !owned {
            return Err("Team member activity cursor does not belong to this Team and bot.".into());
        }
    }
    let cursor_at = before.as_ref().map(|cursor| cursor.created_at);
    let cursor_id = before.as_ref().map(|cursor| cursor.assignment_id.as_str());
    let mut stmt = store.db.prepare(
        "SELECT a.id,t.id,t.orchestration_id,a.bot_id,a.session_id,
                a.state,t.state,a.attempt,a.created_at,a.updated_at,
                length(trim(COALESCE(a.result,'')))>0,
                CASE WHEN (a.attempt=1 AND EXISTS(
                  SELECT 1 FROM sa_audit initial
                  WHERE initial.ticket_id=t.id AND initial.kind='admitted'
                    AND initial.detail='manual readonly coordinator reserved'
                )) OR (a.attempt=2 AND EXISTS(
                  SELECT 1 FROM sa_audit retry
                  WHERE retry.ticket_id=t.id AND retry.kind='coordinator_retry_admitted'
                    AND retry.detail=a.id
                )) THEN 'coordinator' ELSE 'worker' END
         FROM sa_assignments a JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE t.orchestration_id=?1 AND a.bot_id=?2
           AND (?3 IS NULL OR t.id=?3)
           AND (?4 IS NULL OR a.created_at<?4 OR
                (a.created_at=?4 AND a.id<?5))
         ORDER BY a.created_at DESC,a.id DESC LIMIT ?6"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(
        params![orchestration_id, bot_id, demand_id, cursor_at, cursor_id, limit + 1],
        |row| Ok(MemberActivityItem {
            assignment_id: row.get(0)?,
            ticket_id: row.get(1)?,
            orchestration_id: row.get(2)?,
            bot_id: row.get(3)?,
            session_id: row.get(4)?,
            assignment_state: row.get(5)?,
            ticket_state: row.get(6)?,
            attempt: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            has_result: row.get::<_, i64>(10)? != 0,
            chat_available: false,
            role: row.get(11)?,
        }),
    ).map_err(|e| e.to_string())?;
    let mut items = rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let has_more = items.len() > limit as usize;
    items.truncate(limit as usize);
    // Reuse the exact native Ticket/attempt/thread identity check applied at
    // provider admission. A session merely linked to this bot is not enough
    // to offer historical chat navigation.
    for item in &mut items {
        item.chat_available =
            super::assignment_thread_current(store, &item.session_id, &item.ticket_id)?;
    }
    let next_cursor = if has_more {
        items.last().map(|item| MemberActivityCursor {
            created_at: item.created_at,
            assignment_id: item.assignment_id.clone(),
        })
    } else {
        None
    };
    Ok(MemberActivityPage {
        orchestration_id: orchestration_id.into(),
        bot_id: bot_id.into(),
        items,
        assignment_count,
        demand_count,
        attention_demand_count,
        has_more,
        next_cursor,
    })
}


/// A provider log may be reported for a native Assignment only while its
/// original thread, frozen provider and exclusively owned harness IDs agree.
/// Unknown is deliberately distinct from an ordinary, non-SuperAgent session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AssignmentUsageScope {
    Ordinary,
    Unknown,
    Bound {
        assignment_id: String,
        ticket_id: String,
        agent: crate::usage_ledger::Agent,
        links: Vec<String>,
    },
}

pub(crate) fn assignment_usage_scope(
    store: &Store, session_id: &str,
) -> Result<AssignmentUsageScope> {
    use crate::usage_ledger::Agent;
    let assignment: Option<(String, String, String, Option<String>)> = store.db.query_row(
        "SELECT a.id,a.ticket_id,a.bot_id,s.provider
         FROM sa_assignments a LEFT JOIN sessions s ON s.id=a.session_id
         WHERE a.session_id=?1",
        [session_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).optional().map_err(|e| e.to_string())?;
    let Some((assignment_id, ticket_id, bot_id, provider)) = assignment else {
        return Ok(AssignmentUsageScope::Ordinary);
    };
    if !super::assignment_thread_current(store, session_id, &ticket_id)? {
        return Ok(AssignmentUsageScope::Unknown);
    }
    let Some(profile) = super::roster::ticket_profile(store, &ticket_id)? else {
        return Ok(AssignmentUsageScope::Unknown);
    };
    let Some(member) = profile.members.iter().find(|member| member.bot_id == bot_id) else {
        return Ok(AssignmentUsageScope::Unknown);
    };
    if provider.as_deref() != Some(member.provider.as_str()) {
        return Ok(AssignmentUsageScope::Unknown);
    }
    let agent = match member.provider.as_str() {
        "claude" => Agent::Claude,
        "codex" => Agent::Codex,
        _ => return Ok(AssignmentUsageScope::Unknown),
    };
    let links = store.usage_links(session_id)?;
    if links.is_empty() {
        return Ok(AssignmentUsageScope::Unknown);
    }
    for link in &links {
        // A short suffix or differently formatted UUID is not sufficient log
        // identity. Both supported native harnesses use canonical UUIDs.
        if !uuid::Uuid::parse_str(link).is_ok_and(|id| id.to_string() == *link) {
            return Ok(AssignmentUsageScope::Unknown);
        }
        // Ordinary usage supports imported/forked resume IDs without a
        // durable link. A native Assignment must have the recorded link, as
        // required by the prepared provider admission boundary.
        let owned: bool = store.db.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM task_usage_links
               WHERE session_id=?1 AND harness_id=?2
             ) AND NOT EXISTS(
               SELECT 1 FROM task_usage_links
               WHERE session_id<>?1 AND lower(harness_id)=lower(?2)
             ) AND NOT EXISTS(
               SELECT 1 FROM sessions
               WHERE id<>?1 AND lower(resume_id)=lower(?2)
             )",
            params![session_id, link], |r| r.get(0),
        ).map_err(|e| e.to_string())?;
        if !owned {
            return Ok(AssignmentUsageScope::Unknown);
        }
    }
    Ok(AssignmentUsageScope::Bound { assignment_id, ticket_id, agent, links })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentChatLink {
    pub orchestration_id: String,
    pub ticket_id: String,
    pub assignment_id: String,
    pub bot_id: String,
    pub session_id: String,
}

/// Resolve the actual native thread immediately before opening a Team chat.
/// Historical activity and a valid session ID alone cannot authorize
/// navigation to an unrelated bot conversation.
fn assignment_chat_link(
    store: &Store,
    orchestration_id: &str,
    ticket_id: &str,
    assignment_id: &str,
    bot_id: &str,
) -> Result<AssignmentChatLink> {
    configuration(store, orchestration_id)?;
    let session_id: Option<String> = store.db.query_row(
        "SELECT a.session_id FROM sa_assignments a
         JOIN sa_tickets t ON t.id=a.ticket_id
         WHERE a.id=?1 AND a.ticket_id=?2 AND a.bot_id=?3
           AND t.orchestration_id=?4",
        params![assignment_id, ticket_id, bot_id, orchestration_id],
        |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let session_id = session_id.ok_or(
        "Native Team Demand/Assignment identity changed; refresh activity.",
    )?;
    if !super::assignment_thread_current(store, &session_id, ticket_id)? {
        return Err(
            "Native assignment chat thread is missing or no longer task-bound; refresh activity."
                .into(),
        );
    }
    Ok(AssignmentChatLink {
        orchestration_id: orchestration_id.to_owned(),
        ticket_id: ticket_id.to_owned(),
        assignment_id: assignment_id.to_owned(),
        bot_id: bot_id.to_owned(),
        session_id,
    })
}

#[tauri::command]
pub async fn superagent_member_assignment_chat_link(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    orchestration_id: String,
    ticket_id: String,
    assignment_id: String,
    bot_id: String,
) -> Result<AssignmentChatLink> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    assignment_chat_link(
        &store, &orchestration_id, &ticket_id, &assignment_id, &bot_id,
    )
}

#[tauri::command]
pub async fn superagent_member_activity_page(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    orchestration_id: String,
    bot_id: String,
    before: Option<MemberActivityCursor>,
    limit: Option<i64>,
    demand_id: Option<String>,
) -> Result<MemberActivityPage> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    page_for_demand(&store, &orchestration_id, &bot_id, before,
        limit.unwrap_or(25), demand_id.as_deref())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;
    use crate::superagent::{self, NewOrchestration};

    #[test]
    fn activity_cursor_serializes_and_rejects_cross_team_or_forged_boundaries() {
        let path = std::env::temp_dir()
            .join(format!("sa-member-activity-{}.db", storage::id()));
        let mut store = Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('p','Disposable','/tmp/sa-member-activity')",
            [],
        ).unwrap();
        for id in ["coordinator", "worker", "other-coordinator", "other-worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'p','codex','default','plan',?2,?2)",
                params![id,storage::now()],
            ).unwrap();
        }
        let team = superagent::create_configuration(&store, NewOrchestration {
            name: "Activity Team".into(),
            coordinator_bot_id: "coordinator".into(),
            worker_bot_id: "worker".into(),
        }).unwrap();
        let other = superagent::create_configuration(&store, NewOrchestration {
            name: "Other Team".into(),
            coordinator_bot_id: "other-coordinator".into(),
            worker_bot_id: "other-worker".into(),
        }).unwrap();
        store.db.execute(
            "UPDATE sa_orchestrations
             SET enabled=1,policy_revision=policy_revision+1 WHERE id=?1",
            [&team.id],
        ).unwrap();
        let (ticket, launched) = superagent::admit(
            &mut store, &team.id, "Private authorized work", "activity-cursor", false,
        ).unwrap();
        assert!(launched);
        let activity = page(&store, &team.id, "worker", None, 25).unwrap();
        assert_eq!(activity.items.len(), 1);
        let selected = page_for_demand(
            &store, &team.id, "worker", None, 25, Some(&ticket.id),
        ).unwrap();
        assert_eq!(selected.assignment_count, 1);
        assert_eq!(selected.demand_count, 1);
        assert_eq!(selected.items[0].ticket_id, ticket.id);
        assert!(page_for_demand(&store, &other.id, "other-worker", None, 25,
            Some(&ticket.id)).is_err(), "Cross-Team Demand filter must fail closed");
        assert!(page_for_demand(&store, &team.id, "coordinator", None, 25,
            Some(&ticket.id)).is_err(), "Another bot may not borrow Demand activity");
        assert!(page_for_demand(&store, &team.id, "worker", None, 25,
            Some("invalid/foreign")).is_err(), "Malformed filter must fail closed");
        assert_eq!(activity.assignment_count, 1);
        assert_eq!(activity.demand_count, 1);
        assert_eq!(activity.attention_demand_count, 0);
        assert_eq!(page(&store, &other.id, "other-worker", None, 25)
            .unwrap().assignment_count, 0, "Team totals must not borrow another Team's assignments");
        assert_eq!(activity.items[0].ticket_id, ticket.id);
        let item = &activity.items[0];
        let cursor = MemberActivityCursor {
            created_at: item.created_at,
            assignment_id: item.assignment_id.clone(),
        };
        let value = serde_json::to_value(&cursor).unwrap();
        assert_eq!(value["createdAt"], item.created_at);
        assert_eq!(value["assignmentId"], item.assignment_id);
        let roundtrip: MemberActivityCursor = serde_json::from_value(value).unwrap();
        assert_eq!(roundtrip.created_at, cursor.created_at);
        assert_eq!(roundtrip.assignment_id, cursor.assignment_id);
        assert!(page(&store, &team.id, "worker", Some(cursor.clone()), 25)
            .unwrap().items.is_empty());
        assert!(page_for_demand(
            &store, &team.id, "worker", Some(cursor.clone()), 25, Some(&ticket.id),
        ).unwrap().items.is_empty(), "Filtered cursor must preserve Demand scope");
        assert!(page(&store, &team.id, "coordinator", Some(cursor.clone()), 25)
            .is_err(), "A different bot cannot borrow a real assignment cursor");
        assert!(page(&store, &other.id, "other-worker", Some(cursor.clone()), 25)
            .is_err(), "A different Team cannot borrow a real assignment cursor");
        assert!(page(&store, &team.id, "worker", Some(MemberActivityCursor {
            created_at: cursor.created_at,
            assignment_id: "forged-assignment".into(),
        }), 25).is_err(), "A well-formed fabricated cursor cannot skip native history");
        store.db.execute(
            "UPDATE sa_tickets SET state='blocked' WHERE id=?1", [&ticket.id],
        ).unwrap();
        let updated = page(&store, &team.id, "worker", None, 25).unwrap();
        assert_eq!(updated.assignment_count, 1);
        assert_eq!(updated.demand_count, 1);
        assert_eq!(updated.attention_demand_count, 1,
            "Attention reflects native Ticket state, not presence of a provider result");
        let stored_path = store.path.clone();
        drop(store);
        let reopened = Store::open(&stored_path).unwrap();
        let audit_before: i64 = reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get(0),
        ).unwrap();
        let headless: serde_json::Value = serde_json::from_str(
            &superagent::list_headless_team_member_activity(
                &stored_path, &team.id, "worker", None, None, None,
            ).unwrap(),
        ).unwrap();
        assert_eq!(headless["pageLimit"], 100);
        assert_eq!(headless["observationalOnly"], true);
        assert_eq!(headless["recoveryPerformed"], false);
        assert_eq!(headless["providerDispatched"], false);
        assert_eq!(headless["items"][0]["ticketId"], ticket.id);
        assert!(!headless.to_string().contains("Private authorized work"));
        assert_eq!(headless["assignmentCount"], 1);
        assert_eq!(headless["demandCount"], 1);
        assert_eq!(headless["attentionDemandCount"], 1);
        let filtered: serde_json::Value = serde_json::from_str(
            &superagent::list_headless_team_member_activity(
                &stored_path, &team.id, "worker", None, None, Some(&ticket.id),
            ).unwrap(),
        ).unwrap();
        assert_eq!(filtered["demandId"], ticket.id);
        assert_eq!(filtered["items"], headless["items"]);
        assert_eq!(filtered["assignmentCount"], 1);
        let filtered_next: serde_json::Value = serde_json::from_str(
            &superagent::list_headless_team_member_activity(
                &stored_path, &team.id, "worker", Some(cursor.created_at),
                Some(&cursor.assignment_id), Some(&ticket.id),
            ).unwrap(),
        ).unwrap();
        assert!(filtered_next["items"].as_array().unwrap().is_empty());
        assert_eq!(filtered_next["assignmentCount"], 1);
        assert!(superagent::list_headless_team_member_activity(
            &stored_path, &other.id, "other-worker", None, None, Some(&ticket.id),
        ).is_err());
        assert!(superagent::list_headless_team_member_activity(
            &stored_path, &team.id, "coordinator", None, None, Some(&ticket.id),
        ).is_err());
        assert!(superagent::list_headless_team_member_activity(
            &stored_path, &team.id, "worker", None, None, Some("unknown-ticket"),
        ).is_err());
        let next: serde_json::Value = serde_json::from_str(
            &superagent::list_headless_team_member_activity(
                &stored_path, &team.id, "worker",
                Some(cursor.created_at), Some(&cursor.assignment_id), None,
            ).unwrap(),
        ).unwrap();
        assert!(next["items"].as_array().unwrap().is_empty());
        assert!(superagent::list_headless_team_member_activity(
            &stored_path, &other.id, "other-worker",
            Some(cursor.created_at), Some(&cursor.assignment_id), None,
        ).is_err());
        assert!(superagent::list_headless_team_member_activity(
            &stored_path, &team.id, "worker", Some(cursor.created_at), None, None,
        ).is_err());
        let audit_after: i64 = reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(audit_before, audit_after, "headless reads cannot append audit");
        let history = page(&reopened, &team.id, "worker", Some(cursor.clone()), 25)
            .unwrap();
        assert!(history.items.is_empty());
        assert_eq!(history.assignment_count, 1,
            "Counts must cover all native history even after the final cursor");
        assert_eq!(history.demand_count, 1);
        assert_eq!(history.attention_demand_count, 1,
            "Native attention totals must survive reopening SQLite");
        assert!(page(&reopened, &other.id, "other-worker", Some(cursor), 25)
            .is_err(), "Isolation must survive SQLite reopening");
    }

    #[test]
    fn assignment_chat_navigation_rechecks_original_thread_and_surviving_session() {
        let path = std::env::temp_dir()
            .join(format!("sa-chat-link-{}.db", storage::id()));
        let mut store = Store::open(&path).unwrap();
        store.db.execute(
            "INSERT INTO projects(id,name,path) VALUES('chat-project','Disposable','/tmp/sa-chat-link')",
            [],
        ).unwrap();
        for bot in ["chat-lead", "chat-worker"] {
            store.db.execute(
                "INSERT INTO bots(id,name,project_id,provider,model,mode,created_at,updated_at)
                 VALUES(?1,?1,'chat-project','codex','default','plan',?2,?2)",
                params![bot, storage::now()],
            ).unwrap();
        }
        let create = |name: &str, store: &Store| {
            superagent::create_configuration(store, NewOrchestration {
                name: name.into(),
                coordinator_bot_id: "chat-lead".into(),
                worker_bot_id: "chat-worker".into(),
            }).unwrap()
        };
        let team = create("Chat Team", &store);
        let foreign = create("Other Chat Team", &store);
        store.db.execute(
            "UPDATE sa_orchestrations SET enabled=1,policy_revision=policy_revision+1
             WHERE id=?1", [&team.id],
        ).unwrap();
        let (ticket, launched) = superagent::admit(
            &mut store, &team.id, "Task-private chat", "chat-link", false,
        ).unwrap();
        assert!(launched);
        let activity = page(&store, &team.id, "chat-worker", None, 25).unwrap();
        let item = &activity.items[0];
        assert!(item.chat_available, "The original native thread is navigable");
        let link = assignment_chat_link(
            &store, &team.id, &ticket.id, &item.assignment_id, "chat-worker",
        ).unwrap();
        assert_eq!(link.session_id, item.session_id);
        assert_eq!(link.ticket_id, ticket.id);
        assert!(assignment_chat_link(
            &store, &foreign.id, &ticket.id, &item.assignment_id, "chat-worker",
        ).is_err(), "A shared bot cannot cross Team boundaries");
        assert!(assignment_chat_link(
            &store, &team.id, &ticket.id, &item.assignment_id, "chat-lead",
        ).is_err());
        assert!(assignment_chat_link(
            &store, &team.id, "wrong-ticket", &item.assignment_id, "chat-worker",
        ).is_err());
        let assignment_id = item.assignment_id.clone();
        let session_id = item.session_id.clone();
        let harness_id = "aaaa1111-b222-4c33-8d44-555566667777";
        assert_eq!(assignment_usage_scope(&store, &session_id).unwrap(),
            AssignmentUsageScope::Unknown, "No provider logs are not zero usage");
        store.db.execute(
            "UPDATE sessions SET resume_id=?2 WHERE id=?1",
            params![&session_id, harness_id],
        ).unwrap();
        assert_eq!(assignment_usage_scope(&store, &session_id).unwrap(),
            AssignmentUsageScope::Unknown, "An unrecorded resume ID cannot attribute logs");
        store.resume(&session_id, harness_id).unwrap();
        let expected_scope = AssignmentUsageScope::Bound {
            assignment_id: assignment_id.clone(),
            ticket_id: ticket.id.clone(),
            agent: crate::usage_ledger::Agent::Codex,
            links: vec![harness_id.into()],
        };
        assert_eq!(assignment_usage_scope(&store, &session_id).unwrap(), expected_scope);
        store.db.execute(
            "INSERT INTO sessions(id,project_id,title,provider,model,mode,status,created_at,updated_at,resume_id)
             VALUES('ordinary-usage','chat-project','Ordinary','codex','default','plan','idle',0,0,?1)",
            [harness_id.to_uppercase()],
        ).unwrap();
        assert_eq!(assignment_usage_scope(&store, "ordinary-usage").unwrap(),
            AssignmentUsageScope::Ordinary);
        assert_eq!(assignment_usage_scope(&store, &session_id).unwrap(),
            AssignmentUsageScope::Unknown, "A shared current resume ID is ambiguous");
        store.db.execute(
            "UPDATE sessions SET resume_id=NULL WHERE id='ordinary-usage'", [],
        ).unwrap();
        store.db.execute(
            "INSERT INTO task_usage_links(session_id,harness_id,created_at)
             VALUES('ordinary-usage',?1,0)", [harness_id],
        ).unwrap();
        assert_eq!(assignment_usage_scope(&store, &session_id).unwrap(),
            AssignmentUsageScope::Unknown, "A shared historical ID is ambiguous");
        store.db.execute("DELETE FROM sessions WHERE id='ordinary-usage'", []).unwrap();
        assert_eq!(assignment_usage_scope(&store, &session_id).unwrap(), expected_scope);
        store.db.execute("UPDATE sessions SET provider='claude' WHERE id=?1", [&session_id]).unwrap();
        assert_eq!(assignment_usage_scope(&store, &session_id).unwrap(),
            AssignmentUsageScope::Unknown, "Mutable provider cannot replace the frozen one");
        store.db.execute("UPDATE sessions SET provider='codex' WHERE id=?1", [&session_id]).unwrap();
        store.db.execute(
            "UPDATE bot_threads SET external_id='unrelated-ticket' WHERE session_id=?1",
            [&session_id],
        ).unwrap();
        assert_eq!(assignment_usage_scope(&store, &session_id).unwrap(),
            AssignmentUsageScope::Unknown, "A remapped native thread cannot report usage");
        assert!(!page(&store, &team.id, "chat-worker", None, 25).unwrap()
            .items[0].chat_available);
        assert!(assignment_chat_link(
            &store, &team.id, &ticket.id, &assignment_id, "chat-worker",
        ).is_err(), "A remapped thread cannot open another conversation");
        store.db.execute(
            "UPDATE bot_threads SET external_id=?2 WHERE session_id=?1",
            params![&session_id, format!("{}:attempt:1", ticket.id)],
        ).unwrap();
        assert!(assignment_chat_link(
            &store, &team.id, &ticket.id, &assignment_id, "chat-worker",
        ).is_err(), "A fabricated retry alias is not native authority");
        store.db.execute(
            "UPDATE bot_threads SET external_id=?2 WHERE session_id=?1",
            params![&session_id, &ticket.id],
        ).unwrap();
        assert!(assignment_chat_link(
            &store, &team.id, &ticket.id, &assignment_id, "chat-worker",
        ).is_ok());
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert!(assignment_chat_link(
            &reopened, &team.id, &ticket.id, &assignment_id, "chat-worker",
        ).is_ok(), "Historical native identity survives restart");
        assert_eq!(assignment_usage_scope(&reopened, &session_id).unwrap(), expected_scope);
        let audit_before: i64 = reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get(0),
        ).unwrap();
        reopened.db.execute("DELETE FROM sessions WHERE id=?1", [&session_id]).unwrap();
        assert!(assignment_chat_link(
            &reopened, &team.id, &ticket.id, &assignment_id, "chat-worker",
        ).is_err(), "A removed session is not a navigable historical chat");
        assert_eq!(assignment_usage_scope(&reopened, &session_id).unwrap(),
            AssignmentUsageScope::Unknown);
        assert!(!page(&reopened, &team.id, "chat-worker", None, 25).unwrap()
            .items[0].chat_available);
        let audit_after: i64 = reopened.db.query_row(
            "SELECT COUNT(*) FROM sa_audit", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(audit_before, audit_after, "Chat inspection is read-only");
    }
}
