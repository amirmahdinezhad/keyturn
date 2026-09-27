//! Per-profile store file.
//!
//! CLI-only model: the store holds nothing but local preferences
//! (`openAtLogin`, theme). Keys, switch logs and signed-in accounts lived
//! here in v2; auth now belongs to the provider CLIs, so those sections are
//! gone. Legacy files are compacted on launch (see `compact_legacy`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::paths;

fn default_theme() -> String {
    "dark".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub open_at_login: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            open_at_login: false,
            theme: default_theme(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Store {
    #[serde(default)]
    pub settings: Settings,
}

pub fn read_store(path: &Path) -> Store {
    normalize(paths::read_json(path))
}

pub fn normalize(raw: Option<Value>) -> Store {
    let mut store = Store::default();
    let obj = match raw {
        Some(Value::Object(map)) => map,
        _ => return store,
    };
    if let Some(Value::Object(s)) = obj.get("settings") {
        store.settings.open_at_login = s
            .get("openAtLogin")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        store.settings.theme = match s.get("theme").and_then(|v| v.as_str()) {
            Some("light") => "light".to_string(),
            _ => "dark".to_string(),
        };
    }
    store
}

/// Drop v2 sections (`providers`, `log`, accounts …) from the file, if any.
/// Returns true when the file was rewritten.
pub fn compact_legacy(path: &Path) -> bool {
    let dirty = match paths::read_json(path) {
        Some(Value::Object(map)) => {
            map.contains_key("providers")
                || map.contains_key("log")
                || map
                    .get("settings")
                    .and_then(|s| s.as_object())
                    .is_some_and(|s| {
                        s.contains_key("accounts")
                            || s.contains_key("active_account")
                            || s.contains_key("custom_providers")
                    })
        }
        _ => false,
    };
    if dirty {
        write_store(path, &read_store(path));
    }
    dirty
}

/// Serializes every read-modify-write of the store file so concurrent
/// commands cannot clobber each other's changes.
static STORE_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

/// Read, mutate and persist the store under one lock. The closure can fail;
/// nothing is written in that case. Returns the saved store plus its value.
pub fn update_store<F, R>(path: &Path, mutate: F) -> Result<(Store, R), String>
where
    F: FnOnce(&mut Store) -> Result<R, String>,
{
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store = read_store(path);
    let out = mutate(&mut store)?;
    let saved = write_store(path, &store);
    Ok((saved, out))
}

/// Fire-and-forget variant for mutations that cannot fail.
pub fn update_store_infallible<F: FnOnce(&mut Store)>(path: &Path, mutate: F) -> Store {
    update_store(path, |store| {
        mutate(store);
        Ok(())
    })
    .map(|(store, ())| store)
    .unwrap_or_else(|_| read_store(path))
}

pub fn write_store(path: &Path, store: &Store) -> Store {
    let value = serde_json::to_value(normalize(Some(
        serde_json::to_value(store).expect("serialize"),
    )))
    .expect("value");
    paths::atomic_write_json(path, &value);
    read_store(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_malformed_and_defaults_settings() {
        // v2 sections are ignored: keys, logs and accounts do not come back.
        let raw: Value = serde_json::json!({
            "providers": { "p": { "keys": { "good": { "key": "k", "type": "api" } } } },
            "log": [{ "t": 1, "provider": "p", "key": "good", "action": "switch" }],
            "settings": { "accounts": [], "custom_providers": ["x"] },
        });
        let value = serde_json::to_value(normalize(Some(raw))).expect("serialize");
        assert_eq!(value, serde_json::json!({ "settings": { "openAtLogin": false, "theme": "dark" } }));

        let lit = normalize(Some(
            serde_json::json!({ "settings": { "openAtLogin": true, "theme": "light" } }),
        ));
        assert!(lit.settings.open_at_login);
        assert_eq!(lit.settings.theme, "light");
    }

    #[test]
    fn legacy_sections_are_compacted_on_request() {
        let dir = std::env::temp_dir().join(format!("keyturn-compact-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("agent-rtl.keys");
        std::fs::write(
            &file,
            r#"{"providers":{"p":{"keys":{"a":{"key":"k"}}}},"log":[],"settings":{"openAtLogin":true}}"#,
        )
        .unwrap();
        assert!(compact_legacy(&file));
        let raw = std::fs::read_to_string(&file).unwrap();
        assert!(!raw.contains("providers"), "legacy keys must be gone");
        assert!(raw.contains("openAtLogin"));
        assert!(!compact_legacy(&file), "second run is a no-op");
        std::fs::remove_dir_all(&dir).ok();
    }
}
