//! Renew managed ChatGPT credentials through Codex itself, without an inference turn.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const RECONNECT: &str = "session could not be renewed — reconnect this account or replace its auth.json in account settings";
const PROTOCOL: &str = "Cannot renew this session. Update Codex CLI and retry.";

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The caller serializes credential access and supplies the actual owning home,
/// so rotated refresh tokens are persisted even if the subsequent usage GET fails.
pub(crate) fn refresh(home: &Path) -> Result<(), String> {
    let binary = crate::cli::which_binary("codex")
        .ok_or("Install Codex CLI to renew this account's session.")?;
    refresh_with(&binary, home, Duration::from_secs(20)).map(|_| ())
}

fn refresh_with(binary: &Path, home: &Path, timeout: Duration) -> Result<Value, String> {
    read_account_with(binary, home, timeout, true, false)
}

fn read_account_with(
    binary: &Path,
    home: &Path,
    timeout: Duration,
    renew: bool,
    verify_plan: bool,
) -> Result<Value, String> {
    let mut worker = Worker(
        Command::new(binary)
            .args([
                "-c",
                "cli_auth_credentials_store=\"file\"",
                "app-server",
                "--listen",
                "stdio://",
            ])
            .env("CODEX_HOME", home)
            .current_dir(home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| PROTOCOL)?,
    );
    let mut input = worker.0.stdin.take().ok_or(PROTOCOL)?;
    let output = worker.0.stdout.take().ok_or(PROTOCOL)?;
    let (send, receive) = mpsc::sync_channel(16);
    // Bound total output and never forward raw RPC/CLI errors or credentials.
    std::thread::spawn(move || {
        for line in BufReader::new(output.take(1024 * 1024)).lines() {
            let Ok(line) = line else { break };
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if send.send(message).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + timeout;
    let write = |input: &mut std::process::ChildStdin, value: Value| -> Result<(), String> {
        writeln!(input, "{value}")
            .and_then(|_| input.flush())
            .map_err(|_| PROTOCOL.into())
    };
    let reply = |id: u64| -> Result<Value, String> {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let message = receive
                .recv_timeout(remaining)
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => {
                        "Session renewal timed out. Retry when connected.".to_string()
                    }
                    mpsc::RecvTimeoutError::Disconnected => PROTOCOL.to_string(),
                })?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                return Ok(message);
            }
        }
    };
    write(
        &mut input,
        json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"keyturn","title":"Keyturn","version":env!("CARGO_PKG_VERSION")}}}),
    )?;
    if reply(1)?.get("error").is_some() {
        return Err(PROTOCOL.into());
    }
    write(&mut input, json!({"method":"initialized"}))?;
    write(
        &mut input,
        json!({"id":2,"method":"account/read","params":{"refreshToken":renew}}),
    )?;
    let response = reply(2)?;
    if response.get("error").is_some() || response["result"]["account"]["type"] != "chatgpt" {
        return Err(RECONNECT.into());
    }
    let mut account = response["result"]["account"].clone();
    if verify_plan {
        // A copied ambient login must not force token rotation. The usage RPC
        // verifies live entitlement and lets Codex renew only when necessary.
        write(
            &mut input,
            json!({"id":3,"method":"account/rateLimits/read","params":{}}),
        )?;
        let limits = reply(3)?;
        let plan = limits["result"]["rateLimits"]["planType"]
            .as_str()
            .filter(|plan| !plan.trim().is_empty())
            .filter(|_| limits.get("error").is_none())
            .ok_or("Couldn’t verify the ChatGPT subscription. Check your connection and retry.")?;
        account["planType"] = json!(plan);
    }
    Ok(account)
}

/// Obtain server-confirmed identity and entitlement without an inference turn.
pub(crate) fn account(home: &Path) -> Result<Value, String> {
    let binary =
        crate::cli::which_binary("codex").ok_or("Install Codex CLI to verify this account.")?;
    account_with(&binary, home, Duration::from_secs(20))
}

