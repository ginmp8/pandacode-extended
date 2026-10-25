//! Modal sandboxes for explicit sandbox management. Modal exposes sandboxes only
//! through its gRPC SDKs (https://modal.com/docs/reference/modal.Sandbox), so, as AWS uses
//! the user's AWS CLI, PandaCode drives the user's own `modal` Python package through the
//! small helper in `modal_helper.py` (one JSON request on stdin, one reply on stdout). The
//! token goes only into that process's environment.
//!
//! Sandboxes live in the app `pandacode`, tagged `pandacode=sandbox`, using a prepared image.
//! Modal can't pause or restart a sandbox, nor extend its timeout, so each is
//! created with the 24 h maximum and its PID 1 is a watchdog that exits (ending the
//! sandbox) at the deadline `keep_alive` writes.
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::BufRead,
    path::PathBuf,
    process::Command,
    time::Duration,
};

use super::sandbox::{checked, Info, Sandbox};
use super::Machine;
use crate::workspace::run_process_with_input;

type Result<T> = std::result::Result<T, String>;

const HELPER: &str = include_str!("modal_helper.py");
const MAX_MINUTES: u32 = 24 * 60;
const QUICK: Duration = Duration::from_secs(120);
const INSTALL: &str = "The Modal Python SDK is not available in the installed interpreter. Configure an installed Modal CLI/SDK environment, then try again. PandaCode does not install it.";
const DEFAULT_IMAGE: &str = "debian:bookworm-slim";

pub struct Modal {
    python: PathBuf,
    token_id: String,
    secret: String,
    environment: Option<String>,
    image: String,
}

/// The interpreter behind the `modal` CLI (pipx and uv installs have their own), else python3.
fn python() -> Result<PathBuf> {
    let from_cli = crate::token_saver::find_tool("modal")
        .and_then(|cli| std::fs::File::open(cli).ok())
        .and_then(|file| std::io::BufReader::new(file).lines().next()?.ok())
        .and_then(|line| line.strip_prefix("#!").map(|path| PathBuf::from(path.trim())))
        .filter(|path| path.is_file());
    from_cli.or_else(|| crate::token_saver::find_tool("python3")).ok_or_else(|| INSTALL.to_string())
}

fn machine(id: &str, name: &str) -> Machine {
    Machine {
        id: format!("modal:{id}"),
        provider: "modal".into(),
        name: if name.is_empty() { id } else { name }.to_string(),
        state: "running".into(),
        os: "linux".into(),
        desktop: false,
        detail: "Modal · prepared image · ends 24 h after creation · delete only".into(),
    }
}

impl Modal {
    fn connected() -> Result<Self> {
        let secret = super::secret("modal_key").ok_or("Connect Modal in Settings → Cloud first.")?;
        let options = super::settings().providers.get("modal").cloned().unwrap_or_default();
        let token_id = options.get("tokenId").cloned().ok_or("Connect Modal in Settings → Cloud first.")?;
        Ok(Modal { python: python()?, token_id, secret, environment: options.get("environment").cloned(), image: options.get("image").cloned().unwrap_or_else(|| DEFAULT_IMAGE.into()) })
    }

