//! Cross-module integration tests.
//!
//! These exercise what the inline unit tests cannot: the frozen IPC contract the
//! React frontend depends on, and the public store/profiles/filesystem surface
//! as an outside consumer sees it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use keyturn_lib::profiles::{self, AppSettings, ProfileRef};
use keyturn_lib::providers::{self, AuthTarget};
use keyturn_lib::store::{self, Cred, ProviderEntry, Store};
use keyturn_lib::usage::{Bucket, KeyQuota, QuotaWindow};
use keyturn_lib::{paths, AppState, Month, ProviderView, SettingsOut, UsageOut};
use serde_json::{json, Value};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A unique directory that removes itself, so tests never touch real app data.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "keyturn-it-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn file(&self) -> PathBuf {
        self.path("agent-rtl.keys")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `profiles::create` materialises the new profile's store inside the user data
/// dir, so point that at a temp dir for the whole test binary.
fn isolated_user_data() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("keyturn-it-userdata-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create user data dir");
        std::env::set_var("KEYTURN_OPENCODE_USERDATA", &dir);
        dir
    })
}

// ---------- the IPC contract ----------

fn sample_state() -> AppState {
    let mut providers = BTreeMap::new();
    providers.insert(
        "opencode".to_string(),
        ProviderView {
            keys: BTreeMap::from([(
                "main".to_string(),
                Cred {
                    cred_type: "api".into(),
                    key: "sk-1".into(),
                    account: None,
                },
            )]),
            active: Some("main".to_string()),
        },
    );

    let mut windows = Vec::new();
    windows.push(QuotaWindow {
        window: "rolling".into(),
        percent: 40.0,
        resets_at: None,
        status: Some("ok".into()),
    });

    AppState {
        providers,
        discovered: vec!["custom".into()],
        applied: BTreeMap::from([("opencode".to_string(), Some("main".to_string()))]),
        usage: UsageOut {
            by_key: BTreeMap::from([(
                "opencode".to_string(),
                BTreeMap::from([("main".to_string(), Bucket::default())]),
            )]),
            untracked: BTreeMap::new(),
            error: None,
        },
        provider_usage: BTreeMap::from([(
            "opencode".to_string(),
            BTreeMap::from([(
                "main".to_string(),
                KeyQuota {
                    windows: Some(windows),
                    error: None,
                },
            )]),
        )]),
        settings: SettingsOut {
            open_at_login: true,
            theme: "dark".into(),
        },
        custom_providers: vec!["custom".into()],
        month: Month { year: 2026, month: 8 },
        profiles: vec![ProfileRef {
            id: "default".into(),
            name: "Default".into(),
        }],
        active_profile: "default".into(),
        app: AppSettings::default(),
    }
}

#[test]
fn app_state_serializes_to_the_frontend_contract() {
    let value = serde_json::to_value(sample_state()).expect("serialize");
    let obj = value.as_object().expect("AppState is an object");

    // Renaming a field here silently breaks src/types.ts, so pin the exact set.
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = [
        "providers",
        "discovered",
        "applied",
        "usage",
        "providerUsage",
        "settings",
        "customProviders",
        "month",
        "profiles",
        "activeProfile",
        "app",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected, "AppState field names are the IPC contract");

    assert_eq!(obj["settings"], json!({ "openAtLogin": true, "theme": "dark" }));
    assert_eq!(obj["month"], json!({ "year": 2026, "month": 8 }));
    assert_eq!(obj["activeProfile"], json!("default"));
    assert_eq!(obj["customProviders"], json!(["custom"]));
    assert_eq!(obj["applied"]["opencode"], json!("main"));
    assert_eq!(obj["profiles"][0], json!({ "id": "default", "name": "Default" }));

    let usage = obj["usage"].as_object().expect("usage object");
    assert!(usage.contains_key("byKey"));
    assert!(usage.contains_key("untracked"));
    assert!(usage.contains_key("error"));

    let quota = &obj["providerUsage"]["opencode"]["main"];
    assert_eq!(quota["windows"][0]["window"], json!("rolling"));
    assert_eq!(quota["windows"][0]["percent"], json!(40.0));
    // The frontend reads `resetsAt`, not `resets_at`.
    assert_eq!(quota["windows"][0]["resetsAt"], Value::Null);
    assert!(quota["windows"][0].get("resets_at").is_none());
    assert_eq!(quota["error"], Value::Null);
}

#[test]
fn app_settings_keys_are_camel_case_for_the_ui() {
    let value = serde_json::to_value(AppSettings::default()).expect("serialize");
    let obj = value.as_object().expect("AppSettings is an object");

    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = [
        "resetDisplay",
        "usagePercentMode",
        "timeFormat",
        "refreshIntervalSecs",
        "autoSwitch",
        "notifyEnabled",
        "notifyThresholds",
        "notifyCustom",
        "soundEnabled",
        "soundName",
        "trayStyle",
        "trayColorMode",
        "trayCustomColor",
        "trayUsedMode",
        "trayShowRemaining",
        "trayShowLabels",
        "trayShowPace",
        "trayMetrics",
        "trayPrimary",
        "trayMulti",
        "trayProfiles",
        "trayActiveIndicator",
        "billingDay",
        "autoUpdateChecks",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected, "AppSettings field names are the IPC contract");
}

// ---------- provider registry ----------

