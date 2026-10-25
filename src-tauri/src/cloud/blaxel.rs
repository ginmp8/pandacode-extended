//! Blaxel sandboxes for explicit sandbox management. The control plane is
//! `https://api.blaxel.ai/v0` (https://docs.blaxel.ai/api-reference/compute/create-sandbox);
//! commands go to each sandbox's own API at the `metadata.url` Blaxel reports
//! (https://docs.blaxel.ai/api-reference/process/execute-a-command). A sandbox without an
//! open connection drops to standby with memory, files and processes snapshotted.
//! Explicit commands can wake it; PandaCode does not install a background keep-alive process.
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use super::{http, http::block, sandbox::{Info, Sandbox}, Machine};
use crate::Result;

const DEFAULT_API: &str = "https://api.blaxel.ai/v0";
const SERVICE: &str = "Blaxel";
const IMAGE: &str = "blaxel/base-image:latest";
/// `waitForCompletion` on the sandbox API gives up after 60 s; longer commands are polled.
const WAIT_SECONDS: u64 = 55;

pub struct Blaxel {
    api: String,
    key: String,
    workspace: String,
}

fn machine(sandbox: &Value) -> Machine {
    let name = sandbox["metadata"]["name"].as_str().unwrap_or_default();
    let state = match sandbox["status"].as_str().unwrap_or("DEPLOYED") {
        "DEPLOYED" => "running",
        "UPLOADING" | "BUILDING" | "BUILT" | "DEPLOYING" | "UNARCHIVING" => "starting",
        "DEACTIVATING" | "ARCHIVING" | "DELETING" => "stopping",
        "ARCHIVED" => "stopped",
        // Deactivated and terminated sandboxes cannot be revived.
        _ => "error",
    };
    let memory = sandbox["spec"]["runtime"]["memory"].as_u64().unwrap_or(4096);
    Machine {
        id: format!("blaxel:{name}"),
        provider: "blaxel".into(),
        name: sandbox["metadata"]["displayName"].as_str().filter(|n| !n.is_empty()).unwrap_or(name).to_string(),
        state: state.into(),
        os: "linux".into(),
        desktop: false,
        detail: format!("Blaxel · {} CPU · {} GB", (memory / 2048).max(1), memory / 1024),
    }
}

/// Blaxel names are lowercase letters, digits and hyphens, at most 49 characters.
fn slug(name: &str) -> String {
    let clean: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let suffix = &uuid::Uuid::new_v4().simple().to_string()[..6];
    format!("{}-{suffix}", clean.trim_matches('-').chars().take(40).collect::<String>().trim_matches('-')).trim_start_matches('-').to_string()
}

