//! Optional "connect your OpenCode account" support.
//!
//! Two layers:
//! 1. OAuth 2.0 Authorization Code + PKCE (loopback redirect, real browser) —
//!    tokens in the OS keychain. Kept for providers that expose an API.
//! 2. An in-app sign-in window whose website session cookies we read through
//!    Tauri's cookie API. The OpenCode console authenticates its server
//!    functions with that session, not with a Bearer token.
//!
//! Nothing here runs unless the user connects an account.

use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

pub const AUTH_ISSUER: &str = "https://auth.opencode.ai";
const CLIENT_ID: &str = "app";
const KEYCHAIN_SERVICE: &str = "com.7skylab.keyturn";
const KEYCHAIN_ACCESS: &str = "opencode-access-token";
const KEYCHAIN_REFRESH: &str = "opencode-refresh-token";
const KEYCHAIN_SESSION: &str = "opencode-session-cookie";

const CONSOLE_SERVER_URL: &str = "https://opencode.ai/_server";
/// Server-function ids scraped from the site bundle (they change per deploy).
pub const FN_WORKSPACES: &str = "def39973159c7f0483d8793a822b8dbb10d067e12c65455fcb4608459ba0234f";
pub const FN_USAGE_LIST: &str = "bfd684bfc2e4eed05cd0b518f5e4eafd3f3376e3938abb9e536e7c03df831e5c";
pub const FN_KEYS: &str = "def2ab20a296ef06465b1c3cf86da4ea983c0696e7a5708b9468aaed85083d6b";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleKey {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, alias = "masked", alias = "partial")]
    pub prefix: Option<String>,
    #[serde(
        default,
        alias = "value",
        alias = "key",
        alias = "token",
        alias = "secretKey"
    )]
    pub secret: Option<String>,
    #[serde(default, alias = "userEmail", alias = "user_email", alias = "owner")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleRow {
    pub model: String,
    #[serde(default, alias = "keyID", alias = "key_id", alias = "key")]
    pub key_id: Option<String>,
    #[serde(default, alias = "day", alias = "date")]
    pub date: Option<String>,
    #[serde(default, alias = "amount", alias = "total", alias = "usage")]
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsoleUsage {
    #[serde(default)]
    pub keys: Vec<ConsoleKey>,
    #[serde(default, alias = "rows")]
    pub usage: Vec<ConsoleRow>,
    /// Exact Go-panel windows (label, percent, reset text) as displayed.
    #[serde(default)]
    pub go: Vec<GoWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoWindow {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub percent: f64,
    #[serde(default)]
    pub reset: String,
}

// ── OAuth (PKCE + loopback) ────────────────────────────────────────────────

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn random_b64url(len: usize) -> String {
    let mut buf = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buf);
    b64url(&buf)
}

fn pkce_pair() -> (String, String) {
    let verifier = random_b64url(32);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn urlencode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

pub fn authorize_url(redirect_uri: &str, challenge: &str, state: &str) -> String {
    format!(
        "{AUTH_ISSUER}/authorize?client_id={CLIENT_ID}&response_type=code&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
        urlencode(redirect_uri),
        urlencode(state),
        urlencode(challenge),
    )
}

fn wait_for_code(
    listener: &TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        if started.elapsed() > timeout {
            return Err("Login timed out — no response from the browser".to_string());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 4096];
                let read = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..read]);
                let path = request.split_whitespace().nth(1).unwrap_or("").to_string();
                let body = "<!doctype html><meta charset=utf-8><title>Keyturn</title><body style=\"font:14px -apple-system;padding:40px;text-align:center\">You can close this tab and return to Keyturn.</body>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();

                let Some(query) = path.split('?').nth(1) else {
                    continue;
                };
                let params: std::collections::HashMap<_, _> =
                    url::form_urlencoded::parse(query.as_bytes()).collect();
                if let Some(error) = params.get("error") {
                    return Err(format!("Login was rejected ({error})"));
                }
                let (Some(code), Some(state)) = (params.get("code"), params.get("state")) else {
                    continue;
                };
                if state != expected_state {
                    return Err("Login state mismatch — try again".to_string());
                }
                return Ok(code.to_string());
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn exchange_code(code: &str, verifier: &str, redirect_uri: &str) -> Result<AccountTokens, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let body = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier),
    ]
    .iter()
    .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
    .collect::<Vec<_>>()
    .join("&");
    let response = client
        .post(format!("{AUTH_ISSUER}/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Token exchange failed ({status}): {text}"));
    }
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<i64>,
    }
    let parsed: TokenResponse =
        serde_json::from_str(&text).map_err(|e| format!("Unexpected token response: {e}"))?;
    Ok(AccountTokens {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        expires_at: parsed
            .expires_in
            .map(|s| chrono::Utc::now().timestamp() + s),
    })
}

