//! The one WebSocket to the relay: reconnects with backoff, pings, tracks presence,
//! and seals and opens every frame between this computer and the others. A frame goes on
//! a device's direct link instead when one is up ([`super::routes`]).
use super::{auth, config, crypto, emit_presence, emit_status, host, mirror, remote, routes, sign_out_local, Device, Inner};
use crate::Result;
use futures_util::{future::BoxFuture, stream::FuturesUnordered, SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest, Message};

const PING: Duration = Duration::from_secs(25);
/// A ping must be answered (by anything from the relay) this soon, or the socket is dead.
const PONG: Duration = Duration::from_secs(10);
/// How often the socket checks for a missed pong and for the computer having slept.
const CHECK: Duration = Duration::from_secs(2);
/// The wall clock moving this much further than the monotonic one means the computer slept
/// (the monotonic clock and tokio's timers stand still meanwhile).
const SLEPT: Duration = Duration::from_secs(10);
/// A frame the socket can't take this long marks it dead.
const WRITE: Duration = Duration::from_secs(10);
/// Frames waiting for the socket.
const OUTBOX: usize = 512;
/// How often a connected computer checks whether its token is due for a refresh.
const REFRESH: Duration = Duration::from_secs(12 * 3600);
/// The error a call gets when the relay can't deliver it to an offline computer.
pub(crate) const OFFLINE: &str = "That computer is offline.";
/// Hosts keep a subscription for 10 minutes; it is renewed before then.
const RENEW: Duration = Duration::from_secs(9 * 60);
const FIRST_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// A request that waited longer than this in its peer's queue is dropped unanswered by its
/// work: its caller gave up ([`super::client::TIMEOUT`]) and may have sent it again.
const EXPIRED: Duration = Duration::from_secs(super::client::TIMEOUT.as_secs() - 2);
/// A socket must stay up this long before the backoff resets.
const STABLE: Duration = Duration::from_secs(30);
pub(super) const MAX_FRAME: usize = 256 * 1024;
/// The error a call gets when the relay (or a direct link) refused it for the frame rate.
pub(super) const RATE_LIMITED: &str = "Too many requests to that computer. Try again in a moment.";
const MAX_STARTS: usize = 8;
const PEER_QUEUE: usize = 64;
const MAX_PEERS: usize = 64;
/// Relay close codes: another socket replaced this one, or the device was revoked.
const REPLACED: u16 = 4000;
const REVOKED: u16 = 4001;

#[derive(Debug, PartialEq)]
enum Outcome {
    Closed { up: Duration, code: Option<u16> },
    /// The computer slept: the socket is surely dead, so reconnect at once.
    Woke,
    Failed,
    Unauthorized,
}

/// (wait now, base for the next failure), or None to sign out. The first retry after a
/// stable socket (or a wake) is immediate, then 1 s, 2 s, 4 s… up to a minute.
fn backoff(delay: Duration, outcome: &Outcome) -> Option<(Duration, Duration)> {
    let base = match outcome {
        Outcome::Unauthorized | Outcome::Closed { code: Some(REVOKED), .. } => return None,
        Outcome::Woke => Duration::ZERO,
        Outcome::Closed { up, .. } if *up >= STABLE => Duration::ZERO,
        _ => delay,
    };
    let wait = if matches!(outcome, Outcome::Closed { code: Some(REPLACED), .. }) { base.max(MAX_BACKOFF) } else { base };
    Some((wait, (base * 2).clamp(FIRST_BACKOFF, MAX_BACKOFF)))
}

/// Spreads waits over 75–125 %, so computers that lost the relay together don't return together.
fn jitter(wait: Duration, random: f64) -> Duration {
    wait.mul_f64(0.75 + 0.5 * random.clamp(0.0, 1.0))
}

/// Whether the wall clock ran ahead of the monotonic clock: the computer slept.
fn slept(wall: Option<Duration>, monotonic: Duration) -> bool {
    wall.is_some_and(|wall| wall > monotonic + SLEPT)
}

/// Whether a ping sent at `since` went unanswered too long.
fn unanswered(since: Option<Instant>, now: Instant) -> bool {
    since.is_some_and(|since| now.saturating_duration_since(since) > PONG)
}

/// Sends one frame, or false when the socket failed or could not take it in time.
async fn send<S: futures_util::Sink<Message> + Unpin>(write: &mut S, message: Message) -> bool {
    matches!(tokio::time::timeout(WRITE, write.send(message)).await, Ok(Ok(())))
}

/// Starts (or restarts) the socket task while signed in.
pub(crate) fn start(app: &tauri::AppHandle) {
    let generation = {
        let state = remote(app);
        let mut inner = state.lock();
        if inner.token.is_none() {
            return;
        }
        inner.generation += 1;
        inner.outbox = None;
        inner.generation
    };
    tauri::async_runtime::spawn(run(app.clone(), generation));
    tauri::async_runtime::spawn(routes::supervise(app.clone(), generation));
}

pub(super) fn current(app: &tauri::AppHandle, generation: u64) -> bool {
    remote(app).lock().generation == generation
}

/// The relay is gone: its calls fail, except those to a device with a direct link up, which
/// may still be answered on it.
fn fail_relayed(inner: &mut Inner) {
    let lost: Vec<String> = inner.pending.iter().filter(|(_, (device, _))| !routes::direct(device)).map(|(id, _)| id.clone()).collect();
    for id in lost {
        if let Some((_, reply)) = inner.pending.remove(&id) {
            let _ = reply.send(Err(super::client::LOST.to_owned()));
        }
    }
}