fn account_with(binary: &Path, home: &Path, timeout: Duration) -> Result<Value, String> {
    read_account_with(binary, home, timeout, false, true)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(body: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let home = std::env::temp_dir().join(format!("keyturn-renew-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();
        let binary = home.join("codex");
        fs::write(
            &binary,
            format!("#!/bin/sh\nprintf '%s' \"$$\" > \"$CODEX_HOME/pid\"\n{body}\n"),
        )
        .unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        (home, binary)
    }

    #[test]
    fn discovery_reads_existing_session_and_verifies_live_plan_without_forcing_renewal() {
        let (home, binary) = fixture(
            r#"
IFS= read -r init
printf '%s\n' '{"id":1,"result":{}}'
IFS= read -r initialized
IFS= read -r request
case "$request" in
  *'"refreshToken":false'*) printf '%s\n' '{"id":2,"result":{"account":{"type":"chatgpt","email":"fixture@example.com","planType":"free"}}}' ;;
  *) printf '%s\n' '{"id":2,"result":{"account":null}}'; exit 0 ;;
esac
IFS= read -r limits
printf '%s' "$limits" > "$CODEX_HOME/limits-request.json"
printf '%s\n' '{"id":3,"result":{"rateLimits":{"planType":"plus"}}}'
cat >/dev/null
"#,
        );
        let account = account_with(&binary, &home, Duration::from_secs(2)).unwrap();
        assert_eq!(account["planType"], "plus");
        let request: Value =
            serde_json::from_slice(&fs::read(home.join("limits-request.json")).unwrap()).unwrap();
        assert_eq!(request["method"], "account/rateLimits/read");
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn discovery_requires_live_entitlement_and_redacts_rpc_failures() {
        for reply in [
            r#"{"id":3,"error":{"code":-1,"message":"synthetic-secret"}}"#,
            r#"{"id":3,"result":{"rateLimits":{}}}"#,
        ] {
            let (home, binary) = fixture(&format!(
                r#"
IFS= read -r init
printf '%s\n' '{{"id":1,"result":{{}}}}'
IFS= read -r initialized
IFS= read -r request
printf '%s\n' '{{"id":2,"result":{{"account":{{"type":"chatgpt","planType":"plus"}}}}}}'
IFS= read -r limits
printf '%s\n' '{reply}'
cat >/dev/null
"#
            ));
            let error = account_with(&binary, &home, Duration::from_secs(2)).unwrap_err();
            assert_eq!(
                error,
                "Couldn’t verify the ChatGPT subscription. Check your connection and retry."
            );
            assert!(!error.contains("synthetic-secret"));
            fs::remove_dir_all(home).unwrap();
        }
    }

    #[test]
    fn managed_refresh_uses_only_the_owning_home_and_auth_rpc() {
        let (home, binary) = fixture(
            r#"
IFS= read -r init
printf '%s\n' '{"id":1,"result":{}}'
IFS= read -r initialized
IFS= read -r request
printf '%s' "$request" > "$CODEX_HOME/request.json"
printf '%s\n' '{"method":"account/updated","params":{}}' '{"id":2,"result":{"account":{"type":"chatgpt"}}}'
cat >/dev/null
"#,
        );
        refresh_with(&binary, &home, Duration::from_secs(2)).unwrap();
        let request: Value =
            serde_json::from_slice(&fs::read(home.join("request.json")).unwrap()).unwrap();
        assert_eq!(request["method"], "account/read");
        assert_eq!(request["params"]["refreshToken"], true);
        let pid = fs::read_to_string(home.join("pid")).unwrap();
        assert!(!Command::new("/bin/kill")
            .args(["-0", &pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn rejected_refresh_never_exposes_raw_rpc_errors() {
        let (home, binary) = fixture(
            r#"
IFS= read -r init
printf '%s\n' '{"id":1,"result":{}}'
IFS= read -r initialized
IFS= read -r request
printf '%s\n' '{"id":2,"error":{"message":"synthetic-secret","code":-1}}'
"#,
        );
        assert_eq!(
            refresh_with(&binary, &home, Duration::from_secs(2)).unwrap_err(),
            RECONNECT
        );
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn stalled_refresh_has_a_deadline_and_reaps_the_child() {
        let (home, binary) = fixture("cat >/dev/null");
        let started = Instant::now();
        assert!(refresh_with(&binary, &home, Duration::from_secs(2))
            .unwrap_err()
            .contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(5));
        let pid = fs::read_to_string(home.join("pid")).unwrap();
        assert!(!Command::new("/bin/kill")
            .args(["-0", &pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        fs::remove_dir_all(home).unwrap();
    }
}
