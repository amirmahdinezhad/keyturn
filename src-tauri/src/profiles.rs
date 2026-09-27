//! Profile index + app-wide settings.
//!
//! Each profile owns a store file with its per-profile settings; the index
//! file remembers which profiles exist, which one is active, and the
//! app-wide preferences (refresh cadence, billing day, updates) that should
//! not change when you switch profiles. Auth lives in the provider CLIs —
//! nothing here stores keys, accounts, thresholds or automation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::paths;
use crate::store;

pub const APP_SETTINGS_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRef {
    pub id: String,
    pub name: String,
}

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
    /// Sparkle-style update checks are opt-in. Kept here so the UI can say so.
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
        app.normalize();
        assert_eq!(app.refresh_interval_secs, 10);
        assert_eq!(app.billing_day, 28);
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
    }
}
