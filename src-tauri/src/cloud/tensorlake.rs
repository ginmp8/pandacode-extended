//! Tensorlake sandboxes for explicit sandbox management, through the REST API at
//! https://api.tensorlake.ai (https://docs.tensorlake.ai/api-reference/v2/introduction):
//! `/sandboxes` creates, lists, suspends, resumes and deletes; each sandbox's management
//! URL runs commands (`/api/v1/processes/run`, a server-sent event stream that ends with
//! the exit code). Sandboxes are named, so stop
//! suspends and keeps their files. The lifetime is set only at creation (`timeout_secs: 0`
//! asks for the plan's maximum, 1 h on the free plan and 24 h on demand), so `keep_alive`
//! has nothing to extend.
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Duration};

use super::{http, http::block, sandbox::{Info, Sandbox}, Machine};

type Result<T> = std::result::Result<T, String>;

const DEFAULT_API: &str = "https://api.tensorlake.ai";
const SERVICE: &str = "Tensorlake";
/// ponytail: the free plan's 1 h lifetime; read the plan when Tensorlake exposes it.
const MAX_MINUTES: u32 = 60;
/// Keep about this much of a command's output (the tail).
const KEEP: usize = 4 * 1024 * 1024;
// The provider image is used as prepared; creating a sandbox never installs packages.

pub struct Tensorlake {
    api: String,
    key: String,
}

