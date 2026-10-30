//! App secrets. On macOS they live in `secrets.json`, a 0600 JSON file in the app data dir
//! keyed `"<service>/<account>"`: PandaCode has no Apple Team ID, so a Keychain item is pinned
//! to the build that made it and every update asked for the login password on each read.
//! Items saved in the Keychain by earlier versions move to the file on first use, at most one
//! prompt per item, ever. Linux and Windows keep the OS store (Secret Service, Credential
//! Manager). Values never enter prompts, logs, the database or the webview.
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const FILE: &str = "secrets.json";
const LABEL: &str = "PandaCode secrets";

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Sets the app data dir once, at startup before anything reads a secret (and in the cloud
/// bridge process, which reads provider keys).
pub(crate) fn setup(dir: &Path) {
    let _ = DIR.set(dir.to_owned());
}

/// The dir given to `setup`, for child processes that read secrets.
pub(crate) fn dir() -> Option<&'static Path> {
    DIR.get().map(PathBuf::as_path)
}

fn key(service: &str, account: &str) -> String {
    format!("{service}/{account}")
}

/// Linux/Windows: `PANDACODE_SECRETS_FILE=1` keeps secrets in the same 0600 file macOS
/// uses. A server has no Secret Service agent to unlock, and Secret Service cannot answer
/// noninteractive reads; `pandacode serve` sets the variable itself. Off by default.
#[cfg(all(not(test), any(target_os = "linux", target_os = "windows")))]
fn file_mode() -> bool {
    std::env::var_os("PANDACODE_SECRETS_FILE").is_some_and(|value| value == "1")
}

use serde::{de::DeserializeOwned, Serialize};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

#[cfg(test)]
fn memory() -> &'static Mutex<std::collections::HashMap<String, String>> {
    static STORE: OnceLock<Mutex<std::collections::HashMap<String, String>>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

pub(crate) fn set(service: &str, account: &str, value: &str) -> crate::Result<()> {
    let value = value.trim();
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err("That secret looks empty or malformed.".into());
    }
    #[cfg(test)]
    {
        memory().lock().unwrap().insert(key(service, account), value.to_owned());
        Ok(())
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        file_set(app_dir()?, service, account, Some(value))
    }
    #[cfg(all(not(test), any(target_os = "linux", target_os = "windows")))]
    {
        if file_mode() {
            file_set(app_dir()?, service, account, Some(value))
        } else {
            keyring::Entry::new(service, account).and_then(|entry| entry.set_password(value)).map_err(|_| "Could not save the secret in secure storage.".into())
        }
    }
    #[cfg(all(not(test), not(any(target_os = "macos", target_os = "linux", target_os = "windows"))))]
    {
        let _ = (service, account, value);
        Err("Secure storage is not available on this platform.".into())
    }
}

pub(crate) fn get(service: &str, account: &str) -> Option<String> {
    #[cfg(test)]
    {
        memory().lock().unwrap().get(&key(service, account)).cloned()
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        let dir = app_dir().map_err(|error| eprintln!("{error}")).ok()?;
        file_get(dir, service, account, &legacy::Keychain)
    }
    #[cfg(all(not(test), any(target_os = "linux", target_os = "windows")))]
    {
        if file_mode() {
            let dir = app_dir().map_err(|error| eprintln!("{error}")).ok()?;
            file_get(dir, service, account, &legacy::NoStore)
        } else {
            keyring::Entry::new(service, account).and_then(|entry| entry.get_password()).ok()
        }
    }
    #[cfg(all(not(test), not(any(target_os = "macos", target_os = "linux", target_os = "windows"))))]
    {
        let _ = (service, account);
        None
    }
}

/// The file's value only: no Keychain access of any kind (background usage checks).
#[cfg(target_os = "macos")]
pub(crate) fn peek(service: &str, account: &str) -> Option<String> {
    #[cfg(test)]
    {
        get(service, account)
    }
    #[cfg(not(test))]
    {
        load::<Map>(&app_dir().ok()?.join(FILE), LABEL).ok()?.get(&key(service, account)).cloned()
    }
}

pub(crate) fn remove(service: &str, account: &str) {
    #[cfg(test)]
    {
        memory().lock().unwrap().remove(&key(service, account));
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        match app_dir().and_then(|dir| file_set(dir, service, account, None)) {
            Ok(()) => legacy::forget(&legacy::Keychain, service, account),
            Err(error) => eprintln!("{error}"),
        }
    }
    #[cfg(all(not(test), any(target_os = "linux", target_os = "windows")))]
    {
        if file_mode() {
            match app_dir().and_then(|dir| file_set(dir, service, account, None)) {
                Ok(()) => legacy::forget(&legacy::NoStore, service, account),
                Err(error) => eprintln!("{error}"),
            }
        } else if let Ok(entry) = keyring::Entry::new(service, account) {
            let _ = entry.delete_credential();
        }
    }
    #[cfg(all(not(test), not(any(target_os = "macos", target_os = "linux", target_os = "windows"))))]
    {
        let _ = (service, account);
    }
}

/// Serializes read-modify-write of every secrets file in this process.
static WRITE: Mutex<()> = Mutex::new(());

/// A JSON secrets file's contents. Missing means default; any other failure is an error.
pub(crate) fn load<T: DeserializeOwned + Default>(path: &Path, label: &str) -> crate::Result<T> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(error) => return Err(format!("Could not read {label} at {}: {error}", path.display())),
    };
    serde_json::from_str(&text).map_err(|error| format!("{label} at {} are corrupt: {error}", path.display()))
}

/// Replaces `dir/file` with `change(current)` atomically (0600 temp, fsync, rename) under this
/// process's lock and an advisory lock on `<name>.lock`, so app processes cannot interleave.
/// A default result deletes the file; a file that could not be read is never replaced.
pub(crate) fn update<T: Serialize + DeserializeOwned + Default + PartialEq>(dir: &Path, file: &str, label: &str, change: impl FnOnce(T) -> T) -> crate::Result<()> {
    let _guard = WRITE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let lock_name = format!("{}.lock", file.trim_end_matches(".json"));
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(lock_name))
        .and_then(|file| file.lock().map(|_| file))
        .map_err(|e| format!("Could not lock {label}: {e}"))?;
    let path = dir.join(file);
    let next = change(load(&path, label)?);
    let saved = if next == T::default() {
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(format!("Could not remove {label}: {error}")),
            _ => Ok(()),
        }
    } else {
        let text = serde_json::to_vec(&next).map_err(|e| e.to_string())?;
        write_atomic(dir, &path, file, &text).map_err(|e| format!("Could not save {label}: {e}"))
    };
    drop(lock);
    saved
}

fn write_atomic(dir: &Path, path: &Path, file: &str, bytes: &[u8]) -> std::io::Result<()> {
    let temp = dir.join(format!("{file}.{}.tmp", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options.open(&temp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    let renamed = written.and_then(|_| std::fs::rename(&temp, path));
    if renamed.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    renamed
}
