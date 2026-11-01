//! Experimental account-scoped Antigravity OAuth, based on inspected CodexBar behavior.
//! No ambient Keychain writes; OAuth grants stay in private Keyturn-owned files.
use crate::{
    account_login::Progress,
    paths,
    profiles::{private_dir, write_private},
    subscriptions::Subscription,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
    time::{Duration, Instant},
};

// Refresh-token rotation and index mutations must not race account-scoped quota reads.
static REPOSITORY: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
fn with_repository<T>(action: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _guard = REPOSITORY
        .lock()
        .map_err(|_| "Antigravity accounts are busy. Try again.")?;
    action()
}

const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const CONSUMER: &str = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
#[derive(Clone)]
struct Client {
    id: String,
    secret: String,
}
fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(format!(
            "antigravity/hub/2.9.1 darwin/{}",
            if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "amd64"
            }
        ))
        .build()
        .map_err(|_| "Cannot initialize Google connection.".into())
}
fn json_response(request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
    let response = request
        .send()
        .map_err(|_| "Cannot reach Google. Check your connection and retry.")?;
    if !response.status().is_success() {
        return Err(format!("Google could not verify this account (HTTP {}). Sign in again or check account access.", response.status().as_u16()));
    }
    let bytes = response
        .take(1024 * 1024)
        .bytes()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Cannot read Google's response.")?;
    serde_json::from_slice(&bytes).map_err(|_| "Google returned an unsupported response.".into())
}
fn string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| {
        value[*k]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned)
    })
}
fn credentials(home: &Path) -> Result<Value, String> {
    let raw = std::fs::read(home.join("auth.json"))
        .map_err(|_| "Choose a Google credentials JSON file.")?;
    if raw.len() > 256 * 1024 {
        return Err("Use a credentials file smaller than 256 KB.".into());
    }
    let input: Value =
        serde_json::from_slice(&raw).map_err(|_| "This is not valid Google credentials JSON.")?;
    let tokens = input.get("token").unwrap_or(&input);
    let mut value = json!({
        "access_token":string(tokens,&["access_token","accessToken"]),
        "refresh_token":string(tokens,&["refresh_token","refreshToken"]),
        "email":string(&input,&["email"]),
        "project_id":string(&input,&["project_id","projectId"]),
        "client_id":string(&input,&["client_id","clientId"]),
        "client_secret":string(&input,&["client_secret","clientSecret"]),
        "expiry_date": input.get("expiry_date").or(input.get("expiresAt")).cloned().unwrap_or(Value::Null),
    });
    if value["refresh_token"].as_str().is_none() {
        return Err("This file has no Google refresh token. Use Sign in with Google.".into());
    }
    if value["client_id"].as_str().is_none() || value["client_secret"].as_str().is_none() {
        let client = resolve_client()?;
        value["client_id"] = client.id.into();
        value["client_secret"] = client.secret.into();
    }
    Ok(value)
}
/// Server-confirm email and paidTier; model availability and quota percentages are not proof of a paid plan.
pub fn inspect(home: &Path) -> Result<(String, Option<String>, Subscription), String> {
    with_repository(|| inspect_unlocked(home))
}
fn inspect_unlocked(home: &Path) -> Result<(String, Option<String>, Subscription), String> {
    let mut value = credentials(home)?;
    let client = http()?;
    let expired = value["expiry_date"].as_i64().map_or(true, |ms| {
        ms <= chrono::Utc::now().timestamp_millis() + 60_000
    });
    if expired || value["access_token"].as_str().is_none() {
        let token = json_response(client.post(TOKEN).form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", value["refresh_token"].as_str().unwrap()),
            ("client_id", value["client_id"].as_str().unwrap()),
            ("client_secret", value["client_secret"].as_str().unwrap()),
        ]))?;
        value["access_token"] = token["access_token"].clone();
        value["expiry_date"] = (chrono::Utc::now().timestamp_millis()
            + token["expires_in"].as_i64().unwrap_or(3600) * 1000)
            .into();
        if let Some(refresh) = token["refresh_token"].as_str() {
            value["refresh_token"] = refresh.into();
        }
        write_private(&home.join("auth.json"), &value)?;
    }
    let token = value["access_token"]
        .as_str()
        .ok_or("Google did not return a valid access token.")?;
    let user = json_response(
        client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(token),
    )?;
    let email = string(&user, &["email"]).ok_or("Google could not verify the account email.")?;
    if value["email"].as_str().is_some_and(|old| old != email) {
        return Err("Google returned a different account. Sign in again.".into());
    }
    let identity =
        string(&user, &["id"]).ok_or("Google could not verify this account's identity.")?;
    let data = json_response(client.post("https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist").bearer_auth(token)
        .json(&json!({"metadata":{"ideType":"ANTIGRAVITY","platform":"PLATFORM_UNSPECIFIED","pluginType":"GEMINI"}})))?;
    let subscription = paid_subscription(&data)?;
    value["email"] = email.clone().into();
    value["project_id"] = data["cloudaicompanionProject"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| string(&data["cloudaicompanionProject"], &["id", "projectId"]))
        .map(Value::String)
        .unwrap_or(value["project_id"].clone());
    write_private(&home.join("auth.json"), &value)?;
    Ok((identity, Some(email), subscription))
}
fn paid_subscription(data: &Value) -> Result<Subscription, String> {
    let tier = &data["paidTier"];
    let plan = string(tier,&["name","id"]).or_else(|| (data["currentTier"]["id"] == "standard-tier").then(|| "Paid".into()))
        .ok_or("A paid Antigravity plan could not be confirmed. Check your subscription and try again.")?;
    if plan.to_ascii_lowercase().contains("free") || plan.to_ascii_lowercase().contains("trial") {
        return Err("A paid Antigravity plan is required.".into());
    }
    Ok(Subscription {
        id: "antigravity".into(),
        service: "Antigravity".into(),
        plan: Some(plan),
        status: Some("active".into()),
        expires_at: None,
        monthly_price_usd: None,
    })
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Saved {
    pub id: String,
    pub name: String,
    pub home: PathBuf,
    pub email: Option<String>,
    pub subscription: Option<Subscription>,
}
fn index() -> PathBuf {
    paths::opencode_user_data().join("keyturn-antigravity-profiles/profiles.json")
}
pub fn list() -> Result<Vec<Saved>, String> {
    with_repository(list_unlocked)
}
fn list_unlocked() -> Result<Vec<Saved>, String> {
    match std::fs::read(index()) {
        Ok(raw) => serde_json::from_slice(&raw)
            .map_err(|_| "Cannot read saved Antigravity accounts.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(_) => Err("Cannot read Antigravity accounts.".into()),
    }
}
pub fn save(name: &str, source: &Path) -> Result<(String, bool), String> {
    with_repository(|| {
        let mut saved = list_unlocked()?;
        let (_, email, subscription) = inspect_unlocked(source)?;
        let credentials = credentials(source)?;
        for entry in &mut saved {
            if paths::read_json(&entry.home.join("auth.json")).and_then(|v| string(&v, &["email"]))
                == string(&credentials, &["email"])
            {
                write_private(&entry.home.join("auth.json"), &credentials)?;
                entry.email = email;
                entry.subscription = Some(subscription);
                let id = entry.id.clone();
                write_private(&index(), &serde_json::to_value(&saved).unwrap())?;
                return Ok((id, false));
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let home = index().parent().unwrap().join(&id);
        private_dir(&home)?;
        write_private(&home.join("auth.json"), &credentials)?;
        saved.push(Saved {
            id: id.clone(),
            name: name.into(),
            home: home.clone(),
            email,
            subscription: Some(subscription),
        });
        if let Err(error) = write_private(&index(), &serde_json::to_value(saved).unwrap()) {
            let _ = std::fs::remove_dir_all(home);
            return Err(error);
        }
        Ok((id, true))
    })
}
/// Rename saved metadata without reading or changing OAuth credentials.
pub fn rename(id: &str, name: &str) -> Result<(), String> {
    with_repository(|| rename_in(&index(), id, name))
}

fn rename_in(index_path: &Path, id: &str, raw_name: &str) -> Result<(), String> {
    let name = raw_name.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err("Use an account name between 1 and 64 characters without control characters.".into());
    }
    let raw = std::fs::read(index_path).map_err(|_| "Cannot read saved Antigravity accounts.")?;
    let mut saved: Vec<Saved> = serde_json::from_slice(&raw)
        .map_err(|_| "Cannot read saved Antigravity accounts.")?;
    let target = saved.iter_mut().find(|p| p.id == id).ok_or("This account no longer exists.")?;
    target.name = name.into();
    write_private(index_path, &serde_json::to_value(saved).map_err(|_| "Cannot encode Antigravity accounts.")?)
}

pub fn remove(id: &str) -> Result<(), String> {
    with_repository(|| {
        let mut saved = list_unlocked()?;
        let home = saved.iter().find(|p| p.id == id).map(|p| p.home.clone());
        saved.retain(|p| p.id != id);
        write_private(&index(), &serde_json::to_value(saved).unwrap())?;
        if let Some(home) = home {
            let _ = std::fs::remove_dir_all(home);
        }
        Ok(())
    })
}
pub fn quota(id: &str) -> Result<crate::quota::QuotaSet, String> {
    with_repository(|| {
        let saved = list_unlocked()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or("This account no longer exists.")?;
        quota_in_unlocked(&saved.home)
    })
}

/// Validate usage in a staged home before account enrollment.
pub(crate) fn quota_in(home: &Path) -> Result<crate::quota::QuotaSet, String> {
    with_repository(|| quota_in_unlocked(home))
}

fn quota_in_unlocked(home: &Path) -> Result<crate::quota::QuotaSet, String> {
        inspect_unlocked(home)?;
        let credentials = credentials(home)?;
        let data = json_response(
            http()?
                .post(
                    "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
                )
                .bearer_auth(credentials["access_token"].as_str().unwrap())
                .json(&json!({"project":credentials["project_id"]})),
        )?;
        let mut windows = vec![];
        for group in data["groups"].as_array().into_iter().flatten() {
            for bucket in group["buckets"].as_array().into_iter().flatten() {
                if let Some(fraction) = bucket["remaining"]["remainingFraction"]
                    .as_f64()
                    .filter(|f| f.is_finite() && (0.0..=1.0).contains(f))
                {
                    windows.push(crate::quota::QuotaWindow {
                        window: string(bucket, &["displayName", "bucketId"])
                            .unwrap_or_else(|| "Usage".into()),
                        percent: (1.0 - fraction) * 100.0,
                        resets_at: string(bucket, &["resetTime", "resetsAt"]),
                        status: None,
                    });
                }
            }
        }
        if windows.is_empty() {
            return Err("Google did not return measured quota for this account.".into());
        }
        Ok(crate::quota::QuotaSet {
            provider: "antigravity".into(),
            windows: Some(windows),
            error: None,
            reset_credits: None, as_of: Some(chrono::Utc::now().to_rfc3339()),
        })
}

pub struct OAuthLogin {
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Progress>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl OAuthLogin {
    pub fn start(home: &Path) -> Result<Self, String> {
        Self::start_with_client(home, resolve_client()?)
    }
    fn start_with_client(home: &Path, client: Client) -> Result<Self, String> {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|_| "Cannot open Google sign-in callback.")?;
        listener
            .set_nonblocking(true)
            .map_err(|_| "Cannot start Google sign-in.")?;
        let redirect = format!(
            "http://127.0.0.1:{}/callback",
            listener
                .local_addr()
                .map_err(|_| "Cannot start Google sign-in.")?
                .port()
        );
        let state = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let mut url = reqwest::Url::parse(AUTH).unwrap();
        url.query_pairs_mut().extend_pairs([("client_id",client.id.as_str()),("redirect_uri",redirect.as_str()),("response_type","code"),("scope","https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email"),("access_type","offline"),("prompt","select_account consent"),("state",state.as_str())]);
        let progress = Arc::new(Mutex::new(Progress {
            status: "pending".into(),
            auth_url: Some(url.into()),
            ..Default::default()
        }));
        let cancel = Arc::new(AtomicBool::new(false));
        let output = progress.clone();
        let stopped = cancel.clone();
        let home = home.to_path_buf();
        let worker = std::thread::spawn(move || {
            let result = (|| {
                let deadline = Instant::now() + Duration::from_secs(600);
                loop {
                    if stopped.load(Ordering::SeqCst) {
                        return Err("Sign-in cancelled.".to_string());
                    }
                    if Instant::now() >= deadline {
                        return Err("Google sign-in timed out. Try again.".into());
                    }
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
                            let mut bytes = [0u8; 8192];
                            let count = stream
                                .read(&mut bytes)
                                .map_err(|_| "Invalid sign-in callback.")?;
                            let text = String::from_utf8_lossy(&bytes[..count]);
                            let path = text
                                .lines()
                                .next()
                                .and_then(|line| line.strip_prefix("GET "))
                                .and_then(|line| line.split_whitespace().next())
                                .unwrap_or("");
                            let callback = reqwest::Url::parse(&format!("http://localhost{path}"))
                                .map_err(|_| "Invalid callback.")?;
                            let params: std::collections::HashMap<_, _> =
                                callback.query_pairs().into_owned().collect();
                            if callback.path() != "/callback" || params.get("state") != Some(&state)
                            {
                                let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                                continue;
                            }
                            let body = "Return to Keyturn to finish adding your account. You can close this tab.";
                            let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
                            if params.contains_key("error") {
                                return Err(
                                    "Google sign-in was not approved. Try again when ready.".into(),
                                );
                            }
                            let code = params
                                .get("code")
                                .ok_or("Google did not return an authorization code.")?;
                            let token = json_response(http()?.post(TOKEN).form(&[
                                ("code", code.as_str()),
                                ("client_id", client.id.as_str()),
                                ("client_secret", client.secret.as_str()),
                                ("redirect_uri", redirect.as_str()),
                                ("grant_type", "authorization_code"),
                            ]))?;
                            if stopped.load(Ordering::SeqCst) {
                                return Err("Sign-in cancelled.".into());
                            }
                            write_private(
                                &home.join("auth.json"),
                                &json!({"access_token":token["access_token"],"refresh_token":token["refresh_token"],"expiry_date":chrono::Utc::now().timestamp_millis()+token["expires_in"].as_i64().unwrap_or(3600)*1000,"client_id":client.id,"client_secret":client.secret}),
                            )?;
                            return Ok(());
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(50))
                        }
                        Err(_) => return Err("Google callback stopped. Retry sign-in.".into()),
                    }
                }
            })();
            if let Ok(mut progress) = output.lock() {
                progress.status = if result.is_ok() {
                    "completed"
                } else {
                    "failed"
                }
                .into();
                progress.error = result.err();
            }
        });
        Ok(Self {
            cancel,
            progress,
            worker: Some(worker),
        })
    }
    pub fn poll(&mut self) -> Progress {
        self.progress.lock().map(|p| p.clone()).unwrap_or_default()
    }
    pub fn stop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for OAuthLogin {
    fn drop(&mut self) {
        self.stop();
    }
}

fn resolve_client() -> Result<Client, String> {
    if let (Ok(id), Ok(secret)) = (
        std::env::var("ANTIGRAVITY_OAUTH_CLIENT_ID"),
        std::env::var("ANTIGRAVITY_OAUTH_CLIENT_SECRET"),
    ) {
        if !id.is_empty() && !secret.is_empty() {
            return Ok(Client { id, secret });
        }
    }
    let mut candidates = vec![];
    if let Some(binary) = crate::cli::which_binary("agy") {
        candidates.push(binary);
    }
    for root in [
        PathBuf::from("/Applications"),
        dirs::home_dir().unwrap_or_default().join("Applications"),
    ] {
        for suffix in [
            "Contents/MacOS/Gemini",
            "Contents/Resources/app/extensions/antigravity/bin/language_server_macos_arm",
            "Contents/Resources/app/extensions/antigravity/bin/language_server_macos_x64",
            "Contents/Resources/app/out/main.js",
        ] {
            candidates.push(root.join("Antigravity.app").join(suffix));
        }
    }
    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            if let Some(client) = artifact_client(&data) {
                return Ok(client);
            }
        }
    }
    Err("Antigravity's OAuth client could not be identified. Update Antigravity/agy or configure a matching ANTIGRAVITY_OAUTH_CLIENT_ID and ANTIGRAVITY_OAUTH_CLIENT_SECRET pair.".into())
}
fn artifact_client(data: &[u8]) -> Option<Client> {
    // Paired Go-string initialization records, not neighboring strings in the pool.
    // Algorithm independently implemented from the CodexBar artifact format research.
    let word = |offset: usize| {
        data.get(offset..offset + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    if word(0)? != 0xfeedfacf {
        return None;
    }
    let intel = word(4)? == 0x01000007;
    if !intel && word(4)? != 0x0100000c {
        return None;
    }
    let commands_end = 32usize.checked_add(word(20)? as usize)?;
    let mut command = 32;
    let mut found = vec![];
    for _ in 0..word(16)?.min(4096) {
        let size = word(command + 4)? as usize;
        if size < 8 || command.checked_add(size)? > commands_end {
            return None;
        }
        if word(command) == Some(0x19)
            && size >= 72
            && data.get(command + 8..command + 15) == Some(b"__TEXT\0")
            && word(command + 40) == Some(0)
            && word(command + 44) == Some(0)
        {
            let text_end = word(command + 48)? as usize;
            if text_end > data.len() {
                return None;
            }
            for section in 0..word(command + 64)? as usize {
                let section = command + 72 + section * 80;
                if section + 80 > command + size {
                    return None;
                }
                if data.get(section..section + 7) != Some(b"__text\0") {
                    continue;
                }
                let start = word(section + 48)? as usize;
                let length = word(section + 40)? as usize;
                if start.checked_add(length)? > text_end {
                    return None;
                }
                let marker: &[u8] = if intel {
                    &[0x48, 0xc7, 0x40, 0x08]
                } else {
                    &[1, 8, 0, 0xa9]
                };
                for pos in start..start + length.saturating_sub(37) {
                    if data.get(pos..pos + 4) != Some(marker) {
                        continue;
                    }
                    let record = if intel {
                        pos
                    } else {
                        match pos.checked_sub(12) {
                            Some(r) => r,
                            None => continue,
                        }
                    };
                    let reference = |offset: usize| -> Option<(usize, usize)> {
                        if intel {
                            if word(offset)? & 0x00ffffff != 0x000d8d48 {
                                return None;
                            }
                            let target = (offset as i64) + 7 + (word(offset + 3)? as i32 as i64);
                            return Some((usize::try_from(target).ok()?, 0));
                        }
                        let page = word(offset)?;
                        let add = word(offset + 4)?;
                        let count = word(offset + 8)?;
                        if offset % 4 != 0
                            || page & 0x9f00001f != 0x90000001
                            || add & 0xffc003ff != 0x91000021
                            || count & 0xffe0001f != 0xd2800002
                        {
                            return None;
                        }
                        let immediate = ((page >> 5) & 0x7ffff) << 2 | ((page >> 29) & 3);
                        let pages = ((immediate << 11) as i32 >> 11) as i64;
                        let target =
                            ((offset & !4095) as i64) + pages * 4096 + ((add >> 10) & 4095) as i64;
                        Some((
                            usize::try_from(target).ok()?,
                            ((count >> 5) & 0xffff) as usize,
                        ))
                    };
                    let pair = if intel {
                        if data.get(record + 15..record + 22)
                            != Some(&[0x48, 0x89, 8, 0x48, 0xc7, 0x40, 0x18])
                            || word(record + 33) != Some(0x10488948)
                        {
                            continue;
                        }
                        reference(record + 8)
                            .zip(reference(record + 26))
                            .map(|(a, b)| {
                                (
                                    (a.0, word(record + 4).unwrap_or(0) as usize),
                                    (b.0, word(record + 22).unwrap_or(0) as usize),
                                )
                            })
                    } else {
                        if word(record + 28) != Some(0xa9010801) {
                            continue;
                        }
                        reference(record).zip(reference(record + 16))
                    };
                    let Some((id, secret)) = pair else {
                        continue;
                    };
                    if !(30..=256).contains(&id.1) || secret.1 != 35 {
                        continue;
                    }
                    let Some(id_bytes) = data.get(id.0..id.0 + id.1) else {
                        continue;
                    };
                    let Some(secret_bytes) = data.get(secret.0..secret.0 + secret.1) else {
                        continue;
                    };
                    let Ok(id) = std::str::from_utf8(id_bytes) else {
                        continue;
                    };
                    let Ok(secret) = std::str::from_utf8(secret_bytes) else {
                        continue;
                    };
                    if !id.ends_with(".apps.googleusercontent.com")
                        || !secret.starts_with("GOCSPX-")
                    {
                        continue;
                    }
                    if id == CONSUMER {
                        return Some(Client {
                            id: id.into(),
                            secret: secret.into(),
                        });
                    }
                    found.push(Client {
                        id: id.into(),
                        secret: secret.into(),
                    });
                }
            }
        }
        command += size;
    }
    (found.len() == 1).then(|| found.remove(0))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rename_persists_only_the_selected_name_and_rejects_invalid_input() {
        let dir = std::env::temp_dir().join(format!("keyturn-agy-rename-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profiles.json");
        let original = json!([
            {"id":"one","name":"Personal","home":dir.join("one"),"email":"one@example.com","subscription":null},
            {"id":"two","name":"Work","home":dir.join("two"),"email":"two@example.com","subscription":null}
        ]);
        write_private(&path, &original).unwrap();
        rename_in(&path, "two", "  Office  ").unwrap();
        let updated: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let mut expected = original;
        expected[1]["name"] = json!("Office");
        assert_eq!(updated, expected);
        for name in ["", "  ", "bad\nname", &"x".repeat(65)] {
            assert!(rename_in(&path, "two", name).is_err());
        }
        assert!(rename_in(&path, "missing", "Name").is_err());
        let unchanged: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(unchanged, expected);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unknown_free_and_quota_only_are_not_paid() {
        for data in [
            json!({}),
            json!({"paidTier":{"name":"Free"}}),
            json!({"quota":100}),
            json!({"currentTier":{"id":"legacy-tier"}}),
        ] {
            assert!(paid_subscription(&data).is_err());
        }
        assert!(paid_subscription(
            &json!({"paidTier":{"id":"google-ai-pro","name":"Google AI Pro"}})
        )
        .is_ok());
    }
    #[test]
    fn paired_artifact_records_are_used_instead_of_string_pool_order() {
        let mut data = vec![0u8; 2048];
        fn put(data: &mut [u8], at: usize, value: u32) {
            data[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        put(&mut data, 0, 0xfeedfacf);
        put(&mut data, 4, 0x0100000c);
        put(&mut data, 16, 1);
        put(&mut data, 20, 152);
        put(&mut data, 32, 0x19);
        put(&mut data, 36, 152);
        data[40..47].copy_from_slice(b"__TEXT\0");
        put(&mut data, 80, 2048);
        put(&mut data, 96, 1);
        data[104..111].copy_from_slice(b"__text\0");
        put(&mut data, 144, 80);
        put(&mut data, 152, 256);
        put(&mut data, 256, 0x90000001);
        put(&mut data, 260, 0x91000021 | (1024 << 10));
        put(&mut data, 264, 0xd2800002 | ((CONSUMER.len() as u32) << 5));
        put(&mut data, 268, 0xa9000801);
        put(&mut data, 272, 0x90000001);
        put(&mut data, 276, 0x91000021 | (1200 << 10));
        put(&mut data, 280, 0xd2800002 | (35 << 5));
        put(&mut data, 284, 0xa9010801);
        data[1024..1024 + CONSUMER.len()].copy_from_slice(CONSUMER.as_bytes());
        let secret = format!("GOCSPX-{}", "A".repeat(28));
        data[1200..1235].copy_from_slice(secret.as_bytes());
        let client = artifact_client(&data).unwrap();
        assert_eq!(client.id, CONSUMER);
        assert!(client.secret == secret);
        put(&mut data, 284, 0);
        assert!(artifact_client(&data).is_none());
    }
    #[test]
    fn oauth_rejects_wrong_state_and_denial_does_not_save_credentials() {
        let home =
            std::env::temp_dir().join(format!("keyturn-google-test-{}", uuid::Uuid::new_v4()));
        private_dir(&home).unwrap();
        let mut login = OAuthLogin::start_with_client(
            &home,
            Client {
                id: CONSUMER.into(),
                secret: "synthetic".into(),
            },
        )
        .unwrap();
        let auth = reqwest::Url::parse(login.poll().auth_url.as_deref().unwrap()).unwrap();
        let params: std::collections::HashMap<_, _> = auth.query_pairs().into_owned().collect();
        assert!(!params.contains_key("client_secret"));
        let mut wrong = reqwest::Url::parse(&params["redirect_uri"]).unwrap();
        wrong
            .query_pairs_mut()
            .extend_pairs([("state", "wrong"), ("error", "access_denied")]);
        assert_eq!(
            reqwest::blocking::get(wrong).unwrap().status().as_u16(),
            400
        );
        let mut denied = reqwest::Url::parse(&params["redirect_uri"]).unwrap();
        denied.query_pairs_mut().extend_pairs([
            ("state", params["state"].as_str()),
            ("error", "access_denied"),
        ]);
        assert!(reqwest::blocking::get(denied)
            .unwrap()
            .status()
            .is_success());
        let deadline = Instant::now() + Duration::from_secs(2);
        while login.poll().status == "pending" && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(login.poll().status, "failed");
        login.stop();
        assert!(!home.join("auth.json").exists());
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn malformed_artifacts_fail_closed() {
        for size in 0..80 {
            assert!(artifact_client(&vec![0; size]).is_none());
        }
    }
}

/// Only called by an explicit Use current AGY login action; never alters its Keychain.
pub fn copy_current(home: &Path) -> Result<(), String> {
    let source = dirs::home_dir()
        .ok_or("Cannot locate your home directory.")?
        .join(".gemini/antigravity-cli/antigravity-oauth-token");
    if let Ok(metadata) = std::fs::symlink_metadata(&source) {
        if metadata.is_file() && metadata.len() <= 256 * 1024 {
            let raw = std::fs::read(source).map_err(|_| "Cannot read the current AGY login.")?;
            let value: Value = serde_json::from_slice(&raw)
                .map_err(|_| "The current AGY login has an unsupported format.")?;
            return write_private(&home.join("auth.json"), &value);
        }
    }
    #[cfg(target_os = "macos")]
    {
        use base64::Engine;
        let mut child = std::process::Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                "gemini",
                "-a",
                "antigravity",
                "-w",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|_| "Cannot read the current AGY login.")?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match child.try_wait() {
                Ok(Some(exit)) if exit.success() => {
                    let mut raw = vec![];
                    child
                        .stdout
                        .take()
                        .ok_or("Cannot read AGY login.")?
                        .take(256 * 1024)
                        .read_to_end(&mut raw)
                        .map_err(|_| "Cannot read AGY login.")?;
                    let text =
                        String::from_utf8(raw).map_err(|_| "Unsupported AGY login format.")?;
                    let bytes =
                        if let Some(encoded) = text.trim().strip_prefix("go-keyring-base64:") {
                            base64::engine::general_purpose::STANDARD
                                .decode(encoded)
                                .map_err(|_| "Unsupported AGY login format.")?
                        } else {
                            text.into_bytes()
                        };
                    let value: Value = serde_json::from_slice(&bytes)
                        .map_err(|_| "Unsupported AGY login format.")?;
                    return write_private(&home.join("auth.json"), &value);
                }
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(30))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
            }
        }
    }
    Err(
        "No current AGY login could be read. Use Sign in with Google or import a credentials file."
            .into(),
    )
}
