//! Desktop notifications.
//!
//! Quota alerts and alert sounds are gone with CLI-owned auth (there are no
//! stored keys or quota windows to watch). What remains is a thin wrapper
//! for one-shot notices such as "update available".

/// Fire a one-shot notification (e.g. update available).
pub fn send(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}
