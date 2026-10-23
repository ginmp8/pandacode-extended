//! WhatsApp through Meta's Cloud API. Meta delivers messages to a webhook, so the
//! local server's `/whatsapp/<channel id>` route must be reachable over HTTPS, for
//! example through `cloudflared tunnel --url http://127.0.0.1:47321` or Tailscale
//! Funnel. Each call is checked against the app secret's signature; the verify token
//! answers Meta's subscription check. Replies use the phone number ID and an access token.

use super::http::{same, Request, Response};
use super::{client, handle_inbound, secrets, Channel};
use crate::{AppState, Result};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use tauri::Manager;

const GRAPH: &str = "https://graph.facebook.com/v21.0";
const MAX_AGE_MS: i64 = 24 * 60 * 60 * 1000;
const CLOCK_SKEW_MS: i64 = 5 * 60 * 1000;
// bot_activity retains 5000 rows. Stop admitting messages before replay
// receipts within the accepted time window could be evicted by that bound.
const MAX_RECENT_ACTIVITY: i64 = 4000;

/// Checks `X-Hub-Signature-256: sha256=<hex>` against the app secret.
pub(crate) fn signature_valid(secret: &str, body: &[u8], header: &str) -> bool {
    let Some(given) = header.strip_prefix("sha256=") else { return false };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else { return false };
    mac.update(body);
    let expected: String = mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    same(&expected, &given.to_ascii_lowercase())
}

#[derive(Debug, PartialEq)]
pub(crate) struct WebhookMessage {
    pub id: String,
    pub timestamp_ms: i64,
    pub from: String,
    pub name: String,
    pub text: String,
}

/// Text messages belonging to this phone-number channel. The signature belongs
/// to a Meta app and may cover several numbers, so it alone is not enough.
pub(crate) fn messages(payload: &Value, phone_id: &str) -> Vec<WebhookMessage> {
    let mut found = Vec::new();
    if payload["object"] != "whatsapp_business_account" {
        return found;
    }
    for change in payload["entry"].as_array().into_iter().flatten().flat_map(|entry| entry["changes"].as_array().into_iter().flatten()) {
        let value = &change["value"];
        if phone_id.is_empty() || value["metadata"]["phone_number_id"].as_str() != Some(phone_id) {
            continue;
        }
        let names: Vec<(&str, &str)> = value["contacts"].as_array().into_iter().flatten().filter_map(|c| Some((c["wa_id"].as_str()?, c["profile"]["name"].as_str()?))).collect();
        for message in value["messages"].as_array().into_iter().flatten() {
            let Some(id) = message["id"].as_str().filter(|id| !id.is_empty() && id.len() <= 256 && id.bytes().all(|byte| byte.is_ascii_graphic())) else { continue };
            let Some(timestamp_ms) = message["timestamp"].as_str().and_then(|value| value.parse::<i64>().ok()).and_then(|seconds| seconds.checked_mul(1000)) else { continue };
            let Some(from) = message["from"].as_str() else { continue };
            let text = match message["type"].as_str() {
                Some("text") => message["text"]["body"].as_str(),
                Some("button") => message["button"]["text"].as_str(),
                Some("interactive") => message["interactive"]["button_reply"]["title"].as_str(),
                _ => None,
            };
            let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else { continue };
            let name = names.iter().find(|(id, _)| *id == from).map_or(from, |(_, name)| name);
            found.push(WebhookMessage { id: id.to_owned(), timestamp_ms, from: from.to_owned(), name: name.to_owned(), text: text.to_owned() });
        }
    }
    found
}

