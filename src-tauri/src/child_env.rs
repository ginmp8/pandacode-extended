//! A Linux AppImage starts PandaCode with its own libraries and runtimes on the search paths
//! (`LD_LIBRARY_PATH`, `PYTHONHOME`, `GIO_*`, `GTK_*`, `XDG_DATA_DIRS`, `PATH`…), all pointing
//! into the mounted image. External tools must not inherit them: the system `/usr/bin/env`,
//! `curl` or `python3` then load the bundled copies and fail. Every spawn of an external
//! process applies `sanitize`, which drops those entries. The hardened build also
//! disables supported harness auto-updaters and nonessential telemetry on every OS.

use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Variables the AppImage runtime sets to describe itself.
const APPIMAGE_MARKERS: [&str; 4] = ["APPDIR", "APPIMAGE", "ARGV0", "OWD"];

/// Changes that undo the AppImage environment: `(name, Some(value))` sets, `(name, None)`
/// removes. Empty unless both `APPIMAGE` and an absolute `APPDIR` are present.
fn appimage_overrides(vars: &[(OsString, OsString)]) -> Vec<(OsString, Option<OsString>)> {
    let get = |name: &str| vars.iter().find(|(key, _)| key == name).map(|(_, value)| value);
    let (Some(_), Some(appdir)) = (get("APPIMAGE"), get("APPDIR")) else {
        return Vec::new();
    };
    let appdir = Path::new(appdir);
    if !appdir.is_absolute() || appdir.parent().is_none() {
        return Vec::new();
    }
    let inside = |entry: &OsStr| Path::new(entry).starts_with(appdir);
    vars.iter()
        .filter_map(|(key, value)| {
            if APPIMAGE_MARKERS.iter().any(|marker| key == marker) {
                return Some((key.clone(), None));
            }
            let entries: Vec<_> = std::env::split_paths(value).collect();
            if !entries.iter().any(|entry| inside(entry.as_os_str())) {
                return None;
            }
            let kept: Vec<_> = entries
                .into_iter()
                .filter(|entry| !entry.as_os_str().is_empty() && !inside(entry.as_os_str()))
                .collect();
            let joined = (!kept.is_empty()).then(|| std::env::join_paths(kept).ok()).flatten();
            Some((key.clone(), joined))
        })
        .collect()
}

pub(crate) trait EnvTarget {
    fn set_var(&mut self, key: &OsStr, value: &OsStr);
    fn remove_var(&mut self, key: &OsStr);
}

impl EnvTarget for std::process::Command {
    fn set_var(&mut self, key: &OsStr, value: &OsStr) {
        self.env(key, value);
    }
    fn remove_var(&mut self, key: &OsStr) {
        self.env_remove(key);
    }
}

impl EnvTarget for tokio::process::Command {
    fn set_var(&mut self, key: &OsStr, value: &OsStr) {
        self.env(key, value);
    }
    fn remove_var(&mut self, key: &OsStr) {
        self.env_remove(key);
    }
}

impl EnvTarget for portable_pty::CommandBuilder {
    fn set_var(&mut self, key: &OsStr, value: &OsStr) {
        self.env(key, value);
    }
    fn remove_var(&mut self, key: &OsStr) {
        self.env_remove(key);
    }
}

/// Documented opt-outs, applied to child processes only. These do not change the
/// user's global environment, authentication, proxy, model endpoint or tool PATH.
/// External CLIs may load settings after startup, so this is not an egress sandbox.
const HARNESS_PRIVACY: &[(&str, &str)] = &[
    ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
    ("DISABLE_AUTOUPDATER", "1"),
    ("DISABLE_TELEMETRY", "1"),
    ("DISABLE_ERROR_REPORTING", "1"),
    ("DO_NOT_TRACK", "1"),
    ("KIMI_CODE_NO_AUTO_UPDATE", "1"),
    ("KIMI_CLI_NO_AUTO_UPDATE", "1"),
    ("OPENCODE_DISABLE_AUTOUPDATE", "true"),
];

/// Disable supported background updates/telemetry, then remove AppImage runtime
/// paths on Linux. Does not remove credentials needed by user-selected agents.
pub(crate) fn sanitize(command: &mut impl EnvTarget) {
    if cfg!(target_os = "linux") {
        let vars: Vec<_> = std::env::vars_os().collect();
        for (key, value) in appimage_overrides(&vars) {
            match value {
                Some(value) => command.set_var(&key, &value),
                None => command.remove_var(&key),
            }
        }
    }
    for &(key, value) in HARNESS_PRIVACY {
        command.set_var(OsStr::new(key), OsStr::new(value));
    }
    // This Claude option would otherwise override the updater opt-out.
    command.remove_var(OsStr::new("FORCE_AUTOUPDATE_PLUGINS"));
}

