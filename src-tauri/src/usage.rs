use chrono::{Datelike, Local, TimeZone};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Duration;

use crate::store::Store;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaWindow {
    pub window: String,
    pub percent: f64,
    #[serde(rename = "resetsAt")]
    pub resets_at: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyQuota {
    pub windows: Option<Vec<QuotaWindow>>,
    pub error: Option<String>,
}

pub fn parse_usage(json: &serde_json::Value) -> Option<Vec<QuotaWindow>> {
    let usage = json.get("usage")?;
    if !usage.is_object() {
        return None;
    }
    let mut windows = Vec::new();
    for window in ["rolling", "weekly", "monthly"] {
        let Some(w) = usage.get(window) else {
            continue;
        };
        let Some(percent) = w.get("percent").and_then(|v| v.as_f64()) else {
            continue;
        };
        windows.push(QuotaWindow {
            window: window.to_string(),
            percent,
            resets_at: w
                .get("resetsAt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            status: w
                .get("status")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        });
    }
    if windows.is_empty() {
        None
    } else {
        Some(windows)
    }
}

pub async fn fetch_provider_usage(base_url: &str, api_key: &str) -> KeyQuota {
    let url = if base_url.ends_with('/') {
        format!("{base_url}usage")
    } else {
        format!("{base_url}/usage")
    };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(10000))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return KeyQuota {
                windows: None,
                error: Some(e.to_string()),
            }
        }
    };
    let response = match client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return KeyQuota {
                windows: None,
                error: Some(if e.is_timeout() {
                    "timeout".to_string()
                } else {
                    e.to_string()
                }),
            }
        }
    };
    if !response.status().is_success() {
        return KeyQuota {
            windows: None,
            error: Some(format!("HTTP {}", response.status().as_u16())),
        };
    }
    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            return KeyQuota {
                windows: None,
                error: Some(e.to_string()),
            }
        }
    };
    match parse_usage(&json) {
        Some(windows) => KeyQuota {
            windows: Some(windows),
            error: None,
        },
        None => KeyQuota {
            windows: None,
            error: Some("unexpected usage payload".to_string()),
        },
    }
}

pub const USAGE_SQL: &str = "SELECT json_extract(data,'$.providerID') AS provider, json_extract(data,'$.modelID') AS model, json_extract(data,'$.cost') AS cost, json_extract(data,'$.time.created') AS t, json_extract(data,'$.tokens.input') AS tok_in, json_extract(data,'$.tokens.output') AS tok_out, json_extract(data,'$.tokens.reasoning') AS tok_reasoning, json_extract(data,'$.tokens.cache.read') AS tok_cache_read, json_extract(data,'$.tokens.cache.write') AS tok_cache_write FROM message WHERE json_extract(data,'$.role')='assistant'";

#[derive(Debug, Clone)]
pub struct UsageEntry {
    pub provider: String,
    pub model: String,
    pub cost: f64,
    pub t: i64,
    /// Billed tokens (input + output + reasoning + cache read/write) when the
    /// provider recorded them; 0 when the message has no token data.
    pub tokens: f64,
}

