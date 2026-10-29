//! Keyturn — provider status and named Codex account profiles.
//! Provider CLIs own sign-in. Explicit profile actions save and activate
//! Codex file-based logins; other provider credentials remain read-only.

pub mod cli;
pub mod subscriptions;
pub mod claude_usage;
pub mod codex;
mod codex_refresh;
pub mod codex_resets;
pub mod paths;
pub mod profiles;
pub mod claude_profiles;
pub mod profile_login;
mod account_login;
mod account_setup;
mod account_setup_cleanup;
mod antigravity;
pub mod desktop_switch;
mod codex_activity;
pub mod quota;
pub mod settings;
pub mod workspaces;
pub mod store;
pub mod tray;
pub mod tray_text;

use serde::Serialize;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    ActivationPolicy, AppHandle, Manager, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};

// ---------- IPC shapes (frozen contract with the React frontend) ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOut {
    pub open_at_login: bool,
}

/// App preferences and workspace metadata; no credentials or quota payloads.
#[derive(Debug, Clone, Serialize)]
pub struct AppState {
    pub settings: SettingsOut,
    pub app: settings::AppSettings,
}

// ---------- helpers ----------

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

fn build_state(app: &AppHandle) -> AppState {
    let settings_path = paths::settings_file();
    let app_settings = settings::read_settings(&settings_path);
    let store_path = paths::store_file();
    // One-time legacy cleanup: drop stored keys/logs/accounts/theme.
    store::compact_legacy(&store_path);
    let actual = autolaunch_enabled(app);
    let store = {
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

    AppState {
        settings: SettingsOut {
            open_at_login: store.settings.open_at_login,
        },
        app: app_settings,
    }
}

// ---------- commands ----------

#[tauri::command]
async fn get_state(app: AppHandle) -> Result<AppState, String> {
    tauri::async_runtime::spawn_blocking(workspaces::initialize).await
        .map_err(|_| "Cannot load workspaces.".to_string())??;
    let state = build_state(&app);
    app.state::<tray::MenuBarMonitor>().update(&app, state.app.clone());
    Ok(state)
}

fn workspace_changed(app: &AppHandle, mut saved: settings::AppSettings) -> Result<AppState, String> {
    saved.project_workspace_preferences();
    if let Some(workspace) = saved.active_workspace() {
        app.state::<tray::MenuBarMonitor>().set_accent(app, &workspace.accent)?;
    }
    app.state::<tray::MenuBarMonitor>().update(app, saved);
    Ok(build_state(app))
}

#[tauri::command]
async fn create_workspace(app: AppHandle, name: String, accent: String, account_name: String,
    credentials: Option<String>, path: Option<String>) -> Result<AppState, String> {
    let saved = tauri::async_runtime::spawn_blocking(move || workspaces::import_account(None, name, Some(accent), account_name, credentials, path))
        .await.map_err(|_| "Cannot create this workspace.".to_string())??;
    workspace_changed(&app, saved)
}

/// Subscription metadata comes from provider-owned billing claims, never token expiry.
fn paid_workspace_accounts(statuses: Vec<cli::ProviderCliStatus>) -> Vec<cli::ProviderCliStatus> {
    statuses.into_iter().filter_map(|mut status| {
        if status.logged_in != Some(true) { return None; }
        status.subscriptions.retain(|subscription| subscription.is_paid());
        if status.subscriptions.is_empty() { return None; }
        Some(status)
    }).collect()
}

fn validate_workspace_selection(statuses: &[cli::ProviderCliStatus], mut selected: Vec<String>) -> Result<Vec<String>, String> {
    if selected.is_empty() || selected.iter().any(|id| !statuses.iter().any(|s| &s.id == id)) {
        return Err("A selected account is no longer available with a paid plan. Load accounts again.".to_string());
    }
    selected.sort(); selected.dedup();
    Ok(selected)
}

#[tauri::command]
async fn discover_workspace_accounts() -> Result<Vec<cli::ProviderCliStatus>, String> {
    tauri::async_runtime::spawn_blocking(|| paid_workspace_accounts(cli::all_status()))
        .await.map_err(|_| "Cannot load CLI accounts. Try again or import a file.".to_string())
}

/// Recheck entitlement before saving only the user's selected providers.
#[tauri::command]
async fn create_discovered_workspace(app: AppHandle, name: String, accent: String, provider_ids: Vec<String>, account_emails: std::collections::HashMap<String, Option<String>>) -> Result<AppState, String> {
    let saved = tauri::async_runtime::spawn_blocking(move || {
        let statuses = paid_workspace_accounts(cli::all_status());
        let selected = validate_workspace_selection(&statuses, provider_ids)?;
        if selected.iter().any(|id| statuses.iter().find(|s| &s.id == id)
            .is_some_and(|status| account_emails.get(id) != Some(&status.email))) {
            return Err("A selected CLI account changed. Load accounts again before adding it.".into());
        }
        if selected.iter().any(|id| id == "codex") {
            let path = paths::codex_auth_file().to_string_lossy().into_owned();
            let account_name = statuses.iter().find(|s| s.id == "codex")
                .and_then(|s| s.email.clone()).unwrap_or_else(|| "Codex".into());
            return workspaces::import_selected_account(None, name, Some(accent), account_name, None, Some(path), Some(selected));
        }
        workspaces::create_for_cli(&name, &accent, selected)
    }).await.map_err(|_| "Cannot save selected accounts.".to_string())??;
    workspace_changed(&app, saved)
}

#[tauri::command]
async fn import_workspace_account(app: AppHandle, workspace_id: String, account_name: String,
    credentials: Option<String>, path: Option<String>) -> Result<AppState, String> {
    let saved = tauri::async_runtime::spawn_blocking(move || workspaces::import_account(Some(workspace_id), String::new(), None, account_name, credentials, path))
        .await.map_err(|_| "Cannot import this account.".to_string())??;
    workspace_changed(&app, saved)
}

#[tauri::command]
async fn include_workspace_provider(app: AppHandle, workspace_id: String, provider_id: String) -> Result<AppState, String> {
    workspace_changed(&app, workspaces::include_provider(&workspace_id, &provider_id)?)
}

#[tauri::command]
fn begin_account_setup() -> Result<String,String> { account_setup::begin() }
#[tauri::command]
async fn discard_account_setup(session_id:String) -> Result<(),String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::discard(&session_id)).await.map_err(|_| "Cannot close account setup.".to_string())?
}
#[tauri::command]
async fn load_setup_accounts(session_id:String) -> Result<Vec<account_setup::Account>,String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::discover(&session_id)).await.map_err(|_| "Cannot load accounts.".to_string())?
}
#[tauri::command]
async fn start_setup_login(session_id:String, provider:String, method:String, name:String) -> Result<account_setup::LoginResult,String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::start(&session_id,&provider,&method,&name)).await.map_err(|_| "Cannot start sign-in.".to_string())?
}
#[tauri::command]
async fn poll_setup_login(session_id:String, attempt_id:String) -> Result<account_setup::LoginResult,String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::poll(&session_id,&attempt_id)).await.map_err(|_| "Cannot verify sign-in.".to_string())?
}
#[tauri::command]
async fn cancel_setup_login(session_id:String) -> Result<(),String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::cancel(&session_id)).await.map_err(|_| "Cannot cancel sign-in.".to_string())?
}
#[tauri::command]
async fn submit_setup_code(session_id:String, code:String) -> Result<(),String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::submit_code(&session_id,&code)).await.map_err(|_| "Cannot send the sign-in code.".to_string())?
}
#[tauri::command]
async fn import_setup_account(session_id:String, provider:String, name:String, credentials:Option<String>, path:Option<String>) -> Result<account_setup::Account,String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::import(&session_id,&provider,&name,credentials,path)).await.map_err(|_| "Cannot import this account.".to_string())?
}
#[tauri::command]
async fn connect_current_google(session_id:String,name:String) -> Result<account_setup::Account,String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::current_google(&session_id,&name)).await.map_err(|_| "Cannot verify the current Google login.".to_string())?
}
#[tauri::command]
async fn connect_setup_config(session_id:String, name:String, path:String) -> Result<account_setup::Account,String> {
    tauri::async_runtime::spawn_blocking(move || account_setup::connect_config(&session_id,&name,&path)).await.map_err(|_| "Cannot connect this configuration.".to_string())?
}
#[tauri::command]
async fn commit_account_setup(app:AppHandle, session_id:String, workspace_id:Option<String>, name:String, accent:String, account_ids:Vec<String>) -> Result<AppState,String> {
    let id=session_id.clone();
    let saved=tauri::async_runtime::spawn_blocking(move || account_setup::commit(&id,workspace_id,&name,&accent,&account_ids)).await.map_err(|_| "Cannot save these accounts.".to_string())??;
    account_setup::discard(&session_id)?;
    workspace_changed(&app,saved)
}
#[tauri::command]
async fn remove_workspace_account(app:AppHandle, workspace_id:String, provider:String, id:String, everywhere:bool) -> Result<AppState,String> {
    let saved=tauri::async_runtime::spawn_blocking(move || workspaces::remove_membership(&workspace_id,&provider,&id,everywhere)).await.map_err(|_| "Cannot remove this account.".to_string())??;
    workspace_changed(&app,saved)
}
#[tauri::command]
async fn antigravity_accounts() -> Result<Vec<account_setup::Account>,String> {
    tauri::async_runtime::spawn_blocking(|| antigravity::list()?.into_iter().map(|p| {
        let subscription=p.subscription.ok_or("This account needs reverification.")?;
        Ok(account_setup::Account{id:p.id,provider:"antigravity".into(),name:p.name,display_name:None,active:false,email:p.email,subscription})
    }).collect()).await.map_err(|_| "Cannot read Antigravity accounts.".to_string())?
}
#[tauri::command]
async fn rename_antigravity_account(id: String, name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || antigravity::rename(&id, &name))
        .await.map_err(|_| "Cannot rename this account.".to_string())?
}
#[tauri::command]
async fn antigravity_account_quota(id:String) -> Result<quota::QuotaSet,String> {
    tauri::async_runtime::spawn_blocking(move || antigravity::quota(&id)).await.map_err(|_| "Cannot load Antigravity usage.".to_string())?
}

