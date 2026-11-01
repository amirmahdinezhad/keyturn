//! Account-owned banked resets. GETs never redeem; only a reviewed, explicit
//! confirmation can POST. Durable request IDs make ambiguous retries idempotent.
use crate::{codex, paths, profiles, quota::QuotaSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::Duration,
};

const BASE: &str = "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits";
static STATE: LazyLock<Mutex<HashMap<String, Review>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredit {
    pub id: String,
    pub title: String,
    pub expires_at: Option<String>,
    pub covered_windows: Vec<String>,
    pub can_use: bool,
    pub unavailable_reason: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetInventory {
    pub available_count: u32,
    pub credits: Vec<ResetCredit>,
    pub usage: QuotaSet,
    pub pending_credit: Option<ResetCredit>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetReview {
    pub token: String,
    pub credit: ResetCredit,
    pub retry: bool,
}
#[derive(Clone)]
struct Review {
    profile_id: String,
    identity: String,
    request_id: String,
    plan_type: String,
    created_at: i64,
    view: ResetReview,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumeCode {
    Reset,
    NothingToReset,
    NoCredit,
    AlreadyRedeemed,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConsumeResult {
    pub code: ConsumeCode,
    #[serde(default)]
    pub windows_reset: u32,
}
#[derive(Clone, Deserialize, Serialize)]
struct Redemption {
    identity: String,
    credit: ResetCredit,
    request_id: String,
    outcome: Option<ConsumeResult>,
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Keyturn")
        .build()
        .map_err(|_| "Cannot connect to Codex resets.".into())
}
fn request(
    raw: &Value,
    method: reqwest::Method,
    url: &str,
) -> Result<reqwest::blocking::RequestBuilder, String> {
    let (token, account, _) =
        codex::oauth_access_from(raw).ok_or("Reconnect this ChatGPT account to read resets.")?;
    // A workspace-specific mutation must never fall back to the token's default workspace.
    let account = account
        .filter(|s| !s.is_empty())
        .ok_or("Reconnect this account to identify its ChatGPT workspace.")?;
    Ok(client()?
        .request(method, url)
        .bearer_auth(token)
        .header("ChatGPT-Account-Id", account)
        .header("Accept", "application/json"))
}
fn response_json(response: reqwest::blocking::Response) -> Result<Value, String> {
    match response.status().as_u16() {
        200..=299 => response.json().map_err(|_| {
            "Codex returned an unsupported reset response. Refresh and try again.".into()
        }),
        401 => Err("Codex login expired. Reconnect this account and try again.".into()),
        403 => Err("Codex did not allow reset access for this account.".into()),
        404 => Err("Banked resets are not available for this account.".into()),
        429 => Err("Codex reset requests are rate limited. Try again later.".into()),
        status => Err(format!(
            "Codex resets returned HTTP {status}. Try again later."
        )),
    }
}
fn details(raw: &Value) -> Result<Value, String> {
    let response = request(raw, reqwest::Method::GET, BASE)?
        .send()
        .map_err(|_| "Couldn't read resets. Check your connection and retry.")?;
    response_json(response)
}
fn coverage(usage: &QuotaSet) -> Vec<String> {
    let plan = usage
        .reset_credits
        .as_ref()
        .map(|s| s.plan_type.as_str())
        .unwrap_or("");
    usage
        .windows
        .iter()
        .flatten()
        .filter(|w| match plan {
            "plus" | "business" | "business_standard" | "team" => {
                matches!(w.window.as_str(), "5-hour" | "Weekly")
            }
            "pro" | "pro_100" | "pro_200" | "pro_500" => w.window == "Weekly",
            "go" => matches!(w.window.as_str(), "30d" | "Monthly" | "monthly"),
            _ => false,
        })
        .map(|w| w.window.clone())
        .collect()
}
fn parse_inventory(payload: &Value, usage: QuotaSet) -> Result<ResetInventory, String> {
    let available_count = payload
        .get("available_count")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or("Codex reset balance is unavailable. Retry to refresh it.")?;
    let credits = payload
        .get("credits")
        .and_then(Value::as_array)
        .ok_or("Codex reset details are unavailable. Retry to refresh them.")?;
    let covered = coverage(&usage);
    let now = chrono::Utc::now();
    let mut result = Vec::new();
    for credit in credits {
        if credit["status"].as_str() != Some("available") {
            continue;
        }
        let id = credit["id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .ok_or("Codex returned an invalid reset.")?;
        let expires_at = credit["expires_at"].as_str().map(str::to_owned);
        let expiry = expires_at
            .as_deref()
            .map(chrono::DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| "Codex returned an invalid reset expiration. Retry to refresh it.")?;
        let supported = credit["reset_type"].as_str() == Some("codex_rate_limits")
            && credit["is_supported_by_plan"].as_bool() != Some(false)
            && !covered.is_empty();
        let applicable = usage
            .reset_credits
            .as_ref()
            .and_then(|s| s.applicable_available_count);
        let needs_reset = usage
            .windows
            .iter()
            .flatten()
            .any(|w| covered.contains(&w.window) && w.percent.is_finite() && w.percent > 20.0);
        let reason = if expiry.is_some_and(|e| e <= now) {
            Some("This reset has expired.")
        } else if !supported {
            Some("This reset isn't supported by your current plan in Keyturn. Check Codex Usage settings.")
        } else if applicable == Some(0) {
            Some(
                "Codex says your resets aren't available to use right now. Refresh to check again.",
            )
        } else if !needs_reset {
            Some("Your usage does not need a reset right now. A covered allowance must have less than 80% remaining.")
        } else {
            None
        };
        result.push(ResetCredit {
            id: id.into(),
            title: credit["title"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Full reset")
                .into(),
            expires_at,
            covered_windows: if supported {
                covered.clone()
            } else {
                Vec::new()
            },
            can_use: reason.is_none(),
            unavailable_reason: reason.map(str::to_owned),
        });
    }
    // Select usable credits first; among equivalent choices spend the earliest expiry.
    result.sort_by_key(|c| {
        (
            !c.can_use,
            c.expires_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp())
                .unwrap_or(i64::MAX),
        )
    });
    Ok(ResetInventory {
        available_count,
        credits: result,
        usage,
        pending_credit: None,
    })
}
fn ledger_path() -> std::path::PathBuf {
    paths::opencode_user_data().join("keyturn-reset-redemptions.json")
}
fn read_ledger() -> Result<Vec<Redemption>, String> {
    match std::fs::read(ledger_path()) {
        Ok(raw) => serde_json::from_slice(&raw).map_err(|_| "Couldn't read previous reset attempts. Resolve the local reset history before using another reset.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err("Couldn't read previous reset attempts. Check file permissions.".into()),
    }
}
fn save_ledger(records: &[Redemption]) -> Result<(), String> {
    profiles::write_private(
        &ledger_path(),
        &serde_json::to_value(records).map_err(|_| "Cannot save reset attempt.")?,
    )
}
fn pending<'a>(records: &'a [Redemption], identity: &str) -> Option<&'a Redemption> {
    records
        .iter()
        .find(|r| r.identity == identity && r.outcome.is_none())
}

/// Read inventory and live usage, never activate accounts or consume a reset.
pub fn inventory(id: &str) -> Result<ResetInventory, String> {
    let _guard = STATE.lock().map_err(|_| "Reset access is busy.")?;
    profiles::with_reset_account(id, |raw, identity, usage| {
        let records = read_ledger()?;
        let mut inventory = parse_inventory(&details(raw)?, usage)?;
        inventory.pending_credit = pending(&records, identity).map(|r| r.credit.clone());
        Ok(inventory)
    })
}
/// Pin the selected credit, account identity and covered limits for explicit confirmation.
pub fn prepare(id: &str, credit_id: &str) -> Result<ResetReview, String> {
    let mut reviews = STATE.lock().map_err(|_| "Reset access is busy.")?;
    let review = profiles::with_reset_account(id, |raw, identity, usage| {
        let records = read_ledger()?;
        let outstanding = pending(&records, identity);
        let (credit, request_id, retry) = if let Some(record) = outstanding {
            if record.credit.id != credit_id {
                return Err("Check the previous reset attempt before using another reset.".into());
            }
            (record.credit.clone(), record.request_id.clone(), true)
        } else {
            let inventory = parse_inventory(&details(raw)?, usage.clone())?;
            let credit = inventory
                .credits
                .into_iter()
                .find(|c| c.id == credit_id)
                .ok_or("This reset is no longer available. Refresh and choose again.")?;
            if !credit.can_use {
                return Err(credit
                    .unavailable_reason
                    .unwrap_or("This reset isn't available right now.".into()));
            }
            if records.iter().any(|r| {
                r.identity == identity
                    && r.credit.id == credit_id
                    && r.outcome.as_ref().is_some_and(|o| {
                        matches!(o.code, ConsumeCode::Reset | ConsumeCode::AlreadyRedeemed)
                    })
            }) {
                return Err("This reset was already used. Refresh usage before continuing.".into());
            }
            (credit, uuid::Uuid::new_v4().to_string(), false)
        };
        Ok(Review {
            profile_id: id.into(),
            identity: identity.into(),
            request_id,
            plan_type: usage
                .reset_credits
                .as_ref()
                .map(|s| s.plan_type.clone())
                .unwrap_or_default(),
            created_at: chrono::Utc::now().timestamp(),
            view: ResetReview {
                token: uuid::Uuid::new_v4().to_string(),
                credit,
                retry,
            },
        })
    })?;
    reviews.retain(|_, r| chrono::Utc::now().timestamp() - r.created_at < 600);
    reviews.insert(review.view.token.clone(), review.clone());
    Ok(review.view)
}
/// The only mutation path. Persist the request ID before POST; retries use that ID.
pub fn consume(id: &str, review_token: &str) -> Result<ConsumeResult, String> {
    let mut reviews = STATE.lock().map_err(|_| "Reset access is busy.")?;
    let review = reviews
        .get(review_token)
        .cloned()
        .ok_or("Review this reset again before confirming.")?;
    if review.profile_id != id || chrono::Utc::now().timestamp() - review.created_at >= 600 {
        return Err("This confirmation expired. Review the reset again.".into());
    }
    let outcome = profiles::with_reset_account(id, |raw, identity, usage| {
        if review.identity != identity {
            return Err("This account changed. Review its resets again before confirming.".into());
        }
        redeem_with(
            &review,
            identity,
            &mut read_ledger()?,
            || {
                let fresh = parse_inventory(&details(raw)?, usage)?;
                let credit = fresh
                    .credits
                    .iter()
                    .find(|c| c.id == review.view.credit.id)
                    .ok_or("This reset is no longer available. Refresh and review again.")?;
                if !credit.can_use {
                    return Err(credit
                        .unavailable_reason
                        .clone()
                        .unwrap_or("This reset isn't available right now.".into()));
                }
                let plan = fresh
                    .usage
                    .reset_credits
                    .as_ref()
                    .map(|s| s.plan_type.as_str())
                    .unwrap_or("");
                if plan != review.plan_type
                    || credit.covered_windows != review.view.credit.covered_windows
                    || credit.expires_at != review.view.credit.expires_at
                    || credit.title != review.view.credit.title
                {
                    return Err(
                        "Reset details changed. Refresh and review them again before confirming."
                            .into(),
                    );
                }
                Ok(())
            },
            save_ledger,
            |request_id, credit_id| {
                let payload =
                    serde_json::json!({ "redeem_request_id": request_id, "credit_id": credit_id });
                let result = request(raw, reqwest::Method::POST, &format!("{BASE}/consume"))?
            .json(&payload).send().map_err(|_| "Reset result is unknown. Retry the same reset to check it; don't use another reset.".to_string())
            .and_then(response_json)
            .and_then(|value| serde_json::from_value::<ConsumeResult>(value).map_err(|_| "Reset result is unknown. Retry the same reset to check it.".into()));
                let result = result.map_err(|error| {
                    format!(
                        "{error} Your attempt is saved; retry this same reset to check the result."
                    )
                })?;
                Ok(result)
            },
        )
    })?;
    reviews.remove(review_token);
    Ok(outcome)
}

// The durable transaction is independent of transport and credentials so failures
// and process restarts can be tested without spending a real account's reset.
fn redeem_with(
    review: &Review,
    identity: &str,
    records: &mut Vec<Redemption>,
    validate: impl FnOnce() -> Result<(), String>,
    mut persist: impl FnMut(&[Redemption]) -> Result<(), String>,
    post: impl FnOnce(&str, &str) -> Result<ConsumeResult, String>,
) -> Result<ConsumeResult, String> {
    if review.identity != identity {
        return Err("This account changed. Review its resets again before confirming.".into());
    }
    if let Some(record) = records
        .iter()
        .find(|r| r.identity == identity && r.credit.id == review.view.credit.id)
    {
        if record.request_id == review.request_id {
            if let Some(result) = &record.outcome {
                return Ok(result.clone());
            }
        } else if record
            .outcome
            .as_ref()
            .is_none_or(|o| matches!(o.code, ConsumeCode::Reset | ConsumeCode::AlreadyRedeemed))
        {
            return Err(
                "Another attempt already exists for this reset. Refresh before continuing.".into(),
            );
        }
    }
    if pending(records, identity).is_some_and(|r| r.request_id != review.request_id) {
        return Err("Check the previous reset attempt before using another reset.".into());
    }
    let replay = records
        .iter()
        .any(|r| r.identity == identity && r.request_id == review.request_id);
    if !replay {
        validate()?;
        records.retain(|r| r.identity != identity || r.credit.id != review.view.credit.id);
        records.push(Redemption {
            identity: identity.into(),
            credit: review.view.credit.clone(),
            request_id: review.request_id.clone(),
            outcome: None,
        });
        persist(records)?;
    }
    let result = post(&review.request_id, &review.view.credit.id)?;
    if let Some(record) = records
        .iter_mut()
        .find(|r| r.identity == identity && r.request_id == review.request_id)
    {
        record.outcome = Some(result.clone());
    }
    persist(records)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quota::{QuotaWindow, ResetCreditsSummary};
    use serde_json::json;
    fn usage(plan: &str, applicable: Option<u32>, used: f64) -> QuotaSet {
        QuotaSet {
            provider: "codex".into(),
            windows: Some(vec![
                QuotaWindow {
                    window: "5-hour".into(),
                    percent: used,
                    resets_at: None,
                    status: None,
                },
                QuotaWindow {
                    window: "Weekly".into(),
                    percent: used,
                    resets_at: None,
                    status: None,
                },
                QuotaWindow {
                    window: "extra model".into(),
                    percent: 100.0,
                    resets_at: None,
                    status: None,
                },
            ]),
            error: None,
            as_of: None,
            reset_credits: Some(ResetCreditsSummary {
                available_count: 2,
                applicable_available_count: applicable,
                plan_type: plan.into(),
            }),
        }
    }
    fn credit(id: &str, expiry: Option<&str>) -> Value {
        json!({"id":id,"reset_type":"codex_rate_limits","is_supported_by_plan":true,"status":"available","expires_at":expiry,"title":"Full reset"})
    }
    fn fixture() -> Review {
        Review {
            profile_id: "inactive".into(),
            identity: "account-a".into(),
            request_id: "request-1".into(),
            plan_type: "plus".into(),
            created_at: chrono::Utc::now().timestamp(),
            view: ResetReview {
                token: "review-token".into(),
                credit: ResetCredit {
                    id: "credit-1".into(),
                    title: "Full reset".into(),
                    expires_at: None,
                    covered_windows: vec!["Weekly".into()],
                    can_use: true,
                    unavailable_reason: None,
                },
                retry: false,
            },
        }
    }
    #[test]
    fn inventory_preserves_unknown_balance_and_rejects_bad_expiry() {
        for payload in [
            json!({"credits":[]}),
            json!({"available_count":-1,"credits":[]}),
            json!({"available_count":0}),
            json!({"available_count":1,"credits":[credit("one",Some("bad-date"))]}),
        ] {
            assert!(parse_inventory(&payload, usage("plus", None, 50.0)).is_err());
        }
        assert_eq!(
            parse_inventory(
                &json!({"available_count":0,"credits":[]}),
                usage("plus", None, 50.0)
            )
            .unwrap()
            .available_count,
            0
        );
    }
    #[test]
    fn backend_inapplicability_wins_over_display_percent() {
        let inventory = parse_inventory(
            &json!({"available_count":1,"credits":[credit("one",None)]}),
            usage("plus", Some(0), 84.0),
        )
        .unwrap();
        assert!(!inventory.credits[0].can_use);
        assert!(inventory.credits[0]
            .unavailable_reason
            .as_deref()
            .unwrap()
            .contains("Codex says"));
        let healthy = parse_inventory(
            &json!({"available_count":1,"credits":[credit("one",None)]}),
            usage("plus", None, 20.0),
        )
        .unwrap();
        assert!(!healthy.credits[0].can_use);
    }
    #[test]
    fn earliest_usable_expiry_is_first_and_coverage_excludes_extra_models() {
        let mut unsupported = credit("unsupported", Some("2097-01-01T00:00:00Z"));
        unsupported["is_supported_by_plan"] = json!(false);
        let payload = json!({"available_count":4,"credits":[credit("no-expiry",None),credit("later",Some("2099-02-01T00:00:00Z")),unsupported,credit("first",Some("2099-01-01T00:00:00Z")),credit("expired",Some("2020-01-01T00:00:00Z"))]});
        let inventory = parse_inventory(&payload, usage("plus", Some(3), 50.0)).unwrap();
        assert_eq!(inventory.credits[0].id, "first");
        assert_eq!(
            inventory.credits[0].covered_windows,
            vec!["5-hour", "Weekly"]
        );
        assert!(
            !inventory
                .credits
                .iter()
                .find(|c| c.id == "expired")
                .unwrap()
                .can_use
        );
        assert!(
            !inventory
                .credits
                .iter()
                .find(|c| c.id == "unsupported")
                .unwrap()
                .can_use
        );
        assert_eq!(coverage(&usage("pro_200", None, 50.0)), vec!["Weekly"]);
        assert!(coverage(&usage("new-plan", None, 50.0)).is_empty());
    }
    #[test]
    fn unknown_result_survives_restart_and_replays_the_same_request() {
        let review = fixture();
        let mut records = vec![];
        let mut disk = Value::Null;
        let first = redeem_with(
            &review,
            "account-a",
            &mut records,
            || Ok(()),
            |rows| {
                disk = serde_json::to_value(rows).unwrap();
                Ok(())
            },
            |request, credit| {
                assert_eq!((request, credit), ("request-1", "credit-1"));
                Err("connection lost after POST".into())
            },
        );
        assert!(first.is_err());
        let mut restored: Vec<Redemption> = serde_json::from_value(disk).unwrap();
        let result = redeem_with(
            &review,
            "account-a",
            &mut restored,
            || panic!("pending replay must not revalidate changed eligibility"),
            |_| Ok(()),
            |request, credit| {
                assert_eq!((request, credit), ("request-1", "credit-1"));
                Ok(ConsumeResult {
                    code: ConsumeCode::Reset,
                    windows_reset: 2,
                })
            },
        )
        .unwrap();
        assert!(matches!(result.code, ConsumeCode::Reset));
        let again = redeem_with(
            &review,
            "account-a",
            &mut restored,
            || panic!(),
            |_| panic!(),
            |_, _| panic!("duplicate must not POST"),
        )
        .unwrap();
        assert_eq!(again.windows_reset, 2);
    }
    #[test]
    fn disk_or_validation_failure_never_posts() {
        let r = fixture();
        assert!(redeem_with(
            &r,
            "account-a",
            &mut vec![],
            || Err("eligibility changed".into()),
            |_| panic!(),
            |_, _| panic!()
        )
        .is_err());
        assert!(redeem_with(
            &r,
            "account-a",
            &mut vec![],
            || Ok(()),
            |_| Err("disk full".into()),
            |_, _| panic!()
        )
        .is_err());
        assert!(redeem_with(
            &r,
            "other-account",
            &mut vec![],
            || panic!(),
            |_| panic!(),
            |_, _| panic!()
        )
        .is_err());
    }
    #[test]
    fn pending_attempt_blocks_another_credit_or_new_request_id() {
        let r = fixture();
        let mut records = vec![Redemption {
            identity: r.identity.clone(),
            credit: r.view.credit.clone(),
            request_id: r.request_id.clone(),
            outcome: None,
        }];
        let mut other = r.clone();
        other.request_id = "request-2".into();
        assert!(redeem_with(
            &other,
            "account-a",
            &mut records,
            || panic!(),
            |_| panic!(),
            |_, _| panic!()
        )
        .is_err());
        other.view.credit.id = "credit-2".into();
        assert!(redeem_with(
            &other,
            "account-a",
            &mut records,
            || panic!(),
            |_| panic!(),
            |_, _| panic!()
        )
        .is_err());
    }
    #[test]
    fn post_success_with_failed_receipt_write_still_has_a_replayable_id() {
        let r = fixture();
        let mut disk = vec![];
        let mut writes = 0;
        assert!(redeem_with(
            &r,
            "account-a",
            &mut vec![],
            || Ok(()),
            |rows| {
                writes += 1;
                if writes == 1 {
                    disk = rows.to_vec();
                    Ok(())
                } else {
                    Err("disk full".into())
                }
            },
            |_, _| Ok(ConsumeResult {
                code: ConsumeCode::Reset,
                windows_reset: 1
            })
        )
        .is_err());
        assert!(pending(&disk, "account-a").is_some());
        assert!(matches!(
            redeem_with(
                &r,
                "account-a",
                &mut disk,
                || panic!(),
                |_| Ok(()),
                |id, _| {
                    assert_eq!(id, "request-1");
                    Ok(ConsumeResult {
                        code: ConsumeCode::AlreadyRedeemed,
                        windows_reset: 0,
                    })
                }
            )
            .unwrap()
            .code,
            ConsumeCode::AlreadyRedeemed
        ));
    }
}
