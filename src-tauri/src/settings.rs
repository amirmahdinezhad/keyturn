//! App-wide settings (single file, no profiles).
//!
//! One settings file (`agent-rtl.settings.json`) holds panel refresh cadence
//! and used/remaining quota wording. Legacy preferences are compacted on read.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::paths;

fn default_refresh() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Panel refresh cadence in seconds (clamped to 10..=300).
    pub refresh_interval_secs: u32,
    /// Show remaining quota instead of used quota in percent displays.
    pub usage_show_remaining: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            refresh_interval_secs: default_refresh(),
            usage_show_remaining: false,
        }
    }
}

impl AppSettings {
    /// Keep every value inside the range the UI expects.
    pub fn normalize(&mut self) {
        self.refresh_interval_secs = self.refresh_interval_secs.clamp(10, 300);
    }
}

pub fn read_settings(path: &Path) -> AppSettings {
    let raw = paths::read_json(path);
    let mut settings: AppSettings = raw
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    settings.normalize();
    // Compact legacy files once: unknown keys are dropped on rewrite.
    if let Some(serde_json::Value::Object(map)) = raw {
        if map
            .keys()
            .any(|k| k != "refreshIntervalSecs" && k != "usageShowRemaining")
        {
            write_settings(path, &settings);
        }
    }
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
        app.normalize();
        assert_eq!(app.refresh_interval_secs, 10);
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
            s.refresh_interval_secs = 120;
            Ok(())
        })
        .unwrap();
        assert_eq!(saved.refresh_interval_secs, 120);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_keys_are_compacted_on_read() {
        let dir =
            std::env::temp_dir().join(format!("keyturn-settings-legacy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("agent-rtl.settings.json");
        std::fs::write(
            &file,
            r#"{"refreshIntervalSecs":30,"billingDay":15,"autoUpdateChecks":true}"#,
        )
        .unwrap();
        let settings = read_settings(&file);
        assert_eq!(settings.refresh_interval_secs, 30);
        let raw = std::fs::read_to_string(&file).unwrap();
        assert!(!raw.contains("billingDay"), "legacy keys must be gone");
        assert!(!raw.contains("autoUpdateChecks"), "legacy keys must be gone");
        std::fs::remove_dir_all(&dir).ok();
    }
}
