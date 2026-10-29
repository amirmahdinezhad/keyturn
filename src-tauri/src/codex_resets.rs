//! Public Codex reset announcements and forecasts. No account data is sent.
//!
//! Source contract: https://codex-resets.com/api/docs. Scheduled announcements
//! await execution evidence, even after their scheduled time has passed.

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const STATUS_URL: &str = "https://codex-resets.com/api/v1/status";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResetType {
    Regular,
    Banked,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WatchLevel {
    Elevated,
    Strong,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ScheduledReset {
    pub reset_type: ResetType,
    pub announced_at: String,
    pub scheduled_for: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ResetWatch {
    pub level: WatchLevel,
    pub reset_chance_percent: Option<u8>,
    pub forecast_window: String,
    pub observed_at: String,
    pub expires_at: String,
}

/// A confirmed execution from `latest_reset`, including events missed offline.
/// Its timestamp is when the source announced or first observed the reset.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct CompletedReset {
    pub id: String,
    pub reset_type: ResetType,
    pub announced_at: String,
}

/// Only reset metadata crosses IPC; announcement bodies stay at the source.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexResetStatus {
    pub latest_reset: Option<CompletedReset>,
    pub scheduled_reset: Option<ScheduledReset>,
    pub active_watch: Option<ResetWatch>,
    pub generated_at: String,
}

#[derive(Deserialize)]
struct StatusResponse {
    data: StatusData,
    meta: Meta,
}

#[derive(Deserialize)]
struct StatusData {
    latest_reset: Option<CompletedReset>,
    scheduled_reset: Option<ScheduledReset>,
    active_watch: Option<ResetWatch>,
}

#[derive(Deserialize)]
struct Meta {
    api_version: String,
    generated_at: String,
}

fn valid_date(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value).is_ok()
}

fn parse_status(response: StatusResponse) -> Result<CodexResetStatus, String> {
    let generated = chrono::DateTime::parse_from_rfc3339(&response.meta.generated_at)
        .map_err(|_| "Reset outlook format is unsupported")?;
    if response.meta.api_version != "v1" {
        return Err("Reset outlook format is unsupported".into());
    }
    if let Some(reset) = &response.data.latest_reset {
        let reported = chrono::DateTime::parse_from_rfc3339(&reset.announced_at)
            .map_err(|_| "Completed reset metadata is invalid")?;
        if reset.id.trim().is_empty() || reset.id.len() > 64 || reported > generated {
            return Err("Completed reset metadata is invalid".into());
        }
    }
    if let Some(reset) = &response.data.scheduled_reset {
        if !valid_date(&reset.announced_at)
            || reset
                .scheduled_for
                .as_deref()
                .is_some_and(|date| !valid_date(date))
        {
            return Err("Reset announcement has invalid dates".into());
        }
    }
    if let Some(watch) = &response.data.active_watch {
        if watch
            .reset_chance_percent
            .is_some_and(|percent| percent > 100)
            || watch.forecast_window.trim().is_empty()
            || watch.forecast_window.len() > 120
            || !valid_date(&watch.observed_at)
            || !valid_date(&watch.expires_at)
        {
            return Err("Reset forecast is invalid".into());
        }
    }
    Ok(CodexResetStatus {
        latest_reset: response.data.latest_reset,
        scheduled_reset: response.data.scheduled_reset,
        active_watch: response.data.active_watch,
        generated_at: response.meta.generated_at,
    })
}

fn fetch_status() -> Result<CodexResetStatus, String> {
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| "Reset outlook is unavailable")?;
    let response: StatusResponse = client
        .get(STATUS_URL)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(|response| response.json())
        .map_err(|_| "Reset outlook is unavailable")?;
    parse_status(response)
}

struct CachedStatus {
    checked_at: Instant,
    result: Result<CodexResetStatus, String>,
}

/// Reuse one public response across cards, honoring the saved refetch interval.
pub fn status(interval: Duration) -> Result<CodexResetStatus, String> {
    static CACHE: OnceLock<Mutex<Option<CachedStatus>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "Reset outlook is unavailable")?;
    read_cached(&mut cache, interval, fetch_status)
}

