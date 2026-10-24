//! A closed operation vocabulary; remote content never supplies JavaScript.
use serde_json::Value;
const OPERATIONS: &[&str] = &[
    "snapshot", "audit", "resources", "observe_start", "observe_read", "observe_stop", "clear",
    "expect_text", "expect_title", "expect_url", "expect_element", "expect_resource",
    "click", "fill", "select", "check", "focus", "scroll",
];

pub(super) fn script(input: &str) -> Result<String, String> {
    if input.len() > 8192 { return Err("Browser review input exceeds 8 KiB.".into()); }
    let value: Value = serde_json::from_str(input).map_err(|_| "Invalid browser review request.")?;
    let fields = value.as_object().ok_or("Browser review requires an object.")?;
    if fields.keys().any(|k| !["op", "origin", "selector", "value", "token", "ref", "status"].contains(&k.as_str())) {
        return Err("Unknown browser review field.".into());
    }
    let op = value["op"].as_str().ok_or("Browser review operation is required.")?;
    if !OPERATIONS.contains(&op) { return Err("Unknown browser review operation.".into()); }
    let origin = value["origin"].as_str().ok_or("Browser origin is required.")?;
    let parsed = tauri::Url::parse(origin).map_err(|_| "Invalid browser origin.")?;
    if !super::allowed_url(&parsed) || parsed.origin().ascii_serialization() != origin {
        return Err("Use an exact HTTP(S) origin without credentials, path, query or fragment.".into());
    }
    for field in ["selector", "value", "token", "ref"] {
        if let Some(raw) = value.get(field) {
            let text = raw.as_str().ok_or("Invalid browser review string.")?;
            if text.len() > 2048 || text.contains('\0') { return Err("Browser review field is too large or invalid.".into()); }
        }
    }
    if let Some(status) = value.get("status") {
        if !status.as_u64().is_some_and(|n| (100..=599).contains(&n)) { return Err("Expected HTTP status must be 100 to 599.".into()); }
    }
    if ["click", "fill", "select", "check", "focus"].contains(&op) {
        for field in ["token", "ref"] {
            if value[field].as_str().is_none_or(str::is_empty) { return Err("Take a browser snapshot before acting.".into()); }
        }
    }
    if op == "expect_resource" && value.get("status").is_none() { return Err("Resource checks require a known HTTP status.".into()); }
    Ok(include_str!("review.js").replace("__PANDACODE_REVIEW_ARGS__", &value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_operations_and_privileged_schemes() {
        for input in [r#"{"op":"evaluate","origin":"http://localhost:3000"}"#, r#"{"op":"snapshot","origin":"file:///etc/passwd"}"#, r#"{"op":"snapshot","origin":"http://user:pass@localhost"}"#, r#"{"op":"snapshot","origin":"http://localhost:3000/path"}"#, r#"{"op":"snapshot","origin":"null"}"#] { assert!(script(input).is_err()); }
    }
    #[test]
    fn rejects_extra_fields_and_unbounded_values() {
        assert!(script(r#"{"op":"snapshot","origin":"http://localhost:3000","code":"x"}"#).is_err());
        assert!(script(&serde_json::json!({"op":"fill","origin":"http://localhost:3000","value":"x".repeat(2049),"token":"a","ref":"e1"}).to_string()).is_err());
        assert!(script(r#"{"op":"expect_resource","origin":"http://localhost:3000","value":"/api","status":0}"#).is_err());
    }
    #[test]
    fn mutations_require_snapshot_refs_and_embed_only_json_data() {
        assert!(script(r#"{"op":"click","origin":"http://localhost:3000"}"#).is_err());
        let safe = script(r#"{"op":"snapshot","origin":"http://localhost:3000"}"#).unwrap();
        assert!(!safe.contains("__PANDACODE_REVIEW_ARGS__"));
        assert!(safe.contains("\"op\":\"snapshot\""));
    }
}
