//! History ranges, chart buckets and JSON/CSV export.
//!
//! Everything is derived from the same local usage DB, so history follows
//! the active profile for free.

use chrono::{Datelike, Local, TimeZone};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::paths;
use crate::usage::{self, UsageEntry};

pub const RANGES: &[&str] = &["5h", "24h", "7d", "30d", "billing"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryBucket {
    /// Bucket start, epoch millis.
    pub t: i64,
    pub cost: f64,
    pub tokens: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryOut {
    pub range: String,
    pub label: String,
    pub start: i64,
    pub end: i64,
    pub bucket_ms: i64,
    pub buckets: Vec<HistoryBucket>,
    pub total_cost: f64,
    pub total_tokens: f64,
    pub models: BTreeMap<String, f64>,
    pub error: Option<String>,
}

fn midnight(year: i32, month: u32, day: u32) -> i64 {
    Local
        .with_ymd_and_hms(year, month, day, 0, 0, 0)
        .single()
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
}

/// (start, end, bucket width, label) for a range.
pub fn range_spec(range: &str, billing_day: u32, now: i64) -> (i64, i64, i64, String) {
    let minute = 60_000i64;
    let hour = 60 * minute;
    let day = 24 * hour;
    match range {
        "5h" => (now - 5 * hour, now, 5 * minute, "Last 5 hours".to_string()),
        "7d" => (now - 7 * day, now, 6 * hour, "Last 7 days".to_string()),
        "30d" => (now - 30 * day, now, day, "Last 30 days".to_string()),
        "billing" => {
            let today = Local.timestamp_millis_opt(now).unwrap().date_naive();
            let day_of_month = today.day();
            let (year, month) = if day_of_month >= billing_day {
                (today.year(), today.month())
            } else if today.month() == 1 {
                (today.year() - 1, 12)
            } else {
                (today.year(), today.month() - 1)
            };
            let start = midnight(year, month, billing_day.min(28));
            (start, now, day, "Current billing cycle".to_string())
        }
        _ => (
            now - 24 * hour,
            now,
            30 * minute,
            "Last 24 hours".to_string(),
        ),
    }
}

/// Build zero-filled buckets covering the whole range.
pub fn build(
    entries: &[UsageEntry],
    range: &str,
    billing_day: u32,
    now: i64,
) -> HistoryOut {
    let (start, end, bucket_ms, label) = range_spec(range, billing_day, now);
    let count = (((end - start) / bucket_ms).max(1) + 1) as usize;
    let mut buckets = vec![
        HistoryBucket {
            t: 0,
            cost: 0.0,
            tokens: 0.0,
        };
        count
    ];
    for (i, bucket) in buckets.iter_mut().enumerate() {
        bucket.t = start + i as i64 * bucket_ms;
    }
    let mut total_cost = 0.0;
    let mut total_tokens = 0.0;
    let mut models: BTreeMap<String, f64> = BTreeMap::new();

    for row in entries {
        if row.t < start || row.t >= end {
            continue;
        }
        let index = (((row.t - start) / bucket_ms).max(0) as usize).min(count - 1);
        buckets[index].cost += row.cost;
        buckets[index].tokens += row.tokens;
        total_cost += row.cost;
        total_tokens += row.tokens;
        *models.entry(row.model.clone()).or_insert(0.0) += row.cost;
    }

    HistoryOut {
        range: if RANGES.contains(&range) {
            range
        } else {
            "24h"
        }
        .to_string(),
        label,
        start,
        end,
        bucket_ms,
        buckets,
        total_cost,
        total_tokens,
        models,
        error: None,
    }
}

fn iso(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .earliest()
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}

pub fn to_csv(out: &HistoryOut) -> String {
    let mut text = String::from("bucket_start,bucket_end,cost,tokens\n");
    for bucket in &out.buckets {
        text.push_str(&format!(
            "{},{},{:.6},{:.0}\n",
            iso(bucket.t),
            iso(bucket.t + out.bucket_ms),
            bucket.cost,
            bucket.tokens
        ));
    }
    text.push_str(&format!(
        "total,,{:.6},{:.0}\n",
        out.total_cost, out.total_tokens
    ));
    text
}

pub fn serialize(out: &HistoryOut, format: &str) -> Result<String, String> {
    if format.eq_ignore_ascii_case("csv") {
        return Ok(to_csv(out));
    }
    if format.eq_ignore_ascii_case("json") {
        let buckets: Vec<serde_json::Value> = out
            .buckets
            .iter()
            .map(|b| {
                serde_json::json!({
                    "start": iso(b.t),
                    "end": iso(b.t + out.bucket_ms),
                    "cost": b.cost,
                    "tokens": b.tokens,
                })
            })
            .collect();
        let value = serde_json::json!({
            "range": out.range,
            "label": out.label,
            "start": iso(out.start),
            "end": iso(out.end),
            "totalCost": out.total_cost,
            "totalTokens": out.total_tokens,
            "models": out.models,
            "buckets": buckets,
        });
        return serde_json::to_string_pretty(&value).map_err(|e| e.to_string());
    }
    Err(format!("Unsupported format \"{format}\" — use json or csv"))
}

/// Write the export next to the user's other downloads and return the path.
pub fn export(out: &HistoryOut, format: &str) -> Result<PathBuf, String> {
    let body = serialize(out, format)?;
    let ext = if format.eq_ignore_ascii_case("csv") {
        "csv"
    } else {
        "json"
    };
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let file =
        paths::downloads_dir().join(format!("keyturn-{}-{stamp}.{ext}", out.range));
    std::fs::write(&file, body).map_err(|e| e.to_string())?;
    Ok(file)
}

pub fn read_entries() -> (Vec<UsageEntry>, Option<String>) {
    usage::read_usage(&paths::open_code_db_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(cost: f64, tokens: f64, t: i64) -> UsageEntry {
        UsageEntry {
            provider: "opencode-go".to_string(),
            model: "glm-5.1".to_string(),
            cost,
            t,
            tokens,
        }
    }

    #[test]
    fn range_specs_cover_the_expected_windows() {
        let now = 1_700_000_000_000;
        let (start, end, bucket, label) = range_spec("5h", 1, now);
        assert_eq!(end, now);
        assert_eq!(now - start, 5 * 3_600_000);
        assert_eq!(bucket, 5 * 60_000);
        assert_eq!(label, "Last 5 hours");
        let (_, _, bucket, _) = range_spec("24h", 1, now);
        assert_eq!(bucket, 30 * 60_000);
        let (_, _, bucket, _) = range_spec("7d", 1, now);
        assert_eq!(bucket, 6 * 3_600_000);
    }

    #[test]
    fn buckets_are_zero_filled_over_the_range() {
        let now = 1_700_000_000_000;
        let entries = vec![
            entry(2.0, 100.0, now - 60_000),
            entry(3.0, 200.0, now - 4 * 3_600_000),
            entry(9.0, 900.0, now - 40 * 3_600_000), // outside 5h
        ];
        let out = build(&entries, "5h", 1, now);
        assert!((out.total_cost - 5.0).abs() < 1e-9);
        assert_eq!(out.buckets.len(), 61);
        assert_eq!(out.buckets.iter().filter(|b| b.cost > 0.0).count(), 2);
    }

    #[test]
    fn csv_and_json_exports_carry_the_totals() {
        let now = 1_700_000_000_000;
        let out = build(&[entry(1.5, 42.0, now - 1_000)], "24h", 1, now);
        let csv = to_csv(&out);
        assert!(csv.starts_with("bucket_start,bucket_end,cost,tokens"));
        assert!(csv.contains("total,,1.500000,42"));
        let json = serialize(&out, "json").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["totalCost"], serde_json::json!(1.5));
        assert_eq!(parsed["totalTokens"], serde_json::json!(42.0));
        assert!(serialize(&out, "xml").is_err());
    }

    #[test]
    fn billing_cycle_starts_on_the_configured_day() {
        // 2026-09-20 local, billing day 10 → cycle started 2026-09-10.
        let now = Local
            .with_ymd_and_hms(2026, 9, 20, 12, 0, 0)
            .unwrap()
            .timestamp_millis();
        let (start, _, _, _) = range_spec("billing", 10, now);
        let expected = Local
            .with_ymd_and_hms(2026, 9, 10, 0, 0, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(start, expected);
    }
}