impl Blaxel {
    pub fn new(api: Option<String>, key: String, workspace: String) -> Self {
        Blaxel { api: api.filter(|a| !a.trim().is_empty()).unwrap_or_else(|| DEFAULT_API.into()).trim_end_matches('/').to_string(), key, workspace }
    }
    fn connected() -> Result<Self> {
        let key = super::secret("blaxel_key").ok_or("Connect Blaxel in Settings → Cloud first.")?;
        let workspace = super::settings().providers.get("blaxel").and_then(|p| p.get("workspace").cloned()).filter(|w| !w.is_empty()).ok_or("Connect Blaxel in Settings → Cloud first.")?;
        Ok(Blaxel::new(None, key, workspace))
    }
    async fn call(&self, method: reqwest::Method, url: &str, body: Option<Value>, timeout: Duration) -> Result<Value> {
        let request = http::client(timeout)?.request(method, url).bearer_auth(&self.key).header("X-Blaxel-Workspace", &self.workspace);
        http::send(SERVICE, match body { Some(body) => request.json(&body), None => request }).await
    }
    async fn api(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value> {
        self.call(method, &format!("{}{path}", self.api), body, Duration::from_secs(60)).await
    }

    pub async fn list(&self) -> Result<Vec<Machine>> {
        let all = self.api(reqwest::Method::GET, "/sandboxes", None).await?;
        let mine = |s: &&Value| s["metadata"]["labels"]["pandacode"] == "true" && !matches!(s["status"].as_str(), Some("TERMINATED" | "DELETING"));
        Ok(all.as_array().map(|items| items.iter().filter(mine).map(machine).collect()).unwrap_or_default())
    }
    pub async fn create(&self, name: &str) -> Result<Machine> {
        let body = json!({"metadata": {"name": slug(name), "labels": {"pandacode": "true"}}, "spec": {"runtime": {"image": IMAGE, "memory": 4096}}});
        Ok(machine(&self.api(reqwest::Method::POST, "/sandboxes", Some(body)).await?))
    }
    pub async fn power(&self, name: &str, action: &str) -> Result<()> {
        match action {
            "delete" => self.api(reqwest::Method::DELETE, &format!("/sandboxes/{name}"), None).await.map(|_| ()),
            // Standby is automatic and keeps everything; any request to the sandbox wakes it.
            "stop" => Err("Blaxel standby is automatic; it has no explicit Stop operation. Use Delete to remove the sandbox.".into()),
            "start" => self.exec(name, "true", Duration::from_secs(60)).await.map(|_| ()),
            _ => Err(format!("Blaxel sandboxes can start or be deleted, not {action}.")),
        }
    }

    /// The sandbox's own API address, `metadata.url`.
    async fn url(&self, name: &str) -> Result<String> {
        let sandbox = self.api(reqwest::Method::GET, &format!("/sandboxes/{name}"), None).await?;
        let url = sandbox["metadata"]["url"].as_str().filter(|u| !u.is_empty()).ok_or("Blaxel did not report an address for this sandbox yet.")?;
        http::endpoint(SERVICE, url, &["run.blaxel.ai"], &self.api)
    }
    fn output(process: &Value) -> (Option<i32>, String) {
        let logs = process["logs"].as_str().map(str::to_string).unwrap_or_else(|| format!("{}{}", process["stdout"].as_str().unwrap_or_default(), process["stderr"].as_str().unwrap_or_default()));
        (process["exitCode"].as_i64().map(|code| code as i32), logs)
    }

    pub async fn exec(&self, name: &str, command: &str, timeout: Duration) -> Result<(Option<i32>, String)> {
        let url = self.url(name).await?;
        let wait = timeout.as_secs().clamp(1, WAIT_SECONDS);
        let body = json!({"command": format!("sh -c {}", super::shell_quote(command)), "waitForCompletion": true, "timeout": wait});
        let mut process = self.call(reqwest::Method::POST, &format!("{url}/process"), Some(body), Duration::from_secs(wait + 15)).await?;
        let deadline = Instant::now() + timeout;
        // The sandbox API reports the pid as a string, but a number must work too.
        let id = process["pid"].as_str().map(str::to_string).or_else(|| process["pid"].as_u64().map(|n| n.to_string())).or_else(|| process["name"].as_str().map(str::to_string)).unwrap_or_default();
        if process["status"] == "running" { super::sandbox::validate_id(&id)?; }
        while process["status"] == "running" {
            if Instant::now() >= deadline {
                let _ = self.call(reqwest::Method::DELETE, &format!("{url}/process/{id}/kill"), None, Duration::from_secs(15)).await;
                return Err("The command ran past its time limit.".into());
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
            process = self.call(reqwest::Method::GET, &format!("{url}/process/{id}"), None, Duration::from_secs(30)).await?;
        }
        Ok(Self::output(&process))
    }
}

impl Sandbox for Blaxel {
    /// Standby snapshots files and memory. No background keep-alive process is installed.
    fn info(&self) -> Info {
        Info { persistent: true, max_lease_minutes: 24 * 60 }
    }
    fn list(&self) -> Result<Vec<Machine>> {
        block(Blaxel::list(self))
    }
    fn create(&self, name: &str) -> Result<Machine> {
        block(Blaxel::create(self, name))
    }
    fn power(&self, native: &str, action: &str) -> Result<()> {
        block(Blaxel::power(self, native, action))
    }
    fn exec(&self, native: &str, command: &str, timeout: Duration) -> Result<(Option<i32>, String)> {
        block(Blaxel::exec(self, native, command, timeout))
    }
    /// Standby is automatic; this manual adapter does not install a background process.
    fn keep_alive(&self, _native: &str, _minutes: u32) -> Result<()> {
        Err("Blaxel does not offer an explicit activity lease.".into())
    }
}

pub fn connect(key: &str, options: &Value) -> Result<()> {
    let (key, workspace) = (key.trim(), options["workspace"].as_str().unwrap_or_default().trim());
    if key.is_empty() {
        return Err("Enter your Blaxel API key.".into());
    }
    if workspace.is_empty() {
        return Err("Enter your Blaxel workspace.".into());
    }
    block(Blaxel::new(None, key.into(), workspace.into()).api(reqwest::Method::GET, "/sandboxes", None))?;
    super::set_secret("blaxel_key", key)?;
    let mut settings = super::settings();
    settings.providers.insert("blaxel".into(), BTreeMap::from([("workspace".into(), workspace.into()), ("detail".into(), format!("Workspace {workspace}"))]));
    super::save_settings(&settings)
}
pub fn disconnect() -> Result<()> {
    super::remove_secret("blaxel_key");
    let mut settings = super::settings();
    settings.providers.remove("blaxel");
    super::save_settings(&settings)
}
pub fn sandbox() -> Result<Box<dyn Sandbox>> {
    Ok(Box::new(Blaxel::connected()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::http::mock;

    fn blaxel() -> (Blaxel, mock::Seen) {
        let (base, seen) = mock::serve(|request, base| match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/v0/sandboxes") => (200, r#"[{"metadata":{"name":"a-1","labels":{"pandacode":"true"}},"status":"DEPLOYED","state":"STANDBY","spec":{"runtime":{"memory":4096}}},{"metadata":{"name":"b","labels":{}},"status":"DEPLOYED"},{"metadata":{"name":"c","labels":{"pandacode":"true"}},"status":"TERMINATED"},{"metadata":{"name":"d","labels":{"pandacode":"true"}},"status":"DEPLOYING"}]"#.to_string()),
            ("POST", "/v0/sandboxes") => (200, r#"{"metadata":{"name":"job-abc123","labels":{"pandacode":"true"}},"status":"DEPLOYING"}"#.into()),
            ("DELETE", "/v0/sandboxes/a-1") => (200, "{}".into()),
            ("GET", "/v0/sandboxes/a-1") => (200, format!(r#"{{"metadata":{{"name":"a-1","url":"{base}/sbx/"}},"status":"DEPLOYED"}}"#)),
            ("GET", "/v0/sandboxes/gone") => (200, r#"{"metadata":{"name":"gone"},"status":"TERMINATED"}"#.into()),
            ("GET", "/v0/sandboxes/archived") => (200, r#"{"metadata":{"name":"archived"},"status":"ARCHIVED"}"#.into()),
            ("GET", "/v0/sandboxes/pending") => (200, r#"{"metadata":{"name":"pending"},"status":"DEPLOYING"}"#.into()),
            ("POST", "/sbx/process") if request.body.contains("sleep 100") => (200, r#"{"pid":"7","name":"p","status":"running"}"#.into()),
            ("POST", "/sbx/process") if request.body.contains("keepAlive") => (200, r#"{"pid":"8","status":"running"}"#.into()),
            ("POST", "/sbx/process") if request.body.contains("sleep 200") => (200, r#"{"pid":9,"status":"running"}"#.into()),
            ("POST", "/sbx/process") => (200, r#"{"pid":"6","status":"failed","exitCode":3,"logs":"42\n","stdout":"42\n","stderr":""}"#.into()),
            ("GET", "/sbx/process/9") => (200, r#"{"pid":9,"status":"completed","exitCode":1,"logs":"num\n"}"#.into()),
            ("GET", "/sbx/process/7") => (200, r#"{"pid":"7","status":"completed","exitCode":0,"logs":"late\n"}"#.into()),
            _ => (500, r#"{"message":"unexpected"}"#.into()),
        });
        (Blaxel::new(Some(format!("{base}/v0")), "key-1".into(), "ws".into()), seen)
    }

    #[test]
    fn names_follow_blaxel_rules() {
        let name = slug("My Task_01!");
        assert!(name.starts_with("my-task-01-") && name.len() <= 49 && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        assert!(slug(&"x".repeat(100)).len() <= 49);
    }

    #[tokio::test]
    async fn lists_creates_and_changes_power() {
        let (api, seen) = blaxel();
        let machines = api.list().await.unwrap();
        assert_eq!(machines.iter().map(|m| (m.id.as_str(), m.state.as_str())).collect::<Vec<_>>(), vec![("blaxel:a-1", "running"), ("blaxel:d", "starting")]);
        assert_eq!(machines[0].detail, "Blaxel · 2 CPU · 4 GB");
        assert_eq!(api.create("job").await.unwrap().id, "blaxel:job-abc123");
        assert!(api.power("a-1", "stop").await.is_err());
        api.power("a-1", "delete").await.unwrap();
        assert!(api.power("a-1", "pause").await.is_err());
        assert_eq!(machine(&json!({"metadata": {"name": "pending"}, "status": "DEPLOYING"})).state, "starting");
        assert_eq!(machine(&json!({"metadata": {"name": "gone"}, "status": "TERMINATED"})).state, "error");
        assert_eq!(machine(&json!({"metadata": {"name": "archived"}, "status": "ARCHIVED"})).state, "stopped");
        assert!(Sandbox::keep_alive(&api, "a-1", 30).is_err());
        let log = seen.lock().unwrap();
        assert!(log.iter().all(|r| r.authorization.as_deref() == Some("Bearer key-1") && r.headers.iter().any(|(n, v)| n == "x-blaxel-workspace" && v == "ws")));
        let body: Value = serde_json::from_str(&log[1].body).unwrap();
        assert_eq!(body["metadata"]["labels"], json!({"pandacode": "true"}));
        assert_eq!(body["spec"]["runtime"]["image"], IMAGE);
    }

    #[tokio::test]
    async fn runs_commands_and_polls_slow_ones_without_background_processes() {
        let (api, seen) = blaxel();
        assert_eq!(api.exec("a-1", "echo $((6*7))", Duration::from_secs(9)).await.unwrap(), (Some(3), "42\n".to_string()));
        assert_eq!(api.exec("a-1", "sleep 100", Duration::from_secs(9)).await.unwrap(), (Some(0), "late\n".to_string()));
        assert_eq!(api.exec("a-1", "sleep 200", Duration::from_secs(9)).await.unwrap(), (Some(1), "num\n".to_string()));
        assert!(Sandbox::keep_alive(&api, "a-1", 30).is_err());
        assert!(api.exec("pending", "ls", Duration::from_secs(5)).await.unwrap_err().contains("address"));
        let log = seen.lock().unwrap();
        let posts: Vec<Value> = log.iter().filter(|r| r.path == "/sbx/process").map(|r| serde_json::from_str(&r.body).unwrap()).collect();
        assert_eq!(posts[0], json!({"command":"sh -c 'echo $((6*7))'","waitForCompletion":true,"timeout":9}));
        assert_eq!(posts.len(), 3);
        assert!(posts.iter().all(|post| post.get("keepAlive").is_none()));
    }

    #[test]
    fn connect_needs_a_workspace_and_errors_are_mapped() {
        assert_eq!(connect("k", &json!({})).unwrap_err(), "Enter your Blaxel workspace.");
        assert_eq!(connect("", &json!({"workspace":"w"})).unwrap_err(), "Enter your Blaxel API key.");
        let (base, _) = mock::serve(|_, _| (401, r#"{"error":"nope"}"#.to_string()));
        let api = Blaxel::new(Some(base), "bad".into(), "w".into());
        assert!(block(api.list()).unwrap_err().contains("refused the credentials"));
    }

    #[tokio::test]
    async fn does_not_forward_workspace_credentials_to_untrusted_process_servers() {
        let (base, seen) = mock::serve(|_, _| (200, r#"{"metadata":{"url":"https://run.blaxel.ai.attacker.invalid"}}"#));
        let error = Blaxel::new(Some(base), "fixture-api".into(), "workspace".into()).exec("box-1", "true", Duration::from_secs(2)).await.unwrap_err();
        assert!(error.contains("untrusted sandbox address"), "{error}");
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

}
