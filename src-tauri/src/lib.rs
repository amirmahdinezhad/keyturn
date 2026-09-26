pub mod account;
pub mod auth;
pub mod bridge;
pub mod codex;
pub mod history;
pub mod notify;
pub mod paths;
pub mod profiles;
pub mod providers;
pub mod store;
pub mod tray;
pub mod usage;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    ActivationPolicy, AppHandle, Manager, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};

use providers::AuthTarget;
use store::{Cred, Store};
use usage::{Bucket, KeyQuota, UsageEntry};

// ---------- IPC shapes (frozen contract with the React frontend) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Month {
    pub year: i32,
    pub month: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOut {
    pub open_at_login: bool,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageOut {
    #[serde(rename = "byKey")]
    pub by_key: BTreeMap<String, BTreeMap<String, Bucket>>,
    pub untracked: BTreeMap<String, Bucket>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppState {
    pub providers: BTreeMap<String, ProviderView>,
    pub discovered: Vec<String>,
    pub applied: BTreeMap<String, Option<String>>,
    pub usage: UsageOut,
    #[serde(rename = "providerUsage")]
    pub provider_usage: BTreeMap<String, BTreeMap<String, KeyQuota>>,
    pub settings: SettingsOut,
    #[serde(rename = "customProviders")]
    pub custom_providers: Vec<String>,
    pub month: Month,
    pub profiles: Vec<profiles::ProfileRef>,
    #[serde(rename = "activeProfile")]
    pub active_profile: String,
    pub app: profiles::AppSettings,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderView {
    pub keys: BTreeMap<String, Cred>,
    pub active: Option<String>,
}

// ---------- helpers ----------

fn current_month() -> Month {
    let (year, month) = usage::current_month();
    Month { year, month }
}

fn normalize_month(month: Option<Month>) -> Month {
    let current = current_month();
    let m = month.unwrap_or_else(|| current.clone());
    if !(0..=11).contains(&m.month) {
        return current;
    }
    if m.year > current.year || (m.year == current.year && m.month > current.month) {
        return current;
    }
    m
}

fn autolaunch_enabled(app: &AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

fn apply_open_at_login(app: &AppHandle, open_at_login: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let _ = if open_at_login {
        manager.enable()
    } else {
        manager.disable()
    };
}

static QUOTA_CACHE: LazyLock<Mutex<HashMap<String, (Instant, KeyQuota)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const QUOTA_TTL: Duration = Duration::from_secs(60);

/// Console snapshots are expensive to rebuild (page walks), so they are kept
/// briefly: long enough to absorb bursts, short enough that the panel shows
/// fresh keys/costs without the user asking for it.
const CONSOLE_TTL: Duration = Duration::from_secs(45);

// The month the frontend is viewing; mutations keep it (mirrors the
// Electron app's lastMonth) so they never yank the user back to "now".
static LAST_MONTH: LazyLock<Mutex<Option<Month>>> = LazyLock::new(|| Mutex::new(None));

fn remember_month(month: &Month) {
    if let Ok(mut m) = LAST_MONTH.lock() {
        *m = Some(month.clone());
    }
}

fn viewed_month() -> Month {
    LAST_MONTH
        .lock()
        .ok()
        .and_then(|m| m.clone())
        .unwrap_or_else(current_month)
}

async fn provider_usage(
    store: &Store,
    force: bool,
) -> BTreeMap<String, BTreeMap<String, KeyQuota>> {
    let mut out: BTreeMap<String, BTreeMap<String, KeyQuota>> = BTreeMap::new();
    let mut jobs = Vec::new();
    for (provider_id, entry) in &store.providers {
        let Some(base) = providers::usage_api(provider_id) else {
            continue;
        };
        for (name, cred) in &entry.keys {
            jobs.push((
                provider_id.clone(),
                name.clone(),
                base.to_string(),
                cred.key.clone(),
            ));
        }
    }
    let results = futures::future::join_all(jobs.into_iter().map(
        |(provider_id, name, base, key)| async move {
            let cache_key = format!("{provider_id}:{name}:{}", key_tail(&key));
            if !force {
                if let Some(hit) = QUOTA_CACHE
                    .lock()
                    .ok()
                    .and_then(|c| c.get(&cache_key).cloned())
                {
                    if hit.0.elapsed() < QUOTA_TTL {
                        return (provider_id, name, hit.1);
                    }
                }
            }
            let data = usage::fetch_provider_usage(&base, &key).await;
            if let Ok(mut c) = QUOTA_CACHE.lock() {
                c.insert(cache_key, (Instant::now(), data.clone()));
            }
            (provider_id, name, data)
        },
    ))
    .await;
    for (provider_id, name, data) in results {
        out.entry(provider_id).or_default().insert(name, data);
    }
    out
}

fn key_tail(key: &str) -> String {
    key.chars().rev().take(8).collect::<String>()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Live credential value per provider, read from whichever CLI file the
/// provider owns. Used for the "Applied" check and for auto-switch bookkeeping.
fn live_credentials(store: &Store) -> BTreeMap<String, String> {
    let mut opencode = auth::read_auth(&paths::open_code_auth_file());
    let mut codex_key: Option<String> = None;
    let mut out = BTreeMap::new();
    for provider in store.providers.keys() {
        match providers::auth_target(provider) {
            AuthTarget::OpenCode => {
                if let Some(key) = opencode
                    .remove(provider)
                    .and_then(|v| v.get("key").and_then(|v| v.as_str()).map(str::to_string))
                {
                    out.insert(provider.clone(), key);
                }
            }
            AuthTarget::Codex => {
                if codex_key.is_none() {
                    codex_key = codex::api_key();
                }
                if let Some(key) = &codex_key {
                    out.insert(provider.clone(), key.clone());
                }
            }
            AuthTarget::None => {}
        }
    }
    out
}

/// Write a key into the file the provider's CLI actually reads.
fn write_credential(provider: &str, cred: &Cred) -> Result<(), String> {
    match providers::auth_target(provider) {
        AuthTarget::OpenCode => {
            let patch = serde_json::json!({ "type": cred.cred_type, "key": cred.key });
            auth::write_auth_patch(&paths::open_code_auth_file(), provider, &patch);
            Ok(())
        }
        AuthTarget::Codex => codex::set_api_key(&cred.key),
        AuthTarget::None => Ok(()),
    }
}

/// `Some("default")` = OS notification sound; `Some(name)` = library file.
fn sound_choice(app: &profiles::AppSettings) -> Option<&str> {
    if !app.sound_enabled {
        return None;
    }
    Some(app.sound_name.as_deref().unwrap_or("default"))
}

fn quota_score(quota: &KeyQuota) -> f64 {
    quota
        .windows
        .as_ref()
        .map(|windows| windows.iter().map(|w| w.percent).fold(0.0, f64::max))
        .unwrap_or(0.0)
}

/// Out of budget: 100% on any window, a non-ok status, or a 429.
fn exhausted(quota: &KeyQuota) -> bool {
    let over = quota.windows.as_ref().is_some_and(|windows| {
        windows.iter().any(|w| {
            w.percent >= 100.0
                || w.status
                    .as_deref()
                    .is_some_and(|s| !s.is_empty() && !s.eq_ignore_ascii_case("ok"))
        })
    });
    let limited = quota.error.as_deref().is_some_and(|e| {
        let e = e.to_ascii_lowercase();
        e.contains("429") || e.contains("rate limit") || e.contains("too many")
    });
    over || limited
}

/// If the active key is out of budget, move to the least-used healthy key.
/// Returns the switches performed (provider, new key, old key).
fn auto_switch(
    app: &AppHandle,
    store: &mut Store,
    root: &profiles::Root,
    usage: &BTreeMap<String, BTreeMap<String, KeyQuota>>,
) -> Vec<(String, String, String)> {
    if !root.app.auto_switch {
        return Vec::new();
    }
    let mut planned: Vec<(String, String, String)> = Vec::new();
    for (provider, keys) in usage {
        let Some(entry) = store.providers.get(provider) else {
            continue;
        };
        let Some(active) = entry.active.clone() else {
            continue;
        };
        let Some(active_quota) = keys.get(&active) else {
            continue;
        };
        if !exhausted(active_quota) {
            continue;
        }
        let candidate = entry
            .keys
            .keys()
            .filter(|name| **name != active)
            .filter_map(|name| keys.get(name).map(|quota| (name.clone(), quota)))
            .filter(|(_, quota)| !exhausted(quota))
            .min_by(|a, b| {
                quota_score(a.1)
                    .partial_cmp(&quota_score(b.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        let Some((next, next_quota)) = candidate else {
            continue;
        };
        if quota_score(next_quota) >= quota_score(active_quota) {
            continue;
        }
        planned.push((provider.clone(), next, active.clone()));
    }
    if planned.is_empty() {
        return planned;
    }
    let path = paths::store_file();
    let at = now_ms();
    let saved = store::update_store(&path, |s| {
        for (provider, next, active) in &planned {
            store::apply_key(s, provider, next, at, Some(active.clone()))?;
        }
        Ok(())
    })
    .map(|(saved, ())| saved)
    .unwrap_or_else(|_| store.clone());
    for (provider, next, _) in &planned {
        if let Some(cred) = saved.providers.get(provider).and_then(|e| e.keys.get(next)) {
            let _ = write_credential(provider, cred);
        }
        let label = providers::by_id(provider)
            .map(|spec| spec.label.to_string())
            .unwrap_or_else(|| provider.clone());
        notify::send(
            app,
            &format!("Switched to \"{next}\""),
            &format!("{label} · the previous key hit its limit"),
            sound_choice(&root.app),
        );
    }
    *store = saved;
    planned
}

/// Last known quotas per profile, so multi-profile tray items keep their
/// numbers after you switch away from them.
static TRAY_QUOTA: LazyLock<Mutex<HashMap<String, BTreeMap<String, BTreeMap<String, KeyQuota>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn remember_tray_quota(profile: &str, usage: &BTreeMap<String, BTreeMap<String, KeyQuota>>) {
    if let Ok(mut cache) = TRAY_QUOTA.lock() {
        cache.insert(profile.to_string(), usage.clone());
    }
}

fn tray_snapshot(
    root: &profiles::Root,
    store: &Store,
    usage: &BTreeMap<String, BTreeMap<String, KeyQuota>>,
    by_key: &BTreeMap<String, BTreeMap<String, Bucket>>,
) -> tray::TraySnapshot {
    let mut profiles_out = Vec::new();
    let wanted: Vec<String> = if root.app.tray_multi && !root.app.tray_profiles.is_empty() {
        root.app.tray_profiles.iter().take(5).cloned().collect()
    } else {
        vec![root.active.clone()]
    };
    for id in wanted {
        let name = root
            .profiles
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| id.clone());
        let (profile_store, profile_usage, profile_buckets) = if id == root.active {
            (store.clone(), usage.clone(), by_key.clone())
        } else {
            (
                store::read_store(&paths::store_file_for(&id)),
                TRAY_QUOTA
                    .lock()
                    .ok()
                    .and_then(|cache| cache.get(&id).cloned())
                    .unwrap_or_default(),
                BTreeMap::new(),
            )
        };
        // Prefer the first provider that reports quota windows.
        let mut chosen: Option<(String, String, KeyQuota)> = None;
        for (provider, entry) in &profile_store.providers {
            let Some(active) = entry.active.clone() else {
                continue;
            };
            if let Some(quota) = profile_usage.get(provider).and_then(|m| m.get(&active)) {
                if quota.windows.is_some() {
                    chosen = Some((provider.clone(), active, quota.clone()));
                    break;
                }
            }
            if chosen.is_none() {
                chosen = Some((
                    provider.clone(),
                    active,
                    KeyQuota {
                        windows: None,
                        error: None,
                    },
                ));
            }
        }
        let mut metric = tray::TrayMetric::default();
        let mut tokens = 0.0;
        let mut cost = 0.0;
        if let Some((provider, key, quota)) = &chosen {
            for window in quota.windows.iter().flatten() {
                match window.window.as_str() {
                    "rolling" => metric.rolling = Some(window.percent),
                    "weekly" => metric.weekly = Some(window.percent),
                    "monthly" => metric.monthly = Some(window.percent),
                    _ => {}
                }
                if window.window == "rolling" {
                    metric.pace =
                        tray::projected(window.percent, window.resets_at.as_deref(), "rolling");
                }
            }
            if let Some(bucket) = profile_buckets.get(provider).and_then(|m| m.get(key)) {
                tokens = bucket.tokens;
                cost = bucket.total;
            }
        }
        profiles_out.push(tray::TrayProfile {
            id,
            label: name,
            active: false,
            metrics: metric,
            tokens,
            cost,
        });
    }
    for profile in profiles_out.iter_mut() {
        profile.active = profile.id == root.active;
    }
    tray::TraySnapshot {
        profiles: profiles_out,
        style: root.app.tray_style.clone(),
        color_mode: root.app.tray_color_mode.clone(),
        custom_color: tray::custom_color(&root.app.tray_custom_color),
        used_mode: root.app.tray_used_mode.clone(),
        show_remaining: root.app.tray_show_remaining,
        show_labels: root.app.tray_show_labels,
        show_pace: root.app.tray_show_pace,
        metrics: root.app.tray_metrics.clone(),
        primary: root.app.tray_primary.clone(),
        active_indicator: root.app.tray_active_indicator,
    }
}

async fn build_state(app: &AppHandle, month: Option<Month>, force: bool) -> AppState {
    build_state_opts(app, month, force, true).await
}

/// `remember` keeps background refreshes from clobbering the month the user is
/// browsing in the panel.
async fn build_state_opts(
    app: &AppHandle,
    month: Option<Month>,
    force: bool,
    remember: bool,
) -> AppState {
    let month = normalize_month(month);
    if remember {
        remember_month(&month);
    }
    let root = profiles::read_root(&paths::profiles_file());
    paths::set_active_profile(&root.active);
    let store_path = paths::store_file();
    let actual = autolaunch_enabled(app);
    let mut store = {
        let current = store::read_store(&store_path);
        if current.settings.open_at_login == actual {
            current
        } else {
            // The OS is the source of truth for launch-at-login.
            store::update_store_infallible(&store_path, |store| {
                store.settings.open_at_login = actual;
            })
        }
    };
    let (entries, usage_error) = read_usage_entries();
    let (start, end) = usage::month_range(month.year, month.month);
    let month_entries: Vec<UsageEntry> = entries
        .into_iter()
        .filter(|e| e.t >= start && e.t < end)
        .collect();
    let (by_key, untracked) = usage::attribute_usage(&month_entries, &store);
    let mut quotas = provider_usage(&store, force).await;

    // Automation only runs against live data: never while browsing history.
    if month.year == current_month().year && month.month == current_month().month {
        let switched = auto_switch(app, &mut store, &root, &quotas);
        if !switched.is_empty() {
            std::thread::sleep(Duration::from_millis(300));
            quotas = provider_usage(&store, true).await;
        }
        for alert in notify::evaluate(&root.active, &root.app, &quotas) {
            notify::send(app, &alert.title, &alert.body, sound_choice(&root.app));
        }
    }

    remember_tray_quota(&root.active, &quotas);
    tray::update(app, &tray_snapshot(&root, &store, &quotas, &by_key));

    let live = live_credentials(&store);
    AppState {
        providers: store
            .providers
            .iter()
            .map(|(id, e)| {
                (
                    id.clone(),
                    ProviderView {
                        keys: e.keys.clone(),
                        active: e.active.clone(),
                    },
                )
            })
            .collect(),
        discovered: auth::provider_ids_from_auth(&paths::open_code_auth_file()),
        applied: store::compute_applied(&store, &live),
        usage: UsageOut {
            by_key,
            untracked,
            error: usage_error,
        },
        provider_usage: quotas,
        settings: SettingsOut {
            open_at_login: store.settings.open_at_login,
            theme: store.settings.theme.clone(),
        },
        custom_providers: store.settings.custom_providers.clone(),
        month,
        profiles: root.profiles.clone(),
        active_profile: root.active.clone(),
        app: root.app.clone(),
    }
}

fn read_usage_entries() -> (Vec<UsageEntry>, Option<String>) {
    usage::read_usage(&paths::open_code_db_file())
}

/// Background loop: keeps the tray, quota cache, alerts and auto-switch fresh
/// even while the panel is closed, and forces a refresh after the machine
/// wakes from sleep (wall clock jumps further than the monotonic clock).
async fn auto_update_check(app: &AppHandle) {
    use tauri_plugin_updater::UpdaterExt;
    let Ok(updater) = app.updater() else {
        return;
    };
    if let Ok(Some(update)) = updater.check().await {
        notify::send(
            app,
            "Update available",
            &format!("Version {} is ready to install.", update.version),
            None,
        );
    }
}

fn spawn_background_refresh(app: AppHandle) {
    static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    std::thread::spawn(move || {
        let mut last_tick = Instant::now();
        let mut last_wall = std::time::SystemTime::now();
        let mut last_update_check = Instant::now();
        loop {
            let interval_secs = profiles::read_root(&paths::profiles_file())
                .app
                .refresh_interval_secs
                .clamp(10, 300) as u64;
            std::thread::sleep(Duration::from_secs(interval_secs));

            let wall_now = std::time::SystemTime::now();
            let monotonic = last_tick.elapsed().as_secs_f64();
            let wall = wall_now
                .duration_since(last_wall)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let woke = wall > monotonic + 20.0;
            last_tick = Instant::now();
            last_wall = wall_now;

            if BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
                continue;
            }
            let _ = tauri::async_runtime::block_on(build_state_opts(
                &app,
                Some(current_month()),
                woke,
                false,
            ));
            if last_update_check.elapsed() > Duration::from_secs(24 * 3600) {
                last_update_check = Instant::now();
                let wanted = profiles::read_root(&paths::profiles_file())
                    .app
                    .auto_update_checks;
                if wanted {
                    tauri::async_runtime::block_on(auto_update_check(&app));
                }
            }
            BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    });
}

// ---------- commands ----------

#[tauri::command]
async fn get_state(
    app: AppHandle,
    month: Option<Month>,
    refresh: Option<bool>,
) -> Result<AppState, String> {
    Ok(build_state(&app, month, refresh.unwrap_or(false)).await)
}

#[tauri::command]
#[allow(non_snake_case)]
async fn save_key(
    app: AppHandle,
    provider: String,
    name: String,
    apiKey: String,
    account: Option<String>,
) -> Result<AppState, String> {
    let path = paths::store_file();
    store::update_store(&path, |store| {
        store::upsert_key(
            store,
            &provider,
            &name,
            &apiKey,
            account.as_deref().unwrap_or(""),
        )
    })?;
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

#[tauri::command]
async fn delete_key(app: AppHandle, provider: String, name: String) -> Result<AppState, String> {
    let path = paths::store_file();
    store::update_store(&path, |store| store::delete_key(store, &provider, &name))?;
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

#[tauri::command]
async fn apply_key(app: AppHandle, provider: String, name: String) -> Result<AppState, String> {
    let store_path = paths::store_file();
    // Remember which stored key was live before this switch.
    let live_key = live_credentials(&store::read_store(&store_path)).remove(&provider);
    let at = now_ms();
    let (saved, ()) = store::update_store(&store_path, |store| {
        let prev = live_key.as_deref().and_then(|live| {
            store
                .providers
                .get(&provider)?
                .keys
                .iter()
                .find(|(_, c)| c.key == live)
                .map(|(n, _)| n.clone())
        });
        store::apply_key(store, &provider, &name, at, prev)
    })?;
    let cred = saved
        .providers
        .get(&provider)
        .and_then(|e| e.keys.get(&name))
        .ok_or_else(|| "Key vanished after save".to_string())?;
    write_credential(&provider, cred)?;
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

#[tauri::command]
async fn set_active_account_cmd(app: AppHandle, account: String) -> Result<AccountStatus, String> {
    let _ = &app;
    set_active_account(&account);
    let store = store::read_store(&paths::store_file());
    Ok(AccountStatus {
        accounts: store.settings.accounts,
        active_account: store.settings.active_account,
        probe: None,
    })
}

#[tauri::command]
async fn add_provider(app: AppHandle, provider: String) -> Result<AppState, String> {
    let id = provider.trim().to_lowercase();
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err("Provider id must be a short slug (letters, digits, - _ .)".to_string());
    }
    if providers::by_id(&id).is_some() {
        return Err(format!("{id} is already built in"));
    }
    let store_path = paths::store_file();
    store::update_store_infallible(&store_path, |store| {
        if !store.settings.custom_providers.iter().any(|p| p == &id) {
            store.settings.custom_providers.push(id);
        }
    });
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

#[tauri::command]
async fn remove_provider(app: AppHandle, provider: String) -> Result<AppState, String> {
    let store_path = paths::store_file();
    store::update_store_infallible(&store_path, |store| {
        store.settings.custom_providers.retain(|p| p != &provider);
    });
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

#[tauri::command]
#[allow(non_snake_case)]
async fn set_settings(app: AppHandle, openAtLogin: bool) -> Result<AppState, String> {
    let path = paths::store_file();
    let mut store = store::read_store(&path);
    store.settings.open_at_login = openAtLogin;
    apply_open_at_login(&app, openAtLogin);
    store::write_store(&path, &store);
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

/// Merge a partial app-settings object (tray, notifications, automation).
#[tauri::command]
async fn set_app_settings(app: AppHandle, settings: serde_json::Value) -> Result<AppState, String> {
    let path = paths::profiles_file();
    profiles::update_root(&path, |root| {
        let mut merged = serde_json::to_value(&root.app).map_err(|e| e.to_string())?;
        if let (Some(dst), Some(src)) = (merged.as_object_mut(), settings.as_object()) {
            for (key, value) in src {
                dst.insert(key.clone(), value.clone());
            }
        }
        root.app = serde_json::from_value(merged).map_err(|e| format!("Invalid settings: {e}"))?;
        Ok(())
    })?;
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

// ---------- profiles ----------

#[tauri::command]
async fn profile_create(app: AppHandle, name: String) -> Result<AppState, String> {
    profiles::create(&paths::profiles_file(), &name)?;
    Ok(build_state(&app, Some(current_month()), false).await)
}

#[tauri::command]
async fn profile_rename(app: AppHandle, id: String, name: String) -> Result<AppState, String> {
    profiles::rename(&paths::profiles_file(), &id, &name)?;
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

#[tauri::command]
async fn profile_switch(app: AppHandle, id: String) -> Result<AppState, String> {
    profiles::switch(&paths::profiles_file(), &id)?;
    if let Ok(mut cache) = CONSOLE_CACHE.lock() {
        *cache = None;
    }
    Ok(build_state(&app, Some(current_month()), false).await)
}

#[tauri::command]
async fn profile_delete(app: AppHandle, id: String) -> Result<AppState, String> {
    let root = profiles::delete(&paths::profiles_file(), &id)?;
    paths::set_active_profile(&root.active);
    Ok(build_state(&app, Some(current_month()), false).await)
}

// ---------- alerts, codex, history ----------

#[tauri::command]
fn list_sounds() -> Vec<String> {
    notify::list_sounds()
}

#[tauri::command]
fn play_sound(name: String) -> Result<(), String> {
    notify::play(&name)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexOut {
    status: codex::CodexStatus,
    /// Stored key names we hold for the codex provider.
    keys: Vec<String>,
}

#[tauri::command]
fn codex_status() -> CodexOut {
    let store = store::read_store(&paths::store_file());
    CodexOut {
        status: codex::status(),
        keys: store
            .providers
            .get("codex")
            .map(|entry| entry.keys.keys().cloned().collect())
            .unwrap_or_default(),
    }
}

#[tauri::command]
fn codex_refresh(app: AppHandle) -> Result<CodexOut, String> {
    let status = codex::refresh_tokens()?;
    let _ = app;
    let store = store::read_store(&paths::store_file());
    Ok(CodexOut {
        status,
        keys: store
            .providers
            .get("codex")
            .map(|entry| entry.keys.keys().cloned().collect())
            .unwrap_or_default(),
    })
}

#[tauri::command]
async fn history(range: String, key: Option<String>) -> Result<history::HistoryOut, String> {
    let root = profiles::read_root(&paths::profiles_file());
    let store = store::read_store(&paths::store_file());
    let (entries, error) = history::read_entries();
    let mut out = history::build(
        &entries,
        &store,
        &range,
        key.as_deref(),
        root.app.billing_day,
        now_ms(),
    );
    out.error = error;
    Ok(out)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateOut {
    available: bool,
    version: Option<String>,
    notes: Option<String>,
    error: Option<String>,
}

/// Manual update check. The updater plugin is configured in tauri.conf.json;
/// when no signing key is set yet this returns a readable error instead of
/// failing the call.
#[tauri::command]
async fn check_updates(app: AppHandle) -> Result<UpdateOut, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(e) => {
            return Ok(UpdateOut {
                available: false,
                version: None,
                notes: None,
                error: Some(e.to_string()),
            })
        }
    };
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateOut {
            available: true,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
            error: None,
        }),
        Ok(None) => Ok(UpdateOut {
            available: false,
            version: None,
            notes: None,
            error: None,
        }),
        Err(e) => Ok(UpdateOut {
            available: false,
            version: None,
            notes: None,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
async fn export_history(
    range: String,
    format: String,
    key: Option<String>,
) -> Result<String, String> {
    let root = profiles::read_root(&paths::profiles_file());
    let store = store::read_store(&paths::store_file());
    let (entries, _) = history::read_entries();
    let out = history::build(
        &entries,
        &store,
        &range,
        key.as_deref(),
        root.app.billing_day,
        now_ms(),
    );
    let path = history::export(&out, &format)?;
    Ok(path.to_string_lossy().into_owned())
}

// Last console payload (keys + per-key/model rows) so the UI can list the
// account's keys and show exact cost without refetching on every render.
static CONSOLE_CACHE: LazyLock<Mutex<Option<(String, Month, Instant, account::ConsoleUsage)>>> =
    LazyLock::new(|| Mutex::new(None));

/// One console sync at a time: the webview is shared state.
static CONSOLE_SYNC: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Answers from the remote console page, keyed by the request marker.
static CONSOLE_REPLIES: LazyLock<Mutex<std::collections::HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Called from inside the console page (allowed by capabilities/console.json).
#[tauri::command]
fn console_payload(marker: String, payload: String) {
    if let Ok(mut map) = CONSOLE_REPLIES.lock() {
        map.insert(marker, payload);
    }
}

fn cache_console(provider: &str, month: &Month, usage: &account::ConsoleUsage) {
    if let Ok(mut cache) = CONSOLE_CACHE.lock() {
        *cache = Some((
            provider.to_string(),
            month.clone(),
            Instant::now(),
            usage.clone(),
        ));
    }
}

fn cached_console(provider: &str, month: &Month, force: bool) -> Option<account::ConsoleUsage> {
    let cache = CONSOLE_CACHE.lock().ok()?;
    let (cached_provider, cached, at, usage) = cache.as_ref()?;
    if cached_provider == provider && cached.year == month.year && cached.month == month.month {
        if force || at.elapsed() < CONSOLE_TTL {
            return Some(usage.clone());
        }
    }
    None
}

#[derive(serde::Serialize)]
struct ConsoleKeysOut {
    keys: Vec<account::ConsoleKey>,
    month: Month,
    email: Option<String>,
}

/// The provider an account id belongs to (`anthropic~2` → `anthropic`).
fn account_provider(account_id: &str) -> String {
    account_id
        .split('~')
        .next()
        .unwrap_or(account_id)
        .to_string()
}

/// Labels under which the webview/keychain state for an account lives. The
/// very first OpenCode account in the default profile keeps the legacy names
/// so its existing session (and the user's saved sign-in) survives; every
/// other profile gets its own cookie jar so console accounts stay isolated.
fn account_window_key(account_id: &str) -> String {
    let profile = paths::active_profile();
    if profile == paths::DEFAULT_PROFILE && account_id == "opencode-go" {
        "console".to_string()
    } else if profile == paths::DEFAULT_PROFILE {
        format!("console-{account_id}")
    } else {
        format!("console-{profile}-{account_id}")
    }
}

fn account_data_dir(account_id: &str) -> String {
    let profile = paths::active_profile();
    if profile == paths::DEFAULT_PROFILE && account_id == "opencode-go" {
        "console-webview".to_string()
    } else if profile == paths::DEFAULT_PROFILE {
        format!("console-webview-{account_id}")
    } else {
        format!("console-webview-{profile}-{account_id}")
    }
}

/// Next free account id for a provider: `p`, `p~2`, `p~3` …
fn next_account_id(store: &store::Store, provider: &str) -> String {
    if !store
        .settings
        .accounts
        .iter()
        .any(|a| a.provider == provider)
    {
        return provider.to_string();
    }
    for n in 2..100 {
        let candidate = format!("{provider}~{n}");
        if !store.settings.accounts.iter().any(|a| a.id == candidate) {
            return candidate;
        }
    }
    format!("{provider}~{}-", store.settings.accounts.len())
}

/// The console authenticates with a client-side session that never shows up
/// in readable cookies, so the only reliable way in is to ask the site from
/// inside our own authenticated webview and read the answer back. The result
/// travels through `document.title` (no extra IPC/capability needed).
fn console_window_for(
    app: &AppHandle,
    spec: &providers::ProviderSpec,
    account_id: &str,
) -> Result<tauri::WebviewWindow, String> {
    let label = account_window_key(account_id);
    if let Some(win) = app.get_webview_window(&label) {
        return Ok(win);
    }
    let dir_name = account_data_dir(account_id);
    let url: tauri::Url = spec.home_url.parse().map_err(|e| format!("bad url: {e}"))?;
    let data_dir = app.path().app_data_dir().map(|dir| dir.join(dir_name)).ok();
    let mut builder =
        tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::External(url))
            .title(format!("console ({})", spec.label))
            .inner_size(920.0, 720.0)
            .visible(false)
            .initialization_script(bridge::init_script());
    if let Some(dir) = data_dir {
        builder = builder.data_directory(dir);
    }
    builder.build().map_err(|e| e.to_string())
}

fn window_url(win: &tauri::WebviewWindow) -> String {
    win.url()
        .map(|u| {
            format!(
                "{}://{}{}",
                u.scheme(),
                u.host_str().unwrap_or(""),
                u.path()
            )
        })
        .unwrap_or_default()
}

/// Ask the signed-in page for a server-function result.
fn call_via_window(
    win: &tauri::WebviewWindow,
    id: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    // POST first; the console's query functions also answer a GET with the id
    // in the query string, which some deployments expect.
    match call_via_window_mode(win, "p", id, args.clone()) {
        Ok(value) => Ok(value),
        Err(first) => {
            account::log_line(&format!("post failed ({first}); retrying with GET"));
            call_via_window_mode(win, "g", id, args)
        }
    }
}

fn call_via_window_mode(
    win: &tauri::WebviewWindow,
    mode: &str,
    id: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let base = win
        .url()
        .ok()
        .map(|u| {
            let mut b = u.clone();
            b.set_fragment(None);
            b.to_string()
        })
        .unwrap_or_else(|| window_url(win));
    let target = format!("{base}{}", bridge::request_hash_mode(mode, id, &args));
    let parsed: tauri::Url = target.parse().map_err(|e| format!("bad url: {e}"))?;
    win.navigate(parsed).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(600));
    if let Ok(after) = win.url() {
        account::log_line(&format!("issued request, page now at {after}"));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    while std::time::Instant::now() < deadline {
        if let Ok(current) = win.url() {
            let cur = current.as_str().to_string();
            if cur.contains("#kring-alive") {
                account::log_line("init script is alive on the page");
                // clear the ping so real replies are unambiguous
                if let Ok(mut nofrag) = win.url() {
                    nofrag.set_fragment(None);
                    let _ = win.navigate(nofrag);
                }
            }
            if let Some(reply) = bridge::parse_reply(current.as_str()) {
                account::log_line(&format!(
                    "hash fn {} -> {} {}",
                    id.get(..8).unwrap_or(id),
                    reply.status,
                    reply.body.chars().take(120).collect::<String>()
                ));
                if reply.status != "200" {
                    return Err(format!(
                        "console answered {}: {}",
                        reply.status,
                        reply.body.chars().take(140).collect::<String>()
                    ));
                }
                let mut value: serde_json::Value = serde_json::from_str(&reply.body)
                    .map_err(|e| format!("invalid console JSON: {e}"))?;
                if let Some(inner) = value.get("result").cloned() {
                    value = inner;
                }
                return Ok(value);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    account::log_line(&format!(
        "hash timeout on fn {} (page at {})",
        id.get(..8).unwrap_or(id),
        window_url(win)
    ));
    Err("console call timed out".to_string())
}

/// Make sure the provider's console window is signed in, showing it while the
/// user logs in.
fn ensure_signed_in(app: &AppHandle, account_id: &str) -> Result<tauri::WebviewWindow, String> {
    let provider = account_provider(account_id);
    let spec =
        providers::by_id(&provider).ok_or_else(|| format!("unknown provider: {provider}"))?;
    let win = console_window_for(app, spec, account_id).map_err(|e| {
        account::log_line(&format!("console window failed ({account_id}): {e}"));
        e
    })?;
    // Already on a signed-in page: nothing to do. Re-navigating to the login
    // page would flash the window on every background sync.
    if providers::is_signed_in(&provider, &window_url(&win)) {
        let _ = win.hide();
        return Ok(win);
    }
    account::log_line(&format!(
        "console window ready, waiting for {account_id} sign-in"
    ));

    if let Ok(login) = spec.login_url.parse::<tauri::Url>() {
        let _ = win.navigate(login);
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    let mut last_url = String::new();
    let mut shown = false;
    let mut waiting_since = std::time::Instant::now();
    let mut tick: u64 = 0;
    loop {
        let url = window_url(&win);
        if url != last_url {
            account::log_line(&format!("console at {url}"));
            last_url = url.clone();
            waiting_since = std::time::Instant::now();
        }
        if providers::is_signed_in(&provider, &url) {
            let _ = win.hide();
            return Ok(win);
        }
        if !shown {
            let waited = waiting_since.elapsed();
            let on_login = url.contains("/login") || url.contains("/auth");
            let off_host = !url.is_empty() && !url.contains(spec.host);
            // A live session bounces the login page back in about a second, so
            // give it a moment before surfacing the window; SSO hops need the
            // user right away.
            if off_host
                || (on_login && waited > std::time::Duration::from_secs(2))
                || waited > std::time::Duration::from_secs(12)
            {
                shown = true;
                let _ = win.show();
                let _ = win.set_focus();
                account::log_line("console window shown for sign-in");
            }
        }
        if std::time::Instant::now() > deadline {
            account::log_line(&format!("sign-in wait timed out at {url}"));
            return Err("Sign-in was not completed".to_string());
        }
        if tick % 10 == 0 {
            account::log_line(&format!(
                "still waiting at {url} (visible: {})",
                win.is_visible().unwrap_or(false)
            ));
        }
        tick += 1;
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Scrape once, and if every selector came back empty (slow render),
/// wait and try one more time before giving up.
fn scrape_resilient(
    win: &tauri::WebviewWindow,
    selectors: &[&str],
) -> Result<Vec<Vec<String>>, String> {
    let first = scrape_page(win, selectors)?;
    if first.iter().all(|v| v.is_empty()) {
        std::thread::sleep(std::time::Duration::from_millis(3000));
        return scrape_page(win, selectors);
    }
    Ok(first)
}

/// Ask the page (via #kring-scrape) for innerText lists, one per selector.
fn scrape_page(win: &tauri::WebviewWindow, selectors: &[&str]) -> Result<Vec<Vec<String>>, String> {
    use base64::Engine;
    let spec = serde_json::to_string(selectors).map_err(|e| e.to_string())?;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(spec.as_bytes());
    let base = win
        .url()
        .ok()
        .map(|u| {
            let mut b = u.clone();
            b.set_fragment(None);
            b.to_string()
        })
        .unwrap_or_else(|| window_url(win));
    let target = format!("{base}#kring-scrape={encoded}");
    let parsed: tauri::Url = target.parse().map_err(|e| format!("bad url: {e}"))?;
    win.navigate(parsed).map_err(|e| e.to_string())?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    while std::time::Instant::now() < deadline {
        if let Ok(current) = win.url() {
            let cur = current.as_str().to_string();
            if let Some(rest) = cur
                .split('#')
                .nth(1)
                .and_then(|h| h.strip_prefix("kring-res="))
            {
                let decoded: Vec<u8> = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(rest)
                    .or_else(|_| base64::engine::general_purpose::STANDARD.decode(rest))
                    .map_err(|e| format!("bad scrape reply: {e}"))?;
                let text = String::from_utf8(decoded).map_err(|e| e.to_string())?;
                let value: serde_json::Value =
                    serde_json::from_str(&text).map_err(|e| format!("bad scrape JSON: {e}"))?;
                if let Some(err) = value.get("scrapeError").and_then(|v| v.as_str()) {
                    return Err(format!("page scrape failed: {err}"));
                }
                let scraped = value
                    .get("scraped")
                    .and_then(|v| v.as_object())
                    .ok_or("unexpected scrape shape")?;
                let mut out = Vec::new();
                for i in 0..selectors.len() {
                    let texts: Vec<String> = scraped
                        .get(&i.to_string())
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push(texts);
                }
                account::log_line(&format!(
                    "scraped {} selector(s): {:?}",
                    selectors.len(),
                    out.iter().map(|v| v.len()).collect::<Vec<_>>()
                ));
                return Ok(out);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    Err("page scrape timed out".to_string())
}

/// Usage cells look like `Go ($0.0010)` — pull the amount out of the
/// parentheses when present, otherwise parse the bare number.
fn parse_cost(text: &str) -> Option<f64> {
    let inner = text
        .rsplit_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(value, _)| value)
        .unwrap_or(text);
    let cleaned: String = inner
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

/// Log the page's own same-origin RPC traffic: live server-function ids,
/// argument bodies and response shapes. These change on site redeploys, so
/// hardcoded ids/args are only a fallback.
fn log_fetch_traffic(win: &tauri::WebviewWindow, label: &str) {
    let Ok(entries) = scrape_page(win, &["@fetch-log"]) else {
        return;
    };
    let Some(list) = entries.first() else {
        return;
    };
    let mut seen = std::collections::HashSet::new();
    let mut compact = 0;
    for entry in list {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(entry) else {
            continue;
        };
        let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let body = value.get("body").and_then(|v| v.as_str()).unwrap_or("");
        if !seen.insert(format!("{id}|{url}|{body}")) {
            continue;
        }
        let resj = value.get("resj").and_then(|v| v.as_str()).unwrap_or("");
        let res = if !resj.is_empty() {
            resj
        } else {
            value.get("res").and_then(|v| v.as_str()).unwrap_or("")
        };
        let interesting = ["keyID", "keyId", "key_id", "cost", "model", "usage"]
            .iter()
            .any(|needle| res.contains(needle));
        if interesting {
            account::log_line(&format!(
                "[{label}] rpc {} status={} id={} body={} -> {}",
                url.chars().take(100).collect::<String>(),
                value.get("status").and_then(|v| v.as_u64()).unwrap_or(0),
                id.get(..20).unwrap_or(id),
                body.chars().take(160).collect::<String>(),
                res.chars().take(3000).collect::<String>()
            ));
        } else if compact < 8 {
            compact += 1;
            account::log_line(&format!(
                "[{label}] net {} status={} id={} body={} res={}",
                url.chars().take(100).collect::<String>(),
                value.get("status").and_then(|v| v.as_u64()).unwrap_or(0),
                id.get(..20).unwrap_or(id),
                body.chars().take(60).collect::<String>(),
                res.chars().take(80).collect::<String>()
            ));
        }
    }
}

/// Find the first array of usage rows (model + keyId + totalCost) in a value.
fn find_usage_array(value: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
    if let Some(usage) = value.get("usage").and_then(|v| v.as_array()) {
        if usage.iter().any(|row| row.get("keyId").is_some()) {
            return Some(usage.clone());
        }
    }
    match value {
        serde_json::Value::Array(items) => {
            if items.iter().any(|row| row.get("keyId").is_some()) {
                return Some(items.clone());
            }
            items.iter().find_map(find_usage_array)
        }
        serde_json::Value::Object(map) => map.values().find_map(find_usage_array),
        _ => None,
    }
}

/// The month a captured usage request was made for, from its serialized args.
fn captured_month(entry: &serde_json::Value) -> Option<Month> {
    let body: serde_json::Value = serde_json::from_str(entry.get("body")?.as_str()?).ok()?;
    let args = body.pointer("/t/a")?.as_array()?;
    let numbers: Vec<i64> = args
        .iter()
        .filter_map(|arg| arg.get("s").and_then(|s| s.as_i64()))
        .collect();
    (numbers.len() >= 2).then(|| Month {
        year: numbers[0] as i32,
        month: numbers[1] as u32,
    })
}

/// The page's own usage RPC payload (real key ids per row, full month).
fn usage_rows_from_payload(payload: &serde_json::Value) -> Vec<account::ConsoleRow> {
    let Some(items) = find_usage_array(payload) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let model = item.get("model")?.as_str()?.to_string();
            if model.is_empty() {
                return None;
            }
            Some(account::ConsoleRow {
                model,
                key_id: item
                    .get("keyId")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                date: item
                    .get("date")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                // totalCost is stored in 1e-8 dollars.
                cost: item
                    .get("totalCost")
                    .and_then(|v| v.as_f64())
                    .map(|cost| cost / 1e8),
            })
        })
        .collect()
}

fn captured_usage(win: &tauri::WebviewWindow) -> Option<(Month, Vec<account::ConsoleRow>)> {
    let entries = scrape_page(win, &["@fetch-log"]).ok()?;
    for entry in entries.first()? {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(entry) else {
            continue;
        };
        let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if !url.contains("_server") || value.get("id").and_then(|v| v.as_str()).is_none() {
            continue;
        }
        let resj = value.get("resj").and_then(|v| v.as_str()).unwrap_or("");
        if resj.is_empty() || !resj.contains("keyId") {
            continue;
        }
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(resj) else {
            continue;
        };
        let mut rows = usage_rows_from_payload(&payload);
        let captured_for = captured_month(&value).unwrap_or_else(|| normalize_month(None));
        let prefix = format!("{:04}-{:02}", captured_for.year, captured_for.month + 1);
        rows.retain(|row| {
            row.date
                .as_deref()
                .is_none_or(|date| date.starts_with(&prefix))
        });
        if rows.is_empty() {
            continue;
        }
        return Some((captured_for, rows));
    }
    None
}

/// The live usage RPC (function id + serialized args) as sent by the page.
fn captured_usage_request(win: &tauri::WebviewWindow) -> Option<(String, serde_json::Value)> {
    let entries = scrape_page(win, &["@fetch-log"]).ok()?;
    for entry in entries.first()? {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(entry) else {
            continue;
        };
        let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if !url.contains("_server") {
            continue;
        }
        let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let resj = value.get("resj").and_then(|v| v.as_str()).unwrap_or("");
        if !resj.contains("keyId") {
            continue;
        }
        let Some(body) = value.get("body").and_then(|v| v.as_str()) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body) else {
            continue;
        };
        if parsed.pointer("/t/a").and_then(|v| v.as_array()).is_none() {
            continue;
        }
        return Some((id.to_string(), parsed));
    }
    None
}

/// Replay the page's own usage RPC with the month args patched.
fn replay_usage_month(
    win: &tauri::WebviewWindow,
    id: &str,
    body: &serde_json::Value,
    month: &Month,
) -> Option<Vec<account::ConsoleRow>> {
    let mut patched = body.clone();
    let args = patched.pointer_mut("/t/a")?.as_array_mut()?;
    let mut numbers = args
        .iter_mut()
        .filter(|arg| arg.get("s").is_some_and(|s| s.is_number()));
    if let Some(year) = numbers.next() {
        year["s"] = serde_json::json!(month.year);
    }
    if let Some(index) = numbers.next() {
        index["s"] = serde_json::json!(month.month);
    }
    let value = call_via_window(win, id, patched).ok()?;
    let mut rows = usage_rows_from_payload(&value);
    let prefix = format!("{:04}-{:02}", month.year, month.month + 1);
    rows.retain(|row| {
        row.date
            .as_deref()
            .is_none_or(|date| date.starts_with(&prefix))
    });
    Some(rows)
}

/// The workspace base URL for the page the window is currently on.
fn workspace_base(win: &tauri::WebviewWindow) -> Option<String> {
    win.url().ok().and_then(|u| {
        let mut clean = u.clone();
        clean.set_fragment(None);
        let s = clean.as_str().to_string();
        s.split("/workspace/")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .filter(|id| !id.is_empty())
            .map(|id| format!("https://opencode.ai/workspace/{id}"))
    })
}

/// Per-key/per-model usage for a month, read from the rendered pages.
/// The page does the auth + data fetching; we only scrape text, so no server
/// ids, no Seroval and no auth headers are involved on our side.
fn parse_go_windows(go: &[Vec<String>]) -> Vec<account::GoWindow> {
    let labels = go.first().cloned().unwrap_or_default();
    let values = go.get(1).cloned().unwrap_or_default();
    let resets = go.get(2).cloned().unwrap_or_default();
    let mut windows = Vec::new();
    for i in 0..labels.len() {
        windows.push(account::GoWindow {
            label: labels.get(i).cloned().unwrap_or_default(),
            percent: values
                .get(i)
                .and_then(|t| t.replace('%', "").trim().parse::<f64>().ok())
                .unwrap_or(0.0),
            reset: resets.get(i).cloned().unwrap_or_default(),
        });
    }
    windows
}

/// Poll a scrape until at least one selector returns data: the pages hydrate
/// progressively, and fixed sleeps either waste time or miss slow renders.
fn scrape_until(
    win: &tauri::WebviewWindow,
    selectors: &[&str],
    timeout: std::time::Duration,
) -> Option<Vec<Vec<String>>> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Ok(out) = scrape_page(win, selectors) {
            if out.iter().any(|list| !list.is_empty()) {
                return Some(out);
            }
        }
        if std::time::Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
}

/// The site fetches the month's usage itself right after the page opens; wait
/// for that payload (or replay it for another month) instead of scraping the
/// rendered table.
fn poll_usage_rows(
    win: &tauri::WebviewWindow,
    month: &Month,
    timeout: std::time::Duration,
) -> Option<Vec<account::ConsoleRow>> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some((captured_for, rows)) = captured_usage(win) {
            if captured_for.year == month.year && captured_for.month == month.month {
                return Some(rows);
            }
            if let Some((id, body)) = captured_usage_request(win) {
                if let Some(rows) = replay_usage_month(win, &id, &body, month) {
                    return Some(rows);
                }
            }
        }
        if std::time::Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn keys_from_payload(win: &tauri::WebviewWindow) -> Vec<account::ConsoleKey> {
    let Ok(found) = scrape_page(win, &["@keys-json"]) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for text in found.first().cloned().unwrap_or_default() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(id) = value.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
        keys.push(account::ConsoleKey {
            id: id.to_string(),
            name: Some(if name.is_empty() {
                id.to_string()
            } else {
                name.to_string()
            }),
            prefix: value
                .get("display")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            secret: value
                .get("secret")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            email: value
                .get("email")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        });
    }
    keys
}

fn keys_from_table(win: &tauri::WebviewWindow) -> Vec<account::ConsoleKey> {
    let Some(cells) = scrape_until(
        win,
        &[
            "[data-slot=key-name]",
            "[data-slot=key-user-email]",
            "[data-slot=key-value]",
        ],
        std::time::Duration::from_secs(6),
    ) else {
        return Vec::new();
    };
    let count = cells.first().map(|v| v.len()).unwrap_or(0);
    let mut keys = Vec::new();
    for i in 0..count {
        let name = cells
            .first()
            .and_then(|v| v.get(i))
            .cloned()
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        keys.push(account::ConsoleKey {
            id: name.clone(),
            name: Some(name),
            prefix: None,
            secret: cells
                .get(2)
                .and_then(|v| v.get(i))
                .filter(|v| v.starts_with("sk-"))
                .cloned(),
            email: cells.get(1).and_then(|v| v.get(i)).cloned(),
        });
    }
    keys
}

/// Per-key/per-model usage for a month, read from the rendered pages.
/// The page does the auth + data fetching; we only scrape text, so no server
/// ids, no Seroval and no auth headers are involved on our side.
fn sync_from_pages(
    app: &AppHandle,
    account_id: &str,
    month: &Month,
) -> Result<account::ConsoleUsage, String> {
    let win = ensure_signed_in(app, account_id)?;
    let base = workspace_base(&win).ok_or("could not determine the workspace route")?;
    account::log_line(&format!("syncing from {base}"));
    let started = std::time::Instant::now();

    // 1) Go plan windows from the /go/ panel.
    let go_url: tauri::Url = format!("{base}/go/")
        .parse()
        .map_err(|e| format!("bad url: {e}"))?;
    win.navigate(go_url).map_err(|e| e.to_string())?;
    let go_windows = scrape_until(
        &win,
        &[
            "[data-slot=usage-label]",
            "[data-slot=usage-value]",
            "[data-slot=reset-time]",
        ],
        std::time::Duration::from_secs(8),
    )
    .map(|go| parse_go_windows(&go))
    .unwrap_or_default();

    // 2) Keys (ids + secrets) from the keys page payload.
    let keys_url: tauri::Url = format!("{base}/keys/")
        .parse()
        .map_err(|e| format!("bad url: {e}"))?;
    win.navigate(keys_url).map_err(|e| e.to_string())?;
    let keys = {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            let keys = keys_from_payload(&win);
            if !keys.is_empty() {
                break keys;
            }
            if std::time::Instant::now() > deadline {
                break keys_from_table(&win);
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    };
    account::log_line(&format!("keys: {}", keys.len()));

    // 3) Usage rows from the page's own RPC payload.
    let usage_url: tauri::Url = format!("{base}/usage/")
        .parse()
        .map_err(|e| format!("bad url: {e}"))?;
    win.navigate(usage_url).map_err(|e| e.to_string())?;
    let mut usage_rows = Vec::new();
    if let Some(rows) = poll_usage_rows(&win, month, std::time::Duration::from_secs(10)) {
        account::log_line(&format!(
            "usage rows: {} ({} keys seen)",
            rows.len(),
            rows.iter()
                .filter_map(|r| r.key_id.as_deref())
                .collect::<std::collections::HashSet<_>>()
                .len()
        ));
        usage_rows = rows;
    } else {
        // Fallback: scrape the rendered table (session-level rows).
        account::log_line("usage RPC unavailable, scraping the table");
        let cells = scrape_resilient(
            &win,
            &[
                "[data-slot=usage-date]",
                "[data-slot=usage-model]",
                "[data-slot=usage-cost]",
                "[data-slot=usage-session]",
            ],
        )?;
        for i in 0..cells[0].len().max(cells[1].len()) {
            let cost = cells
                .get(2)
                .and_then(|v| v.get(i))
                .and_then(|t| parse_cost(t));
            let model = cells
                .get(1)
                .and_then(|v| v.get(i))
                .cloned()
                .unwrap_or_default();
            if model.is_empty() {
                continue;
            }
            usage_rows.push(account::ConsoleRow {
                model,
                key_id: cells.get(3).and_then(|v| v.get(i)).cloned(),
                date: cells.first().and_then(|v| v.get(i)).cloned(),
                cost,
            });
        }
    }
    account::log_line(&format!(
        "sync finished in {:.1}s",
        started.elapsed().as_secs_f32()
    ));
    Ok(account::ConsoleUsage {
        keys,
        usage: usage_rows,
        go: go_windows,
    })
}

fn console_snapshot_windows(
    app: &AppHandle,
    account_id: &str,
    month: &Month,
    force: bool,
) -> Result<account::ConsoleUsage, String> {
    let _guard = CONSOLE_SYNC.lock();
    if let Some(hit) = cached_console(account_id, month, force) {
        return Ok(hit);
    }
    let provider = account_provider(account_id);
    if provider != "opencode-go" {
        let usage = probe_provider(app, account_id)?;
        cache_console(account_id, month, &usage);
        return Ok(usage);
    }
    // Keys are month-independent: reuse them and only refresh the live quota
    // windows plus the usage RPC for the requested month. A forced refresh
    // always walks the pages so stale keys/windows drop out.
    if !force {
        if let Some(previous) = any_cached_console(account_id) {
            if let Some(rows) = fetch_usage_rows_for_month(app, account_id, month) {
                let go = scrape_go_windows(app, account_id).unwrap_or(previous.go.clone());
                let usage = account::ConsoleUsage {
                    keys: previous.keys,
                    usage: rows,
                    go,
                };
                cache_console(account_id, month, &usage);
                return Ok(usage);
            }
        }
    }
    let usage = sync_from_pages(app, account_id, month)?;
    cache_console(account_id, month, &usage);
    Ok(usage)
}

/// Fresh Go-plan windows from the /go/ panel: the limits popup must track
/// live usage, so the cached fast-path never reuses windows this old.
fn scrape_go_windows(app: &AppHandle, account_id: &str) -> Option<Vec<account::GoWindow>> {
    let win = ensure_signed_in(app, account_id).ok()?;
    let base = workspace_base(&win)?;
    let go_url: tauri::Url = format!("{base}/go/").parse().ok()?;
    win.navigate(go_url).ok()?;
    scrape_until(
        &win,
        &[
            "[data-slot=usage-label]",
            "[data-slot=usage-value]",
            "[data-slot=reset-time]",
        ],
        std::time::Duration::from_secs(8),
    )
    .map(|go| parse_go_windows(&go))
    .filter(|windows| !windows.is_empty())
}

fn any_cached_console(provider: &str) -> Option<account::ConsoleUsage> {
    let cache = CONSOLE_CACHE.lock().ok()?;
    let (cached_provider, _, _, usage) = cache.as_ref()?;
    (cached_provider == provider).then(|| usage.clone())
}

/// Sign in to a non-OpenCode provider and read what its console offers.
/// Extraction is built iteratively from the diagnostics this logs.
fn probe_provider(app: &AppHandle, account_id: &str) -> Result<account::ConsoleUsage, String> {
    let provider = account_provider(account_id);
    let spec =
        providers::by_id(&provider).ok_or_else(|| format!("unknown provider: {provider}"))?;
    let win = ensure_signed_in(app, account_id)?;
    let base = format!("https://{}", spec.host);
    account::log_line(&format!("probing {provider} from {base} (signed in)"));

    let keys_url: tauri::Url = format!("{base}{}", spec.keys_path)
        .parse()
        .map_err(|e| format!("bad url: {e}"))?;
    win.navigate(keys_url).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(3500));
    log_fetch_traffic(&win, &format!("{provider}-keys"));
    if let Ok(vocab) = scrape_page(&win, &["@data-slot"]) {
        account::log_line(&format!(
            "[{provider}-keys] slots: {}",
            vocab.first().map(|v| v.join(" | ")).unwrap_or_default()
        ));
    }
    if let Ok(text) = scrape_page(&win, &["body"]) {
        account::log_line(&format!(
            "[{provider}-keys] text: {}",
            text.first()
                .and_then(|v| v.first())
                .map(|t| t.chars().take(500).collect::<String>())
                .unwrap_or_default()
        ));
    }
    if let Ok(found) = scrape_page(&win, &["@find=c2st"]) {
        account::log_line(&format!(
            "[{provider}-keys] sk hits: {:?}",
            found.first().map(|v| v.iter().take(3).collect::<Vec<_>>())
        ));
    }

    if let Some(path) = spec.usage_path {
        let usage_url: tauri::Url = format!("{base}{path}")
            .parse()
            .map_err(|e| format!("bad url: {e}"))?;
        win.navigate(usage_url).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(3500));
        log_fetch_traffic(&win, &format!("{provider}-usage"));
        if let Ok(vocab) = scrape_page(&win, &["@data-slot"]) {
            account::log_line(&format!(
                "[{provider}-usage] slots: {}",
                vocab.first().map(|v| v.join(" | ")).unwrap_or_default()
            ));
        }
        if let Ok(text) = scrape_page(&win, &["body"]) {
            account::log_line(&format!(
                "[{provider}-usage] text: {}",
                text.first()
                    .and_then(|v| v.first())
                    .map(|t| t.chars().take(700).collect::<String>())
                    .unwrap_or_default()
            ));
        }
    }

    Ok(account::ConsoleUsage {
        keys: Vec::new(),
        usage: Vec::new(),
        go: Vec::new(),
    })
}
/// Replay the month's usage RPC without re-walking every console page.
fn fetch_usage_rows_for_month(
    app: &AppHandle,
    account_id: &str,
    month: &Month,
) -> Option<Vec<account::ConsoleRow>> {
    let win = ensure_signed_in(app, account_id).ok()?;
    let base = workspace_base(&win)?;
    let usage_url: tauri::Url = format!("{base}/usage/").parse().ok()?;
    win.navigate(usage_url).ok()?;
    let rows = poll_usage_rows(&win, month, std::time::Duration::from_secs(10));
    match &rows {
        Some(rows) => account::log_line(&format!(
            "month switch: {} rows for {}-{:02}",
            rows.len(),
            month.year,
            month.month + 1
        )),
        None => account::log_line(&format!(
            "month switch: no live usage RPC captured for {}-{:02}",
            month.year,
            month.month + 1
        )),
    }
    rows
}

#[tauri::command]
async fn console_keys(
    app: AppHandle,
    month: Option<Month>,
    account: Option<String>,
    refresh: Option<bool>,
) -> Result<ConsoleKeysOut, String> {
    let month = normalize_month(month);
    let account = account.unwrap_or_else(|| "opencode-go".to_string());
    let force = refresh.unwrap_or(false);
    if let Some(hit) = cached_console(&account, &month, force) {
        return Ok(ConsoleKeysOut {
            email: hit.keys.iter().find_map(|k| k.email.clone()),
            keys: hit.keys,
            month,
        });
    }
    let usage = {
        let app = app.clone();
        let month = month.clone();
        let account = account.clone();
        tauri::async_runtime::spawn_blocking(move || {
            console_snapshot_windows(&app, &account, &month, force)
        })
        .await
        .map_err(|e| e.to_string())??
    };
    cache_console(&account, &month, &usage);
    Ok(ConsoleKeysOut {
        email: usage.keys.iter().find_map(|k| k.email.clone()),
        keys: usage.keys,
        month,
    })
}

#[tauri::command]
async fn activate_account_key(
    app: AppHandle,
    key_id: String,
    account: Option<String>,
) -> Result<AppState, String> {
    let account = account.unwrap_or_else(|| "opencode-go".to_string());
    let provider = account_provider(&account);
    let month = normalize_month(None);
    let usage = if let Some(hit) = cached_console(&account, &month, false) {
        hit
    } else {
        let fetched = {
            let app = app.clone();
            let month = month.clone();
            let account = account.clone();
            tauri::async_runtime::spawn_blocking(move || {
                console_snapshot_windows(&app, &account, &month, false)
            })
            .await
            .map_err(|e| e.to_string())??
        };
        cache_console(&account, &month, &fetched);
        fetched
    };
    let key = usage
        .keys
        .iter()
        .find(|k| k.id == key_id)
        .ok_or("That key is no longer in the account")?
        .clone();

    let store_path = paths::store_file();

    let name = key
        .name
        .clone()
        .unwrap_or_else(|| format!("key-{}", &key.id.chars().take(6).collect::<String>()));
    let email = key.email.clone().unwrap_or_default();
    let live_key = live_credentials(&store::read_store(&store_path)).remove(&provider);
    let at = now_ms();
    let mut secret_holder = key.secret.clone();
    let (saved, ()) = store::update_store(&store_path, |store| {
        // 1) the account told us the secret; 2) match a key we already hold.
        let secret = secret_holder.take().or_else(|| {
            store
                .providers
                .get(&provider)
                .and_then(|p| {
                    p.keys.values().find(|cred| match &key.prefix {
                        Some(prefix) if !prefix.is_empty() => cred.key.starts_with(prefix),
                        _ => false,
                    })
                })
                .map(|cred| cred.key.clone())
        });
        let Some(secret) = secret else {
            return Err(
                "This key's secret is not available — create a fresh key in the console, then activate it."
                    .to_string(),
            );
        };
        store::upsert_key(store, &provider, &name, &secret, &email)?;
        let prev = live_key.as_deref().and_then(|live| {
            store
                .providers
                .get(&provider)?
                .keys
                .iter()
                .find(|(_, c)| c.key == live)
                .map(|(n, _)| n.clone())
        });
        store::apply_key(store, &provider, &name, at, prev)
    })?;
    let cred = saved
        .providers
        .get(&provider)
        .and_then(|e| e.keys.get(&name))
        .ok_or("Key vanished after save")?;
    write_credential(&provider, cred)?;
    Ok(build_state(&app, Some(viewed_month()), false).await)
}

#[tauri::command]
async fn console_usage(
    app: AppHandle,
    month: Option<Month>,
    account: Option<String>,
    refresh: Option<bool>,
) -> Result<account::ConsoleUsage, String> {
    let month = normalize_month(month);
    let account = account.unwrap_or_else(|| "opencode-go".to_string());
    let force = refresh.unwrap_or(false);
    if let Some(hit) = cached_console(&account, &month, force) {
        return Ok(hit);
    }
    let usage = {
        let app = app.clone();
        let month = month.clone();
        let account = account.clone();
        tauri::async_runtime::spawn_blocking(move || {
            console_snapshot_windows(&app, &account, &month, force)
        })
        .await
        .map_err(|e| e.to_string())??
    };
    cache_console(&account, &month, &usage);
    Ok(usage)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountStatus {
    accounts: Vec<store::AccountRef>,
    active_account: Option<String>,
    /// Result of the post sign-in console probe (shown in the UI toast).
    #[serde(skip_serializing_if = "Option::is_none")]
    probe: Option<String>,
}

#[tauri::command]
async fn account_status() -> AccountStatus {
    let store = store::read_store(&paths::store_file());
    AccountStatus {
        accounts: store.settings.accounts,
        active_account: store.settings.active_account,
        probe: None,
    }
}

fn set_active_account(account_id: &str) {
    let store_path = paths::store_file();
    store::update_store_infallible(&store_path, |store| {
        store.settings.active_account = Some(account_id.to_string());
    });
}

#[tauri::command]
async fn connect_account_webview(
    app: AppHandle,
    provider: Option<String>,
    account: Option<String>,
) -> Result<AccountStatus, String> {
    let provider = provider.unwrap_or_else(|| "opencode-go".to_string());
    let account_id = match account {
        Some(id) => id,
        None => {
            let store = store::read_store(&paths::store_file());
            next_account_id(&store, &provider)
        }
    };
    let app_for_task = app.clone();
    let account_for_task = account_id.clone();
    let probe = tauri::async_runtime::spawn_blocking(move || {
        // Shows the console window while the user signs in, then hides it.
        let win = ensure_signed_in(&app_for_task, &account_for_task)?;
        account::log_line(&format!("signed in at {}", window_url(&win)));
        let month = normalize_month(None);
        match console_snapshot_windows(&app_for_task, &account_for_task, &month, true) {
            Ok(usage) => Ok(usage),
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    match probe {
        Ok(usage) => {
            let store_path = paths::store_file();
            let account_ref = store::AccountRef {
                id: account_id.clone(),
                provider: provider.clone(),
                label: usage.keys.iter().find_map(|k| k.email.clone()),
            };
            let saved = store::update_store_infallible(&store_path, |store| {
                if let Some(existing) = store
                    .settings
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == account_id)
                {
                    if account_ref.label.is_some() {
                        existing.label = account_ref.label.clone();
                    }
                } else {
                    store.settings.accounts.push(account_ref.clone());
                }
                store.settings.active_account = Some(account_id.clone());
            });
            Ok(AccountStatus {
                accounts: saved.settings.accounts,
                active_account: saved.settings.active_account,
                probe: Some(format!(
                    "account data available ({} keys, {} rows)",
                    usage.keys.len(),
                    usage.usage.len()
                )),
            })
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn disconnect_account(
    app: AppHandle,
    account: Option<String>,
) -> Result<AccountStatus, String> {
    let account_id = account.unwrap_or_else(|| "opencode-go".to_string());
    let provider = account_provider(&account_id);
    // Logging out must forget the console session too: wipe the webview's
    // cookies/storage and drop the window. The next connect then starts at
    // the sign-in page and lets the user pick an account instead of silently
    // reusing whichever Google session was last in the webview.
    let label = account_window_key(&account_id);
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.clear_all_browsing_data();
        let _ = win.destroy();
        account::log_line(&format!("disconnected {account_id}: console session wiped"));
    }
    if let Ok(mut cache) = CONSOLE_CACHE.lock() {
        *cache = None;
    }
    if provider == "opencode-go" {
        account::forget_tokens();
        account::forget_session_cookie();
    }
    let store_path = paths::store_file();
    let store = store::update_store_infallible(&store_path, |store| {
        store.settings.accounts.retain(|a| a.id != account_id);
        if store.settings.active_account.as_deref() == Some(account_id.as_str()) {
            store.settings.active_account = store.settings.accounts.last().map(|a| a.id.clone());
        }
    });
    account::log_line(&format!("disconnected {account_id}: app state cleared"));
    Ok(AccountStatus {
        accounts: store.settings.accounts,
        active_account: store.settings.active_account,
        probe: None,
    })
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---------- menubar shell ----------

fn toggle_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show_panel(app);
    }
}

/// Pure panel placement math (unit-tested): start-right under the tray icon
/// (the panel's RIGHT edge sits under the icon's right edge, so it unfolds
/// leftwards), then clamped inside the monitor. Physical pixels throughout.
fn panel_origin(
    tray: (f64, f64, f64, f64),            // x, y, w, h
    panel: (f64, f64),                     // width, height
    gap: f64,                              // vertical gap below the tray icon
    monitor: Option<(f64, f64, f64, f64)>, // x, y, w, h
) -> (i32, i32) {
    let (tx, ty, tw, th) = tray;
    let (pw, ph) = panel;
    let mut px = tx + tw - pw;
    let mut py = ty + th + gap;
    if let Some((mx, my, mw, mh)) = monitor {
        let (left, top) = (mx, my);
        let (right, bottom) = (mx + mw, my + mh);
        px = px.clamp(left, (right - pw).max(left));
        py = py.clamp(top, (bottom - ph).max(top));
    }
    (px as i32, py as i32)
}

fn show_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // Show first so the window has a backing store, then anchor it.
    let _ = window.show();
    position_panel(app, &window);
    let _ = window.set_focus();
}

/// Anchor the panel under the real tray icon rect; fall back to the
/// positioner plugin when the rect is unavailable.
fn position_panel(app: &AppHandle, window: &tauri::WebviewWindow) {
    let placed = (|| {
        let tray = app.tray_by_id("main")?;
        let rect = tray.rect().ok()??;
        let scale = window.scale_factor().ok()?;
        let (x, y) = match rect.position {
            tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
            tauri::Position::Logical(p) => (p.x * scale, p.y * scale),
        };
        let (w, h) = match rect.size {
            tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
            tauri::Size::Logical(s) => (s.width * scale, s.height * scale),
        };
        let panel_w = 430.0 * scale;
        let panel_h = 600.0 * scale;
        let monitor = window
            .current_monitor()
            .or_else(|_| window.primary_monitor())
            .ok()
            .flatten()
            .map(|m| {
                let p = m.position();
                let s = m.size();
                (p.x as f64, p.y as f64, s.width as f64, s.height as f64)
            });
        let (px, py) = panel_origin((x, y, w, h), (panel_w, panel_h), 6.0 * scale, monitor);
        window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: px,
                y: py,
            }))
            .ok()?;
        #[cfg(debug_assertions)]
        eprintln!("[keyturn] panel anchored at {px},{py} (tray rect {x},{y} {w}x{h})");
        Some(())
    })();
    if placed.is_none() {
        #[cfg(debug_assertions)]
        eprintln!("[keyturn] tray rect unavailable — falling back to positioner");
        let _ = window.move_window(Position::TrayCenter);
    }
}

/// Icon shown before the first state arrives: the profile name and no numbers.
fn idle_tray_snapshot(root: &profiles::Root) -> tray::TraySnapshot {
    let name = root
        .profiles
        .iter()
        .find(|p| p.id == root.active)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| root.active.clone());
    tray::TraySnapshot {
        profiles: vec![tray::TrayProfile {
            id: root.active.clone(),
            label: name,
            active: true,
            metrics: tray::TrayMetric::default(),
            tokens: 0.0,
            cost: 0.0,
        }],
        style: root.app.tray_style.clone(),
        color_mode: root.app.tray_color_mode.clone(),
        custom_color: tray::custom_color(&root.app.tray_custom_color),
        used_mode: root.app.tray_used_mode.clone(),
        show_remaining: root.app.tray_show_remaining,
        show_labels: root.app.tray_show_labels,
        show_pace: root.app.tray_show_pace,
        metrics: root.app.tray_metrics.clone(),
        primary: root.app.tray_primary.clone(),
        active_indicator: root.app.tray_active_indicator,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_panel(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_key,
            delete_key,
            apply_key,
            set_settings,
            set_app_settings,
            profile_create,
            profile_rename,
            profile_switch,
            profile_delete,
            list_sounds,
            play_sound,
            codex_status,
            codex_refresh,
            history,
            export_history,
            check_updates,
            account_status,
            connect_account_webview,
            disconnect_account,
            console_keys,
            console_usage,
            activate_account_key,
            set_active_account_cmd,
            add_provider,
            remove_provider,
            quit_app
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            // Direct toggle on click — no menu, so nothing intercepts the click.
            let root = profiles::read_root(&paths::profiles_file());
            paths::set_active_profile(&root.active);
            let mut builder = TrayIconBuilder::with_id("main").tooltip("Keyturn");
            let (rgba, width, height, template) = tray::render(&idle_tray_snapshot(&root));
            builder = builder.icon(tauri::image::Image::new_owned(rgba, width, height));
            #[cfg(target_os = "macos")]
            {
                builder = builder.icon_as_template(template);
            }
            let app_handle = app.handle().clone();
            spawn_background_refresh(app.handle().clone());
            builder
                .on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_panel(&app_handle);
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::Focused(false) = event {
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod panel_tests {
    use super::panel_origin;

    #[test]
    fn ends_at_the_tray_icon_right_edge() {
        // Tray icon at x=1200, 24px wide (2x display), panel 860x1200 physical.
        let (x, y) = panel_origin(
            (1200.0, 0.0, 24.0, 24.0),
            (860.0, 1200.0),
            12.0,
            Some((0.0, 0.0, 2880.0, 1800.0)),
        );
        assert_eq!(
            x,
            1200 + 24 - 860,
            "panel right edge must equal the tray icon right edge"
        );
        assert_eq!(y, 24 + 12, "panel hangs below the tray icon by the gap");
    }

    #[test]
    fn clamps_inside_the_monitor_on_the_right() {
        // Tray hugging the screen edge must not push the panel off-screen.
        let (x, _) = panel_origin(
            (2860.0, 0.0, 24.0, 24.0),
            (860.0, 1200.0),
            12.0,
            Some((0.0, 0.0, 2880.0, 1800.0)),
        );
        assert_eq!(x, 2880 - 860);
    }

    #[test]
    fn respects_a_monitor_with_a_non_zero_origin() {
        // Second display placed to the left of the primary one.
        let (x, _) = panel_origin(
            (-1900.0, 0.0, 24.0, 24.0),
            (860.0, 1200.0),
            12.0,
            Some((-1920.0, 0.0, 1920.0, 1080.0)),
        );
        assert_eq!(x, -1920, "clamped to that monitor's left edge");
    }

    #[test]
    fn falls_back_to_the_tray_position_without_a_monitor() {
        let (x, y) = panel_origin((500.0, 10.0, 24.0, 24.0), (860.0, 1200.0), 6.0, None);
        assert_eq!((x, y), (500 + 24 - 860, 10 + 24 + 6));
    }
}

#[cfg(test)]
mod automation_tests {
    use super::*;

    fn quota(percent: f64) -> KeyQuota {
        KeyQuota {
            windows: Some(vec![usage::QuotaWindow {
                window: "rolling".to_string(),
                percent,
                resets_at: None,
                status: None,
            }]),
            error: None,
        }
    }

    #[test]
    fn exhaustion_detects_full_windows_statuses_and_429s() {
        assert!(!exhausted(&quota(42.0)));
        assert!(exhausted(&quota(100.0)));
        let mut limited = quota(10.0);
        limited.error = Some("HTTP 429".to_string());
        assert!(exhausted(&limited));
        let mut status = quota(10.0);
        status.windows.as_mut().unwrap()[0].status = Some("limit_reached".to_string());
        assert!(exhausted(&status));
        assert_eq!(quota_score(&quota(88.0)), 88.0);
    }
}

#[cfg(test)]
mod live_smoke {
    use crate::{live_credentials, paths, providers, store, usage};

    fn live() -> bool {
        if std::env::var("KEYTURN_LIVE_SMOKE").is_err() {
            eprintln!("skipped (set KEYTURN_LIVE_SMOKE=1 for live smoke)");
            return false;
        }
        true
    }

    #[test]
    fn live_store_db_quota() {
        if !live() {
            return;
        }
        // Real store: never write here, read-only.
        let store = store::read_store(&paths::store_file());
        assert!(
            store.providers.contains_key("opencode-go"),
            "expected opencode-go provider"
        );
        let keys: Vec<String> = store.providers["opencode-go"]
            .keys
            .keys()
            .cloned()
            .collect();
        assert!(!keys.is_empty(), "expected at least one key");

        // Real db: readonly open, structured rows.
        let (entries, error) = usage::read_usage(&paths::open_code_db_file());
        assert!(error.is_none(), "db error: {error:?}");
        assert!(!entries.is_empty(), "expected assistant messages");
        assert!(entries
            .iter()
            .all(|e| !e.provider.is_empty() && !e.model.is_empty() && e.t > 0));

        // Current-month attribution covers every in-month row exactly once.
        let (cy, cm) = usage::current_month();
        let (start, end) = usage::month_range(cy, cm);
        assert!(start <= chrono::Local::now().timestamp_millis());
        let month_entries: Vec<_> = entries
            .iter()
            .filter(|e| e.t >= start && e.t < end)
            .collect();
        let expected: f64 = month_entries.iter().map(|e| e.cost).sum();
        let owned: Vec<usage::UsageEntry> = month_entries.into_iter().cloned().collect();
        let (by_key, untracked) = usage::attribute_usage(&owned, &store);
        let got: f64 = by_key
            .values()
            .flat_map(|m| m.values())
            .chain(untracked.values())
            .map(|b| b.total)
            .sum();
        assert!(
            (got - expected).abs() < 1e-6,
            "attribution must cover all rows: {got} vs {expected}"
        );

        // auth.json live key matches a stored key (or nothing applied).
        let live = live_credentials(&store);
        let applied = store::compute_applied(&store, &live);
        for (provider, key) in &applied {
            if let Some(k) = key {
                assert!(
                    store.providers[provider].keys.contains_key(k),
                    "applied key must exist"
                );
            }
        }

        // Write path round-trips in a temp dir (never the real file), 0600.
        let dir = std::env::temp_dir().join(format!("kr-smoke-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tmp = dir.join("agent-rtl.keys");
        let mut copy = store.clone();
        store::upsert_key(
            &mut copy,
            "opencode-go",
            "SmokeTmp",
            "sk-smoke",
            "smoke@x.com",
        )
        .unwrap();
        let saved = store::write_store(&tmp, &copy);
        assert_eq!(
            saved.providers["opencode-go"].keys["SmokeTmp"]
                .account
                .as_deref(),
            Some("smoke@x.com")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&tmp).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        store::delete_key(&mut copy, "opencode-go", "SmokeTmp").unwrap();
        let saved = store::write_store(&tmp, &copy);
        assert!(!saved.providers["opencode-go"].keys.contains_key("SmokeTmp"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn live_quota_fetch() {
        if !live() {
            return;
        }
        let store = store::read_store(&paths::store_file());
        let entry = &store.providers["opencode-go"];
        let (name, cred) = entry.keys.iter().next().expect("a key");
        let base = providers::usage_api("opencode-go").unwrap();
        let quota = usage::fetch_provider_usage(base, &cred.key).await;
        assert!(
            quota.windows.is_some(),
            "expected quota windows for {name}, got error: {:?}",
            quota.error
        );
        let windows = quota.windows.unwrap();
        assert_eq!(windows.len(), 3);
        assert!(windows.iter().all(|w| (0.0..=100.0).contains(&w.percent)));
    }
}
