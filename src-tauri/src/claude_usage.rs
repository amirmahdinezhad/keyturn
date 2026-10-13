//! Read-only Claude rate-limit snapshot from local transcripts.
//!
//! Scan JSONL transcripts under the Claude projects directory for tool-result
//! JSON containing `five_hour`, `seven_day` and optional model buckets with
//! `utilization` percentages and `resets_at` timestamps.
//!
//! We scan transcripts for the NEWEST such snapshot and show it with its
//! observation time. No keychain, no API calls, no writes: if the user
//! transcripts contain no matching snapshot, there is nothing to show.

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
        let items: Vec<&serde_json::Value> = match record
            .get("message")
            .and_then(|m| m.get("content"))
        {
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
    let limits = claude_limits()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