/// Admit at most once before spawning any agent work. The store mutex held by
/// the caller serializes admission, and the receipt survives application restart.
fn admit(store: &crate::storage::Store, channel: &Channel, message: &WebhookMessage, now_ms: i64) -> Result<bool> {
    let oldest = now_ms.saturating_sub(MAX_AGE_MS);
    if message.timestamp_ms < oldest || message.timestamp_ms > now_ms.saturating_add(CLOCK_SKEW_MS) {
        return Ok(false);
    }
    // Reset intentionally erases the activity trail. Its fresh reset marker
    // must invalidate earlier signed commands whose receipts were erased.
    let last_reset: Option<i64> = store.db.query_row("SELECT MAX(at) FROM bot_activity WHERE bot_id=?1 AND kind='reset'", [&channel.bot_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    if last_reset.is_some_and(|at| message.timestamp_ms <= at) {
        return Ok(false);
    }
    let exists: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM bot_activity WHERE bot_id=?1 AND kind='webhook-received' AND json_extract(detail,'$.channelId')=?2 AND json_extract(detail,'$.messageId')=?3)",
        rusqlite::params![channel.bot_id, channel.id, message.id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if exists {
        return Ok(false);
    }
    let retention_start = oldest.saturating_sub(CLOCK_SKEW_MS);
    let recent: i64 = store.db.query_row("SELECT COUNT(*) FROM bot_activity WHERE bot_id=?1 AND at>=?2", rusqlite::params![channel.bot_id, retention_start], |row| row.get(0)).map_err(|error| error.to_string())?;
    if recent >= MAX_RECENT_ACTIVITY {
        return Err("Webhook replay protection is at capacity. Retry later.".into());
    }
    crate::bots::log(store, &channel.bot_id, "webhook-received", "Accepted an authenticated WhatsApp message", json!({"channelId": channel.id, "messageId": message.id}))?;
    Ok(true)
}

fn channel(app: &tauri::AppHandle, id: &str) -> Option<Channel> {
    let state = app.state::<AppState>();
    let store = state.store.lock().ok()?;
    super::get(&store, id).ok().filter(|c| c.kind == "whatsapp" && c.enabled)
}

pub(crate) async fn webhook(app: &tauri::AppHandle, channel_id: &str, request: &Request) -> Response {
    let Some(channel) = channel(app, channel_id) else { return Response::text(404, "Not found.") };
    match request.method.as_str() {
        "GET" => {
            let token = channel.config["verifyToken"].as_str().unwrap_or_default().to_owned();
            let given = request.query.get("hub.verify_token").map(String::as_str).unwrap_or_default();
            if request.query.get("hub.mode").map(String::as_str) == Some("subscribe") && !token.is_empty() && same(given, &token) {
                super::update_status(app, &channel.id, "Webhook verified");
                Response::text(200, request.query.get("hub.challenge").map(String::as_str).unwrap_or_default())
            } else {
                Response::text(403, "Verification failed.")
            }
        }
        "POST" => {
            let secret = secrets::get(&channel.id, "signing_secret").unwrap_or_default();
            if secret.is_empty() || !signature_valid(&secret, &request.body, request.header("x-hub-signature-256").unwrap_or_default()) {
                return Response::text(401, "Bad signature.");
            }
            let Ok(payload) = serde_json::from_slice::<Value>(&request.body) else { return Response::text(400, "Invalid webhook payload.") };
            for message in messages(&payload, channel.config["phoneNumberId"].as_str().unwrap_or_default()) {
                let accepted = {
                    let state = app.state::<AppState>();
                    let Ok(store) = state.store.lock() else { return Response::text(503, "Webhook receipt storage is unavailable.") };
                    admit(&store, &channel, &message, crate::storage::now())
                };
                match accepted {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(_) => return Response::text(503, "Webhook replay protection is unavailable. Retry later."),
                }
                let (app, channel_id) = (app.clone(), channel.id.clone());
                tauri::async_runtime::spawn(async move { handle_inbound(&app, &channel_id, &message.from, &message.name, &message.from, &message.text).await });
            }
            super::update_status(app, &channel.id, "Receiving messages");
            Response::text(200, "OK")
        }
        _ => Response::text(405, "Not supported."),
    }
}

/// Checks the access token against the phone number ID and returns the business number.
pub(crate) async fn verify(phone_id: &str, token: &str) -> Result<super::Identity> {
    if phone_id.is_empty() || !phone_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("Enter the WhatsApp phone number ID (digits only) from Meta's API Setup page.".into());
    }
    let response = client()?.get(format!("{GRAPH}/{phone_id}?fields=display_phone_number,verified_name")).bearer_auth(token).send().await.map_err(|e| e.to_string())?;
    let ok = response.status().is_success();
    let value: Value = response.json().await.unwrap_or(Value::Null);
    if !ok {
        return Err(match value["error"]["code"].as_i64() {
            Some(190) => "Meta rejected the access token. Create a new one (temporary tokens expire after 24 hours; a system user token does not).".into(),
            Some(100) => "Meta does not know that phone number ID for this token. Copy it again from WhatsApp → API Setup.".into(),
            _ => format!("WhatsApp: {}", value["error"]["message"].as_str().unwrap_or("request failed")),
        });
    }
    let number = value["display_phone_number"].as_str().unwrap_or_default();
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    let name = value["verified_name"].as_str().map_or_else(|| number.to_owned(), |name| format!("{name} ({number})"));
    Ok(super::Identity { name, link: (!digits.is_empty()).then(|| format!("https://wa.me/{digits}")), ..Default::default() })
}

pub(crate) async fn send(channel: &Channel, thread: &str, text: &str) -> Result<()> {
    let token = secrets::get(&channel.id, "access_token").ok_or("The WhatsApp access token is missing.")?;
    let phone_id = channel.config["phoneNumberId"].as_str().filter(|id| id.chars().all(|c| c.is_ascii_digit())).ok_or("The WhatsApp phone number ID is missing.")?;
    if thread.is_empty() || !thread.chars().all(|c| c.is_ascii_digit()) {
        return Err("That WhatsApp number looks wrong.".into());
    }
    let body = json!({"messaging_product": "whatsapp", "recipient_type": "individual", "to": thread, "type": "text", "text": {"preview_url": false, "body": text}});
    let response = client()?.post(format!("{GRAPH}/{phone_id}/messages")).bearer_auth(token).json(&body).send().await.map_err(|e| e.to_string())?;
    if response.status().is_success() {
        return Ok(());
    }
    let value: Value = response.json().await.unwrap_or(Value::Null);
    // Outside the 24-hour customer service window WhatsApp only accepts templates.
    Err(match value["error"]["code"].as_i64() {
        Some(131047) => "WhatsApp only allows replies within 24 hours of the person's last message.".into(),
        Some(190) => "WhatsApp rejected the access token. Create a new one in Meta's app dashboard.".into(),
        _ => format!("WhatsApp: {}", value["error"]["message"].as_str().unwrap_or("request failed")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        store: crate::storage::Store,
        channel: Channel,
        root: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("pandacode-webhook-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root).unwrap();
            let store = crate::storage::Store::open(&root.join("state.sqlite")).unwrap();
            let bot = crate::bots::create(&store, &crate::bots::BotInput {
                name: "Webhook test".into(), provider: "claude".into(), model: "default".into(), mode: "ask".into(), ..Default::default()
            }, &root.join("bots")).unwrap();
            let channel = Channel { id: "channel-test".into(), bot_id: bot.id, kind: "whatsapp".into(), label: "Test".into(), config: json!({"phoneNumberId": "123"}), enabled: true, status: String::new(), secrets: Vec::new() };
            Self { store, channel, root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // Close SQLite before removing its files on Windows.
            self.store = crate::storage::Store::open(std::path::Path::new(":memory:")).unwrap();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn message(id: &str, at: i64) -> WebhookMessage {
        WebhookMessage { id: id.into(), timestamp_ms: at, from: "1234".into(), name: "Owner".into(), text: "Run the task".into() }
    }

    #[test]
    fn signed_webhook_replays_are_rejected_after_database_reopen() {
        let mut fixture = Fixture::new();
        let now = crate::storage::now();
        let first = message("message-1", now);
        assert!(admit(&fixture.store, &fixture.channel, &first, now).unwrap());
        assert!(!admit(&fixture.store, &fixture.channel, &first, now).unwrap());
        fixture.store = crate::storage::Store::open(&fixture.root.join("state.sqlite")).unwrap();
        assert!(!admit(&fixture.store, &fixture.channel, &first, now + 1000).unwrap());
        assert!(admit(&fixture.store, &fixture.channel, &message("message-2", now), now).unwrap());
    }

    #[test]
    fn webhook_age_and_reset_prevent_old_commands_being_readmitted() {
        let fixture = Fixture::new();
        let now = crate::storage::now();
        assert!(!admit(&fixture.store, &fixture.channel, &message("old", now - MAX_AGE_MS - 1), now).unwrap());
        assert!(!admit(&fixture.store, &fixture.channel, &message("future", now + CLOCK_SKEW_MS + 1), now).unwrap());
        assert!(admit(&fixture.store, &fixture.channel, &message("accepted", now - 1000), now).unwrap());
        crate::bots::reset(&fixture.store, &fixture.channel.bot_id).unwrap();
        assert!(!admit(&fixture.store, &fixture.channel, &message("accepted", now - 1000), now).unwrap());
    }

    #[test]
    fn webhook_admission_fails_closed_before_receipts_can_be_pruned() {
        let fixture = Fixture::new();
        let now = crate::storage::now();
        fixture.store.db.execute(
            "WITH RECURSIVE rows(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM rows WHERE n<4000) INSERT INTO bot_activity(bot_id,kind,summary,at) SELECT ?1,'event','test',?2 FROM rows",
            rusqlite::params![fixture.channel.bot_id, now],
        ).unwrap();
        assert!(admit(&fixture.store, &fixture.channel, &message("new", now), now).is_err());
    }

    #[test]
    fn webhook_payloads_require_identity_time_and_the_correct_phone_number() {
        let mut payload = json!({"object": "whatsapp_business_account", "entry": [{"changes": [{"value": {
            "metadata": {"phone_number_id": "123"}, "messages": [{"id": "m1", "timestamp": "1000", "from": "1234", "type": "text", "text": {"body": "Hello"}}]
        }}]}]});
        assert_eq!(messages(&payload, "123").len(), 1);
        assert!(messages(&payload, "other").is_empty());
        for invalid in [Value::Null, json!(""), json!("a b")] {
            payload["entry"][0]["changes"][0]["value"]["messages"][0]["id"] = invalid;
            assert!(messages(&payload, "123").is_empty());
        }
        payload["entry"][0]["changes"][0]["value"]["messages"][0]["id"] = json!("m1");
        payload["entry"][0]["changes"][0]["value"]["messages"][0]["timestamp"] = json!(i64::MAX.to_string());
        assert!(messages(&payload, "123").is_empty());
    }
}
