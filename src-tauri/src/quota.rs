//! Read-only quota windows (T3-style display, no auth).
//!
//! Two sources, both read-only:
//! - OpenCode: `GET {base}/usage` with the CLI's own API key as `Bearer`.
//! - Codex: `GET https://chatgpt.com/backend-api/wham/usage` with the CLI's
//!   own OAuth access token. Usage popups read all available sources before
//!   UI filtering; the native menu-bar monitor reads only its selected source.
//!
//! Honest failures (shown verbatim in the UI):
//! - 401 / expired token → sign in again / open Codex once to refresh,
//! - 403 → the key carries no quota (no active plan?),
//! - anything else → the raw status / transport error.

use serde::Serialize;
use std::time::{Duration, SystemTime};

use crate::{codex, paths};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub window: String,
    pub percent: f64,
    pub resets_at: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSet {
    /// Source id: `opencode`, `opencode-go`, `claude` or `codex`.
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<Vec<QuotaWindow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When the snapshot was observed, ISO timestamp (Claude transcripts).
    /// Absent for live probes (OpenCode), which are always current.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
}

/// auth.json entry → quota base URL. Only these two have a usage endpoint.
fn quota_base(entry_id: &str) -> Option<&'static str> {
    match entry_id {
        "opencode-go" => Some("https://opencode.ai/zen/go/v1"),
        "opencode" => Some("https://opencode.ai/zen/v1"),
        _ => None,
    }
}

/// Shared by quota display and Go subscription renewal detection.
pub(crate) fn parse_windows(json: &serde_json::Value) -> Option<Vec<QuotaWindow>> {
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

fn api_error(status: u16) -> String {
    match status {
        401 => "key rejected — run `opencode auth login` again".to_string(),
        403 => "no quota on this key (no active plan?)".to_string(),
        _ => format!("HTTP {status}"),
    }
}

/// Read-only fetch for one auth.json entry. Never writes credentials.
fn fetch_entry(entry_id: &str, base: &str, api_key: &str) -> QuotaSet {
    let fail = |error: String| QuotaSet {
        provider: entry_id.to_string(),
        windows: None,
        error: Some(error),
        as_of: None,
    };
    let client = match reqwest::blocking::Client::builder()
        .user_agent(concat!("Keyturn/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => return fail(e.to_string()),
    };
    let response = match client
        .get(format!("{base}/usage"))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Accept", "application/json")
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            return fail(if e.is_timeout() {
                "request timed out".to_string()
            } else {
                e.to_string()
            })
        }
    };
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return fail(api_error(status));
    }
    let json: serde_json::Value = match response.json() {
        Ok(j) => j,
        Err(e) => return fail(format!("invalid response: {e}")),
    };
    match parse_windows(&json) {
        Some(windows) => QuotaSet {
            provider: entry_id.to_string(),
            windows: Some(windows),
            error: None,
            as_of: None,
        },
        None => fail("unexpected usage payload".to_string()),
    }
}

/// Quota for every usable OpenCode entry in the CLI's auth file.
/// Empty when nothing usable is stored. Read-only throughout.
pub fn opencode_quotas() -> Vec<QuotaSet> {
    let file = paths::open_code_auth_file();
    let Some(raw) = paths::read_json(&file) else {
        return Vec::new();
    };
    let Some(map) = raw.as_object() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (id, entry) in map {
        if id.starts_with('$') {
            continue;
        }
        let Some(base) = quota_base(id) else {
            continue;
        };
        let Some(key) = entry.get("key").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) else {
            continue;
        };
        out.push(fetch_entry(id, base, key));
    }
    out.sort_by(|a, b| a.provider.cmp(&b.provider));
    out
}

