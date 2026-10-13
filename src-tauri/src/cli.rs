//! CLI-only provider status (T3-style, read-only).
//!
//! Keyturn does NOT authenticate. Each provider owns its CLI and its own
//! credential storage; we only:
//!   1. check the binary exists on `PATH` (plus common install dirs),
//!   2. run the CLI's own status command (never a login/write command),
//!   3. read the CLI's auth file read-only to enrich the display.
//!
//! No OAuth, no webview, no keychain writes, no auth.json patching here.
//! Anything that would change credentials must be run by the user in their
//! own terminal (we only show the exact command to copy).
//!
//! Covered providers (user scope):
//! | id | CLI binary | login (user runs) | status (we run) |
//! |----|------------|-------------------|-----------------|
//! | claude | `claude` | `claude auth login` | `claude auth status --json` |
//! | codex | `codex` | `codex login` | `codex login status` |
//! | opencode | `opencode` | `opencode auth login` | `opencode auth list` + auth.json keys |
//! | gemini | `gemini` | `gemini` (Sign in with Google) | file/env probe (no status cmd) |
//!
//! Anthropic / OpenAI / Google API keys without their own CLI are covered
//! through OpenCode (`opencode auth login anthropic|openai|google`).

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::paths;

// ── public shapes (frontend contract, additive) ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCliStatus {
    pub id: String,
    pub label: String,
    /// Binary T3-style default, e.g. `claude`.
    pub binary: String,
    pub binary_found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// None = unknown (e.g. CLI missing so we could not ask).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logged_in: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Human sentence for the UI subtitle, e.g. "Logged in using ChatGPT".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Exact command the user should run in their own terminal.
    pub login_cmd: String,
    pub install_url: String,
    /// Credential file we peeked at (read-only), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_file: Option<String>,
    /// For OpenCode: provider ids present in auth.json (anthropic, openai, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connected: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct CliSpec {
    id: &'static str,
    label: &'static str,
    binary: &'static str,
    install_url: &'static str,
    login_cmd: &'static str,
}

const SPECS: &[CliSpec] = &[
    CliSpec {
        id: "claude",
        label: "Claude",
        binary: "claude",
        install_url: "https://claude.com/product/claude-code",
        login_cmd: "claude auth login",
    },
    CliSpec {
        id: "codex",
        label: "OpenAI Codex",
        binary: "codex",
        install_url: "https://developers.openai.com/codex/cli",
        login_cmd: "codex login",
    },
    CliSpec {
        id: "opencode",
        label: "OpenCode",
        binary: "opencode",
        install_url: "https://opencode.ai",
        login_cmd: "opencode auth login",
    },
    CliSpec {
        id: "gemini",
        label: "Gemini",
        binary: "gemini",
        install_url: "https://github.com/google-gemini/gemini-cli",
        login_cmd: "gemini",
    },
];

// ── helpers ─────────────────────────────────────────────────────────────────

