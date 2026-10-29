//! Bounded, cancellable CLI-owned sign-in. Only approved login URLs/codes cross IPC.
use serde::Serialize;
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub status: String,
    pub auth_url: Option<String>,
    pub user_code: Option<String>,
    pub error: Option<String>,
}
pub struct Login {
    child: Child,
    output: Arc<Mutex<String>>,
    started: Instant,
    provider: String,
    sent_enter: bool,
}

/// Use a real PTY for Claude, as CodexBar does; Codex's login supports pipes.
pub fn command(binary: &Path, provider: &str, method: &str, home: &Path) -> Command {
    let mut command = if provider == "claude" {
        let mut c = Command::new("/usr/bin/script");
        c.args(["-q", "/dev/null"])
            .arg(binary)
            .args(["auth", "login", "--claudeai"]);
        c.env("CLAUDE_CONFIG_DIR", home)
            .env("CLAUDE_SECURESTORAGE_CONFIG_DIR", home);
        c
    } else {
        let mut c = Command::new(binary);
        c.args(["-c", "cli_auth_credentials_store=\"file\"", "login"]);
        if method == "device" {
            c.arg("--device-auth");
        }
        c.env("CODEX_HOME", home);
        c
    };
    for (key, _) in std::env::vars().filter(|(key, _)| {
        key.starts_with("ANTHROPIC_")
            || key.starts_with("CLAUDE_CODE_USE_")
            || matches!(
                key.as_str(),
                "CLAUDE_CODE_OAUTH_TOKEN"
                    | "CODEX_ACCESS_TOKEN"
                    | "OPENAI_API_KEY"
                    | "CODEX_API_KEY"
            )
    }) {
        command.env_remove(key);
    }
    command
        .current_dir(home)
        .env("NO_COLOR", "1")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}
