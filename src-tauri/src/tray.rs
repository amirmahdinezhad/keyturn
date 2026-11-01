//! Transparent Keyturn mark and usage metrics tinted with the panel accent.

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::{AppHandle, Manager};

use crate::quota::{self, QuotaSet, QuotaWindow};
use crate::settings::{canonical_account_window, AppSettings, CodexAccountPreferences, MenuBarProvider, MenuBarWindow};

static ICON: &[u8] = include_bytes!("../icons/tray-template.png");
/// Matches the panel's default accent before its saved preferences are restored.
pub const DEFAULT_ACCENT: [u8; 3] = [0, 122, 255];

/// Tint the original mark's RGB while preserving every alpha-mask pixel.
pub fn render(accent: [u8; 3]) -> Image<'static> {
    let image = Image::from_bytes(ICON).expect("bundled tray icon must decode");
    let mut rgba = image.rgba().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[..3].copy_from_slice(&accent);
    }
    Image::new_owned(rgba, image.width(), image.height())
}

fn parse_accent(accent: &str) -> Result<[u8; 3], String> {
    let hex = accent.trim().strip_prefix('#').unwrap_or("");
    if !matches!(hex.len(), 3 | 6) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Accent must be a three- or six-digit hex color".into());
    }
    let expanded = if hex.len() == 3 {
        hex.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        hex.into()
    };
    Ok(std::array::from_fn(|i| {
        u8::from_str_radix(&expanded[i * 2..i * 2 + 2], 16).unwrap()
    }))
}

/// Native usage monitor; its timer keeps running when the webview is hidden.
pub struct MenuBarMonitor {
    state: Arc<Mutex<MenuBarState>>,
    wake: mpsc::SyncSender<()>,
}

struct MenuBarState {
    accent: [u8; 3],
    settings: AppSettings,
    quota: Option<QuotaSet>,
    profiles: Vec<ProfileUsage>,
    generation: u64,
}

impl MenuBarState {
    fn update(&mut self, settings: AppSettings) -> bool {
        let identity_changed = self.settings.menu_bar_provider != settings.menu_bar_provider
            || self.settings.active_workspace_id != settings.active_workspace_id
            || self.settings.active_workspace().map(|w| (&w.account_ids, &w.claude_account_ids, &w.antigravity_account_ids, w.menu_bar_provider(), w.menu_bar_account_id(), &w.usage.active_accounts))
                != settings.active_workspace().map(|w| (&w.account_ids, &w.claude_account_ids, &w.antigravity_account_ids, w.menu_bar_provider(), w.menu_bar_account_id(), &w.usage.active_accounts));
        let source_changed = self.settings.menu_bar_provider != settings.menu_bar_provider
            || self.settings.workspaces != settings.workspaces
            || self.settings.active_workspace_id != settings.active_workspace_id
            || self.settings.codex_accounts != settings.codex_accounts
            || self.settings.codex_display_mode != settings.codex_display_mode
            || self.settings.menu_bar_enabled != settings.menu_bar_enabled
            || self.settings.multi_profile != settings.multi_profile
            || self.settings.auto_switch_enabled != settings.auto_switch_enabled;
        let refresh_changed = source_changed
            || self.settings.provider_accents != settings.provider_accents
            || self.settings.menu_bar_window != settings.menu_bar_window
            || self.settings.refresh_interval_secs != settings.refresh_interval_secs
            || self.settings.usage_appearance != settings.usage_appearance
            || self.settings.multi_profile != settings.multi_profile
            || self.settings.auto_switch_enabled != settings.auto_switch_enabled;
        if source_changed {
            self.generation += 1;
            // Presentation changes repaint the current sample immediately. Only an
            // identity change can make that sample belong to the wrong account.
            if identity_changed { self.quota = None; self.profiles.clear(); }
        }
        self.settings = settings;
        if let Some(workspace) = self.settings.active_workspace() {
            if let Ok(accent) = parse_accent(&workspace.accent) { self.accent = accent; }
        }
        refresh_changed
    }

    fn accept(&mut self, generation: u64, quota: Option<QuotaSet>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.quota = quota;
        true
    }
}

impl MenuBarMonitor {
    /// System appearance changes repaint the current usage without network reads.
    pub fn appearance_changed(&self, app: &AppHandle) { paint(app, self.state.clone()); }
    /// Repaint from cached usage without making another quota request.
    pub fn set_accent(&self, app: &AppHandle, accent: &str) -> Result<(), String> {
        let accent = parse_accent(accent)?;
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.accent == accent {
                return Ok(());
            }
            state.accent = accent;
        }
        paint(app, self.state.clone());
        Ok(())
    }

    /// Discard old-account results, including a probe that is still in flight.
    pub fn account_changed(&self, app: &AppHandle) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.generation += 1;
            state.quota = None;
            state.profiles.clear();
        }
        paint(app, self.state.clone());
        self.refresh();
    }

    /// Apply saved preferences immediately and wake the selected-source probe.
    pub fn update(&self, app: &AppHandle, settings: AppSettings) {
        app.state::<crate::desktop_switch::DesktopSwitch>().cancel_unenrolled(&settings);
        let changed = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .update(settings);
        if changed {
            paint(app, self.state.clone());
            self.refresh();
        }
    }

    /// Coalesce refresh requests while a read-only probe is already running.
    pub fn refresh(&self) {
        let _ = self.wake.try_send(());
    }
}

