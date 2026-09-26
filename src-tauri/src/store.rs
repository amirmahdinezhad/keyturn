use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

use crate::paths;

pub const LOG_CAP: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cred {
    #[serde(rename = "type")]
    pub cred_type: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderEntry {
    #[serde(default)]
    pub keys: BTreeMap<String, Cred>,
    pub active: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub t: i64,
    pub provider: String,
    pub key: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub open_at_login: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Signed-in accounts (one per console session; providers can repeat).
    #[serde(default)]
    pub accounts: Vec<AccountRef>,
    /// Account currently shown in the app.
    #[serde(default)]
    pub active_account: Option<String>,
    /// Providers the user added by hand (managed with manual keys).
    #[serde(default)]
    pub custom_providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountRef {
    /// `provider` for the first account, `provider~2`, `provider~3` … after.
    pub id: String,
    pub provider: String,
    #[serde(default)]
    pub label: Option<String>,
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            open_at_login: false,
            theme: default_theme(),
            accounts: Vec::new(),
            active_account: None,
            custom_providers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Store {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderEntry>,
    #[serde(default)]
    pub log: Vec<LogEntry>,
    #[serde(default)]
    pub settings: Settings,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '_' || c == '-')
}

fn valid_account(account: &str) -> bool {
    if account.is_empty() {
        return true;
    }
    if valid_name(account) {
        return true;
    }
    // email: no spaces, one @, dot after @
    if account.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    let mut parts = account.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty() && domain.contains('.')
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
    if let Some(Value::Object(providers)) = obj.get("providers") {
        for (id, entry) in providers {
            let mut keys = BTreeMap::new();
            if let Some(Value::Object(k)) = entry.get("keys") {
                for (name, cred) in k {
                    let key = cred.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    if key.is_empty() {
                        continue;
                    }
                    keys.insert(
                        name.clone(),
                        Cred {
                            cred_type: cred
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("api")
                                .to_string(),
                            key: key.to_string(),
                            account: cred
                                .get("account")
                                .and_then(|v| v.as_str())
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string()),
                        },
                    );
                }
            }
            store.providers.insert(
                id.clone(),
                ProviderEntry {
                    keys,
                    active: entry
                        .get("active")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                },
            );
        }
    }
    if let Some(Value::Array(log)) = obj.get("log") {
        for e in log {
            let (Some(t), Some(provider), Some(key), Some(action)) = (
                e.get("t").and_then(|v| v.as_i64()),
                e.get("provider").and_then(|v| v.as_str()),
                e.get("key").and_then(|v| v.as_str()),
                e.get("action").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            store.log.push(LogEntry {
                t,
                provider: provider.to_string(),
                key: key.to_string(),
                action: action.to_string(),
                prev: e
                    .get("prev")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }
    }
    if let Some(Value::Object(s)) = obj.get("settings") {
        store.settings.open_at_login = s
            .get("openAtLogin")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        store.settings.theme = match s.get("theme").and_then(|v| v.as_str()) {
            Some("light") => "light".to_string(),
            _ => "dark".to_string(),
        };
        if let Some(Value::Array(accounts)) = s.get("accounts") {
            for a in accounts {
                let (Some(id), Some(provider)) = (
                    a.get("id").and_then(|v| v.as_str()),
                    a.get("provider").and_then(|v| v.as_str()),
                ) else {
                    continue;
                };
                store.settings.accounts.push(AccountRef {
                    id: id.to_string(),
                    provider: provider.to_string(),
                    label: a
                        .get("label")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                });
            }
        }
        store.settings.active_account = s
            .get("active_account")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if let Some(Value::Array(custom)) = s.get("custom_providers") {
            for id in custom.iter().filter_map(|v| v.as_str()) {
                store.settings.custom_providers.push(id.to_string());
            }
        }
    }
    store
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

pub fn upsert_key(
    store: &mut Store,
    provider_id: &str,
    name: &str,
    api_key: &str,
    account: &str,
) -> Result<(), String> {
    let trimmed = name.trim();
    if !valid_name(trimmed) {
        return Err("Key name must use letters, digits, spaces, - or _".to_string());
    }
    let key = api_key.trim();
    if key.is_empty() {
        return Err("API key must not be empty".to_string());
    }
    let account_trimmed = account.trim();
    if !valid_account(account_trimmed) {
        return Err("Account must be an email or a short label".to_string());
    }
    let entry = store.providers.entry(provider_id.to_string()).or_default();
    entry.keys.insert(
        trimmed.to_string(),
        Cred {
            cred_type: "api".to_string(),
            key: key.to_string(),
            account: if account_trimmed.is_empty() {
                None
            } else {
                Some(account_trimmed.to_string())
            },
        },
    );
    if entry.active.is_none() {
        entry.active = Some(trimmed.to_string());
    }
    Ok(())
}

pub fn delete_key(store: &mut Store, provider_id: &str, name: &str) -> Result<(), String> {
    let entry = store
        .providers
        .get_mut(provider_id)
        .filter(|e| e.keys.contains_key(name))
        .ok_or_else(|| format!("No key named \"{name}\" for {provider_id}"))?;
    entry.keys.remove(name);
    if entry.active.as_deref() == Some(name) {
        entry.active = entry.keys.keys().next().cloned();
    }
    if entry.keys.is_empty() {
        store.providers.remove(provider_id);
    }
    Ok(())
}

pub fn apply_key(
    store: &mut Store,
    provider_id: &str,
    name: &str,
    now: i64,
    prev: Option<String>,
) -> Result<(), String> {
    if !store
        .providers
        .get(provider_id)
        .is_some_and(|e| e.keys.contains_key(name))
    {
        return Err(format!("No key named \"{name}\" for {provider_id}"));
    }
    let entry = store.providers.get_mut(provider_id).expect("checked");
    entry.active = Some(name.to_string());
    append_log(
        store,
        LogEntry {
            t: now,
            provider: provider_id.to_string(),
            key: name.to_string(),
            action: "switch".to_string(),
            prev,
        },
    );
    Ok(())
}

pub fn append_log(store: &mut Store, entry: LogEntry) {
    store.log.push(entry);
    if store.log.len() > LOG_CAP {
        let excess = store.log.len() - LOG_CAP;
        store.log.drain(..excess);
    }
}

/// Which stored key matches the live credential the CLI reads, per provider.
pub fn compute_applied(
    store: &Store,
    live_keys: &BTreeMap<String, String>,
) -> BTreeMap<String, Option<String>> {
    let mut applied = BTreeMap::new();
    for (provider_id, entry) in &store.providers {
        let live_key = live_keys.get(provider_id);
        applied.insert(
            provider_id.clone(),
            live_key.and_then(|live| {
                entry
                    .keys
                    .iter()
                    .find(|(_, c)| c.key == *live)
                    .map(|(n, _)| n.clone())
            }),
        );
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn empty() -> Store {
        Store::default()
    }

    #[test]
    fn upserts_and_validates() {
        let mut s = empty();
        upsert_key(&mut s, "opencode-go", "personal", "sk-pers", "").unwrap();
        upsert_key(&mut s, "opencode-go", "work", "sk-work", "me@example.com").unwrap();
        assert_eq!(
            s.providers["opencode-go"].active.as_deref(),
            Some("personal")
        );
        assert_eq!(
            s.providers["opencode-go"].keys["work"].account.as_deref(),
            Some("me@example.com")
        );
        assert!(s.providers["opencode-go"].keys["personal"]
            .account
            .is_none());
        assert!(upsert_key(&mut s, "p", "  ", "k", "").is_err());
        assert!(upsert_key(&mut s, "p", "bad/name!", "k", "").is_err());
        assert!(upsert_key(&mut s, "p", "ok", "", "").is_err());
        assert!(upsert_key(&mut s, "p", "ok", "k", "has spaces!").is_err());
    }

    #[test]
    fn delete_repairs_active_and_prunes_provider() {
        let mut s = empty();
        upsert_key(&mut s, "p", "a", "k1", "").unwrap();
        upsert_key(&mut s, "p", "b", "k2", "").unwrap();
        apply_key(&mut s, "p", "b", 5, None).unwrap();
        delete_key(&mut s, "p", "b").unwrap();
        assert_eq!(s.providers["p"].active.as_deref(), Some("a"));
        delete_key(&mut s, "p", "a").unwrap();
        assert!(!s.providers.contains_key("p"));
        assert!(delete_key(&mut s, "p", "nope").is_err());
    }

    #[test]
    fn normalizes_malformed_and_defaults_settings() {
        let raw: Value = serde_json::json!({
            "providers": { "p": { "keys": { "bad": {}, "good": { "key": "k", "type": "api" } }, "active": "gone" } },
            "log": [{ "nope": 1 }],
        });
        let s = normalize(Some(raw));
        assert!(s.providers["p"].keys.contains_key("good"));
        assert!(!s.providers["p"].keys.contains_key("bad"));
        assert!(s.log.is_empty());
        assert!(!s.settings.open_at_login);
        assert_eq!(s.settings.theme, "dark");
        let lit = normalize(Some(
            serde_json::json!({ "settings": { "openAtLogin": true, "theme": "light" } }),
        ));
        assert!(lit.settings.open_at_login);
        assert_eq!(lit.settings.theme, "light");
    }

    #[test]
    fn accounts_and_custom_providers_survive_a_round_trip() {
        let raw = serde_json::json!({
            "providers": {},
            "settings": {
                "openAtLogin": false,
                "theme": "dark",
                "accounts": [
                    {"id": "opencode-go", "provider": "opencode-go", "label": "a@b.c"},
                    {"id": "google", "provider": "google", "label": null}
                ],
                "active_account": "google",
                "custom_providers": ["mistral"]
            }
        });
        let store = normalize(Some(raw));
        assert_eq!(store.settings.accounts.len(), 2);
        assert_eq!(store.settings.accounts[0].label.as_deref(), Some("a@b.c"));
        assert_eq!(store.settings.active_account.as_deref(), Some("google"));
        assert_eq!(store.settings.custom_providers, vec!["mistral".to_string()]);

        // And they survive the serialization write path.
        let value = serde_json::to_value(normalize(Some(
            serde_json::to_value(&store).expect("serialize"),
        )))
        .expect("value");
        let again = normalize(Some(value));
        assert_eq!(again.settings.accounts.len(), 2);
        assert_eq!(again.settings.custom_providers.len(), 1);
    }

    #[test]
    fn compute_applied_matches_live_key_by_value() {
        let mut s = empty();
        upsert_key(&mut s, "p", "a", "k1", "").unwrap();
        upsert_key(&mut s, "p", "b", "k2", "").unwrap();
        let mut live = BTreeMap::new();
        live.insert("p".to_string(), "k2".to_string());
        let applied = compute_applied(&s, &live);
        assert_eq!(applied["p"].as_deref(), Some("b"));

        live.insert("p".to_string(), "gone".to_string());
        assert_eq!(compute_applied(&s, &live)["p"], None);
    }

    #[test]
    fn log_stays_capped() {
        let mut s = empty();
        for i in 0..(LOG_CAP + 50) as i64 {
            append_log(
                &mut s,
                LogEntry {
                    t: i,
                    provider: "p".to_string(),
                    key: "k".to_string(),
                    action: "switch".to_string(),
                    prev: None,
                },
            );
        }
        assert_eq!(s.log.len(), LOG_CAP);
        assert_eq!(s.log.last().unwrap().t, (LOG_CAP + 50) as i64 - 1);
    }
}