fn value_to_f64(v: rusqlite::types::Value) -> f64 {
    match v {
        rusqlite::types::Value::Integer(i) => i as f64,
        rusqlite::types::Value::Real(f) => f,
        rusqlite::types::Value::Text(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn value_to_i64(v: rusqlite::types::Value) -> i64 {
    match v {
        rusqlite::types::Value::Integer(i) => i,
        rusqlite::types::Value::Real(f) => f as i64,
        rusqlite::types::Value::Text(s) => s.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

pub fn read_usage(db_path: &Path) -> (Vec<UsageEntry>, Option<String>) {
    let conn = match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(e) => return (Vec::new(), Some(e.to_string())),
    };
    let mut stmt = match conn.prepare(USAGE_SQL) {
        Ok(s) => s,
        Err(e) => return (Vec::new(), Some(e.to_string())),
    };
    let rows = match stmt.query_map([], |row| {
        let provider: Option<String> = row.get(0)?;
        let model: Option<String> = row.get(1)?;
        let cost: rusqlite::types::Value = row.get(2)?;
        let t: rusqlite::types::Value = row.get(3)?;
        let tokens: Vec<rusqlite::types::Value> = (4..9)
            .map(|i| row.get::<_, rusqlite::types::Value>(i))
            .collect::<Result<_, _>>()?;
        Ok((provider, model, cost, t, tokens))
    }) {
        Ok(r) => r,
        Err(e) => return (Vec::new(), Some(e.to_string())),
    };
    let mut entries = Vec::new();
    for row in rows {
        let (provider, model, cost, t, tokens) = match row {
            Ok(r) => r,
            Err(e) => return (Vec::new(), Some(e.to_string())),
        };
        let (Some(provider), Some(model)) = (provider, model) else {
            continue;
        };
        if provider.is_empty() || model.is_empty() {
            continue;
        }
        // rusqlite maps JSON null to Null; missing columns read as Null too.
        entries.push(UsageEntry {
            provider,
            model,
            cost: value_to_f64(cost),
            t: value_to_i64(t),
            tokens: tokens.into_iter().map(value_to_f64).sum(),
        });
    }
    (entries, None)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bucket {
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub models: BTreeMap<String, f64>,
    /// Summed token counts for the bucket, when the provider recorded them.
    #[serde(default)]
    pub tokens: f64,
}

/// Switch log folded into a per-provider timeline so any timestamp can be
/// attributed to the key that was active then (used by usage and history).
pub struct KeyTimeline {
    switches: HashMap<String, Vec<(i64, String, Option<String>)>>,
}

impl KeyTimeline {
    pub fn new(store: &Store) -> Self {
        let mut switches: HashMap<String, Vec<(i64, String, Option<String>)>> = HashMap::new();
        for e in &store.log {
            // Mirrors the JS truthiness checks: action==='switch' with real provider/key/t.
            if e.action == "switch" && !e.provider.is_empty() && !e.key.is_empty() && e.t != 0 {
                switches.entry(e.provider.clone()).or_default().push((
                    e.t,
                    e.key.clone(),
                    e.prev.clone(),
                ));
            }
        }
        for list in switches.values_mut() {
            list.sort_by_key(|e| e.0);
        }
        KeyTimeline { switches }
    }

    pub fn key_at(&self, provider: &str, t: i64) -> Option<String> {
        let list = self.switches.get(provider)?;
        if list.is_empty() {
            return None;
        }
        let mut key = list[0].2.clone();
        for (switched_at, name, _) in list {
            if *switched_at <= t {
                key = Some(name.clone());
            }
        }
        key
    }
}

fn add_to_bucket(bucket: &mut Bucket, row: &UsageEntry) {
    bucket.total += row.cost;
    bucket.tokens += row.tokens;
    *bucket.models.entry(row.model.clone()).or_insert(0.0) += row.cost;
}

pub fn attribute_usage(
    entries: &[UsageEntry],
    store: &Store,
) -> (
    BTreeMap<String, BTreeMap<String, Bucket>>,
    BTreeMap<String, Bucket>,
) {
    let timeline = KeyTimeline::new(store);
    let mut by_key: BTreeMap<String, BTreeMap<String, Bucket>> = BTreeMap::new();
    let mut untracked: BTreeMap<String, Bucket> = BTreeMap::new();
    for row in entries {
        let stored_keys: Vec<&String> = store
            .providers
            .get(&row.provider)
            .map(|p| p.keys.keys().collect())
            .unwrap_or_default();
        let mut key = timeline.key_at(&row.provider, row.t);
        if key.is_none() && stored_keys.len() == 1 {
            key = Some(stored_keys[0].clone());
        }
        match key {
            None => add_to_bucket(untracked.entry(row.provider.clone()).or_default(), row),
            Some(k) => add_to_bucket(
                by_key
                    .entry(row.provider.clone())
                    .or_default()
                    .entry(k)
                    .or_default(),
                row,
            ),
        }
    }
    (by_key, untracked)
}

/// Local-time [start, end) of the given calendar month, as epoch millis.
pub fn month_range(year: i32, month: u32) -> (i64, i64) {
    let start = Local
        .with_ymd_and_hms(year, month + 1, 1, 0, 0, 0)
        .single()
        .map(|d| d.timestamp_millis())
        .unwrap_or(0);
    let (ny, nm) = if month == 11 {
        (year + 1, 1)
    } else {
        (year, month + 2)
    };
    let end = Local
        .with_ymd_and_hms(ny, nm, 1, 0, 0, 0)
        .single()
        .map(|d| d.timestamp_millis())
        .unwrap_or(i64::MAX);
    (start, end)
}

pub fn current_month() -> (i32, u32) {
    let now = Local::now().date_naive();
    (now.year(), now.month0())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{LogEntry, ProviderEntry, Settings, Store};
    use std::collections::BTreeMap;

    fn test_store() -> Store {
        let mut providers = BTreeMap::new();
        providers.insert(
            "opencode-go".to_string(),
            ProviderEntry {
                keys: [
                    (
                        "personal".to_string(),
                        crate::store::Cred {
                            cred_type: "api".to_string(),
                            key: "a".to_string(),
                            account: None,
                        },
                    ),
                    (
                        "work".to_string(),
                        crate::store::Cred {
                            cred_type: "api".to_string(),
                            key: "b".to_string(),
                            account: None,
                        },
                    ),
                ]
                .into_iter()
                .collect(),
                active: Some("work".to_string()),
            },
        );
        providers.insert(
            "other".to_string(),
            ProviderEntry {
                keys: [(
                    "x".to_string(),
                    crate::store::Cred {
                        cred_type: "api".to_string(),
                        key: "c".to_string(),
                        account: None,
                    },
                )]
                .into_iter()
                .collect(),
                active: Some("x".to_string()),
            },
        );
        Store {
            providers,
            log: vec![
                LogEntry {
                    t: 100,
                    provider: "opencode-go".to_string(),
                    key: "personal".to_string(),
                    action: "switch".to_string(),
                    prev: None,
                },
                LogEntry {
                    t: 300,
                    provider: "opencode-go".to_string(),
                    key: "work".to_string(),
                    action: "switch".to_string(),
                    prev: None,
                },
                LogEntry {
                    t: 200,
                    provider: "other".to_string(),
                    key: "x".to_string(),
                    action: "switch".to_string(),
                    prev: None,
                },
            ],
            settings: Settings::default(),
        }
    }

    fn entry(provider: &str, model: &str, cost: f64, t: i64) -> UsageEntry {
        UsageEntry {
            provider: provider.to_string(),
            model: model.to_string(),
            cost,
            t,
            tokens: 0.0,
        }
    }

    #[test]
    fn attributes_each_message_to_key_active_at_message_time() {
        let entries = vec![
            entry("opencode-go", "glm-5.1", 2.0, 150),
            entry("opencode-go", "glm-5.1", 1.0, 250),
            entry("opencode-go", "kimi-k2.6", 0.5, 350),
        ];
        let (by_key, untracked) = attribute_usage(&entries, &test_store());
        assert!(untracked.is_empty());
        let personal = &by_key["opencode-go"]["personal"];
        assert!((personal.total - 3.0).abs() < 1e-9);
        assert!((personal.models["glm-5.1"] - 3.0).abs() < 1e-9);
        let work = &by_key["opencode-go"]["work"];
        assert!((work.total - 0.5).abs() < 1e-9);
    }

    #[test]
    fn uses_logged_prev_for_history_before_first_switch() {
        let mut store = test_store();
        store.log = vec![LogEntry {
            t: 300,
            provider: "opencode-go".to_string(),
            key: "work".to_string(),
            action: "switch".to_string(),
            prev: Some("personal".to_string()),
        }];
        let entries = vec![
            entry("opencode-go", "glm-5.1", 9.0, 50),
            entry("opencode-go", "glm-5.1", 1.0, 350),
        ];
        let (by_key, untracked) = attribute_usage(&entries, &store);
        assert!(untracked.is_empty());
        assert!((by_key["opencode-go"]["personal"].total - 9.0).abs() < 1e-9);
        assert!((by_key["opencode-go"]["work"].total - 1.0).abs() < 1e-9);
    }

    #[test]
    fn pre_log_history_goes_untracked_when_prev_unknown() {
        let entries = vec![
            entry("opencode-go", "glm-5.1", 9.0, 50),
            entry("opencode-go", "glm-5.1", 1.0, 150),
        ];
        let (by_key, untracked) = attribute_usage(&entries, &test_store());
        assert!((by_key["opencode-go"]["personal"].total - 1.0).abs() < 1e-9);
        assert!((untracked["opencode-go"].total - 9.0).abs() < 1e-9);
        assert!(!by_key["opencode-go"].contains_key("work"));
    }

    #[test]
    fn single_key_inherits_untracked_history() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "opencode-go".to_string(),
            ProviderEntry {
                keys: [(
                    "Xquad".to_string(),
                    crate::store::Cred {
                        cred_type: "api".to_string(),
                        key: "c".to_string(),
                        account: None,
                    },
                )]
                .into_iter()
                .collect(),
                active: Some("Xquad".to_string()),
            },
        );
        let store = Store {
            providers,
            log: vec![LogEntry {
                t: 100,
                provider: "opencode-go".to_string(),
                key: "Xquad".to_string(),
                action: "switch".to_string(),
                prev: None,
            }],
            settings: Settings::default(),
        };
        let entries = vec![entry("opencode-go", "glm-5.1", 26.86, 50)];
        let (by_key, untracked) = attribute_usage(&entries, &store);
        assert!((by_key["opencode-go"]["Xquad"].total - 26.86).abs() < 1e-9);
        assert!(untracked.is_empty());
    }

    #[test]
    fn tokens_accumulate_and_timeline_tracks_switches() {
        let store = test_store();
        let timeline = KeyTimeline::new(&store);
        assert_eq!(
            timeline.key_at("opencode-go", 150).as_deref(),
            Some("personal")
        );
        assert_eq!(timeline.key_at("opencode-go", 350).as_deref(), Some("work"));
        assert_eq!(timeline.key_at("opencode-go", 50), None);
        assert_eq!(timeline.key_at("nope", 350), None);

        let mut with_tokens = entry("opencode-go", "glm-5.1", 1.0, 150);
        with_tokens.tokens = 1_000.0;
        let (by_key, _) = attribute_usage(&[with_tokens], &store);
        assert!((by_key["opencode-go"]["personal"].tokens - 1_000.0).abs() < 1e-9);
    }

    #[test]
    fn parse_usage_extracts_windows() {
        let json: serde_json::Value = serde_json::json!({
            "usage": {
                "rolling": { "status": "ok", "percent": 98, "resetsAt": "2026-09-15T02:32:15.039Z" },
                "weekly": { "status": "ok", "percent": 39, "resetsAt": "2026-09-21T00:00:00.039Z" },
                "monthly": { "status": "ok", "percent": 21, "resetsAt": "2026-10-10T22:04:51.039Z" }
            }
        });
        let windows = parse_usage(&json).expect("windows");
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].window, "rolling");
        assert!((windows[0].percent - 98.0).abs() < 1e-9);
        assert_eq!(
            windows[0].resets_at.as_deref(),
            Some("2026-09-15T02:32:15.039Z")
        );
        assert!((windows[2].percent - 21.0).abs() < 1e-9);
    }

    #[test]
    fn parse_usage_keeps_exact_percent_and_drops_unknown() {
        let json: serde_json::Value = serde_json::json!({ "usage": { "rolling": { "percent": 98.4 }, "daily": { "percent": 5 } } });
        let windows = parse_usage(&json).expect("windows");
        assert_eq!(windows.len(), 1);
        assert!((windows[0].percent - 98.4).abs() < 1e-9);
        assert!(windows[0].resets_at.is_none());
        assert!(parse_usage(&serde_json::json!({})).is_none());
    }

    #[test]
    fn read_usage_from_real_db_file() {
        let dir = std::env::temp_dir().join(format!("kr-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("opencode.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE message (id text PRIMARY KEY, data text NOT NULL);",
            [],
        )
        .unwrap();
        let rows = [
            (
                "msg_1",
                r#"{"role":"assistant","providerID":"opencode-go","modelID":"glm-5.1","cost":1.25,"time":{"created":1700000000000},"tokens":{"input":1000,"output":250,"reasoning":0,"cache":{"read":500,"write":100}}}"#,
            ),
            (
                "msg_2",
                r#"{"role":"assistant","providerID":"opencode-go","modelID":"kimi-k2.6","cost":0.75,"time":{"created":1700000001000}}"#,
            ),
            (
                "msg_3",
                r#"{"role":"user","providerID":"opencode-go","modelID":"glm-5.1","cost":99,"time":{"created":1700000002000}}"#,
            ),
        ];
        for (id, data) in rows {
            conn.execute(
                "INSERT INTO message (id, data) VALUES (?1, ?2)",
                rusqlite::params![id, data],
            )
            .unwrap();
        }
        drop(conn);
        let (entries, error) = read_usage(&db);
        assert!(error.is_none());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].model, "glm-5.1");
        assert!((entries[0].cost - 1.25).abs() < 1e-9);
        assert_eq!(entries[0].t, 1700000000000);
        assert!((entries[0].tokens - 1850.0).abs() < 1e-9);
        assert!((entries[1].tokens - 0.0).abs() < 1e-9);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn month_range_scopes_calendar_month() {
        let (start, end) = month_range(2026, 8);
        // September 2026 local bounds contain Sep 15 and exclude Oct 1.
        let mid = Local
            .with_ymd_and_hms(2026, 9, 15, 12, 0, 0)
            .unwrap()
            .timestamp_millis();
        let oct = Local
            .with_ymd_and_hms(2026, 10, 1, 0, 0, 0)
            .unwrap()
            .timestamp_millis();
        assert!(start <= mid && mid < end);
        assert!(oct >= end);
    }
}