#[tauri::command]
async fn update_workspace(app: AppHandle, id: String, name: String, accent: String, usage: workspaces::WorkspaceUsage) -> Result<AppState, String> {
    workspace_changed(&app, workspaces::edit(&id, &name, &accent, usage)?)
}

#[tauri::command]
async fn select_workspace(app: AppHandle, id: String) -> Result<AppState, String> {
    workspace_changed(&app, workspaces::select(&id)?)
}

#[tauri::command]
async fn move_workspace_account(app: AppHandle, id: String, target_id: String) -> Result<AppState, String> {
    workspace_changed(&app, workspaces::move_account(&id, &target_id)?)
}

#[tauri::command]
#[allow(non_snake_case)]
async fn set_settings(app: AppHandle, openAtLogin: bool) -> Result<AppState, String> {
    let path = paths::store_file();
    let mut store = store::read_store(&path);
    store.settings.open_at_login = openAtLogin;
    apply_open_at_login(&app, openAtLogin);
    store::write_store(&path, &store);
    Ok(build_state(&app))
}

/// Merge panel/menu-bar preferences and immediately apply the native display.
#[tauri::command]
async fn set_app_settings(app: AppHandle, settings: serde_json::Value) -> Result<AppState, String> {
    let path = paths::settings_file();
    let (saved, ()) = settings::update_settings(&path, |current| {
        settings::merge_global(current, &settings)
    })?;
    app.state::<tray::MenuBarMonitor>().update(&app, saved);
    Ok(build_state(&app))
}

