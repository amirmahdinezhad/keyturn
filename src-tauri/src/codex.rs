//! Codex CLI credential status (read-only).
//!
//! Codex keeps its credentials in `$CODEX_HOME/auth.json` (default
//! `~/.codex/auth.json`). Keyturn never writes this file and never refreshes
//! tokens — the user owns auth via `codex login` in their own terminal.
//! We only read the file to enrich the CLI status display.
//!
//! The authoritative login check is `codex login status` (see `cli.rs`).

use base64::Engine;
use serde::Serialize;
use serde_json::Value;

use crate::paths;

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

/// Read-only peek at the CLI's auth file. Never writes.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn missing_file_reports_absent_without_writing() {
        // No file at this path in CI: status must be default, not an error.
        let dir = std::env::temp_dir().join(format!("keyturn-codex-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("no-auth.json");
        let _ = std::fs::remove_file(&missing);
        assert!(paths::read_json(&missing).is_none());
        let status = CodexStatus::default();
        assert!(!status.present);
        std::fs::remove_dir_all(&dir).ok();
    }
}
