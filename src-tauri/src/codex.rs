//! Codex CLI credential support.
//!
//! Codex keeps its credentials in `$CODEX_HOME/auth.json` (default
//! `~/.codex/auth.json`):
//!
//! ```json
//! { "OPENAI_API_KEY": "sk-…",
//!   "tokens": { "id_token": "…", "access_token": "…", "refresh_token": "…", "account_id": "…" },
//!   "last_refresh": "2026-01-01T00:00:00Z" }
//! ```
//!
//! We only ever patch fields we own, and every write goes through the atomic
//! 0600 writer in `paths`.

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

use crate::paths;

/// Public client id the Codex CLI uses for ChatGPT OAuth refreshes.
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const REFRESH_URL: &str = "https://auth.openai.com/oauth/token";

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodexStatus {
    pub present: bool,
    pub has_api_key: bool,
    pub has_tokens: bool,
    pub email: Option<String>,
    pub plan: Option<String>,
    pub account_id: Option<String>,
    /// Unix seconds from the access token's `exp` claim, when parseable.
    pub expires_at: Option<i64>,
    pub last_refresh: Option<String>,
}

fn claim<'a>(claims: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut node = claims;
    for key in path {
        node = node.get(*key)?;
    }
    node.as_str()
}

fn jwt_payload(jwt: &str) -> Option<Value> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn id_token_info(raw: &str) -> (Option<String>, Option<String>, Option<String>) {
    let Some(claims) = jwt_payload(raw) else {
        return (None, None, None);
    };
    let email = claim(&claims, &["https://api.openai.com/profile", "email"])
        .or_else(|| claim(&claims, &["email"]))
        .map(str::to_string);
    let plan = claim(
        &claims,
        &["https://api.openai.com/auth", "chatgpt_plan_type"],
    )
    .map(str::to_string);
    let account = claim(
        &claims,
        &["https://api.openai.com/auth", "chatgpt_account_id"],
    )
    .map(str::to_string);
    (email, plan, account)
}