/// Update one account atomically so simultaneous editors cannot overwrite siblings.
#[tauri::command]
async fn set_codex_account_preferences(
    app: AppHandle,
    id: String,
    preferences: settings::CodexAccountPreferences,
    start_cycle_with: Option<String>,
) -> Result<AppState, String> {
    let accounts = tauri::async_runtime::spawn_blocking(profiles::list).await
        .map_err(|_| "Cannot read Codex accounts.".to_string())??;
    if !accounts.profiles.iter().any(|p| p.id == id) {
        return Err("This Codex account no longer exists.".into());
    }
    if let Some(partner) = &start_cycle_with {
        if !preferences.auto_switch || partner == &id || !accounts.profiles.iter().any(|p| &p.id == partner) {
            return Err("Choose an existing second Codex account for the switch cycle.".into());
        }
    }
    let (saved, ()) = settings::update_settings(&paths::settings_file(), |current| {
        if let Some(partner) = &start_cycle_with {
            if !current.contains_codex_account(&id) || !current.contains_codex_account(partner) {
                return Err("Choose a second account from this workspace.".into());
            }
        }
        current.codex_accounts.insert(id.clone(), preferences.clone());
        if let Some(w) = current.workspaces.iter_mut().find(|w| Some(&w.id) == current.active_workspace_id.as_ref()) {
            if w.account_preferences.is_none() { w.account_preferences = Some(current.codex_accounts.clone()); }
            w.account_preferences.as_mut().unwrap().insert(id, preferences);
        }
        if let Some(partner) = start_cycle_with {
            current.codex_accounts.entry(partner.clone()).or_default().auto_switch = true;
            if let Some(w) = current.workspaces.iter_mut().find(|w| Some(&w.id) == current.active_workspace_id.as_ref()) { if let Some(prefs) = &mut w.account_preferences { prefs.entry(partner).or_default().auto_switch = true; } }
        }
        Ok(())
    })?;
    app.state::<tray::MenuBarMonitor>().update(&app, saved);
    Ok(build_state(&app))
}