async fn run(app: tauri::AppHandle, generation: u64) {
    let mut delay = Duration::ZERO;
    let mut attempt = "socket.connecting";
    while current(&app, generation) {
        let Some(token) = remote(&app).lock().token.clone() else { return };
        super::diagnostics::record(attempt, "");
        let outcome = session(&app, generation, &token).await;
        attempt = record_outcome(&outcome);
        let changed = {
            let state = remote(&app);
            let mut inner = state.lock();
            let changed = inner.generation == generation && inner.connected;
            if inner.generation == generation {
                inner.connected = false;
                inner.outbox = None;
                inner.presence = false;
                fail_relayed(&mut inner);
            }
            changed
        };
        let Some((wait, next)) = backoff(delay, &outcome) else {
            if current(&app, generation) {
                sign_out_local(&app, Some(&token));
            }
            return;
        };
        if changed {
            emit_status(&app);
        }
        if !wait.is_zero() {
            tokio::time::sleep(jitter(wait, (uuid::Uuid::new_v4().as_u128() % 1000) as f64 / 1000.0)).await;
        }
        delay = next;
    }
}

/// Logs how a socket ended; returns the kind of the next attempt.
fn record_outcome(outcome: &Outcome) -> &'static str {
    let (kind, detail, next) = match outcome {
        Outcome::Closed { up, code } => ("socket.close", format!("Closed after {}s{}", up.as_secs(), code.map(|c| format!(" (code {c})")).unwrap_or_default()), "socket.reconnect"),
        Outcome::Woke => ("socket.close", "The computer slept".to_owned(), "socket.reconnect"),
        Outcome::Failed => ("socket.error", "Could not reach the relay".to_owned(), "socket.retry"),
        Outcome::Unauthorized => ("socket.error", "The relay refused the sign-in".to_owned(), "socket.retry"),
    };
    super::diagnostics::record(kind, detail);
    next
}