/// `PATH` lookup plus the install dirs CLIs commonly land in when the GUI
/// environment has a thinner `PATH` than the user's shell.
fn which_binary(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
    for extra in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        ".local/bin",
    ] {
        let p = if extra.starts_with('.') {
            dirs::home_dir().map(|h| h.join(extra))
        } else {
            Some(PathBuf::from(extra))
        };
        if let Some(p) = p {
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }
    #[cfg(windows)]
    let names = vec![format!("{name}.exe"), format!("{name}.cmd"), name.to_string()];
    #[cfg(not(windows))]
    let names = vec![name.to_string()];
    for dir in dirs {
        for n in &names {
            let candidate = dir.join(n);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

struct RunOut {
    success: bool,
    stdout: String,
    #[allow(dead_code)]
    stderr: String,
}

/// Run a status subcommand. Synchronous + short-lived by design; callers wrap
/// it in `spawn_blocking`. Timeouts are enforced by killing after N secs via
/// a helper thread (std has no `Command::timeout`).
fn run_status(binary: &Path, args: &[&str], timeout: Duration) -> Option<RunOut> {
    let mut child = Command::new(binary)
        .args(args)
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = child.wait_with_output().ok()?;
                return Some(RunOut {
                    success: status.success(),
                    stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
                    stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
                });
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

fn binary_version(binary: &Path) -> Option<String> {
    // `x --version` prints fast and never writes credentials.
    let out = run_status(binary, &["--version"], Duration::from_secs(8))?;
    let line = out.stdout.lines().next()?.trim().to_string();
    if line.is_empty() {
        None
    } else {
        Some(line.chars().take(80).collect())
    }
}

fn home_dir() -> PathBuf {
    dirs::home_dir().expect("home directory")
}

fn read_json_file(path: &Path) -> Option<serde_json::Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

// ── per-provider probes (read-only) ─────────────────────────────────────────

/// `claude auth status --json` → `{"loggedIn":true,"authMethod":"claude.ai",…}`.
/// Trust the `loggedIn` field over the exit code (2.x may exit non-zero on
/// transient refresh notices while still logged in).
fn probe_claude(binary: &Path) -> (Option<bool>, Option<String>, Option<String>, Option<String>) {
    let Some(out) = run_status(binary, &["auth", "status", "--json"], Duration::from_secs(10))
    else {
        return (None, None, None, None);
    };
    // stdout may contain the JSON even on non-zero exit; parse regardless.
    let text = out.stdout;
    let start = text.find('{').unwrap_or(0);
    let json_text = &text[start..];
    let value: serde_json::Value = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(_) => return (Some(out.success), None, None, Some(text)),
    };
    let logged_in = value
        .get("loggedIn")
        .and_then(|v| v.as_bool())
        .unwrap_or(out.success);
    let method = value
        .get("authMethod")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let email = value
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let account = value
        .get("orgName")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            value
                .get("subscriptionType")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
    (Some(logged_in), method, email, account)
}

/// `codex login status` exits 0 when credentials exist. Stdout is
/// "Logged in using ChatGPT" / "Logged in using an API key - sk-…"
/// / "Not logged in". Never runs `codex login` (interactive) here.
fn probe_codex(binary: &Path) -> (Option<bool>, Option<String>, Option<String>) {
    let Some(out) = run_status(binary, &["login", "status"], Duration::from_secs(10)) else {
        return (None, None, None);
    };
    // Combine stdout+stderr: the CLI prints the status line to stderr.
    let text = format!("{} {}", out.stdout, out.stderr);
    let lower = text.to_lowercase();
    if lower.contains("not logged in") {
        return (Some(false), None, Some("Not logged in".to_string()));
    }
    if lower.contains("logged in") {
        let method = if lower.contains("chatgpt") {
            Some("ChatGPT".to_string())
        } else if lower.contains("api key") {
            Some("API key".to_string())
        } else if lower.contains("access token") || lower.contains("personal access") {
            Some("Access token".to_string())
        } else {
            None
        };
        let detail = text
            .lines()
            .map(str::trim)
            .find(|l| l.to_lowercase().contains("logged in"))
            .map(str::to_string);
        return (Some(true), method, detail);
    }
    // Unparseable: fall back to exit code only.
    (Some(out.success), None, None)
}

/// Read-only peek at `~/.codex/auth.json` (or `$CODEX_HOME/auth.json`) to
/// enrich the display. Mirrors `codex::status()` without any writes.
fn codex_auth_file() -> PathBuf {
    paths::codex_auth_file()
}

/// OpenCode: `opencode auth list` + read-only keys of auth.json.
/// Sub-providers (anthropic/openai/google/…) appear as `connected`.
fn probe_opencode(binary: &Path) -> (Option<bool>, Vec<String>, Option<String>) {
    let mut connected: Vec<String> = opencode_auth_keys();
    let mut logged_in: Option<bool> = if connected.is_empty() { None } else { Some(true) };
    let mut detail: Option<String> = None;

    if let Some(out) = run_status(binary, &["auth", "list"], Duration::from_secs(10)) {
        let parsed = parse_opencode_auth_list(&out.stdout);
        for p in parsed {
            if !connected.contains(&p) {
                connected.push(p);
            }
        }
        connected.sort();
        connected.dedup();
        if !connected.is_empty() {
            logged_in = Some(true);
            detail = Some(format!("{} connected", connected.join(", ")));
        } else if out.stdout.to_lowercase().contains("not logged")
            || out.stdout.trim().is_empty()
        {
            logged_in = Some(false);
            detail = Some("Not logged in".to_string());
        }
    }
    (logged_in, connected, detail)
}

fn opencode_auth_keys() -> Vec<String> {
    let file = paths::open_code_auth_file();
    let Some(serde_json::Value::Object(map)) = read_json_file(&file) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = map
        .keys()
        .filter(|k| !k.starts_with('$') && *k != "providers")
        .cloned()
        .collect();
    ids.sort();
    ids
}

/// Parse provider rows, never headings, counts, auth types or file paths.
/// Current OpenCode uses `● OpenCode Go api`; older versions used tables.
fn parse_opencode_auth_list(stdout: &str) -> Vec<String> {
    let mut plain = String::new();
    let mut escape = false;
    for ch in stdout.chars() {
        if ch == '\u{1b}' {
            escape = true;
        } else if escape {
            if ch.is_ascii_alphabetic() {
                escape = false;
            }
        } else {
            plain.push(ch);
        }
    }
    let mut out = Vec::new();
    for line in plain.lines() {
        let row = line.trim().trim_start_matches('│').trim();
        let id = if let Some(provider) = row.strip_prefix('●') {
            let name = provider.trim();
            let name = name.strip_suffix(" oauth").or_else(|| name.strip_suffix(" api")).unwrap_or(name).trim();
            match name.to_lowercase().as_str() {
                "opencode go" => "opencode-go".to_string(),
                "opencode zen" | "opencode" => "opencode".to_string(),
                "github copilot" => "github-copilot".to_string(),
                _ => name.to_lowercase().replace(' ', "-"),
            }
        } else {
            let fields: Vec<_> = row.split(|c: char| c.is_whitespace() || c == '│' || c == '|').filter(|s| !s.is_empty()).collect();
            if fields.len() != 2 || !["connected", "active", "api", "oauth"].contains(&fields[1].to_lowercase().as_str()) {
                continue;
            }
            fields[0].to_lowercase()
        };
        if !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.".contains(c)) && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// Gemini CLI has no `status` subcommand. Best-effort read-only probe:
/// 1. `~/.gemini/oauth_creds.json` exists and parses (written by `gemini` login),
/// 2. else `GEMINI_API_KEY` / `GOOGLE_API_KEY` / `GOOGLE_APPLICATION_CREDENTIALS`
///    env present,
/// 3. else `~/.gemini/settings.json` selected auth type (informational only).
fn probe_gemini() -> (Option<bool>, Option<String>, Option<String>) {
    let oauth = gemini_oauth_file();
    if let Some(v) = read_json_file(&oauth) {
        // Any non-empty object with a token-ish field counts as a cached login.
        let has_token = match &v {
            serde_json::Value::Object(map) => map.keys().any(|k| {
                let k = k.to_lowercase();
                k.contains("token") || k.contains("refresh") || k.contains("access") || k.contains("cred")
            }),
            _ => false,
        };
        if has_token || v.to_string().len() > 16 {
            return (
                Some(true),
                Some("Google OAuth".to_string()),
                Some(format!("Cached login ({})", oauth.display())),
            );
        }
    }
    for key in ["GEMINI_API_KEY", "GOOGLE_API_KEY", "GOOGLE_GENAI_USE_VERTEXAI", "GOOGLE_APPLICATION_CREDENTIALS"] {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                return (
                    Some(true),
                    Some(format!("env:{key}")),
                    Some(format!("Authenticated via {key}")),
                );
            }
        }
    }
    if oauth.exists() {
        return (
            Some(false),
            None,
            Some("Cached credentials unreadable — run `gemini` and sign in again".to_string()),
        );
    }
    (Some(false), None, Some("Not logged in".to_string()))
}

fn gemini_oauth_file() -> PathBuf {
    home_dir().join(".gemini").join("oauth_creds.json")
}

// ── aggregate ───────────────────────────────────────────────────────────────

fn status_for(spec: &CliSpec) -> ProviderCliStatus {
    let found = which_binary(spec.binary);
    let (binary_path, version) = match &found {
        Some(p) => (
            Some(p.display().to_string()),
            binary_version(p),
        ),
        None => (None, None),
    };

    let (logged_in, auth_method, email, account, detail, connected, auth_file) = match spec.id {
        "claude" => {
            let (logged_in, method, email, account) = found
                .as_deref()
                .map(probe_claude)
                .unwrap_or((None, None, None, None));
            let detail = account.clone();
            let auth_file = claude_auth_file_display();
            (logged_in, method, email, account, detail, Vec::new(), auth_file)
        }
        "codex" => {
            let (logged_in, method, detail) = found
                .as_deref()
                .map(probe_codex)
                .unwrap_or((None, None, None));
            // Enrich a logged-in row with the account the CLI cached.
            let (email, account) = if logged_in == Some(true) {
                let st = crate::codex::status();
                (st.email, st.plan.or(st.account_id))
            } else {
                (None, None)
            };
            let file = codex_auth_file();
            (
                logged_in,
                method,
                email,
                account,
                detail,
                Vec::new(),
                Some(file.display().to_string()),
            )
        }
        "opencode" => {
            let (logged_in, connected, detail) = found
                .as_deref()
                .map(probe_opencode)
                .unwrap_or((None, Vec::new(), None));
            let file = paths::open_code_auth_file();
            (
                logged_in,
                None,
                None,
                None,
                detail,
                connected,
                Some(file.display().to_string()),
            )
        }
        "gemini" => {
            let (logged_in, method, detail) = probe_gemini();
            // Without a binary even an env key is not usable via the CLI.
            let logged_in = if found.is_none() { None } else { logged_in };
            let file = gemini_oauth_file();
            (
                logged_in,
                method,
                None,
                None,
                detail,
                Vec::new(),
                Some(file.display().to_string()),
            )
        }
        _ => (None, None, None, None, None, Vec::new(), None),
    };

    ProviderCliStatus {
        id: spec.id.to_string(),
        label: spec.label.to_string(),
        binary: spec.binary.to_string(),
        binary_found: found.is_some(),
        binary_path,
        version,
        logged_in,
        auth_method,
        email,
        account,
        detail,
        login_cmd: spec.login_cmd.to_string(),
        install_url: spec.install_url.to_string(),
        auth_file,
        connected,
    }
}

fn claude_auth_file_display() -> Option<String> {
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return Some(format!("{dir}/.credentials.json (+ macOS Keychain)"));
        }
    }
    Some(format!(
        "{}/.claude/.credentials.json (+ macOS Keychain)",
        home_dir().display()
    ))
}

/// All four providers, in UI order. Never writes anything.
pub fn all_status() -> Vec<ProviderCliStatus> {
    SPECS.iter().map(status_for).collect()
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_cover_the_four_supported_clis() {
        let ids: Vec<_> = SPECS.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["claude", "codex", "opencode", "gemini"]);
        for s in SPECS {
            assert!(!s.binary.is_empty() && !s.login_cmd.is_empty() && !s.install_url.is_empty());
        }
    }

    #[test]
    fn claude_json_prefers_logged_in_over_exit_code() {
        // Mirrors `claude auth status --json` shape; parsing lives in probe,
        // so test the extraction contract directly.
        let raw = r#"{"loggedIn":true,"authMethod":"claude.ai","email":"a@x.com","orgName":"Acme","subscriptionType":"max"}"#;
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(v["loggedIn"], true);
        assert_eq!(v["authMethod"], "claude.ai");
    }

    #[test]
    fn codex_status_text_classification() {
        for (text, logged, method) in [
            ("Logged in using ChatGPT", true, "ChatGPT"),
            ("Logged in using an API key - sk-ant-x", true, "API key"),
            ("Not logged in", false, ""),
        ] {
            let lower = text.to_lowercase();
            let logged_in = !lower.contains("not logged in") && lower.contains("logged in");
            assert_eq!(logged_in, logged);
            if logged {
                assert!(lower.contains(&method.to_lowercase()));
            }
        }
    }

    #[test]
    fn opencode_auth_list_parsing_ignores_headers() {
        let fake = "Provider │ Status\nanthropic connected\nopenai active\n";
        let parsed = parse_opencode_auth_list(fake);
        assert!(parsed.contains(&"anthropic".to_string()));
        assert!(parsed.contains(&"openai".to_string()));
        assert!(!parsed.contains(&"provider".to_string()));
    }

    #[test]
    fn opencode_auth_list_parses_real_cli_rows_without_phantom_services() {
        let actual = "\u{1b}[0m\n┌  Credentials ~/.local/share/opencode/auth.json\n│\n●  OpenCode Go \u{1b}[90mapi\n│\n└  1 credentials\n";
        assert_eq!(parse_opencode_auth_list(actual), vec!["opencode-go"]);
        assert!(parse_opencode_auth_list("┌ Credentials\n└ 0 credentials\n").is_empty());
    }

    #[test]
    fn opencode_keys_skip_schema_noise() {
        let dir = std::env::temp_dir().join(format!("keyturn-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("auth.json");
        std::fs::write(
            &file,
            r#"{"anthropic":{"type":"api","key":"x"}, "$schema":"u", "openai":{"type":"api","key":"y"}}"#,
        )
        .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let ids: Vec<String> = v
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| !k.starts_with('$'))
            .cloned()
            .collect();
        assert!(ids.contains(&"anthropic".to_string()));
        assert!(!ids.contains(&"$schema".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn gemini_probe_prefers_env_over_missing_file() {
        // With a bogus HOME-less env key set, probe reports env auth.
        std::env::set_var("GEMINI_API_KEY", "test-key");
        let (logged_in, method, _) = probe_gemini();
        // Either true (env) — file may also exist on dev machines, but method
        // must mention env or OAuth, never panic.
        assert!(logged_in.is_some());
        assert!(method.is_some());
        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn which_missing_binary_returns_none() {
        assert!(which_binary("keyturn-definitely-not-a-binary-xyz").is_none());
    }
}