/// Mirror localStorage appearance into the native tray without persisting another copy.
#[tauri::command]
fn set_menu_bar_accent(app: AppHandle, accent: String) -> Result<(), String> {
    app.state::<tray::MenuBarMonitor>().set_accent(&app, &accent)
}

/// CLI-only provider status (read-only). Never logs in, never writes.
#[tauri::command]
async fn cli_status(app: AppHandle) -> Vec<cli::ProviderCliStatus> {
    app.state::<tray::MenuBarMonitor>().refresh();
    // Status subcommands are blocking + short-lived; keep them off the async
    // runtime. Read-only: never logs in, never writes credentials.
    tauri::async_runtime::spawn_blocking(cli::all_status)
        .await
        .unwrap_or_default()
}

/// Read all supported quota sources: OpenCode, Claude transcripts and Codex.
/// Credential files are read only; the frontend filters by provider.
#[tauri::command]
async fn cli_quotas() -> Vec<quota::QuotaSet> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut out = quota::opencode_quotas();
        if let Some(claude) = claude_usage::claude_quota_set() {
            out.push(claude);
        }
        if let Some(codex) = quota::codex_quota_set() {
            out.push(codex);
        }
        out.sort_by(|a, b| a.provider.cmp(&b.provider));
        out
    })
    .await
    .unwrap_or_default()
}

/// Public reset outlook, independent of CLI credentials and quota requests.
#[tauri::command]
async fn codex_reset_status() -> Result<codex_resets::CodexResetStatus, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let interval = settings::read_settings(&paths::settings_file()).codex_reset.refetch_interval_secs;
        codex_resets::status(std::time::Duration::from_secs(u64::from(interval)))
    })
        .await
        .map_err(|_| "Reset outlook is unavailable".to_string())?
}

#[tauri::command]
async fn codex_profiles() -> Result<profiles::ProfilesState, String> {
    tauri::async_runtime::spawn_blocking(profiles::list).await
        .map_err(|_| "Cannot read Codex profiles.".to_string())?
}

#[tauri::command]
async fn create_codex_profile(app: AppHandle, name: String, import_current: bool) -> Result<profiles::ProfilesState, String> {
    let result = tauri::async_runtime::spawn_blocking(move || profiles::create(&name, import_current)).await
        .map_err(|_| "Cannot create the profile.".to_string())??;
    app.state::<tray::MenuBarMonitor>().account_changed(&app);
    Ok(result)
}

#[tauri::command]
async fn rename_codex_profile(app: AppHandle, id: String, name: String) -> Result<profiles::ProfilesState, String> {
    let result = tauri::async_runtime::spawn_blocking(move || profiles::rename(&id, &name)).await
        .map_err(|_| "Cannot rename the profile.".to_string())??;
    app.state::<tray::MenuBarMonitor>().account_changed(&app);
    Ok(result)
}

#[tauri::command]
async fn remove_codex_profile(app: AppHandle, id: String) -> Result<profiles::ProfilesState, String> {
    let removed_id = id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || profiles::remove(&id)).await
        .map_err(|_| "Cannot remove the profile.".to_string())??;
    let (saved, ()) = settings::update_settings(&paths::settings_file(), |current| {
        current.codex_accounts.remove(&removed_id);
        for workspace in &mut current.workspaces { workspace.account_ids.retain(|id| id != &removed_id); }
        Ok(())
    })?;
    app.state::<tray::MenuBarMonitor>().update(&app, saved);
    app.state::<tray::MenuBarMonitor>().account_changed(&app);
    Ok(result)
}

