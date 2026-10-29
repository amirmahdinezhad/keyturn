//! Read-only Claude usage: live OAuth limits on demand, with local snapshot fallback.
//!
//! Scan JSONL transcripts under the Claude projects directory for tool-result
//! JSON containing `five_hour`, `seven_day` and optional model buckets with
//! `utilization` percentages and `resets_at` timestamps.
//!
//! We scan transcripts for the NEWEST such snapshot and show it with its
//! observation time as a fallback. Live reads use the selected config’s OAuth
//! login without modifying credentials.

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::paths;
use crate::quota::{QuotaSet, QuotaWindow};

/// Transcript buckets we surface, in display order.
const BUCKETS: &[(&str, &str)] = &[
    ("five_hour", "5-hour"),
    ("seven_day", "Weekly"),
    ("seven_day_opus", "Opus weekly"),
    ("seven_day_sonnet", "Sonnet weekly"),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeLimits {
    pub windows: Vec<ClaudeWindow>,
    /// ISO timestamp of the transcript record the snapshot came from.
    pub as_of: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeWindow {
    pub label: String,
    /// Percent USED, like the CLI reports it.
    pub percent: f64,
    pub resets_at: Option<String>,
}

/// A candidate snapshot: its JSON plus the record timestamp for recency.
struct Snapshot {
    at: String,
    payload: serde_json::Value,
}

/// Pull the rate-limit JSON out of a tool_result content blob.
/// Content is a bare JSON object on its first line (trailing lines ignored).
fn extract_snapshot(text: &str) -> Option<serde_json::Value> {
    let first = text.trim().split('\n').next()?;
    let value: serde_json::Value = serde_json::from_str(first).ok()?;
    if value.get("five_hour").is_some() || value.get("seven_day").is_some() {
        Some(value)
    } else {
        None
    }
}

/// Yield every transcript line mentioning a snapshot, cheaply.
fn scan_file(path: &Path, out: &mut Vec<Snapshot>) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return;
    };
    for line in raw.lines() {
        if !line.contains("five_hour") && !line.contains("seven_day") {
            continue;
        }
        let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let at = record
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let items: Vec<&serde_json::Value> =
            match record.get("message").and_then(|m| m.get("content")) {
                Some(serde_json::Value::Array(items)) => items.iter().collect(),
                _ => Vec::new(),
            };
        for item in items {
            if item.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
                continue;
            }
            let texts: Vec<&str> = match item.get("content") {
                Some(serde_json::Value::String(s)) => vec![s],
                Some(serde_json::Value::Array(blocks)) => blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect(),
                _ => Vec::new(),
            };
            for text in texts {
                if let Some(payload) = extract_snapshot(text) {
                    out.push(Snapshot {
                        at: at.clone(),
                        payload,
                    });
                }
            }
        }
    }
}

