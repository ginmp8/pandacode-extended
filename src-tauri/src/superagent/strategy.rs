//! Versioned Team resource policy. Optimization is advisory; native runtime
//! capabilities and safety floors are enforced independently of the priority.
//! A Team edit never mutates previously admitted Ticket snapshots.

use super::{effects, insert_team_version, load_team_version_snapshot, make_team_snapshot,
    require_main_window, TeamSnapshotProvenance, MAX_ASSIGNMENT_RUN_MS};
use crate::{storage::{self, Store}, AppState, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

fn default_priority() -> String { "balanced".into() }
fn default_attempt_timeout() -> i64 { MAX_ASSIGNMENT_RUN_MS }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategySettings {
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default = "default_attempt_timeout")]
    pub attempt_timeout_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_ceiling: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost_ceiling_minor: Option<u64>,
    #[serde(default)]
    pub subagent_delegation_enabled: bool,
}

impl Default for StrategySettings {
    fn default() -> Self {
        Self {
            priority: default_priority(),
            attempt_timeout_ms: MAX_ASSIGNMENT_RUN_MS,
            token_ceiling: None,
            estimated_cost_ceiling_minor: None,
            subagent_delegation_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategyUpdate {
    pub priority: String,
    pub attempt_timeout_ms: i64,
    pub max_attempts_per_assignment: u8,
    pub max_concurrent_tasks: u8,
    #[serde(default)]
    pub token_ceiling: Option<u64>,
    #[serde(default)]
    pub estimated_cost_ceiling_minor: Option<u64>,
    #[serde(default)]
    pub subagent_delegation_enabled: bool,
}

impl StrategyUpdate {
    fn validate(&self) -> Result<StrategySettings> {
        if !matches!(self.priority.as_str(),
            "balanced" | "cost" | "latency" | "throughput" | "quality") {
            return Err("Optimization priority must be balanced, cost, latency, throughput or quality.".into());
        }
        if !(1..=8).contains(&self.max_attempts_per_assignment)
            || !(1..=8).contains(&self.max_concurrent_tasks) {
            return Err("Worker attempt and concurrency limits must each be between 1 and 8.".into());
        }
        if !(60_000..=MAX_ASSIGNMENT_RUN_MS).contains(&self.attempt_timeout_ms) {
            return Err("Worker attempt timeout must be between 60 seconds and 30 minutes.".into());
        }
        if self.token_ceiling.is_some_and(|value| value == 0)
            || self.estimated_cost_ceiling_minor.is_some_and(|value| value == 0) {
            return Err("Optional resource ceilings must be positive.".into());
        }
        Ok(StrategySettings {
            priority: self.priority.clone(),
            attempt_timeout_ms: self.attempt_timeout_ms,
            token_ceiling: self.token_ceiling,
            estimated_cost_ceiling_minor: self.estimated_cost_ceiling_minor,
            subagent_delegation_enabled: self.subagent_delegation_enabled,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyView {
    pub orchestration_id: String,
    pub policy_revision: i64,
    pub team_version: i64,
    pub settings: StrategySettings,
    pub max_attempts_per_assignment: u8,
    pub max_concurrent_tasks: u8,
    pub executable_parallel_workers: u8,
    pub token_metering_available: bool,
    pub priced_cost_metering_available: bool,
    pub nested_child_admission_available: bool,
    pub enable_blockers: Vec<String>,
}

fn read(store: &Store, orchestration_id: &str) -> Result<StrategyView> {
    let cfg = super::configuration(store, orchestration_id)?;
    let snapshot = load_team_version_snapshot(&store.db, orchestration_id, cfg.team_version)?;
    if snapshot.policy.policy_revision > cfg.policy_revision
        || snapshot.policy.council_required != cfg.council_required {
        return Err("Team strategy conflicts with the authoritative versioned Team.".into());
    }
    let mut enable_blockers = Vec::new();
    if snapshot.policy.max_concurrent_tasks != 1 {
        enable_blockers.push("Parallel writes over one leased checkout are not supported; the current native scheduler serializes conflicting workers.".into());
    }
    if snapshot.policy.strategy.token_ceiling.is_some() {
        enable_blockers.push("A native provider token meter is not available for a hard token ceiling.".into());
    }
    if snapshot.policy.strategy.estimated_cost_ceiling_minor.is_some() {
        enable_blockers.push("A verified price/usage meter is not available for a hard monetary ceiling.".into());
    }
    if snapshot.policy.strategy.subagent_delegation_enabled {
        enable_blockers.push("The Team strategy alone cannot authorize descendant execution; a separate exact native delegation policy is required.".into());
    }
    Ok(StrategyView {
        orchestration_id: cfg.id,
        policy_revision: cfg.policy_revision,
        team_version: cfg.team_version,
        settings: snapshot.policy.strategy,
        max_attempts_per_assignment: snapshot.policy.max_attempts_per_assignment,
        max_concurrent_tasks: snapshot.policy.max_concurrent_tasks,
        executable_parallel_workers: 1,
        token_metering_available: false,
        priced_cost_metering_available: false,
        nested_child_admission_available: true,
        enable_blockers,
    })
}

fn update(store: &Store, orchestration_id: &str, expected_revision: i64,
    input: StrategyUpdate) -> Result<StrategyView> {
    let strategy = input.validate()?;
    let current = super::configuration(store, orchestration_id)?;
    if current.policy_revision != expected_revision || current.enabled {
        return Err("Disable this Team and reload its exact policy revision before editing strategy.".into());
    }
    let previous = load_team_version_snapshot(&store.db, orchestration_id, current.team_version)?;
    if previous.policy.strategy == strategy
        && previous.policy.max_attempts_per_assignment == input.max_attempts_per_assignment
        && previous.policy.max_concurrent_tasks == input.max_concurrent_tasks {
        return read(store, orchestration_id);
    }
    // External effects and active user decisions cannot race a new Team policy.
    let _fence = effects::lock_dispatch(&store.path)?;
    let tx = store.db.unchecked_transaction().map_err(|error| error.to_string())?;
    let unsettled: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sa_tickets t WHERE t.orchestration_id=?1
           AND (t.state NOT IN ('completed','failed','stopped','interrupted','inconclusive')
             OR EXISTS(SELECT 1 FROM sa_assignments a WHERE a.ticket_id=t.id AND a.state='running')
             OR EXISTS(SELECT 1 FROM sa_effect_receipts e WHERE e.ticket_id=t.id AND e.state IN ('pending','uncertain'))))",
        [orchestration_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if unsettled {
        return Err("Settle Tickets and uncertain effects before changing Team strategy.".into());
    }
    let next_revision = expected_revision.checked_add(1)
        .ok_or("Team policy revision limit reached.")?;
    let next_version = current.team_version.checked_add(1)
        .ok_or("Team version limit reached.")?;
    let updated = tx.execute(
        "UPDATE sa_orchestrations SET policy_revision=?2,updated_at=?3
         WHERE id=?1 AND enabled=0 AND policy_revision=?4",
        params![orchestration_id, next_revision, storage::now(), expected_revision],
    ).map_err(|error| error.to_string())?;
    if updated != 1 {
        return Err("Team policy changed during strategy update; reload before editing.".into());
    }
    let mut next = make_team_snapshot(&tx, orchestration_id, next_version,
        &current.name, &current.project_id, next_revision,
        &current.coordinator_bot_id, &current.worker_bot_ids,
        &current.reviewer_bot_ids, current.council_required, TeamSnapshotProvenance::Native)?;
    next.policy.max_attempts_per_assignment = input.max_attempts_per_assignment;
    next.policy.max_concurrent_tasks = input.max_concurrent_tasks;
    next.policy.strategy = strategy;
    insert_team_version(&tx, &next)?;
    tx.commit().map_err(|error| error.to_string())?;
    read(store, orchestration_id)
}

pub(crate) fn headless_inspect(database_path: &std::path::Path,
    orchestration_id: &str) -> Result<String> {
    let store = Store::open_read_only(database_path)?;
    serde_json::to_string_pretty(&read(&store, orchestration_id)?)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn superagent_strategy_get(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    orchestration_id: String,
) -> Result<StrategyView> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|error| error.to_string())?;
    read(&store, &orchestration_id)
}

#[tauri::command]
pub async fn superagent_strategy_update(
    window: tauri::WebviewWindow, state: State<'_, AppState>,
    orchestration_id: String, expected_revision: i64, input: StrategyUpdate,
) -> Result<StrategyView> {
    require_main_window(&window)?;
    let store = state.store.lock().map_err(|error| error.to_string())?;
    update(&store, &orchestration_id, expected_revision, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_defaults_preserve_existing_manual_teams() {
        let default = StrategySettings::default();
        assert_eq!(default.priority, "balanced");
        assert_eq!(default.attempt_timeout_ms, MAX_ASSIGNMENT_RUN_MS);
        assert!(default.token_ceiling.is_none());
        assert!(!default.subagent_delegation_enabled);
        let old: StrategySettings = serde_json::from_str("{}").unwrap();
        assert_eq!(old, default);
    }

    #[test]
    fn capability_budget_validation_is_bounded_and_non_privileged() {
        let mut request = StrategyUpdate {
            priority: "cost".into(), attempt_timeout_ms: 300_000,
            max_attempts_per_assignment: 3, max_concurrent_tasks: 2,
            token_ceiling: None, estimated_cost_ceiling_minor: None,
            subagent_delegation_enabled: false,
        };
        assert_eq!(request.validate().unwrap().priority, "cost");
        request.max_concurrent_tasks = 0;
        assert!(request.validate().is_err());
        request.max_concurrent_tasks = 1;
        request.attempt_timeout_ms = MAX_ASSIGNMENT_RUN_MS + 1;
        assert!(request.validate().is_err());
        request.attempt_timeout_ms = 60_000;
        request.priority = "unbounded".into();
        assert!(request.validate().is_err());
    }
}