#[tauri::command]
async fn activate_codex_profile(app: AppHandle, id: String) -> Result<profiles::ProfilesState, String> {
    let switched_id = id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || profiles::activate(&id)).await
        .map_err(|_| "Cannot activate the profile.".to_string())??;
    let (saved, ()) = settings::update_settings(&paths::settings_file(), |current| {
        workspaces::record_account_switch(current, "codex", &switched_id); Ok(())
    })?;
    app.state::<tray::MenuBarMonitor>().update(&app, saved);
    app.state::<tray::MenuBarMonitor>().account_changed(&app);
    Ok(result)
}

/// Read the path entered in the custom import form; credential bytes stay in Rust.
#[tauri::command]
async fn import_codex_profile(name: String, path: String, replace_id: Option<String>) -> Result<profiles::ProfilesState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let source = resolve_import_path(&path)?;
        profiles::import_file(&name, &source, replace_id.as_deref())
    }).await.map_err(|_| "Cannot import the login file.".to_string())?
}

fn resolve_import_path(path: &str) -> Result<std::path::PathBuf, String> {
    let path = path.trim();
    let source = if let Some(relative) = path.strip_prefix("~/") {
        dirs::home_dir().ok_or("Cannot locate your home folder.")?.join(relative)
    } else {
        std::path::PathBuf::from(path)
    };
    if !source.is_absolute() {
        return Err("Enter the full path to auth.json, starting with / or ~/.".into());
    }
    Ok(source)
}

#[tauri::command]
async fn claude_profile_quota(id: Option<String>) -> Result<quota::QuotaSet, String> {
    tauri::async_runtime::spawn_blocking(move || claude_profiles::quota(id.as_deref()))
        .await.map_err(|_| "Cannot read Claude usage.".to_string())?
}
#[tauri::command]
async fn claude_profiles() -> Result<Vec<claude_profiles::ProfileView>,String> {
    tauri::async_runtime::spawn_blocking(claude_profiles::list).await.map_err(|_|"Cannot read Anthropic profiles.".to_string())?
}
#[tauri::command]
async fn create_claude_profile(name:String,use_current:bool) -> Result<Vec<claude_profiles::ProfileView>,String> {
    tauri::async_runtime::spawn_blocking(move||claude_profiles::create(&name,use_current)).await.map_err(|_|"Cannot create this profile.".to_string())?
}
#[tauri::command]
async fn rescan_claude_profile(app:AppHandle,id:String) -> Result<Vec<claude_profiles::ProfileView>,String> {
    let profiles=tauri::async_runtime::spawn_blocking(move||claude_profiles::rescan(&id)).await.map_err(|_|"Cannot check this profile.".to_string())??;
    app.state::<tray::MenuBarMonitor>().refresh();
    Ok(profiles)
}
#[tauri::command]
async fn edit_claude_profile(app:AppHandle,id:String,name:Option<String>) -> Result<Vec<claude_profiles::ProfileView>,String> {
    let profiles=tauri::async_runtime::spawn_blocking(move||claude_profiles::edit(&id,name.as_deref())).await.map_err(|_|"Cannot update this profile.".to_string())??;
    app.state::<tray::MenuBarMonitor>().account_changed(&app);
    Ok(profiles)
}

/// Pasted auth.json is validated natively; no credential content is returned.
#[tauri::command]
async fn import_codex_profile_json(app: AppHandle, name: String, credentials: String, replace_id: Option<String>) -> Result<profiles::ProfilesState, String> {
    let result = tauri::async_runtime::spawn_blocking(move || profiles::import_json(&name, &credentials, replace_id.as_deref())).await
        .map_err(|_| "Cannot save these credentials.".to_string())??;
    app.state::<tray::MenuBarMonitor>().account_changed(&app);
    Ok(result)
}

#[tauri::command]
async fn codex_account_status() -> codex::CodexStatus { codex::status() }

#[tauri::command]
async fn codex_account_quota() -> Result<quota::QuotaSet,String> {
    tauri::async_runtime::spawn_blocking(|| quota::codex_quota_set().ok_or("No ChatGPT login is available.".to_string())).await
        .map_err(|_| "Cannot check the Codex connection.".to_string())?
}