/// Fetch only the chosen menu-bar source; never probe unrelated accounts.
pub fn provider_quota(provider: crate::settings::MenuBarProvider) -> Option<QuotaSet> {
    use crate::settings::MenuBarProvider;
    match provider {
        MenuBarProvider::Codex => codex_quota_set(),
        MenuBarProvider::Claude => crate::claude_usage::claude_quota_set(),
        MenuBarProvider::Opencode | MenuBarProvider::OpencodeGo => {
            let id = provider.id();
            let raw = paths::read_json(&paths::open_code_auth_file())?;
            let key = raw.get(id)?.get("key")?.as_str().filter(|s| !s.is_empty())?;
            Some(fetch_entry(id, quota_base(id)?, key))
        }
    }
}

/// Label a Codex window by its duration, never by its position: plans move
/// windows around (a 604800s primary is the weekly bucket, not "5-hour").
fn codex_window_label(seconds: i64) -> String {
    match seconds {
        18_000 => "5-hour".to_string(),
        604_800 => "Weekly".to_string(),
        _ if seconds % 86_400 == 0 => format!("{}d", seconds / 86_400),
        _ if seconds % 3_600 == 0 => format!("{}h", seconds / 3_600),
        _ => format!("{}m", seconds / 60),
    }
}

fn codex_window(bucket: &serde_json::Value, label: String) -> Option<QuotaWindow> {
    let percent = bucket.get("used_percent").and_then(|v| v.as_f64())?;
    let resets_at = bucket
        .get("reset_at")
        .and_then(|v| v.as_i64())
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|d| d.to_rfc3339());
    Some(QuotaWindow {
        window: label,
        percent,
        resets_at,
        status: None,
    })
}

/// Read-only Codex quota via the CLI's own wham endpoint. Returns `None`
/// when no OAuth session exists (API-key logins have no wham quota);
/// otherwise a set with windows or a human error. Never writes.
pub fn codex_quota_set() -> Option<QuotaSet> {
    let raw = paths::read_json(&paths::codex_auth_file())?;
    codex_quota_from_auth(&raw)
}

