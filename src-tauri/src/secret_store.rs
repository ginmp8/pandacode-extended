//! The OS secure store (Keychain, Secret Service, Credential Manager) for app secrets.
//! Values never enter prompts, logs, the database or the webview.

use serde::{de::DeserializeOwned, Serialize};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

#[cfg(test)]
fn memory() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static STORE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> = std::sync::OnceLock::new();
    STORE.get_or_init(Default::default)
}

pub(crate) fn set(service: &str, account: &str, value: &str) -> crate::Result<()> {
    let value = value.trim();
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err("That secret looks empty or malformed.".into());
    }
    #[cfg(test)]
    {
        memory().lock().unwrap().insert(format!("{service}/{account}"), value.to_owned());
        Ok(())
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        security_framework::passwords::set_generic_password(service, account, value.as_bytes()).map_err(|_| "Could not save the secret in Keychain.".into())
    }
    #[cfg(all(not(test), any(target_os = "linux", target_os = "windows")))]
    {
        keyring::Entry::new(service, account).and_then(|entry| entry.set_password(value)).map_err(|_| "Could not save the secret in secure storage.".into())
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
        memory().lock().unwrap().get(&format!("{service}/{account}")).cloned()
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        match security_framework::passwords::get_generic_password(service, account) {
            Ok(bytes) => String::from_utf8(bytes).ok(),
            // -25300 is "not found"; anything else (a denied or dismissed access prompt) is worth logging.
            Err(error) if error.code() == -25300 => None,
            Err(error) => {
                eprintln!("Keychain read for {service} failed ({}): {error}", error.code());
                None
            }
        }
    }
    #[cfg(all(not(test), any(target_os = "linux", target_os = "windows")))]
    {
        keyring::Entry::new(service, account).and_then(|entry| entry.get_password()).ok()
    }
    #[cfg(all(not(test), not(any(target_os = "macos", target_os = "linux", target_os = "windows"))))]
    {
        let _ = (service, account);
        None
    }
}

pub(crate) fn remove(service: &str, account: &str) {
    #[cfg(test)]
    {
        memory().lock().unwrap().remove(&format!("{service}/{account}"));
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        let _ = security_framework::passwords::delete_generic_password(service, account);
    }
    #[cfg(all(not(test), any(target_os = "linux", target_os = "windows")))]
    {
        if let Ok(entry) = keyring::Entry::new(service, account) {
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