fn visit_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_jsonl(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

fn bucket_window(payload: &serde_json::Value, key: &str, label: &str) -> Option<ClaudeWindow> {
    let bucket = payload.get(key)?;
    if bucket.is_null() {
        return None;
    }
    let percent = bucket.get("utilization").and_then(|v| v.as_f64())?;
    Some(ClaudeWindow {
        label: label.to_string(),
        percent,
        resets_at: bucket
            .get("resets_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

/// Newest persisted rate-limit snapshot, if the user ever observed one.
/// Pure local files; staleness is reported via `as_of` for the UI.
pub fn claude_limits() -> Option<ClaudeLimits> {
    claude_limits_in(&paths::claude_home().join("projects"))
}

fn claude_limits_in(projects: &Path) -> Option<ClaudeLimits> {
    let mut files = Vec::new();
    visit_jsonl(&projects, &mut files);
    let mut snapshots = Vec::new();
    for path in files {
        scan_file(&path, &mut snapshots);
    }
    // ISO timestamps sort lexicographically in time order.
    snapshots.sort_by(|a, b| a.at.cmp(&b.at));
    let newest = snapshots.into_iter().last()?;
    if newest.at.is_empty() {
        return None;
    }
    let mut windows = Vec::new();
    for (key, label) in BUCKETS {
        if let Some(w) = bucket_window(&newest.payload, key, label) {
            windows.push(w);
        }
    }
    if windows.is_empty() {
        return None;
    }
    Some(ClaudeLimits {
        windows,
        as_of: newest.at,
    })
}

/// Shape shared with the quota UI: one `QuotaSet` for the card.
pub fn claude_quota_set() -> Option<QuotaSet> {
    claude_quota_in(&paths::claude_home())
}

/// Isolated Claude profiles keep their own local transcript history.
pub fn claude_quota_in(config: &Path) -> Option<QuotaSet> {
    let limits = claude_limits_in(&config.join("projects"))?;
    Some(QuotaSet {
        provider: "claude".to_string(),
        windows: Some(
            limits
                .windows
                .into_iter()
                .map(|w| QuotaWindow {
                    window: w.label,
                    percent: w.percent,
                    resets_at: w.resets_at,
                    status: None,
                })
                .collect(),
        ),
        error: None,
        as_of: Some(limits.as_of),
    })
}

/// Read only this configuration's Claude-owned OAuth login. Never use another
/// profile's credentials or an API key in place of its subscription session.
fn access_token(config: &Path) -> Result<String, String> {
    fn token(raw: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(raw).ok()?;
        value
            .get("claudeAiOauth")?
            .get("accessToken")?
            .as_str()
            .filter(|token| !token.trim().is_empty())
            .map(str::to_owned)
    }
    #[cfg(target_os = "macos")]
    {
        use sha2::{Digest, Sha256};
        let default = dirs::home_dir().unwrap_or_default().join(".claude");
        let hash = format!("{:x}", Sha256::digest(config.to_string_lossy().as_bytes()));
        // CLAUDE_CONFIG_DIR scopes macOS Keychain service names by directory hash.
        let mut services = vec![format!("Claude Code-credentials-{}", &hash[..8])];
        if config == default {
            services.push("Claude Code-credentials".into());
        }
        for service in services {
            if let Some(raw) = crate::cli::claude_keychain_credentials(&service) {
                if let Some(token) = token(&raw) {
                    return Ok(token);
                }
            }
        }
    }
    if let Ok(raw) = std::fs::read_to_string(config.join(".credentials.json")) {
        if let Some(token) = token(&raw) {
            return Ok(token);
        }
    }
    Err("Claude login is unavailable for this profile. Sign in with its login command, then try again.".into())
}

fn live_windows(payload: &serde_json::Value) -> Result<Vec<QuotaWindow>, String> {
    let windows: Vec<_> = BUCKETS
        .iter()
        .filter_map(|(key, label)| {
            let w = bucket_window(payload, key, label)?;
            if !w.percent.is_finite() || !(0.0..=100.0).contains(&w.percent) {
                return None;
            }
            Some(QuotaWindow {
                window: w.label,
                percent: w.percent,
                resets_at: w.resets_at,
                status: None,
            })
        })
        .collect();
    if windows.is_empty() {
        Err("Claude returned no subscription usage windows for this account.".into())
    } else {
        Ok(windows)
    }
}

fn fetch_live(token: &str) -> Result<Vec<QuotaWindow>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Cannot initialize Claude usage request.")?;
    let response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("Accept", "application/json")
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                "Claude usage request timed out."
            } else {
                "Cannot reach Claude usage. Check your connection and try again."
            }
        })?;
    match response.status().as_u16() {
        200 => {}
        401 => {
            return Err(
                "Claude session expired. Open Claude with this profile and sign in again.".into(),
            )
        }
        403 => return Err("Claude did not allow usage access for this account.".into()),
        429 => return Err("Claude usage is temporarily rate limited. Try again later.".into()),
        status => return Err(format!("Claude usage returned HTTP {status}.")),
    }
    let payload = response
        .json::<serde_json::Value>()
        .map_err(|_| "Claude returned an invalid usage response.")?;
    live_windows(&payload)
}

/// Fetch live usage on demand. Local transcript fallback always retains its age.
/// Neither tokens nor raw provider error responses cross the IPC boundary.
pub fn live_quota_in(config: &Path) -> QuotaSet {
    quota_with(config, || fetch_live(&access_token(config)?))
}

/// Health checks must expose current failures even when a transcript snapshot exists.
pub fn current_quota_in(config: &Path) -> QuotaSet {
    current_quota_with(|| fetch_live(&access_token(config)?))
}

fn current_quota_with(fetch: impl FnOnce() -> Result<Vec<QuotaWindow>, String>) -> QuotaSet {
    match fetch() {
        Ok(windows) => QuotaSet { provider: "claude".into(), windows: Some(windows), error: None, as_of: Some(chrono::Utc::now().to_rfc3339()) },
        Err(error) => QuotaSet { provider: "claude".into(), windows: None, error: Some(error), as_of: None },
    }
}

fn quota_with(config: &Path, fetch: impl FnOnce() -> Result<Vec<QuotaWindow>, String>) -> QuotaSet {
    match fetch() {
        Ok(windows) => QuotaSet {
            provider: "claude".into(),
            windows: Some(windows),
            error: None,
            as_of: Some(chrono::Utc::now().to_rfc3339()),
        },
        Err(error) => claude_quota_in(config).unwrap_or(QuotaSet {
            provider: "claude".into(),
            windows: None,
            error: Some(error),
            as_of: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn live_usage_does_not_require_transcript_and_failure_is_explicit() {
        let dir = std::env::temp_dir().join(format!("keyturn-live-{}", uuid::Uuid::new_v4()));
        let payload = serde_json::json!({"five_hour": {"utilization":42.0,"resets_at":"2026-10-28T14:00:00Z"},"seven_day":null});
        let quota = quota_with(&dir, || live_windows(&payload));
        assert_eq!(quota.windows.unwrap()[0].percent, 42.0);
        assert!(quota.error.is_none());
        assert!(quota.as_of.is_some());
        let error = quota_with(&dir, || Err("Session expired".into()));
        assert!(error.windows.is_none());
        assert_eq!(error.error.as_deref(), Some("Session expired"));
        assert!(live_windows(&serde_json::json!({"five_hour":{"utilization":-1}})).is_err());
    }

    #[test]
    fn current_health_failure_is_explicit_even_when_historical_usage_is_allowed_elsewhere() {
        let failed = current_quota_with(|| Err("Session expired".into()));
        assert_eq!(failed.error.as_deref(), Some("Session expired"));
        assert!(failed.windows.is_none());
        let limited = current_quota_with(|| Ok(vec![QuotaWindow { window: "weekly".into(), percent: 100.0, resets_at: None, status: None }]));
        assert!(limited.error.is_none());
        assert_eq!(limited.windows.unwrap()[0].percent, 100.0);
    }

    #[test]
    fn failed_live_read_retains_only_this_profiles_snapshot_and_timestamp() {
        let dir = std::env::temp_dir().join(format!("keyturn-fallback-{}", uuid::Uuid::new_v4()));
        let projects = dir.join("projects");
        std::fs::create_dir_all(&projects).unwrap();
        transcript_with_snapshot(&projects, "one.jsonl", "2026-10-28T10:00:00Z", false);
        let result = quota_with(&dir, || Err("Offline".into()));
        assert_eq!(result.as_of.as_deref(), Some("2026-10-28T10:00:00Z"));
        assert!(result.windows.is_some());
        assert!(quota_with(&dir.join("other"), || Err("Offline".into()))
            .windows
            .is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn transcript_with_snapshot(dir: &Path, name: &str, at: &str, extra_nulls: bool) {
        let opus = if extra_nulls {
            serde_json::json!({"utilization": 12.0, "resets_at": "2026-09-27T00:00:00Z"})
        } else {
            serde_json::Value::Null
        };
        let tool_text = serde_json::json!({
            "five_hour": {"utilization": 50.0, "resets_at": "2026-09-26T21:19:59Z"},
            "seven_day": {"utilization": 16.0, "resets_at": "2026-09-27T16:59:59Z"},
            "seven_day_opus": opus,
        })
        .to_string()
            + "\nSome trailing assistant chatter";
        let record = serde_json::json!({
            "timestamp": at,
            "type": "user",
            "message": {
                "role": "user",
                "content": [{ "type": "tool_result", "content": tool_text }]
            }
        });
        let mut f = std::fs::File::create(dir.join(name)).unwrap();
        writeln!(f, "{record}").unwrap();
        writeln!(f, "{{\"type\":\"assistant\",\"message\":{{}}}}").unwrap();
    }

    #[test]
    fn newest_snapshot_wins_and_null_buckets_drop() {
        let dir = std::env::temp_dir().join(format!("keyturn-cl-{}", std::process::id()));
        let projects = dir.join("projects").join("p");
        std::fs::create_dir_all(&projects).unwrap();
        transcript_with_snapshot(&projects, "old.jsonl", "2026-09-20T10:00:00Z", false);
        transcript_with_snapshot(&projects, "new.jsonl", "2026-09-26T17:15:26Z", true);

        let limits = claude_limits_in(&projects).expect("snapshot");
        assert_eq!(limits.as_of, "2026-09-26T17:15:26Z");
        let labels: Vec<_> = limits.windows.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(labels, vec!["5-hour", "Weekly", "Opus weekly"]);
        assert!((limits.windows[0].percent - 50.0).abs() < 1e-9);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_snapshot_means_nothing_to_show() {
        let dir = std::env::temp_dir().join(format!("keyturn-cl-empty-{}", std::process::id()));
        let projects = dir.join("projects");
        std::fs::create_dir_all(&projects).unwrap();
        assert!(claude_limits_in(&projects).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