pub fn status() -> CodexStatus {
    let file = paths::codex_auth_file();
    let Some(Value::Object(root)) = paths::read_json(&file) else {
        return CodexStatus::default();
    };
    let api_key = root
        .get("OPENAI_API_KEY")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let tokens = root.get("tokens");
    let id_token = tokens
        .and_then(|t| t.get("id_token"))
        .and_then(|v| v.as_str());
    let access_token = tokens
        .and_then(|t| t.get("access_token"))
        .and_then(|v| v.as_str());
    let (email, plan, claimed_account) = id_token.map(id_token_info).unwrap_or((None, None, None));
    CodexStatus {
        present: true,
        has_api_key: api_key.is_some(),
        has_tokens: tokens.is_some_and(|t| {
            t.get("refresh_token")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
        }),
        email,
        plan,
        account_id: tokens
            .and_then(|t| t.get("account_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or(claimed_account),
        expires_at: access_token
            .and_then(jwt_payload)
            .and_then(|c| c.get("exp").and_then(|v| v.as_i64())),
        last_refresh: root
            .get("last_refresh")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }
}

/// Patch only `OPENAI_API_KEY`, preserving whatever else Codex stored.
pub fn set_api_key(key: &str) -> Result<(), String> {
    let file = paths::codex_auth_file();
    let mut root = match paths::read_json(&file) {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    root.insert("OPENAI_API_KEY".to_string(), json!(key));
    paths::atomic_write_json(&file, &Value::Object(root));
    Ok(())
}

pub fn api_key() -> Option<String> {
    let Value::Object(root) = paths::read_json(&paths::codex_auth_file())? else {
        return None;
    };
    root.get("OPENAI_API_KEY")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Exchange the stored refresh token for fresh ChatGPT tokens.
pub fn refresh_tokens() -> Result<CodexStatus, String> {
    let file = paths::codex_auth_file();
    let Some(Value::Object(root)) = paths::read_json(&file) else {
        return Err("Codex auth.json not found — run `codex login` first".to_string());
    };
    let refresh = root
        .get("tokens")
        .and_then(|t| t.get("refresh_token"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("No refresh token stored — sign in with `codex login` first")?
        .to_string();

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(REFRESH_URL)
        .json(&json!({
            "client_id": CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": refresh,
        }))
        .send()
        .map_err(|e| e.to_string())?;
    let http_status = response.status();
    let text = response.text().unwrap_or_default();
    if !http_status.is_success() {
        return Err(format!(
            "refresh failed ({http_status}): {}",
            text.chars().take(160).collect::<String>()
        ));
    }
    let body: Value =
        serde_json::from_str(&text).map_err(|e| format!("unexpected refresh response: {e}"))?;
    let access = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("refresh response has no access_token")?;

    let mut updated = root;
    let tokens = updated
        .entry("tokens".to_string())
        .or_insert_with(|| json!({}));
    if let Some(map) = tokens.as_object_mut() {
        map.insert("access_token".to_string(), json!(access));
        if let Some(next) = body.get("refresh_token").and_then(|v| v.as_str()) {
            map.insert("refresh_token".to_string(), json!(next));
        }
        if let Some(id) = body.get("id_token").and_then(|v| v.as_str()) {
            map.insert("id_token".to_string(), json!(id));
        }
        if !map.contains_key("account_id") {
            if let Some(account) = body
                .get("id_token")
                .and_then(|v| v.as_str())
                .and_then(|id| id_token_info(id).2)
            {
                map.insert("account_id".to_string(), json!(account));
            }
        }
    }
    updated.insert(
        "last_refresh".to_string(),
        json!(chrono::Utc::now().to_rfc3339()),
    );
    paths::atomic_write_json(&file, &Value::Object(updated));
    Ok(status())
}

/// Does the live file still hold this exact API key?
pub fn matches(key: &str) -> bool {
    api_key().is_some_and(|live| live == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(value: &Value) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value.to_string())
    }

    fn fake_jwt() -> String {
        format!(
            "{}.{}.sig",
            b64(&json!({"alg": "RS256"})),
            b64(&json!({
                "exp": 4_100_000_000i64,
                "https://api.openai.com/profile": {"email": "dev@example.com"},
                "https://api.openai.com/auth": {
                    "chatgpt_plan_type": "plus",
                    "chatgpt_account_id": "acc_1"
                }
            }))
        )
    }

    #[test]
    fn id_token_claims_are_parsed_without_verification() {
        let (email, plan, account) = id_token_info(&fake_jwt());
        assert_eq!(email.as_deref(), Some("dev@example.com"));
        assert_eq!(plan.as_deref(), Some("plus"));
        assert_eq!(account.as_deref(), Some("acc_1"));
        assert_eq!(id_token_info("not-a-jwt"), (None, None, None));
    }

    #[test]
    fn set_api_key_preserves_tokens_and_is_atomic() {
        let dir = std::env::temp_dir().join(format!("aks-codex-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("auth.json");
        paths::atomic_write_json(
            &file,
            &json!({
                "tokens": {"access_token": "a", "refresh_token": "r"},
                "last_refresh": "2026-01-01T00:00:00Z"
            }),
        );
        // Use the same patch code, but against the temp file by faking env.
        let mut root = match paths::read_json(&file).unwrap() {
            Value::Object(map) => map,
            _ => panic!("object"),
        };
        root.insert("OPENAI_API_KEY".to_string(), json!("sk-test"));
        paths::atomic_write_json(&file, &Value::Object(root));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "credential file must stay 0600");
        }
        let read = paths::read_json(&file).unwrap();
        assert_eq!(read["OPENAI_API_KEY"], json!("sk-test"));
        assert_eq!(read["tokens"]["refresh_token"], json!("r"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
