//! Persisted opt-in switches for actions initiated by PandaCode itself.
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use tauri::Emitter;

const SERVICE: &str = "app.pandacode.feature-controls";
const KEY: &str = "settings";

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub agent_update_checks: bool,
    pub cloud_credential_transfer: bool,
    pub agent_cloud_management: bool,
    pub external_mcp_servers: bool,
    /// Global pause. Per-bot and per-channel preferences are preserved.
    pub bots_paused: bool,
}

static CURRENT: OnceLock<Mutex<Settings>> = OnceLock::new();
static OPERATIONS: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn state() -> &'static Mutex<Settings> {
    CURRENT.get_or_init(|| Mutex::new(Settings::default()))
}

pub fn setup() {
    let settings = crate::secret_store::get(SERVICE, KEY)
        .and_then(|value| serde_json::from_str::<Settings>(&value).ok())
        .unwrap_or_default();
    let _ = CURRENT.set(Mutex::new(settings));
}

pub fn get() -> Settings {
    *state().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn update(key: &str, enabled: bool) -> crate::Result<(Settings, bool)> {
    let mut current = state().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut next = *current;
    match key {
        "agentUpdateChecks" => next.agent_update_checks = enabled,
        "cloudCredentialTransfer" => next.cloud_credential_transfer = enabled,
        "agentCloudManagement" => next.agent_cloud_management = enabled,
        "externalMcpServers" => next.external_mcp_servers = enabled,
        "botsPaused" => next.bots_paused = enabled,
        _ => return Err("Unknown feature control.".into()),
    }
    if next == *current {
        return Ok((next, false));
    }
    crate::secret_store::set(
        SERVICE,
        KEY,
        &serde_json::to_string(&next).map_err(|error| error.to_string())?,
    )?;
    *current = next;
    Ok((next, true))
}

#[tauri::command]
pub async fn feature_controls_get() -> crate::Result<Settings> {
    Ok(get())
}

#[tauri::command]
pub async fn feature_controls_set(
    app: tauri::AppHandle,
    key: String,
    enabled: bool,
) -> crate::Result<Settings> {
    // Serialize runtime transitions too: a quick pause/resume cannot restart
    // channels before the preceding pause has finished stopping them.
    let _turn = OPERATIONS.get_or_init(Default::default).lock().await;
    let (settings, changed) = update(&key, enabled)?;
    if changed && key == "botsPaused" {
        if enabled {
            crate::bots::pause_all(&app).await;
        } else {
            crate::bots::channels::start_all(app.clone());
        }
    }
    if changed {
        let _ = app.emit("feature-controls-changed", settings);
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_new_control_requires_explicit_opt_in() {
        let settings = Settings::default();
        assert!(!settings.agent_update_checks);
        assert!(!settings.cloud_credential_transfer);
        assert!(!settings.agent_cloud_management);
        assert!(!settings.external_mcp_servers);
        assert!(!settings.bots_paused);
    }

    #[test]
    fn partial_settings_keep_safe_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"agentUpdateChecks":true}"#).unwrap();
        assert!(settings.agent_update_checks);
        assert!(!settings.cloud_credential_transfer);
        assert!(!settings.agent_cloud_management);
        assert!(!settings.external_mcp_servers);
        assert!(!settings.bots_paused);
    }
}