async fn session(app: &tauri::AppHandle, generation: u64, token: &str) -> Outcome {
    let Some(build) = config::build() else { return Outcome::Failed };
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let hosting = remote(app).lock().settings.host_enabled;
    let url = format!("wss://{}/v1/connect?host={}", build.relay.trim_start_matches("https://"), u8::from(hosting));
    let Ok(mut request) = url.into_client_request() else { return Outcome::Failed };
    let Ok(authorization) = format!("Bearer {token}").parse() else { return Outcome::Unauthorized };
    request.headers_mut().insert("Authorization", authorization);
    let mut socket_config = tungstenite::protocol::WebSocketConfig::default();
    socket_config.max_message_size = Some(MAX_FRAME);
    socket_config.max_frame_size = Some(MAX_FRAME);
    let stream = match tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async_with_config(request, Some(socket_config), false)).await {
        Ok(Ok((stream, _))) => stream,
        Ok(Err(tungstenite::Error::Http(response))) if matches!(response.status().as_u16(), 401 | 403) => return Outcome::Unauthorized,
        _ => return Outcome::Failed,
    };
    let opened = Instant::now();
    let (mut write, mut read) = stream.split();
    let (tx, mut rx) = mpsc::channel::<String>(OUTBOX);
    let host_enabled = {
        let state = remote(app);
        let mut inner = state.lock();
        if inner.generation != generation {
            return Outcome::Failed;
        }
        inner.outbox = Some(tx);
        inner.connected = true;
        inner.settings.host_enabled
    };
    super::diagnostics::record("socket.open", "");
    emit_status(app);
    let closed = |code| Outcome::Closed { up: opened.elapsed(), code };
    // Hosting was toggled while connecting, before the outbox could carry it.
    if host_enabled != hosting && !send(&mut write, Message::text(json!({"kind": "host", "enabled": host_enabled}).to_string())).await {
        return closed(None);
    }
    let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + PING, PING);
    let mut check = tokio::time::interval_at(tokio::time::Instant::now() + CHECK, CHECK);
    // The first subscription follows the first presence frame.
    let mut renew = tokio::time::interval_at(tokio::time::Instant::now() + RENEW, RENEW);
    // The first tick is now: a token is checked at connect, then every 12 h.
    let mut refresh = tokio::time::interval(REFRESH);
    // An unanswered ping, and the clocks at the last check.
    let mut pinged: Option<Instant> = None;
    let (mut wall, mut monotonic) = (SystemTime::now(), Instant::now());
    loop {
        tokio::select! {
            _ = refresh.tick() => {
                let token = remote(app).lock().token.clone();
                if let Some(token) = token.filter(|token| config::needs_refresh(token, chrono::Utc::now().timestamp())) {
                    tauri::async_runtime::spawn(refresh_token(app.clone(), token));
                }
            }
            // None once stop() or a restart drops the outbox.
            outgoing = rx.recv() => match outgoing {
                Some(text) => if !send(&mut write, Message::text(text)).await { return closed(None) },
                None => {
                    let _ = send(&mut write, Message::Close(None)).await;
                    return closed(None);
                }
            },
            _ = renew.tick() => mirror::subscribe_soon(app),
            _ = ping.tick() => {
                if !send(&mut write, Message::text("ping")).await {
                    return closed(None);
                }
                pinged.get_or_insert_with(Instant::now);
            }
            _ = check.tick() => {
                let (now_wall, now) = (SystemTime::now(), Instant::now());
                if slept(now_wall.duration_since(wall).ok(), now.duration_since(monotonic)) {
                    return Outcome::Woke;
                }
                if unanswered(pinged, now) {
                    return closed(None);
                }
                (wall, monotonic) = (now_wall, now);
            }
            incoming = read.next() => {
                // Anything from the relay answers the ping.
                pinged = None;
                match incoming {
                    Some(Ok(Message::Text(text))) => frame(app, text.as_str()),
                    Some(Ok(Message::Close(close))) => return closed(close.map(|frame| u16::from(frame.code))),
                    Some(Err(_)) | None => return closed(None),
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

/// Trades a token in its last week for a fresh 30-day one (`POST /v1/token`). Failures are
/// left for the next check; a refused token signs out as any 401 does.
async fn refresh_token(app: tauri::AppHandle, old: String) {
    let Some(build) = config::build() else { return };
    let reply = match auth::relay(&build, &old, reqwest::Method::POST, "/v1/token").await {
        Ok(reply) => reply,
        Err(error) if error == auth::SIGNED_OUT => return sign_out_local(&app, Some(&old)),
        Err(_) => return,
    };
    let Some(fresh) = reply["token"].as_str().filter(|token| token.len() <= 4096) else { return };
    let (Some(was), Some(now)) = (config::parse_token(&old), config::parse_token(fresh)) else { return };
    if now.acct != was.acct || now.dev != was.dev || now.exp <= was.exp {
        return;
    }
    let state = remote(&app);
    {
        let mut inner = state.lock();
        // Signed out or in again meanwhile: the old token is no longer the one to replace.
        if inner.token.as_deref() != Some(old.as_str()) {
            return;
        }
        inner.token = Some(fresh.to_owned());
    }
    if let Err(error) = crate::secret_store::set(config::SERVICE, &state.accounts.token, fresh) {
        eprintln!("Could not save the refreshed PandaCode Connect token: {error}");
    }
}

/// A request's authenticated peer key and queue time. Identity is rechecked at
/// dispatch and delivery because revocation may occur while it is queued.
struct Request {
    message: Value,
    key: crypto::Key,
    public_key: String,
    queued_at: Instant,
}

impl Request {
    fn new(message: Value, key: crypto::Key, public_key: String) -> Self {
        Self { message, key, public_key, queued_at: Instant::now() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Lane {
    /// Overlapping, up to a small bound: reads, and `task.cancelStart`, which must never
    /// wait behind the start it cancels.
    Read,
    /// One at a time, in arrival order: everything that changes a task.
    Ordered,
    /// Each `task.start` on its own (bounded by [`MAX_STARTS`]): a clone can take minutes,
    /// and stops, answers and prompts must not wait behind it.
    Start,
}

fn lane(method: &str) -> Lane {
    match method {
        "task.start" => Lane::Start,
        "batch" | "task.cancelStart" => Lane::Read,
        method if host::is_read_only_method(method) => Lane::Read,
        _ => Lane::Ordered,
    }
}

#[derive(Default)]
struct PendingRequests {
    reads: VecDeque<Request>,
    starts: VecDeque<Request>,
    ordered: VecDeque<Request>,
}

impl PendingRequests {
    fn len(&self) -> usize {
        self.reads.len() + self.ordered.len() + self.starts.len()
    }

    fn push(&mut self, request: Request) {
        match lane(request.message["method"].as_str().unwrap_or_default()) {
            Lane::Read => self.reads.push_back(request),
            Lane::Ordered => self.ordered.push_back(request),
            Lane::Start => self.starts.push_back(request),
        }
    }

    fn next_ordered(&mut self, active: &mut bool) -> Option<Request> {
        if *active {
            return None;
        }
        let request = self.ordered.pop_front()?;
        *active = true;
        Some(request)
    }
}


struct SendStats {
    payload_bytes: usize,
    frame_bytes: usize,
    json_micros: u128,
    seal_micros: u128,
}

/// One relay frame: presence, delivery failures and errors, or an encrypted message.
fn frame(app: &tauri::AppHandle, text: &str) {
    if text == "pong" {
        return;
    }
    let Ok(value) = serde_json::from_str::<Value>(text) else { return };
    match value["kind"].as_str() {
        Some("presence") => presence(app, &value["devices"]),
        Some("undeliverable") => {
            if let Some(to) = value["to"].as_str() {
                let error = if value["reason"] == "rate" { RATE_LIMITED } else { OFFLINE };
                remote(app).lock().fail_ref(to, value["ref"].as_str(), error);
            }
        }
        Some("error") => eprintln!("PandaCode Connect relay: {}", value["error"].as_str().unwrap_or("error")),
        _ => {
            let (Some(from), Some(to), Some(body)) = (value["from"].as_str(), value["to"].as_str(), value["body"].as_str()) else { return };
            sealed(app, from, to, body);
        }
    }
}

/// An encrypted message from `from` to `to`, off the relay or a direct link: opened with the
/// pair key from `from`'s presence key. Responses and events are handled at once; only
/// requests wait in their peer's queue.
pub(super) fn sealed(app: &tauri::AppHandle, from: &str, to: &str, body: &str) {
    if host::valid_id(from).is_err() {
        return;
    }
    let me = remote(app).device_id();
    if to != me || from == me {
        return;
    }
    let Ok((key, authenticated_key)) = peer_key(app, from) else { return };
    let Ok(plain) = crypto::open(&key, from, to, body) else { return };
    let Ok(message) = serde_json::from_slice::<Value>(&plain) else { return };
    if matches!(message["type"].as_str(), Some("res" | "event")) && !fresh(app, from, &message) {
        return;
    }
    match message["type"].as_str() {
        Some("req") => queue(app, from, Request::new(message, key, authenticated_key)),
        Some("res") => super::client::resolve(app, from, &message),
        Some("event") => match message["name"].as_str() {
            Some("agent.events") => mirror::events(app, from, &message["data"]),
            Some("run.finished") => mirror::finished(app, from, &message["data"]),
            Some("task.progress") => super::start::progress(app, from, &message["data"]),
            Some(routes::ANNOUNCE) => routes::announced(app, from, &message["data"]),
            _ => {}
        },
        _ => {}
    }
}

/// The error a call gets when its reply is stale, repeated, or from a host without `ts`/`nonce`.
const STALE: &str = "That computer's reply was stale or repeated. Update PandaCode on both computers and check their clocks.";

/// Replies and events carry `ts` and `nonce` like requests; a stale or repeated one is
/// dropped (a reply fails its call at once instead of timing out).
fn fresh(app: &tauri::AppHandle, from: &str, message: &Value) -> bool {
    let checked = remote(app).lock().replay.check(message["ts"].as_i64(), message["nonce"].as_str(), chrono::Utc::now().timestamp_millis());
    if checked.is_err() {
        eprintln!("PandaCode Connect: dropped a stale or repeated {} from {from}.", message["type"].as_str().unwrap_or_default());
        if message["type"] == "res" {
            super::client::resolve(app, from, &json!({"id": message["id"], "ok": false, "error": STALE}));
        }
    }
    checked.is_ok()
}

/// How long a request id is remembered with its answer, and the bounds on what is kept.
const ANSWERED: Duration = Duration::from_secs(60);
const MAX_ANSWERED: usize = 512;
const MAX_ANSWER_BYTES: usize = 4 * 1024 * 1024;

enum Seen {
    Running,
    /// The answer sent, or None when it was too large to keep.
    Answered(Option<Value>, usize),
}

enum Repeat {
    New,
    Running,
    Answered(Value),
}

/// Requests by (device, id) for a minute. A viewer retries a request on the relay after its
/// direct link failed (the identical sealed frame); the copy is answered again from here, or
/// dropped while the first one runs, so a request never runs twice whichever route carried it.
#[derive(Default)]
struct Answers {
    seen: HashMap<(String, String), (Instant, Seen)>,
    bytes: usize,
}

impl Answers {
    fn prune(&mut self, now: Instant) {
        let bytes = &mut self.bytes;
        self.seen.retain(|_, (at, seen)| {
            let keep = now.saturating_duration_since(*at) < ANSWERED;
            if let (false, Seen::Answered(Some(_), size)) = (keep, seen) {
                *bytes -= *size;
            }
            keep
        });
    }

    fn arrive(&mut self, from: &str, id: &str, now: Instant) -> Repeat {
        self.prune(now);
        let key = (from.to_owned(), id.to_owned());
        match self.seen.get(&key) {
            Some((_, Seen::Running)) => return Repeat::Running,
            Some((_, Seen::Answered(Some(answer), _))) => return Repeat::Answered(answer.clone()),
            // Not kept: the copy goes on, and its repeated nonce gets it refused.
            Some((_, Seen::Answered(None, _))) => return Repeat::New,
            None => {}
        }
        if self.seen.len() >= MAX_ANSWERED {
            if let Some(oldest) = self.seen.iter().min_by_key(|(_, (at, _))| *at).map(|(key, _)| key.clone()) {
                if let Some((_, Seen::Answered(Some(_), size))) = self.seen.remove(&oldest) {
                    self.bytes -= size;
                }
            }
        }
        self.seen.insert(key, (now, Seen::Running));
        Repeat::New
    }

    fn answered(&mut self, to: &str, id: &str, answer: &Value, size: usize) {
        let Some((_, seen)) = self.seen.get_mut(&(to.to_owned(), id.to_owned())) else { return };
        if !matches!(seen, Seen::Running) {
            return;
        }
        let kept = self.bytes + size <= MAX_ANSWER_BYTES;
        if kept {
            self.bytes += size;
        }
        *seen = Seen::Answered(kept.then(|| answer.clone()), size);
    }
}

fn answers() -> std::sync::MutexGuard<'static, Answers> {
    static ANSWERS: std::sync::OnceLock<std::sync::Mutex<Answers>> = std::sync::OnceLock::new();
    ANSWERS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// One ordered request queue per peer, shared by the relay and direct links, for one sign-in.
fn peers() -> std::sync::MutexGuard<'static, (u64, HashMap<String, mpsc::Sender<Request>>)> {
    static PEERS: std::sync::OnceLock<std::sync::Mutex<(u64, HashMap<String, mpsc::Sender<Request>>)>> = std::sync::OnceLock::new();
    PEERS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// Queues a request for its peer. Freshness and replays are checked here, on arrival, so
/// time spent waiting in the queue never makes a valid request expire.
fn queue(app: &tauri::AppHandle, from: &str, request: Request) {
    if let Some(id) = request.message["id"].as_str().filter(|id| host::valid_id(id).is_ok()) {
        let repeat = answers().arrive(from, id, Instant::now());
        match repeat {
            Repeat::New => {}
            Repeat::Running => return,
            Repeat::Answered(answer) => {
                // The device may have lost access since: check as a new request would be.
                let allowed = still_allowed(app, from, &request.public_key, &request.message);
                let answer = cached_reply(id, answer, allowed);
                let pairing = request.message["method"] == super::grants::REDEEM;
                let _ = send_sealed(app, from, &request.key, Some(&request.public_key), pairing, true, &answer);
                return;
            }
        }
    }
    let (fresh, generation) = {
        let state = remote(app);
        let mut inner = state.lock();
        (host::fresh_request(&mut inner, &request.message, chrono::Utc::now().timestamp_millis()), inner.generation)
    };
    if let Err(error) = fresh {
        return refuse(app, from, request, &error);
    }
    let refused = {
        let mut peers = peers();
        if peers.0 != generation {
            // Another sign-in or a reconnect: dropping the senders stops the old workers.
            *peers = (generation, HashMap::new());
        }
        let peers = &mut peers.1;
        if !peers.contains_key(from) && peers.len() >= MAX_PEERS {
            Some(request)
        } else {
            let tx = peers.entry(from.to_owned()).or_insert_with(|| {
                let (tx, rx) = mpsc::channel(PEER_QUEUE);
                tauri::async_runtime::spawn(peer(app.clone(), from.to_owned(), rx));
                tx
            });
            tx.try_send(request).err().map(|error| error.into_inner())
        }
    };
    if let Some(request) = refused {
        busy(app, from, request);
    }
}

/// `host::handle`'s admission for a repeated request answered from [`Answers`]: remote
/// control on, the sender in presence, and its grant (for a batch, every call's).
fn still_allowed(app: &tauri::AppHandle, from: &str, authenticated_key: &str, message: &Value) -> Result<()> {
    {
        let state = remote(app);
        let inner = state.lock();
        host::gate(inner.settings.host_enabled,
            inner.devices.iter().any(|d| d.id == from && !authenticated_key.is_empty() && d.public_key == authenticated_key),
            &super::config::device_name())?;
        if message["method"] != super::grants::REDEEM {
            host::gate(inner.settings.host_enabled, inner.authenticated(from, authenticated_key), &super::config::device_name())?;
        }
    }
    granted_methods(message).into_iter().try_for_each(|method| super::grants::check(app, from, method))
}

/// The methods a request needs a grant for: its own and each batch call's. `pair.redeem` needs
/// none (a pending device redeems a code; its answer is a refusal or its new scope).
fn granted_methods(message: &Value) -> Vec<&str> {
    let method = message["method"].as_str().unwrap_or_default();
    if method == super::grants::REDEEM {
        return Vec::new();
    }
    let calls = if method == "batch" { message["params"]["calls"].as_array().map(Vec::as_slice).unwrap_or_default() } else { &[] };
    std::iter::once(method).chain(calls.iter().map(|call| call["method"].as_str().unwrap_or_default())).collect()
}

fn cached_reply(id: &str, answer: Value, allowed: Result<()>) -> Value {
    match allowed {
        Ok(()) => answer,
        Err(error) => json!({"type": "res", "id": id, "ok": false, "error": error}),
    }
}

/// Refuses a request at once instead of dropping it, so the caller doesn't wait for its timeout.
fn busy(app: &tauri::AppHandle, from: &str, request: Request) {
    refuse(app, from, request, "This computer is busy. Try again in a moment.");
}

fn refuse(app: &tauri::AppHandle, from: &str, request: Request, error: &str) {
    let failure = json!({"type": "res", "id": request.message["id"], "ok": false, "error": error});
    let pairing = request.message["method"] == super::grants::REDEEM;
    let _ = send_sealed(app, from, &request.key, Some(&request.public_key), pairing, true, &failure);
}

/// Whether a request waited so long that its caller gave up; running it now could run a
/// retried prompt twice. A start is kept: `startId` makes a repeated one harmless.
fn expired(method: &str, waited: Duration) -> bool {
    method != "task.start" && waited >= EXPIRED
}

const READS_PER_PEER: usize = 4;

/// Read-only requests overlap up to a small bound. State-changing requests share one FIFO
/// lane; each `task.start` runs on its own. Response ids let replies return in any order.
async fn peer(app: tauri::AppHandle, from: String, mut requests: mpsc::Receiver<Request>) {
    let mut pending = PendingRequests::default();
    let mut in_flight = FuturesUnordered::<BoxFuture<'static, Lane>>::new();
    let mut active_reads = 0usize;
    let mut active_starts = 0usize;
    let mut ordered_active = false;
    let mut input_open = true;

    loop {
        while active_reads < READS_PER_PEER {
            let Some(request) = pending.reads.pop_front() else { break };
            active_reads += 1;
            let (app, from) = (app.clone(), from.clone());
            in_flight.push(Box::pin(async move {
                answer(&app, &from, request).await;
                Lane::Read
            }));
        }
        while active_starts < MAX_STARTS {
            let Some(request) = pending.starts.pop_front() else { break };
            active_starts += 1;
            let (app, from) = (app.clone(), from.clone());
            in_flight.push(Box::pin(async move {
                answer(&app, &from, request).await;
                Lane::Start
            }));
        }
        if let Some(request) = pending.next_ordered(&mut ordered_active) {
            let (app, from) = (app.clone(), from.clone());
            in_flight.push(Box::pin(async move {
                answer(&app, &from, request).await;
                Lane::Ordered
            }));
        }
        if !input_open && pending.len() == 0 && in_flight.is_empty() {
            break;
        }

        tokio::select! {
            request = requests.recv(), if input_open => match request {
                Some(request) if pending.len() + active_reads + active_starts + usize::from(ordered_active) < PEER_QUEUE => pending.push(request),
                Some(request) => busy(&app, &from, request),
                None => input_open = false,
            },
            completed = in_flight.next(), if !in_flight.is_empty() => match completed {
                Some(Lane::Read) => active_reads -= 1,
                Some(Lane::Start) => active_starts -= 1,
                Some(Lane::Ordered) => ordered_active = false,
                None => {}
            },
        }
    }
}

async fn answer(app: &tauri::AppHandle, from: &str, request: Request) {
    let method = request.message["method"].as_str().unwrap_or("unknown").to_owned();
    let queue_wait = request.queued_at.elapsed();
    if expired(&method, queue_wait) {
        return refuse(app, from, request, "This request waited too long on the other computer and was dropped. Try again.");
    }
    let Request { message, key, public_key, .. } = request;
    let start_waiting = Instant::now();
    let _start_permit = if method == "task.start" {
        static STARTS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
        STARTS.get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_STARTS)))
            .clone().acquire_owned().await.ok()
    } else { None };
    let slot_wait = start_waiting.elapsed();
    let handling = Instant::now();
    let pairing = method == super::grants::REDEEM;
    if let Some(response) = host::handle(app, from, &message, &public_key).await {
        // A remote response is bound to the key that authenticated the original frame,
        // even when the device is revoked or rotates its key during an await.
        if let Err(error) = send_sealed(app, from, &key, Some(&public_key), pairing, true, &response) {
            let failure = json!({"type": "res", "id": response["id"], "ok": false, "error": error});
            let _ = send_sealed(app, from, &key, Some(&public_key), pairing, true, &failure);
        }
    }
    if std::env::var_os("PANDACODE_CONNECT_TIMING").is_some() {
        eprintln!("PandaCode Connect timing method={method} queue_wait_ms={} start_slot_wait_ms={} handle_ms={}",
            queue_wait.as_secs_f64() * 1000.0, slot_wait.as_secs_f64() * 1000.0,
            handling.elapsed().as_secs_f64() * 1000.0);
    }
}

/// Applies a presence list, which carries the full device rows. A host that comes online (or restarted, forgetting subscriptions) is subscribed again.
fn presence(app: &tauri::AppHandle, list: &Value) {
    let Some(list) = list.as_array() else { return };
    let renamed = {
        let state = remote(app);
        let mut inner = state.lock();
        apply_presence(&mut inner, list);
        super::remember_names(&inner.devices)
    };
    emit_presence(app);
    if renamed {
        use tauri::Emitter;
        let _ = app.emit("sessions-changed", ());
    }
    super::grants::presence(app);
    mirror::subscribe_soon(app);
    routes::presence(app);
}

/// The list replaces the known devices. A device missing from it is gone, with its subscription.
/// A device that went offline, came back or changed hosting loses its cached harness lists.
fn apply_presence(inner: &mut Inner, list: &[Value]) {
    let devices = parse_devices(list);
    for old in &inner.devices {
        if !devices.iter().any(|d| d.id == old.id && d.public_key == old.public_key) {
            inner.uploads.forget(&old.id);
            inner.subscribers.remove(&old.id);
        }
        if !devices.iter().any(|d| d.id == old.id && d.online == old.online && d.host == old.host) {
            inner.catalog.forget(&old.id);
        }
    }
    inner.devices = devices;
    inner.presence = true;
    let Inner { devices, subscribers, keys, .. } = inner;
    subscribers.retain(|id, _| devices.iter().any(|d| &d.id == id));
    keys.retain(|id, _| devices.iter().any(|d| &d.id == id));
}

fn parse_devices(list: &[Value]) -> Vec<Device> {
    list.iter().filter_map(|d| serde_json::from_value(d.clone()).ok()).filter(|d: &Device| host::valid_id(&d.id).is_ok()).take(64).collect()
}

/// Fetches the account's devices; used only before the socket's first presence frame.
pub(crate) async fn refresh_devices(app: &tauri::AppHandle) -> Result<()> {
    let build = config::build().ok_or("Remote access is not configured in this build.")?;
    let token = remote(app).lock().token.clone().ok_or("Sign in to PandaCode Connect first.")?;
    let reply = match auth::relay(&build, &token, reqwest::Method::GET, "/v1/devices").await {
        Err(error) if error == auth::SIGNED_OUT => {
            sign_out_local(app, Some(&token));
            return Err(error);
        }
        other => other?,
    };
    let devices = parse_devices(reply["devices"].as_array().map(Vec::as_slice).unwrap_or_default());
    {
        let state = remote(app);
        let mut inner = state.lock();
        // Presence that arrived meanwhile is newer.
        if !inner.presence {
            inner.subscribers.retain(|id, _| devices.iter().any(|d| &d.id == id));
            super::remember_names(&devices);
            inner.devices = devices;
        }
    }
    emit_presence(app);
    Ok(())
}

/// The pair key with a device, derived from its current presence key. Keys are cached
/// by (device id, public key), so key changes derive a fresh pair key.
pub(super) fn peer_key(app: &tauri::AppHandle, device_id: &str) -> Result<(crypto::Key, String)> {
    let state = remote(app);
    let public = {
        let inner = state.lock();
        let public = &inner.devices.iter().find(|d| d.id == device_id).filter(|d| !d.public_key.is_empty()).ok_or(super::UNKNOWN_DEVICE)?.public_key;
        if let Some((_, key)) = inner.keys.get(device_id).filter(|(cached, _)| cached == public) {
            return Ok((*key, public.clone()));
        }
        public.clone()
    };
    let key = crypto::pair_key(&state.secret()?, &state.device_id(), &public, device_id)?;
    state.lock().keys.insert(device_id.to_owned(), (public.clone(), key));
    Ok((key, public))
}

/// Adds `ts` and a fresh `nonce`, so the receiver can refuse replays (docs/remote-access.md).
fn stamp(message: &Value, now_ms: i64) -> Value {
    let mut message = message.clone();
    message["ts"] = json!(now_ms);
    message["nonce"] = json!(uuid::Uuid::new_v4().simple().to_string());
    message
}

/// Seals a stamped message with a known key and queues it as `{to, body, ref}` on the
/// device's direct link when one is up, else on the relay socket. Replies go straight out;
/// only mirrored event frames use the 100 ms flush.
fn send_sealed(app: &tauri::AppHandle, to: &str, key: &crypto::Key, expected_key: Option<&str>, pairing: bool, hosting: bool, message: &Value) -> Result<SendStats> {
    seal_and_send(app, to, key, expected_key, pairing, hosting, message, true)
}

fn seal_and_send(app: &tauri::AppHandle, to: &str, key: &crypto::Key, expected_key: Option<&str>, pairing: bool, hosting: bool, message: &Value, direct: bool) -> Result<SendStats> {
    let me = remote(app).device_id();
    let json_started = Instant::now();
    let plain = stamp_delivery(message, chrono::Utc::now().timestamp_millis()).to_string();
    let payload_bytes = plain.len();
    let mut json_micros = json_started.elapsed().as_micros();
    let seal_started = Instant::now();
    let body = crypto::seal(key, &me, to, plain.as_bytes())?;
    let seal_micros = seal_started.elapsed().as_micros();
    let mut frame = json!({"to": to, "body": body});
    if let Some(id) = message["id"].as_str() { frame["ref"] = json!(id); }
    let json_started = Instant::now();
    let frame = frame.to_string();
    json_micros += json_started.elapsed().as_micros();
    if frame.len() > MAX_FRAME { return Err("That reply is too large to send to another computer.".into()); }
    let frame_bytes = frame.len();
    let stats = SendStats { payload_bytes, frame_bytes, json_micros, seal_micros };
    // Re-check the original peer key, grant and hosting just before releasing data.
    {
        let state = remote(app);
        let inner = state.lock();
        if hosting && !inner.settings.host_enabled { return Err(host::OFF.into()); }
        if let Some(expected) = expected_key {
            let current = inner.devices.iter().any(|d| d.id == to && !expected.is_empty() && d.public_key == expected);
            if !current || (!pairing && !inner.authenticated(to, expected)) {
                return Err("This device is no longer trusted with the key that authenticated its request.".into());
            }
        }
    }
    if let (Some("res"), Some(id)) = (message["type"].as_str(), message["id"].as_str()) {
        answers().answered(to, id, message, payload_bytes);
    }
    if !direct || pairing { return relay(app, frame).map(|_| stats); }
    let request = message["id"].as_str().filter(|_| message["type"] == "req")
        .map(|id| (id, message["method"].as_str().unwrap_or_default()));
    match routes::send(to, frame, request) {
        Ok(()) => Ok(stats),
        Err(frame) => relay(app, frame).map(|_| stats),
    }
}

/// Queues a sealed frame on the relay socket.
pub(super) fn relay(app: &tauri::AppHandle, frame: String) -> Result<()> {
    let state = remote(app);
    let inner = state.lock();
    let outbox = inner.outbox.as_ref().ok_or("Not connected to the relay.")?;
    outbox.try_send(frame).map_err(|error| match error {
        mpsc::error::TrySendError::Full(_) => "Too much is waiting to go to the relay. Try again in a moment.",
        mpsc::error::TrySendError::Closed(_) => "Not connected to the relay.",
    })?;
    Ok(())
}

/// The timestamp and nonce are sealed inside the authenticated payload. A relay
/// cannot freshen captured responses or events; requests already carry both.
fn stamp_delivery(message: &Value, now_ms: i64) -> Value {
    let mut message = message.clone();
    if matches!(message["type"].as_str(), Some("res" | "event")) {
        message["ts"] = json!(now_ms);
        message["nonce"] = json!(uuid::Uuid::new_v4().simple().to_string());
    }
    message
}

/// Requests and task content, sealed only to a device trusted here with its current key.
pub(crate) fn send_trusted(app: &tauri::AppHandle, to: &str, message: &Value) -> Result<()> {
    remote(app).lock().callable(to)?;
    let (key, public_key) = peer_key(app, to)?;
    send_sealed(app, to, &key, Some(&public_key), false, false, message).map(|_| ())
}

/// Events produced by a request remain bound to its original authenticating key.
pub(crate) fn send_authenticated(app: &tauri::AppHandle, to: &str, authenticated_key: &str, message: &Value) -> Result<()> {
    let (key, current_key) = peer_key(app, to)?;
    if current_key != authenticated_key {
        return Err("The requesting device's key changed.".into());
    }
    send_sealed(app, to, &key, Some(authenticated_key), false, true, message).map(|_| ())
}

/// The sole outbound method that is allowed before the local host has granted trust.
/// The host will verify the current peer key, code, and scoped grant before replying.
pub(crate) fn send_pairing(app: &tauri::AppHandle, to: &str, message: &Value) -> Result<()> {
    if message["type"] != "req" || message["method"] != super::grants::REDEEM {
        return Err("Only a pairing-code redemption may be sent without existing trust.".into());
    }
    let (key, public_key) = peer_key(app, to)?;
    seal_and_send(app, to, &key, Some(&public_key), true, false, message, false).map(|_| ())
}

/// Like [`send_trusted`], on the relay only: route announcements.
pub(super) fn send_relay(app: &tauri::AppHandle, to: &str, message: &Value) -> Result<()> {
    remote(app).lock().callable(to)?;
    let (key, public_key) = peer_key(app, to)?;
    seal_and_send(app, to, &key, Some(&public_key), false, false, message, false).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_starts_do_not_block_ordered_task_controls() {
        let mut pending = PendingRequests::default();
        for method in ["task.start", "task.send", "task.messages", "task.stop", "task.team", "approval.respond"] {
            pending.push(Request::new(json!({"method": method}), [0; 32], "pinned".into()));
        }
        let ordered = pending.ordered.iter().map(|request| request.message["method"].as_str().unwrap()).collect::<Vec<_>>();
        let reads = pending.reads.iter().map(|request| request.message["method"].as_str().unwrap()).collect::<Vec<_>>();
        assert_eq!(ordered, ["task.send", "task.stop"]);
        assert_eq!(reads, ["task.messages", "task.team"]);
        assert_eq!(pending.starts.len(), 1, "a start never holds the ordered lane");
        let mut active = false;
        let first = pending.next_ordered(&mut active).unwrap();
        assert_eq!(first.message["method"], "task.send");
        assert_eq!(first.public_key, "pinned");
        assert!(pending.next_ordered(&mut active).is_none());
        active = false;
        assert_eq!(pending.next_ordered(&mut active).unwrap().message["method"], "task.stop");
        pending.push(Request::new(json!({"method": "batch"}), [0; 32], "pinned".into()));
        assert_eq!(pending.reads.back().unwrap().message["method"], "batch");
    }

    #[test]
    fn a_repeated_request_is_checked_again_before_its_cached_answer() {
        let batch = json!({"method": "batch", "params": {"calls": [{"method": "tasks.list"}, {"method": "task.start"}]}});
        assert_eq!(granted_methods(&batch), ["batch", "tasks.list", "task.start"]);
        assert_eq!(granted_methods(&json!({"method": "task.send"})), ["task.send"]);
        assert!(granted_methods(&json!({"method": super::super::grants::REDEEM})).is_empty());
        let answer = json!({"type": "res", "id": "r1", "ok": true, "result": {"secret": 1}});
        assert_eq!(cached_reply("r1", answer.clone(), Ok(())), answer);
        let refused = cached_reply("r1", answer, Err("This device can only view tasks on Mac.".into()));
        assert_eq!((refused["ok"].as_bool(), refused["result"].is_null(), refused["id"].as_str()), (Some(false), true, Some("r1")));
    }

    #[test]
    fn stops_and_answers_never_wait_behind_a_start() {
        assert_eq!(lane("task.start"), Lane::Start);
        for method in ["task.stop", "approval.respond", "input.respond", "task.send", "task.pullMerge"] {
            assert_eq!(lane(method), Lane::Ordered, "{method}");
        }
        // Cancelling and asking about a start overlap with it.
        for method in ["task.cancelStart", "task.startStatus", "subscribe", "batch"] {
            assert_eq!(lane(method), Lane::Read, "{method}");
        }
    }

    #[test]
    fn a_request_its_caller_gave_up_on_is_dropped() {
        assert!(EXPIRED < super::super::client::TIMEOUT);
        assert!(!expired("task.send", Duration::from_secs(5)));
        assert!(expired("task.send", EXPIRED));
        assert!(expired("approval.respond", Duration::from_secs(60)));
        assert!(!expired("task.start", Duration::from_secs(600)), "startId makes a repeated start harmless");
    }

    #[test]
    fn sleep_and_a_dead_socket_are_found_in_seconds() {
        let s = Duration::from_secs;
        assert!(!slept(Some(s(2)), s(2)));
        assert!(!slept(Some(s(11)), s(2)), "a small clock correction");
        assert!(slept(Some(s(3600)), s(2)), "the wall clock ran on while the computer slept");
        assert!(!slept(None, s(2)), "the clock was set back");
        let now = Instant::now();
        assert!(!unanswered(None, now));
        assert!(!unanswered(Some(now - s(5)), now));
        assert!(unanswered(Some(now - PONG - s(1)), now));
        assert!(PING + PONG < Duration::from_secs(60));
    }

    #[test]
    fn every_sealed_message_is_stamped_and_a_replay_is_refused() {
        let event = json!({"type": "event", "name": "run.finished", "data": {}});
        let (a, b) = (stamp(&event, 1_000), stamp(&event, 1_000));
        assert_eq!((a["ts"].as_i64(), a["name"].as_str()), (Some(1_000), Some("run.finished")));
        assert_ne!(a["nonce"], b["nonce"]);
        // The viewer's check: once only, inside the window, and never without the fields.
        let mut inbound = host::Replay::default();
        assert!(inbound.check(a["ts"].as_i64(), a["nonce"].as_str(), 1_500).is_ok());
        assert!(inbound.check(a["ts"].as_i64(), a["nonce"].as_str(), 1_600).is_err(), "repeated");
        assert!(inbound.check(b["ts"].as_i64(), b["nonce"].as_str(), 1_000 + 60_001).is_err(), "stale");
        assert!(inbound.check(event["ts"].as_i64(), event["nonce"].as_str(), 1_000).is_err(), "an old host without ts/nonce");
    }

    #[test]
    fn backoff_resets_only_after_a_stable_socket() {
        let s = Duration::from_secs;
        assert_eq!(backoff(s(8), &Outcome::Failed), Some((s(8), s(16))));
        assert_eq!(backoff(s(40), &Outcome::Failed), Some((s(40), s(60))));
        assert_eq!(backoff(s(8), &Outcome::Closed { up: s(5), code: None }), Some((s(8), s(16))));
        // The first retry is immediate, then 1 s, 2 s…
        assert_eq!(backoff(s(8), &Outcome::Closed { up: s(30), code: None }), Some((s(0), s(1))));
        assert_eq!(backoff(s(0), &Outcome::Failed), Some((s(0), s(1))));
        assert_eq!(backoff(s(1), &Outcome::Failed), Some((s(1), s(2))));
        assert_eq!(backoff(s(30), &Outcome::Woke), Some((s(0), s(1))), "after sleep, at once");
        assert_eq!(backoff(s(1), &Outcome::Closed { up: s(300), code: Some(REPLACED) }), Some((s(60), s(1))));
        assert_eq!(backoff(s(1), &Outcome::Closed { up: s(300), code: Some(REVOKED) }), None);
        assert_eq!(backoff(s(1), &Outcome::Unauthorized), None);
        assert_eq!(jitter(s(0), 0.7), s(0));
        assert_eq!((jitter(s(8), 0.0), jitter(s(8), 1.0)), (s(6), s(10)));
    }

    #[test]
    fn presence_is_the_device_list_and_prunes_subscribers() {
        let mut inner = Inner::default();
        inner.subscribers.insert("a".into(), super::super::Subscriber::default());
        inner.subscribers.insert("b".into(), super::super::Subscriber::default());
        apply_presence(&mut inner, &[json!({"id": "a", "name": "Mac", "platform": "macos", "publicKey": "k", "host": false, "online": true, "lastSeen": 5}), json!({"id": "c", "online": false, "host": true}), json!({"id": "bad id"})]);
        assert!(inner.presence);
        assert_eq!(inner.devices.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        assert_eq!((inner.devices[0].name.as_str(), inner.devices[0].online, inner.devices[0].host), ("Mac", true, false));
        assert_eq!(inner.subscribers.keys().collect::<Vec<_>>(), ["a"]);
        apply_presence(&mut inner, &[]);
        assert!(inner.devices.is_empty() && inner.subscribers.is_empty());
    }

    #[test]
    fn going_offline_or_coming_back_forgets_cached_lists() {
        let mut inner = Inner::default();
        let row = |id: &str, online: bool| json!({"id": id, "online": online, "host": true});
        apply_presence(&mut inner, &[row("a", true), row("b", true)]);
        let now = std::time::Instant::now();
        for id in ["a", "b"] {
            inner.catalog.put((id.into(), None), now, json!([]));
        }
        apply_presence(&mut inner, &[row("a", false), row("b", true)]);
        assert!(!inner.catalog.knows("a") && inner.catalog.knows("b"));
        inner.catalog.put(("a".into(), None), now, json!([]));
        apply_presence(&mut inner, &[row("a", true), row("b", true)]);
        assert!(!inner.catalog.knows("a") && inner.catalog.knows("b"));
    }

    #[test]
    fn trust_follows_the_pinned_key() {
        let (host, old, new) = (crypto::generate(), crypto::generate(), crypto::generate());
        let mut settings = config::Settings::default();
        settings.trusted.insert("viewer".into(), crypto::public_key(&old));
        assert!(settings.is_trusted("viewer", &crypto::public_key(&old)));
        // The relay (or the device) reports a new key: untrusted until trusted again.
        assert!(!settings.is_trusted("viewer", &crypto::public_key(&new)));
        assert!(!settings.is_trusted("other", &crypto::public_key(&old)));
        // A frame sealed by the new key never opens with the pinned pair key.
        let pinned = crypto::pair_key(&host, "host", &crypto::public_key(&old), "viewer").unwrap();
        let swapped = crypto::pair_key(&new, "viewer", &crypto::public_key(&host), "host").unwrap();
        let body = crypto::seal(&swapped, "viewer", "host", b"{}").unwrap();
        assert!(crypto::open(&pinned, "viewer", "host", &body).is_err());
        let view = Device { id: "viewer".into(), name: String::new(), platform: String::new(), public_key: crypto::public_key(&new), host: false, online: true, last_seen: Value::Null }.view("host", &settings);
        assert_eq!(view["trusted"], false);
    }
}