#[tauri::command]
async fn codex_profile_quota(id: String) -> Result<quota::QuotaSet, String> {
    tauri::async_runtime::spawn_blocking(move || profiles::quota_for(&id).ok_or("No quota data for this profile.".to_string())).await
        .map_err(|_| "Cannot check this profile's usage.".to_string())?
}

#[tauri::command]
async fn start_codex_profile_login(id: String) -> Result<profile_login::LoginStatus, String> {
    tauri::async_runtime::spawn_blocking(move || profile_login::start(&id)).await
        .map_err(|_| "Cannot start sign-in.".to_string())?
}

#[tauri::command]
async fn codex_profile_login_status(id: String) -> Result<profile_login::LoginStatus, String> {
    tauri::async_runtime::spawn_blocking(move || profile_login::status(&id)).await
        .map_err(|_| "Cannot check sign-in.".to_string())?
}

#[tauri::command]
async fn cancel_codex_profile_login(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || profile_login::cancel(&id)).await
        .map_err(|_| "Cannot cancel sign-in.".to_string())?
}

#[tauri::command]
fn schedule_codex_restart(app: AppHandle, id: String, allow_limited: Option<bool>) -> Result<desktop_switch::SwitchStatus, String> {
    app.state::<desktop_switch::DesktopSwitch>().schedule(app.clone(), &id, allow_limited.unwrap_or(false))
}
#[tauri::command]
fn codex_restart_status(app: AppHandle) -> Option<desktop_switch::SwitchStatus> { app.state::<desktop_switch::DesktopSwitch>().status() }
#[tauri::command]
fn cancel_codex_restart(app: AppHandle) -> Result<Option<desktop_switch::SwitchStatus>, String> { app.state::<desktop_switch::DesktopSwitch>().cancel() }
#[tauri::command]
fn run_codex_restart_now(app: AppHandle) -> Result<Option<desktop_switch::SwitchStatus>, String> { app.state::<desktop_switch::DesktopSwitch>().run_now() }

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---------- menubar shell ----------

#[derive(Default)]
struct PanelFileImport(std::sync::atomic::AtomicBool);

#[tauri::command]
fn set_panel_file_import(window: tauri::Window, active: bool) {
    if window.label() == "main" {
        window.state::<PanelFileImport>().0.store(active, std::sync::atomic::Ordering::Relaxed);
    }
}

fn dismiss_panel_on_blur(import_active: bool) -> bool {
    !import_active
}

#[cfg(test)]
mod panel_focus_tests {
    use super::dismiss_panel_on_blur;

    #[test]
    fn finder_focus_does_not_hide_an_open_file_import() {
        assert!(!dismiss_panel_on_blur(true), "Import file must stay visible when Finder takes focus");
    }