#[test]
fn every_registered_provider_is_well_formed() {
    let specs = providers::all();
    assert!(!specs.is_empty(), "the registry must not be empty");

    let mut seen = BTreeSet::new();
    for spec in specs {
        assert!(seen.insert(spec.id), "duplicate provider id: {}", spec.id);
        assert!(!spec.id.is_empty());
        assert!(!spec.label.is_empty(), "{} has no label", spec.id);

        // The lookup helpers must agree with the spec we just walked.
        assert_eq!(providers::by_id(spec.id).expect("lookup").id, spec.id);
        assert_eq!(providers::usage_api(spec.id).is_some(), spec.usage_api.is_some());

        let caps = providers::capabilities(spec.id);
        assert_eq!(
            caps.token_refresh,
            spec.auth == AuthTarget::Codex,
            "{} token_refresh disagrees with its auth target",
            spec.id
        );
        assert_eq!(
            caps.auth_patch,
            spec.auth != AuthTarget::None,
            "{} auth_patch disagrees with its auth target",
            spec.id
        );

        // Anything the picker offers as connectable needs somewhere to land.
        if spec.connectable {
            assert!(!spec.host.is_empty(), "{} is connectable without a host", spec.id);
            assert!(
                !spec.home_url.is_empty(),
                "{} is connectable without a console home",
                spec.id
            );
        }
    }
}

#[test]
fn unknown_providers_get_the_conservative_default() {
    let caps = providers::capabilities("totally-unknown");
    assert!(!caps.console);
    assert!(!caps.usage_api);
    assert!(!caps.auth_patch);
    assert!(!caps.token_refresh);

    assert!(providers::by_id("totally-unknown").is_none());
    assert!(providers::usage_api("totally-unknown").is_none());
    assert!(providers::console_home("totally-unknown").is_none());
}

// ---------- filesystem helpers ----------

#[cfg(unix)]
#[test]
fn atomic_writes_are_owner_only_and_round_trip() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let file = dir.path("auth.json");
    paths::atomic_write_json(&file, &json!({ "key": "sk-secret" }));

    let mode = fs::metadata(&file).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "credential files must not be world readable");
    assert_eq!(paths::read_json(&file).expect("read back")["key"], json!("sk-secret"));

    // The atomic write must not leave its temp file behind.
    let leftovers: Vec<_> = fs::read_dir(&dir.0)
        .expect("read dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "temp file left behind: {leftovers:?}");
}

// ---------- store ----------

#[test]
fn keys_survive_a_round_trip_and_deleting_the_active_one_clears_it() {
    let dir = TempDir::new();
    let file = dir.file();

    store::update_store(&file, |s| {
        store::upsert_key(s, "opencode", "main", "sk-1", "")?;
        store::upsert_key(s, "opencode", "side", "sk-2", "me@example.com")?;
        store::apply_key(s, "opencode", "main", 1_700_000_000_000, None)
    })
    .expect("save keys");

    let loaded = store::read_store(&file);
    let entry = loaded.providers.get("opencode").expect("provider entry");
    assert_eq!(entry.keys.len(), 2);
    assert_eq!(entry.keys["main"].key, "sk-1");
    assert_eq!(entry.keys["side"].account.as_deref(), Some("me@example.com"));
    assert_eq!(entry.active.as_deref(), Some("main"));

    store::update_store(&file, |s| store::delete_key(s, "opencode", "main")).expect("delete key");

    let after = store::read_store(&file);
    let entry = after.providers.get("opencode").expect("provider entry");
    assert!(!entry.keys.contains_key("main"));
    assert_eq!(
        entry.active.as_deref(),
        Some("side"),
        "deleting the active key must re-point it, not leave it dangling"
    );

    // Removing the last key drops the provider entirely.
    store::update_store(&file, |s| store::delete_key(s, "opencode", "side")).expect("delete last");
    assert!(!store::read_store(&file).providers.contains_key("opencode"));
}

#[test]
fn malformed_key_names_are_rejected_rather_than_written() {
    let mut store = Store::default();
    assert!(store::upsert_key(&mut store, "opencode", "bad/name", "sk-1", "").is_err());
    assert!(store::upsert_key(&mut store, "opencode", "   ", "sk-1", "").is_err());
    assert!(store::upsert_key(&mut store, "opencode", "fine_name-1", "", "").is_err());
    assert!(store::upsert_key(&mut store, "opencode", "fine_name-1", "sk-1", "").is_ok());

    let entry: &ProviderEntry = store.providers.get("opencode").expect("provider entry");
    assert_eq!(entry.keys.len(), 1);
}

// ---------- profiles ----------

#[test]
fn profiles_round_trip_through_the_public_api() {
    let user_data = isolated_user_data();
    let dir = TempDir::new();
    let index = dir.path("agent-rtl.profiles.json");

    let fresh = profiles::read_root(&index);
    assert_eq!(fresh.profiles.len(), 1, "a fresh install has exactly one profile");
    assert_eq!(fresh.active, paths::DEFAULT_PROFILE);

    let (_root, work) = profiles::create(&index, "Work Laptop").expect("create profile");
    assert_eq!(work.id, "work-laptop");
    assert!(
        user_data.join("agent-rtl.keys.work-laptop").exists(),
        "creating a profile must materialise its store"
    );

    let switched = profiles::switch(&index, &work.id).expect("switch");
    assert_eq!(switched.active, work.id);

    let renamed = profiles::rename(&index, &work.id, "Studio").expect("rename");
    assert_eq!(
        renamed.profiles.iter().find(|p| p.id == work.id).expect("profile").name,
        "Studio"
    );

    let deleted = profiles::delete(&index, &work.id).expect("delete");
    assert_eq!(deleted.active, paths::DEFAULT_PROFILE, "active falls back");
    assert!(
        !user_data.join("agent-rtl.keys.work-laptop").exists(),
        "deleting a profile removes its store"
    );

    assert!(
        profiles::delete(&index, paths::DEFAULT_PROFILE).is_err(),
        "the last profile must be protected"
    );

    paths::set_active_profile(paths::DEFAULT_PROFILE);
}
