//! Bot schedules and check-ins. Times are computed in the computer's local time zone
//! with plain arithmetic on a fixed UTC offset sampled at scheduling time, so a run
//! lands at the wall-clock time the user chose; the next run is recomputed after
//! every run, which absorbs daylight-saving changes. Overdue runs after a restart
//! run once, not once per missed slot.

use super::{gateway, Bot};
use crate::{storage::Store, AppState, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{Emitter, Manager, State};

const MINUTE: i64 = 60_000;
const DAY: i64 = 24 * 60 * MINUTE;

/// Authorization belongs to the schedule, not to the bounded activity log.
/// An accepted occurrence and its saved initiating turn are committed together.
pub(crate) fn migrate(db: &rusqlite::Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS bot_schedule_authorizations(
        schedule_id TEXT PRIMARY KEY REFERENCES bot_schedules(id) ON DELETE CASCADE,
        permission_mode TEXT NOT NULL CHECK(permission_mode IN ('plan','ask','auto','review','full')));
        CREATE TABLE IF NOT EXISTS bot_scheduled_runs(
        bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
        occurrence TEXT NOT NULL,
        session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        message_id INTEGER NOT NULL UNIQUE REFERENCES messages(id) ON DELETE CASCADE,
        permission_mode TEXT NOT NULL CHECK(permission_mode IN ('plan','ask','auto','review','full')),
        accepted_at INTEGER NOT NULL,
        PRIMARY KEY(bot_id,occurrence));
        INSERT OR IGNORE INTO bot_schedule_authorizations(schedule_id,permission_mode)
        SELECT schedule.id,CASE WHEN schedule.created_by<>'user' THEN 'plan' ELSE
          COALESCE((SELECT CASE json_extract(CASE WHEN json_valid(activity.detail) THEN activity.detail ELSE '{}' END,'$.permissionMode')
            WHEN 'plan' THEN 'plan' WHEN 'ask' THEN 'ask' WHEN 'auto' THEN 'auto'
            WHEN 'review' THEN 'review' WHEN 'full' THEN 'full' ELSE 'plan' END
            FROM bot_activity activity WHERE activity.bot_id=schedule.bot_id AND activity.kind='schedule'
              AND json_extract(CASE WHEN json_valid(activity.detail) THEN activity.detail ELSE '{}' END,'$.scheduleId')=schedule.id ORDER BY activity.id LIMIT 1),'plan')
          END FROM bot_schedules schedule;")
        .map_err(|error| error.to_string())
}

/// A single scheduled occurrence or explicit Run Now request. Current bot policy
/// is rechecked at native admission; none of these values grants permission.
#[derive(Clone, Debug)]
pub(crate) struct Admission {
    bot_id: String,
    occurrence: Occurrence,
}

#[derive(Clone, Debug)]
enum Occurrence {
    Schedule { id: String, at: i64 },
    Manual { id: String, request_id: String },
    Heartbeat { at: i64 },
}

impl Admission {
    pub(crate) fn schedule(schedule: &Schedule, manual: bool) -> Result<Self> {
        Ok(Self {
            bot_id: schedule.bot_id.clone(),
            occurrence: if manual {
                // Run Now is a separate, intentional request. It neither consumes
                // the next timer occurrence nor reenables a completed one-off.
                Occurrence::Manual {
                    id: schedule.id.clone(),
                    request_id: crate::storage::id(),
                }
            } else {
                Occurrence::Schedule {
                    id: schedule.id.clone(),
                    at: schedule
                        .next_run_at
                        .ok_or("This scheduled occurrence was already accepted.")?,
                }
            },
        })
    }

    pub(crate) fn heartbeat(bot: &Bot) -> Result<Self> {
        Ok(Self {
            bot_id: bot.id.clone(),
            occurrence: Occurrence::Heartbeat {
                at: bot
                    .next_heartbeat_at
                    .ok_or("This check-in is no longer scheduled.")?,
            },
        })
    }

    fn occurrence(&self) -> String {
        match &self.occurrence {
            Occurrence::Schedule { id, at } => format!("schedule:{id}:{at}"),
            Occurrence::Manual { id, request_id } => format!("manual:{id}:{request_id}"),
            Occurrence::Heartbeat { at } => format!("heartbeat:{at}"),
        }
    }
}

pub(crate) struct Work {
    pub bot: Bot,
    pub label: String,
    pub prompt: String,
    pub mode: &'static str,
}

pub(crate) fn resolve(store: &Store, admission: &Admission, now: i64) -> Result<Work> {
    let bot = super::get(store, &admission.bot_id)?;
    if bot.paused {
        return Err("This bot is paused.".into());
    }
    let (label, prompt, mode) = match &admission.occurrence {
        Occurrence::Schedule { id, .. } | Occurrence::Manual { id, .. } => {
            let schedule = get(store, id)?;
            if schedule.bot_id != bot.id {
                return Err("This scheduled occurrence does not belong to this bot.".into());
            }
            let label = if let Occurrence::Schedule { at, .. } = &admission.occurrence {
                if schedule.next_run_at != Some(*at) {
                    return Err("This scheduled occurrence was changed or already accepted.".into());
                }
                if !schedule.enabled || *at > now {
                    return Err("This scheduled occurrence is no longer due.".into());
                }
                "Scheduled"
            } else {
                "Run now"
            };
            let mode = execution_mode(store, &schedule, &bot.mode)?;
            (
                format!("{label}: {}", schedule.title),
                schedule.prompt,
                mode,
            )
        }
        Occurrence::Heartbeat { at } => {
            if bot.heartbeat_minutes <= 0 || bot.next_heartbeat_at != Some(*at) || *at > now {
                return Err("This check-in is no longer due.".into());
            }
            ("Check-in".into(), HEARTBEAT_PROMPT.into(), "plan")
        }
    };
    let accepted: bool = store
        .db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM bot_scheduled_runs WHERE bot_id=?1 AND occurrence=?2)",
            params![bot.id, admission.occurrence()],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if accepted {
        return Err("This scheduled occurrence was already accepted.".into());
    }
    Ok(Work {
        bot,
        label,
        prompt,
        mode,
    })
}

