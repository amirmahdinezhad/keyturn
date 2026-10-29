//! Immediate switching with an explicit confirmation gate for active desktop turns.
//! Verification covers the selected auth cache and a newly launched app process.
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::{profiles, tray};
#[cfg(not(test))]
const BUNDLE_ID: &str = "com.openai.codex";
// Native lifecycle regression checks must never control the user's Codex app.
#[cfg(test)]
const BUNDLE_ID: &str = "com.7skylab.keyturn.restart-fixture";
fn switch_timing(_automatic: bool) -> (Duration, &'static str) {
    (Duration::ZERO, "checking")
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchStatus {
    pub profile_id: String,
    pub profile_name: String,
    pub deadline_at: String,
    pub phase: String,
    pub error: Option<String>,
    pub process_id: Option<i32>,
    pub credentials_verified: bool,
    pub running_threads: u32,
    pub activity_uncertain: bool,
}

#[derive(Default)]
struct State {
    generation: u64,
    status: Option<SwitchStatus>,
    run_now: bool,
    automatic_members: Option<(String, String)>,
}
#[derive(Clone, Default)]
pub struct DesktopSwitch(Arc<Mutex<State>>);

/// Only the Codex desktop bundle is controlled, never the Keyturn/CLI processes.
pub trait DesktopDriver {
    fn ensure_available(&self) -> Result<(), String>;
    fn pid(&self) -> Result<Option<i32>, String>;
    fn quit(&self) -> Result<(), String>;
    fn reopen(&self) -> Result<(), String>;
    fn activity(&self) -> crate::codex_activity::Activity { crate::codex_activity::Activity::Idle }
    fn request_attention(&self) {}

}

/// Check the destination session immediately before quit.
struct CheckedDesktop<D, F> {
    desktop: D,
    check_login: F,
}
impl<D: DesktopDriver, F: Fn() -> Result<(), String>> DesktopDriver for CheckedDesktop<D, F> {
    fn ensure_available(&self) -> Result<(), String> {
        self.desktop.ensure_available()?;
        (self.check_login)()
    }
    fn pid(&self) -> Result<Option<i32>, String> { self.desktop.pid() }
    fn quit(&self) -> Result<(), String> { self.desktop.quit() }
    fn reopen(&self) -> Result<(), String> { self.desktop.reopen() }
    fn activity(&self) -> crate::codex_activity::Activity { self.desktop.activity() }
    fn request_attention(&self) { self.desktop.request_attention(); }
}
#[derive(Default)]
pub struct NativeCodex {
    bundle_path: std::sync::OnceLock<std::path::PathBuf>,
    app: Option<AppHandle>,
}

/// A dev worker owned by Codex disappears with its host before reopening it.
fn ensure_independent_restart(app_pids: &[i32]) -> Result<(), String> {
    if app_pids.is_empty() {
        return Ok(());
    }
    let mut pid = std::process::id() as i32;
    for _ in 0..64 {
        if app_pids.contains(&pid) {
            return Err("Keyturn was started by Codex and would close with it. Start Keyturn independently with pnpm run run, then retry. Your login has not changed.".into());
        }
        if pid <= 1 {
            return Ok(());
        }
        let output = std::process::Command::new("/bin/ps")
            .args(["-o", "ppid=", "-p", &pid.to_string()])
            .output().map_err(|_| "Cannot check Keyturn's parent process. Restart Keyturn independently before switching.")?;
        if !output.status.success() {
            return Err("Cannot check Keyturn's parent process. Restart Keyturn independently before switching.".into());
        }
        pid = String::from_utf8_lossy(&output.stdout).trim().parse::<i32>()
            .map_err(|_| "Cannot check Keyturn's parent process. Restart Keyturn independently before switching.")?;
    }
    Err("Cannot confirm Keyturn can survive a Codex restart. Restart Keyturn independently before switching.".into())
}
impl DesktopDriver for NativeCodex {
    fn activity(&self) -> crate::codex_activity::Activity {
        match self.pid() {
            Ok(Some(pid)) => crate::codex_activity::probe(&[pid]),
            Ok(None) => crate::codex_activity::Activity::Idle,
            Err(_) => crate::codex_activity::Activity::Unknown,
        }
    }
    fn request_attention(&self) {
        if let Some(app) = &self.app {
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || crate::show_panel(&handle));
            let _ = app.emit("codex-switch-attention", ());
        }
    }

    fn ensure_available(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::NSWorkspace;
            use objc2_foundation::NSString;
            return objc2::rc::autoreleasepool(|_| {
                use objc2_app_kit::NSRunningApplication;
                let bundle = NSString::from_str(BUNDLE_ID);
                let running =
                    NSRunningApplication::runningApplicationsWithBundleIdentifier(&bundle);
                ensure_independent_restart(
                    &running
                        .iter()
                        .map(|app| app.processIdentifier())
                        .collect::<Vec<_>>(),
                )?;
                let url = running.iter().find_map(|app| app.bundleURL()).or_else(|| {
                    NSWorkspace::sharedWorkspace().URLForApplicationWithBundleIdentifier(&bundle)
                });
                let path = url.and_then(|url| url.path()).map(|path| std::path::PathBuf::from(path.to_string()))
                    .filter(|path| path.is_dir())
                    .ok_or("Codex desktop is not installed. Use Activate for CLI only, or install Codex first.")?;
                let _ = self.bundle_path.set(path);
                Ok(())
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("Automatic Codex restart is currently available on macOS. Use Activate for CLI only.".into())
        }
    }
    fn pid(&self) -> Result<Option<i32>, String> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::NSRunningApplication;
            use objc2_foundation::NSString;
            return objc2::rc::autoreleasepool(|_| {
                let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(
                    &NSString::from_str(BUNDLE_ID),
                );
                Ok(apps.iter().next().map(|app| app.processIdentifier()))
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("Automatic Codex restart is currently available on macOS.".into())
        }
    }
    fn quit(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::NSRunningApplication;
            use objc2_foundation::NSString;
            return objc2::rc::autoreleasepool(|_| {
                let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(
                    &NSString::from_str(BUNDLE_ID),
                );
                ensure_independent_restart(
                    &apps
                        .iter()
                        .map(|a| a.processIdentifier())
                        .collect::<Vec<_>>(),
                )?;
                for app in apps.iter() {
                    if !app.terminate() {
                        return Err("Codex did not accept the quit request. Close it manually and try again.".into());
                    }
                }
                Ok(())
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("Automatic Codex restart is currently available on macOS.".into())
        }
    }
    fn reopen(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let path = self
                .bundle_path
                .get()
                .ok_or("Cannot locate the Codex app selected before restart. Open it manually.")?;
            let result = std::process::Command::new("/usr/bin/open")
                .arg("-a")
                .arg(path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map_err(|_| "Cannot reopen Codex. Open it manually.")?;
            if result.success() {
                Ok(())
            } else {
                Err("Cannot reopen Codex. Open it manually.".into())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("Automatic Codex restart is currently available on macOS.".into())
        }
    }
}

fn wait_for(
    driver: &impl DesktopDriver,
    ready: impl Fn(Option<i32>) -> bool,
) -> Result<Option<i32>, String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let pid = driver.pid()?;
        if ready(pid) {
            return Ok(pid);
        }
        if Instant::now() >= deadline {
            return Err("Codex did not finish restarting. Your previous login can be restored from its saved profile.".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Install credentials only after graceful exit. Verification checks file/process,
/// not the account displayed inside Codex. Closures keep the transaction testable.
pub(crate) fn restart_with(
    driver: &impl DesktopDriver,
    mut phase: impl FnMut(&str),
    activate: impl FnOnce() -> Result<(), String>,
    verify: impl FnOnce() -> Result<(), String>,
) -> Result<i32, String> {
    driver.ensure_available()?;
    let old_pid = driver.pid()?;
    phase("quitting");
    driver.quit()?;
    wait_for(driver, |pid| pid.is_none())?;
    phase("activating");
    if let Err(error) = activate() {
        // A failed file transaction must not leave the user's old account closed.
        phase("reopening");
        return match driver.reopen() {
            Ok(()) => Err(format!(
                "{error} Codex was reopened with the previous login."
            )),
            Err(_) => Err(format!(
                "{error} Open Codex manually; the previous login is still active."
            )),
        };
    }
    phase("reopening");
    driver.reopen()?;
    let new_pid =
        wait_for(driver, |pid| pid.is_some() && pid != old_pid)?.ok_or("Codex is not running.")?;
    verify()?;
    Ok(new_pid)
}

impl DesktopSwitch {
    pub fn status(&self) -> Option<SwitchStatus> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .status
            .clone()
    }
    pub fn cancel(&self) -> Result<Option<SwitchStatus>, String> {
        let mut state = self.0.lock().map_err(|_| "Restart state is unavailable.")?;
        if let Some(status) = &mut state.status {
            if !matches!(status.phase.as_str(), "checking" | "awaiting_confirmation" | "countdown") {
                return Err("The restart has already started. Wait for it to finish.".into());
            }
            status.phase = "cancelled".into();
        }
        state.generation += 1;
        Ok(state.status.clone())
    }
    pub fn run_now(&self) -> Result<Option<SwitchStatus>, String> {
        let mut state = self.0.lock().map_err(|_| "Restart state is unavailable.")?;
        if state.status.as_ref().is_none_or(|s| !matches!(s.phase.as_str(), "awaiting_confirmation" | "countdown")) {
            return Err("No switch is waiting for confirmation.".into());
        }
        state.run_now = true;
        Ok(state.status.clone())
    }
    pub fn schedule(&self, app: AppHandle, id: &str, allow_limited: bool) -> Result<SwitchStatus, String> {
        self.schedule_inner(app, id, None, allow_limited)
    }

    /// Automatic jobs start immediately; active turns require explicit confirmation.
    pub fn schedule_automatic(&self, app: AppHandle, source: &str, target: &str) -> Result<SwitchStatus, String> {
        self.schedule_inner(app, target, Some((source.into(), target.into())), false)
    }

    pub fn cancel_unenrolled(&self, settings: &crate::settings::AppSettings) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let unenrolled = state.automatic_members.as_ref().is_some_and(|(source, target)| {
            settings.active_workspace().is_some_and(|w| !w.usage.auto_switch_enabled)
                || [source, target].iter().any(|id| !settings.contains_codex_account(id) || !settings.codex_accounts.get(*id).is_some_and(|p| p.auto_switch))
        });
        if unenrolled && state.status.as_ref().is_some_and(|s| matches!(s.phase.as_str(), "checking" | "awaiting_confirmation" | "countdown")) {
            state.status.as_mut().unwrap().phase = "cancelled".into();
            state.generation += 1;
        }
    }

    fn schedule_inner(&self, app: AppHandle, id: &str, automatic_members: Option<(String, String)>, allow_limited: bool) -> Result<SwitchStatus, String> {
        let driver = NativeCodex { app: Some(app.clone()), ..Default::default() };
        driver.ensure_available()?;
        let data = profiles::list()?;
        if let Some(error) = data.switch_error {
            return Err(error);
        }
        let profile = data
            .profiles
            .iter()
            .find(|p| p.id == id && p.connected)
            .ok_or("Connect this profile before restarting Codex.")?;
        let (grace, phase) = switch_timing(automatic_members.is_some());
        let status = SwitchStatus {
            profile_id: id.into(),
            profile_name: profile.name.clone(),
            deadline_at: (chrono::Utc::now() + chrono::Duration::seconds(grace.as_secs() as i64))
                .to_rfc3339(),
            phase: phase.into(),
            error: None,
            process_id: None,
            credentials_verified: false,
            running_threads: 0, activity_uncertain: false,
        };
        let switch_id = id.to_owned();
        let verify_id = id.to_owned();
        let verify_id_for_defaults = id.to_owned();
        let check_id = id.to_owned();
        let driver = CheckedDesktop {
            desktop: driver,
            check_login: move || profiles::check_switch_login(&check_id, allow_limited),
        };
        self.begin(status.clone(), automatic_members, grace, driver,
            move || profiles::activate(&switch_id).map(|_| ()),
            move || {
                if profiles::list()?.active_profile_id.as_deref() == Some(&verify_id) { Ok(()) }
                else { Err("The active login file does not match this profile. Check the account in Codex.".into()) }
            },
            move || {
                if let Ok((saved, ())) = crate::settings::update_settings(&crate::paths::settings_file(), |settings| {
                    crate::workspaces::record_account_switch(settings, "codex", &verify_id_for_defaults);
                    Ok(())
                }) {
                    app.state::<tray::MenuBarMonitor>().update(&app, saved);
                }
                app.state::<tray::MenuBarMonitor>().account_changed(&app);
                let _ = app.emit("codex-profile-switched", &verify_id_for_defaults);
            })?;
        Ok(status)
    }

    fn confirm_activity(&self, driver: &impl DesktopDriver, generation: u64) -> Result<bool, String> {
        use crate::codex_activity::Activity;
        // Called after login validation, immediately before quit, so an active
        // turn that started during preflight also blocks automatic replacement.
        let activity = driver.activity();
        let needs_confirmation = activity != Activity::Idle;
        {
            let mut state = self.0.lock().map_err(|_| "Switch state is unavailable.")?;
            if state.generation != generation { return Err("Switch cancelled.".into()); }
            if let Some(status) = &mut state.status {
                status.running_threads = if let Activity::Running(count) = activity { count } else { 0 };
                status.activity_uncertain = activity == Activity::Unknown;
                status.phase = if needs_confirmation { "awaiting_confirmation" } else { "quitting" }.into();
            }
        }
        if !needs_confirmation { return Ok(false); }
        driver.request_attention();
        loop {
            {
                let mut state = self.0.lock().map_err(|_| "Switch state is unavailable.")?;
                if state.generation != generation { return Err("Switch cancelled.".into()); }
                if state.run_now {
                    state.run_now = false;
                    if let Some(status) = &mut state.status { status.phase = "quitting".into(); }
                    return Ok(true);
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn begin(
        &self,
        status: SwitchStatus,
        automatic_members: Option<(String, String)>,
        grace: Duration,
        driver: impl DesktopDriver + Send + 'static,
        activate: impl FnOnce() -> Result<(), String> + Send + 'static,
        verify: impl FnOnce() -> Result<(), String> + Send + 'static,
        finished: impl FnOnce() + Send + 'static,
    ) -> Result<(), String> {
        let generation = {
            let mut state = self.0.lock().map_err(|_| "Restart state is unavailable.")?;
            if state.status.as_ref().is_some_and(|s| {
                matches!(
                    s.phase.as_str(),
                    "checking" | "awaiting_confirmation" | "countdown" | "quitting" | "activating" | "reopening"
                )
            }) {
                return Err("A Codex restart is already pending. Cancel it before choosing another profile.".into());
            }
            state.generation += 1;
            state.run_now = false;
            state.status = Some(status.clone());
            state.automatic_members = automatic_members;
            state.generation
        };
        let monitor = self.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + grace;
            loop {
                let mut state = monitor.0.lock().unwrap_or_else(|e| e.into_inner());
                if state.generation != generation {
                    return;
                }
                let running = driver.pid();
                if let Err(error) = running {
                    if let Some(status) = &mut state.status {
                        status.phase = "failed".into();
                        status.error = Some(error);
                    }
                    return;
                }
                let ready =
                    state.run_now || Instant::now() >= deadline || running.ok().flatten().is_none();
                drop(state);
                if ready {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            let guarded = GuardedDesktop { desktop: driver, monitor: monitor.clone(), generation };
            let result = restart_with(
                &guarded,
                |phase| {
                    let mut state = monitor.0.lock().unwrap_or_else(|e| e.into_inner());
                    if state.generation == generation && phase != "quitting" {
                        if let Some(status) = &mut state.status { status.phase = phase.into(); }
                    }
                },
                activate,
                verify,
            );
            if monitor.0.lock().unwrap_or_else(|e| e.into_inner()).generation != generation { return; }
            // Persist the new display identity before publishing success. A failed
            // restart must not select the attempted destination in the menu bar.
            if result.is_ok() { finished(); }
            {
                let mut state = monitor.0.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(status) = &mut state.status {
                    match result {
                        Ok(pid) => {
                            status.phase = "verified".into();
                            status.process_id = Some(pid);
                            status.credentials_verified = true;
                        }
                        Err(error) => {
                            status.phase = "failed".into();
                            status.error = Some(error);
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

/// The gate is inside quit so neither delayed credential checks nor a hidden
/// panel can bypass user confirmation for work that is currently running.
struct GuardedDesktop<D> { desktop: D, monitor: DesktopSwitch, generation: u64 }
impl<D: DesktopDriver> DesktopDriver for GuardedDesktop<D> {
    fn ensure_available(&self) -> Result<(), String> { self.desktop.ensure_available() }
    fn pid(&self) -> Result<Option<i32>, String> { self.desktop.pid() }
    fn quit(&self) -> Result<(), String> {
        if self.monitor.confirm_activity(&self.desktop, self.generation)? {
            // The prompt may have remained open for a long time. Recheck the
            // destination session after consent, before touching the live app.
            self.desktop.ensure_available()?;
        }
        self.desktop.quit()
    }
    fn reopen(&self) -> Result<(), String> { self.desktop.reopen() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_switch_has_no_grace_and_finishes_without_publishing_a_countdown() {
        let (grace, phase) = switch_timing(false);
        assert!(grace.is_zero()); assert_eq!(phase, "checking");
        assert_eq!(switch_timing(true), (Duration::ZERO, "checking"));
        let mut status = pending(); status.phase = phase.into();
        let monitor = DesktopSwitch::default(); let desktop = FakeDesktop::new();
        let events = desktop.events.clone(); let activation = events.clone();
        monitor.begin(status, None, grace, desktop, move || { activation.lock().unwrap().push("activate"); Ok(()) }, || Ok(()), || {}).unwrap();
        assert_ne!(monitor.status().unwrap().phase, "countdown");
        await_phase(&monitor, "verified");
        assert_eq!(*events.lock().unwrap(), ["quit", "activate", "open"]);
    }

    #[test]
    fn failed_switch_does_not_publish_a_new_active_or_menu_bar_account() {
        let monitor = DesktopSwitch::default();
        let (grace, phase) = switch_timing(false); let mut status = pending(); status.phase = phase.into();
        let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let completion = completed.clone();
        monitor.begin(status, None, grace, FakeDesktop::new(), || Err("write failed".into()), || Ok(()), move || { completion.store(true, std::sync::atomic::Ordering::SeqCst); }).unwrap();
        await_phase(&monitor, "failed");
        assert!(!completed.load(std::sync::atomic::Ordering::SeqCst), "failed switch must never publish completion");
        assert!(!monitor.status().unwrap().credentials_verified);
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires the independent Keyturn Restart Fixture app"]
    fn native_restart_installs_after_exit_and_launches_the_same_app_again() {
        let driver = NativeCodex::default();
        assert!(driver.pid().unwrap().is_some(), "start the fixture first");
        let installed = std::sync::atomic::AtomicBool::new(false);
        let result = restart_with(
            &driver,
            |_| {},
            || {
                installed.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            },
            || {
                assert!(installed.load(std::sync::atomic::Ordering::SeqCst));
                Ok(())
            },
        );
        assert!(
            result.is_ok(),
            "desktop stayed closed: {}",
            result.unwrap_err()
        );
    }
    #[derive(Clone)]
    struct FakeDesktop {
        pid: Arc<Mutex<Option<i32>>>,
        events: Arc<Mutex<Vec<&'static str>>>,
        available: bool,
        accepts_quit: bool,
    }
    impl FakeDesktop {
        fn new() -> Self {
            Self {
                pid: Arc::new(Mutex::new(Some(1))),
                events: Arc::default(),
                available: true,
                accepts_quit: true,
            }
        }
    }
    impl DesktopDriver for FakeDesktop {
        fn ensure_available(&self) -> Result<(), String> {
            if self.available {
                Ok(())
            } else {
                Err("not installed".into())
            }
        }
        fn pid(&self) -> Result<Option<i32>, String> {
            Ok(*self.pid.lock().unwrap())
        }
        fn quit(&self) -> Result<(), String> {
            self.events.lock().unwrap().push("quit");
            if !self.accepts_quit {
                return Err("quit declined".into());
            }
            *self.pid.lock().unwrap() = None;
            Ok(())
        }
        fn reopen(&self) -> Result<(), String> {
            self.events.lock().unwrap().push("open");
            *self.pid.lock().unwrap() = Some(2);
            Ok(())
        }
    }
    fn pending() -> SwitchStatus {
        SwitchStatus {
            profile_id: "work".into(),
            profile_name: "Work".into(),
            deadline_at: "2030-01-01T00:00:00Z".into(),
            phase: "countdown".into(),
            error: None,
            process_id: None,
            credentials_verified: false,
            running_threads: 0, activity_uncertain: false,
        }
    }
    struct BusyDesktop { desktop: FakeDesktop, activity: crate::codex_activity::Activity }
    impl DesktopDriver for BusyDesktop {
        fn ensure_available(&self) -> Result<(), String> { self.desktop.ensure_available() }
        fn pid(&self) -> Result<Option<i32>, String> { self.desktop.pid() }
        fn quit(&self) -> Result<(), String> { self.desktop.quit() }
        fn reopen(&self) -> Result<(), String> { self.desktop.reopen() }
        fn activity(&self) -> crate::codex_activity::Activity { self.activity }
    }
    #[test]
    fn active_threads_pause_automatic_switch_until_explicit_confirmation() {
        let monitor = DesktopSwitch::default(); let desktop = FakeDesktop::new(); let events = desktop.events.clone();
        let (grace, phase) = switch_timing(true); let mut status = pending(); status.phase = phase.into();
        monitor.begin(status, Some(("old".into(), "work".into())), grace,
            BusyDesktop { desktop, activity: crate::codex_activity::Activity::Running(2) }, || Ok(()), || Ok(()), || {}).unwrap();
        await_phase(&monitor, "awaiting_confirmation");
        assert_eq!(monitor.status().unwrap().running_threads, 2);
        assert!(events.lock().unwrap().is_empty(), "active work must not be closed before consent");
        monitor.run_now().unwrap(); await_phase(&monitor, "verified");
        assert_eq!(*events.lock().unwrap(), ["quit", "open"]);
    }
    #[test]
    fn cancelling_unknown_activity_keeps_the_app_and_login_unchanged() {
        let monitor = DesktopSwitch::default(); let desktop = FakeDesktop::new(); let events = desktop.events.clone();
        let mut status = pending(); status.phase = "checking".into();
        monitor.begin(status, None, Duration::ZERO,
            BusyDesktop { desktop, activity: crate::codex_activity::Activity::Unknown },
            || panic!("cancelled switch replaced credentials"), || Ok(()), || {}).unwrap();
        await_phase(&monitor, "awaiting_confirmation");
        assert!(monitor.status().unwrap().activity_uncertain);
        monitor.cancel().unwrap();
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(monitor.status().unwrap().phase, "cancelled");
        assert!(events.lock().unwrap().is_empty());
    }
    #[test]
    fn destination_is_revalidated_after_waiting_for_user_confirmation() {
        let monitor = DesktopSwitch::default(); let mut status = pending(); status.phase = "checking".into();
        let desktop = FakeDesktop::new(); let events = desktop.events.clone();
        let validations = Arc::new(std::sync::atomic::AtomicU32::new(0)); let checks = validations.clone();
        let checked = CheckedDesktop {
            desktop: BusyDesktop { desktop, activity: crate::codex_activity::Activity::Running(1) },
            check_login: move || if checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 { Ok(()) } else { Err("session expired while awaiting confirmation".into()) },
        };
        monitor.begin(status, None, Duration::ZERO, checked,
            || panic!("rejected login replaced credentials"), || Ok(()), || {}).unwrap();
        await_phase(&monitor, "awaiting_confirmation"); monitor.run_now().unwrap(); await_phase(&monitor, "failed");
        assert_eq!(validations.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert!(events.lock().unwrap().is_empty(), "rejected destination must not close the working app");
    }

    #[test]
    fn opting_out_cancels_an_automatic_switch_waiting_for_active_work_confirmation() {
        let monitor = DesktopSwitch::default(); let mut status = pending(); status.phase = "checking".into();
        let desktop = FakeDesktop::new(); let events = desktop.events.clone();
        monitor.begin(status, Some(("old".into(), "work".into())), Duration::ZERO,
            BusyDesktop { desktop, activity: crate::codex_activity::Activity::Running(1) },
            || panic!("unenrolled account replaced credentials"), || Ok(()), || {}).unwrap();
        await_phase(&monitor, "awaiting_confirmation");
        monitor.cancel_unenrolled(&crate::settings::AppSettings::default());
        assert_eq!(monitor.status().unwrap().phase, "cancelled"); assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn rejected_destination_never_quits_or_replaces_the_current_login() {
        let desktop = FakeDesktop::new();
        let events = desktop.events.clone();
        let installed = std::cell::Cell::new(false);
        let checked = CheckedDesktop {
            desktop,
            check_login: || Err("session rejected — reconnect this account".into()),
        };
        let result = restart_with(&checked, |_| {}, || {
            installed.set(true);
            Ok(())
        }, || Ok(()));
        assert!(result.is_err(), "an invalid saved login was accepted for restart");
        assert!(!installed.get(), "invalid credentials replaced the working login");
        assert!(events.lock().unwrap().is_empty(), "the user's app was closed");
    }
    #[test]
    #[cfg(unix)]
    fn worker_cannot_quit_its_own_host_before_installing_and_reopening() {
        struct OwnedDesktop(FakeDesktop, i32);
        impl DesktopDriver for OwnedDesktop {
            fn ensure_available(&self) -> Result<(), String> {
                Ok(())
            }
            fn pid(&self) -> Result<Option<i32>, String> {
                self.0.pid()
            }
            fn quit(&self) -> Result<(), String> {
                ensure_independent_restart(&[self.1])?;
                self.0.quit()
            }
            fn reopen(&self) -> Result<(), String> {
                self.0.reopen()
            }
        }
        let parent = std::process::Command::new("/bin/ps")
            .args(["-o", "ppid=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        let parent = String::from_utf8(parent.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let desktop = OwnedDesktop(FakeDesktop::new(), parent);
        let installed = std::cell::Cell::new(false);
        let outcome = restart_with(
            &desktop,
            |_| {},
            || {
                installed.set(true);
                Ok(())
            },
            || Ok(()),
        );
        assert!(
            outcome.is_err(),
            "restart accepted: quitting the host also kills this worker before reopen"
        );
        assert!(!installed.get());
        assert!(desktop.0.events.lock().unwrap().is_empty());
    }
    fn await_phase(monitor: &DesktopSwitch, phase: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while monitor.status().unwrap().phase != phase {
            assert!(
                Instant::now() < deadline,
                "restart did not reach expected phase"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn expired_timer_quits_before_replacement_then_reopens_and_verifies() {
        let desktop = FakeDesktop::new();
        let events = desktop.events.clone();
        let activation_events = events.clone();
        let monitor = DesktopSwitch::default();
        monitor
            .begin(
                pending(),
                None,
                Duration::from_millis(10),
                desktop,
                move || {
                    activation_events.lock().unwrap().push("activate");
                    Ok(())
                },
                || Ok(()),
                || {},
            )
            .unwrap();
        await_phase(&monitor, "verified");
        assert_eq!(*events.lock().unwrap(), ["quit", "activate", "open"]);
        assert!(monitor.status().unwrap().credentials_verified);
        assert_eq!(monitor.status().unwrap().process_id, Some(2));
    }
    #[test]
    fn cancellation_prevents_background_restart_even_after_deadline() {
        let desktop = FakeDesktop::new();
        let events = desktop.events.clone();
        let monitor = DesktopSwitch::default();
        monitor
            .begin(
                pending(),
                None,
                Duration::from_millis(50),
                desktop,
                || panic!("cancelled switch activated"),
                || Ok(()),
                || {},
            )
            .unwrap();
        monitor.cancel().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert!(events.lock().unwrap().is_empty());
        assert_eq!(monitor.status().unwrap().phase, "cancelled");
    }

    #[test]
    fn leaving_the_cycle_cancels_automatic_countdowns_but_preserves_manual_switches() {
        let monitor = DesktopSwitch::default();
        let desktop = FakeDesktop::new();
        let events = desktop.events.clone();
        monitor.begin(pending(), Some(("first".into(), "work".into())), Duration::from_secs(30), desktop,
            || panic!("opted-out account must not activate"), || Ok(()), || {}).unwrap();
        let mut settings = crate::settings::AppSettings::default();
        for id in ["first", "work"] {
            settings.codex_accounts.insert(id.into(), crate::settings::CodexAccountPreferences { auto_switch: true, ..Default::default() });
        }
        monitor.cancel_unenrolled(&settings);
        assert_eq!(monitor.status().unwrap().phase, "countdown");
        settings.codex_accounts.get_mut("work").unwrap().auto_switch = false;
        monitor.cancel_unenrolled(&settings);
        assert_eq!(monitor.status().unwrap().phase, "cancelled");
        std::thread::sleep(Duration::from_millis(300));
        assert!(events.lock().unwrap().is_empty());

        let manual = DesktopSwitch::default();
        manual.0.lock().unwrap().status = Some(pending());
        manual.cancel_unenrolled(&settings);
        assert_eq!(manual.status().unwrap().phase, "countdown");
    }
    #[test]
    fn restart_now_does_not_wait_for_the_countdown() {
        let monitor = DesktopSwitch::default();
        monitor
            .begin(
                pending(),
                None,
                Duration::from_secs(30),
                FakeDesktop::new(),
                || Ok(()),
                || Ok(()),
                || {},
            )
            .unwrap();
        monitor.run_now().unwrap();
        await_phase(&monitor, "verified");
    }
    #[test]
    fn manual_close_starts_replacement_without_waiting_for_timeout() {
        let desktop = FakeDesktop::new();
        *desktop.pid.lock().unwrap() = None;
        let monitor = DesktopSwitch::default();
        monitor
            .begin(
                pending(),
                None,
                Duration::from_secs(30),
                desktop,
                || Ok(()),
                || Ok(()),
                || {},
            )
            .unwrap();
        await_phase(&monitor, "verified");
    }
    #[test]
    fn unavailable_or_refusing_app_never_changes_credentials() {
        let mut desktop = FakeDesktop::new();
        desktop.available = false;
        assert!(restart_with(&desktop, |_| {}, || panic!("app unavailable"), || Ok(())).is_err());
        assert!(desktop.events.lock().unwrap().is_empty());
        desktop.available = true;
        desktop.accepts_quit = false;
        assert!(restart_with(&desktop, |_| {}, || panic!("app still running"), || Ok(())).is_err());
        assert_eq!(desktop.pid().unwrap(), Some(1));
    }
    #[test]
    fn failed_activation_reopens_the_old_login_and_failed_verification_is_not_success() {
        let desktop = FakeDesktop::new();
        assert!(restart_with(
            &desktop,
            |_| {},
            || Err("write failed".into()),
            || panic!("cannot verify")
        )
        .unwrap_err()
        .contains("previous login"));
        assert_eq!(*desktop.events.lock().unwrap(), ["quit", "open"]);
        assert!(restart_with(
            &FakeDesktop::new(),
            |_| {},
            || Ok(()),
            || Err("login changed".into())
        )
        .is_err());
    }
    #[test]
    fn cancellation_invalidates_the_native_countdown_and_keeps_its_profile() {
        let monitor = DesktopSwitch::default();
        {
            let mut state = monitor.0.lock().unwrap();
            state.generation = 1;
            state.status = Some(SwitchStatus {
                profile_id: "work".into(),
                profile_name: "Work".into(),
                deadline_at: "2030-01-01T00:00:00Z".into(),
                phase: "countdown".into(),
                error: None,
                process_id: None,
                credentials_verified: false,
            running_threads: 0, activity_uncertain: false,
            });
        }
        assert_eq!(monitor.cancel().unwrap().unwrap().phase, "cancelled");
        assert_eq!(monitor.0.lock().unwrap().generation, 2);
        assert!(monitor.run_now().is_err());
        assert_eq!(monitor.status().unwrap().profile_id, "work");
    }
    #[test]
    fn cancellation_cannot_interrupt_an_account_replacement_already_started() {
        let monitor = DesktopSwitch::default();
        {
            let mut state = monitor.0.lock().unwrap();
            state.status = Some(SwitchStatus {
                profile_id: "work".into(),
                profile_name: "Work".into(),
                deadline_at: "2030-01-01T00:00:00Z".into(),
                phase: "activating".into(),
                error: None,
                process_id: None,
                credentials_verified: false,
            running_threads: 0, activity_uncertain: false,
            });
        }
        assert!(monitor.cancel().is_err());
        assert_eq!(monitor.status().unwrap().phase, "activating");
    }
}
