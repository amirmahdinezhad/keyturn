use std::path::Path;
pub fn claude(home: &Path) {
    if let Some(binary) = crate::cli::which_binary("claude") {
        let mut command = crate::account_login::command(&binary, "claude", "browser", home);
        // Reuse the sanitized environment, then replace the PTY login arguments.
        let env: Vec<_> = command
            .get_envs()
            .map(|(k, v)| (k.to_os_string(), v.map(|v| v.to_os_string())))
            .collect();
        command = std::process::Command::new(&binary);
        command
            .args(["auth", "logout"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        for (key, value) in env {
            if let Some(value) = value {
                command.env(key, value);
            } else {
                command.env_remove(key);
            }
        }
        if let Ok(mut child) = command.spawn() {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while matches!(child.try_wait(), Ok(None)) && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
