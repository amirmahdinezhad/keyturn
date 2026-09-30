//! Cross-module integration tests.
//!
//! These exercise what the inline unit tests cannot: the frozen IPC contract the
//! React frontend depends on, and the public store/profiles/filesystem surface
//! as an outside consumer sees it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use keyturn_lib::cli::ProviderCliStatus;
use keyturn_lib::settings::AppSettings;
use keyturn_lib::{paths, AppState, SettingsOut};
use serde_json::json;

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

/// Point the user-data dir at a temp dir, so tests never touch real data.
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
    AppState {
        settings: SettingsOut {
            open_at_login: true,
        },
        app: AppSettings::default(),
    }
}

#[test]
fn app_state_serializes_to_the_frontend_contract() {
    // No profiles, no theme, no keys: the state is settings + app prefs.
    let value = serde_json::to_value(sample_state()).expect("serialize");
    let obj = value.as_object().expect("AppState is an object");

    // Renaming a field here silently breaks src/types.ts, so pin the exact set.
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = ["settings", "app"];
    expected.sort_unstable();
    assert_eq!(keys, expected, "AppState field names are the IPC contract");

    assert_eq!(obj["settings"], json!({ "openAtLogin": true }));
}

#[test]
fn app_settings_keys_are_camel_case_for_the_ui() {
    let value = serde_json::to_value(AppSettings::default()).expect("serialize");
    let obj = value.as_object().expect("AppSettings is an object");

    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = ["refreshIntervalSecs", "billingDay", "autoUpdateChecks"];
    expected.sort_unstable();
    assert_eq!(keys, expected, "AppSettings field names are the IPC contract");
}

// ---------- CLI status contract (replaces the provider registry) ----------

#[test]
fn cli_status_covers_the_four_supported_clis() {
    let statuses = keyturn_lib::cli::all_status();
    let ids: Vec<&str> = statuses.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["claude", "codex", "opencode", "gemini"]);

    for s in &statuses {
        assert!(!s.label.is_empty(), "{} has no label", s.id);
        assert!(!s.binary.is_empty(), "{} has no binary", s.id);
        assert!(!s.login_cmd.is_empty(), "{} has no login command", s.id);
        assert!(!s.install_url.is_empty(), "{} has no install url", s.id);
    }
}

#[test]
fn cli_status_serializes_to_the_frontend_contract() {
    let status = ProviderCliStatus {
        id: "claude".into(),
        label: "Claude".into(),
        binary: "claude".into(),
        binary_found: true,
        binary_path: Some("/opt/homebrew/bin/claude".into()),
        version: Some("2.1.220".into()),
        logged_in: Some(true),
        auth_method: Some("claude.ai".into()),
        email: Some("dev@example.com".into()),
        account: None,
        detail: Some("Logged in via claude.ai".into()),
        login_cmd: "claude auth login".into(),
        install_url: "https://claude.com/product/claude-code".into(),
        auth_file: None,
        connected: Vec::new(),
    };
    let value = serde_json::to_value(&status).expect("serialize");
    let obj = value.as_object().expect("status is an object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = [
        "id",
        "label",
        "binary",
        "binaryFound",
        "binaryPath",
        "version",
        "loggedIn",
        "authMethod",
        "email",
        "detail",
        "loginCmd",
        "installUrl",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected, "ProviderCliStatus field names are the IPC contract");
    assert_eq!(obj["binaryFound"], json!(true));
    assert_eq!(obj["loginCmd"], json!("claude auth login"));
    // `connected` is skipped when empty so OpenCode-only noise stays out.
    assert!(obj.get("connected").is_none());
    // `None` optionals are skipped the same way.
    assert!(obj.get("account").is_none());
    assert!(obj.get("authFile").is_none());
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

// ---------- store (settings only; keys were removed with CLI-owned auth) ----------

#[test]
fn store_round_trips_settings_and_compacts_legacy_keys() {
    let dir = TempDir::new();
    let file = dir.file();

    keyturn_lib::store::update_store(&file, |s| {
        s.settings.open_at_login = true;
        Ok(())
    })
    .expect("save settings");

    let loaded = keyturn_lib::store::read_store(&file);
    assert!(loaded.settings.open_at_login);

    // A v2 file full of keys comes back slim, then is compacted on disk.
    std::fs::write(
        &file,
        r#"{"providers":{"p":{"keys":{"a":{"key":"k"}}}},"log":[],"settings":{"openAtLogin":false}}"#,
    )
    .unwrap();
    assert!(keyturn_lib::store::compact_legacy(&file));
    let raw = std::fs::read_to_string(&file).unwrap();
    assert!(!raw.contains("providers"));
    assert!(!keyturn_lib::store::compact_legacy(&file));
}

// ---------- settings file ----------

#[test]
fn settings_round_trip_through_the_public_api() {
    let _user_data = isolated_user_data();
    let dir = TempDir::new();
    let file = dir.path("agent-rtl.settings.json");

    let fresh = keyturn_lib::settings::read_settings(&file);
    assert_eq!(fresh.refresh_interval_secs, 60);

    let (saved, ()) = keyturn_lib::settings::update_settings(&file, |s| {
        s.billing_day = 15;
        Ok(())
    })
    .expect("update settings");
    assert_eq!(saved.billing_day, 15);
    assert_eq!(
        keyturn_lib::settings::read_settings(&file).billing_day,
        15
    );
}
