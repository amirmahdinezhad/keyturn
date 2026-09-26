//! Profile index + app-wide settings.
//!
//! Each profile owns a store file with its providers, switch log and
//! per-profile settings (theme, signed-in accounts, manual providers). The
//! index file remembers which profiles exist, which one is active, and the
//! app-wide preferences (tray, notifications, automation) that should not
//! change when you switch profiles.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::paths;
use crate::store;

pub const APP_SETTINGS_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRef {
    pub id: String,
    pub name: String,
}

fn default_true() -> bool {
    true
}

fn default_thresholds() -> Vec<u32> {
    vec![75, 90, 95]
}

fn default_refresh() -> u32 {
    60
}

fn default_reset_display() -> String {
    "remaining".to_string()
}

fn default_usage_percent_mode() -> String {
    "used".to_string()
}

fn default_time_format() -> String {
    "system".to_string()
}

fn default_tray_style() -> String {
    "battery".to_string()
}

fn default_color_mode() -> String {
    "mono".to_string()
}

fn default_used_mode() -> String {
    "percent".to_string()
}

fn default_metrics() -> Vec<String> {
    vec!["rolling".to_string(), "weekly".to_string()]
}

fn default_tray_primary() -> String {
    "auto".to_string()
}

fn default_billing_day() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// How reset times are worded: `clock`, `remaining`, or `both`.
    pub reset_display: String,
    /// How quota percentages read in the panel: `used`, `remaining`, or `both`.
    pub usage_percent_mode: String,
    /// `system`, `12h`, or `24h`.
    pub time_format: String,
    /// Panel refresh cadence in seconds (clamped to 10..=300).
    pub refresh_interval_secs: u32,
    /// Switch to the least-used key automatically when one hits its limit.
    pub auto_switch: bool,
    pub notify_enabled: bool,
    pub notify_thresholds: Vec<u32>,
    /// Extra custom threshold on top of the presets, e.g. 80.
    pub notify_custom: Option<u32>,
    pub sound_enabled: bool,
    /// File name inside `/System/Library/Sounds` (macOS) or an absolute path.
    pub sound_name: Option<String>,
    /// `battery`, `bar`, `percent`, `iconbar`, or `compact`.
    pub tray_style: String,
    /// `multi`, `mono`, or `custom`.
    pub tray_color_mode: String,
    /// `#rrggbb` used when `tray_color_mode` is `custom`.
    pub tray_custom_color: String,
    /// `percent` or `tokens`.
    pub tray_used_mode: String,
    /// Show `100 - used` instead of used.
    pub tray_show_remaining: bool,
    pub tray_show_labels: bool,
    pub tray_show_pace: bool,
    /// Quota windows shown in bar/icon styles.
    pub tray_metrics: Vec<String>,
    /// Which window drives the single-value styles: `auto` (first selected
    /// metric), `rolling`, `weekly`, `monthly`, or `worst` (highest percent).
    pub tray_primary: String,
    /// Show several profiles side by side in the menu bar.
    pub tray_multi: bool,
    pub tray_profiles: Vec<String>,
    /// Mark the active profile in multi-profile mode.
    pub tray_active_indicator: bool,
    /// Day of month a billing cycle starts (used by the history chart).
    pub billing_day: u32,
    /// Sparkle-style update checks are opt-in. Kept here so the UI can say so.
    pub auto_update_checks: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            reset_display: default_reset_display(),
            usage_percent_mode: default_usage_percent_mode(),
            time_format: default_time_format(),
            refresh_interval_secs: default_refresh(),
            auto_switch: false,
            notify_enabled: false,
            notify_thresholds: default_thresholds(),
            notify_custom: None,
            sound_enabled: true,
            sound_name: None,
            tray_style: default_tray_style(),
            tray_color_mode: default_color_mode(),
            tray_custom_color: "#0a84ff".to_string(),
            tray_used_mode: default_used_mode(),
            tray_show_remaining: false,
            tray_show_labels: true,
            tray_show_pace: true,
            tray_metrics: default_metrics(),
            tray_primary: default_tray_primary(),
            tray_multi: false,
            tray_profiles: Vec::new(),
            tray_active_indicator: true,
            billing_day: default_billing_day(),
            auto_update_checks: false,
        }
    }
}

