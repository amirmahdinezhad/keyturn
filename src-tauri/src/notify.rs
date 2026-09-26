//! Threshold notifications and alert sounds.
//!
//! The backend walks the freshly fetched quota windows, fires once per
//! key/window/threshold crossing, and resets that latch when the window
//! recovers (5% hysteresis). Sounds come from the OS library so users can
//! preview them in settings.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use crate::profiles::AppSettings;
use crate::usage::KeyQuota;

static FIRED: LazyLock<Mutex<HashMap<String, HashSet<u32>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct Alert {
    pub title: String,
    pub body: String,
}

/// Reset latch distance: crossing back below threshold - 5% re-arms it.
const HYSTERESIS: u32 = 5;

fn window_label(window: &str) -> &str {
    match window {
        "rolling" => "5-hour",
        "weekly" => "weekly",
        "monthly" => "monthly",
        other => other,
    }
}

/// Compare every window against the configured thresholds and return the
/// alerts that should be shown now.
pub fn evaluate(
    profile: &str,
    settings: &AppSettings,
    quota: &BTreeMap<String, BTreeMap<String, KeyQuota>>,
) -> Vec<Alert> {
    if !settings.notify_enabled {
        return Vec::new();
    }
    let mut thresholds = settings.notify_thresholds.clone();
    if let Some(custom) = settings.notify_custom {
        thresholds.push(custom);
    }
    thresholds.sort_unstable();
    thresholds.dedup();
    if thresholds.is_empty() {
        return Vec::new();
    }

    let mut alerts = Vec::new();
    let mut state = match FIRED.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    for (provider, keys) in quota {
        for (key, quota) in keys {
            for window in quota.windows.iter().flatten() {
                let latch_key = format!("{profile}:{provider}:{key}:{}", window.window);
                let fired = state.entry(latch_key.clone()).or_default();
                for threshold in &thresholds {
                    let threshold = *threshold;
                    let crossed = window.percent >= threshold as f64;
                    if crossed {
                        if fired.insert(threshold) {
                            let severity = if threshold >= 95 {
                                "critical"
                            } else if threshold >= 90 {
                                "warning"
                            } else {
                                "notice"
                            };
                            alerts.push(Alert {
                                title: format!(
                                    "{severity}: {key} hit {threshold}% ({})",
                                    window_label(&window.window)
                                ),
                                body: format!(
                                    "{provider} · {:.0}% of the {} window used{}",
                                    window.percent,
                                    window_label(&window.window),
                                    window
                                        .resets_at
                                        .as_deref()
                                        .map(|iso| format!(" · {}", resets_in(iso)))
                                        .unwrap_or_default()
                                ),
                            });
                        }
                    } else if (window.percent as u32 + HYSTERESIS) < threshold {
                        fired.remove(&threshold);
                    }
                }
                if fired.is_empty() {
                    state.remove(&latch_key);
                }
            }
        }
    }
    alerts
}

/// "resets in 3 hours 12 minutes", same wording as the panel.
pub fn resets_in(iso: &str) -> String {
    let Ok(resets) = chrono::DateTime::parse_from_rfc3339(iso) else {
        return String::new();
    };
    let seconds = resets.timestamp() - chrono::Utc::now().timestamp();
    if seconds <= 0 {
        return "resets now".to_string();
    }
    let minutes = seconds / 60;
    let days = minutes / 1440;
    let hours = (minutes % 1440) / 60;
    let minutes = minutes % 60;
    let plural = |n: i64, unit: &str| format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
    if days >= 1 {
        format!(
            "resets in {} {}",
            plural(days, "day"),
            plural(hours, "hour")
        )
    } else if hours >= 1 {
        format!(
            "resets in {} {}",
            plural(hours, "hour"),
            plural(minutes, "minute")
        )
    } else {
        format!("resets in {}", plural(minutes, "minute"))
    }
}

// ── sounds ─────────────────────────────────────────────────────────────────

