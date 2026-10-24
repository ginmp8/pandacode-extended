//! Offline API cost estimates from a bundled or previously saved price list.
//! This hardened build never refreshes pricing from the network.

use super::parse::Record;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

const BUNDLED: &str = include_str!("prices.json");

/// Dollars per token. `high` rates apply once a prompt exceeds `threshold` tokens.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Price {
    input: f64,
    output: f64,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
    cache_write_1h: Option<f64>,
    threshold: Option<u64>,
    input_high: Option<f64>,
    output_high: Option<f64>,
    cache_read_high: Option<f64>,
    cache_write_high: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Source {
    Saved,
    Bundled,
}

#[derive(Clone, Debug)]
pub(crate) struct PriceBook {
    prices: HashMap<String, Price>,
    pub source: Source,
    pub fetched_at: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPrices {
    fetched_at: i64,
    prices: HashMap<String, Price>,
}

fn rate(entry: &Value, key: &str) -> Option<f64> {
    entry[key].as_f64().filter(|value| value.is_finite() && *value >= 0.0)
}

/// The long-context tier LiteLLM describes as `…_above_200k_tokens` or `…_above_272k_tokens`.
fn tier(entry: &Value) -> Option<(u64, String)> {
    let object = entry.as_object()?;
    object.keys().find_map(|key| {
        let rest = key.strip_prefix("input_cost_per_token_above_")?;
        let thousands = rest.strip_suffix("k_tokens")?.parse::<u64>().ok()?;
        Some((thousands * 1000, format!("_above_{thousands}k_tokens")))
    })
}

fn price(entry: &Value) -> Option<Price> {
    let high = tier(entry);
    let tiered = |key: &str| high.as_ref().and_then(|(_, suffix)| rate(entry, &format!("{key}{suffix}")));
    Some(Price {
        input: rate(entry, "input_cost_per_token")?,
        output: rate(entry, "output_cost_per_token")?,
        cache_read: rate(entry, "cache_read_input_token_cost"),
        cache_write: rate(entry, "cache_creation_input_token_cost"),
        cache_write_1h: rate(entry, "cache_creation_input_token_cost_above_1hr"),
        threshold: high.as_ref().map(|(threshold, _)| *threshold),
        input_high: tiered("input_cost_per_token"),
        output_high: tiered("output_cost_per_token"),
        cache_read_high: tiered("cache_read_input_token_cost"),
        cache_write_high: tiered("cache_creation_input_token_cost"),
    })
}

/// Collapses LiteLLM's provider-prefixed keys to one price per model name,
/// preferring the vendor's own listing over resellers (`azure/…`, `vertex_ai/…`).
pub(crate) fn parse_prices(document: &Value) -> HashMap<String, Price> {
    let Some(entries) = document.as_object() else { return HashMap::new() };
    let rank = |key: &str| match key.split_once('/') {
        None => 0,
        Some(("openai" | "anthropic", _)) => 1,
        Some(_) => 2,
    };
    let mut keys: Vec<&String> = entries.keys().collect();
    keys.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.cmp(b)));
    let mut prices = HashMap::new();
    for key in keys {
        let entry = &entries[key];
        if !matches!(entry["mode"].as_str(), None | Some("chat" | "responses")) {
            continue;
        }
        let name = key.rsplit('/').next().unwrap_or(key).to_ascii_lowercase();
        if prices.contains_key(&name) {
            continue;
        }
        if let Some(price) = price(entry) {
            prices.insert(name, price);
        }
    }
    prices
}

impl PriceBook {
    #[cfg(test)]
    pub(crate) fn from_prices(prices: HashMap<String, Price>) -> Self {
        Self { prices, source: Source::Bundled, fetched_at: None }
    }

    pub(crate) fn bundled() -> Self {
        let document = serde_json::from_str(BUNDLED).unwrap_or(Value::Null);
        Self { prices: parse_prices(&document), source: Source::Bundled, fetched_at: None }
    }

    /// Finds a model by exact name, then without a `[1m]`-style suffix or a date stamp.
    pub(crate) fn find(&self, model: &str) -> Option<&Price> {
        let name = model.rsplit('/').next().unwrap_or(model);
        let name = name.split('[').next().unwrap_or(name).trim();
        self.prices.get(name).or_else(|| {
            let (stem, date) = name.rsplit_once('-')?;
            (date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit())).then(|| self.prices.get(stem)).flatten()
        })
    }

    /// Cost and cache savings in dollars, or `None` when the model is not listed.
    pub(crate) fn cost(&self, record: &Record) -> Option<(f64, f64)> {
        let price = self.find(&record.model)?;
        let high = price.threshold.is_some_and(|threshold| match record.class {
            0 => false,
            1 => threshold < 272_000,
            _ => true,
        });
        let pick = |base: f64, tiered: Option<f64>| if high { tiered.unwrap_or(base) } else { base };
        let input = pick(price.input, price.input_high);
        let cache_read = pick(price.cache_read.unwrap_or(price.input), price.cache_read_high);
        let cache_write = pick(price.cache_write.unwrap_or(price.input), price.cache_write_high);
        let cache_write_1h = price.cache_write_1h.unwrap_or(cache_write);
        let output = pick(price.output, price.output_high);
        let cost = record.input as f64 * input
            + record.cache_read as f64 * cache_read
            + record.cache_write as f64 * cache_write
            + record.cache_write_1h as f64 * cache_write_1h
            + record.output as f64 * output;
        Some((cost, record.cache_read as f64 * (input - cache_read).max(0.0)))
    }
}

pub(crate) fn load_saved(path: &Path) -> Option<PriceBook> {
    let saved: SavedPrices = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (!saved.prices.is_empty()).then_some(PriceBook { prices: saved.prices, source: Source::Saved, fetched_at: Some(saved.fetched_at) })
}

/// A saved local snapshot or the bundled one; no background network activity.
pub(crate) async fn load(path: &Path, _now_ms: i64) -> PriceBook {
    load_saved(path).unwrap_or_else(PriceBook::bundled)
}

#[cfg(test)]
mod offline_tests {
    use super::*;

    #[tokio::test]
    async fn missing_or_stale_prices_never_trigger_a_network_refresh() {
        let path = std::env::temp_dir().join(format!("pandacode-prices-{}.json", uuid::Uuid::new_v4()));
        assert_eq!(load(&path, i64::MAX).await.source, Source::Bundled);
        let saved = SavedPrices { fetched_at: 1, prices: PriceBook::bundled().prices };
        let original = serde_json::to_vec(&saved).unwrap();
        std::fs::write(&path, &original).unwrap();
        let loaded = load(&path, i64::MAX).await;
        assert_eq!(loaded.source, Source::Saved);
        assert_eq!(loaded.fetched_at, Some(1));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_file(path).unwrap();
    }
}