impl AppSettings {
    /// Keep every value inside the range the UI and the tray renderer expect.
    pub fn normalize(&mut self) {
        self.refresh_interval_secs = self.refresh_interval_secs.clamp(10, 300);
        self.billing_day = self.billing_day.clamp(1, 28);
        self.notify_thresholds.retain(|t| (1..=100).contains(t));
        self.notify_thresholds.sort_unstable();
        self.notify_thresholds.dedup();
        if self.notify_thresholds.is_empty() {
            self.notify_thresholds = default_thresholds();
        }
        if let Some(custom) = self.notify_custom {
            self.notify_custom = (custom <= 100).then_some(custom);
        }
        if !matches!(self.reset_display.as_str(), "clock" | "remaining" | "both") {
            self.reset_display = default_reset_display();
        }
        if !matches!(self.time_format.as_str(), "system" | "12h" | "24h") {
            self.time_format = default_time_format();
        }
        if !matches!(
            self.usage_percent_mode.as_str(),
            "used" | "remaining" | "both"
        ) {
            self.usage_percent_mode = default_usage_percent_mode();
        }
        if !matches!(
            self.tray_style.as_str(),
            "battery" | "bar" | "percent" | "iconbar" | "compact"
        ) {
            self.tray_style = default_tray_style();
        }
        if !matches!(self.tray_color_mode.as_str(), "multi" | "mono" | "custom") {
            self.tray_color_mode = default_color_mode();
        }
        if !matches!(self.tray_used_mode.as_str(), "percent" | "tokens") {
            self.tray_used_mode = default_used_mode();
        }
        self.tray_metrics
            .retain(|m| matches!(m.as_str(), "rolling" | "weekly" | "monthly" | "api"));
        if self.tray_metrics.is_empty() {
            self.tray_metrics = default_metrics();
        }
        if !matches!(
            self.tray_primary.as_str(),
            "auto" | "rolling" | "weekly" | "monthly" | "worst"
        ) {
            self.tray_primary = default_tray_primary();
        }
        if !self.tray_custom_color.starts_with('#') || self.tray_custom_color.len() != 7 {
            self.tray_custom_color = "#0a84ff".to_string();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub active: String,
    pub profiles: Vec<ProfileRef>,
    #[serde(default)]
    pub app: AppSettings,
}

impl Default for Root {
    fn default() -> Self {
        Root {
            active: paths::DEFAULT_PROFILE.to_string(),
            profiles: vec![ProfileRef {
                id: paths::DEFAULT_PROFILE.to_string(),
                name: "Default".to_string(),
            }],
            app: AppSettings::default(),
        }
    }
}

impl Root {
    fn normalize(&mut self) {
        if self.profiles.is_empty() {
            self.profiles.push(ProfileRef {
                id: paths::DEFAULT_PROFILE.to_string(),
                name: "Default".to_string(),
            });
        }
        if !self.profiles.iter().any(|p| p.id == self.active) {
            self.active = self.profiles[0].id.clone();
        }
        self.app.normalize();
    }
}

pub fn normalize_root(raw: Option<Value>, legacy_open_at_login: Option<bool>) -> Root {
    let mut root = match raw {
        Some(Value::Object(map)) => {
            let active = map
                .get("active")
                .and_then(|v| v.as_str())
                .unwrap_or(paths::DEFAULT_PROFILE)
                .to_string();
            let profiles = map
                .get("profiles")
                .and_then(|v| v.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|p| {
                            let id = p.get("id").and_then(|v| v.as_str())?;
                            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or(id);
                            Some(ProfileRef {
                                id: id.to_string(),
                                name: name.to_string(),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let app = map
                .get("app")
                .cloned()
                .and_then(|v| serde_json::from_value::<AppSettings>(v).ok())
                .unwrap_or_default();
            Root {
                active,
                profiles,
                app,
            }
        }
        _ => Root {
            active: paths::DEFAULT_PROFILE.to_string(),
            profiles: Vec::new(),
            app: AppSettings {
                // Carry the old per-store launch-at-login flag forward once.
                ..AppSettings::default()
            },
        },
    };
    let _ = legacy_open_at_login;
    root.normalize();
    root
}

static ROOT_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

pub fn read_root(path: &Path) -> Root {
    normalize_root(paths::read_json(path), None)
}

pub fn write_root(path: &Path, root: &Root) -> Root {
    let mut value = serde_json::to_value(root).expect("serialize root");
    if let Some(app) = value.get_mut("app") {
        *app = serde_json::to_value(&root.app).expect("serialize app settings");
    }
    paths::atomic_write_json(path, &value);
    read_root(path)
}

pub fn update_root<F, R>(path: &Path, mutate: F) -> Result<(Root, R), String>
where
    F: FnOnce(&mut Root) -> Result<R, String>,
{
    let _guard = ROOT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut root = read_root(path);
    let out = mutate(&mut root)?;
    root.normalize();
    let saved = write_root(path, &root);
    Ok((saved, out))
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 24 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

fn unique_id(root: &Root, base: &str) -> String {
    let base = if base.is_empty() { "profile" } else { base };
    if !root.profiles.iter().any(|p| p.id == base) {
        return base.to_string();
    }
    for n in 2..100 {
        let candidate = format!("{base}-{n}");
        if !root.profiles.iter().any(|p| p.id == candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", root.profiles.len())
}

pub const MAX_PROFILES: usize = 20;

pub fn create(path: &Path, name: &str) -> Result<(Root, ProfileRef), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Profile name must not be empty".to_string());
    }
    let (root, created) = update_root(path, |root| {
        if root.profiles.len() >= MAX_PROFILES {
            return Err(format!("At most {MAX_PROFILES} profiles are supported"));
        }
        let id = unique_id(root, &slug(trimmed));
        if id == paths::DEFAULT_PROFILE {
            return Err("That name is reserved".to_string());
        }
        let created = ProfileRef {
            id,
            name: trimmed.to_string(),
        };
        root.profiles.push(created.clone());
        Ok(created)
    })?;
    // Materialize the profile's store so a crash right after creation still
    // leaves a complete set of files.
    let store_path = paths::store_file_for(&created.id);
    if !store_path.exists() {
        store::write_store(&store_path, &store::Store::default());
    }
    Ok((root, created))
}

pub fn rename(path: &Path, id: &str, name: &str) -> Result<Root, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Profile name must not be empty".to_string());
    }
    update_root(path, |root| {
        let profile = root
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("No profile \"{id}\""))?;
        profile.name = trimmed.to_string();
        Ok(())
    })
    .map(|(root, ())| root)
}

pub fn delete(path: &Path, id: &str) -> Result<Root, String> {
    let (root, ()) = update_root(path, |root| {
        if root.profiles.len() <= 1 {
            return Err("Keep at least one profile".to_string());
        }
        let before = root.profiles.len();
        root.profiles.retain(|p| p.id != id);
        if root.profiles.len() == before {
            return Err(format!("No profile \"{id}\""));
        }
        if root.active == id {
            root.active = root.profiles[0].id.clone();
        }
        Ok(())
    })?;
    let _ = std::fs::remove_file(paths::store_file_for(id));
    Ok(root)
}

pub fn switch(path: &Path, id: &str) -> Result<Root, String> {
    let root = update_root(path, |root| {
        if !root.profiles.iter().any(|p| p.id == id) {
            return Err(format!("No profile \"{id}\""));
        }
        root.active = id.to_string();
        Ok(())
    })
    .map(|(root, ())| root)?;
    paths::set_active_profile(&root.active);
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane_and_are_clamped() {
        let mut app = AppSettings::default();
        app.refresh_interval_secs = 1;
        app.billing_day = 99;
        app.notify_thresholds = vec![90, 75, 75, 500];
        app.tray_style = "hologram".to_string();
        app.tray_custom_color = "blue".to_string();
        app.normalize();
        assert_eq!(app.refresh_interval_secs, 10);
        assert_eq!(app.billing_day, 28);
        assert_eq!(app.notify_thresholds, vec![75, 90]);
        assert_eq!(app.tray_style, "battery");
        assert_eq!(app.tray_custom_color, "#0a84ff");
    }

    #[test]
    fn create_rename_switch_and_delete_round_trip() {
        let dir = std::env::temp_dir().join(format!("aks-profiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let index = dir.join("agent-rtl.profiles.json");

        let root = read_root(&index);
        assert_eq!(root.profiles.len(), 1);
        assert_eq!(root.active, paths::DEFAULT_PROFILE);

        let (root, work) = create(&index, "Work Laptop").unwrap();
        assert_eq!(work.id, "work-laptop");
        assert_eq!(root.profiles.len(), 2);

        let (_, again) = create(&index, "work laptop").unwrap();
        assert_eq!(again.id, "work-laptop-2");

        let root = switch(&index, "work-laptop").unwrap();
        assert_eq!(root.active, "work-laptop");
        assert_eq!(paths::active_profile(), "work-laptop");
        assert!(paths::store_file().ends_with("agent-rtl.keys.work-laptop"));

        let root = rename(&index, "work-laptop", "Studio").unwrap();
        assert_eq!(root.profiles[1].name, "Studio");

        let root = delete(&index, "work-laptop").unwrap();
        assert_eq!(root.active, paths::DEFAULT_PROFILE);
        assert_eq!(root.profiles.len(), 2, "the duplicate profile stays");
        let root = delete(&index, "work-laptop-2").unwrap();
        assert_eq!(root.profiles.len(), 1);
        assert!(delete(&index, paths::DEFAULT_PROFILE).is_err());

        paths::set_active_profile(paths::DEFAULT_PROFILE);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_index_migrates_to_default_profile() {
        let root = normalize_root(None, Some(true));
        assert_eq!(root.active, "default");
        assert_eq!(root.profiles.len(), 1);
        assert_eq!(root.app.notify_thresholds, vec![75, 90, 95]);
    }
}