pub fn sounds_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let dir = PathBuf::from("/System/Library/Sounds");
        if dir.is_dir() {
            return Some(dir);
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        for candidate in [
            "/usr/share/sounds/freedesktop/stereo",
            "/usr/share/sounds/alsa",
            "/usr/share/sounds",
        ] {
            let dir = PathBuf::from(candidate);
            if dir.is_dir() {
                return Some(dir);
            }
        }
        None
    }
    #[cfg(target_os = "windows")]
    {
        let dir = PathBuf::from("C:\\Windows\\Media");
        if dir.is_dir() {
            return Some(dir);
        }
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

const AUDIO_EXTENSIONS: &[&str] = &["aiff", "aif", "caf", "wav", "mp3", "ogg"];

pub fn list_sounds_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                return None;
            }
            Some(path.file_stem()?.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

pub fn list_sounds() -> Vec<String> {
    sounds_dir()
        .map(|dir| list_sounds_in(&dir))
        .unwrap_or_default()
}

/// Resolve a user-selected sound name inside `dir`, refusing traversal.
pub fn resolve_sound_in(dir: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        return None;
    }
    for ext in AUDIO_EXTENSIONS {
        let candidate = dir.join(format!("{name}.{ext}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn play(name: &str) -> Result<(), String> {
    let dir = sounds_dir().ok_or("No system sound directory on this platform")?;
    let path = resolve_sound_in(&dir, name).ok_or_else(|| format!("Unknown sound \"{name}\""))?;
    play_file(&path)
}

pub fn play_file(path: &Path) -> Result<(), String> {
    let path = path.to_string_lossy().to_string();
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("afplay");
        c.arg(&path);
        c
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut c = std::process::Command::new("paplay");
        c.arg(&path);
        c
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("powershell");
        c.args([
            "-NoProfile",
            "-Command",
            &format!("(New-Object Media.SoundPlayer '{path}').PlaySync();"),
        ]);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return Err("Sound playback is not supported on this platform".to_string());
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Fire a notification; sound is best-effort. `sound = Some("default")` asks
/// the OS for its notification sound, any other name plays a library file.
pub fn send(app: &tauri::AppHandle, title: &str, body: &str, sound: Option<&str>) {
    use tauri_plugin_notification::NotificationExt;
    let mut builder = app.notification().builder().title(title).body(body);
    if matches!(sound, Some("default")) {
        builder = builder.sound("default");
    }
    let _ = builder.show();
    if let Some(name) = sound.filter(|name| *name != "default") {
        let _ = play(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::QuotaWindow;

    fn settings() -> AppSettings {
        AppSettings {
            notify_enabled: true,
            notify_thresholds: vec![75, 90, 95],
            notify_custom: Some(80),
            ..AppSettings::default()
        }
    }

    /// Tests share the process-wide latch, so every test uses its own
    /// provider tag to stay independent.
    fn quota(tag: &str, percent: f64) -> BTreeMap<String, BTreeMap<String, KeyQuota>> {
        let mut out = BTreeMap::new();
        out.entry(tag.to_string())
            .or_insert_with(BTreeMap::new)
            .insert(
                "work".to_string(),
                KeyQuota {
                    windows: Some(vec![QuotaWindow {
                        window: "rolling".to_string(),
                        percent,
                        resets_at: None,
                        status: None,
                    }]),
                    error: None,
                },
            );
        out
    }

    #[test]
    fn thresholds_fire_once_and_only_once() {
        let _ = evaluate("p", &settings(), &quota("t1", 0.0));
        let first = evaluate("p", &settings(), &quota("t1", 91.0));
        assert_eq!(first.len(), 3, "75, 80 and 90 each fire once");
        assert!(first.iter().any(|a| a.title.contains('9')));
        assert!(first.iter().all(|a| !a.title.contains("critical")));
        let second = evaluate("p", &settings(), &quota("t1", 91.0));
        assert!(second.is_empty(), "no repeat alerts while the latch holds");
        let third = evaluate("p", &settings(), &quota("t1", 96.0));
        assert_eq!(third.len(), 1, "95% fires once");
        assert!(third[0].title.contains("critical"));
    }

    #[test]
    fn dropping_below_hysteresis_rearms_the_latch() {
        let _ = evaluate("p", &settings(), &quota("t2", 0.0));
        assert_eq!(evaluate("p", &settings(), &quota("t2", 95.5)).len(), 4);
        // 89% is far enough below 95 to reset that latch only.
        assert!(evaluate("p", &settings(), &quota("t2", 89.0)).is_empty());
        let rearmed = evaluate("p", &settings(), &quota("t2", 96.0));
        assert_eq!(rearmed.len(), 1);
        assert!(rearmed[0].title.contains("critical"));
    }

    #[test]
    fn disabled_notifications_are_silent() {
        let mut settings = settings();
        settings.notify_enabled = false;
        assert!(evaluate("p", &settings, &quota("t3", 99.0)).is_empty());
    }

    #[test]
    fn reset_wording_matches_the_panel() {
        let soon = (chrono::Utc::now() + chrono::Duration::minutes(183))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        assert_eq!(resets_in(&soon), "resets in 3 hours 3 minutes");
        assert_eq!(resets_in("nope"), "");
    }

    #[test]
    fn sound_resolution_refuses_traversal() {
        let dir = std::env::temp_dir().join(format!("aks-sounds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Glass.aiff"), b"x").unwrap();
        assert!(resolve_sound_in(&dir, "Glass").is_some());
        assert!(resolve_sound_in(&dir, "Nope").is_none());
        assert!(resolve_sound_in(&dir, "../Glass").is_none());
        assert_eq!(list_sounds_in(&dir), vec!["Glass".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