/// Read the usage of a saved login without activating it.
pub fn codex_quota_from_auth(raw: &serde_json::Value) -> Option<QuotaSet> {
    let (token, account_id, expires_at) = codex::oauth_access_from(raw)?;
    let fail = |error: String| QuotaSet {
        provider: "codex".to_string(),
        windows: None,
        error: Some(error),
        as_of: None,
    };
    if let Some(exp) = expires_at {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        if exp <= now {
            return Some(fail(
                "session expired — open Codex once to refresh, then retry".to_string(),
            ));
        }
    }
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Some(fail(e.to_string())),
    };
    let mut request = client
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .header("User-Agent", "Keyturn");
    if let Some(account) = account_id.as_deref().filter(|s| !s.is_empty()) {
        request = request.header("ChatGPT-Account-Id", account);
    }
    let response = match request.send() {
        Ok(r) => r,
        Err(e) => {
            return Some(fail(if e.is_timeout() {
                "request timed out".to_string()
            } else {
                e.to_string()
            }))
        }
    };
    let status = response.status().as_u16();
    if status == 401 {
        return Some(fail(
            "session rejected — sign in again with `codex login`".to_string(),
        ));
    }
    if !(200..300).contains(&status) {
        return Some(fail(api_error(status)));
    }
    let json: serde_json::Value = match response.json() {
        Ok(j) => j,
        Err(e) => return Some(fail(format!("invalid response: {e}"))),
    };
    let rate_limit = json.get("rate_limit").cloned().unwrap_or_default();
    let mut windows = Vec::new();
    for key in ["primary_window", "secondary_window"] {
        let Some(bucket) = rate_limit.get(key) else {
            continue;
        };
        if bucket.is_null() {
            continue;
        }
        let seconds = bucket
            .get("limit_window_seconds")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if let Some(w) = codex_window(bucket, codex_window_label(seconds)) {
            windows.push(w);
        }
    }
    if let Some(extras) = json
        .get("additional_rate_limits")
        .and_then(|v| v.as_array())
    {
        for extra in extras {
            let name = extra
                .get("limit_name")
                .and_then(|v| v.as_str())
                .unwrap_or("extra");
            // Internal spare capacity, not a user-facing limit — hide it.
            if name.eq_ignore_ascii_case("gpt-reserve") {
                continue;
            }
            let rl = extra.get("rate_limit").cloned().unwrap_or_default();
            for (key, suffix) in [("primary_window", ""), ("secondary_window", " · long")] {
                let Some(bucket) = rl.get(key) else {
                    continue;
                };
                if bucket.is_null() {
                    continue;
                }
                if let Some(mut w) = codex_window(bucket, String::new()) {
                    w.window = format!("{name}{suffix}");
                    windows.push(w);
                }
            }
        }
    }
    if windows.is_empty() {
        return Some(fail("no quota windows in the response".to_string()));
    }
    Some(QuotaSet {
        provider: "codex".to_string(),
        windows: Some(windows),
        error: None,
        as_of: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_base_covers_only_the_two_zen_entries() {
        assert_eq!(quota_base("opencode-go"), Some("https://opencode.ai/zen/go/v1"));
        assert_eq!(quota_base("opencode"), Some("https://opencode.ai/zen/v1"));
        assert_eq!(quota_base("anthropic"), None);
        assert_eq!(quota_base("google"), None);
    }

    #[test]
    fn windows_parse_in_exact_shape() {
        let json = serde_json::json!({
            "usage": {
                "rolling": { "status": "ok", "percent": 4.0, "resetsAt": "2026-09-13T16:27:38.287Z" },
                "weekly": { "status": "ok", "percent": 3.0 },
                "monthly": { "status": "limited", "percent": 100.0, "resetsAt": null }
            }
        });
        let windows = parse_windows(&json).expect("windows");
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].window, "rolling");
        assert_eq!(windows[0].resets_at.as_deref(), Some("2026-09-13T16:27:38.287Z"));
        assert_eq!(windows[2].status.as_deref(), Some("limited"));
        assert!(parse_windows(&serde_json::json!({})).is_none());
        assert!(parse_windows(&serde_json::json!({"usage": {"rolling": {}}})).is_none());
    }

    #[test]
    fn api_errors_speak_human() {
        assert!(api_error(401).contains("rejected"));
        assert!(api_error(403).contains("no quota"));
        assert_eq!(api_error(500), "HTTP 500");
    }

    #[test]
    fn codex_window_labels_come_from_durations() {
        assert_eq!(codex_window_label(18_000), "5-hour");
        assert_eq!(codex_window_label(604_800), "Weekly");
        assert_eq!(codex_window_label(86_400), "1d");
        assert_eq!(codex_window_label(3_600), "1h");
    }

    #[test]
    fn codex_payload_parses_windows_and_extras() {
        let json = serde_json::json!({
            "plan_type": "prolite",
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "primary_window": {
                    "used_percent": 10.0,
                    "limit_window_seconds": 604800,
                    "reset_after_seconds": 500000,
                    "reset_at": 1791591439
                },
                "secondary_window": null
            },
            "additional_rate_limits": [
                {
                    "limit_name": "gpt-reserve",
                    "rate_limit": {
                        "primary_window": {
                            "used_percent": 0.0,
                            "limit_window_seconds": 18000,
                            "reset_after_seconds": 18000,
                            "reset_at": 1791591439
                        }
                    }
                }
            ]
        });
        let rate_limit = json.get("rate_limit").cloned().unwrap_or_default();
        let primary = rate_limit.get("primary_window").unwrap();
        let w = codex_window(primary, codex_window_label(604800)).expect("window");
        assert_eq!(w.window, "Weekly");
        assert!((w.percent - 10.0).abs() < 1e-9);
        assert!(w.resets_at.as_deref().is_some_and(|s| s.starts_with("2026-")));
        let extra = json["additional_rate_limits"][0]["rate_limit"]["primary_window"].clone();
        let spare = codex_window(&extra, "gpt-reserve".to_string()).expect("extra");
        assert_eq!(spare.window, "gpt-reserve");
        assert!(codex_window(&serde_json::json!({}), "x".to_string()).is_none());
    }
}