#[cfg(test)]
mod hardened_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Environment(BTreeMap<OsString, Option<OsString>>);
    impl EnvTarget for Environment {
        fn set_var(&mut self, key: &OsStr, value: &OsStr) {
            self.0.insert(key.into(), Some(value.into()));
        }
        fn remove_var(&mut self, key: &OsStr) {
            self.0.insert(key.into(), None);
        }
    }

    #[test]
    fn suppresses_harness_auto_updates_without_removing_auth_or_changing_endpoints() {
        let mut env = Environment::default();
        for (key, value) in [
            ("DISABLE_AUTOUPDATER", "0"),
            ("FORCE_AUTOUPDATE_PLUGINS", "1"),
            ("ANTHROPIC_API_KEY", "synthetic-fixture"),
            ("ANTHROPIC_BASE_URL", "http://127.0.0.1:9999"),
        ] {
            env.set_var(OsStr::new(key), OsStr::new(value));
        }
        sanitize(&mut env);
        for &(key, value) in HARNESS_PRIVACY {
            assert_eq!(env.0.get(OsStr::new(key)), Some(&Some(OsString::from(value))));
        }
        assert_eq!(env.0.get(OsStr::new("FORCE_AUTOUPDATE_PLUGINS")), Some(&None));
        assert_eq!(env.0.get(OsStr::new("ANTHROPIC_API_KEY")), Some(&Some("synthetic-fixture".into())));
        assert_eq!(env.0.get(OsStr::new("ANTHROPIC_BASE_URL")), Some(&Some("http://127.0.0.1:9999".into())));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs.iter().map(|(key, value)| (key.into(), value.into())).collect()
    }

    fn overrides(pairs: &[(&str, &str)]) -> Vec<(String, Option<String>)> {
        appimage_overrides(&vars(pairs))
            .into_iter()
            .map(|(key, value)| {
                (key.into_string().unwrap(), value.map(|value| value.into_string().unwrap()))
            })
            .collect()
    }

    #[test]
    fn appimage_entries_are_removed_and_the_rest_is_kept() {
        let mount = "/tmp/.mount_pandacjIIGkD";
        let found = overrides(&[
            ("APPIMAGE", "/home/me/.local/bin/pandacode"),
            ("APPDIR", mount),
            ("ARGV0", "pandacode"),
            ("OWD", "/home/me"),
            ("PATH", "/tmp/.mount_pandacjIIGkD/usr/bin:/home/me/.local/bin:/usr/bin"),
            ("LD_LIBRARY_PATH", "/tmp/.mount_pandacjIIGkD/usr/lib:/tmp/.mount_pandacjIIGkD/lib"),
            ("PYTHONHOME", "/tmp/.mount_pandacjIIGkD/usr"),
            ("GDK_PIXBUF_MODULE_FILE", "/tmp/.mount_pandacjIIGkD/usr/lib/loaders.cache"),
            ("XDG_DATA_DIRS", "/tmp/.mount_pandacjIIGkD/usr/share:/usr/local/share:/usr/share"),
            // A sibling mount sharing the prefix is not inside APPDIR.
            ("OTHER", "/tmp/.mount_pandacjIIGkDx/lib"),
            ("HOME", "/home/me"),
        ]);
        assert_eq!(
            found,
            [
                ("APPIMAGE".into(), None),
                ("APPDIR".into(), None),
                ("ARGV0".into(), None),
                ("OWD".into(), None),
                ("PATH".into(), Some("/home/me/.local/bin:/usr/bin".into())),
                ("LD_LIBRARY_PATH".into(), None),
                ("PYTHONHOME".into(), None),
                ("GDK_PIXBUF_MODULE_FILE".into(), None),
                ("XDG_DATA_DIRS".into(), Some("/usr/local/share:/usr/share".into())),
            ]
        );
    }

    #[test]
    fn nothing_changes_outside_an_appimage() {
        let path = ("PATH", "/tmp/.mount_x/usr/bin:/usr/bin");
        assert!(overrides(&[path, ("HOME", "/home/me")]).is_empty());
        assert!(overrides(&[path, ("APPDIR", "/tmp/.mount_x")]).is_empty());
        assert!(overrides(&[path, ("APPIMAGE", "/a"), ("APPDIR", "relative")]).is_empty());
        assert!(overrides(&[path, ("APPIMAGE", "/a"), ("APPDIR", "/")]).is_empty());
    }
}
