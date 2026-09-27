//! Keyturn — CLI-only provider status panel (T3-style).
//!
//! Keyturn does NOT authenticate. It never runs a login flow, never opens a
//! sign-in window, never touches the keychain, and never writes any CLI
//! credential file. Each provider's own CLI owns its auth; the user runs the
//! login command in their own terminal. This app only:
//!   1. runs each CLI's read-only status command (`cli.rs`),
//!   2. shows whether the binary is installed and logged in,
//!   3. shows the exact command / install page to copy.
//!
//! The remaining backend owns: profiles, app settings, launch-at-login,
//! local history charts (read from the local usage DB), update checks,
//! sounds, and the tray icon.

pub mod cli;
pub mod codex;
pub mod history;
pub mod notify;
pub mod paths;
pub mod profiles;
pub mod store;
pub mod tray;
pub mod usage;

use serde::Serialize;
use std::time::{Duration, Instant};
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
    pub theme: String,
}

/// Slim app state: no keys, no accounts, no quotas. Auth lives in the CLIs.
#[derive(Debug, Clone, Serialize)]
pub struct AppState {
    pub settings: SettingsOut,
    pub profiles: Vec<profiles::ProfileRef>,
    #[serde(rename = "activeProfile")]
    pub active_profile: String,
    pub app: profiles::AppSettings,
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

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn build_state(app: &AppHandle) -> AppState {
    let root = profiles::read_root(&paths::profiles_file());
    paths::set_active_profile(&root.active);
    let store_path = paths::store_file();
    // One-time v2 cleanup: drop stored keys/logs/accounts from the file.
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
            theme: store.settings.theme.clone(),
        },
        profiles: root.profiles.clone(),
        active_profile: root.active.clone(),
        app: root.app.clone(),
    }
}

/// Background loop: checks for updates every 24 h when enabled.
/// (Wake detection lives in the frontend panel, which refreshes on focus.)
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
        );
    }
}

fn spawn_background_refresh(app: AppHandle) {
    static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    std::thread::spawn(move || {
        let mut last_update_check = Instant::now();
        loop {
            let interval_secs = profiles::read_root(&paths::profiles_file())
                .app
                .refresh_interval_secs
                .clamp(10, 300) as u64;
            std::thread::sleep(Duration::from_secs(interval_secs));

            if BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
                continue;
            }
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
async fn get_state(app: AppHandle) -> Result<AppState, String> {
    Ok(build_state(&app))
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

/// Merge a partial app-settings object (refresh cadence, billing, updates).
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
    Ok(build_state(&app))
}

/// CLI-only provider status (read-only). Never logs in, never writes.
#[tauri::command]
async fn cli_status() -> Vec<cli::ProviderCliStatus> {
    // Status subcommands are blocking + short-lived; keep them off the async
    // runtime. Read-only: never logs in, never writes credentials.
    tauri::async_runtime::spawn_blocking(cli::all_status)
        .await
        .unwrap_or_default()
}

// ---------- profiles ----------

#[tauri::command]
async fn profile_create(app: AppHandle, name: String) -> Result<AppState, String> {
    profiles::create(&paths::profiles_file(), &name)?;
    Ok(build_state(&app))
}

#[tauri::command]
async fn profile_rename(app: AppHandle, id: String, name: String) -> Result<AppState, String> {
    profiles::rename(&paths::profiles_file(), &id, &name)?;
    Ok(build_state(&app))
}

#[tauri::command]
async fn profile_switch(app: AppHandle, id: String) -> Result<AppState, String> {
    profiles::switch(&paths::profiles_file(), &id)?;
    Ok(build_state(&app))
}

#[tauri::command]
async fn profile_delete(app: AppHandle, id: String) -> Result<AppState, String> {
    let root = profiles::delete(&paths::profiles_file(), &id)?;
    paths::set_active_profile(&root.active);
    Ok(build_state(&app))
}

// ---------- history, updates ----------

#[tauri::command]
async fn history(range: String) -> Result<history::HistoryOut, String> {
    let root = profiles::read_root(&paths::profiles_file());
    let (entries, error) = history::read_entries();
    let mut out = history::build(&entries, &range, root.app.billing_day, now_ms());
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
async fn export_history(range: String, format: String) -> Result<String, String> {
    let root = profiles::read_root(&paths::profiles_file());
    let (entries, _) = history::read_entries();
    let out = history::build(&entries, &range, root.app.billing_day, now_ms());
    let path = history::export(&out, &format)?;
    Ok(path.to_string_lossy().into_owned())
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
            set_settings,
            set_app_settings,
            profile_create,
            profile_rename,
            profile_switch,
            profile_delete,
            history,
            export_history,
            check_updates,
            cli_status,
            quit_app
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            // Direct toggle on click — no menu, so nothing intercepts the click.
            let root = profiles::read_root(&paths::profiles_file());
            paths::set_active_profile(&root.active);
            let mut builder = TrayIconBuilder::with_id("main").tooltip("Keyturn");
            builder = builder.icon(tray::render());
            #[cfg(target_os = "macos")]
            {
                builder = builder.icon_as_template(true);
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
