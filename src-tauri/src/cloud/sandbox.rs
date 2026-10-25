//! Explicit cloud machine management. No login upload, checkout synchronization,
//! runtime installation or unattended agent transport is part of this interface.
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use super::Machine;
use crate::Result;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Info { pub persistent: bool, pub max_lease_minutes: u32 }

pub trait Sandbox: Send + Sync {
    fn info(&self) -> Info;
    fn list(&self) -> Result<Vec<Machine>>;
    fn create(&self, name: &str) -> Result<Machine>;
    fn power(&self, native: &str, action: &str) -> Result<()>;
    fn exec(&self, native: &str, command: &str, timeout: Duration) -> Result<(Option<i32>, String)>;
    /// Only called by an explicit, approval-gated lease request.
    fn keep_alive(&self, native: &str, minutes: u32) -> Result<()>;
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub start: bool,
    pub stop: bool,
    pub delete: bool,
    pub exec: bool,
    pub lease: bool,
}

pub fn capabilities(provider: &str) -> Capabilities {
    let known = super::PROVIDERS.contains(&provider);
    Capabilities {
        start: known && !matches!(provider, "modal" | "vercel" | "agentcore"),
        stop: known && !matches!(provider, "modal" | "vercel" | "blaxel"),
        delete: known,
        exec: known,
        lease: matches!(provider, "e2b" | "modal" | "vercel" | "codesandbox" | "runloop" | "freestyle" | "boat" | "instabox"),
    }
}

pub const PROVIDERS: [&str; 11] = ["gcloud", "e2b", "modal", "vercel", "codesandbox", "runloop", "blaxel", "tensorlake", "freestyle", "boat", "instabox"];
pub const KEYED: [&str; 9] = ["e2b", "modal", "codesandbox", "runloop", "blaxel", "tensorlake", "freestyle", "boat", "instabox"];

pub fn connected(provider: &str) -> Result<Box<dyn Sandbox>> {
    match provider {
        "gcloud" => super::gcloud_vm::sandbox(),
        "e2b" => super::e2b::sandbox(),
        "modal" => super::modal::sandbox(),
        "vercel" => super::vercel_sandbox::sandbox(),
        "codesandbox" => super::codesandbox::sandbox(),
        "runloop" => super::runloop::sandbox(),
        "blaxel" => super::blaxel::sandbox(),
        "tensorlake" => super::tensorlake::sandbox(),
        "freestyle" => super::freestyle::sandbox(),
        "boat" => super::boat::sandbox(),
        "instabox" => super::instabox::sandbox(),
        _ => Err("Unknown cloud sandbox provider.".into()),
    }
}

pub fn connect(provider: &str, key: &str, options: &Value) -> Result<()> {
    if key.trim().is_empty() || key.len() > 16 * 1024 || key.chars().any(char::is_control) { return Err("Enter a valid cloud API credential.".into()); }
    let result = match provider {
        "e2b" => super::e2b::connect(key, options),
        "modal" => super::modal::connect(key, options),
        "codesandbox" => super::codesandbox::connect(key, options),
        "runloop" => super::runloop::connect(key, options),
        "blaxel" => super::blaxel::connect(key, options),
        "tensorlake" => super::tensorlake::connect(key, options),
        "freestyle" => super::freestyle::connect(key, options),
        "boat" => super::boat::connect(key, options),
        "instabox" => super::instabox::connect(key, options),
        _ => Err("Unknown cloud sandbox provider.".into()),
    };
    result.map_err(|error| super::http::redact(&error, &[key]))
}
pub fn disconnect(provider: &str) -> Result<()> {
    match provider {
        "e2b" => super::e2b::disconnect(),
        "modal" => super::modal::disconnect(),
        "codesandbox" => super::codesandbox::disconnect(),
        "runloop" => super::runloop::disconnect(),
        "blaxel" => super::blaxel::disconnect(),
        "tensorlake" => super::tensorlake::disconnect(),
        "freestyle" => super::freestyle::disconnect(),
        "boat" => super::boat::disconnect(),
        "instabox" => super::instabox::disconnect(),
        _ => Err("Unknown cloud sandbox provider.".into()),
    }
}

/// IDs are data in URL path segments and CLI arguments, never flags or path traversal.
pub fn validate_id(id: &str) -> Result<()> {
    if !id.is_empty() && id.len() <= 200 && !id.starts_with(['-', '.']) && !id.contains("..") && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
        Ok(())
    } else { Err("Invalid cloud machine identifier.".into()) }
}
pub fn validate_name(name: &str) -> Result<()> {
    if !name.is_empty() && name.len() <= 63 && name.starts_with(|c: char| c.is_ascii_lowercase()) && !name.ends_with('-') && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
        Ok(())
    } else { Err("Use a cloud machine name of up to 63 lowercase letters, digits and dashes, starting with a letter.".into()) }
}

pub(super) fn download_command(path: &str, offset: u64, max: usize) -> String {
    format!("[ -f {p} ] && tail -c +{} {p} 2>/dev/null | head -c {max} | base64 2>/dev/null | tr -d '\\n'", offset + 1, p = super::shell_quote(path))
}
pub fn checked((code, _output): (Option<i32>, String)) -> Result<()> {
    if code == Some(0) { Ok(()) } else { Err("The cloud management command failed.".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destructive_stop_is_never_advertised_as_pause() {
        for provider in ["modal", "vercel"] {
            let caps = capabilities(provider);
            assert!(!caps.start && !caps.stop && caps.delete && caps.exec);
        }
        assert!(!capabilities("blaxel").stop);
        assert!(capabilities("instabox").lease);
        assert_eq!(capabilities("unknown"), Capabilities { start: false, stop: false, delete: false, exec: false, lease: false });
    }
    #[test]
    fn path_and_flag_identifiers_are_rejected() {
        for id in ["", "../secret", "--project=other", "a/b", "a?key=x", "a#x", "a\\b", "a\n"] { assert!(validate_id(id).is_err(), "{id}"); }
        for id in ["sb_123", "my-vm.us-central1-a", "123-abc"] { assert!(validate_id(id).is_ok()); }
        assert!(validate_name("pandacode-test").is_ok());
        assert!(validate_name("../other").is_err());
    }
}
