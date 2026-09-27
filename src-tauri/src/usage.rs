//! Local usage DB reading (read-only).
//!
//! Cost/token rows come from the provider CLI's own SQLite database
//! (`opencode.db`). Key attribution, quota windows and month scoping lived
//! here in v2; with CLI-owned auth there are no stored keys to attribute to,
//! so this module only yields raw per-message entries for the history charts.

use rusqlite::{Connection, OpenFlags};
use std::path::Path;

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
        rusqlite::types::Value::Real(f) => f,
        rusqlite::types::Value::Integer(i) => i as f64,
        _ => 0.0,
    }
}

fn value_to_i64(v: rusqlite::types::Value) -> i64 {
    match v {
        rusqlite::types::Value::Integer(i) => i,
        rusqlite::types::Value::Real(f) => f as i64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_usage_from_real_db_file() {
        let dir = std::env::temp_dir().join(format!("keyturn-usage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("opencode.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE message(data TEXT);
             INSERT INTO message VALUES('{\"role\":\"assistant\",\"providerID\":\"p\",\"modelID\":\"m\",\"cost\":1.5,\"time\":{\"created\":1700000000000},\"tokens\":{\"input\":10,\"output\":5}}');
             INSERT INTO message VALUES('{\"role\":\"user\",\"providerID\":\"p\",\"modelID\":\"m\"}');",
        )
        .unwrap();
        drop(conn);
        let (entries, error) = read_usage(&db);
        assert!(error.is_none());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].provider, "p");
        assert!((entries[0].cost - 1.5).abs() < 1e-9);
        assert!((entries[0].tokens - 15.0).abs() < 1e-9);
        std::fs::remove_dir_all(&dir).ok();
    }
}