impl Login {
    pub fn start(binary: &Path, provider: &str, method: &str, home: &Path) -> Result<Self, String> {
        let mut child = command(binary, provider, method, home)
            .spawn()
            .map_err(|_| "Cannot start sign-in. Check the CLI installation.")?;
        let output = Arc::new(Mutex::new(String::new()));
        fn drain(reader: impl Read + Send + 'static, output: Arc<Mutex<String>>) {
            std::thread::spawn(move || {
                let mut reader = reader.take(256 * 1024);
                let mut bytes = [0u8; 2048];
                while let Ok(count) = reader.read(&mut bytes) {
                    if count == 0 {
                        break;
                    }
                    if let Ok(mut text) = output.lock() {
                        if text.len() < 256 * 1024 {
                            text.push_str(&String::from_utf8_lossy(&bytes[..count]));
                        }
                    }
                }
            });
        }
        if let Some(reader) = child.stdout.take() {
            drain(reader, output.clone());
        }
        if let Some(reader) = child.stderr.take() {
            drain(reader, output.clone());
        }
        Ok(Self {
            child,
            output,
            started: Instant::now(),
            provider: provider.into(),
            sent_enter: false,
        })
    }
    pub fn poll(&mut self) -> Progress {
        let text = self.output.lock().map(|s| s.clone()).unwrap_or_default();
        if !self.sent_enter && text.to_lowercase().contains("press enter to open") {
            if let Some(input) = &mut self.child.stdin {
                let _ = input.write_all(b"\r");
                let _ = input.flush();
            }
            self.sent_enter = true;
        }
        let mut progress = parse_instructions(&text, &self.provider);
        match self.child.try_wait() {
            Ok(Some(exit)) => {
                progress.status = if exit.success() {
                    "completed"
                } else {
                    "failed"
                }
                .into();
                if !exit.success() {
                    progress.error =
                        Some("Sign-in did not complete. Retry or choose another method.".into());
                }
            }
            Ok(None) if self.started.elapsed() < Duration::from_secs(600) => {}
            _ => {
                self.stop();
                progress.status = "failed".into();
                progress.error = Some("Sign-in timed out. Try again when ready.".into());
            }
        }
        progress
    }
    pub fn submit_code(&mut self, code: &str) -> Result<(), String> {
        if code.is_empty() || code.len() > 2048 || code.chars().any(char::is_control) {
            return Err("Enter the authorization code shown by the provider.".into());
        }
        let input = self
            .child
            .stdin
            .as_mut()
            .ok_or("This sign-in no longer accepts a code.")?;
        input
            .write_all(format!("{code}\r").as_bytes())
            .and_then(|_| input.flush())
            .map_err(|_| "Cannot send the code to the CLI.".into())
    }
    pub fn stop(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGTERM);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for Login {
    fn drop(&mut self) {
        self.stop();
    }
}

fn parse_instructions(text: &str, provider: &str) -> Progress {
    let mut progress = Progress {
        status: "pending".into(),
        ..Default::default()
    };
    // Never relay a CLI transcript, stderr, refresh token or arbitrary URL to the webview.
    for token in text.split(|c: char| c.is_whitespace() || c == '\u{1b}' || c == '"') {
        let candidate = token.trim_matches(|c: char| matches!(c, ')' | ',' | '\'' | '>'));
        if let Ok(url) = reqwest::Url::parse(candidate) {
            let allowed = if provider == "codex" {
                matches!(url.host_str(), Some("auth.openai.com" | "auth0.openai.com"))
            } else {
                matches!(
                    url.host_str(),
                    Some("claude.ai" | "platform.claude.com" | "console.anthropic.com")
                )
            };
            if allowed && url.scheme() == "https" {
                progress.auth_url = Some(url.to_string());
            }
        }
        let code = candidate.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
        if provider == "codex"
            && (code.len() == 9 || code.len() == 11)
            && code.contains('-')
            && code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
        {
            progress.user_code = Some(code.into());
        }
    }
    progress
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_provider_login_instructions_are_exposed() {
        let progress = parse_instructions("secret-token https://evil.example/signin https://auth.openai.com/codex/device ABCD-EFGH", "codex");
        let json = serde_json::to_string(&progress).unwrap();
        assert!(!json.contains("secret-token") && !json.contains("evil.example"));
        assert_eq!(progress.user_code.as_deref(), Some("ABCD-EFGH"));
        assert!(
            parse_instructions("https://auth.openai.com/phish", "claude")
                .auth_url
                .is_none()
        );
    }
    #[test]
    fn claude_uses_real_pty_and_isolated_storage() {
        let cmd = command(
            Path::new("/bin/claude"),
            "claude",
            "browser",
            Path::new("/tmp/draft"),
        );
        assert_eq!(cmd.get_program(), "/usr/bin/script");
        assert!(cmd.get_args().any(|arg| arg == "--claudeai"));
        assert!(cmd.get_envs().any(|(key, value)| key == "CLAUDE_CONFIG_DIR"
            && value == Some(std::ffi::OsStr::new("/tmp/draft"))));
    }
    #[cfg(unix)]
    #[test]
    fn cancelled_cli_is_reaped_and_untrusted_output_never_enters_progress() {
        use std::os::unix::fs::PermissionsExt;
        let home = std::env::temp_dir().join(format!("keyturn-runner-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&home).unwrap();
        let binary = home.join("fake-cli");
        std::fs::write(&binary,"#!/bin/sh\nprintf '%s\\n' 'secret-refresh-token' 'https://auth.openai.com/codex/device' 'ABCD-EFGH'\nexec sleep 60\n").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut login = Login::start(&binary, "codex", "device", &home).unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        while login.poll().user_code.is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        let progress = login.poll();
        assert_eq!(progress.user_code.as_deref(), Some("ABCD-EFGH"));
        assert!(!serde_json::to_string(&progress)
            .unwrap()
            .contains("secret-refresh-token"));
        login.stop();
        assert!(login.child.try_wait().unwrap().is_some());
        drop(login);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn claude_pty_accepts_enter_and_forwards_authorization_code() {
        use std::os::unix::fs::PermissionsExt;
        let home = std::env::temp_dir().join(format!("keyturn-pty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&home).unwrap();
        let binary = home.join("fake-claude");
        std::fs::write(&binary,"#!/bin/sh\n[ -t 0 ] || exit 7\nprintf '%s\\n' 'Press Enter to open sign-in'\nread -r enter\nprintf '%s\\n' 'https://claude.ai/oauth/authorize?state=fake'\nread -r code\n[ \"$code\" = 'synthetic-code' ] || exit 8\nprintf verified > \"$CLAUDE_CONFIG_DIR/received\"\n").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut login = Login::start(&binary, "claude", "browser", &home).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while login.poll().auth_url.is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(login.poll().auth_url.is_some());
        login.submit_code("synthetic-code").unwrap();
        while login.poll().status == "pending" && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(login.poll().status, "completed");
        assert_eq!(
            std::fs::read_to_string(home.join("received")).unwrap(),
            "verified"
        );
        drop(login);
        std::fs::remove_dir_all(home).unwrap();
    }
}