/// Interactive OAuth login via the user's own browser.
pub fn login() -> Result<AccountTokens, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let (verifier, challenge) = pkce_pair();
    let state = random_b64url(16);
    let url = authorize_url(&redirect_uri, &challenge, &state);
    tauri_plugin_opener::open_url(url, None::<String>).map_err(|e| e.to_string())?;
    let code = wait_for_code(&listener, &state, Duration::from_secs(180))?;
    let tokens = exchange_code(&code, &verifier, &redirect_uri)?;
    save_tokens(&tokens)?;
    Ok(tokens)
}

// ── keychain ───────────────────────────────────────────────────────────────

fn store(name: &str, value: &str) -> Result<(), String> {
    let entry = keyring_crate::Entry::new(KEYCHAIN_SERVICE, name).map_err(|e| e.to_string())?;
    entry.set_password(value).map_err(|e| e.to_string())
}

fn load(name: &str) -> Option<String> {
    keyring_crate::Entry::new(KEYCHAIN_SERVICE, name)
        .ok()?
        .get_password()
        .ok()
}

fn remove(name: &str) {
    if let Ok(entry) = keyring_crate::Entry::new(KEYCHAIN_SERVICE, name) {
        let _ = entry.delete_credential();
    }
}

pub fn save_tokens(tokens: &AccountTokens) -> Result<(), String> {
    store(KEYCHAIN_ACCESS, &tokens.access_token)?;
    match &tokens.refresh_token {
        Some(refresh) => store(KEYCHAIN_REFRESH, refresh),
        None => Ok(()),
    }
}

pub fn stored_tokens() -> Option<AccountTokens> {
    Some(AccountTokens {
        access_token: load(KEYCHAIN_ACCESS)?,
        refresh_token: load(KEYCHAIN_REFRESH),
        expires_at: None,
    })
}

pub fn refresh_tokens() -> Result<AccountTokens, String> {
    let current = stored_tokens().ok_or("not connected")?;
    let refresh = current
        .refresh_token
        .clone()
        .ok_or("no refresh token stored — sign in again")?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let body = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh.as_str()),
        ("client_id", CLIENT_ID),
    ]
    .iter()
    .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
    .collect::<Vec<_>>()
    .join("&");
    let response = client
        .post(format!("{AUTH_ISSUER}/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let text = response.text().unwrap_or_default();
        return Err(format!(
            "refresh failed: {}",
            text.chars().take(120).collect::<String>()
        ));
    }
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<i64>,
    }
    let parsed: TokenResponse = response
        .json()
        .map_err(|e| format!("unexpected refresh response: {e}"))?;
    let tokens = AccountTokens {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token.or(Some(refresh)),
        expires_at: parsed
            .expires_in
            .map(|s| chrono::Utc::now().timestamp() + s),
    };
    save_tokens(&tokens)?;
    Ok(tokens)
}

pub fn forget_tokens() {
    remove(KEYCHAIN_ACCESS);
    remove(KEYCHAIN_REFRESH);
}

// ── website session (what the console actually uses) ───────────────────────

pub fn save_session_cookie(cookie: &str) -> Result<(), String> {
    store(KEYCHAIN_SESSION, cookie)
}

pub fn session_cookie() -> Option<String> {
    load(KEYCHAIN_SESSION)
}

pub fn forget_session_cookie() {
    remove(KEYCHAIN_SESSION);
}