    #[test]
    fn leaving_import_restores_normal_panel_dismissal() {
        assert!(dismiss_panel_on_blur(false));
    }
}

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
    // Keep the mounted webview and its current screen on every tray click.
    // Vite HMR handles source changes; data refresh is manual or scheduled.
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(desktop_switch::DesktopSwitch::default())
        .manage(PanelFileImport::default())
        .on_page_load(|webview, payload| {
            // A full Vite reload must not retain a previous form's import state.
            if webview.label() == "main" && payload.event() == tauri::webview::PageLoadEvent::Started {
                webview.state::<PanelFileImport>().0.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_panel(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            set_panel_file_import,
            get_state,
            create_workspace,
            create_discovered_workspace,
            discover_workspace_accounts,
            include_workspace_provider,
            begin_account_setup, discard_account_setup, load_setup_accounts,
            start_setup_login, poll_setup_login, cancel_setup_login, submit_setup_code,
            import_setup_account, connect_setup_config, connect_current_google, commit_account_setup,
            remove_workspace_account, antigravity_accounts, antigravity_account_quota, rename_antigravity_account,
            import_workspace_account,
            update_workspace,
            select_workspace,
            move_workspace_account,
            set_settings,
            set_app_settings,
            set_codex_account_preferences,
            set_menu_bar_accent,
            cli_status,
            cli_quotas,
            codex_reset_status,
            codex_profiles,
            claude_profiles,
            claude_profile_quota,
            create_claude_profile,
            rescan_claude_profile,
            edit_claude_profile,
            create_codex_profile,
            rename_codex_profile,
            remove_codex_profile,
            activate_codex_profile,
            import_codex_profile,
            import_codex_profile_json,
            codex_account_status,
            codex_account_quota,
            codex_profile_quota,
            start_codex_profile_login,
            codex_profile_login_status,
            cancel_codex_profile_login,
            schedule_codex_restart,
            codex_restart_status,
            cancel_codex_restart,
            run_codex_restart_now,
            quit_app
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            // Direct toggle on click — no menu, so nothing intercepts the click.
            let mut builder = TrayIconBuilder::with_id("main");
            builder = builder.icon(tray::render(tray::DEFAULT_ACCENT));
            #[cfg(target_os = "macos")]
            {
                builder = builder.icon_as_template(false);
            }
            let app_handle = app.handle().clone();
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
            tray::start_monitor(app.handle(), settings::read_settings(&paths::settings_file()));
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::ThemeChanged(_) = event {
                    if let Some(monitor) = window.try_state::<tray::MenuBarMonitor>() { monitor.appearance_changed(window.app_handle()); }
                }
                if let WindowEvent::Focused(false) = event {
                    // WebKit's file chooser is app-modal and temporarily takes focus
                    // from the panel. Keep the existing panel visible behind it.
                    #[cfg(target_os = "macos")]
                    if let Some(mtm) = objc2::MainThreadMarker::new() {
                        if objc2_app_kit::NSApplication::sharedApplication(mtm).modalWindow().is_some() {
                            return;
                        }
                    }
                    // Finder takes focus before the user can drag a desktop file.
                    // Keep the mounted import form visible for the whole operation.
                    let import_active = window.state::<PanelFileImport>().0.load(std::sync::atomic::Ordering::Relaxed);
                    if dismiss_panel_on_blur(import_active) {
                        let _ = window.hide();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, event| {
            if matches!(event, tauri::RunEvent::Exit) { profile_login::shutdown();
            account_setup::shutdown(); }
        });
}

#[cfg(test)]
mod import_path_tests {
    use super::resolve_import_path;

    #[test]
    fn accepts_absolute_paths_with_spaces_without_shell_escaping() {
        assert_eq!(resolve_import_path("  /tmp/Work Account/auth.json  ").unwrap(), std::path::PathBuf::from("/tmp/Work Account/auth.json"));
    }

    #[test]
    fn expands_the_current_users_home() {
        assert_eq!(resolve_import_path("~/Downloads/auth.json").unwrap(), dirs::home_dir().unwrap().join("Downloads/auth.json"));
    }

    #[test]
    fn rejects_missing_and_relative_paths() {
        for path in ["", "   ", "auth.json", "~someone/auth.json"] {
            assert!(resolve_import_path(path).is_err());
        }
    }
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
mod workspace_discovery_tests {
    use super::*;

    fn account(id: &str, plan: &str, logged_in: bool) -> cli::ProviderCliStatus {
        cli::ProviderCliStatus {
            id: id.into(), label: id.into(), binary: id.into(), binary_found: true,
            binary_path: None, version: None, logged_in: Some(logged_in), auth_method: None,
            email: None, account: None, detail: None, login_cmd: String::new(), install_url: String::new(),
            auth_file: None, connected: vec![], subscriptions: vec![subscriptions::Subscription {
                id: if id == "codex" { "chatgpt".into() } else { id.into() }, service: id.into(),
                plan: Some(plan.into()), expires_at: None, status: None, monthly_price_usd: None,
            }],
        }
    }

    #[test]
    fn discovery_excludes_free_unknown_and_signed_out_accounts() {
        let result = paid_workspace_accounts(vec![account("codex", "free", true), account("claude", "max", true), account("codex", "plus", false), account("gemini", "unknown", true)]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "claude");
    }

    #[test]
    fn selection_never_implicitly_adds_other_discovered_providers() {
        let candidates = paid_workspace_accounts(vec![account("codex", "plus", true), account("claude", "pro", true)]);
        assert_eq!(validate_workspace_selection(&candidates, vec!["claude".into(), "claude".into()]).unwrap(), vec!["claude"]);
        assert!(validate_workspace_selection(&candidates, vec![]).is_err());
        assert!(validate_workspace_selection(&candidates, vec!["gemini".into()]).is_err());
    }
}