/// Start once after the tray exists; no quota requests are made while disabled.
pub fn start_monitor(app: &AppHandle, settings: AppSettings) {
    let (wake, receiver) = mpsc::sync_channel(1);
    let state = Arc::new(Mutex::new(MenuBarState {
        accent: settings.active_workspace().and_then(|w| parse_accent(&w.accent).ok()).unwrap_or(DEFAULT_ACCENT),
        settings,
        quota: None,
        profiles: Vec::new(),
        generation: 0,
    }));
    app.manage(MenuBarMonitor {
        state: state.clone(),
        wake,
    });
    paint(app, state.clone());
    let app = app.clone();
    std::thread::spawn(move || {
        let mut attempted_switches = std::collections::BTreeSet::new();
        loop {
            let (settings, generation) = {
                let state = state.lock().unwrap_or_else(|e| e.into_inner());
                (state.settings.clone(), state.generation)
            };
            if monitoring_enabled(&settings) {
                let primary = if settings.menu_bar_provider != MenuBarProvider::Codex && settings.menu_bar_enabled {
                    quota::provider_quota(settings.menu_bar_provider)
                } else {
                    None
                };
                let profiles = if settings.multi_profile.enabled
                    || settings.menu_bar_enabled
                    || settings.codex_menu_bar_enabled()
                    || codex_cycle_enabled(&settings)
                {
                    collect_profiles(&settings)
                } else {
                    Vec::new()
                };
                let accepted = {
                    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                    if state.accept(generation, primary.clone()) {
                        state.profiles = profiles.clone();
                        true
                    } else {
                        false
                    }
                };
                if accepted {
                    paint(&app, state.clone());
                    let now = chrono::Utc::now();
                    // Do not schedule a switch for settings superseded during collection.
                    if state.lock().unwrap_or_else(|e| e.into_inner()).generation == generation {
                        use tauri_plugin_notification::NotificationExt;
                        if codex_cycle_enabled(&settings) {
                            if let Some((active, target, period)) =
                                auto_switch_target(&profiles, &settings, now)
                            {
                                let key = format!("{}:{period}", active.id);
                                if attempted_switches.insert(key) {
                                    match app
                                        .state::<crate::desktop_switch::DesktopSwitch>()
                                        .schedule_automatic(app.clone(), &active.id, &target.id)
                                    {
                                        Ok(_) => {
                                            let _ = app.notification().builder().title("Keyturn · Switching profile").body(format!("{} reached its usage limit. Switching to {}. If a thread is running, Keyturn will ask before closing Codex.",active.name,target.name)).show();
                                        }
                                        Err(error) => {
                                            let _ = app
                                                .notification()
                                                .builder()
                                                .title("Keyturn · Auto-switch unavailable")
                                                .body(error)
                                                .show();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let result = if monitoring_enabled(&settings) {
                receiver.recv_timeout(Duration::from_secs(settings.refresh_interval_secs.into()))
            } else {
                receiver
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            };
            if matches!(result, Err(mpsc::RecvTimeoutError::Disconnected)) {
                break;
            }
        }
    });
}

fn paint(app: &AppHandle, state: Arc<Mutex<MenuBarState>>) {
    let handle = app.clone();
    // Resolve the latest state on the main thread so queued paints cannot
    // restore an old provider's title after a settings change.
    let _ = app.run_on_main_thread(move || {
        let state = state.lock().unwrap_or_else(|e| e.into_inner());
        let now = chrono::Utc::now();
        let mut labels = Vec::new();
        let mut tooltip = Vec::new();
        let mut icon_accent = state.accent;
        let mut warning_used = 0.0_f64;
        let thresholds = state.settings.menu_bar_warning_thresholds();
        let light_menu_bar = handle.get_webview_window("main").and_then(|window| window.theme().ok()) == Some(tauri::Theme::Light);
        if state.settings.codex_menu_bar_enabled() {
            let workspace = state.settings.active_workspace();
            let provider = workspace.map_or("codex", |w| w.menu_bar_provider());
            let selected_id = workspace.and_then(|w| w.menu_bar_account_id()).or_else(|| {
                workspace.and_then(|_| state.profiles.iter().find(|p| p.provider == provider).map(|p| p.id.as_str()))
            });
            for profile in state.profiles.iter().filter(|p| p.provider == provider && (if provider == "codex" { codex_menu_bar_account(&state.settings, &p.id, p.active) } else { selected_id.is_none_or(|id| p.id == id) }) && workspace.is_none_or(|w| match provider {
                "codex" => w.account_ids.contains(&p.id),
                "claude" => w.claude_account_ids.as_ref().is_none_or(|ids| ids.contains(&p.id)),
                "antigravity" => w.antigravity_account_ids.contains(&p.id),
                _ => false,
            })) {
                let prefs = menu_bar_account_preferences(&state.settings, &profile.id);
                if !prefs.menu_bar_enabled || prefs.menu_bar_windows.is_empty() { continue; }
                let remaining = workspace.is_none_or(|w| w.usage.show_remaining);
                let (mut label, detail) = account_label(profile, &prefs, &thresholds, remaining, now);
                if let Some((used, color)) = account_warning(profile, &prefs, &thresholds, now) {
                    let color = readable_warning_color(color, light_menu_bar);
                    if used > warning_used { icon_accent = color; warning_used = used; }
                    tint_label(&mut label, color);
                }
                labels.push(label); tooltip.push(detail);
            }
        }
        if state.settings.active_workspace().is_none() && state.settings.menu_bar_enabled && state.settings.menu_bar_provider != MenuBarProvider::Codex && !state.settings.multi_profile.enabled {
            let mut windows = Vec::new();
            if state.settings.usage_appearance.session_enabled {
                windows.push(state.settings.menu_bar_window);
            }
            if state.settings.usage_appearance.weekly_enabled
                && !windows.contains(&MenuBarWindow::Weekly)
            {
                windows.push(MenuBarWindow::Weekly);
            }
            for window in windows {
                let mut settings = state.settings.clone();
                settings.menu_bar_window = window;
                let (title, detail) = usage_title(&settings, state.quota.as_ref(), now);
                let segments = provider_segments(&settings, &title, settings.usage_appearance.monochrome);
                labels.push(crate::tray_text::Label {
                    text: title,
                    caption: None,
                    color: selected_color(&settings, state.quota.as_ref(), now),
                    segments,
                    metrics: Vec::new(),
                });
                tooltip.push(detail);
            }
        }
        if state.settings.active_workspace().is_none() && state.settings.menu_bar_enabled && state.settings.menu_bar_provider != MenuBarProvider::Codex && state.settings.multi_profile.enabled {
            for profile in &state.profiles {
                if profile.provider == "codex" { continue; }
                if !state
                    .settings
                    .multi_profile
                    .selected_ids
                    .contains(&profile.id)
                {
                    continue;
                }
                let mut settings = state.settings.clone();
                settings.menu_bar_enabled = true;
                settings.menu_bar_provider = if profile.provider == "claude" {
                    crate::settings::MenuBarProvider::Claude
                } else {
                    crate::settings::MenuBarProvider::Codex
                };
                settings.menu_bar_window = MenuBarWindow::FiveHour;
                settings.usage_appearance.show_labels = state.settings.usage_appearance.show_labels;
                settings.usage_appearance.show_remaining =
                    state.settings.multi_profile.show_remaining;
                let (session, mut detail) = usage_title(&settings, profile.quota.as_ref(), now);
                let mut text = session;
                if state.settings.multi_profile.show_weekly {
                    settings.menu_bar_window = MenuBarWindow::Weekly;
                    let (weekly, week_detail) = usage_title(&settings, profile.quota.as_ref(), now);
                    text.push_str(&format!("  {weekly}"));
                    detail.push_str(&format!("\n{week_detail}"));
                    settings.menu_bar_window = MenuBarWindow::FiveHour;
                }
                let segments = provider_segments(&settings, &text, state.settings.multi_profile.monochrome);
                labels.push(crate::tray_text::Label {
                    text,
                    caption: state
                        .settings
                        .multi_profile
                        .show_profile_label
                        .then(|| account_initials(&profile.name)),
                    color: selected_color(&settings, profile.quota.as_ref(), now),
                    segments,
                    metrics: Vec::new(),
                });
                tooltip.push(format!(
                    "{}{}\n{detail}",
                    profile.name,
                    if profile.active { " (Active)" } else { "" }
                ));
            }
        }
        if let Some(tray) = handle.tray_by_id("main") {
            let monochrome = if state.settings.multi_profile.enabled {
                state.settings.multi_profile.monochrome
            } else {
                state.settings.usage_appearance.monochrome
            };
            let fallback = labels
                .iter()
                .map(|v| v.text.as_str())
                .collect::<Vec<_>>()
                .join("  ");
            if labels.is_empty() {
                let _ = tray.set_title(Some(""));
                let _ = tray.set_icon(Some(render(icon_accent)));
                let _ = tray.set_icon_as_template(false);
            } else if let Some(image) = crate::tray_text::render(&labels, monochrome, icon_accent, state.settings.active_workspace().map(|w| w.usage.menu_bar_style).unwrap_or_default()) {
                let _ = tray.set_title(Some(""));
                let _ = tray.set_icon(Some(image));
                // Template mode would replace the chosen accent with the OS tint.
                let _ = tray.set_icon_as_template(false);
            } else {
                let _ = tray.set_icon(Some(render(icon_accent)));
                let _ = tray.set_icon_as_template(false);
                let _ = tray.set_title(Some(fallback));
            }
            let _ = tray.set_tooltip(Some(if tooltip.is_empty() {
                "Keyturn".into()
            } else {
                tooltip.join("\n\n")
            }));
        }
    });
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsage {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub provider: String,
    pub quota: Option<QuotaSet>,
}

/// Apply workspace presentation without modifying saved account preferences.
fn menu_bar_account_preferences(settings: &AppSettings, id: &str) -> CodexAccountPreferences {
    let mut prefs = settings.codex_accounts.get(id).cloned().unwrap_or_default();
    if let Some(workspace) = settings.active_workspace() {
        prefs.color = workspace.accent.clone();
        prefs.menu_bar_enabled = true;
        if let Some(windows) = workspace.usage.account_windows.get(id) { prefs.menu_bar_windows = windows.clone(); }
        prefs.warning_colors = workspace.usage.warning_colors;
        prefs.show_profile_label = workspace.usage.show_account_name.unwrap_or(prefs.show_profile_label);
    }
    prefs
}

/// Preserve provider-specific bucket names; Codex continues to use canonical main windows.
fn metric_window_key(provider: &str, window: &str) -> String {
    canonical_account_window(window).unwrap_or_else(|| if provider == "codex" { String::new() } else { window.to_owned() })
}

/// Resolve one warning hue from the selected, current limits for the whole account.
fn account_warning(
    profile: &ProfileUsage,
    prefs: &CodexAccountPreferences,
    thresholds: &[u8],
    now: chrono::DateTime<chrono::Utc>,
) -> Option<(f64, [u8; 3])> {
    if !prefs.warning_colors { return None; }
    let used = profile.quota.as_ref().filter(|q| q.error.is_none())?.windows.as_ref()?.iter()
        .filter(|w| current_window(w, now) && prefs.menu_bar_windows.contains(&metric_window_key(&profile.provider, &w.window)))
        .map(|w| w.percent).max_by(f64::total_cmp)?;
    threshold_color(used, thresholds).map(|color| (used, color))
}

/// Warning tint includes initials, all metrics and window labels.
fn account_label(
    profile: &ProfileUsage,
    prefs: &CodexAccountPreferences,
    thresholds: &[u8],
    show_remaining: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> (crate::tray_text::Label, String) {
    let warning = account_warning(profile, prefs, thresholds, now).map(|(_, color)| color);
    let color = warning.unwrap_or_else(|| parse_accent(&prefs.color).unwrap_or(DEFAULT_ACCENT));
    let mut segments = Vec::new();
    let mut metrics = Vec::new();
    let mut details = vec![format!("{}{}", profile.name, if profile.active { " (Active)" } else { "" })];
    for (index, kind) in prefs.menu_bar_windows.iter().enumerate() {
        let window = profile.quota.as_ref().filter(|q| q.error.is_none())
            .and_then(|q| q.windows.as_ref())
            .and_then(|ws| ws.iter().find(|w| metric_window_key(&profile.provider, &w.window) == *kind && current_window(w, now)));
        let label = match kind.as_str() {
            "5-hour" => "5h",
            "weekly" => "1w",
            "monthly" => "1mo",
            other => other,
        };
        let value = window.map(|w| format!("{:.0}%", if show_remaining { remaining_percent(w.percent) } else { w.percent.clamp(0.0, 100.0).round() }))
            .unwrap_or_else(|| "—".into());
        let metric_color = warning.unwrap_or(if window.is_none() { [160, 166, 176] } else { color });
        metrics.push(crate::tray_text::Metric {
            label: prefs.show_labels.then(|| label.to_owned()),
            fraction: window.map(|w| (if show_remaining { 100.0 - w.percent } else { w.percent }).clamp(0.0, 100.0) / 100.0),
            color: metric_color,
            time_marker: window.and_then(|window| usage_time_marker(window, now, show_remaining)),
        });
        let prefix = if index == 0 { "" } else { "  " };
        segments.push((if prefs.show_labels { format!("{prefix}{label} {value}") } else { format!("{prefix}{value}") }, metric_color));
        details.push(format!("{kind}: {value} {}{}", if show_remaining { "left" } else { "used" }, if window.is_none() { " · Usage unavailable" } else { "" }));
    }
    (crate::tray_text::Label {
        text: segments.iter().map(|(text, _)| text.as_str()).collect::<String>(),
        segments,
        metrics,
        caption: prefs.show_profile_label.then(|| account_initials(&profile.name)),
        color,
    }, details.join("\n"))
}

/// Match the Usage limits popup: known session/week durations only, never infer
/// a monthly duration or fabricate timing when the reset is absent or invalid.
fn usage_time_marker(window: &QuotaWindow, now: chrono::DateTime<chrono::Utc>, show_remaining: bool) -> Option<f64> {
    let duration = match window.window.to_lowercase().as_str() {
        "5-hour" | "rolling" => 18_000_000.0,
        "weekly" => 604_800_000.0,
        _ => return None,
    };
    let reset = chrono::DateTime::parse_from_rfc3339(window.resets_at.as_deref()?).ok()?;
    let left = reset.signed_duration_since(now).num_milliseconds() as f64;
    if !window.percent.is_finite() || left <= 0.0 || left > duration { return None; }
    let elapsed = 1.0 - left / duration;
    Some(if show_remaining { 1.0 - elapsed } else { elapsed })
}

fn monitoring_enabled(settings: &AppSettings) -> bool {
    settings.menu_bar_enabled
        || settings.codex_menu_bar_enabled()
        || codex_cycle_enabled(settings)
}

fn codex_cycle_enabled(settings: &AppSettings) -> bool {
    settings.active_workspace().is_none_or(|w| w.usage.auto_switch_enabled)
        && settings.codex_accounts.iter().filter(|(id, p)| settings.contains_codex_account(id) && p.auto_switch).count() >= 2
}

/// The menu bar follows the live Codex login only within the current workspace.
fn codex_menu_bar_account(settings: &AppSettings, id: &str, active: bool) -> bool {
    active && settings.codex_menu_bar_enabled()
        && settings.contains_codex_account(id)
        && settings.active_workspace().is_none_or(|w| w.menu_bar_provider() == "codex")
}

/// All account requests use their own saved login; none activate an account.
pub fn collect_profiles(settings: &AppSettings) -> Vec<ProfileUsage> {
    let data = crate::profiles::list().ok();
    let mut profiles: Vec<_> = data
        .map(|d| d.profiles)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            if !settings.contains_codex_account(&p.id) { return false; }
            let prefs = settings.codex_accounts.get(&p.id).cloned().unwrap_or_default();
            {
                (codex_cycle_enabled(settings) && prefs.auto_switch)
                    || (codex_menu_bar_account(settings, &p.id, p.active)
                        && (settings.active_workspace().is_some() || prefs.menu_bar_enabled)
                        && (!prefs.menu_bar_windows.is_empty() || settings.active_workspace().is_some_and(|w| w.usage.account_windows.get(&p.id).is_some_and(|v| !v.is_empty()))))
            }
        })
        .collect();
    if let Some(workspace) = settings.active_workspace() {
        profiles.sort_by_key(|p| workspace.account_ids.iter().position(|id| id == &p.id));
    }
    let mut samples = std::thread::scope(|scope| {
        let jobs: Vec<_> = profiles
            .into_iter()
            .map(|p| {
                scope.spawn(move || ProfileUsage {
                    quota: if p.connected {
                        crate::profiles::quota_for(&p.id)
                    } else {
                        None
                    },
                    id: p.id,
                    name: p.name,
                    active: p.active,
                    provider: "codex".into(),
                })
            })
            .collect();
        jobs.into_iter()
            .filter_map(|j| j.join().ok())
            .collect::<Vec<_>>()
    });
    if let Ok(profiles) = crate::claude_profiles::list() {
        samples.extend(
            profiles
                .into_iter()
                .filter(|p| {
                    if let Some(w) = settings.active_workspace() {
                        w.claude_account_ids.as_ref().is_none_or(|ids| ids.contains(&p.profile.id))
                            && (w.usage.menu_bar_enabled && w.menu_bar_provider() == "claude")
                    } else { settings.multi_profile.selected_ids.contains(&p.profile.id) }
                })
                .map(|p| ProfileUsage {
                    id: p.profile.id,
                    name: p.profile.name,
                    provider: "claude".into(),
                    active: p.active,
                    quota: p.quota,
                }),
        );
    }
    if let Some(workspace) = settings.active_workspace().filter(|w| w.usage.menu_bar_enabled && w.menu_bar_provider() == "antigravity") {
        if let Ok(accounts) = crate::antigravity::list() {
            for account in accounts.into_iter().filter(|a| workspace.antigravity_account_ids.contains(&a.id)) {
                samples.push(ProfileUsage { quota: crate::antigravity::quota(&account.id).ok(), id: account.id, name: account.name, provider: "antigravity".into(), active: false });
            }
        }
    }
    // Provider selection never borrows Codex's active identity.
    if let Some(workspace) = settings.active_workspace() {
        for provider in ["claude", "antigravity"] {
            let members: Vec<_> = samples.iter().filter(|p| p.provider == provider && match provider {
                "claude" => workspace.claude_account_ids.as_ref().is_none_or(|ids| ids.contains(&p.id)),
                _ => workspace.antigravity_account_ids.contains(&p.id),
            }).collect();
            let active = members.iter().find(|p| p.active).map(|p| p.id.clone())
                .or_else(|| workspace.usage.active_accounts.get(provider).filter(|id| members.iter().any(|p| p.id == **id)).cloned())
                .or_else(|| members.first().map(|p| p.id.clone()));
            for sample in samples.iter_mut().filter(|p| p.provider == provider) { sample.active = active.as_ref() == Some(&sample.id); }
        }
    }
    samples
}

/// Keep names compact while retaining the full name in the native tooltip.
fn account_initials(name: &str) -> String {
    let words: Vec<_> = name.split_whitespace().filter(|part| part.chars().any(char::is_alphanumeric)).collect();
    let first = words.first().and_then(|word| word.chars().find(|c| c.is_alphanumeric()));
    let last = (words.len() > 1).then(|| words.last().unwrap().chars().find(|c| c.is_alphanumeric())).flatten();
    first.into_iter().chain(last).flat_map(char::to_uppercase).collect()
}

fn threshold_color(used: f64, thresholds: &[u8]) -> Option<[u8; 3]> {
    if !used.is_finite() { return None; }
    thresholds.iter().copied().filter(|level| used >= f64::from(*level)).max().map(|level| warning_color(f64::from(level)))
}

/// Native appearance follows macOS, independently of the panel's CSS theme.
fn readable_warning_color(color: [u8; 3], light: bool) -> [u8; 3] {
    if light && color == [255, 214, 10] { [138, 83, 0] } else { color }
}

fn tint_label(label: &mut crate::tray_text::Label, color: [u8; 3]) {
    label.color = color;
    for (_, metric_color) in &mut label.segments { *metric_color = color; }
    for metric in &mut label.metrics { metric.color = color; }
}

fn warning_color(used: f64) -> [u8; 3] {
    if used >= 95.0 { [255, 69, 58] } else if used >= 90.0 { [255, 159, 10] } else if used >= 75.0 { [255, 214, 10] } else { [48, 184, 107] }
}

/// Never switch on missing, stale, expired or model-specific data.
fn auto_switch_target<'a>(
    profiles: &'a [ProfileUsage],
    settings: &AppSettings,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<(&'a ProfileUsage, &'a ProfileUsage, String)> {
    let active_index = profiles
        .iter()
        .position(|p| p.active && p.provider == "codex")?;
    let active = &profiles[active_index];
    if !settings.contains_codex_account(&active.id) || !settings.codex_accounts.get(&active.id).is_some_and(|p| p.auto_switch) { return None; }
    let data = active.quota.as_ref().filter(|q| q.error.is_none())?;
    let window = data.windows.as_ref()?.iter().find(|w| {
        canonical_account_window(&w.window).is_some() && current_window(w, now)
            && w.percent >= 100.0 - f64::from(settings.active_workspace().map_or(2, |w| w.usage.auto_switch_remaining)) && w.resets_at.as_deref()
                .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok()).is_some_and(|reset| reset > now)
    })?;
    let period = window.resets_at.clone()?;
    for offset in 1..profiles.len() {
        let candidate = &profiles[(active_index + offset) % profiles.len()];
        if candidate.provider != "codex" || !settings.contains_codex_account(&candidate.id) || !settings.codex_accounts.get(&candidate.id).is_some_and(|p| p.auto_switch) {
            continue;
        }
        let Some(data) = candidate.quota.as_ref() else {
            continue;
        };
        if data.error.is_some() { continue; }
        if data.windows.iter().flatten().any(|w| canonical_account_window(&w.window).is_some() && !current_window(w, now)) { continue; }
        let windows: Vec<_> = data.windows.iter().flatten()
            .filter(|w| canonical_account_window(&w.window).is_some() && current_window(w, now)).collect();
        let known = &settings.codex_accounts[&candidate.id].known_windows;
        if !windows.is_empty() && windows.iter().all(|w| w.percent < 100.0 - f64::from(settings.active_workspace().map_or(2, |w| w.usage.auto_switch_remaining)))
            && known.iter().all(|kind| windows.iter().any(|w| canonical_account_window(&w.window).as_ref() == Some(kind))) {
            return Some((active, candidate, period));
        }
    }
    None
}

fn provider_accent(settings: &AppSettings) -> Option<[u8; 3]> {
    settings.provider_accents.get(settings.menu_bar_provider.id()).and_then(|color| parse_accent(color).ok())
}

/// Explicit provider colors also survive monochrome mode; the tray mark keeps its workspace color.
fn provider_segments(settings: &AppSettings, text: &str, monochrome: bool) -> Vec<(String, [u8; 3])> {
    if monochrome {
        provider_accent(settings).map(|color| vec![(text.to_owned(), color)]).unwrap_or_default()
    } else { Vec::new() }
}

fn selected_color(
    settings: &AppSettings,
    data: Option<&QuotaSet>,
    now: chrono::DateTime<chrono::Utc>,
) -> [u8; 3] {
    let Some(data) = data else {
        return [160, 166, 176];
    };
    let window = data.windows.as_ref().and_then(|ws| {
        ws.iter()
            .filter(|w| {
                current_window(w, now)
                    && window_kind(&w.window).is_some_and(|kind| {
                        settings.menu_bar_window == MenuBarWindow::Auto
                            || kind == settings.menu_bar_window
                    })
            })
            .max_by(|a, b| a.percent.total_cmp(&b.percent))
    });
    let Some(window) = window.filter(|_| data.error.is_none()) else {
        return [160, 166, 176];
    };
    let used = window.percent;
    if used >= 95.0 {
        [255, 69, 58]
    } else if used >= 90.0 {
        [255, 159, 10]
    } else if used >= 75.0 {
        [255, 214, 10]
    } else {
        provider_accent(settings).unwrap_or([48, 184, 107])
    }
}

fn window_kind(window: &str) -> Option<MenuBarWindow> {
    match window.to_ascii_lowercase().as_str() {
        "rolling" | "5-hour" => Some(MenuBarWindow::FiveHour),
        "weekly" => Some(MenuBarWindow::Weekly),
        "monthly" => Some(MenuBarWindow::Monthly),
        _ => None,
    }
}

fn window_labels(window: MenuBarWindow) -> (&'static str, &'static str) {
    match window {
        MenuBarWindow::Auto => ("", "Automatic"),
        MenuBarWindow::FiveHour => ("5h", "5-hour"),
        MenuBarWindow::Weekly => ("1w", "Weekly"),
        MenuBarWindow::Monthly => ("1mo", "Monthly"),
    }
}

fn current_window(window: &QuotaWindow, now: chrono::DateTime<chrono::Utc>) -> bool {
    window.percent.is_finite()
        && !window
            .resets_at
            .as_deref()
            .and_then(|iso| chrono::DateTime::parse_from_rfc3339(iso).ok())
            .is_some_and(|reset| reset <= now)
}

/// Match Codex's Math.round(remaining): formatting alone rounds ties to even.
fn remaining_percent(used: f64) -> f64 {
    (100.0 - used).clamp(0.0, 100.0).round()
}

/// Always show remaining capacity for Codex, regardless of popup wording.
fn usage_title(
    settings: &AppSettings,
    quota: Option<&QuotaSet>,
    now: chrono::DateTime<chrono::Utc>,
) -> (String, String) {
    if !settings.menu_bar_enabled {
        return (String::new(), "Keyturn".into());
    }
    let provider = settings.menu_bar_provider;
    let quota = quota.filter(|q| q.provider == provider.id());
    let selected = quota
        .filter(|q| q.error.is_none())
        .and_then(|q| q.windows.as_ref())
        .and_then(|windows| {
            windows
                .iter()
                .filter(|w| current_window(w, now))
                .filter(|w| {
                    window_kind(&w.window).is_some_and(|kind| {
                        settings.menu_bar_window == MenuBarWindow::Auto
                            || kind == settings.menu_bar_window
                    })
                })
                .max_by(|a, b| a.percent.total_cmp(&b.percent))
        });
    let Some(window) = selected else {
        let (short, label) = window_labels(settings.menu_bar_window);
        let reason = quota
            .and_then(|q| q.error.as_deref())
            .unwrap_or("No current quota data for this window");
        let title = if short.is_empty() || !settings.usage_appearance.show_labels {
            "—".into()
        } else {
            format!("{short} —")
        };
        return (
            title,
            format!("Keyturn · {} · {label}\n{reason}", provider.label()),
        );
    };
    let (short, label) =
        window_labels(window_kind(&window.window).expect("filtered supported window"));
    let value = if provider == MenuBarProvider::Codex || settings.usage_appearance.show_remaining {
        100.0 - window.percent
    } else {
        window.percent
    };
    let remaining = if provider == MenuBarProvider::Codex {
        remaining_percent(window.percent)
    } else {
        (value.clamp(0.0, 100.0) * 10.0).round() / 10.0
    };
    let percent = if remaining.fract() == 0.0 {
        format!("{remaining:.0}")
    } else {
        format!("{remaining:.1}")
    };
    let title = if settings.usage_appearance.show_labels {
        format!("{short} {percent}%")
    } else {
        format!("{percent}%")
    };
    let wording = if provider == MenuBarProvider::Codex || settings.usage_appearance.show_remaining {
        "remaining"
    } else {
        "used"
    };
    let mut tooltip = format!(
        "Keyturn · {} · {label}: {percent}% {wording}",
        provider.label()
    );
    if let Some(reset) = window.resets_at.as_deref() {
        tooltip.push_str(&format!("\nResets: {reset}"));
    }
    if let Some(as_of) = quota.and_then(|q| q.as_of.as_deref()) {
        tooltip.push_str(&format!("\nSnapshot: {as_of}"));
    }
    (title, tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::MenuBarProvider;

    #[test]
    fn progress_time_marker_matches_popup_direction_and_rejects_unreliable_resets() {
        let now = chrono::Utc::now();
        for (kind, duration) in [("5-hour", 18_000_000), ("rolling", 18_000_000), ("Weekly", 604_800_000)] {
            let mut window = quota(&[(kind, 30.0)]).windows.unwrap().remove(0);
            window.resets_at = Some((now + chrono::Duration::milliseconds(duration / 4)).to_rfc3339());
            assert_eq!(usage_time_marker(&window, now, true), Some(0.25));
            assert_eq!(usage_time_marker(&window, now, false), Some(0.75));
            window.resets_at = Some((now + chrono::Duration::milliseconds(duration + 1)).to_rfc3339());
            assert_eq!(usage_time_marker(&window, now, true), None);
            window.resets_at = Some(now.to_rfc3339()); assert_eq!(usage_time_marker(&window, now, true), None);
            window.resets_at = Some("invalid".into()); assert_eq!(usage_time_marker(&window, now, true), None);
            window.resets_at = None; assert_eq!(usage_time_marker(&window, now, true), None);
        }
        let monthly = quota(&[("monthly", 30.0)]).windows.unwrap().remove(0);
        assert_eq!(usage_time_marker(&monthly, now, true), None);
    }

    #[test]
    fn yellow_warning_has_readable_light_and_dark_variants_and_one_shared_tint() {
        let luminance = |color: [u8; 3]| {
            let linear = |channel: u8| { let value = f64::from(channel) / 255.0; if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) } };
            0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2])
        };
        for (light, background) in [(true, [255, 255, 255]), (false, [35, 38, 43])] {
            let color = readable_warning_color(warning_color(75.0), light);
            let (a, b) = (luminance(color), luminance(background));
            assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
            let profile = ProfileUsage { id: "emma".into(), name: "Emma Brooks".into(), active: true, provider: "codex".into(), quota: Some(quota(&[("weekly", 80.0)])) };
            let prefs = CodexAccountPreferences { warning_colors: true, menu_bar_windows: vec!["weekly".into()], ..Default::default() };
            let (mut label, _) = account_label(&profile, &prefs, &[75, 90, 95], true, chrono::Utc::now());
            tint_label(&mut label, color);
            assert_eq!(label.color, color); assert!(label.segments.iter().all(|(_, c)| *c == color)); assert!(label.metrics.iter().all(|m| m.color == color));
            if let Ok(path) = std::env::var("KEYTURN_TEST_WARNING_PALETTE") {
                for (style_name, style) in [("percentages", crate::settings::MenuBarStyle::Percentages), ("progress", crate::settings::MenuBarStyle::Progress)] {
                    let image = crate::tray_text::render(&[label.clone()], true, color, style).unwrap();
                    let name = format!("{path}-{}-{style_name}", if light { "light" } else { "dark" });
                    std::fs::write(format!("{name}.rgba"), image.rgba()).unwrap();
                    std::fs::write(format!("{name}.json"), format!("[{},{}]", image.width(), image.height())).unwrap();
                }
            }
        }
        assert_eq!(readable_warning_color(warning_color(95.0), true), warning_color(95.0));
    }

    #[test]
    fn provider_color_overrides_monochrome_without_changing_workspace_accent() {
        let mut settings = AppSettings { menu_bar_provider: MenuBarProvider::Claude, ..Default::default() };
        settings.provider_accents.insert("claude".into(), "#7c6cf0".into());
        settings.provider_accents.insert("opencode".into(), "#2f9e6e".into());
        assert_eq!(provider_segments(&settings, "82%", true), vec![("82%".into(), [124, 108, 240])]);
        assert!(provider_segments(&settings, "82%", false).is_empty());
        assert_eq!(selected_color(&settings, Some(&quota(&[("5-hour", 20.0)])), chrono::Utc::now()), [124, 108, 240]);
        assert_eq!(selected_color(&settings, Some(&quota(&[("5-hour", 96.0)])), chrono::Utc::now()), [255, 69, 58]);
        settings.menu_bar_provider = MenuBarProvider::Opencode;
        assert_eq!(provider_accent(&settings), Some([47, 158, 110]));
        settings.menu_bar_provider = MenuBarProvider::Codex;
        assert_eq!(provider_accent(&settings), None);
    }

    #[test]
    fn accent_accepts_saved_hex_colors_and_rejects_invalid_input() {
        assert_eq!(parse_accent(" #7C6CF0 ").unwrap(), [124, 108, 240]);
        assert_eq!(parse_accent("#09f").unwrap(), [0, 153, 255]);
        for value in ["red", "007aff", "#12", "#gggggg", "#12345678", "#éabc"] {
            assert!(parse_accent(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn every_accent_preserves_the_original_transparent_mark() {
        let original = Image::from_bytes(ICON).unwrap();
        for accent in [DEFAULT_ACCENT, [47, 158, 110], [194, 106, 43], [124, 108, 240]] {
            let image = render(accent);
            assert_eq!((image.width(), image.height()), (original.width(), original.height()));
            for (pixel, source) in image.rgba().chunks_exact(4).zip(original.rgba().chunks_exact(4)) {
                assert_eq!(&pixel[..3], &accent);
                assert_eq!(pixel[3], source[3]);
            }
        }
    }

    fn settings(window: MenuBarWindow) -> AppSettings {
        AppSettings {
            menu_bar_enabled: true,
            menu_bar_window: window,
            ..Default::default()
        }
    }

    fn quota(windows: &[(&str, f64)]) -> QuotaSet {
        QuotaSet {
            provider: "codex".into(),
            error: None,
            reset_credits: None, as_of: None,
            windows: Some(
                windows
                    .iter()
                    .map(|(window, percent)| QuotaWindow {
                        window: (*window).into(),
                        percent: *percent,
                        resets_at: None,
                        status: None,
                    })
                    .collect(),
            ),
        }
    }

    #[test]
    fn selected_windows_show_remaining_and_normalize_provider_labels() {
        let data = quota(&[("rolling", 23.5), ("Weekly", 81.0), ("monthly", 50.0)]);
        for (window, expected) in [
            (MenuBarWindow::FiveHour, "5h 77%"),
            (MenuBarWindow::Weekly, "1w 19%"),
            (MenuBarWindow::Monthly, "1mo 50%"),
        ] {
            assert_eq!(
                usage_title(&settings(window), Some(&data), chrono::Utc::now()).0,
                expected
            );
        }
    }

    #[test]
    fn auto_selects_the_tightest_supported_limit() {
        let data = quota(&[
            ("5-hour", 20.0),
            ("Weekly", 91.0),
            ("monthly", 70.0),
            ("gpt-reserve", 100.0),
            ("Opus weekly", 99.0),
        ]);
        assert_eq!(
            usage_title(
                &settings(MenuBarWindow::Auto),
                Some(&data),
                chrono::Utc::now()
            )
            .0,
            "1w 9%"
        );
    }

    #[test]
    fn missing_windows_and_errors_never_fall_back_to_another_limit() {
        let mut data = quota(&[("5-hour", 12.0)]);
        let app = settings(MenuBarWindow::Monthly);
        assert_eq!(
            usage_title(&app, Some(&data), chrono::Utc::now()).0,
            "1mo —"
        );
        data.error = Some("session expired".into());
        let (title, tooltip) = usage_title(
            &settings(MenuBarWindow::FiveHour),
            Some(&data),
            chrono::Utc::now(),
        );
        assert_eq!(title, "5h —");
        assert!(tooltip.contains("session expired"));
        data.error = None;
        data.provider = "claude".into();
        assert_eq!(
            usage_title(
                &settings(MenuBarWindow::FiveHour),
                Some(&data),
                chrono::Utc::now()
            )
            .0,
            "5h —"
        );
    }

    #[test]
    fn invalid_percentages_are_unavailable_and_capacity_is_clamped() {
        for (used, expected) in [
            (-10.0, "5h 100%"),
            (120.0, "5h 0%"),
            (f64::NAN, "5h —"),
            (f64::INFINITY, "5h —"),
        ] {
            let data = quota(&[("5-hour", used)]);
            assert_eq!(
                usage_title(
                    &settings(MenuBarWindow::FiveHour),
                    Some(&data),
                    chrono::Utc::now()
                )
                .0,
                expected
            );
        }
    }

    #[test]
    fn elapsed_resets_are_unavailable_and_snapshot_age_is_identifiable() {
        let now = chrono::Utc::now();
        let mut data = quota(&[("5-hour", 30.0), ("Weekly", 20.0)]);
        data.as_of = Some("2026-10-13T04:00:00Z".into());
        data.windows.as_mut().unwrap()[0].resets_at =
            Some((now - chrono::Duration::seconds(1)).to_rfc3339());
        assert_eq!(
            usage_title(&settings(MenuBarWindow::FiveHour), Some(&data), now).0,
            "5h —"
        );
        let (title, tooltip) = usage_title(&settings(MenuBarWindow::Auto), Some(&data), now);
        assert_eq!(title, "1w 80%");
        assert!(tooltip.contains("Snapshot: 2026-10-13T04:00:00Z"));
    }

    #[test]
    fn disabling_clears_the_title_and_ignores_popup_wording() {
        let data = quota(&[("5-hour", 20.0)]);
        assert_eq!(
            usage_title(&AppSettings::default(), Some(&data), chrono::Utc::now()),
            ("".into(), "Keyturn".into())
        );
        let mut app = settings(MenuBarWindow::FiveHour);
        for remaining_mode in [true, false] {
            app.usage_show_remaining = remaining_mode;
            assert_eq!(
                usage_title(&app, Some(&data), chrono::Utc::now()).0,
                "5h 80%"
            );
        }
    }

    #[test]
    fn provider_changes_discard_in_flight_results_and_window_changes_use_cache() {
        let mut state = MenuBarState {
            accent: [124, 108, 240],
            settings: settings(MenuBarWindow::FiveHour),
            quota: Some(quota(&[("5-hour", 20.0), ("Weekly", 70.0)])),
            profiles: Vec::new(),
            generation: 0,
        };
        state.update(settings(MenuBarWindow::Weekly));
        assert_eq!(state.accent, [124, 108, 240]);
        assert_eq!(state.generation, 0);
        assert_eq!(
            usage_title(&state.settings, state.quota.as_ref(), chrono::Utc::now()).0,
            "1w 30%"
        );
        let mut next = settings(MenuBarWindow::Weekly);
        next.menu_bar_provider = MenuBarProvider::Claude;
        state.update(next);
        assert!(state.quota.is_none());
        assert!(!state.accept(0, Some(quota(&[("Weekly", 90.0)]))));
        assert!(state.quota.is_none());
        assert!(state.accept(1, None));
    }

    #[test]
    fn percentage_wording_and_labels_are_independent_of_popup_preferences() {
        let data = quota(&[("5-hour", 23.5)]);
        let mut app = settings(MenuBarWindow::FiveHour);
        app.usage_appearance.show_labels = false;
        app.usage_appearance.show_remaining = false;
        let (title, tooltip) = usage_title(&app, Some(&data), chrono::Utc::now());
        assert_eq!(title, "77%");
        assert!(tooltip.contains("77% remaining"));
        app.usage_appearance.show_remaining = true;
        assert_eq!(
            usage_title(&app, Some(&data), chrono::Utc::now()).0,
            "77%"
        );
    }

    #[test]
    fn auto_switch_skips_unavailable_exhausted_and_unrelated_accounts() {
        let now = chrono::Utc::now();
        let profile = |id: &str, active: bool, used: f64, weekly: f64| {
            let mut quota = quota(&[("5-hour", used), ("Weekly", weekly)]);
            quota.windows.as_mut().unwrap()[0].resets_at =
                Some((now + chrono::Duration::hours(2)).to_rfc3339());
            ProfileUsage {
                id: id.into(),
                name: id.into(),
                active,
                provider: "codex".into(),
                quota: Some(quota),
            }
        };
        let mut samples = vec![
            profile("work", true, 100.0, 20.0),
            profile("limited", false, 10.0, 100.0),
            profile("available", false, 25.0, 50.0),
        ];
        let mut prefs = AppSettings::default();
        for sample in &samples { prefs.codex_accounts.insert(sample.id.clone(), CodexAccountPreferences { auto_switch: true, ..Default::default() }); }
        let (_, target, _) = auto_switch_target(&samples, &prefs, now).expect("available account");
        assert_eq!(target.id, "available");
        samples[2].quota.as_mut().unwrap().error = Some("expired login".into());
        assert!(auto_switch_target(&samples, &prefs, now).is_none());
        samples[1].provider = "claude".into();
        assert!(auto_switch_target(&samples, &prefs, now).is_none());
        samples[0].quota.as_mut().unwrap().windows.as_mut().unwrap()[0].percent = 99.0;
        assert!(auto_switch_target(&samples, &prefs, now).is_none());
    }

    #[test]
    fn menu_bar_follows_only_the_live_codex_member() {
        let mut settings = AppSettings::default();
        settings.active_workspace_id = Some("studio".into());
        settings.workspaces.push(serde_json::from_value(serde_json::json!({
            "id":"studio", "name":"Studio", "accent":"#007aff",
            "accountIds":["live", "backup"],
            "usage":{"menuBarAccountId":"backup", "activeAccounts":{"codex":"backup"}}
        })).unwrap());
        assert!(codex_menu_bar_account(&settings, "live", true));
        assert!(!codex_menu_bar_account(&settings, "backup", false));
        assert!(!codex_menu_bar_account(&settings, "outside", true));
        // An account switch follows the new login despite stale saved selections.
        assert!(!codex_menu_bar_account(&settings, "live", false));
        assert!(codex_menu_bar_account(&settings, "backup", true));
        settings.workspaces[0].usage.menu_bar_enabled = false;
        assert!(!codex_menu_bar_account(&settings, "live", true));
        settings.active_workspace_id = None;
        settings.menu_bar_enabled = true;
        settings.codex_display_mode = crate::settings::CodexDisplayMode::Selected;
        assert!(codex_menu_bar_account(&settings, "live", true));
        assert!(!codex_menu_bar_account(&settings, "backup", false));
    }

    #[test]
    fn codex_low_remaining_percentage_rounds_up_at_half_points() {
        let now = chrono::Utc::now();
        let prefs = CodexAccountPreferences {
            menu_bar_windows: vec!["5-hour".into()],
            ..Default::default()
        };
        let profile = ProfileUsage {
            id: "personal".into(), name: "Personal".into(), active: true,
            provider: "codex".into(), quota: Some(quota(&[("5-hour", 93.5)])),
        };
        assert_eq!(account_label(&profile, &prefs, &[], true, now).0.text, "5h 7%");
    }

    #[test]
    fn workspace_accent_change_recolors_cached_numbers_and_progress_with_the_icon() {
        let mut app = AppSettings::default();
        app.active_workspace_id = Some("studio".into());
        app.codex_accounts.insert("emma".into(), CodexAccountPreferences { color:"#007aff".into(), ..Default::default() });
        app.workspaces.push(serde_json::from_value(serde_json::json!({"id":"studio","name":"Studio","accent":"#007aff","accountIds":["emma"],"usage":{"accountWindows":{"emma":["weekly"]},"showAccountName":true}})).unwrap());
        let profile = ProfileUsage {id:"emma".into(),name:"Emma Brooks".into(),active:true,provider:"codex".into(),quota:Some(quota(&[("weekly",20.0)]))};
        let mut state = MenuBarState {accent:DEFAULT_ACCENT,settings:app.clone(),quota:None,profiles:vec![profile],generation:0};
        app.workspaces[0].accent = "#7c6cf0".into();
        assert!(state.update(app));
        assert_eq!(state.profiles.len(),1,"workspace color changes must keep the sample for immediate repaint");
        assert_eq!(state.accent,[124,108,240]);
        let prefs = menu_bar_account_preferences(&state.settings,"emma");
        let (label,_) = account_label(&state.profiles[0],&prefs,state.settings.menu_bar_warning_thresholds(),true,chrono::Utc::now());
        assert_eq!(label.color,state.accent,"metrics must follow the workspace color just like the logo");
        assert!(label.segments.iter().all(|(_,color)| *color==state.accent));
        assert!(label.metrics.iter().all(|metric| metric.color==state.accent));
        for style in [crate::settings::MenuBarStyle::Percentages,crate::settings::MenuBarStyle::Progress] {
            for monochrome in [true,false] {
                let image = crate::tray_text::render(&[label.clone()],monochrome,state.accent,style).unwrap();
                for logo in [true,false] {
                    assert!(image.rgba().chunks_exact(4).enumerate().any(|(index,p)| p[..3]==state.accent && p[3]>240 && if logo { index as u32 % image.width()<46 } else { index as u32 % image.width()>=52 }));
                }
            }
        }
        assert_eq!(state.settings.codex_accounts["emma"].color,"#007aff","account card colors remain independently saved");
        // The workspace override must not hide warning colors, for any provider.
        state.settings.workspaces[0].usage.warning_colors = true;
        for provider in ["codex","claude","antigravity"] {
            let prefs = menu_bar_account_preferences(&state.settings,"emma");
            let profile = ProfileUsage {provider:provider.into(),quota:Some(quota(&[("weekly",96.0)])),..state.profiles[0].clone()};
            let (label,_) = account_label(&profile,&prefs,state.settings.menu_bar_warning_thresholds(),true,chrono::Utc::now());
            assert_eq!(label.color,[255,69,58]);
            assert!(label.metrics.iter().all(|metric| metric.color==[255,69,58]));
        }
        state.settings.active_workspace_id = None;
        assert_eq!(menu_bar_account_preferences(&state.settings,"emma").color,"#007aff");
    }

    #[test]
    fn enabling_workspace_warning_colors_repaints_cached_usage_immediately() {
        let mut app = AppSettings::default();
        let workspace: crate::workspaces::Workspace = serde_json::from_value(serde_json::json!({
            "id":"studio", "name":"Studio", "accent":"#007aff", "accountIds":["emma"],
            "claudeAccountIds":[], "usage":{"accountWindows":{"emma":["weekly"]}}
        })).unwrap();
        app.active_workspace_id = Some(workspace.id.clone());
        app.workspaces.push(workspace);
        let mut state = MenuBarState { accent: DEFAULT_ACCENT, settings: app.clone(), quota: None,
            profiles: vec![ProfileUsage { id: "emma".into(), name: "Emma Brooks".into(), active: true,
                provider: "codex".into(), quota: Some(quota(&[("weekly", 96.0)])) }], generation: 0 };
        app.workspaces[0].usage.warning_colors = true;
        assert!(state.update(app));
        assert_eq!(state.profiles.len(), 1, "enabling colors must preserve the sample needed for the immediate repaint");
        let prefs = CodexAccountPreferences { menu_bar_windows: vec!["weekly".into()], warning_colors: true, ..Default::default() };
        let (label, _) = account_label(&state.profiles[0], &prefs, &[75, 90, 95], true, chrono::Utc::now());
        assert_eq!(label.segments[0].1, [255, 69, 58]);
    }

    #[test]
    fn saved_color_levels_repaint_cached_metrics_and_icon_together() {
        let mut app = AppSettings::default();
        app.active_workspace_id = Some("studio".into());
        app.workspaces.push(serde_json::from_value(serde_json::json!({"id":"studio","name":"Studio","accent":"#007aff","accountIds":["emma"],"usage":{"warningColors":true,"warningThresholds":[75,90]}})).unwrap());
        let profile = ProfileUsage {id:"emma".into(),name:"Emma Brooks".into(),active:true,provider:"codex".into(),quota:Some(quota(&[("weekly",96.0)]))};
        let mut state = MenuBarState {accent:DEFAULT_ACCENT,settings:app.clone(),quota:None,profiles:vec![profile],generation:0};
        let prefs = CodexAccountPreferences {menu_bar_windows:vec!["weekly".into()],warning_colors:true,..Default::default()};
        let now = chrono::Utc::now();
        let levels = state.settings.menu_bar_warning_thresholds();
        assert_eq!(account_warning(&state.profiles[0],&prefs,levels,now).unwrap().1,[255,159,10]);
        for style in [crate::settings::MenuBarStyle::Percentages,crate::settings::MenuBarStyle::Progress] {
            let (label,_) = account_label(&state.profiles[0],&prefs,levels,true,now);
            assert_eq!(label.metrics[0].color,[255,159,10]);
            let image = crate::tray_text::render(&[label],true,[255,159,10],style).unwrap();
            assert!(image.rgba().chunks_exact(4).any(|p| p[..3]==[255,159,10] && p[3]>240));
        }
        app.workspaces[0].usage.warning_thresholds = Some(vec![]);
        assert!(state.update(app));
        assert_eq!(state.profiles.len(),1);
        assert!(account_warning(&state.profiles[0],&prefs,state.settings.menu_bar_warning_thresholds(),now).is_none());
    }

    #[test]
    fn workspace_thresholds_color_both_metrics_and_icon_and_names_use_initials() {
        let profile = ProfileUsage { id: "emma".into(), name: "Emma Louise Brooks".into(), active: true,
            provider: "codex".into(), quota: Some(quota(&[("weekly", 96.0)])) };
        let mut prefs = CodexAccountPreferences { menu_bar_windows: vec!["weekly".into()], warning_colors: true, ..Default::default() };
        let thresholds = [75, 90]; // Disabled critical level falls back to orange.
        let (label, tooltip) = account_label(&profile, &prefs, &thresholds, true, chrono::Utc::now());
        let orange = [255, 159, 10];
        assert_eq!(label.caption.as_deref(), Some("EB"));
        assert!(tooltip.contains("Emma Louise Brooks"));
        assert_eq!(label.segments[0].1, orange);
        let image = crate::tray_text::render(&[label], true, threshold_color(96.0, &thresholds).unwrap(), crate::settings::MenuBarStyle::Percentages).unwrap();
        for logo in [true, false] {
            assert!(image.rgba().chunks_exact(4).enumerate().any(|(index,pixel)| {
                let x = index as u32 % image.width();
                pixel[..3] == orange && pixel[3] > 240 && if logo { x < 46 } else { x >= 52 }
            }));
        }
        prefs.show_profile_label = false;
        assert!(account_label(&profile, &prefs, &thresholds, true, chrono::Utc::now()).0.caption.is_none());
        assert_eq!(threshold_color(96.0, &[]), None);
        assert_eq!(account_initials("  امیر محمدی "), "ام");
        assert_eq!(account_initials("Emma"), "E");
        assert_eq!(account_initials(""), "");
    }

    #[test]
    fn progress_metrics_follow_selected_limits_and_capacity_direction() {
        let now = chrono::Utc::now();
        let profile = ProfileUsage { id: "emma".into(), name: "Emma Brooks".into(), active: true, provider: "codex".into(), quota: Some(quota(&[("5-hour", 30.0), ("weekly", 96.0), ("monthly", 50.0)])) };
        let prefs = CodexAccountPreferences { menu_bar_windows: vec!["5-hour".into(), "weekly".into(), "15m".into()], warning_colors: true, ..Default::default() };
        let (remaining, _) = account_label(&profile, &prefs, &[75, 90, 95], true, now);
        assert_eq!(remaining.metrics.len(), 3);
        assert_eq!(remaining.metrics[0].fraction, Some(0.7));
        assert_eq!(remaining.metrics[1].fraction, Some(0.04));
        assert_eq!(remaining.metrics[1].color, [255, 69, 58]);
        assert_eq!(remaining.metrics[2].fraction, None);
        let (used, _) = account_label(&profile, &prefs, &[75, 90, 95], false, now);
        assert_eq!(used.metrics[0].fraction, Some(0.3));
        assert_eq!(used.metrics[1].fraction, Some(0.96));
    }

    #[test]
    fn limit_warning_tints_every_menu_bar_element_in_both_styles() {
        let now = chrono::Utc::now();
        let profile = ProfileUsage { id: "emma".into(), name: "Emma Brooks".into(), active: true,
            provider: "codex".into(), quota: Some(quota(&[("5-hour", 30.0), ("weekly", 96.0)])) };
        let prefs = CodexAccountPreferences { color: "#7c6cf0".into(), menu_bar_windows: vec!["5-hour".into(), "weekly".into(), "15m".into()], warning_colors: true, ..Default::default() };
        let (label, _) = account_label(&profile, &prefs, &[75, 90, 95], true, now);
        let red = [255, 69, 58];
        assert_eq!(label.color, red, "initials and window labels must share the warning color");
        assert!(label.segments.iter().all(|(_, color)| *color == red));
        assert!(label.metrics.iter().all(|metric| metric.color == red));
        let (_, icon_color) = account_warning(&profile, &prefs, &[75, 90, 95], now).unwrap();
        for style in [crate::settings::MenuBarStyle::Percentages, crate::settings::MenuBarStyle::Progress] {
            let image = crate::tray_text::render(&[label.clone()], true, icon_color, style).unwrap();
            assert!(image.rgba().chunks_exact(4).filter(|pixel| pixel[3] == 255).all(|pixel| pixel[..3] == red), "every visible element must share the same warning hue");
        }
    }

    #[test]
    fn uniform_warning_follows_enabled_levels_and_restores_identity_on_recovery() {
        let now = chrono::Utc::now();
        let mut profile = ProfileUsage { id: "emma".into(), name: "Emma Brooks".into(), active: true,
            provider: "codex".into(), quota: Some(quota(&[("5-hour", 80.0), ("weekly", 96.0), ("monthly", 100.0)])) };
        let mut prefs = CodexAccountPreferences { color: "#7c6cf0".into(), menu_bar_windows: vec!["5-hour".into(), "weekly".into()], warning_colors: true, ..Default::default() };
        assert_eq!(account_warning(&profile, &prefs, &[75, 90], now), Some((96.0, [255, 159, 10])));
        profile.quota.as_mut().unwrap().windows.as_mut().unwrap()[1].resets_at = Some((now - chrono::Duration::seconds(1)).to_rfc3339());
        assert_eq!(account_warning(&profile, &prefs, &[75, 90, 95], now), Some((80.0, [255, 214, 10])), "expired and unselected limits cannot escalate the tint");
        profile.quota.as_mut().unwrap().windows.as_mut().unwrap()[0].percent = 20.0;
        assert_eq!(account_warning(&profile, &prefs, &[75, 90, 95], now), None);
        assert_eq!(account_label(&profile, &prefs, &[75, 90, 95], true, now).0.color, [124, 108, 240]);
        profile.quota.as_mut().unwrap().windows.as_mut().unwrap()[0].percent = 100.0;
        assert_eq!(account_warning(&profile, &prefs, &[], now), None);
        prefs.warning_colors = false;
        assert_eq!(account_warning(&profile, &prefs, &[75, 90, 95], now), None);
        prefs.warning_colors = true;
        profile.quota.as_mut().unwrap().error = Some("Unavailable".into());
        assert_eq!(account_warning(&profile, &prefs, &[75, 90, 95], now), None);
    }

    #[test]
    fn account_warning_colors_are_uniform_and_keep_unavailable_selections() {
        let now = chrono::Utc::now();
        let mut prefs = CodexAccountPreferences {
            color: "#7c6cf0".into(),
            menu_bar_windows: vec!["5-hour".into(), "weekly".into(), "15m".into()],
            warning_colors: true,
            ..Default::default()
        };
        let mut profile = ProfileUsage { id: "work".into(), name: "Work".into(), active: true, provider: "codex".into(), quota: Some(quota(&[("5-hour", 30.0), ("Weekly", 96.0)])) };
        let (label, _) = account_label(&profile, &prefs, &[75, 90, 95], true, now);
        assert_eq!(label.text, "5h 70%  1w 4%  15m —");
        assert_eq!(label.segments[0].1, [255, 69, 58]);
        assert_eq!(label.segments[1].1, [255, 69, 58]);
        assert_eq!(label.segments[2].1, [255, 69, 58]);
        assert_eq!(account_label(&profile, &prefs, &[], true, now).0.segments[1].1, [124, 108, 240]);
        prefs.warning_colors = false;
        assert_eq!(account_label(&profile, &prefs, &[75, 90, 95], true, now).0.segments[1].1, [124, 108, 240]);
        profile.quota.as_mut().unwrap().error = Some("Unavailable".into());
        assert_eq!(account_label(&profile, &prefs, &[75, 90, 95], true, now).0.text, "5h —  1w —  15m —");
    }

    #[test]
    fn switch_cycle_requires_explicit_source_and_target_membership_and_supports_weekly_only() {
        let now = chrono::Utc::now();
        let sample = |id: &str, active: bool, used: f64| {
            let mut q = quota(&[("Weekly", used)]);
            q.windows.as_mut().unwrap()[0].resets_at = Some((now + chrono::Duration::days(2)).to_rfc3339());
            ProfileUsage { id: id.into(), name: id.into(), active, provider: "codex".into(), quota: Some(q) }
        };
        let samples = vec![sample("first", true, 100.0), sample("excluded", false, 0.0), sample("second", false, 20.0)];
        let mut settings = AppSettings::default();
        settings.codex_accounts.insert("first".into(), CodexAccountPreferences { auto_switch: true, ..Default::default() });
        settings.codex_accounts.insert("second".into(), CodexAccountPreferences { auto_switch: true, ..Default::default() });
        assert_eq!(auto_switch_target(&samples, &settings, now).unwrap().1.id, "second");
        settings.codex_accounts.get_mut("second").unwrap().auto_switch = false;
        assert!(auto_switch_target(&samples, &settings, now).is_none());
        settings.codex_accounts.get_mut("second").unwrap().auto_switch = true;
        settings.codex_accounts.get_mut("first").unwrap().auto_switch = false;
        assert!(auto_switch_target(&samples, &settings, now).is_none());
        settings.codex_accounts.get_mut("first").unwrap().auto_switch = true;
        settings.codex_accounts.get_mut("second").unwrap().known_windows = vec!["weekly".into(), "5-hour".into()];
        assert!(auto_switch_target(&samples, &settings, now).is_none());
    }

    #[test]
    fn named_provider_buckets_keep_their_own_limit_and_warning_color() {
        let now = chrono::Utc::now();
        let profile = ProfileUsage { id: "google".into(), name: "Google work".into(), provider: "antigravity".into(), active: false, quota: Some(quota(&[("Gemini Pro", 96.0), ("Gemini Flash", 20.0)])) };
        let prefs = CodexAccountPreferences { menu_bar_windows: vec!["Gemini Pro".into()], warning_colors: true, ..Default::default() };
        let (label, _) = account_label(&profile, &prefs, &[75, 90, 95], true, now);
        assert_eq!(label.text, "Gemini Pro 4%");
        assert_eq!(label.segments[0].1, [255, 69, 58]);
        assert_eq!(warning_color(75.0), [255, 214, 10]);
        assert_eq!(warning_color(90.0), [255, 159, 10]);
    }

    #[test]
    fn switch_threshold_uses_remaining_capacity_and_rejects_targets_inside_threshold() {
        let now = chrono::Utc::now();
        let quota = |used| QuotaSet { provider: "codex".into(), windows: Some(vec![crate::quota::QuotaWindow { window: "weekly".into(), percent: used, resets_at: Some((now + chrono::Duration::hours(1)).to_rfc3339()), status: None }]), error: None, reset_credits: None, as_of: None };
        let mut samples = vec![ProfileUsage { id: "first".into(), name: "First".into(), active: true, provider: "codex".into(), quota: Some(quota(97.9)) }, ProfileUsage { id: "second".into(), name: "Second".into(), active: false, provider: "codex".into(), quota: Some(quota(10.0)) }];
        let mut settings = AppSettings::default();
        for id in ["first", "second"] { settings.codex_accounts.insert(id.into(), CodexAccountPreferences { auto_switch: true, ..Default::default() }); }
        assert!(auto_switch_target(&samples, &settings, now).is_none());
        samples[0].quota = Some(quota(98.0));
        assert_eq!(auto_switch_target(&samples, &settings, now).unwrap().1.id, "second");
        samples[1].quota = Some(quota(98.0));
        assert!(auto_switch_target(&samples, &settings, now).is_none());
    }

    #[test]
    fn cycles_monitor_without_the_menu_bar_and_legacy_flags_do_not_enroll_accounts() {
        let mut app = AppSettings { auto_switch_enabled: true, ..Default::default() };
        assert!(!monitoring_enabled(&app));
        for id in ["first", "second"] { app.codex_accounts.insert(id.into(), CodexAccountPreferences { auto_switch: true, ..Default::default() }); }
        assert!(monitoring_enabled(&app));
        app.codex_accounts.remove("second");
        assert!(!monitoring_enabled(&app));
    }

    #[test]
    fn workspace_policy_never_enrolls_accounts_from_another_workspace() {
        let mut app = AppSettings { workspace_version: 1, active_workspace_id: Some("studio".into()), ..Default::default() };
        app.workspaces = vec![crate::workspaces::Workspace {
            id: "studio".into(), name: "Studio".into(), accent: "#7c6cf0".into(),
            account_ids: vec!["first".into(), "second".into()], provider_ids: None, claude_account_ids: None, antigravity_account_ids: vec![], account_preferences: None, usage: Default::default(),
        }];
        for id in ["first", "second", "outside"] {
            app.codex_accounts.insert(id.into(), CodexAccountPreferences { auto_switch: true, ..Default::default() });
        }
        app.workspaces[0].usage.menu_bar_enabled = false;
        assert!(!monitoring_enabled(&app));
        app.workspaces[0].usage.auto_switch_enabled = true;
        assert!(codex_cycle_enabled(&app));
        assert!(!app.contains_codex_account("outside"));
        app.workspaces[0].account_ids.pop();
        assert!(!codex_cycle_enabled(&app));
        app.workspaces[0].usage.menu_bar_enabled = true;
        assert!(app.codex_menu_bar_enabled());
        assert!(!app.menu_bar_enabled);
    }

    #[test]
    fn template_is_a_transparent_glyph_instead_of_an_opaque_tile() {
        let icon = render(DEFAULT_ACCENT);
        assert_eq!((icon.width(), icon.height()), (46, 36));
        let pixels: Vec<_> = icon.rgba().chunks_exact(4).collect();
        assert_eq!(pixels[0][3], 0);
        assert_eq!(pixels[pixels.len() - 1][3], 0);
        let visible = pixels.iter().filter(|pixel| pixel[3] > 128).count();
        assert!(visible > pixels.len() / 10, "logo must remain visible");
        assert!(
            visible < pixels.len() / 2,
            "background must stay transparent"
        );
    }
}