fn machine(sandbox: &Value) -> Machine {
    let id = sandbox["id"].as_str().or(sandbox["sandbox_id"].as_str()).unwrap_or_default();
    let state = match sandbox["status"].as_str().or(sandbox["state"].as_str()).unwrap_or("unknown") {
        "pending" => "starting",
        "snapshotting" | "suspending" => "stopping",
        "suspended" => "stopped",
        // Gone for good: the user can create another sandbox explicitly.
        "terminated" | "failed" => "error",
        other => other,
    };
    let resources = match (sandbox["resources"]["cpus"].as_f64(), sandbox["resources"]["memory_mb"].as_u64()) {
        (Some(cpus), Some(mb)) => format!(" · {cpus} CPU · {} GB", mb / 1024),
        _ => String::new(),
    };
    Machine {
        id: format!("tensorlake:{id}"),
        provider: "tensorlake".into(),
        name: sandbox["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(id).to_string(),
        state: state.into(),
        os: "linux".into(),
        desktop: false,
        detail: format!("Tensorlake{resources}"),
    }
}

/// Exit code and output lines from a `processes/run` event stream.
fn read_events(text: &str, code: &mut Option<i32>, output: &mut String) {
    for data in text.lines().filter_map(|line| line.strip_prefix("data:")) {
        let Ok(event) = serde_json::from_str::<Value>(data.trim()) else { continue };
        if let Some(line) = event["line"].as_str() {
            output.push_str(line);
            output.push('\n');
        }
        if event.get("exit_code").is_some() {
            *code = event["exit_code"].as_i64().map(|c| c as i32);
        }
    }
    if output.len() > 2 * KEEP {
        let cut = output.len() - KEEP;
        let cut = (cut..output.len()).find(|&i| output.is_char_boundary(i)).unwrap_or(cut);
        output.drain(..cut);
    }
}

impl Tensorlake {
    fn new(api: &str, key: &str) -> Self {
        Tensorlake { api: api.trim_end_matches('/').into(), key: key.into() }
    }
    fn connected() -> Result<Self> {
        let key = super::secret("tensorlake_key").ok_or("Connect Tensorlake in Settings → Cloud first.")?;
        Ok(Tensorlake::new(DEFAULT_API, &key))
    }

    async fn api(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value> {
        let client = http::client(Duration::from_secs(300))?;
        let request = client.request(method, format!("{}{path}", self.api)).bearer_auth(&self.key);
        http::send(SERVICE, match body { Some(body) => request.json(&body), None => request }).await
    }

    /// The sandbox's management URL, once it runs.
    async fn management(&self, id: &str) -> Result<String> {
        let sandbox = self.api(reqwest::Method::GET, &format!("/sandboxes/{id}"), None).await?;
        match sandbox["status"].as_str() {
            Some("running") => {}
            status => return Err(format!("This Tensorlake sandbox is {}. Start it first.", http::redact(status.unwrap_or("not running"), &[&self.key]).chars().take(100).collect::<String>())),
        }
        let url = sandbox["sandbox_url"].as_str().or(sandbox["ingress_endpoint"].as_str()).filter(|u| !u.is_empty()).map(str::to_string);
        http::endpoint(SERVICE, &url.unwrap_or_else(|| format!("https://{id}.sandbox.tensorlake.ai")), &["sandbox.tensorlake.ai"], &self.api)
    }

    async fn list(&self) -> Result<Vec<Machine>> {
        let page = self.api(reqwest::Method::GET, "/sandboxes?limit=100", None).await?;
        Ok(page["sandboxes"].as_array().into_iter().flatten().map(machine).collect())
    }

    async fn create(&self, name: &str) -> Result<Machine> {
        // Names make sandboxes suspendable and must be unique.
        let name = format!("{name}-{}", &uuid::Uuid::new_v4().simple().to_string()[..6]);
        let body = json!({"name": name, "resources": {"cpus": 2, "memory_mb": 4096}, "timeout_secs": 0, "wait": true});
        let created = self.api(reqwest::Method::POST, "/sandboxes", Some(body)).await?;
        let id = created["sandbox_id"].as_str().ok_or("Tensorlake did not report the new sandbox.")?.to_string();
        Ok(machine(&json!({"id": id, "name": name, "status": created["status"]})))
    }

    async fn power(&self, id: &str, action: &str) -> Result<()> {
        let (method, path) = match action {
            "start" => (reqwest::Method::POST, format!("/sandboxes/{id}/resume")),
            "stop" => (reqwest::Method::POST, format!("/sandboxes/{id}/suspend")),
            "delete" => (reqwest::Method::DELETE, format!("/sandboxes/{id}")),
            _ => return Err(format!("Tensorlake sandboxes can start, stop or be deleted, not {action}.")),
        };
        self.api(method, &path, None).await.map(|_| ())
    }

    async fn exec(&self, id: &str, command: &str, timeout: Duration) -> Result<(Option<i32>, String)> {
        let url = self.management(id).await?;
        let client = http::client(timeout + Duration::from_secs(30))?;
        let body = json!({"command": "sh", "args": ["-c", command], "timeout": timeout.as_secs()});
        let mut response = client.post(format!("{url}/api/v1/processes/run")).bearer_auth(&self.key).header("Accept", "text/event-stream").json(&body).send().await
            .map_err(|e| if e.is_timeout() { format!("{SERVICE} did not answer in time.") } else { format!("Could not reach {SERVICE}. Check your connection.") })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(http::error_message(SERVICE, status, ""));
        }
        let (mut code, mut output, mut pending) = (None, String::new(), Vec::new());
        while let Some(chunk) = response.chunk().await.map_err(|_| format!("{SERVICE} output was cut off."))? {
            if pending.len().saturating_add(chunk.len()) > http::LIMIT { return Err("Tensorlake output event exceeded its size limit.".into()); }
            pending.extend_from_slice(&chunk);
            if let Some(end) = pending.iter().rposition(|&byte| byte == b'\n') {
                read_events(&String::from_utf8_lossy(&pending[..end]), &mut code, &mut output);
                pending.drain(..=end);
            }
        }
        read_events(&String::from_utf8_lossy(&pending), &mut code, &mut output);
        Ok((code, http::redact(&output, &[&self.key])))
    }

}

impl Sandbox for Tensorlake {
    fn info(&self) -> Info {
        Info { persistent: true, max_lease_minutes: MAX_MINUTES }
    }
    fn list(&self) -> Result<Vec<Machine>> {
        block(Tensorlake::list(self))
    }
    fn create(&self, name: &str) -> Result<Machine> {
        block(Tensorlake::create(self, name))
    }
    fn power(&self, native: &str, action: &str) -> Result<()> {
        block(Tensorlake::power(self, native, action))
    }
    fn exec(&self, native: &str, command: &str, timeout: Duration) -> Result<(Option<i32>, String)> {
        block(Tensorlake::exec(self, native, command, timeout))
    }
    /// The lifetime is fixed at creation; manual activity leases are unavailable.
    fn keep_alive(&self, _native: &str, _minutes: u32) -> Result<()> {
        Ok(())
    }
}

pub fn connect(key: &str, _options: &Value) -> Result<()> {
    block(Tensorlake::new(DEFAULT_API, key.trim()).api(reqwest::Method::GET, "/sandboxes?limit=1", None))?;
    super::set_secret("tensorlake_key", key.trim())?;
    let mut settings = super::settings();
    settings.providers.insert("tensorlake".into(), BTreeMap::from([("detail".into(), "API key saved".into())]));
    super::save_settings(&settings)
}
pub fn disconnect() -> Result<()> {
    super::remove_secret("tensorlake_key");
    let mut settings = super::settings();
    settings.providers.remove("tensorlake");
    super::save_settings(&settings)
}
pub fn sandbox() -> Result<Box<dyn Sandbox>> {
    Ok(Box::new(Tensorlake::connected()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::http::mock;

    fn tensorlake() -> (Tensorlake, mock::Seen) {
        let (base, seen) = mock::serve(|request, base| match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/sandboxes?limit=100") => (200, r#"{"sandboxes":[{"id":"s1","name":"box-1","status":"running","resources":{"cpus":2.0,"memory_mb":4096}},{"id":"s2","name":"","status":"suspended"},{"id":"s3","name":"old","status":"terminated"}]}"#.to_string()),
            ("POST", "/sandboxes") => (200, r#"{"sandbox_id":"s1","status":"running"}"#.into()),
            ("GET", "/sandboxes/s1") => (200, format!(r#"{{"id":"s1","status":"running","sandbox_url":"{base}/s1/"}}"#)),
            ("GET", "/sandboxes/s2") => (200, r#"{"id":"s2","status":"suspended"}"#.into()),
            ("POST", "/sandboxes/s1/suspend") => (202, "{}".into()),
            ("DELETE", "/sandboxes/s1") => (200, String::new()),
            ("POST", "/s1/api/v1/processes/run") if !request.body.contains("echo 42") => (200, "event: exited\ndata: {\"exit_code\":0}\n\n".into()),
            ("POST", "/s1/api/v1/processes/run") => (200, "event: started\ndata: {\"pid\":4}\n\nevent: output\ndata: {\"line\":\"42\",\"stream\":\"stdout\"}\n\nevent: output\ndata: {\"line\":\"oops\",\"stream\":\"stderr\"}\n\nevent: exited\ndata: {\"exit_code\":3}\n\n".into()),
            ("PUT", "/s1/api/v1/files?path=%2Fhome%2Ftl-user%2Fa+b") => (204, String::new()),
            _ => (404, r#"{"message":"not found"}"#.into()),
        });
        (Tensorlake::new(&base, "tl-key"), seen)
    }

    #[tokio::test]
    async fn lists_creates_and_powers_sandboxes_with_the_key() {
        let (api, seen) = tensorlake();
        let machines = api.list().await.unwrap();
        assert_eq!(machines.iter().map(|m| (m.id.as_str(), m.name.as_str(), m.state.as_str())).collect::<Vec<_>>(), vec![("tensorlake:s1", "box-1", "running"), ("tensorlake:s2", "s2", "stopped"), ("tensorlake:s3", "old", "error")]);
        assert_eq!(machines[0].detail, "Tensorlake · 2 CPU · 4 GB");
        let created = api.create("box").await.unwrap();
        assert_eq!((created.id.as_str(), created.state.as_str()), ("tensorlake:s1", "running"));
        assert!(created.name.starts_with("box-"));
        api.power("s1", "stop").await.unwrap();
        api.power("s1", "delete").await.unwrap();
        assert!(api.power("s1", "reboot").await.is_err());
        let log = seen.lock().unwrap();
        assert!(log.iter().all(|r| r.authorization.as_deref() == Some("Bearer tl-key")));
        let body: Value = serde_json::from_str(&log[1].body).unwrap();
        assert_eq!((body["timeout_secs"].as_u64(), body["wait"].as_bool()), (Some(0), Some(true)));
        assert!(log.iter().all(|request| !request.path.ends_with("/processes/run")), "create never installs packages or executes setup commands");
    }

    #[tokio::test]
    async fn runs_commands_from_the_event_stream() {
        let (api, seen) = tensorlake();
        assert_eq!(api.exec("s1", "echo 42", Duration::from_secs(9)).await.unwrap(), (Some(3), "42\noops\n".to_string()));
        assert!(api.exec("s2", "ls", Duration::from_secs(5)).await.unwrap_err().contains("suspended. Start it first"));
        assert_eq!(api.exec("gone", "ls", Duration::from_secs(5)).await.unwrap_err(), "Tensorlake: not found");
        let log = seen.lock().unwrap();
        let run: Value = serde_json::from_str(&log[1].body).unwrap();
        assert_eq!(run, json!({"command":"sh","args":["-c","echo 42"],"timeout":9}));

    }

    #[test]
    fn keeps_the_tail_of_long_output() {
        let (mut code, mut output) = (None, String::new());
        let line = format!("data: {{\"line\":\"{}\"}}\n", "x".repeat(1024 * 1024));
        read_events(&line.repeat(9), &mut code, &mut output);
        assert!(output.len() <= KEEP + 1024 * 1024 + 9 && output.len() >= KEEP && code.is_none());
        read_events("data: {\"exit_code\":null}\n", &mut code, &mut output);
        assert!(code.is_none());
    }

    #[tokio::test]
    async fn rejects_untrusted_management_urls_without_executing_a_command() {
        let (base, seen) = mock::serve(|_, _| (200, r#"{"status":"running","sandbox_url":"https://attacker.invalid"}"#));
        let error = Tensorlake::new(&base, "fixture-api").exec("box-1", "true", Duration::from_secs(2)).await.unwrap_err();
        assert!(error.contains("untrusted sandbox address"), "{error}");
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

}