fn read_cached(
    cache: &mut Option<CachedStatus>,
    interval: Duration,
    fetch: impl FnOnce() -> Result<CodexResetStatus, String>,
) -> Result<CodexResetStatus, String> {
    if let Some(cached) = cache.as_ref() {
        if cached.checked_at.elapsed() < interval {
            return cached.result.clone();
        }
    }
    let result = fetch();
    *cache = Some(CachedStatus {
        checked_at: Instant::now(),
        result: result.clone(),
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn payload() -> Value {
        json!({
            "data": { "scheduled_reset": null, "active_watch": null },
            "meta": { "api_version": "v1", "generated_at": "2026-10-13T04:00:00Z" }
        })
    }

    fn parse(value: Value) -> Result<CodexResetStatus, String> {
        parse_status(serde_json::from_value(value).map_err(|e| e.to_string())?)
    }

    #[test]
    fn selected_interval_controls_success_and_failure_cache_requests() {
        let mut cache = None;
        let mut requests = 0;
        let mut fetch = || { requests += 1; parse(payload()) };
        read_cached(&mut cache, Duration::from_secs(300), &mut fetch).unwrap();
        cache.as_mut().unwrap().checked_at = Instant::now() - Duration::from_secs(120);
        read_cached(&mut cache, Duration::from_secs(300), &mut fetch).unwrap();
        read_cached(&mut cache, Duration::from_secs(60), &mut fetch).unwrap();
        assert_eq!(requests, 2);

        cache.as_mut().unwrap().result = Err("offline".into());
        let mut failed_requests = 0;
        let mut fail = || { failed_requests += 1; Err("offline".into()) };
        assert!(read_cached(&mut cache, Duration::from_secs(60), &mut fail).is_err());
        cache.as_mut().unwrap().checked_at = Instant::now() - Duration::from_secs(61);
        assert!(read_cached(&mut cache, Duration::from_secs(60), &mut fail).is_err());
        assert_eq!(failed_requests, 1);
    }

    #[test]
    fn no_announcement_is_distinct_from_a_source_failure() {
        let status = parse(payload()).unwrap();
        assert!(status.latest_reset.is_none());
        assert!(status.scheduled_reset.is_none());
        assert!(status.active_watch.is_none());
        assert!(parse(json!({})).is_err());
    }

    #[test]
    fn completed_reset_metadata_survives_ipc_without_announcement_content() {
        let mut value = payload();
        value["data"]["latest_reset"] = json!({
            "id": "observed-reset-1", "reset_type": "regular",
            "announced_at": "2026-10-12T07:00:00Z",
            "text": "Source announcement", "source": { "type": "observed" }
        });
        let serialized = serde_json::to_value(parse(value).unwrap()).unwrap();
        assert_eq!(serialized["latestReset"]["id"], "observed-reset-1");
        assert_eq!(serialized["latestReset"]["resetType"], "regular");
        assert_eq!(
            serialized["latestReset"]["announcedAt"],
            "2026-10-12T07:00:00Z"
        );
        assert!(serialized["latestReset"].get("text").is_none());
        assert!(serialized["latestReset"].get("source").is_none());
    }

    #[test]
    fn completed_resets_require_an_id_and_a_valid_nonfuture_date() {
        for (id, date) in [
            ("", "2026-10-12T07:00:00Z"),
            ("   ", "2026-10-12T07:00:00Z"),
            ("reset-1", "invalid"),
            ("reset-1", "2026-10-14T07:00:00Z"),
        ] {
            let mut value = payload();
            value["data"]["latest_reset"] = json!({
                "id": id, "reset_type": "regular", "announced_at": date
            });
            assert!(
                parse(value).is_err(),
                "accepted invalid reset: {id} / {date}"
            );
        }
        let mut value = payload();
        value["data"]["latest_reset"] = json!({
            "id": "x".repeat(65), "reset_type": "banked",
            "announced_at": "2026-10-12T07:00:00Z"
        });
        assert!(parse(value).is_err());
    }

    #[test]
    fn scheduled_reset_is_serialized_as_an_announcement_even_if_overdue() {
        let mut value = payload();
        value["data"]["scheduled_reset"] = json!({
            "reset_type": "banked", "announced_at": "2026-10-12T01:00:00Z",
            "scheduled_for": "2026-10-12T07:00:00Z"
        });
        let serialized = serde_json::to_value(parse(value).unwrap()).unwrap();
        assert_eq!(serialized["scheduledReset"]["resetType"], "banked");
        assert_eq!(
            serialized["scheduledReset"]["scheduledFor"],
            "2026-10-12T07:00:00Z"
        );
        assert_eq!(serialized["generatedAt"], "2026-10-13T04:00:00Z");
        assert!(serialized["activeWatch"].is_null());
    }

    #[test]
    fn forecast_preserves_nullable_probability_and_expiry() {
        let mut value = payload();
        value["data"]["active_watch"] = json!({
            "level": "strong", "reset_chance_percent": null,
            "forecast_window": "Within 24 hours", "observed_at": "2026-10-13T03:00:00Z",
            "expires_at": "2026-10-14T03:00:00Z"
        });
        let serialized = serde_json::to_value(parse(value.clone()).unwrap()).unwrap();
        assert!(serialized["activeWatch"]["resetChancePercent"].is_null());
        assert_eq!(
            serialized["activeWatch"]["expiresAt"],
            "2026-10-14T03:00:00Z"
        );
        value["data"]["active_watch"]["reset_chance_percent"] = json!(101);
        assert!(parse(value).is_err());
    }

    #[test]
    fn unsupported_schema_and_invalid_dates_are_rejected() {
        let mut value = payload();
        value["meta"]["api_version"] = json!("v2");
        assert!(parse(value).is_err());
        let mut value = payload();
        value["meta"]["generated_at"] = json!("invalid");
        assert!(parse(value).is_err());
    }
}