/// Build a `Cookie:` header, session-ish cookies first.
pub fn cookie_header(pairs: &[(String, String)]) -> Option<String> {
    let mut sorted: Vec<&(String, String)> = pairs.iter().collect();
    sorted.sort_by_key(|(name, _)| {
        let n = name.to_ascii_lowercase();
        if n.contains("session") || n.contains("token") || n.contains("auth") {
            0
        } else {
            1
        }
    });
    let header = sorted
        .iter()
        .map(|(n, v)| format!("{n}={v}"))
        .collect::<Vec<_>>()
        .join("; ");
    if header.is_empty() {
        None
    } else {
        Some(header)
    }
}

// ── console server functions ───────────────────────────────────────────────

/// Diagnostics log (rotated per run) so console failures can be inspected
/// without screenshots.
pub fn log_line(message: &str) {
    use std::io::Write as _;
    let path = std::env::temp_dir().join("keyturn-console.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(
            file,
            "[{}] {message}",
            chrono::Local::now().format("%H:%M:%S%.3f")
        );
    }
}

fn call_server_fn(
    cookie: &str,
    id: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(CONSOLE_SERVER_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Origin", "https://opencode.ai")
        .header("Referer", "https://opencode.ai/")
        .header("X-Server-Id", id)
        .header("X-Server-Instance", "server-fn:0")
        .header("Cookie", cookie)
        .body(args.to_string())
        .send()
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let text = response.text().unwrap_or_default();
    log_line(&format!(
        "fn {} -> {status} {}",
        &id[..8],
        text.chars().take(120).collect::<String>()
    ));
    if status == 401 || status == 403 {
        return Err("AUTH".to_string());
    }
    if !(200..300).contains(&status) {
        return Err(format!(
            "console answered {status}: {}",
            text.chars().take(140).collect::<String>()
        ));
    }
    let mut value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("invalid JSON from console: {e}"))?;
    if let Some(inner) = value.get("result").cloned() {
        value = inner;
    }
    Ok(value)
}

/// Workspace ids available to the signed-in account.
pub fn fetch_workspaces(cookie: &str) -> Result<Vec<String>, String> {
    let value = call_server_fn(cookie, FN_WORKSPACES, serde_json::json!([]))
        .map_err(|e| format!("workspaces: {e}"))?;
    let mut ids = Vec::new();
    collect_ids(&value, &mut ids);
    if ids.is_empty() {
        return Err("no workspace found for this account".to_string());
    }
    Ok(ids)
}

pub fn collect_ids_public(value: &serde_json::Value, out: &mut Vec<String>) {
    collect_ids(value, out)
}

fn collect_ids(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ids(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(id) = map.get("id").and_then(|v| v.as_str()) {
                if !out.iter().any(|existing| existing == id) {
                    out.push(id.to_string());
                }
            }
            for key in ["workspaces", "workspace", "items", "data"] {
                if let Some(child) = map.get(key) {
                    collect_ids(child, out);
                }
            }
        }
        _ => {}
    }
}

/// Per-key/per-model cost for the account (workspace usage page, page 0).
pub fn fetch_console_usage_with_cookie(
    cookie: &str,
    workspace_id: &str,
    year: i32,
    month: u32,
    timezone: &str,
) -> Result<ConsoleUsage, String> {
    // The endpoint is (workspace, page); month/year come from each row's date.
    let _ = (year, month, timezone);
    let value = call_server_fn(cookie, FN_USAGE_LIST, serde_json::json!([workspace_id, 0]))
        .map_err(|e| format!("usage({workspace_id}): {e}"))?;
    let text = serde_json::to_string(&value).unwrap_or_default();
    parse_console_usage(&text)
}

/// The account's keys (id, name) for a workspace.
pub fn fetch_console_keys(cookie: &str, workspace_id: &str) -> Result<Vec<ConsoleKey>, String> {
    let value = call_server_fn(cookie, FN_KEYS, serde_json::json!([workspace_id]))
        .map_err(|e| format!("keys({workspace_id}): {e}"))?;
    let items = value.get("keys").cloned().unwrap_or(value);
    serde_json::from_value(items).map_err(|e| format!("unexpected key list: {e}"))
}

/// Post sign-in probe: resolve the workspace and pull one page of usage.
pub fn probe_console(cookie: &str) -> String {
    match fetch_workspaces(cookie) {
        Ok(ids) => match fetch_console_usage_with_cookie(cookie, &ids[0], 0, 0, "UTC") {
            Ok(usage) => format!(
                "account data available ({} keys, {} rows)",
                usage.keys.len(),
                usage.usage.len()
            ),
            Err(e) if e == "AUTH" => "session rejected — sign in again".to_string(),
            Err(e) => format!("usage call failed: {e}"),
        },
        Err(e) => format!("workspace lookup failed: {e}"),
    }
}

