//! Machine preferences and workspace metadata share one atomic settings file.
//!
//! One settings file (`agent-rtl.settings.json`) holds panel refresh cadence
//! quota wording, menu-bar display and workspace membership. Legacy preferences are compacted on read.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::collections::BTreeMap;

use crate::paths;

fn default_refresh() -> u32 {
    60
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MenuBarProvider {
    #[default]
    Codex,
    Claude,
    Opencode,
    OpencodeGo,
}

impl MenuBarProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
            Self::OpencodeGo => "opencode-go",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Opencode => "OpenCode Zen",
            Self::OpencodeGo => "OpenCode Go",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MenuBarWindow {
    Auto,
    #[default]
    #[serde(rename = "5-hour")]
    FiveHour,
    Weekly,
    Monthly,
}

/// Per-workspace rendering; old workspaces retain their numeric percentages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum MenuBarStyle {
    #[default]
    Percentages,
    Progress,
}

/// Shared appearance preferences for numeric metrics and usage bars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UsageAppearance {
    pub monochrome: bool,
    pub show_labels: bool,
    pub show_remaining: bool,
    pub session_enabled: bool,
    pub weekly_enabled: bool,
    pub show_time_marker: bool,
    pub color_marker_by_pace: bool,
    pub pace_aware_colors: bool,
}
impl Default for UsageAppearance {
    fn default() -> Self {
        Self {
            monochrome: true,
            show_labels: true,
            show_remaining: true,
            session_enabled: true,
            weekly_enabled: false,
            show_time_marker: true,
            color_marker_by_pace: true,
            pace_aware_colors: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MultiProfileSettings {
    pub enabled: bool,
    pub selected_ids: Vec<String>,
    pub show_weekly: bool,
    pub show_profile_label: bool,
    pub monochrome: bool,
    pub show_remaining: bool,
    pub show_active_indicator: bool,
}
impl Default for MultiProfileSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            selected_ids: Vec::new(),
            show_weekly: false,
            show_profile_label: true,
            monochrome: true,
            show_remaining: true,
            show_active_indicator: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Panel refresh cadence in seconds (clamped to 10..=300).
    pub refresh_interval_secs: u32,
    /// Count remaining quota down from 100% by default; false shows used quota.
    pub usage_show_remaining: bool,
    /// Opt-in native title; its refresh continues while the panel is hidden.
    pub menu_bar_enabled: bool,
    pub menu_bar_provider: MenuBarProvider,
    pub menu_bar_window: MenuBarWindow,
    pub provider_accents: BTreeMap<String, String>,
    pub usage_appearance: UsageAppearance,
    pub multi_profile: MultiProfileSettings,
    pub auto_switch_enabled: bool,
    pub codex_display_mode: CodexDisplayMode,
    pub codex_accounts: BTreeMap<String, CodexAccountPreferences>,
    pub codex_reset: CodexResetSettings,
    pub workspace_version: u8,
    pub active_workspace_id: Option<String>,
    pub workspaces: Vec<crate::workspaces::Workspace>,
}

/// Public outlook polling is independent of quota refresh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodexResetSettings {
    pub show_probable_resets: bool,
    pub refetch_interval_secs: u32,
}

impl Default for CodexResetSettings {
    fn default() -> Self {
        Self { show_probable_resets: true, refetch_interval_secs: 300 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexDisplayMode {
    #[default]
    Active,
    Selected,
}

/// Preferences are intentionally not copied from the legacy shared settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodexAccountPreferences {
    pub menu_bar_enabled: bool,
    pub menu_bar_windows: Vec<String>,
    pub known_windows: Vec<String>,
    pub color: String,
    pub show_labels: bool,
    pub show_profile_label: bool,
    pub show_active_indicator: bool,
    pub show_time_marker: bool,
    pub color_marker_by_pace: bool,
    pub pace_aware_colors: bool,
    pub warning_colors: bool,
    pub auto_switch: bool,
}

impl Default for CodexAccountPreferences {
    fn default() -> Self {
        Self {
            menu_bar_enabled: true,
            menu_bar_windows: Vec::new(),
            known_windows: Vec::new(),
            color: "#007aff".into(),
            show_labels: true,
            show_profile_label: true,
            show_active_indicator: true,
            show_time_marker: true,
            color_marker_by_pace: true,
            pace_aware_colors: false,
            warning_colors: false,
            auto_switch: false,
        }
    }
}

/// Main windows come from returned durations, including nonstandard provider windows.
pub fn canonical_account_window(value: &str) -> Option<String> {
    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "rolling" | "5-hour" => Some("5-hour".into()),
        "weekly" | "monthly" => Some(value),
        _ if value.is_ascii() && value.len() <= 12 && value.len() > 1 => {
            let (number, unit) = value.split_at(value.len() - 1);
            (matches!(unit, "m" | "h" | "d")
                && number.parse::<u32>().is_ok_and(|n| n > 0))
                .then_some(value)
        }
        _ => None,
    }
}

impl CodexAccountPreferences {
    pub(crate) fn normalize(&mut self) {
        for values in [&mut self.known_windows, &mut self.menu_bar_windows] {
            *values = values.iter().filter_map(|v| canonical_account_window(v)).collect();
            values.sort();
            values.dedup();
            values.truncate(16);
        }
        let hex = self.color.strip_prefix('#').unwrap_or("");
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            self.color = "#007aff".into();
        } else {
            self.color.make_ascii_lowercase();
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            refresh_interval_secs: default_refresh(),
            usage_show_remaining: true,
            menu_bar_enabled: false,
            menu_bar_provider: MenuBarProvider::default(),
            menu_bar_window: MenuBarWindow::default(),
            provider_accents: BTreeMap::new(),
            usage_appearance: UsageAppearance::default(),
            multi_profile: MultiProfileSettings::default(),
            auto_switch_enabled: false,
            codex_display_mode: CodexDisplayMode::default(),
            codex_accounts: BTreeMap::new(),
            codex_reset: CodexResetSettings::default(),
            workspace_version: 0,
            active_workspace_id: None,
            workspaces: Vec::new(),
        }
    }
}

impl AppSettings {
    pub fn active_workspace(&self) -> Option<&crate::workspaces::Workspace> {
        self.workspaces.iter().find(|w| Some(&w.id) == self.active_workspace_id.as_ref())
    }
    /// Keep legacy consumers scoped to the selected workspace without sharing its overrides.
    pub fn project_workspace_preferences(&mut self) {
        if let Some(prefs) = self.active_workspace().and_then(|w| w.account_preferences.clone()) { self.codex_accounts = prefs; }
    }
    pub fn contains_codex_account(&self, id: &str) -> bool {
        self.active_workspace().map_or(self.workspace_version == 0, |w| w.account_ids.iter().any(|account| account == id))
    }
    pub fn codex_menu_bar_enabled(&self) -> bool {
        self.active_workspace().map_or(self.menu_bar_enabled && self.menu_bar_provider == MenuBarProvider::Codex, |w| w.usage.menu_bar_enabled)
    }
    /// Colors are independent of removed usage-notification preferences.
    pub fn menu_bar_warning_thresholds(&self) -> &[u8] {
        self.active_workspace().and_then(|w| w.usage.warning_thresholds.as_deref()).unwrap_or(&[75, 90, 95])
    }
    pub fn workspace_display_mode(&self) -> CodexDisplayMode {
        self.active_workspace().map_or(self.codex_display_mode, |w| w.usage.display_mode)
    }
    /// Keep every value inside the range the UI expects.
    pub fn normalize(&mut self) {
        self.refresh_interval_secs = self.refresh_interval_secs.clamp(10, 300);
        self.provider_accents.retain(|id, color| {
            matches!(id.as_str(), "codex" | "claude" | "opencode" | "opencode-go" | "gemini")
                && color.len() == 7 && color.starts_with('#') && color[1..].bytes().all(|c| c.is_ascii_hexdigit())
        });
        for color in self.provider_accents.values_mut() {
            color.make_ascii_lowercase();
        }
        self.codex_reset.refetch_interval_secs = self.codex_reset.refetch_interval_secs.clamp(30, 3600);
        for workspace in &mut self.workspaces {
            if let Some(levels) = &mut workspace.usage.warning_thresholds {
                levels.retain(|v| (1..=100).contains(v));
                levels.sort_unstable();
                levels.dedup();
            }
            workspace.usage.auto_switch_remaining = workspace.usage.auto_switch_remaining.min(100);
            if !matches!(workspace.usage.menu_bar_provider.as_str(), "codex" | "claude" | "antigravity") { workspace.usage.menu_bar_provider = "codex".into(); }
            if let Some(prefs) = &mut workspace.account_preferences { prefs.retain(|id,_| uuid::Uuid::parse_str(id).is_ok()); for value in prefs.values_mut() { value.normalize(); } }
        }
        self.codex_accounts.retain(|id, _| uuid::Uuid::parse_str(id).is_ok());
        for prefs in self.codex_accounts.values_mut() {
            prefs.normalize();
        }
        self.multi_profile
            .selected_ids
            .retain(|id| uuid::Uuid::parse_str(id).is_ok());
        self.multi_profile.selected_ids.sort();
        self.multi_profile.selected_ids.dedup();
    }
}

pub fn read_settings(path: &Path) -> AppSettings {
    let raw = paths::read_json(path);
    let mut settings: AppSettings = raw
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    settings.normalize();
    for workspace in &mut settings.workspaces {
        if workspace.account_preferences.is_none() {
            workspace.account_preferences = Some(settings.codex_accounts.iter().filter(|(id,_)| workspace.account_ids.contains(id)).map(|(id,prefs)|(id.clone(),prefs.clone())).collect());
        }
    }
    settings.project_workspace_preferences();
    // Compact legacy files once: unknown keys are dropped on rewrite.
    if let Some(serde_json::Value::Object(map)) = raw {
        if map.keys().any(|k| {
            !matches!(
                k.as_str(),
                "refreshIntervalSecs"
                    | "usageShowRemaining"
                    | "menuBarEnabled"
                    | "menuBarProvider"
                    | "menuBarWindow"
                    | "providerAccents"
                    | "usageAppearance"
                    | "multiProfile"
                    | "autoSwitchEnabled"
                    | "codexDisplayMode"
                    | "codexAccounts"
                    | "codexReset"
                    | "workspaceVersion"
                    | "activeWorkspaceId"
                    | "workspaces"
            )
        }) {
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

/// Machine-wide updates cannot reset workspace membership or account-owned preferences.
pub fn merge_global(current: &mut AppSettings, patch: &serde_json::Value) -> Result<(), String> {
    let mut merged = serde_json::to_value(&current).map_err(|e| e.to_string())?;
    if let (Some(dst), Some(src)) = (merged.as_object_mut(), patch.as_object()) {
        for (key, value) in src {
            if matches!(key.as_str(), "workspaces" | "activeWorkspaceId" | "workspaceVersion" | "codexAccounts") { continue; }
            dst.insert(key.clone(), value.clone());
        }
    }
    *current = serde_json::from_value(merged).map_err(|e| format!("Invalid settings: {e}"))?;
    Ok(())
}

/// Read, mutate and persist the settings under one lock.
pub fn update_settings<F, R>(path: &Path, mutate: F) -> Result<(AppSettings, R), String>
where
    F: FnOnce(&mut AppSettings) -> Result<R, String>,
{
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut settings = read_settings(path);
    let out = mutate(&mut settings)?;
    settings.normalize();
    crate::profiles::write_private(path, &serde_json::to_value(&settings).map_err(|_| "Cannot save settings.")?)?;
    let saved = read_settings(path);
    Ok((saved, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_levels_round_trip_independently_of_removed_alerts() {
        let mut app: AppSettings = serde_json::from_value(serde_json::json!({
            "activeWorkspaceId":"studio", "workspaces":[{"id":"studio","name":"Studio","accent":"#007aff","accountIds":[],
            "usage":{"warningColors":true,"warningThresholds":[95,0,75,75,110,55]}}]
        })).unwrap();
        app.normalize();
        assert_eq!(app.menu_bar_warning_thresholds(), [55,75,95]);
        let mut saved: AppSettings = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
        assert_eq!(saved.menu_bar_warning_thresholds(), [55,75,95]);
        saved.workspaces[0].usage.warning_thresholds = Some(vec![]);
        assert!(saved.menu_bar_warning_thresholds().is_empty());
        saved.workspaces[0].usage.warning_thresholds = None;
        assert_eq!(saved.menu_bar_warning_thresholds(), [75,90,95]);
    }

    #[test]
    fn removed_alert_preferences_are_ignored_and_not_saved() {
        let mut app: AppSettings = serde_json::from_value(serde_json::json!({
            "notifications":{"enabled":true}, "activeWorkspaceId":"studio",
            "workspaces":[{"id":"studio","name":"Studio","accent":"#007aff","accountIds":[],
                "usage":{"alertsEnabled":true,"alertThresholds":[50],"alertSessionReset":true}}]
        })).unwrap();
        app.normalize();
        let value = serde_json::to_value(&app).unwrap();
        assert!(value.get("notifications").is_none());
        let usage = &value["workspaces"][0]["usage"];
        for field in ["alertsEnabled", "alertThresholds", "alertSessionReset"] { assert!(usage.get(field).is_none()); }
        assert_eq!(usage["showCodexResets"], true);
    }

    #[test]
    fn workspace_codex_selection_and_provider_colors_survive_round_trip() {
        let dir = std::env::temp_dir().join(format!("keyturn-provider-colors-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = AppSettings { workspace_version: 1, menu_bar_provider: MenuBarProvider::Codex, ..Default::default() };
        app.provider_accents = BTreeMap::from([
            ("claude".into(), "#7C6CF0".into()),
            ("gemini".into(), "invalid".into()),
            ("unknown".into(), "#007aff".into()),
        ]);
        let saved = write_settings(&dir.join("settings.json"), &app);
        assert_eq!(saved.menu_bar_provider, MenuBarProvider::Codex);
        assert_eq!(saved.provider_accents, BTreeMap::from([("claude".into(), "#7c6cf0".into())]));
        assert!(serde_json::from_str::<AppSettings>("{}").unwrap().provider_accents.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn global_reset_preserves_workspace_policy_membership_and_account_preferences() {
        let mut app = AppSettings { workspace_version: 1, active_workspace_id: Some("studio".into()), refresh_interval_secs: 120, ..Default::default() };
        let id = uuid::Uuid::new_v4().to_string();
        app.workspaces.push(crate::workspaces::Workspace { id: "studio".into(), name: "Studio".into(), accent: "#7c6cf0".into(), account_ids: vec![id.clone()], provider_ids: None, claude_account_ids: None, antigravity_account_ids: vec![], account_preferences: None, usage: crate::workspaces::WorkspaceUsage { ..Default::default() } });
        app.codex_accounts.insert(id.clone(), CodexAccountPreferences { color: "#2f9e6e".into(), ..Default::default() });
        let workspaces = app.workspaces.clone();
        merge_global(&mut app, &serde_json::to_value(AppSettings::default()).unwrap()).unwrap();
        assert_eq!(app.refresh_interval_secs, 60);
        assert_eq!(app.workspace_version, 1);
        assert_eq!(app.active_workspace_id.as_deref(), Some("studio"));
        assert_eq!(app.workspaces, workspaces);
        assert_eq!(app.codex_accounts[&id].color, "#2f9e6e");
    }

    #[test]
    fn reset_outlook_defaults_normalize_and_persist_independently() {
        let mut app: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(app.codex_reset.show_probable_resets);
        assert_eq!(app.codex_reset.refetch_interval_secs, 300);
        app.codex_reset.refetch_interval_secs = 1;
        app.normalize();
        assert_eq!(app.codex_reset.refetch_interval_secs, 30);
        app.codex_reset.refetch_interval_secs = 9999;
        app.normalize();
        assert_eq!(app.codex_reset.refetch_interval_secs, 3600);
        let dir = std::env::temp_dir().join(format!("keyturn-reset-prefs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        app.codex_reset.show_probable_resets = false;
        app.codex_reset.refetch_interval_secs = 60;
        write_settings(&file, &app);
        let saved = read_settings(&file);
        assert!(!saved.codex_reset.show_probable_resets);
        assert_eq!(saved.codex_reset.refetch_interval_secs, 60);
        assert_eq!(saved.refresh_interval_secs, 60);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_shared_settings_do_not_configure_or_enroll_codex_accounts() {
        let app: AppSettings = serde_json::from_str(r#"{"menuBarEnabled":true,"notifications":{"enabled":true},"usageAppearance":{"weeklyEnabled":true},"autoSwitchEnabled":true}"#).unwrap();
        assert!(app.codex_accounts.is_empty());
        assert_eq!(app.codex_display_mode, CodexDisplayMode::Active);
        let prefs = CodexAccountPreferences::default();
        assert!(prefs.menu_bar_windows.is_empty());
        assert!(!prefs.auto_switch);
    }

    #[test]
    fn account_preferences_are_normalized_and_survive_legacy_cleanup_independently() {
        let dir = std::env::temp_dir().join(format!("keyturn-account-prefs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        std::fs::write(&file, serde_json::to_vec(&serde_json::json!({
            "legacyUnknown": true,
            "codexDisplayMode":"selected",
            "codexAccounts": {
                first.clone(): {"color":"#7C6CF0", "menuBarWindows":["Weekly","gpt-reserve","15m","weekly"], "notifications":{"thresholds":[95,0,50,95,101]}, "autoSwitch":true},
                second.clone(): {"color":"invalid", "menuBarWindows":["5-hour"]},
                "invalid-id": {"autoSwitch":true}
            }
        })).unwrap()).unwrap();
        let app = read_settings(&file);
        assert_eq!(app.codex_accounts.len(), 2);
        assert_eq!(app.codex_accounts[&first].color, "#7c6cf0");
        assert_eq!(app.codex_accounts[&first].menu_bar_windows, ["15m", "weekly"]);
        assert_eq!(app.codex_accounts[&second].color, "#007aff");
        assert!(!app.codex_accounts[&second].auto_switch);
        let saved = paths::read_json(&file).unwrap();
        assert!(saved.get("legacyUnknown").is_none());
        assert_eq!(saved["codexAccounts"][&first]["autoSwitch"], true);
        assert_eq!(saved["codexDisplayMode"], "selected");
        assert!(canonical_account_window("هفتگی").is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_percentage_direction_defaults_to_remaining_everywhere() {
        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(settings.usage_show_remaining);
        assert!(settings.usage_appearance.show_remaining);
        assert!(settings.multi_profile.show_remaining);
    }

    #[test]
    fn explicit_used_direction_survives_save_and_reload() {
        let dir = std::env::temp_dir().join(format!(
            "keyturn-settings-used-direction-{}",
            std::process::id()
        ));
        let file = dir.join("agent-rtl.settings.json");
        let settings: AppSettings = serde_json::from_str(
            r#"{"usageShowRemaining":false,"usageAppearance":{"showRemaining":false},"multiProfile":{"showRemaining":false}}"#,
        )
        .unwrap();
        let saved = write_settings(&file, &settings);
        assert!(!saved.usage_show_remaining);
        assert!(!saved.usage_appearance.show_remaining);
        assert!(!saved.multi_profile.show_remaining);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn older_settings_keep_their_values_with_menu_bar_disabled() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"refreshIntervalSecs":120,"usageShowRemaining":true}"#)
                .unwrap();
        assert_eq!(settings.refresh_interval_secs, 120);
        assert!(settings.usage_show_remaining);
        assert!(!settings.menu_bar_enabled);
        assert_eq!(settings.menu_bar_provider, MenuBarProvider::Codex);
        assert_eq!(settings.menu_bar_window, MenuBarWindow::FiveHour);
    }

    #[test]
    fn menu_bar_preferences_persist_and_survive_legacy_cleanup() {
        let dir =
            std::env::temp_dir().join(format!("keyturn-settings-menu-bar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("agent-rtl.settings.json");
        std::fs::write(&file, r#"{"menuBarEnabled":true,"menuBarProvider":"opencode-go","menuBarWindow":"monthly","billingDay":15}"#).unwrap();
        let settings = read_settings(&file);
        assert!(settings.menu_bar_enabled);
        assert_eq!(settings.menu_bar_provider, MenuBarProvider::OpencodeGo);
        assert_eq!(settings.menu_bar_window, MenuBarWindow::Monthly);
        let raw = paths::read_json(&file).unwrap();
        assert_eq!(raw["menuBarWindow"], "monthly");
        assert_eq!(raw["menuBarProvider"], "opencode-go");
        assert!(raw.get("billingDay").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn usage_preferences_migrate_and_normalize_without_dropping_existing_values() {
        let mut app: AppSettings = serde_json::from_value(serde_json::json!({
            "refreshIntervalSecs":120,"menuBarEnabled":true,
            "usageAppearance":{"weeklyEnabled":true,"monochrome":false},
            "notifications":{"enabled":true,"thresholds":[95,0,50,95,101]},
            "multiProfile":{"enabled":true,"selectedIds":["invalid"]},"autoSwitchEnabled":true
        }))
        .unwrap();
        app.normalize();
        assert_eq!(app.refresh_interval_secs, 120);
        assert!(
            app.menu_bar_enabled && app.usage_appearance.weekly_enabled && app.auto_switch_enabled
        );
        assert!(!app.usage_appearance.monochrome);
        assert!(app.usage_appearance.show_remaining);
        assert!(app.multi_profile.selected_ids.is_empty());
    }

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
        assert!(
            !raw.contains("autoUpdateChecks"),
            "legacy keys must be gone"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
