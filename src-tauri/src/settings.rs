//! App-wide settings (single file, no profiles).
//!
//! Profiles lived here in v3 and are gone: one user, one settings file
//! (`agent-rtl.settings.json`). Only live preferences remain — panel refresh
//! cadence, history billing day and update checks.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::paths;

fn default_refresh() -> u32 {
    60
}

fn default_billing_day() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Panel refresh cadence in seconds (clamped to 10..=300).
    pub refresh_interval_secs: u32,
    /// Day of month a billing cycle starts (used by the history chart).
    pub billing_day: u32,
    /// Update checks are opt-in. Kept here so the UI can say so.
    pub auto_update_checks: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            refresh_interval_secs: default_refresh(),
            billing_day: default_billing_day(),
            auto_update_checks: false,
        }
    }
}

impl AppSettings {
    /// Keep every value inside the range the UI expects.
    pub fn normalize(&mut self) {
        self.refresh_interval_secs = self.refresh_interval_secs.clamp(10, 300);
        self.billing_day = self.billing_day.clamp(1, 28);
    }
}

pub fn read_settings(path: &Path) -> AppSettings {
    let mut settings: AppSettings = paths::read_json(path)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    settings.normalize();
    settings
}

pub fn write_settings(path: &Path, settings: &AppSettings) -> AppSettings {
    let mut normalized = settings.clone();
    normalized.normalize();
    paths::atomic_write_json(
        path,
        &serde_json::to_value(&normalized).expect("serialize settings"),
    );
    read_settings(path)
}

static SETTINGS_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

/// Read, mutate and persist the settings under one lock.
pub fn update_settings<F, R>(path: &Path, mutate: F) -> Result<(AppSettings, R), String>
where
    F: FnOnce(&mut AppSettings) -> Result<R, String>,
{
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut settings = read_settings(path);
    let out = mutate(&mut settings)?;
    let saved = write_settings(path, &settings);
    Ok((saved, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane_and_are_clamped() {
        let mut app = AppSettings::default();
        app.refresh_interval_secs = 1;
        app.billing_day = 99;
        app.normalize();
        assert_eq!(app.refresh_interval_secs, 10);
        assert_eq!(app.billing_day, 28);
    }

    #[test]
    fn malformed_files_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("keyturn-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("agent-rtl.settings.json");
        std::fs::write(&file, "{not json").unwrap();
        let settings = read_settings(&file);
        assert_eq!(settings.refresh_interval_secs, 60);
        let (saved, ()) = update_settings(&file, |s| {
            s.billing_day = 15;
            Ok(())
        })
        .unwrap();
        assert_eq!(saved.billing_day, 15);
        std::fs::remove_dir_all(&dir).ok();
    }
}