/// Best-effort IANA zone: `$TZ`, then the system zoneinfo symlink, then UTC.
pub fn iana_timezone() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        if !tz.is_empty() {
            return tz;
        }
    }
    if let Ok(link) = std::fs::read_link("/etc/localtime") {
        let parts: Vec<String> = link
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        if let Some(pos) = parts.iter().position(|p| p == "zoneinfo") {
            if pos + 1 < parts.len() {
                return parts[pos + 1..].join("/");
            }
        }
    }
    "UTC".to_string()
}

/// Server payloads may nest the rows; accept both `{usage,keys}` and `{result:…}`.
pub fn parse_console_usage(text: &str) -> Result<ConsoleUsage, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid JSON from console: {e}"))?;
    let payload = value
        .get("result")
        .or_else(|| value.get("data"))
        .unwrap_or(&value)
        .clone();
    serde_json::from_value(payload).map_err(|e| format!("unexpected console payload: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_s256_of_verifier() {
        let (verifier, challenge) = pkce_pair();
        assert!(verifier.len() >= 43);
        assert_eq!(challenge, b64url(&Sha256::digest(verifier.as_bytes())));
        assert!(!challenge.contains('='));
    }

    #[test]
    fn authorize_url_carries_pkce_and_loopback_redirect() {
        let (_, challenge) = pkce_pair();
        let url = authorize_url("http://127.0.0.1:8765/callback", &challenge, "state123");
        assert!(url.starts_with("https://auth.opencode.ai/authorize?"));
        assert!(url.contains("client_id=app"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8765%2Fcallback"));
    }

    #[test]
    fn parses_console_payload_in_both_shapes() {
        let direct = r#"{"keys":[{"id":"k_1","name":"personal"}],"usage":[{"model":"glm-5.3","keyID":"k_1","date":"2026-09-02","cost":1.25}]}"#;
        let usage = parse_console_usage(direct).expect("direct");
        assert_eq!(usage.keys.len(), 1);
        assert_eq!(usage.usage[0].key_id.as_deref(), Some("k_1"));
        assert!((usage.usage[0].cost.unwrap_or(0.0) - 1.25).abs() < 1e-9);

        let wrapped = r#"{"result":{"keys":[],"usage":[{"model":"kimi-k3","keyID":"k_9","day":"2026-09-03","amount":2.5}]}}"#;
        let usage = parse_console_usage(wrapped).expect("wrapped");
        assert_eq!(usage.usage[0].date.as_deref(), Some("2026-09-03"));
        assert!(parse_console_usage("not json").is_err());
    }

    #[test]
    fn workspace_ids_are_collected_from_nested_payloads() {
        let value: serde_json::Value = serde_json::json!({
            "workspaces": [{ "id": "ws_1", "name": "personal" }, { "id": "ws_2" }]
        });
        let mut ids = Vec::new();
        collect_ids(&value, &mut ids);
        assert_eq!(ids, vec!["ws_1".to_string(), "ws_2".to_string()]);
    }

    #[test]
    fn cookie_header_prefers_session_cookies() {
        let pairs = vec![
            ("_ga".to_string(), "1".to_string()),
            ("opencode-session".to_string(), "abc".to_string()),
        ];
        let header = cookie_header(&pairs).unwrap();
        assert!(header.starts_with("opencode-session=abc"));
        assert!(cookie_header(&[]).is_none());
    }

    #[test]
    fn timezone_falls_back_to_something_non_empty() {
        assert!(!iana_timezone().is_empty());
    }

    #[test]
    fn callback_parses_code_and_rejects_bad_state() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            wait_for_code(&listener, "expected-state", Duration::from_secs(5))
        });
        std::thread::sleep(Duration::from_millis(150));
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(
                b"GET /callback?code=abc123&state=expected-state HTTP/1.1\r\nHost: x\r\n\r\n",
            )
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert_eq!(handle.join().unwrap().unwrap(), "abc123");
        assert!(response.contains("You can close this tab"));
    }
}
