//! Subscription billing state. OAuth `exp`/`expiresAt` describe credentials,
//! not billing, and must never cause an account to disappear from the panel.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub service: String,
    pub plan: Option<String>,
    pub expires_at: Option<String>,
    pub status: Option<String>,
    pub monthly_price_usd: Option<f64>,
}

impl Subscription {
    /// Discovery lists only confirmed paid plans with no known billing expiry.
    pub fn is_paid(&self) -> bool {
        if self.is_expired() { return false; }
        let plan = self.plan.as_deref().unwrap_or("").trim().to_ascii_lowercase().replace([' ', '-'], "_");
        if matches!(plan.as_str(), "free" | "trial" | "free_tier") { return false; }
        if let Some(price) = self.monthly_price_usd { return price.is_finite() && price > 0.0; }
        match self.id.as_str() {
            "chatgpt" => matches!(plan.as_str(), "go" | "plus" | "pro" | "prolite" | "pro_lite" | "pro_100" | "pro_200" | "pro_500" | "team" | "business" | "enterprise" | "edu"),
            "claude" => matches!(plan.as_str(), "pro" | "max" | "max_5x" | "max_20x" | "team" | "enterprise"),
            "antigravity" => self.status.as_deref() == Some("active"),
            "opencode-go" => self.status.as_deref() == Some("active"),
            _ => false,
        }
    }

    /// Only confirmed billing expiry permits deleting a saved service connection.
    pub fn is_expired(&self) -> bool {
        self.status.as_deref() == Some("expired")
            || self.expires_at.as_deref()
                .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
                .is_some_and(|end| end <= chrono::Utc::now())
    }
}

/// ChatGPT's billing claims are distinct from the JWT/session lifetime.
pub fn chatgpt(claims: &Value) -> Subscription {
    let auth = &claims["https://api.openai.com/auth"];
    Subscription {
        id: "chatgpt".into(),
        service: "ChatGPT".into(),
        plan: auth["chatgpt_plan_type"].as_str().map(str::to_owned),
        expires_at: auth["chatgpt_subscription_active_until"]
            .as_str()
            .map(str::to_owned),
        status: None,
        monthly_price_usd: None,
    }
}

/// Claude reports the subscription independently from the organization name.
pub fn claude(status: &Value) -> Subscription {
    let plan = match status["rateLimitTier"].as_str().unwrap_or("") {
        tier if tier.contains("max_20x") => Some("max_20x"),
        tier if tier.contains("max_5x") => Some("max_5x"),
        _ => status["subscriptionType"].as_str(),
    };
    Subscription {
        id: "claude".into(),
        service: "Claude".into(),
        plan: plan.map(str::to_owned),
        expires_at: status["subscriptionExpiresAt"].as_str().map(str::to_owned),
        status: None,
        monthly_price_usd: None,
    }
}

type GoCache = Option<(String, Instant, Subscription)>;
static GO_CACHE: LazyLock<Mutex<GoCache>> = LazyLock::new(|| Mutex::new(None));

/// A newly added connection must recheck billing, even when it reuses the old key.
pub(crate) fn forget_opencode_go(key: &str) {
    if let Ok(mut cache) = GO_CACHE.lock() {
        if cache.as_ref().is_some_and(|(cached_key, _, _)| cached_key == key) {
            *cache = None;
        }
    }
}

fn go_metadata(payload: &Value) -> Subscription {
    let metadata = &payload["subscription"];
    let error = &payload["error"];
    // A generic 401/403, exhausted quota or a transport failure proves nothing
    // about billing. Only explicit subscription state may hide this service.
    let expired = metadata["status"].as_str() == Some("expired")
        || error["code"].as_str() == Some("subscription_expired")
        || payload["code"].as_str() == Some("subscription_expired")
        // The live Go endpoint reports ended entitlement without an error code.
        || (error["type"].as_str() == Some("EntitlementError")
            && error["message"].as_str().is_some_and(|message| {
                message.trim().trim_end_matches('.').eq_ignore_ascii_case("OpenCode Go subscription required")
            }));
    let active = metadata["status"].as_str() == Some("active")
        || crate::quota::parse_windows(payload).is_some();
    Subscription {
        id: "opencode-go".into(),
        service: "OpenCode Go".into(),
        plan: metadata["plan"].as_str().map(str::to_owned),
        expires_at: metadata["expiresAt"].as_str().map(str::to_owned),
        status: Some(if expired { "expired" } else if active { "active" } else { "unknown" }.into()),
        monthly_price_usd: metadata["monthlyPriceUsd"]
            .as_f64()
            .filter(|price| price.is_finite() && *price >= 0.0),
    }
}

/// A transport/WAF error cannot undo confirmed billing state; real usage can.
fn reconcile_go(previous: Option<&Subscription>, payload: Option<&Value>) -> Subscription {
    let fresh = go_metadata(payload.unwrap_or(&Value::Null));
    if fresh.status.as_deref() == Some("unknown") {
        if let Some(known) = previous.filter(|s| matches!(s.status.as_deref(), Some("active" | "expired"))) {
            return known.clone();
        }
    }
    fresh
}

