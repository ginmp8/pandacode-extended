//! PandaCode Connect (docs/remote-access.md): sign-in, one end-to-end encrypted relay
//! socket, host RPC for this computer's tasks, and mirrors: local tasks that run on
//! another of the user's computers. Nothing touches the network until the user signs in.
mod auth;
mod bench;
pub(crate) mod catalog;
mod client;
mod config;
mod crypto;
pub(crate) mod diagnostics;
pub(crate) mod grants;
mod host;
mod images;
mod permissions;
pub(crate) mod mirror;
mod replay;
pub(crate) mod routes;
mod socket;
mod start;

pub(crate) use mirror::route;
pub(crate) use host::{check_current_request, is_remote_request};

use crate::{secret_store, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;
use tauri::{Emitter, Manager};
use x25519_dalek::StaticSecret;

/// A device on the account, as the relay lists it and presence updates it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Device {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub host: bool,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub last_seen: Value,
}

impl Device {
    fn view(&self, me: &str, settings: &config::Settings) -> Value {
        json!({
            "id": self.id, "name": self.name, "platform": self.platform,
            "fingerprint": crypto::fingerprint(&self.public_key),
            "host": self.host, "online": self.online, "lastSeen": self.last_seen, "current": self.id == me,
            "trusted": settings.is_trusted(&self.id, &self.public_key),
        })
    }
}

type Reply = tokio::sync::oneshot::Sender<Result<Value>>;

const UNKNOWN_DEVICE: &str = "That computer is not on this account.";

/// A trusted device mirroring some of this computer's tasks, and the events waiting
/// for its next frame.
#[derive(Default)]
pub(crate) struct Subscriber {
    until: Option<Instant>,
    sessions: std::collections::HashSet<String>,
    events: Vec<Value>,
    finished: Vec<Value>,
}

#[derive(Default)]
pub(crate) struct Inner {
    settings: config::Settings,
    /// A failed permission transaction also denies the running process when the
    /// filesystem could not record the durable fence. Only a complete local
    /// sign-out may recover an earlier failed transaction.
    access_locked_down: bool,
    token: Option<String>,
    /// Signed in, but the keychain refused the token read at startup; retried in the background.
    unreadable: bool,
    connected: bool,
    /// Bumped on every start/stop so an old socket task knows to exit.
    generation: u64,
    /// Frames waiting for the socket, bounded so a stalled socket can't hoard memory.
    outbox: Option<tokio::sync::mpsc::Sender<String>>,
    /// The account's devices, as the latest presence frame lists them.
    devices: Vec<Device>,
    /// Whether presence arrived on this socket; until then `remote_devices` asks the relay.
    presence: bool,
    /// Outstanding calls to hosts: request id → (device id, reply).
    pending: HashMap<String, (String, Reply)>,
    /// Pairing responses can arrive before trust, but only for the exact request and key.
    pairing_pending: HashMap<String, (String, String)>,
    subscribers: HashMap<String, Subscriber>,
    /// A flush of subscriber events is scheduled.
    flushing: bool,
    flushed_at: Option<Instant>,
    flush_frames: u32,
    journal: host::Journal,
    viewer: mirror::Viewer,
    replay: replay::Replay,
    /// Pair keys: device id → (the public key it was derived from, key).
    keys: HashMap<String, (String, crypto::Key)>,
    flow: Option<auth::Flow>,
    /// Dropping this cancels the device-flow poll in `remote_sign_in_finish`.
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    /// Other computers' harnesses and models, for their mirrors' model pickers.
    catalog: catalog::Cache,
    /// Bounded image uploads from authenticated peers.
    uploads: images::Uploads,
}

impl Inner {
    fn access_available(&self, dir: &std::path::Path) -> Result<()> {
        if self.access_locked_down || config::locked_down(dir) {
            return Err(config::ACCESS_LOCKED.into());
        }
        Ok(())
    }

    fn deny_access(&mut self) {
        self.access_locked_down = true;
        self.settings.host_enabled = false;
        self.settings.allow_full_access = false;
        self.settings.trusted.clear();
        self.subscribers.clear();
        self.uploads = Default::default();
        self.catalog = Default::default();
        self.keys.clear();
        self.fail_pending(None, config::ACCESS_LOCKED);
    }