    fn call(&self, request: Value, timeout: Duration) -> Result<Value> {
        let mut command = Command::new(&self.python);
        command.args(["-I", "-c", HELPER]).env("MODAL_TOKEN_ID", &self.token_id).env("MODAL_TOKEN_SECRET", &self.secret).env_remove("MODAL_PROFILE").env_remove("MODAL_ENVIRONMENT");
        if let Some(environment) = &self.environment {
            command.env("MODAL_ENVIRONMENT", environment);
        }
        let out = run_process_with_input(command, "Modal", 16 << 20, timeout, Some(request.to_string().as_bytes()))?;
        let stdout = String::from_utf8_lossy(&out.stdout.bytes);
        let Some(reply) = stdout.lines().rev().find_map(|line| serde_json::from_str::<Value>(line).ok()) else {
            let stderr = String::from_utf8_lossy(&out.stderr.bytes);
            return Err(format!("Modal: {}", super::http::redact(stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the helper did not answer.").trim(), &[&self.secret]).chars().take(400).collect::<String>()));
        };
        match reply["error"].as_str() {
            Some("missing-modal") => Err(INSTALL.into()),
            Some(error) if error.starts_with("AuthError") => Err("Modal refused the token. Check the token ID and secret in Settings → Cloud.".into()),
            Some(error) => Err(format!("Modal: {}", super::http::redact(error, &[&self.secret]).chars().take(400).collect::<String>())),
            None => Ok(reply),
        }
    }

    /// Whether the sandbox runs, and seconds until Modal's timeout ends it (unknown for
    /// sandboxes created before PandaCode tagged the expiry).
    fn status(&self, native: &str) -> Result<(bool, Option<i64>)> {
        let reply = self.call(json!({"op": "running", "id": native}), QUICK)?;
        Ok((reply["running"] == true, reply["remaining"].as_i64()))
    }

    fn run(&self, native: &str, command: &str, timeout: Duration) -> Result<(Option<i32>, String)> {
        let request = json!({"op": "exec", "id": native, "command": command, "timeout": timeout.as_secs().max(1)});
        let reply = self.call(request, timeout + Duration::from_secs(90))?;
        Ok((reply["code"].as_i64().map(|code| code as i32), super::http::redact(reply["output"].as_str().unwrap_or_default(), &[&self.secret])))
    }
}

impl Sandbox for Modal {
    fn info(&self) -> Info {
        Info { persistent: false, max_lease_minutes: MAX_MINUTES }
    }
    fn list(&self) -> Result<Vec<Machine>> {
        let reply = self.call(json!({"op": "list"}), QUICK)?;
        Ok(reply["sandboxes"].as_array().into_iter().flatten().map(|s| machine(s["id"].as_str().unwrap_or_default(), s["name"].as_str().unwrap_or_default())).collect())
    }
    fn create(&self, name: &str) -> Result<Machine> {
        let reply = self.call(json!({"op": "create", "name": name, "image": self.image}), Duration::from_secs(600))?;
        Ok(machine(reply["id"].as_str().ok_or("Modal did not report the new sandbox.")?, name))
    }
    fn power(&self, native: &str, action: &str) -> Result<()> {
        match action {
            "delete" => self.call(json!({"op": "terminate", "id": native}), QUICK).map(|_| ()),
            _ => Err("Modal sandboxes can't be restarted. Create a new one.".into()),
        }
    }
    fn exec(&self, native: &str, command: &str, timeout: Duration) -> Result<(Option<i32>, String)> {
        self.run(native, command, timeout)
    }
    fn keep_alive(&self, native: &str, minutes: u32) -> Result<()> {
        let seconds = minutes.min(MAX_MINUTES) as i64 * 60;
        if let (_, Some(remaining)) = self.status(native)? {
            if seconds > remaining {
                return Err(format!("This Modal sandbox expires in {} minutes; delete it to start a fresh one.", remaining.max(0) / 60));
            }
        }
        checked(self.exec(native, &format!("echo $(( $(date +%s) + {seconds} )) > /tmp/.pandacode-deadline"), Duration::from_secs(60))?)
    }
}

pub fn connect(key: &str, options: &Value) -> Result<()> {
    let token_id = options["tokenId"].as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or("Enter the Modal token ID (ak-…) with its secret.")?;
    let environment = options["environment"].as_str().map(str::trim).filter(|e| !e.is_empty()).map(str::to_string);
    if token_id.len() > 256 || token_id.chars().any(char::is_control) || environment.as_deref().is_some_and(|e| e.len() > 128 || e.chars().any(char::is_control)) { return Err("Enter a valid Modal token ID and environment.".into()); }
    let image = options["image"].as_str().map(str::trim).filter(|v| !v.is_empty()).unwrap_or(DEFAULT_IMAGE);
    if image.len() > 256 || image.starts_with('-') || !image.bytes().all(|b| b.is_ascii_alphanumeric() || b"/._-:@".contains(&b)) { return Err("Enter a prepared OCI image name or digest for Modal.".into()); }
    let modal = Modal { python: python()?, token_id: token_id.into(), secret: key.trim().into(), environment: environment.clone(), image: image.into() };
    modal.call(json!({"op": "check"}), QUICK)?;
    super::set_secret("modal_key", key.trim())?;
    let mut options = BTreeMap::from([("tokenId".to_string(), token_id.to_string()), ("detail".to_string(), format!("Token {token_id}"))]);
    options.insert("image".into(), image.into());
    if let Some(environment) = environment {
        options.insert("detail".into(), format!("Token {token_id} · {environment}"));
        options.insert("environment".into(), environment);
    }
    let mut settings = super::settings();
    settings.providers.insert("modal".into(), options);
    super::save_settings(&settings)
}
pub fn disconnect() -> Result<()> {
    super::remove_secret("modal_key");
    let mut settings = super::settings();
    settings.providers.remove("modal");
    super::save_settings(&settings)
}
pub fn sandbox() -> Result<Box<dyn Sandbox>> {
    Ok(Box::new(Modal::connected()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake `modal` package that records what the helper asked for.
    const FAKE: &str = r#"
import os
calls = []
class App:
    app_id = "ap-1"
    @staticmethod
    def lookup(name, create_if_missing, environment_name):
        assert name == "pandacode"
        assert os.environ["MODAL_TOKEN_ID"] == "ak-1" and os.environ["MODAL_TOKEN_SECRET"] == "as-2"
        assert environment_name == "dev"
        with open(os.path.join(os.path.dirname(__file__), "lookups"), "a") as log:
            log.write("create\n" if create_if_missing else "read\n")
        return App()
class Image:
    @staticmethod
    def from_registry(name):
        assert name == "debian:bookworm-slim"
        return Image()
class _Out:
    def __init__(self, text): self.text = text
    def __iter__(self):
        for offset in range(0, len(self.text), 65536):
            yield self.text[offset:offset + 65536]
class _Process:
    def __init__(self, command):
        self.command = command
        self.returncode = None
    @property
    def stdout(self): return _Out("x" * (6 * 1024 * 1024) if self.command.endswith("large") else "ran " + self.command.split("\n", 1)[1])
    def wait(self): self.returncode = 7
class Sandbox:
    object_id = "sb-1"
    @staticmethod
    def create(*command, app, image, timeout, cpu, memory):
        assert command[:2] == ("sh", "-c") and "pandacode-deadline" in command[2] and timeout == 86400
        return Sandbox()
    @staticmethod
    def list(tags, app_id=None):
        assert app_id in (None, "ap-1") and tags == {"pandacode": "sandbox"}
        return [Sandbox()]
    @staticmethod
    def from_id(id):
        assert id == "sb-1"
        return Sandbox()
    def set_tags(self, tags):
        import time
        assert tags.pop("expires") == str(int(time.time()) + 86400) and tags == {"pandacode": "sandbox", "name": "box"}
    def get_tags(self):
        # Seconds left before the timeout, written by the test into `left` beside this package.
        import time
        left = os.path.join(os.path.dirname(__file__), "left")
        if not os.path.exists(left): return {"name": "box"}
        return {"name": "box", "expires": str(int(time.time()) + int(open(left).read()))}
    def poll(self): return None
    def terminate(self): pass
    def exec(self, *args, timeout):
        assert args[:2] == ("sh", "-c") and args[2].startswith("exec 2>&1\n")
        return _Process(args[2])
"#;

    fn fake(package: &str) -> Option<(Modal, tempfile_dir::Dir)> {
        // The fake interpreter is a `sh` wrapper; Windows has no shell script runner.
        if !cfg!(unix) {
            return None;
        }
        let python3 = crate::token_saver::find_tool("python3")?;
        let dir = tempfile_dir::Dir::new();
        std::fs::create_dir_all(dir.0.join("modal")).unwrap();
        std::fs::write(dir.0.join("modal/__init__.py"), package).unwrap();
        let wrapper = dir.0.join("python");
        std::fs::write(&wrapper, format!("#!/bin/sh\nexec '{}' -c 'import sys; sys.path.insert(0, \"{}\"); exec(sys.argv[-1])' \"$@\"\n", python3.display(), dir.0.display())).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&wrapper, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        Some((Modal { python: wrapper, token_id: "ak-1".into(), secret: "as-2".into(), environment: Some("dev".into()), image: DEFAULT_IMAGE.into() }, dir))
    }

    mod tempfile_dir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Self {
                let path = std::env::temp_dir().join(format!("pandacode-modal-{}", uuid::Uuid::new_v4()));
                std::fs::create_dir_all(&path).unwrap();
                Dir(path)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn the_helper_speaks_json_to_the_modal_package() {
        let Some((modal, dir)) = fake(FAKE) else { return };
        assert_eq!(modal.call(json!({"op": "check"}), QUICK).unwrap()["checked"], true);
        assert!(!dir.0.join("modal/lookups").exists(), "Connecting must not create an app");
        assert_eq!(modal.list().unwrap(), vec![machine("sb-1", "box")]);
        assert_eq!(std::fs::read_to_string(dir.0.join("modal/lookups")).unwrap(), "read\n");
        assert_eq!(modal.create("box").unwrap().id, "modal:sb-1");
        assert_eq!(std::fs::read_to_string(dir.0.join("modal/lookups")).unwrap(), "read\ncreate\n");
        assert!(modal.status("sb-1").unwrap().0);
        assert_eq!(modal.exec("sb-1", "echo hi", Duration::from_secs(5)).unwrap(), (Some(7), "ran echo hi".to_string()));
        modal.power("sb-1", "delete").unwrap();
        assert!(modal.power("sb-1", "start").is_err());
        assert!(modal.call(json!({"op": "nope", "id": "sb-1"}), QUICK).unwrap_err().contains("unknown request"));
    }

    #[test]
    fn old_sandboxes_refuse_leases_beyond_their_remaining_lifetime() {
        let Some((modal, dir)) = fake(FAKE) else { return };
        let left = |seconds: u32| std::fs::write(dir.0.join("modal/left"), seconds.to_string()).unwrap();
        left(4 * 3600);
        assert!(modal.status("sb-1").unwrap().0);
        let error = modal.keep_alive("sb-1", 5 * 60).unwrap_err();
        assert!(error.contains("expires in 239 minutes") || error.contains("expires in 240 minutes"), "{error}");
        left(29 * 60);
        assert!(modal.keep_alive("sb-1", 30).unwrap_err().contains("expires in"));
    }

    #[test]
    fn a_missing_modal_package_says_how_to_install_it() {
        let Some((modal, _dir)) = fake("raise ImportError('no modal')") else { return };
        assert_eq!(modal.call(json!({"op": "check"}), QUICK).unwrap_err(), INSTALL);
    }

    #[test]
    fn helper_retains_only_bounded_output_from_the_sdk_iterator() {
        let Some((modal, _dir)) = fake(FAKE) else { return };
        let (code, output) = modal.exec("sb-1", "large", Duration::from_secs(5)).unwrap();
        assert_eq!(code, Some(7));
        assert_eq!(output.len(), 4 * 1024 * 1024);
    }

}
