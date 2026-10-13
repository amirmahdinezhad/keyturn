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
//! The backend owns app settings, launch-at-login, read-only quota sources
//! and the tray panel. The frontend uses the updater plugin directly.

pub mod cli;
pub mod claude_usage;
pub mod codex;
pub mod paths;
pub mod quota;
pub mod settings;
pub mod store;
pub mod tray;

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

/// Slim app state: no profiles, no theme, no keys, no quotas.
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

/// Merge partial app preferences (refresh cadence and quota wording).
#[tauri::command]
async fn set_app_settings(app: AppHandle, settings: serde_json::Value) -> Result<AppState, String> {
    let path = paths::settings_file();
    settings::update_settings(&path, |current| {
        let mut merged = serde_json::to_value(&current).map_err(|e| e.to_string())?;
        if let (Some(dst), Some(src)) = (merged.as_object_mut(), settings.as_object()) {
            for (key, value) in src {
                dst.insert(key.clone(), value.clone());
            }
        }
        *current = serde_json::from_value(merged).map_err(|e| format!("Invalid settings: {e}"))?;
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
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_settings,
            set_app_settings,
            cli_status,
            cli_quotas,
            quit_app
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            // Direct toggle on click — no menu, so nothing intercepts the click.
            let mut builder = TrayIconBuilder::with_id("main").tooltip("Keyturn");
            builder = builder.icon(tray::render());
            #[cfg(target_os = "macos")]
            {
                builder = builder.icon_as_template(true);
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
