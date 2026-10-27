//! What each paired device may do on this computer (scoped pairing). A device seen for
//! the first time waits for the user's approval here before any request is served.
//!
//! Scopes, narrowest first: `pending` (nothing; a new device or one whose key changed),
//! `view` (read-only), `control` (view plus driving existing tasks and answering
//! approvals) and `full` (everything, including starting tasks and merging pull
//! requests). `blocked` is refused outright and stays blocked when its key changes.
//! A grant belongs to a device id and the fingerprint of its public key. Grants live in a
//! 0600 JSON file in the app data dir (`connect-grants.json`, via `secret_store`).
use super::{config, crypto, remote};
use crate::Result;
use chacha20poly1305::aead::{rand_core::RngCore, OsRng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Emitter;

const FILE: &str = "connect-grants.json";
const LABEL: &str = "PandaCode Connect device grants";
/// How long a pairing code stays valid, and how many wrong guesses end it.
const CODE_TTL: i64 = 10 * 60;
const MAX_TRIES: u8 = 5;
pub(crate) const REDEEM: &str = "pair.redeem";
const INVALID_CODE: &str = "That pairing code is not valid or has expired.";

const VIEW: [&str; 18] = [
    "subscribe", "tasks.list", "task.messages", "task.changes", "task.diff", "task.attachment", "task.workspaceImage", "projects.icons", "providers.list", "providers.models",
    "task.team", "task.workerMessages", "bots.list", "bot.thread", "task.startStatus", "task.pull", "task.pullDiff", "batch",
];
const CONTROL: [&str; 9] = ["task.send", "task.steer", "task.stop", "task.configure", "approval.respond", "input.respond", "image.put", "task.cancelStart", "task.pullDetect"];
/// Needs `full`: starting work, merging, and acting as a bot. Anything not listed anywhere
/// is `full`-only too.
#[cfg(test)]
const FULL_ONLY: [&str; 4] = ["task.start", "task.pullMerge", "bot.message", "bot.pause"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Scope {
    #[default]
    Pending,
    View,
    Control,
    Full,
    Blocked,
}

impl Scope {
    fn parse(text: &str) -> Result<Scope> {
        Ok(match text {
            "pending" => Scope::Pending,
            "view" => Scope::View,
            "control" => Scope::Control,
            "full" => Scope::Full,
            "blocked" => Scope::Blocked,
            _ => return Err("Unknown access level.".into()),
        })
    }

    fn name(self) -> &'static str {
        match self {
            Scope::Pending => "pending",
            Scope::View => "view",
            Scope::Control => "control",
            Scope::Full => "full",
            Scope::Blocked => "blocked",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Scope::Pending | Scope::Blocked => 0,
            Scope::View => 1,
            Scope::Control => 2,
            Scope::Full => 3,
        }
    }

    /// Whether an approved scope may call `method`. Pending and blocked devices never do
    /// (a pending one may redeem a code, which `decide` handles).
    fn allows(self, method: &str) -> bool {
        match self {
            Scope::Pending | Scope::Blocked => false,
            Scope::Full => true,
            Scope::View => VIEW.contains(&method),
            Scope::Control => VIEW.contains(&method) || CONTROL.contains(&method),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Entry {
    scope: Scope,
    /// Fingerprint of the public key the scope was granted to.
    fingerprint: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    devices: BTreeMap<String, Entry>,
    /// Legacy upstream adoption marker. Extended never trusts newly discovered devices
    /// through presence; this marker is cleared without creating any grant.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    adopt: bool,
}

/// A device on the account as presence lists it.
#[derive(Debug, Clone, PartialEq)]
struct Peer {
    id: String,
    name: String,
    platform: String,
    fingerprint: String,
    online: bool,
}

impl From<&super::Device> for Peer {
    fn from(device: &super::Device) -> Self {
        Self { id: device.id.clone(), name: device.name.clone(), platform: device.platform.clone(), fingerprint: crypto::fingerprint(&device.public_key), online: device.online }
    }
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Allow,
    /// Waiting for approval; `announce` is true the first time this key is seen.
    Pending { announce: bool },
    Denied(Scope),
}

struct Code {
    digits: String,
    scope: Scope,
    expires: i64,
    tries: u8,
}

#[derive(Default)]
struct Grants {
    store: Store,
    code: Option<Code>,
    /// `id|fingerprint` pairs already announced in this run.
    announced: HashSet<String>,
    dirty: bool,
}

impl Grants {
    fn load(dir: &Path) -> Grants {
        let store = crate::secret_store::load(&dir.join(FILE), LABEL).unwrap_or_else(|error| {
            eprintln!("{error}");
            Store::default()
        });
        Grants { store, ..Default::default() }
    }

    /// The scope in force: the stored one only while the device still has the key it was
    /// granted to (`blocked` holds whatever the key).
    fn scope_of(&self, peer: &Peer) -> Scope {
        match self.store.devices.get(&peer.id) {
            Some(entry) if entry.scope == Scope::Blocked => Scope::Blocked,
            Some(entry) if !peer.fingerprint.is_empty() && entry.fingerprint == peer.fingerprint => entry.scope,
            _ => Scope::Pending,
        }
    }

    fn set(&mut self, peer: &Peer, scope: Scope) {
        self.store.devices.insert(peer.id.clone(), Entry { scope, fingerprint: peer.fingerprint.clone(), name: peer.name.clone() });
        self.announced.remove(&format!("{}|{}", peer.id, peer.fingerprint));
        self.dirty = true;
    }

    /// Records a waiting device (once per key) and says whether this is its first sighting.
    fn note_pending(&mut self, peer: &Peer) -> bool {
        let waiting = self.store.devices.get(&peer.id).is_some_and(|entry| entry.scope == Scope::Pending && entry.fingerprint == peer.fingerprint && entry.name == peer.name);
        if !waiting {
            self.store.devices.insert(peer.id.clone(), Entry { scope: Scope::Pending, fingerprint: peer.fingerprint.clone(), name: peer.name.clone() });
            self.dirty = true;
        }
        self.announced.insert(format!("{}|{}", peer.id, peer.fingerprint))
    }

    fn decide(&mut self, peer: &Peer, method: &str) -> Verdict {
        match self.scope_of(peer) {
            Scope::Blocked => Verdict::Denied(Scope::Blocked),
            Scope::Pending if method == REDEEM => Verdict::Allow,
            Scope::Pending => Verdict::Pending { announce: self.note_pending(peer) },
            scope if scope.allows(method) => Verdict::Allow,
            scope => Verdict::Denied(scope),
        }
    }

    /// A historical upstream adoption marker never authorizes newly observed peers.
    fn adopt(&mut self, peers: &[Peer]) {
        if !self.store.adopt {
            return;
        }
        // Extended: an upgrade cannot silently grant access to newly discovered devices.
        let _ = peers;
        self.store.adopt = false;
        self.dirty = true;
    }

    fn blocks(&self, device: &str) -> bool {
        self.store.devices.get(device).is_some_and(|entry| entry.scope == Scope::Blocked)
    }

    /// Starts (replacing any earlier) a single-use code for `scope`; returns its expiry.
    fn new_code(&mut self, scope: Scope, digits: String, now: i64) -> i64 {
        let expires = now + CODE_TTL;
        self.code = Some(Code { digits, scope, expires, tries: 0 });
        expires
    }

    /// Consume a valid code without activating its grant. The cross-store
    /// transaction below is the only place that publishes the resulting scope.
    /// A failed commit burns the code too; the owner must issue a new one.
    fn take_code(&mut self, peer: &Peer, input: &str, now: i64) -> Result<Scope> {
        if self.scope_of(peer) == Scope::Blocked {
            return Err(INVALID_CODE.into());
        }
        let given: String = input.chars().filter(char::is_ascii_digit).collect();
        let Some(code) = self.code.as_mut().filter(|code| code.expires > now) else {
            self.code = None;
            return Err(INVALID_CODE.into());
        };
        let same = given.len() == code.digits.len() && given.bytes().zip(code.digits.bytes()).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0;
        if !same {
            code.tries += 1;
            if code.tries >= MAX_TRIES {
                self.code = None;
            }
            return Err(INVALID_CODE.into());
        }
        let granted = self.code.take().map(|code| code.scope).unwrap_or_default();
        let current = self.scope_of(peer);
        let scope = if current.rank() >= granted.rank() { current } else { granted };
        Ok(scope)
    }

    #[cfg(test)]
    fn redeem(&mut self, peer: &Peer, input: &str, now: i64) -> Result<Scope> {
        let scope = self.take_code(peer, input, now)?;
        self.set(peer, scope);
        Ok(scope)
    }

    /// Every device with its scope, devices waiting for a decision first.
    fn view(&self, peers: &[Peer]) -> Vec<Value> {
        let mut rows: Vec<(bool, Value)> = peers
            .iter()
            .map(|peer| {
                let scope = self.scope_of(peer);
                let requested = self.store.devices.get(&peer.id).is_some_and(|entry| entry.scope == Scope::Blocked || entry.fingerprint == peer.fingerprint);
                let waiting = scope == Scope::Pending && requested;
                (waiting, json!({"deviceId": peer.id, "name": peer.name, "platform": peer.platform, "fingerprint": peer.fingerprint, "scope": scope.name(), "online": peer.online, "requested": requested}))
            })
            .collect();
        rows.sort_by_key(|(waiting, _)| !waiting);
        rows.into_iter().map(|(_, row)| row).collect()
    }
}

/// A pending-approval or blocked device's request, as a user-facing refusal.
fn refusal(scope: Scope, host: &str) -> String {
    match scope {
        Scope::Blocked => format!("{host} has blocked this device."),
        Scope::View => format!("This device can only view tasks on {host}."),
        _ => format!("This device is not allowed to do that on {host}. Ask for full access."),
    }
}

struct Snapshot {
    dir: PathBuf,
    peers: Vec<Peer>,
}

/// The account's other devices as presence lists them. Takes and releases the remote lock.
fn snapshot(app: &tauri::AppHandle) -> Snapshot {
    let state = remote(app);
    let inner = state.lock();
    let peers = inner.devices.iter().filter(|device| device.id != inner.settings.device_id).map(|device| Peer { id: device.id.clone(), name: device.name.clone(), platform: device.platform.clone(), fingerprint: crypto::fingerprint(&device.public_key), online: device.online }).collect();
    Snapshot { dir: state.dir.clone(), peers }
}

static GRANTS: std::sync::LazyLock<Mutex<HashMap<PathBuf, Grants>>> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Runs `f` on the loaded grants and saves them afterwards when it changed them.
/// The cache is tied to the settings directory: tests and account migrations
/// must never reuse another directory's in-memory authorization.
fn with<R>(dir: &Path, f: impl FnOnce(&mut Grants) -> R) -> Result<R> {
    let mut guard = GRANTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let grants = guard.entry(dir.to_owned()).or_insert_with(|| Grants::load(dir));
    let original = grants.store.clone();
    let result = f(grants);
    if std::mem::take(&mut grants.dirty) {
        let store = grants.store.clone();
        if let Err(error) = crate::secret_store::update(dir, FILE, LABEL, |_: Store| store) {
            // A failed durable grant update must never activate in memory.
            grants.store = original;
            return Err(error);
        }
    }
    Ok(result)
}

/// Preserve explicit, key-pinned trust on upgrade, but never discover or elevate
/// additional devices from a presence frame. Full is carried over only when the
/// owner separately opted into the local Full-access permission gate.
pub(crate) fn seed_trusted(dir: &Path, settings: &config::Settings) -> Result<()> {
    crate::secret_store::update::<Store>(dir, FILE, LABEL, |mut current| {
        for (id, key) in &settings.trusted {
            if key.is_empty() || super::host::valid_id(id).is_err() { continue; }
            current.devices.entry(id.clone()).or_insert_with(|| Entry {
                scope: if settings.allow_full_access { Scope::Full } else { Scope::Control },
                fingerprint: crypto::fingerprint(key),
                name: String::new(),
            });
        }
        current.adopt = false;
        current
    })
}

/// Remove a revoked legacy trust grant from the independent scoped store before
/// lifting the durability fence. Trusting the same key again is not a regrant.
pub(crate) fn revoke_legacy_trust(dir: &Path, device: &str) -> Result<()> {
    with(dir, |grants| {
        if !grants.store.devices.get(device).is_some_and(|entry| entry.scope == Scope::Blocked) {
            grants.store.devices.remove(device);
        }
        grants.dirty = true;
    })
}

/// Disabling the local Full switch must not leave reusable Full grants on disk.
pub(crate) fn downgrade_full_access(dir: &Path) -> Result<()> {
    with(dir, |grants| {
        if grants.code.as_ref().is_some_and(|code| code.scope == Scope::Full) {
            grants.code = None;
        }
        for entry in grants.store.devices.values_mut() {
            if entry.scope == Scope::Full {
                entry.scope = Scope::Control;
            }
        }
        grants.dirty = true;
    })
}

/// Account sign-out invalidates scoped grants as well as the legacy trust map.
pub(crate) fn clear_on_sign_out(dir: &Path) -> Result<()> {
    with(dir, |grants| {
        grants.store.devices.clear();
        grants.code = None;
        grants.dirty = true;
    })
}

/// Called on every presence frame (never on the HTTP device list): the first one after an
/// upgrade adopts the devices it lists (`Grants::adopt`).
pub(crate) fn presence(app: &tauri::AppHandle) {
    let snap = snapshot(app);
    let adopted = with(&snap.dir, |grants| {
        let adopting = grants.store.adopt;
        grants.adopt(&snap.peers);
        adopting
    });
    if adopted.unwrap_or(false) {
        emit_grants(app);
    }
}

/// Whether the user blocked `device` here.
pub(crate) fn is_blocked(app: &tauri::AppHandle, device: &str) -> bool {
    with(&remote(app).dir, |grants| grants.blocks(device)).unwrap_or(true)
}

fn emit_grants(app: &tauri::AppHandle) {
    let snap = snapshot(app);
    let list = with(&snap.dir, |grants| grants.view(&snap.peers));
    if let Ok(list) = list { let _ = app.emit("remote://device-grants", list); }
}

fn decision(dir: &Path, inner: &super::Inner, device: &str, authenticated_key: &str, method: &str) -> Result<(Peer, Verdict)> {
    inner.access_available(dir)?;
    super::host::gate(inner.settings.host_enabled, inner.authenticated(device, authenticated_key), &config::device_name())?;
    if !super::host::METHODS.contains(&method) || method == REDEEM {
        return Err("That remote method is not available.".into());
    }
    if !inner.settings.allow_full_access && !Scope::Control.allows(method) {
        return Err("Remote Full access is disabled on this computer.".into());
    }
    let peer = Peer::from(inner.devices.iter().find(|peer| peer.id == device && peer.public_key == authenticated_key).ok_or(super::UNKNOWN_DEVICE)?);
    let verdict = with(dir, |grants| grants.decide(&peer, method))?;
    Ok((peer, verdict))
}

fn allowed(verdict: &Verdict) -> Result<()> {
    match verdict {
        Verdict::Allow => Ok(()),
        Verdict::Pending { .. } => Err(format!("Waiting for approval on {}.", config::device_name())),
        Verdict::Denied(scope) => Err(refusal(*scope, &config::device_name())),
    }
}

/// One authoritative decision while the caller holds Remote: original key,
/// current presence, legacy pin, scoped grant, local Full switch and both denial
/// fences. Remote is always acquired before GRANTS, including permission writes.
pub(super) fn authorize(dir: &Path, inner: &super::Inner, device: &str, authenticated_key: &str, method: &str) -> Result<()> {
    let (_, verdict) = decision(dir, inner, device, authenticated_key, method)?;
    allowed(&verdict)
}

/// Rechecks the current device grant and local Full switch without executing a
/// particular Full-only operation. `task.start` is the canonical Full-only
/// method in the remote method table.
pub(super) fn authorize_full(dir: &Path, inner: &super::Inner, device: &str, authenticated_key: &str) -> Result<()> {
    authorize(dir, inner, device, authenticated_key, "task.start")
}

pub(crate) fn with_current_full_request<T>(mode: &str, effect: impl FnOnce() -> Result<T>) -> Result<T> {
    super::host::with_current_full_request(mode, effect)
}

/// Pairing replies may be retried after the owner changes a grant. Never replay
/// an old successful scope as though it still described current authorization.
pub(super) fn authorize_pairing_response(dir: &Path, inner: &super::Inner, device: &str, authenticated_key: &str, response: &Value) -> Result<()> {
    if response["ok"] != true { return Ok(()); }
    let (peer, verdict) = decision(dir, inner, device, authenticated_key, "tasks.list")?;
    allowed(&verdict)?;
    let reported = response["result"]["scope"].as_str().ok_or("The pairing response has no access level.")?;
    let reported = Scope::parse(reported)?;
    let current = with(dir, |grants| grants.scope_of(&peer))?;
    if reported != current || (reported == Scope::Full && !inner.settings.allow_full_access) {
        return Err("This computer's access changed after pairing. Refresh its current access level.".into());
    }
    Ok(())
}

pub(crate) fn check(app: &tauri::AppHandle, device: &str, authenticated_key: &str, method: &str) -> Result<()> {
    let (peer, verdict) = {
        let state = remote(app);
        let inner = state.lock();
        decision(&state.dir, &inner, device, authenticated_key, method)?
    };
    if let Verdict::Pending { announce } = &verdict {
        if *announce {
            let _ = app.emit("remote://device-pending", json!({"deviceId": peer.id, "name": peer.name, "fingerprint": peer.fingerprint}));
            emit_grants(app);
        }
    }
    allowed(&verdict)
}

/// Both local approvals and code redemption use this fenced transaction. A
/// scope written to disk is never effective until the matching trust write and
/// fence removal succeed. On any failure all access stays denied in memory.
fn set_scope_with(
    dir: &Path,
    inner: &mut super::Inner,
    peer: &Peer,
    scope: Scope,
    key: Option<&str>,
    mut save_settings: impl FnMut(&Path, &config::Settings) -> Result<()>,
) -> Result<()> {
    let earlier_lockdown = inner.access_locked_down || config::locked_down(dir);
    let approving = matches!(scope, Scope::View | Scope::Control | Scope::Full);
    if approving {
        inner.access_available(dir)?;
        if scope == Scope::Full && !inner.settings.allow_full_access {
            return Err("Remote Full access is disabled on this computer.".into());
        }
        let key = key.ok_or(super::UNKNOWN_DEVICE)?;
        super::verified_fingerprint(key, Some(&peer.fingerprint))?;
        if !inner.devices.iter().any(|device| device.id == peer.id && device.public_key == key) {
            return Err("The device key changed during pairing. Compare the new fingerprint.".into());
        }
    }
    let mut active = inner.settings.clone();
    active.trusted.remove(&peer.id);
    inner.settings = active.clone();
    inner.access_locked_down = true;
    inner.subscribers.remove(&peer.id);
    inner.uploads.forget(&peer.id);
    inner.catalog.forget(&peer.id);
    inner.keys.remove(&peer.id);
    inner.fail_pending(Some(&peer.id), "This computer's Connect permissions changed.");
    let result = (|| {
        config::begin_lockdown(dir)?;
        save_settings(dir, &active)?;
        with(dir, |grants| grants.set(peer, scope))?;
        if approving {
            active.trusted.insert(peer.id.clone(), key.ok_or(super::UNKNOWN_DEVICE)?.to_owned());
            save_settings(dir, &active)?;
        }
        if !earlier_lockdown { config::end_lockdown(dir)?; }
        Ok(())
    })();
    if result.is_err() {
        inner.deny_access();
        let _ = config::begin_lockdown(dir);
    } else {
        inner.settings = active;
        inner.access_locked_down = earlier_lockdown;
    }
    result
}

fn redeem_with(
    dir: &Path,
    inner: &mut super::Inner,
    device: &str,
    authenticated_key: &str,
    code: &str,
    now: i64,
    save_settings: impl FnMut(&Path, &config::Settings) -> Result<()>,
) -> Result<(Peer, Scope)> {
    inner.access_available(dir)?;
    if !inner.settings.host_enabled { return Err(super::host::OFF.into()); }
    let device = inner.devices.iter().find(|peer| peer.id == device && !authenticated_key.is_empty() && peer.public_key == authenticated_key).ok_or("The device key changed during pairing. Compare the new fingerprint.")?;
    let peer = Peer::from(device);
    super::verified_fingerprint(authenticated_key, Some(&peer.fingerprint))?;
    let scope = with(dir, |grants| {
        let current = grants.scope_of(&peer);
        let requested = grants.code.as_ref().map(|code| code.scope).unwrap_or_default();
        if !inner.settings.allow_full_access && (current == Scope::Full || requested == Scope::Full) {
            return Err("Remote Full access is disabled on this computer.".to_owned());
        }
        grants.take_code(&peer, code, now)
    })??;
    set_scope_with(dir, inner, &peer, scope, Some(authenticated_key), save_settings)?;
    Ok((peer, scope))
}

/// Host side of `pair.redeem`. The key that opened the original ciphertext is
/// carried through the entire commit, under the same lock as presence/revokes.
pub(crate) fn redeem(app: &tauri::AppHandle, device: &str, authenticated_key: &str, params: &Value) -> Result<Value> {
    let code = params["code"].as_str().filter(|code| code.len() <= 32).ok_or(INVALID_CODE)?;
    let result = {
        let state = remote(app);
        let mut inner = state.lock();
        redeem_with(&state.dir, &mut inner, device, authenticated_key, code, chrono::Utc::now().timestamp(), config::save)
    };
    super::emit_status(app);
    super::emit_presence(app);
    emit_grants(app);
    let (peer, scope) = result?;
    let _ = app.emit("remote://pair-code-used", json!({"deviceId": peer.id, "name": peer.name, "scope": scope.name()}));
    Ok(json!({"scope": scope.name()}))
}

fn random_code() -> String {
    loop {
        let n = OsRng.next_u32();
        // 4_294_000_000 is a multiple of a million, so the digits are uniform.
        if n < 4_294_000_000 {
            return format!("{:06}", n % 1_000_000);
        }
    }
}

/// Every known device with its scope (`pending`, `full`, `control`, `view`, `blocked`).
#[tauri::command]
pub async fn remote_device_grants(app: tauri::AppHandle) -> Result<Value> {
    let snap = snapshot(&app);
    Ok(Value::Array(with(&snap.dir, |grants| grants.view(&snap.peers))?))
}

/// Sets a device's scope; `pending` devices become usable once given another scope.
#[tauri::command]
pub async fn remote_set_device_grant(app: tauri::AppHandle, device_id: String, scope: String, expected_fingerprint: Option<String>) -> Result<()> {
    super::valid_device_id(&device_id)?;
    let scope = Scope::parse(&scope)?;
    let state = remote(&app);
    // Keep presence identity fixed across both persisted stores and the final
    // trust activation. A new key cannot win a race after the UI's approval.
    let result = {
        let mut inner = state.lock();
        if device_id == inner.settings.device_id { return Err("That is this computer.".into()); }
        let device = inner.devices.iter().find(|peer| peer.id == device_id).cloned();
        let approving = matches!(scope, Scope::View | Scope::Control | Scope::Full);
        let peer = if let Some(device) = &device {
            if approving {
                super::verified_fingerprint(&device.public_key, expected_fingerprint.as_deref())?;
            }
            Peer::from(device)
        } else if approving {
            return Err(super::UNKNOWN_DEVICE.into());
        } else {
            // Revocation and blocking work even after a device left presence or
            // rotated its key. A blocked entry applies to any key for this id.
            Peer { id: device_id.clone(), name: String::new(), platform: String::new(), fingerprint: String::new(), online: false }
        };
        set_scope_with(&state.dir, &mut inner, &peer, scope, device.as_ref().map(|device| device.public_key.as_str()), config::save)
    };
    super::emit_status(&app);
    super::emit_presence(&app);
    emit_grants(&app);
    result
}

/// Shows a one-time code (valid 10 minutes) that gives a new device `scope` when entered
/// there. A new code replaces the old one.
#[tauri::command]
pub async fn remote_pair_code(app: tauri::AppHandle, scope: String) -> Result<Value> {
    let scope = Scope::parse(&scope)?;
    if !matches!(scope, Scope::View | Scope::Control | Scope::Full) {
        return Err("A pairing code can give view, control or full access.".into());
    }
    let (digits, now) = (random_code(), chrono::Utc::now().timestamp());
    let expires = {
        let state = remote(&app);
        let inner = state.lock();
        inner.access_available(&state.dir)?;
        if scope == Scope::Full && !inner.settings.allow_full_access {
            return Err("Enable remote Full access locally before granting a device Full access.".into());
        }
        with(&state.dir, |grants| grants.new_code(scope, digits.clone(), now))?
    };
    Ok(json!({"code": digits, "scope": scope.name(), "expiresAt": expires * 1000}))
}

#[tauri::command]
pub async fn remote_pair_code_cancel(app: tauri::AppHandle) -> Result<()> {
    let snap = snapshot(&app);
    with(&snap.dir, |grants| grants.code = None)?;
    Ok(())
}

/// Viewer side: sends the code shown on `device_id` (a host) over the encrypted channel.
/// Returns `{scope}` when the host accepted it.
#[tauri::command]
pub async fn remote_pair_redeem(app: tauri::AppHandle, device_id: String, code: String) -> Result<Value> {
    let digits: String = code.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 6 {
        return Err("Enter the 6-digit code shown on the other computer.".into());
    }
    super::client::call(&app, &device_id, REDEEM, json!({"code": digits})).await
}

#[cfg(test)]
pub(crate) fn test_scope_from_file(dir: &Path, id: &str, key: &str) -> &'static str {
    let grants = Grants::load(dir);
    let peer = Peer { id: id.into(), name: String::new(), platform: String::new(), fingerprint: crypto::fingerprint(key), online: true };
    grants.scope_of(&peer).name()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::host::RequestAuthorization;

    fn peer(id: &str, fingerprint: &str) -> Peer {
        Peer { id: id.into(), name: format!("Device {id}"), platform: "iOS".into(), fingerprint: fingerprint.into(), online: true }
    }

    fn granted(grants: &mut Grants, id: &str, fingerprint: &str, scope: Scope) -> Peer {
        let peer = peer(id, fingerprint);
        grants.set(&peer, scope);
        peer
    }

    struct AccessFixture {
        dir: PathBuf,
        inner: super::super::Inner,
        key: String,
        peer: Peer,
    }

    impl AccessFixture {
        fn new(scope: Scope) -> Self {
            let dir = std::env::temp_dir().join(format!("pandacode-connect-access-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let key = crypto::public_key(&crypto::generate());
            let device = super::super::Device { id: "peer".into(), name: "Peer".into(), platform: "test".into(), public_key: key.clone(), host: true, online: true, last_seen: Value::Null };
            let peer = Peer::from(&device);
            let mut settings = config::Settings { device_id: uuid::Uuid::new_v4().to_string(), host_enabled: true, allow_full_access: true, grants_scoped: true, ..Default::default() };
            settings.trusted.insert(peer.id.clone(), key.clone());
            config::save(&dir, &settings).unwrap();
            with(&dir, |grants| grants.set(&peer, scope)).unwrap();
            Self { dir, inner: super::super::Inner { settings, devices: vec![device], ..Default::default() }, key, peer }
        }

        fn code(&self, scope: Scope) {
            with(&self.dir, |grants| grants.new_code(scope, "314159".into(), 1_000)).unwrap();
        }

        fn assert_denied(&self) {
            for method in super::super::host::METHODS.into_iter().filter(|method| *method != REDEEM) {
                assert!(authorize(&self.dir, &self.inner, "peer", &self.key, method).is_err(), "{method} remained authorized");
            }
            assert!(!self.inner.authenticated("peer", &self.key));
            assert!(!self.inner.allows_incoming("peer", &self.key, "event", None));
            assert!(!self.inner.allows_incoming("peer", &self.key, "res", Some("old-request")));
        }

        fn assert_restart_denied(&self) {
            let restarted = super::super::Inner { settings: config::load(&self.dir), devices: self.inner.devices.clone(), ..Default::default() };
            for method in FULL_ONLY {
                assert!(authorize(&self.dir, &restarted, "peer", &self.key, method).is_err(), "{method} reopened after restart");
            }
            assert!(restarted.settings.trusted.is_empty());
            assert!(!restarted.settings.host_enabled);
        }
    }

    impl Drop for AccessFixture {
        fn drop(&mut self) {
            GRANTS.lock().unwrap().remove(&self.dir);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn effective_authority_requires_local_full_and_both_denial_fences() {
        let mut fixture = AccessFixture::new(Scope::Full);
        for method in FULL_ONLY {
            assert!(authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, method).is_ok());
        }
        fixture.inner.settings.allow_full_access = false;
        for method in FULL_ONLY {
            assert!(authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, method).is_err(), "{method} ignored the local Full switch");
        }
        assert!(authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.send").is_ok(), "the local Full gate does not remove an otherwise valid Control action");
        fixture.inner.access_locked_down = true;
        fixture.assert_denied();
        fixture.inner.access_locked_down = false;
        config::begin_lockdown(&fixture.dir).unwrap();
        assert!(authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, "tasks.list").is_err(), "the durable fence is authoritative even before restart");
        fixture.assert_restart_denied();
    }

    #[test]
    fn full_scope_recheck_requires_current_full_grant_and_local_switch() {
        let mut fixture = AccessFixture::new(Scope::Control);
        assert!(authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.configure").is_ok());
        assert!(authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.steer").is_ok());
        assert!(authorize_full(&fixture.dir, &fixture.inner, "peer", &fixture.key).is_err(), "Control cannot perform a Full-only action");

        with(&fixture.dir, |grants| grants.set(&fixture.peer, Scope::Full)).unwrap();
        assert!(authorize_full(&fixture.dir, &fixture.inner, "peer", &fixture.key).is_ok());

        // A later demotion must still leave ordinary Control methods available,
        // while a Full-only effect is denied at its final authorization check.
        with(&fixture.dir, |grants| grants.set(&fixture.peer, Scope::Control)).unwrap();
        assert!(authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.steer").is_ok());
        assert!(authorize_full(&fixture.dir, &fixture.inner, "peer", &fixture.key).is_err(), "a demoted grant is not cached");

        with(&fixture.dir, |grants| grants.set(&fixture.peer, Scope::Full)).unwrap();
        fixture.inner.settings.allow_full_access = false;
        assert!(authorize_full(&fixture.dir, &fixture.inner, "peer", &fixture.key).is_err(), "the local switch remains authoritative");
    }

    #[test]
    fn configure_and_bot_steer_effects_recheck_current_full_authority() {
        let mut fixture = AccessFixture::new(Scope::Full);
        let store = crate::storage::Store::open(Path::new(":memory:")).unwrap();
        let project = store.add_project(&std::env::temp_dir()).unwrap();
        let task = store.create_session(&project.id, "claude", "sonnet", "ask").unwrap();

        let mut full_steer_enqueued = false;
        super::super::host::with_authorized_request_effect(
            RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.steer"),
            "ask",
            true,
            || { full_steer_enqueued = true; Ok(()) },
        ).unwrap();
        assert!(full_steer_enqueued, "Full bot steering reaches the enqueue effect");

        super::super::host::with_authorized_request_effect(
            RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.configure"),
            "full",
            true,
            || store.update(&task.id, None, None, None, Some("full".into())),
        ).unwrap();
        assert_eq!(store.session(&task.id).unwrap().mode, "full", "Full can configure a privileged mode while both gates are on");

        // A downgrade that wins before the final mutation leaves the old mode
        // intact, even though admission previously succeeded under Full.
        store.update(&task.id, None, None, None, Some("ask".into())).unwrap();
        with(&fixture.dir, |grants| grants.set(&fixture.peer, Scope::Control)).unwrap();
        let mut run_input_enqueued = false;
        let steer = super::super::host::with_authorized_request_effect(
            RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.steer"),
            "ask",
            true,
            || { run_input_enqueued = true; Ok(()) },
        );
        assert!(steer.is_err());
        assert!(!run_input_enqueued, "a revoked bot steer never reaches the RunInput enqueue effect");

        for mode in ["full", "review"] {
            let configure = super::super::host::with_authorized_request_effect(
                RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.configure"),
                mode,
                true,
                || store.update(&task.id, None, None, None, Some(mode.into())),
            );
            assert!(configure.is_err(), "Control cannot persist {mode} mode while the host switch is on");
            assert_eq!(store.session(&task.id).unwrap().mode, "ask", "the denied mode write leaves the task unchanged");
        }

        // Control still reaches ordinary steering and model configuration.
        let mut ordinary_steer_enqueued = false;
        super::super::host::with_authorized_request_effect(
            RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.steer"),
            "ask",
            false,
            || { ordinary_steer_enqueued = true; Ok(()) },
        ).unwrap();
        assert!(ordinary_steer_enqueued);
        super::super::host::with_authorized_request_effect(
            RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.configure"),
            "ask",
            false,
            || store.update(&task.id, None, None, Some("haiku".into()), None),
        ).unwrap();
        assert_eq!(store.session(&task.id).unwrap().model, "haiku");
        super::super::host::with_authorized_request_effect(
            RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.configure"),
            "plan",
            false,
            || store.update(&task.id, None, None, None, Some("plan".into())),
        ).unwrap();
        assert_eq!(store.session(&task.id).unwrap().mode, "plan");

        // A Full grant alone still cannot set review after the local switch is off.
        with(&fixture.dir, |grants| grants.set(&fixture.peer, Scope::Full)).unwrap();
        fixture.inner.settings.allow_full_access = false;
        let review = super::super::host::with_authorized_request_effect(
            RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.configure"),
            "review",
            true,
            || store.update(&task.id, None, None, None, Some("review".into())),
        );
        assert!(review.is_err(), "the local Full switch remains required at persistence");
        assert_eq!(store.session(&task.id).unwrap().mode, "plan");
    }

    #[test]
    fn privileged_effect_and_grant_demotion_linearize_on_remote_lock() {
        use std::sync::mpsc::{self, TryRecvError};
        use std::time::Duration;

        const WAIT: Duration = Duration::from_secs(5);

        let fixture = AccessFixture::new(Scope::Full);
        // Production holds Remote's mutex around with_authorized_request_effect;
        // model that caller-owned lock here so the test can force a revoke race.
        let remote_lock = std::sync::Mutex::new(());
        let (effect_started_tx, effect_started_rx) = mpsc::channel();
        let (release_effect_tx, release_effect_rx) = mpsc::channel();
        let (effect_finished_tx, effect_finished_rx) = mpsc::channel();
        let (demotion_blocked_tx, demotion_blocked_rx) = mpsc::channel();
        let (demotion_finished_tx, demotion_finished_rx) = mpsc::channel();
        let effect_dir = fixture.dir.clone();
        let effect_key = fixture.key.clone();
        let effect_inner = super::super::Inner { settings: fixture.inner.settings.clone(), devices: fixture.inner.devices.clone(), ..Default::default() };
        let demotion_dir = fixture.dir.clone();
        let demotion_peer = fixture.peer.clone();
        let effect_lock = &remote_lock;
        let demotion_lock = &remote_lock;

        std::thread::scope(|threads| {
            let effect = threads.spawn(move || {
                let _remote = effect_lock.lock().unwrap();
                super::super::host::with_authorized_request_effect(
                    RequestAuthorization::new(&effect_dir, &effect_inner, "peer", &effect_key, "task.configure"),
                    "full",
                    true,
                    || {
                        effect_started_tx.send(()).unwrap();
                        release_effect_rx.recv_timeout(WAIT).map_err(|error| error.to_string())?;
                        effect_finished_tx.send(()).map_err(|error| error.to_string())?;
                        Ok(())
                    },
                )
            });

            let demotion = threads.spawn(move || {
                effect_started_rx.recv_timeout(WAIT).expect("privileged effect did not start");
                let blocked = match demotion_lock.try_lock() {
                    Ok(guard) => {
                        drop(guard);
                        false
                    }
                    Err(std::sync::TryLockError::WouldBlock) => true,
                    Err(std::sync::TryLockError::Poisoned(error)) => panic!("remote lock poisoned: {error}"),
                };
                demotion_blocked_tx.send(blocked).unwrap();

                let _remote = demotion_lock.lock().unwrap();
                with(&demotion_dir, |grants| grants.set(&demotion_peer, Scope::Control)).unwrap();
                demotion_finished_tx.send(()).unwrap();
            });

            assert!(demotion_blocked_rx.recv_timeout(WAIT).expect("demotion did not try the remote lock"), "demotion acquired the lock before the privileged effect finished");
            assert!(matches!(demotion_finished_rx.try_recv(), Err(TryRecvError::Empty)), "demotion completed while the privileged effect held the lock");

            release_effect_tx.send(()).unwrap();
            effect_finished_rx.recv_timeout(WAIT).expect("privileged effect did not finish");
            demotion_finished_rx.recv_timeout(WAIT).expect("demotion did not finish after the effect released the lock");
            effect.join().unwrap().unwrap();
            demotion.join().unwrap();
        });

        let _remote = remote_lock.lock().unwrap();
        let mut next_effect_ran = false;
        let next = super::super::host::with_authorized_request_effect(
            RequestAuthorization::new(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.configure"),
            "full",
            true,
            || {
                next_effect_ran = true;
                Ok(())
            },
        );
        assert!(next.is_err(), "the following privileged effect must see the demoted grant");
        assert!(!next_effect_ran, "the denied effect must not run");
    }

    #[test]
    fn failed_full_revocation_denies_rpc_events_and_restart_until_complete_sign_out() {
        let mut fixture = AccessFixture::new(Scope::Full);
        fixture.code(Scope::Full);
        // Fail the real scoped file's advisory-lock open after remote.json has
        // saved Full OFF. This used to restore the old cached Full grant.
        std::fs::remove_file(fixture.dir.join("connect-grants.lock")).unwrap();
        std::fs::create_dir(fixture.dir.join("connect-grants.lock")).unwrap();
        let mut next = fixture.inner.settings.clone();
        next.allow_full_access = false;
        assert!(super::super::persist_scoped_revocation(&fixture.dir, &mut fixture.inner, next, false, downgrade_full_access).is_err());
        assert_eq!(Grants::load(&fixture.dir).scope_of(&fixture.peer), Scope::Full, "the regression must actually retain the stale on-disk Full grant");
        assert!(config::locked_down(&fixture.dir));
        fixture.assert_denied();
        fixture.assert_restart_denied();

        std::fs::remove_dir(fixture.dir.join("connect-grants.lock")).unwrap();
        // Neither another revoke nor a permission increase may lift this fence.
        let mut next = fixture.inner.settings.clone();
        next.host_enabled = false;
        super::super::update_access(&fixture.dir, &mut fixture.inner, next, true).unwrap();
        assert!(config::locked_down(&fixture.dir));
        let mut next = fixture.inner.settings.clone();
        next.host_enabled = true;
        assert!(super::super::update_access(&fixture.dir, &mut fixture.inner, next, false).is_err());

        let mut signed_out = fixture.inner.settings.clone();
        signed_out.account = None;
        signed_out.host_enabled = false;
        signed_out.allow_full_access = false;
        signed_out.trusted.clear();
        super::super::persist_scoped_revocation(&fixture.dir, &mut fixture.inner, signed_out, true, clear_on_sign_out).unwrap();
        assert!(!config::locked_down(&fixture.dir) && !fixture.inner.access_locked_down);
        assert_eq!(Grants::load(&fixture.dir).scope_of(&fixture.peer), Scope::Pending);
        assert!(!fixture.inner.settings.host_enabled && fixture.inner.settings.trusted.is_empty());
    }

    #[test]
    fn pairing_failure_at_either_settings_write_never_activates_the_new_grant() {
        for fail_at in [1, 2] {
            let mut fixture = AccessFixture::new(Scope::Control);
            fixture.code(Scope::Full);
            let mut writes = 0;
            let result = redeem_with(&fixture.dir, &mut fixture.inner, "peer", &fixture.key, "314159", 1_001, |dir, settings| {
                writes += 1;
                if writes == fail_at { std::fs::create_dir(dir.join("remote.json.tmp")).unwrap(); }
                config::save(dir, settings)
            });
            assert!(result.is_err(), "write {fail_at} should fail");
            assert_eq!(writes, fail_at);
            assert!(config::locked_down(&fixture.dir));
            assert!(with(&fixture.dir, |grants| grants.code.is_none()).unwrap(), "an attempted commit consumes the code even when persistence fails");
            assert_eq!(Grants::load(&fixture.dir).scope_of(&fixture.peer), if fail_at == 1 { Scope::Control } else { Scope::Full });
            fixture.assert_denied();
            fixture.assert_restart_denied();
        }
    }

    #[test]
    fn pairing_scoped_write_and_fence_clear_failures_remain_denied() {
        for fail_scopes in [true, false] {
            let mut fixture = AccessFixture::new(Scope::Control);
            fixture.code(Scope::Full);
            if fail_scopes {
                std::fs::remove_file(fixture.dir.join("connect-grants.lock")).unwrap();
                std::fs::create_dir(fixture.dir.join("connect-grants.lock")).unwrap();
            }
            let mut writes = 0;
            let result = redeem_with(&fixture.dir, &mut fixture.inner, "peer", &fixture.key, "314159", 1_001, |dir, settings| {
                writes += 1;
                config::save(dir, settings)?;
                if !fail_scopes && writes == 2 {
                    std::fs::remove_file(dir.join("connect-access-lockdown")).unwrap();
                    std::fs::create_dir(dir.join("connect-access-lockdown")).unwrap();
                }
                Ok(())
            });
            assert!(result.is_err());
            assert!(config::locked_down(&fixture.dir));
            fixture.assert_denied();
            fixture.assert_restart_denied();
        }
    }

    #[test]
    fn pairing_binds_the_ciphertext_key_and_invalid_attempts_do_not_revoke_trust() {
        let mut fixture = AccessFixture::new(Scope::Control);
        fixture.code(Scope::Full);
        assert!(redeem_with(&fixture.dir, &mut fixture.inner, "peer", &fixture.key, "000000", 1_001, config::save).is_err());
        assert!(authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.send").is_ok());
        assert!(!config::locked_down(&fixture.dir));
        let replacement = crypto::public_key(&crypto::generate());
        fixture.inner.devices[0].public_key = replacement.clone();
        assert!(redeem_with(&fixture.dir, &mut fixture.inner, "peer", &fixture.key, "314159", 1_001, config::save).is_err(), "a queued old-key request cannot approve the new presence key");
        assert!(with(&fixture.dir, |grants| grants.code.is_some()).unwrap());
        assert!(authorize(&fixture.dir, &fixture.inner, "peer", &replacement, "task.send").is_err());
        let stale_peer = fixture.peer.clone();
        assert!(set_scope_with(&fixture.dir, &mut fixture.inner, &stale_peer, Scope::Full, Some(&replacement), config::save).is_err(), "a stale displayed fingerprint cannot approve a rotated key");
    }

    #[test]
    fn concurrent_pairing_redemptions_commit_only_once() {
        let fixture = AccessFixture::new(Scope::Control);
        fixture.code(Scope::Full);
        let state = std::sync::Mutex::new(super::super::Inner { settings: fixture.inner.settings.clone(), devices: fixture.inner.devices.clone(), ..Default::default() });
        let ready = std::sync::Barrier::new(2);
        let (dir, key) = (&fixture.dir, &fixture.key);
        let results = std::thread::scope(|threads| {
            let calls: Vec<_> = (0..2).map(|_| threads.spawn(|| {
                ready.wait();
                redeem_with(dir, &mut state.lock().unwrap(), "peer", key, "314159", 1_001, config::save).is_ok()
            })).collect();
            calls.into_iter().map(|thread| thread.join().unwrap()).filter(|ok| *ok).count()
        });
        assert_eq!(results, 1);
        assert_eq!(Grants::load(&fixture.dir).scope_of(&fixture.peer), Scope::Full);
        assert!(authorize(&fixture.dir, &state.lock().unwrap(), "peer", &fixture.key, "task.pullMerge").is_ok());
        assert!(with(&fixture.dir, |grants| grants.code.is_none()).unwrap());
    }

    #[test]
    fn pairing_responses_do_not_replay_an_obsolete_successful_scope() {
        for scope in [Scope::Control, Scope::View, Scope::Pending, Scope::Blocked] {
            let mut fixture = AccessFixture::new(Scope::Full);
            let response = json!({"type":"res", "id":"pair-request", "ok":true, "result":{"scope":"full"}});
            assert!(authorize_pairing_response(&fixture.dir, &fixture.inner, "peer", &fixture.key, &response).is_ok());
            set_scope_with(&fixture.dir, &mut fixture.inner, &fixture.peer, scope, Some(&fixture.key), config::save).unwrap();
            assert!(authorize_pairing_response(&fixture.dir, &fixture.inner, "peer", &fixture.key, &response).is_err(), "{scope:?} must not announce the previous Full grant");
        }
        let mut fixture = AccessFixture::new(Scope::Full);
        let response = json!({"ok":true, "result":{"scope":"full"}});
        fixture.inner.settings.allow_full_access = false;
        assert!(authorize_pairing_response(&fixture.dir, &fixture.inner, "peer", &fixture.key, &response).is_err());
        assert!(authorize_pairing_response(&fixture.dir, &fixture.inner, "peer", &fixture.key, &json!({"ok":false,"error":INVALID_CODE})).is_ok(), "replaying a refusal never advertises authority");
    }

    #[test]
    fn unwritable_denial_fence_still_locks_the_running_process() {
        let mut fixture = AccessFixture::new(Scope::Full);
        let moved = fixture.dir.with_extension("unavailable");
        std::fs::rename(&fixture.dir, &moved).unwrap();
        let mut next = fixture.inner.settings.clone();
        next.allow_full_access = false;
        let result = super::super::persist_scoped_revocation(&fixture.dir, &mut fixture.inner, next, false, downgrade_full_access);
        std::fs::rename(&moved, &fixture.dir).unwrap();
        assert!(result.is_err(), "no durable-revocation success is claimed when storage is unavailable");
        assert!(!config::locked_down(&fixture.dir), "this scenario must exercise the in-memory fence independently");
        fixture.assert_denied();
        let mut next = fixture.inner.settings.clone();
        next.allow_full_access = true;
        assert!(super::super::update_access(&fixture.dir, &mut fixture.inner, next, false).is_err());
    }

    #[test]
    fn pull_merge_rechecks_real_authority_after_delayed_lookup() {
        for change in ["control", "pending", "blocked", "full-off", "host-off", "key-rotation", "lockdown"] {
            let fixture = AccessFixture::new(Scope::Full);
            let state = std::sync::Mutex::new(super::super::Inner { settings: fixture.inner.settings.clone(), devices: fixture.inner.devices.clone(), ..Default::default() });
            let effects = std::sync::atomic::AtomicUsize::new(0);
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let (resume_tx, resume_rx) = std::sync::mpsc::channel();
            std::thread::scope(|threads| {
                let (dir, key, state_ref, effects_ref) = (&fixture.dir, &fixture.key, &state, &effects);
                let pending = threads.spawn(move || crate::github_pull::merge_after_lookup(42, "squash", || {
                    entered_tx.send(()).unwrap();
                    resume_rx.recv().unwrap();
                    crate::github_pull::parse_detail(&json!({"number":42,"state":"OPEN","mergeable":"MERGEABLE","headRefOid":"verified-head"}))
                }, || authorize(dir, &state_ref.lock().unwrap(), "peer", key, "task.pullMerge"), |_, _| {
                    effects_ref.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }));
                entered_rx.recv().unwrap();
                {
                    let mut inner = state.lock().unwrap();
                    match change {
                        "control" | "pending" | "blocked" => set_scope_with(&fixture.dir, &mut inner, &fixture.peer, Scope::parse(change).unwrap(), Some(&fixture.key), config::save).unwrap(),
                        "full-off" => {
                            let mut next = inner.settings.clone();
                            next.allow_full_access = false;
                            super::super::persist_scoped_revocation(&fixture.dir, &mut inner, next, false, downgrade_full_access).unwrap();
                        }
                        "host-off" => {
                            let mut next = inner.settings.clone();
                            next.host_enabled = false;
                            super::super::update_access(&fixture.dir, &mut inner, next, true).unwrap();
                        }
                        "key-rotation" => inner.devices[0].public_key = crypto::public_key(&crypto::generate()),
                        "lockdown" => config::begin_lockdown(&fixture.dir).unwrap(),
                        _ => unreachable!(),
                    }
                }
                resume_tx.send(()).unwrap();
                assert!(pending.join().unwrap().is_err(), "{change} must cancel the prepared merge");
            });
            assert_eq!(effects.load(std::sync::atomic::Ordering::SeqCst), 0, "no merge command after {change}");
        }
    }

    #[test]
    fn authorized_merge_keeps_the_verified_head_and_executes_once() {
        let fixture = AccessFixture::new(Scope::Full);
        let effects = std::cell::Cell::new(0);
        crate::github_pull::merge_after_lookup(42, "squash", || {
            crate::github_pull::parse_detail(&json!({"number":42,"state":"OPEN","mergeable":"MERGEABLE","headRefOid":"verified-head"}))
        }, || authorize(&fixture.dir, &fixture.inner, "peer", &fixture.key, "task.pullMerge"), |flag, head| {
            assert_eq!((flag, head), ("--squash", "verified-head"));
            effects.set(effects.get() + 1);
            Ok(())
        }).unwrap();
        assert_eq!(effects.get(), 1);
    }

    #[test]
    fn scope_matrix_covers_every_host_method() {
        for method in super::super::host::METHODS {
            let listed = VIEW.contains(&method) || CONTROL.contains(&method) || FULL_ONLY.contains(&method) || method == REDEEM;
            assert!(listed, "{method} is not classified in grants.rs");
        }
        let mut seen = HashSet::new();
        for method in VIEW.iter().chain(&CONTROL).chain(&FULL_ONLY) {
            assert!(seen.insert(*method), "{method} is listed twice");
        }
        for method in VIEW.iter().chain(&CONTROL) {
            assert!(super::super::host::METHODS.contains(method), "{method} is not a host method");
        }
    }

    #[test]
    fn each_scope_allows_exactly_its_methods() {
        for method in VIEW {
            assert!(Scope::View.allows(method) && Scope::Control.allows(method) && Scope::Full.allows(method), "{method}");
        }
        for method in CONTROL {
            assert!(!Scope::View.allows(method) && Scope::Control.allows(method) && Scope::Full.allows(method), "{method}");
        }
        for method in FULL_ONLY {
            assert!(!Scope::View.allows(method) && !Scope::Control.allows(method) && Scope::Full.allows(method), "{method}");
        }
        for scope in [Scope::Pending, Scope::Blocked] {
            for method in VIEW.iter().chain(&CONTROL).chain(&FULL_ONLY) {
                assert!(!scope.allows(method), "{method}");
            }
        }
        // Unknown methods are for full only.
        assert!(Scope::Full.allows("something.new"));
        assert!(!Scope::Control.allows("something.new") && !Scope::View.allows("something.new"));
        assert!(!Scope::Control.allows("task.start") && !Scope::Control.allows("task.pullMerge"));
    }

    #[test]
    fn a_new_device_waits_and_is_announced_once() {
        let mut grants = Grants::default();
        let phone = peer("phone", "aa11");
        assert_eq!(grants.decide(&phone, "tasks.list"), Verdict::Pending { announce: true });
        assert_eq!(grants.decide(&phone, "subscribe"), Verdict::Pending { announce: false });
        assert!(grants.dirty);
        assert_eq!(grants.store.devices["phone"].scope, Scope::Pending);
        assert_eq!(grants.decide(&phone, REDEEM), Verdict::Allow);
        grants.set(&phone, Scope::View);
        assert_eq!(grants.decide(&phone, "tasks.list"), Verdict::Allow);
        assert_eq!(grants.decide(&phone, "task.send"), Verdict::Denied(Scope::View));
    }

    #[test]
    fn a_changed_key_falls_back_to_pending() {
        let mut grants = Grants::default();
        granted(&mut grants, "phone", "aa11", Scope::Full);
        assert_eq!(grants.decide(&peer("phone", "aa11"), "task.start"), Verdict::Allow);
        let swapped = peer("phone", "bb22");
        assert_eq!(grants.scope_of(&swapped), Scope::Pending);
        assert_eq!(grants.decide(&swapped, "tasks.list"), Verdict::Pending { announce: true });
        assert_eq!(grants.store.devices["phone"].fingerprint, "bb22");
        // The old key no longer matches the new pending entry either.
        assert_eq!(grants.scope_of(&peer("phone", "aa11")), Scope::Pending);
        // A device without a key is never approved.
        granted(&mut grants, "keyless", "", Scope::Full);
        assert_eq!(grants.scope_of(&peer("keyless", "")), Scope::Pending);
    }

    #[test]
    fn blocked_stays_blocked_across_key_changes() {
        let mut grants = Grants::default();
        granted(&mut grants, "phone", "aa11", Scope::Blocked);
        for key in ["aa11", "bb22"] {
            assert_eq!(grants.decide(&peer("phone", key), "tasks.list"), Verdict::Denied(Scope::Blocked));
            assert_eq!(grants.decide(&peer("phone", key), REDEEM), Verdict::Denied(Scope::Blocked));
        }
        grants.new_code(Scope::Full, "123456".into(), 0);
        assert!(grants.redeem(&peer("phone", "aa11"), "123456", 1).is_err());
        assert!(grants.code.is_some());
    }

    #[test]
    fn upgrades_never_auto_grant_devices() {
        let mut grants = Grants { store: Store { adopt: true, ..Default::default() }, ..Default::default() };
        let known = [peer("laptop", "aa11"), peer("nas", "bb22")];
        grants.adopt(&known);
        assert!(!grants.store.adopt && grants.dirty);
        assert_eq!(grants.decide(&known[0], "task.start"), Verdict::Pending { announce: true });
        // A device that shows up afterwards starts pending, and its key still binds.
        grants.adopt(&[peer("new", "cc33")]);
        assert_eq!(grants.decide(&peer("new", "cc33"), "tasks.list"), Verdict::Pending { announce: true });
        assert_eq!(grants.scope_of(&peer("laptop", "other")), Scope::Pending);
    }

    #[test]
    fn an_empty_first_presence_still_ends_adoption() {
        let mut grants = Grants { store: Store { adopt: true, ..Default::default() }, ..Default::default() };
        grants.adopt(&[]);
        assert!(!grants.store.adopt && grants.dirty, "only the first presence frame adopts");
        grants.adopt(&[peer("later", "dd44")]);
        assert_eq!(grants.scope_of(&peer("later", "dd44")), Scope::Pending);
    }

    #[test]
    fn only_a_blocked_device_is_blocked() {
        let mut grants = Grants::default();
        grants.set(&peer("phone", "aa11"), Scope::Blocked);
        grants.set(&peer("laptop", "bb22"), Scope::Full);
        grants.note_pending(&peer("new", "cc33"));
        assert!(grants.blocks("phone"));
        assert!(!grants.blocks("laptop") && !grants.blocks("new") && !grants.blocks("unknown"));
    }


    #[test]
    fn a_blocked_device_is_denied_even_when_its_key_rotated() {
        let mut grants = Grants::default();
        let first = peer("device", "aa11");
        grants.set(&first, Scope::Full);
        let changed = peer("device", "different-fingerprint");
        grants.set(&changed, Scope::Blocked);
        assert_eq!(grants.scope_of(&first), Scope::Blocked);
        assert_eq!(grants.scope_of(&changed), Scope::Blocked);
        assert_eq!(grants.decide(&changed, "tasks.list"), Verdict::Denied(Scope::Blocked));
    }

    #[test]
    fn legacy_untrust_cannot_reactivate_a_saved_full_grant() {
        let dir = std::env::temp_dir().join(format!("pandacode-revoke-scope-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = peer("device", "fingerprint");
        // Exercise the real persistent helpers rather than only the Scope enum.
        with(&dir, |grants| grants.set(&p, Scope::Full)).unwrap();
        assert_eq!(with(&dir, |grants| grants.scope_of(&p)).unwrap(), Scope::Full);
        revoke_legacy_trust(&dir, "device").unwrap();
        assert_eq!(with(&dir, |grants| grants.scope_of(&p)).unwrap(), Scope::Pending);
        assert_eq!(Grants::load(&dir).scope_of(&p), Scope::Pending);
        with(&dir, |grants| grants.set(&p, Scope::Full)).unwrap();
        downgrade_full_access(&dir).unwrap();
        assert_eq!(Grants::load(&dir).scope_of(&p), Scope::Control);
        clear_on_sign_out(&dir).unwrap();
        assert_eq!(Grants::load(&dir).scope_of(&p), Scope::Pending);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn seeding_and_saving_round_trip_through_the_private_file() {
        let dir = std::env::temp_dir().join(format!("pandacode-grants-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut settings = config::Settings::default();
        let pinned_key = crypto::public_key(&crypto::generate());
        settings.trusted.insert("laptop".into(), pinned_key.clone());
        seed_trusted(&dir, &settings).unwrap();
        let mut grants = Grants::load(&dir);
        assert!(!grants.store.adopt);
        grants.adopt(&[peer("laptop", "aa11")]);
        let store = grants.store.clone();
        crate::secret_store::update(&dir, FILE, LABEL, |_: Store| store).unwrap();
        let reloaded = Grants::load(&dir);
        assert!(!reloaded.store.adopt);
        assert_eq!(reloaded.scope_of(&Peer { fingerprint: crypto::fingerprint(&pinned_key), ..peer("laptop", "aa11") }), Scope::Control);
        assert_eq!(reloaded.scope_of(&peer("new", "cc33")), Scope::Pending);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.join(FILE)).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::write(dir.join(FILE), "{nope").unwrap();
        assert_eq!(Grants::load(&dir).store, Store::default());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_code_is_single_use_and_expires() {
        let mut grants = Grants::default();
        let phone = peer("phone", "aa11");
        let expires = grants.new_code(Scope::Control, "042917".into(), 1_000);
        assert_eq!(expires, 1_000 + 600);
        assert!(grants.redeem(&phone, "000000", 1_001).is_err());
        assert_eq!(grants.scope_of(&phone), Scope::Pending);
        assert_eq!(grants.redeem(&phone, "042 917", 1_002), Ok(Scope::Control));
        assert_eq!(grants.decide(&phone, "task.send"), Verdict::Allow);
        // Used: a second device cannot reuse it.
        let other = peer("other", "bb22");
        assert!(grants.redeem(&other, "042917", 1_003).is_err());
        assert_eq!(grants.scope_of(&other), Scope::Pending);
        // Expired exactly at the deadline.
        grants.new_code(Scope::View, "111111".into(), 2_000);
        assert!(grants.redeem(&other, "111111", 2_600).is_err());
        assert!(grants.code.is_none());
        // Cancelling or replacing invalidates the old code.
        grants.new_code(Scope::View, "222222".into(), 3_000);
        grants.new_code(Scope::Full, "333333".into(), 3_001);
        assert!(grants.redeem(&other, "222222", 3_002).is_err());
        assert_eq!(grants.redeem(&other, "333333", 3_003), Ok(Scope::Full));
    }

    #[test]
    fn wrong_guesses_burn_the_code_and_redeeming_never_downgrades() {
        let mut grants = Grants::default();
        let phone = peer("phone", "aa11");
        grants.new_code(Scope::View, "424242".into(), 0);
        for _ in 0..MAX_TRIES {
            assert!(grants.redeem(&phone, "999999", 1).is_err());
        }
        assert!(grants.redeem(&phone, "424242", 2).is_err(), "the code is gone after too many wrong guesses");
        granted(&mut grants, "tablet", "dd44", Scope::Full);
        grants.new_code(Scope::View, "424242".into(), 10);
        assert_eq!(grants.redeem(&peer("tablet", "dd44"), "424242", 11), Ok(Scope::Full));
        assert!(grants.redeem(&phone, "12345", 11).is_err());
    }

    #[test]
    fn the_list_puts_waiting_devices_first_with_their_scope() {
        let mut grants = Grants::default();
        let (a, b, c) = (peer("a", "aa"), peer("b", "bb"), peer("c", "cc"));
        grants.set(&a, Scope::View);
        grants.set(&b, Scope::Blocked);
        grants.decide(&c, "tasks.list");
        let list = grants.view(&[a, b, peer("d", "dd"), c]);
        let ids: Vec<_> = list.iter().map(|row| row["deviceId"].as_str().unwrap()).collect();
        assert_eq!(ids[0], "c");
        assert_eq!(list[0]["scope"], "pending");
        assert_eq!(list[0]["requested"], true);
        let row = |id: &str| list.iter().find(|row| row["deviceId"] == id).unwrap().clone();
        assert_eq!((row("a")["scope"].clone(), row("b")["scope"].clone(), row("d")["scope"].clone(), row("d")["requested"].clone()), (json!("view"), json!("blocked"), json!("pending"), json!(false)));
    }

    #[test]
    fn scopes_parse_by_name_and_refusals_name_the_host() {
        for scope in [Scope::Pending, Scope::View, Scope::Control, Scope::Full, Scope::Blocked] {
            assert_eq!(Scope::parse(scope.name()), Ok(scope));
        }
        assert!(Scope::parse("admin").is_err());
        assert_eq!(refusal(Scope::Blocked, "Studio"), "Studio has blocked this device.");
        assert!(refusal(Scope::View, "Studio").contains("only view"));
        assert_eq!(random_code().len(), 6);
    }
}
