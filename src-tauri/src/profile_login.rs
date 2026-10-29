//! CLI-owned browser login in an isolated profile; no secrets or CLI output in IPC.
use serde::Serialize;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::{cli, profiles};

static SESSION: LazyLock<Mutex<Option<Session>>> = LazyLock::new(|| Mutex::new(None));
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus {
    pub profile_id: String,
    pub status: &'static str,
    pub error: Option<String>,
}

struct Session {
    id: String,
    child: Child,
    started: Instant,
    result: Option<LoginStatus>,
}

impl Session {
    fn spawn(id: &str, binary: &Path, home: &Path) -> Result<Self, String> {
        let child = Command::new(binary)
            .args(["-c", "cli_auth_credentials_store=\"file\"", "login"])
            .env("CODEX_HOME", home)
            .current_dir(home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Cannot start Codex sign-in. Check the CLI installation.")?;
        Ok(Self {
            id: id.into(),
            child,
            started: Instant::now(),
            result: None,
        })
    }

    fn poll(&mut self) -> LoginStatus {
        if let Some(result) = &self.result {
            return result.clone();
        }
        let result = match self.child.try_wait() {
            Ok(Some(exit)) if exit.success() => LoginStatus {
                profile_id: self.id.clone(),
                status: "completed",
                error: None,
            },
            Ok(Some(_)) => LoginStatus {
                profile_id: self.id.clone(),
                status: "failed",
                error: Some(
                    "Codex sign-in did not finish. Try again or use the Terminal instructions."
                        .into(),
                ),
            },
            Ok(None) if self.started.elapsed() < LOGIN_TIMEOUT => {
                return LoginStatus {
                    profile_id: self.id.clone(),
                    status: "pending",
                    error: None,
                }
            }
            Ok(None) => {
                self.stop();
                LoginStatus {
                    profile_id: self.id.clone(),
                    status: "failed",
                    error: Some("Sign-in timed out. Start again when you are ready.".into()),
                }
            }
            Err(_) => {
                self.stop();
                LoginStatus {
                    profile_id: self.id.clone(),
                    status: "failed",
                    error: Some("Cannot check Codex sign-in. Try again.".into()),
                }
            }
        };
        self.result = Some(result.clone());
        result
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.result.is_none() {
            self.stop();
        }
    }
}

/// Prevent replacing or deleting credentials while their CLI login is writing.
pub fn ensure_not_running(id: &str) -> Result<(), String> {
    let mut session = SESSION
        .lock()
        .map_err(|_| "Sign-in is busy. Restart Keyturn.")?;
    if session
        .as_mut()
        .is_some_and(|s| s.id == id && s.poll().status == "pending")
    {
        return Err("Finish or cancel this profile's sign-in first.".into());
    }
    Ok(())
}

/// The CLI opens the system browser; it writes only this profile's auth cache.
pub fn start(id: &str) -> Result<LoginStatus, String> {
    let mut session = SESSION
        .lock()
        .map_err(|_| "Sign-in is busy. Restart Keyturn.")?;
    if session
        .as_mut()
        .is_some_and(|s| s.poll().status == "pending")
    {
        return Err("A sign-in is already open. Finish or cancel it first.".into());
    }
    let home = profiles::login_home(id)?;
    let binary = cli::which_binary("codex").ok_or("Install Codex CLI before signing in. Use Add account → OpenAI Codex for the installation guide.")?;
    let mut started = Session::spawn(id, &binary, &home)?;
    let status = started.poll();
    *session = Some(started);
    Ok(status)
}

/// Poll process completion only; profile metadata separately validates its saved login.
pub fn status(id: &str) -> Result<LoginStatus, String> {
    let mut session = SESSION
        .lock()
        .map_err(|_| "Sign-in is busy. Restart Keyturn.")?;
    Ok(match session.as_mut().filter(|s| s.id == id) {
        Some(session) => session.poll(),
        None => LoginStatus {
            profile_id: id.into(),
            status: "idle",
            error: None,
        },
    })
}

/// Cancel the child process without logging out or changing the active account.
pub fn cancel(id: &str) -> Result<(), String> {
    let mut session = SESSION
        .lock()
        .map_err(|_| "Sign-in is busy. Restart Keyturn.")?;
    if session.as_ref().is_some_and(|s| s.id == id) {
        if let Some(mut child) = session.take() {
            child.stop();
        }
    }
    Ok(())
}

/// Clean up the callback server when Keyturn exits.
pub fn shutdown() {
    if let Ok(mut session) = SESSION.lock() {
        if let Some(mut child) = session.take() {
            child.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    fn fake_cli(body: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("keyturn-login-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("codex");
        std::fs::write(&binary, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        (dir, binary)
    }
    fn await_exit(session: &mut Session) -> LoginStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let status = session.poll();
            if status.status != "pending" || Instant::now() >= deadline { return status; }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[cfg(unix)]
    #[test]
    fn browser_login_uses_the_isolated_home_and_file_storage_override() {
        let (dir, binary) = fake_cli("printf '%s\\n' \"$CODEX_HOME\" \"$@\" > launch.txt\nexit 0");
        let mut session = Session::spawn("work", &binary, &dir).unwrap();
        assert_eq!(await_exit(&mut session).status, "completed");
        let launch = std::fs::read_to_string(dir.join("launch.txt")).unwrap();
        assert_eq!(
            launch.lines().collect::<Vec<_>>(),
            vec![
                dir.to_str().unwrap(),
                "-c",
                "cli_auth_credentials_store=\"file\"",
                "login"
            ]
        );
        drop(session);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn failed_cli_output_is_not_exposed_and_a_pending_login_can_be_killed() {
        let (dir, binary) = fake_cli("echo 'secret-access-token' >&2\nexit 1");
        let mut session = Session::spawn("work", &binary, &dir).unwrap();
        let status = await_exit(&mut session);
        assert_eq!(status.status, "failed");
        assert!(!status.error.unwrap().contains("secret-access-token"));
        std::fs::write(&binary, "#!/bin/sh\nexec sleep 60\n").unwrap();
        let mut pending = Session::spawn("work", &binary, &dir).unwrap();
        assert_eq!(pending.poll().status, "pending");
        pending.stop();
        assert!(pending.child.try_wait().unwrap().is_some());
        drop(pending);
        drop(session);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