    fn fail_pending(&mut self, device: Option<&str>, error: &str) {
        let ids: Vec<String> = self.pending.iter().filter(|(_, (d, _))| device.is_none_or(|device| d == device)).map(|(id, _)| id.clone()).collect();
        for id in ids {
            self.pairing_pending.remove(&id);
            if let Some((_, reply)) = self.pending.remove(&id) {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }

    /// An undeliverable frame fails the call it carried, or every call to `to` without a ref.
    fn fail_ref(&mut self, to: &str, reference: Option<&str>, error: &str) {
        let Some(id) = reference else { return self.fail_pending(Some(to), error) };
        if self.pending.get(id).is_some_and(|(device, _)| device == to) {
            self.pairing_pending.remove(id);
            if let Some((_, reply)) = self.pending.remove(id) {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }

    /// Calls and task content go only to a device trusted here with its current key.
    fn callable(&self, device_id: &str) -> Result<()> {
        if self.access_locked_down { return Err(config::ACCESS_LOCKED.into()); }
        let device = self.devices.iter().find(|d| d.id == device_id).ok_or(UNKNOWN_DEVICE)?;
        if !self.settings.is_trusted(device_id, &device.public_key) {
            let name = if device.name.is_empty() { "That computer" } else { &device.name };
            return Err(format!("{name} isn't trusted on this computer yet. Trust it in Settings → Remote access."));
        }
        Ok(())
    }

    /// Revalidate the key that authenticated this message, not a cached trust
    /// decision. Queued requests and in-flight replies can outlive revocation.
    fn authenticated(&self, device_id: &str, received_key: &str) -> bool {
        !self.access_locked_down && self.settings.is_trusted(device_id, received_key)
            && self.devices.iter().any(|device| device.id == device_id && device.public_key == received_key)
    }

    /// Incoming events and ordinary results always require current pinned trust.
    /// The one exception is a response to an explicitly initiated pairing-code
    /// redemption, bound to its request ID, device ID and unchanged presence key.
    fn allows_incoming(&self, from: &str, key: &str, kind: &str, id: Option<&str>) -> bool {
        if self.access_locked_down { return false; }
        if self.authenticated(from, key) { return true; }
        if kind != "res" { return false; }
        id.is_some_and(|id| {
            self.pending.get(id).is_some_and(|(device, _)| device == from)
                && self.pairing_pending.get(id).is_some_and(|(device, expected)| device == from && expected == key)
                && self.devices.iter().any(|device| device.id == from && device.public_key == key && !key.is_empty())
        })
    }

    /// Forgets the sign-in and stops the socket. With `used`, only while that is still the
    /// token: a refusal of an old token must not sign out a newer sign-in.
    fn clear_sign_in(&mut self, used: Option<&str>) -> bool {
        if used.is_some_and(|used| self.token.as_deref() != Some(used)) {
            return false;
        }
        self.generation += 1;
        self.outbox = None;
        self.connected = false;
        self.fail_pending(None, "Signed out of PandaCode Connect.");
        self.token = None;
        self.unreadable = false;
        self.settings.account = None;
        self.devices.clear();
        self.presence = false;
        self.subscribers.clear();
        self.viewer = Default::default();
        self.uploads = Default::default();
        self.catalog = Default::default();
        true
    }
}

pub(crate) struct Remote {
    dir: PathBuf,
    accounts: config::Accounts,
    inner: Mutex<Inner>,
    /// The device key, separate from `inner` so a slow keychain prompt blocks nothing else.
    key: Mutex<Option<StaticSecret>>,
}

impl Remote {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn key(&self) -> MutexGuard<'_, Option<StaticSecret>> {
        self.key.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn save(&self, _settings: &config::Settings) -> Result<()> {
        // Never save an out-of-date snapshot: the persisted state must be the
        // one guarded by the Remote lock, and errors must reach callers.
        config::save(&self.dir, &self.lock().settings)
    }

    /// The device's X25519 key from the keychain. Never creates one.
    fn secret(&self) -> Result<StaticSecret> {
        let mut key = self.key();
        if let Some(secret) = key.as_ref() {
            return Ok(secret.clone());
        }
        let secret = config::read_key(&self.accounts.key)?.and_then(|text| crypto::decode_secret(&text)).ok_or("This computer has no PandaCode Connect key. Sign in again.")?;
        *key = Some(secret.clone());
        Ok(secret)
    }

    /// Reads or makes the key at sign-in, behind the key lock so two sign-ins cannot race.
    fn ensure_secret(&self) -> Result<StaticSecret> {
        let mut key = self.key();
        if let Some(secret) = key.as_ref() {
            return Ok(secret.clone());
        }
        let secret = config::load_or_create_key(config::read_key(&self.accounts.key), |text| secret_store::set(config::SERVICE, &self.accounts.key, text))?;
        *key = Some(secret.clone());
        Ok(secret)
    }

    fn device_id(&self) -> String {
        self.lock().settings.device_id.clone()
    }

    fn status(&self) -> Value {
        let signed_in = self.lock().token.is_some();
        let cached = self.key().as_ref().map(|secret| crypto::fingerprint(&crypto::public_key(secret)));
        let fingerprint = cached.or_else(|| signed_in.then(|| self.secret().map(|secret| crypto::fingerprint(&crypto::public_key(&secret))).ok()).flatten()).unwrap_or_default();
        let inner = self.lock();
        let mut status = json!({
            "configured": config::build().is_some(),
            "signedIn": inner.token.is_some() || inner.unreadable,
            "device": {"id": inner.settings.device_id, "name": config::device_name(), "fingerprint": fingerprint},
            "hostEnabled": inner.settings.host_enabled,
            "allowFullAccess": inner.settings.allow_full_access,
            "directRoutes": routes::enabled(&self.dir),
            "connected": inner.connected,
        });
        if let Some(account) = &inner.settings.account {
            status["account"] = json!({"login": account.login, "avatarUrl": account.avatar_url});
        }
        status
    }

    fn devices_view(&self) -> Vec<Value> {
        let inner = self.lock();
        inner.devices.iter().map(|device| device.view(&inner.settings.device_id, &inner.settings)).collect()
    }
}

fn remote(app: &tauri::AppHandle) -> tauri::State<'_, Remote> {
    app.state::<Remote>()
}

fn emit_status(app: &tauri::AppHandle) {
    let _ = app.emit("remote://status", remote(app).status());
}

fn emit_presence(app: &tauri::AppHandle) {
    let _ = app.emit("remote://presence", remote(app).devices_view());
}

/// Loads settings and starts the socket when already signed in. No network otherwise.
pub(crate) fn setup(app: &tauri::AppHandle, dir: &std::path::Path) {
    let settings = config::load(dir);
    let accounts = config::accounts(dir);
    // Only a signed-in computer reads the keychain at startup.
    let (startup, key) = if settings.account.is_some() { signed_in_start(dir, &accounts, &settings.device_id) } else { (config::Startup::SignedOut, None) };
    let unreadable = startup == config::Startup::Unreadable;
    let token = match startup {
        config::Startup::SignedIn(token) => Some(token),
        _ => None,
    };
    let signed_in = token.is_some();
    let replay = replay::Replay::open(dir).unwrap_or_else(|error| {
        eprintln!("{error}");
        // No in-memory fallback: requests, responses and events stay refused.
        replay::Replay::default()
    });
    app.manage(Remote { dir: dir.to_owned(), accounts, inner: Mutex::new(Inner { settings, access_locked_down: config::locked_down(dir), token, unreadable, replay, ..Default::default() }), key: Mutex::new(key) });
    if signed_in && config::build().is_some() {
        socket::start(app);
    }
    if unreadable {
        tauri::async_runtime::spawn(retry_token(app.clone()));
    }
}

/// Reads the token, first adopting the pre-0.3.20 unscoped items once (one attempt per
/// launch) when the scoped ones are missing. Returns the adopted key so it is used even if
/// writing it failed.
fn signed_in_start(dir: &std::path::Path, accounts: &config::Accounts, device_id: &str) -> (config::Startup, Option<StaticSecret>) {
    let now = chrono::Utc::now().timestamp();
    let token = config::read_key(&accounts.token);
    let key = config::read_key(&accounts.key);
    let legacy = config::legacy_accounts();
    let read_legacy = || (config::read_key(&legacy.token).ok().flatten(), config::read_key(&legacy.key).ok().flatten());
    let Some(adopted) = config::adoption(dir, device_id, now, &token, &key, read_legacy) else { return (config::startup(token, now), None) };
    let mut saved = secret_store::set(config::SERVICE, &accounts.token, &adopted.token);
    let secret = adopted.key.as_deref().and_then(crypto::decode_secret);
    if let Some(text) = &adopted.key {
        saved = saved.and(secret_store::set(config::SERVICE, &accounts.key, text));
    }
    match saved {
        Ok(()) => {
            secret_store::remove(config::SERVICE, &legacy.token);
            secret_store::remove(config::SERVICE, &legacy.key);
        }
        Err(error) => eprintln!("Could not move the PandaCode Connect sign-in to this install's Keychain items: {error}"),
    }
    (config::Startup::SignedIn(adopted.token), secret)
}

/// After a keychain error at startup: reads the token again at 5 s, 15 s and 60 s. A token
/// starts the socket; a real not-found signs out; errors keep the account as it is.
async fn retry_token(app: tauri::AppHandle) {
    for wait in [5, 15, 60] {
        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        let state = remote(&app);
        if !state.lock().unreadable {
            return;
        }
        let startup = config::startup(config::read_key(&state.accounts.token), chrono::Utc::now().timestamp());
        if startup == config::Startup::Unreadable {
            continue;
        }
        let signed_in = {
            let mut inner = state.lock();
            // A sign-in or sign-out meanwhile settled it.
            if !inner.unreadable {
                return;
            }
            inner.unreadable = false;
            match startup {
                config::Startup::SignedIn(token) => inner.token = Some(token),
                _ => inner.token = None,
            }
            inner.token.is_some()
        };
        if signed_in && config::build().is_some() {
            socket::start(&app);
        }
        emit_status(&app);
        return;
    }
}

/// Cross-file revocation: disable effective permission immediately under Remote's
/// lock, record a denial fence, save the legacy state, update scoped grants, then
/// remove the fence. An interrupted step never claims success or reopens on boot.
fn persist_scoped_revocation(
    dir: &std::path::Path,
    inner: &mut Inner,
    next: config::Settings,
    recover: bool,
    update_scopes: impl FnOnce(&std::path::Path) -> Result<()>,
) -> Result<()> {
    let earlier_lockdown = inner.access_locked_down || config::locked_down(dir);
    inner.access_locked_down = true;
    inner.settings = next.clone();
    let result = (|| {
        config::begin_lockdown(dir)?;
        config::save(dir, &next)?;
        update_scopes(dir)?;
        if recover || !earlier_lockdown { config::end_lockdown(dir)?; }
        Ok(())
    })();
    if result.is_err() {
        inner.deny_access();
        // In particular, restore a fence if removing it succeeded but syncing
        // the directory failed. An unwritable filesystem remains an error.
        let _ = config::begin_lockdown(dir);
    } else {
        inner.access_locked_down = earlier_lockdown && !recover;
    }
    result
}

/// Single-file permission changes share the same in-memory denial as scoped
/// transactions. An unrelated successful revoke cannot clear an earlier fence.
fn update_access(dir: &std::path::Path, inner: &mut Inner, next: config::Settings, revoke: bool) -> Result<()> {
    if !revoke { inner.access_available(dir)?; }
    let earlier_lockdown = inner.access_locked_down || config::locked_down(dir);
    inner.access_locked_down = true;
    let result = config::update_access(dir, &mut inner.settings, next, revoke);
    if result.is_err() {
        inner.deny_access();
        let _ = config::begin_lockdown(dir);
    } else {
        inner.access_locked_down = earlier_lockdown || config::locked_down(dir);
    }
    result
}

/// Forget authorization in memory immediately, and persist a denial fence before
/// returning to a local UI. A failed write is an error, never a successful sign-out.
fn sign_out_local_checked(app: &tauri::AppHandle, used: Option<&str>) -> Result<()> {
    let state = remote(app);
    let result = {
        let mut inner = state.lock();
        if !inner.clear_sign_in(used) { return Ok(()); }
        let mut next = inner.settings.clone();
        next.account = None;
        next.host_enabled = false;
        next.allow_full_access = false;
        next.trusted.clear();
        persist_scoped_revocation(&state.dir, &mut inner, next, true, grants::clear_on_sign_out)
    };
    secret_store::remove(config::SERVICE, &state.accounts.token);
    emit_status(app);
    emit_presence(app);
    result
}

/// Network-triggered sign-outs have no UI result to return; keep the denial
/// active and record failures instead of ignoring them.
fn sign_out_local(app: &tauri::AppHandle, used: Option<&str>) {
    if let Err(error) = sign_out_local_checked(app, used) {
        eprintln!("Could not persist the Connect sign-out: {error}");
    }
}

/// Hooked into every agent event: forwards it to devices mirroring that task.
/// Allocates nothing while hosting is off or nobody mirrors the task.
pub(crate) fn agent_event(app: &tauri::AppHandle, event: &crate::providers::AgentEvent) {
    host::agent_event(app, event);
}

/// Hooked into every run end, with the text of its `done` event.
pub(crate) fn run_finished(app: &tauri::AppHandle, session_id: &str, successful: bool, stopped: bool, text: &str) {
    host::run_finished(app, session_id, successful, stopped, text);
}

/// Device names as presence last listed them, for mirrors' labels and errors.
fn names() -> &'static Mutex<HashMap<String, String>> {
    static NAMES: std::sync::OnceLock<Mutex<HashMap<String, String>>> = std::sync::OnceLock::new();
    NAMES.get_or_init(Default::default)
}

/// A device's name, or a neutral label before presence names it.
pub(crate) fn device_label(id: &str) -> String {
    names().lock().unwrap_or_else(|e| e.into_inner()).get(id).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| "another computer".into())
}

/// Remembers the names in a device list; true if any changed.
fn remember_names(devices: &[Device]) -> bool {
    let mut names = names().lock().unwrap_or_else(|e| e.into_inner());
    let mut changed = false;
    for device in devices {
        if names.get(&device.id) != Some(&device.name) {
            names.insert(device.id.clone(), device.name.clone());
            changed = true;
        }
    }
    changed
}

fn valid_device_id(id: &str) -> Result<()> {
    host::valid_id(id).map_err(|_| "Unknown computer.".into())
}

#[tauri::command]
pub async fn remote_status(app: tauri::AppHandle) -> Value {
    remote(&app).status()
}

/// Stops any device-flow poll and forgets its code.
fn cancel_sign_in(app: &tauri::AppHandle) {
    let state = remote(app);
    let mut inner = state.lock();
    inner.flow = None;
    inner.cancel = None;
}

#[tauri::command]
pub async fn remote_sign_in_start(app: tauri::AppHandle) -> Result<Value> {
    let build = config::build().ok_or("Remote access is not configured in this build.")?;
    cancel_sign_in(&app);
    let (flow, view) = auth::start(&build).await?;
    remote(&app).lock().flow = Some(flow);
    Ok(view)
}

#[tauri::command]
pub async fn remote_sign_in_cancel(app: tauri::AppHandle) -> Value {
    cancel_sign_in(&app);
    remote(&app).status()
}

#[tauri::command]
pub async fn remote_sign_in_finish(app: tauri::AppHandle) -> Result<Value> {
    let build = config::build().ok_or("Remote access is not configured in this build.")?;
    let state = remote(&app);
    let (flow, cancelled) = {
        let mut inner = state.lock();
        let flow = inner.flow.take().ok_or("Start signing in first.")?;
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        inner.cancel = Some(cancel);
        (flow, cancelled)
    };
    let public_key = crypto::public_key(&state.ensure_secret()?);
    let github_token = tokio::select! {
        token = auth::github_token(&build, flow) => token?,
        _ = cancelled => return Err("Sign-in was cancelled.".into()),
    };
    let device = |id: String| json!({"id": id, "name": config::device_name(), "platform": std::env::consts::OS, "publicKey": public_key});
    let (token, account) = match auth::register(&build, &github_token, device(state.device_id())).await {
        // This id is registered with another key (a restored keychain, say): become a new device once.
        Err(error) if error == auth::KEY_CHANGED => {
            let settings = {
                let mut inner = state.lock();
                inner.settings.device_id = uuid::Uuid::new_v4().to_string();
                inner.settings.clone()
            };
            state.save(&settings)?;
            auth::register(&build, &github_token, device(settings.device_id)).await?
        }
        other => other?,
    };
    if config::parse_token(&token).is_none_or(|claims| claims.dev != state.device_id()) {
        return Err("The relay returned an unexpected sign-in token.".into());
    }
    secret_store::set(config::SERVICE, &state.accounts.token, &token)?;
    let settings = {
        let mut inner = state.lock();
        inner.token = Some(token);
        inner.unreadable = false;
        inner.settings.account = Some(account);
        inner.settings.clone()
    };
    state.save(&settings)?;
    socket::start(&app);
    emit_status(&app);
    Ok(state.status())
}

#[tauri::command]
pub async fn remote_sign_out(app: tauri::AppHandle) -> Result<Value> {
    let (token, me) = {
        let state = remote(&app);
        let inner = state.lock();
        (inner.token.clone(), inner.settings.device_id.clone())
    };
    // A slow or disconnected relay must not keep remote control active while
    // the user is signing out. Deny in memory and persist before any await.
    sign_out_local_checked(&app, None)?;
    if let (Some(build), Some(token)) = (config::build(), token) {
        let _ = auth::relay(&build, &token, reqwest::Method::DELETE, &format!("/v1/devices/{me}")).await;
    }
    Ok(remote(&app).status())
}

#[tauri::command]
pub async fn remote_set_host_enabled(app: tauri::AppHandle, enabled: bool) -> Result<Value> {
    let state = remote(&app);
    let result = {
        let mut inner = state.lock();
        let mut next = inner.settings.clone();
        next.host_enabled = enabled;
        let result = update_access(&state.dir, &mut inner, next, !enabled);
        if !enabled {
            inner.subscribers.clear();
            inner.uploads = Default::default();
        }
        if !enabled || result.is_ok() {
            if let Some(outbox) = &inner.outbox {
                let _ = outbox.try_send(json!({"kind": "host", "enabled": enabled}).to_string());
            }
        }
        result
    };
    emit_status(&app);
    result?;
    Ok(state.status())
}

/// Local IPC only: never part of the remote host RPC method allowlist.
#[tauri::command]
pub async fn remote_set_full_access(app: tauri::AppHandle, enabled: bool) -> Result<Value> {
    let state = remote(&app);
    let result = {
        let mut inner = state.lock();
        let mut settings = inner.settings.clone();
        settings.allow_full_access = enabled;
        if enabled {
            update_access(&state.dir, &mut inner, settings, false)
        } else {
            persist_scoped_revocation(&state.dir, &mut inner, settings, false, grants::downgrade_full_access)
        }
    };
    emit_status(&app);
    result?;
    Ok(state.status())
}

#[tauri::command]
pub async fn remote_devices(app: tauri::AppHandle) -> Result<Vec<Value>> {
    // Presence carries the list; the relay is asked only before the first presence frame.
    if !remote(&app).lock().presence {
        socket::refresh_devices(&app).await?;
    }
    Ok(remote(&app).devices_view())
}

/// The fingerprint is captured when the user views the key. A different
/// authenticated presence key must never be pinned by a stale approval.
pub(crate) fn verified_fingerprint(public_key: &str, expected: Option<&str>) -> Result<()> {
    let actual = crypto::fingerprint(public_key);
    if actual.is_empty() || expected.is_none_or(|value| value.len() != 32 || !crypto::same(value, &actual)) {
        return Err("This computer's key changed or has not been verified. Compare the new fingerprint before approving.".into());
    }
    Ok(())
}

/// Pins (or forgets) a device's current public key, for both directions.
#[tauri::command]
pub async fn remote_set_trusted(app: tauri::AppHandle, device_id: String, trusted: bool, expected_fingerprint: Option<String>) -> Result<Vec<Value>> {
    valid_device_id(&device_id)?;
    if trusted && !remote(&app).lock().presence {
        socket::refresh_devices(&app).await?;
    }
    let state = remote(&app);
    let result = {
        let mut inner = state.lock();
        if device_id == inner.settings.device_id {
            return Err("That is this computer.".into());
        }
        let mut next = inner.settings.clone();
        if trusted {
            let key = inner.devices.iter().find(|d| d.id == device_id).map(|d| d.public_key.clone()).ok_or(UNKNOWN_DEVICE)?;
            verified_fingerprint(&key, expected_fingerprint.as_deref())?;
            next.trusted.insert(device_id.clone(), key);
        } else {
            next.trusted.remove(&device_id);
        }
        let result = if trusted {
            update_access(&state.dir, &mut inner, next, false)
        } else {
            inner.fail_pending(Some(&device_id), "This computer is no longer trusted.");
            persist_scoped_revocation(&state.dir, &mut inner, next, false, |dir| grants::revoke_legacy_trust(dir, &device_id))
        };
        if !trusted {
            inner.subscribers.remove(&device_id);
            inner.uploads.forget(&device_id);
            inner.catalog.forget(&device_id);
            inner.keys.remove(&device_id);
        }
        result
    };
    emit_presence(&app);
    result?;
    Ok(state.devices_view())
}

#[tauri::command]
pub async fn remote_revoke_device(app: tauri::AppHandle, device_id: String) -> Result<Value> {
    valid_device_id(&device_id)?;
    let build = config::build().ok_or("Remote access is not configured in this build.")?;
    let (token, me) = {
        let state = remote(&app);
        let inner = state.lock();
        (inner.token.clone().ok_or("Sign in to PandaCode Connect first.")?, inner.settings.device_id.clone())
    };
    // Revoke locally before network I/O. A slow relay can no longer prolong
    // the device's active authority on this computer.
    let local_result = if device_id == me {
        sign_out_local_checked(&app, Some(&token))
    } else {
        let state = remote(&app);
        let result = {
            let mut inner = state.lock();
            let mut next = inner.settings.clone();
            next.trusted.remove(&device_id);
            inner.fail_pending(Some(&device_id), "This computer was revoked.");
            let result = persist_scoped_revocation(&state.dir, &mut inner, next, false, |dir| grants::revoke_legacy_trust(dir, &device_id));
            inner.devices.retain(|device| device.id != device_id);
            inner.subscribers.remove(&device_id);
            inner.uploads.forget(&device_id);
            inner.keys.remove(&device_id);
            inner.catalog.forget(&device_id);
            result
        };
        emit_presence(&app);
        result
    };
    let remote_result = auth::relay(&build, &token, reqwest::Method::DELETE, &format!("/v1/devices/{device_id}")).await;
    if remote_result.as_ref().is_err_and(|error| error == auth::SIGNED_OUT) {
        sign_out_local(&app, Some(&token));
    }
    local_result?;
    remote_result?;
    Ok(json!({}))
}

/// Computers a task can run on: trusted here (and so, symmetrically, trusting this one)
/// with hosting on.
#[tauri::command]
pub async fn remote_targets(app: tauri::AppHandle) -> Vec<Value> {
    let state = remote(&app);
    let inner = state.lock();
    targets(&inner)
}

fn targets(inner: &Inner) -> Vec<Value> {
    inner
        .devices
        .iter()
        .filter(|d| d.id != inner.settings.device_id && d.host && inner.settings.is_trusted(&d.id, &d.public_key))
        .map(|d| json!({"deviceId": d.id, "name": d.name, "platform": d.platform, "online": d.online}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str, key: &str) -> Device {
        Device { id: id.into(), name: name.into(), platform: String::new(), public_key: key.into(), host: true, online: true, last_seen: Value::Null }
    }

    #[test]
    fn undeliverable_fails_only_the_call_it_names() {
        let mut inner = Inner::default();
        let (a, mut ra) = tokio::sync::oneshot::channel();
        let (b, mut rb) = tokio::sync::oneshot::channel();
        let (c, mut rc) = tokio::sync::oneshot::channel();
        inner.pending.insert("r1".into(), ("host".into(), a));
        inner.pending.insert("r2".into(), ("host".into(), b));
        inner.pending.insert("r3".into(), ("other".into(), c));
        // A ref naming another device's call is ignored.
        inner.fail_ref("host", Some("r3"), "offline");
        assert!(rc.try_recv().is_err());
        inner.fail_ref("host", Some("r1"), "offline");
        assert_eq!(ra.try_recv().unwrap(), Err("offline".to_string()));
        assert!(rb.try_recv().is_err());
        // Without a ref, every call to that device fails.
        inner.fail_ref("host", None, "rate");
        assert_eq!(rb.try_recv().unwrap(), Err("rate".to_string()));
        assert!(rc.try_recv().is_err());
        assert_eq!(inner.pending.len(), 1);
    }

    #[test]
    fn the_viewer_refuses_hosts_it_does_not_trust() {
        let (old, new) = (crypto::public_key(&crypto::generate()), crypto::public_key(&crypto::generate()));
        let mut inner = Inner { devices: vec![device("host", "Studio Mac", &new)], ..Default::default() };
        let refused = Err("Studio Mac isn't trusted on this computer yet. Trust it in Settings → Remote access.".to_string());
        assert_eq!(inner.callable("host"), refused);
        // Pinned to an older key: still refused.
        inner.settings.trusted.insert("host".into(), old);
        assert_eq!(inner.callable("host"), refused);
        inner.settings.trusted.insert("host".into(), new);
        assert_eq!(inner.callable("host"), Ok(()));
        assert_eq!(inner.callable("gone"), Err(UNKNOWN_DEVICE.to_string()));
    }

    #[test]
    fn queued_messages_recheck_the_authenticated_key_after_revocation_or_replacement() {
        let mut inner = Inner { devices: vec![device("peer", "Peer", "first-key")], ..Default::default() };
        inner.settings.trusted.insert("peer".into(), "first-key".into());
        assert!(inner.authenticated("peer", "first-key"));
        inner.settings.trusted.remove("peer");
        assert!(!inner.authenticated("peer", "first-key"), "revocation invalidates a queued request");
        inner.settings.trusted.insert("peer".into(), "first-key".into());
        inner.devices[0].public_key = "replacement-key".into();
        assert!(!inner.authenticated("peer", "first-key"), "presence no longer identifies this key");
        assert!(!inner.authenticated("peer", "replacement-key"), "the replacement is not pinned");
        inner.settings.trusted.insert("peer".into(), "replacement-key".into());
        assert!(!inner.authenticated("peer", "first-key"), "retrusting a new key cannot authorize old queued messages");
        assert!(inner.authenticated("peer", "replacement-key"));
        inner.devices.clear();
        assert!(!inner.authenticated("peer", "replacement-key"), "removed devices cannot receive pending replies");
    }

    #[test]
    fn targets_are_trusted_hosts_other_than_this_one() {
        let key = crypto::public_key(&crypto::generate());
        let mut guest = device("guest", "Laptop", &key);
        guest.host = false;
        let mut inner = Inner { devices: vec![device("me", "This Mac", &key), device("studio", "Studio Mac", &key), device("stranger", "Other", &key), guest], ..Default::default() };
        inner.settings.device_id = "me".into();
        for id in ["me", "studio", "guest"] {
            inner.settings.trusted.insert(id.into(), key.clone());
        }
        assert_eq!(targets(&inner), vec![json!({"deviceId": "studio", "name": "Studio Mac", "platform": "", "online": true})]);
    }

    #[test]
    fn a_stale_token_never_signs_out_a_newer_sign_in() {
        let mut inner = Inner { token: Some("new".into()), generation: 3, ..Default::default() };
        inner.settings.account = Some(config::Account::default());
        assert!(!inner.clear_sign_in(Some("old")));
        assert_eq!((inner.token.as_deref(), inner.generation, inner.settings.account.is_some()), (Some("new"), 3, true));
        assert!(inner.clear_sign_in(Some("new")));
        assert_eq!((inner.token.as_deref(), inner.generation, inner.settings.account.is_some()), (None, 4, false));
        inner.token = Some("again".into());
        assert!(inner.clear_sign_in(None));
        assert!(inner.token.is_none());
    }
}

#[cfg(test)]
mod permission_regression_tests {
    use super::*;

    #[test]
    fn revoked_device_cannot_send_events_or_responses_to_pending_calls() {
        let mut inner = Inner::default();
        inner.devices.push(Device {
            id: "peer".into(), name: "Test peer".into(), platform: "test".into(),
            public_key: "key-1".into(), host: true, online: true, last_seen: Value::Null,
        });
        inner.settings.trusted.insert("peer".into(), "key-1".into());
        assert!(inner.allows_incoming("peer", "key-1", "event", None));
        assert!(inner.allows_incoming("peer", "key-1", "res", Some("call")));
        inner.settings.trusted.clear();
        assert!(!inner.allows_incoming("peer", "key-1", "event", None));
        assert!(!inner.allows_incoming("peer", "key-1", "res", Some("call")));
        let (tx, _rx) = tokio::sync::oneshot::channel();
        inner.pending.insert("pair-1".into(), ("peer".into(), tx));
        inner.pairing_pending.insert("pair-1".into(), ("peer".into(), "key-1".into()));
        assert!(inner.allows_incoming("peer", "key-1", "res", Some("pair-1")));
        assert!(!inner.allows_incoming("peer", "key-1", "event", Some("pair-1")));
        assert!(!inner.allows_incoming("peer", "key-2", "res", Some("pair-1")));
        inner.devices[0].public_key = "key-2".into();
        assert!(!inner.allows_incoming("peer", "key-1", "res", Some("pair-1")));
        inner.fail_pending(Some("peer"), "revoked");
        assert!(inner.pairing_pending.is_empty());
    }

    #[test]
    fn stale_or_missing_displayed_fingerprint_never_trusts_a_new_key() {
        let original = crypto::public_key(&crypto::generate());
        let rotated = crypto::public_key(&crypto::generate());
        let reviewed = crypto::fingerprint(&original);
        assert!(verified_fingerprint(&original, Some(&reviewed)).is_ok());
        assert!(verified_fingerprint(&rotated, Some(&reviewed)).is_err());
        assert!(verified_fingerprint(&original, None).is_err());
        assert!(verified_fingerprint(&original, Some("bad")).is_err());
        assert!(verified_fingerprint("not a valid public key", Some(&reviewed)).is_err());
    }
}