/// Called with the same agents/store locks that capture the run configuration.
pub(crate) fn admission_work(
    store: &Store,
    session_id: &str,
    admission: &Admission,
    now: i64,
) -> Result<Work> {
    let mut work = resolve(store, admission, now)?;
    let belongs: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM bot_threads WHERE bot_id=?1 AND channel_id='schedule' AND external_id='schedule' AND session_id=?2)",
        params![work.bot.id, session_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if !belongs {
        return Err("This scheduled occurrence does not belong to this conversation.".into());
    }
    let text = format!("{}\n\n{}", work.label, work.prompt);
    work.prompt = if super::brief_version_of(store, session_id)? != work.bot.memory_version {
        let memories = super::memories(store, &work.bot.id)?;
        format!(
            "{text}{}",
            super::brief::compose(&work.bot, &memories, "schedule", "")
        )
    } else {
        text
    };
    Ok(work)
}

/// Runs inside the initiating-message transaction. A failure rolls back the
/// session claim, saved prompt, receipt and advancement as one unit. A process
/// restart after commit exposes an interrupted accepted turn; it never submits
/// the same occurrence to an external provider a second time automatically.
pub(crate) fn accept(
    store: &Store,
    session_id: &str,
    message_id: i64,
    admission: &Admission,
    now: i64,
) -> Result<()> {
    let work = admission_work(store, session_id, admission, now)?;
    let mode = work.mode;
    store.db.execute(
        "INSERT INTO bot_scheduled_runs(bot_id,occurrence,session_id,message_id,permission_mode,accepted_at) VALUES(?1,?2,?3,?4,?5,?6)",
        params![admission.bot_id, admission.occurrence(), session_id, message_id, mode, now],
    ).map_err(|error| error.to_string())?;
    let changed = match &admission.occurrence {
        Occurrence::Schedule { id, at } => {
            let schedule = get(store, id)?;
            let next = if schedule.kind == "once" {
                None
            } else {
                next_run(
                    &schedule.kind,
                    schedule.interval_minutes,
                    schedule.time_of_day.as_deref(),
                    now,
                    local_offset_ms(now),
                )
            };
            store.db.execute(
                "UPDATE bot_schedules SET last_run_at=?2,next_run_at=?3,enabled=CASE WHEN ?4 THEN 0 ELSE enabled END WHERE id=?1 AND next_run_at=?5",
                params![id, now, next, schedule.kind == "once", at],
            ).map_err(|error| error.to_string())?
        }
        Occurrence::Manual { .. } => 1,
        Occurrence::Heartbeat { at } => {
            let bot = super::get(store, &admission.bot_id)?;
            store.db.execute(
                "UPDATE bots SET next_heartbeat_at=?2 WHERE id=?1 AND paused=0 AND next_heartbeat_at=?3",
                params![bot.id, heartbeat_after(bot.heartbeat_minutes, now), at],
            ).map_err(|error| error.to_string())?
        }
    };
    if changed != 1 {
        return Err("This scheduled occurrence changed before it was accepted.".into());
    }
    store
        .db
        .execute(
            "UPDATE sessions SET mode=?2 WHERE id=?1",
            params![session_id, mode],
        )
        .map_err(|error| error.to_string())?;
    super::mark_briefed(store, session_id, work.bot.memory_version)?;
    super::log(
        store,
        &admission.bot_id,
        "scheduled",
        "Scheduled work accepted",
        json!({"sessionId":session_id,"messageId":message_id,"occurrence":admission.occurrence(),"mode":mode}),
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: String,
    pub bot_id: String,
    pub title: String,
    pub prompt: String,
    pub kind: String,
    pub interval_minutes: Option<i64>,
    pub time_of_day: Option<String>,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
    pub enabled: bool,
    pub created_by: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScheduleInput {
    pub title: String,
    pub prompt: String,
    pub kind: String,
    pub interval_minutes: Option<i64>,
    pub time_of_day: Option<String>,
    /// For `once`: when to run, in ms.
    pub run_at: Option<i64>,
}

/// The local UTC offset in ms at `at`, from the operating system's time zone.
pub(crate) fn local_offset_ms(at: i64) -> i64 {
    use chrono::{Offset, TimeZone};
    chrono::Local.timestamp_millis_opt(at).single().map_or(0, |time| i64::from(time.offset().fix().local_minus_utc()) * 1000)
}

fn parse_time(text: &str) -> Option<i64> {
    let (hours, minutes) = text.trim().split_once(':')?;
    let (hours, minutes): (i64, i64) = (hours.parse().ok()?, minutes.parse().ok()?);
    ((0..24).contains(&hours) && (0..60).contains(&minutes)).then_some((hours * 60 + minutes) * MINUTE)
}

/// Next run strictly after `after`, given a fixed local offset.
pub(crate) fn next_run(kind: &str, interval_minutes: Option<i64>, time_of_day: Option<&str>, after: i64, offset: i64) -> Option<i64> {
    match kind {
        "interval" => interval_minutes.filter(|m| (5..=7 * 24 * 60).contains(m)).map(|m| after + m * MINUTE),
        "daily" | "weekdays" => {
            let at = parse_time(time_of_day?)?;
            let local = after + offset;
            let mut day_start = local.div_euclid(DAY) * DAY;
            for _ in 0..8 {
                let candidate = day_start + at;
                // 1970-01-01 was a Thursday, so (days + 4) % 7 counts from Sunday.
                let weekday = (candidate.div_euclid(DAY) + 4).rem_euclid(7); // 0 = Sunday
                let weekend = weekday == 0 || weekday == 6;
                if candidate > local && (kind == "daily" || !weekend) {
                    return Some(candidate - offset);
                }
                day_start += DAY;
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn heartbeat_after(minutes: i64, now: i64) -> Option<i64> {
    (minutes > 0).then(|| now + minutes * MINUTE)
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Schedule> {
    Ok(Schedule {
        id: r.get(0)?,
        bot_id: r.get(1)?,
        title: r.get(2)?,
        prompt: r.get(3)?,
        kind: r.get(4)?,
        interval_minutes: r.get(5)?,
        time_of_day: r.get(6)?,
        next_run_at: r.get(7)?,
        last_run_at: r.get(8)?,
        enabled: r.get(9)?,
        created_by: r.get(10)?,
    })
}
const COLUMNS: &str = "id,bot_id,title,prompt,kind,interval_minutes,time_of_day,next_run_at,last_run_at,enabled,created_by";

pub(crate) fn list(store: &Store, bot_id: &str) -> Result<Vec<Schedule>> {
    let mut statement = store.db.prepare(&format!("SELECT {COLUMNS} FROM bot_schedules WHERE bot_id=?1 ORDER BY created_at")).map_err(|e| e.to_string())?;
    let rows = statement.query_map([bot_id], row).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
}

pub(crate) fn get(store: &Store, id: &str) -> Result<Schedule> {
    store
        .db
        .query_row(&format!("SELECT {COLUMNS} FROM bot_schedules WHERE id=?1"), [id], row)
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "This schedule no longer exists.".into())
}

pub(crate) fn create(store: &Store, bot_id: &str, input: &ScheduleInput, created_by: &str, now: i64) -> Result<Schedule> {
    let bot = super::get(store, bot_id)?;
    let title = input.title.trim().chars().take(120).collect::<String>();
    let prompt = input.prompt.trim().chars().take(8000).collect::<String>();
    if prompt.is_empty() {
        return Err("A schedule needs a prompt: what should the bot do?".into());
    }
    let count: i64 = store.db.query_row("SELECT COUNT(*) FROM bot_schedules WHERE bot_id=?1", [bot_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if count >= 50 {
        return Err("A bot can have up to 50 schedules.".into());
    }
    let next = match input.kind.as_str() {
        "once" => input.run_at.filter(|at| *at > now - MINUTE && *at < now + 366 * DAY),
        kind @ ("interval" | "daily" | "weekdays") => next_run(kind, input.interval_minutes, input.time_of_day.as_deref(), now, local_offset_ms(now)),
        _ => return Err("Choose once, every N minutes, daily, or weekdays.".into()),
    }
    .ok_or("Check the schedule's time: intervals are 5 minutes to a week, times are HH:MM, one-off runs are within a year.")?;
    let id = crate::storage::id();
    let transaction = store
        .db
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    store
        .db
        .execute(
            "INSERT INTO bot_schedules(id,bot_id,title,prompt,kind,interval_minutes,time_of_day,next_run_at,enabled,created_by,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10)",
            params![id, bot_id, if title.is_empty() { prompt.chars().take(60).collect::<String>() } else { title }, prompt, input.kind, input.interval_minutes, input.time_of_day, next, created_by, now],
        )
        .map_err(|e| e.to_string())?;
    let ceiling = if created_by == "user" {
        bot.mode.as_str()
    } else {
        "plan"
    };
    store
        .db
        .execute(
            "INSERT INTO bot_schedule_authorizations(schedule_id,permission_mode) VALUES(?1,?2)",
            params![id, ceiling],
        )
        .map_err(|error| error.to_string())?;
    let schedule = get(store, &id)?;
    super::log(
        store,
        bot_id,
        "schedule",
        &format!("Scheduled: {}", schedule.title),
        json!({"scheduleId": id, "by": created_by, "permissionMode": ceiling}),
    )?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(schedule)
}

/// Unattended work cannot acquire later bot permissions. The schedule-owned
/// authorization outlives activity retention; missing policy fails read-only.
/// Bot-created schedules, including jobs from remote-originated turns, stay read-only.
pub(crate) fn execution_mode(store: &Store, schedule: &Schedule, current: &str) -> Result<&'static str> {
    if schedule.created_by != "user" || current == "plan" {
        return Ok("plan");
    }
    let original: Option<String> = store
        .db
        .query_row(
            "SELECT permission_mode FROM bot_schedule_authorizations WHERE schedule_id=?1",
            [&schedule.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(match original.as_deref() {
        None | Some("plan") => "plan",
        Some("ask") if current == "ask" => "ask",
        Some("auto") if current == "auto" => "auto",
        Some("review") if current == "review" => "review",
        Some("full") if current == "full" => "full",
        // An upgraded bot mode cannot make an older schedule privileged.
        // Legacy user schedules without proven authorization migrate to Plan.
        _ => "ask",
    })
}

pub(crate) fn set_enabled(store: &Store, id: &str, enabled: bool, now: i64) -> Result<Schedule> {
    let schedule = get(store, id)?;
    if enabled && schedule.kind == "once" && schedule.next_run_at.is_none() {
        return Err(
            "This one-off schedule was already accepted. Create a new schedule to run it again."
                .into(),
        );
    }
    let next = if enabled && schedule.kind != "once" {
        next_run(&schedule.kind, schedule.interval_minutes, schedule.time_of_day.as_deref(), now, local_offset_ms(now))
    } else {
        schedule.next_run_at
    };
    store.db.execute("UPDATE bot_schedules SET enabled=?2,next_run_at=?3 WHERE id=?1", params![id, enabled, next]).map_err(|e| e.to_string())?;
    get(store, id)
}

pub(crate) fn delete(store: &Store, bot_id: &str, id: &str) -> Result<()> {
    let removed = store.db.execute("DELETE FROM bot_schedules WHERE id=?1 AND bot_id=?2", params![id, bot_id]).map_err(|e| e.to_string())?;
    if removed == 0 {
        return Err("That schedule was already removed.".into());
    }
    super::log(store, bot_id, "schedule", "Removed a schedule", json!({"scheduleId": id}))
}

/// Due work: schedules whose time has come and bots whose check-in is due, for unpaused bots.
pub(crate) fn due(store: &Store, now: i64) -> Result<(Vec<Schedule>, Vec<Bot>)> {
    let mut statement = store
        .db
        .prepare(&format!("SELECT {COLUMNS} FROM bot_schedules WHERE enabled=1 AND next_run_at IS NOT NULL AND next_run_at<=?1 AND bot_id IN (SELECT id FROM bots WHERE paused=0) ORDER BY next_run_at,id"))
        .map_err(|e| e.to_string())?;
    let schedules = statement.query_map([now], row).map_err(|e| e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
    let bots = super::list(store)?.into_iter().filter(|bot| !bot.paused && bot.heartbeat_minutes > 0 && bot.next_heartbeat_at.is_some_and(|at| at <= now)).collect();
    Ok((schedules, bots))
}

pub(crate) const HEARTBEAT_PROMPT: &str = "Check-in: review your standing goals and what you remember. Use read-only steps only (reading files, pages or connected data); do not send messages, change data or spend money without asking. If something needs the user's attention or a decision, say so briefly and propose the next step.";

/// Scheduler loop: every 30 seconds, start due schedules and check-ins as bot turns.
pub(crate) fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tick.tick().await;
            let now = crate::storage::now();
            let due = {
                let state = app.state::<AppState>();
                let Ok(store) = state.store.lock() else { continue };
                let Ok(due) = due(&store, now) else { continue };
                due
            };
            for schedule in due.0 {
                let label = format!("Scheduled: {}", schedule.title);
                let result = match Admission::schedule(&schedule, false) {
                    Ok(admission) => gateway::run_scheduled(&app, admission).await,
                    Err(error) => Err(error),
                };
                if let Err(error) = result {
                    super::note(
                        &app,
                        &schedule.bot_id,
                        "error",
                        &format!("{label} could not start: {error}"),
                        json!({"scheduleId": schedule.id}),
                    );
                }
            }
            for bot in due.1 {
                if let Ok(admission) = Admission::heartbeat(&bot) {
                    let _ = gateway::run_scheduled(&app, admission).await;
                }
            }
            let _ = app.emit("bots-changed", ());
        }
    });
}

#[tauri::command]
pub async fn bot_schedule_create(app: tauri::AppHandle, state: State<'_, AppState>, bot_id: String, input: ScheduleInput) -> Result<Schedule> {
    let schedule = create(&*state.store.lock().map_err(|e| e.to_string())?, &bot_id, &input, "user", crate::storage::now())?;
    let _ = app.emit("bots-changed", ());
    Ok(schedule)
}

#[tauri::command]
pub async fn bot_schedule_set_enabled(app: tauri::AppHandle, state: State<'_, AppState>, schedule_id: String, enabled: bool) -> Result<Schedule> {
    let schedule = set_enabled(&*state.store.lock().map_err(|e| e.to_string())?, &schedule_id, enabled, crate::storage::now())?;
    let _ = app.emit("bots-changed", ());
    Ok(schedule)
}

#[tauri::command]
pub async fn bot_schedule_delete(app: tauri::AppHandle, state: State<'_, AppState>, bot_id: String, schedule_id: String) -> Result<()> {
    delete(&*state.store.lock().map_err(|e| e.to_string())?, &bot_id, &schedule_id)?;
    let _ = app.emit("bots-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn bot_run_schedule_now(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    schedule_id: String,
) -> Result<()> {
    let schedule = get(
        &*state.store.lock().map_err(|e| e.to_string())?,
        &schedule_id,
    )?;
    gateway::run_scheduled(&app, Admission::schedule(&schedule, true)?).await
}