/// Read billing metadata from the existing Go usage endpoint, once a minute.
/// Missing metadata stays unknown: a stored Go key also works with Go Plus.
pub fn opencode_go(key: &str) -> Subscription {
    let mut previous = None;
    if let Ok(cache) = GO_CACHE.lock() {
        if let Some((cached_key, at, subscription)) = cache.as_ref() {
            if cached_key == key {
                if at.elapsed() < Duration::from_secs(60) {
                    return subscription.clone();
                }
                previous = Some(subscription.clone());
            }
        }
    }
    let payload = reqwest::blocking::Client::builder()
        .user_agent(concat!("Keyturn/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(4))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()
        .and_then(|client| {
            client
                .get("https://opencode.ai/zen/go/v1/usage")
                .bearer_auth(key)
                .header("Accept", "application/json")
                .send()
                .ok()
        })
        .and_then(|response| response.json::<Value>().ok());
    let subscription = reconcile_go(previous.as_ref(), payload.as_ref());
    if let Ok(mut cache) = GO_CACHE.lock() {
        *cache = Some((key.into(), Instant::now(), subscription.clone()));
    }
    subscription
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn discovery_requires_confirmed_paid_billing_and_ignores_token_expiry() {
        let mut paid = chatgpt(&json!({"exp": 1, "https://api.openai.com/auth": {"chatgpt_plan_type": "plus"}}));
        assert!(paid.is_paid());
        paid.plan = Some("free".into()); assert!(!paid.is_paid());
        paid.plan = None; assert!(!paid.is_paid());
        paid.plan = Some("plus".into()); paid.monthly_price_usd = Some(0.0); assert!(!paid.is_paid());
        paid.monthly_price_usd = None; paid.status = Some("expired".into()); assert!(!paid.is_paid());
        paid.status = None; paid.expires_at = Some("2000-01-01T00:00:00Z".into()); assert!(!paid.is_paid());
        assert!(claude(&json!({"subscriptionType":"max"})).is_paid());
        assert!(!claude(&json!({"subscriptionType":"free"})).is_paid());
        assert!(go_metadata(&json!({"subscription":{"status":"active"}})).is_paid());
        assert!(!go_metadata(&json!({})).is_paid());
    }

    #[test]
    fn billing_expiry_is_independent_of_token_expiry() {
        let value = json!({"exp": 1, "https://api.openai.com/auth": {
            "chatgpt_plan_type": "plus", "chatgpt_subscription_active_until": "2099-01-01T00:00:00Z"
        }});
        let subscription = chatgpt(&value);
        assert_eq!(
            subscription.expires_at.as_deref(),
            Some("2099-01-01T00:00:00Z")
        );
        assert!(chatgpt(&json!({"exp": 1})).expires_at.is_none());
    }

    #[test]
    fn claude_retains_plan_and_capacity_in_organization_accounts() {
        let subscription = claude(
            &json!({"orgName":"Acme", "subscriptionType":"max", "rateLimitTier":"default_claude_max_20x", "expiresAt": 1}),
        );
        assert_eq!(subscription.plan.as_deref(), Some("max_20x"));
        assert!(subscription.expires_at.is_none());
    }

    #[test]
    fn go_subscription_required_means_no_active_subscription() {
        let ended = go_metadata(&json!({"error": {
            "type": "EntitlementError", "message": "OpenCode Go subscription required."
        }}));
        assert_eq!(ended.status.as_deref(), Some("expired"));
    }

    #[test]
    fn deleted_connections_forget_only_their_own_cached_billing_state() {
        let ended = go_metadata(&json!({"code":"subscription_expired"}));
        *GO_CACHE.lock().unwrap() = Some(("deleted-key".into(), Instant::now(), ended));
        forget_opencode_go("another-key");
        assert!(GO_CACHE.lock().unwrap().is_some());
        forget_opencode_go("deleted-key");
        assert!(GO_CACHE.lock().unwrap().is_none());
    }

    #[test]
    fn ended_go_entitlement_survives_transport_errors_but_real_usage_restores_it() {
        let ended = go_metadata(&json!({"error": {
            "type": "EntitlementError", "message": "OpenCode Go subscription required."
        }}));
        assert_eq!(reconcile_go(Some(&ended), None).status.as_deref(), Some("expired"));
        assert_eq!(reconcile_go(Some(&ended), Some(&json!({"status":403,"error_code":1010}))).status.as_deref(), Some("expired"));
        let renewed = reconcile_go(Some(&ended), Some(&json!({"usage":{"monthly":{"percent":100}}})));
        assert_eq!(renewed.status.as_deref(), Some("active"));
        // A model permission failure does not establish the state of the plan.
        let model_error = json!({"error":{"type":"EntitlementError","message":"Model is not included."}});
        assert_eq!(reconcile_go(None, Some(&model_error)).status.as_deref(), Some("unknown"));
    }

    #[test]
    fn go_requires_billing_evidence_before_hiding_or_pricing() {
        let unknown = go_metadata(&json!({"status":403,"error_code":"access_denied"}));
        assert_eq!(unknown.status.as_deref(), Some("unknown"));
        assert!(unknown.plan.is_none() && unknown.expires_at.is_none());
        let expired = go_metadata(&json!({"code":"subscription_expired"}));
        assert_eq!(expired.status.as_deref(), Some("expired"));
        let plus = go_metadata(
            &json!({"subscription":{"plan":"go_plus","monthlyPriceUsd":40,"expiresAt":"2099-01-01T00:00:00Z"}}),
        );
        assert_eq!(plus.plan.as_deref(), Some("go_plus"));
        assert_eq!(plus.monthly_price_usd, Some(40.0));
    }
}
