//! Named Codex logins. Credentials stay in owner-only files, never in IPC.
//! A profile's isolated CODEX_HOME lets the CLI own browser login and refresh.
//! Activation replaces only the active auth cache, preserving configuration.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use crate::paths;

static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone, Deserialize, Serialize)]
struct Profile {
    id: String,
    name: String,
    #[serde(default)]
    created_at: Option<String>,
    /// Explicit file replacements win over the old active-account snapshot.
    #[serde(default)]
    needs_activation: bool,
}

#[derive(Default, Deserialize, Serialize)]
struct Index {
    #[serde(default)]
    profiles: Vec<Profile>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub created_at: Option<String>,
    pub email: Option<String>,
    pub plan: Option<String>,
    pub subscription: Option<crate::subscriptions::Subscription>,
    pub connected: bool,
    pub connection_error: Option<String>,
    pub active: bool,
    pub needs_activation: bool,
    pub login_command: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesState {
    pub profiles: Vec<ProfileView>,
    pub active_profile_id: Option<String>,
    pub can_import: bool,
    pub switch_error: Option<String>,
}

/// Identity includes subject and workspace; email is display metadata only.
#[derive(PartialEq, Eq)]
struct Identity {
    subject: String,
    account: String,
}

struct Login {
    raw: Value,
    identity: Identity,
    email: Option<String>,
    plan: Option<String>,
    subscription: crate::subscriptions::Subscription,
}

fn nonempty(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

fn parse_login(raw: Value) -> Result<Login, String> {
    if nonempty(raw.get("OPENAI_API_KEY")).is_some() {
        return Err("Profiles require ChatGPT login rather than an API key.".into());
    }
    let tokens = raw
        .get("tokens")
        .ok_or("Sign in to Codex with ChatGPT first.")?;
    let id = nonempty(tokens.get("id_token"))
        .ok_or("This ChatGPT login is incomplete. Sign in again.")?;
    if nonempty(tokens.get("access_token")).is_none()
        || nonempty(tokens.get("refresh_token")).is_none()
    {
        return Err("This ChatGPT login is incomplete. Sign in again.".into());
    }
    let claims = id
        .split('.')
        .nth(1)
        .and_then(|p| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(p)
                .ok()
        })
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .ok_or("Cannot identify this ChatGPT login. Sign in again.")?;
    let subject = nonempty(claims.get("sub"))
        .ok_or("Cannot identify this ChatGPT login. Sign in again.")?
        .to_owned();
    let auth = claims.get("https://api.openai.com/auth");
    let account = nonempty(tokens.get("account_id"))
        .or_else(|| nonempty(auth.and_then(|a| a.get("chatgpt_account_id"))))
        .ok_or("Cannot identify this ChatGPT workspace. Sign in again.")?
        .to_owned();
    let email = nonempty(claims.get("email"))
        .or_else(|| {
            nonempty(
                claims
                    .get("https://api.openai.com/profile")
                    .and_then(|a| a.get("email")),
            )
        })
        .map(str::to_owned);
    let plan = nonempty(auth.and_then(|a| a.get("chatgpt_plan_type"))).map(str::to_owned);
    let subscription = crate::subscriptions::chatgpt(&claims);
    Ok(Login {
        raw,
        identity: Identity { subject, account },
        email,
        plan,
        subscription,
    })
}

fn read_login(path: &Path) -> Result<Option<Login>, String> {
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Cannot read Codex credentials. Check file permissions.".into()),
    };
    // Never put parser input or tokens into an error sent to the webview.
    let value = serde_json::from_slice(&raw)
        .map_err(|_| "Codex credentials are invalid. Sign in again.")?;
    parse_login(value).map(Some)
}

pub(crate) fn private_dir(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Profile directories must not be symbolic links.".into());
    }
    fs::create_dir_all(path).map_err(|_| "Cannot create the profile directory.")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect the profile directory.")?;
    }
    Ok(())
}

/// Create the temporary file with private permissions before writing secrets.
pub(crate) fn write_private(path: &Path, value: &Value) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Credential and profile files must not be symbolic links.".into());
    }
    let parent = path.parent().ok_or("Invalid credential directory.")?;
    fs::create_dir_all(parent).map_err(|_| "Cannot create the credential directory.")?;
    let temp = parent.join(format!(".keyturn-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .map_err(|_| "Cannot create a private credential file.")?;
        let body = serde_json::to_vec_pretty(value).map_err(|_| "Cannot encode profile data.")?;
        file.write_all(&body)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Cannot save profile data.")?;
        fs::rename(&temp, path).map_err(|_| "Cannot replace profile data.")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 64 || trimmed.chars().any(char::is_control) {
        return Err("Use a profile name with 1–64 characters.".into());
    }
    Ok(trimmed.to_owned())
}

fn quote_shell(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', "'\\''"))
}

/// Bound paths are explicit so filesystem tests cannot touch the user's login.
struct Repository {
    root: PathBuf,
    codex_home: PathBuf,
}

impl Repository {
    fn system() -> Self {
        Self {
            root: paths::opencode_user_data().join("keyturn-codex-profiles"),
            codex_home: paths::codex_home(),
        }
    }

    fn index(&self) -> Result<Index, String> {
        if fs::symlink_metadata(&self.root).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Profile directories must not be symbolic links.".into());
        }
        let raw = match fs::read(self.root.join("profiles.json")) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Index::default()),
            Err(_) => return Err("Cannot read saved profiles.".into()),
        };
        let index: Index = serde_json::from_slice(&raw)
            .map_err(|_| "Saved profiles are invalid. Restore your profile backup.")?;
        for profile in &index.profiles {
            self.home(&profile.id)?;
        }
        Ok(index)
    }

    fn save(&self, index: &Index) -> Result<(), String> {
        private_dir(&self.root)?;
        write_private(
            &self.root.join("profiles.json"),
            &serde_json::to_value(index).map_err(|_| "Cannot encode profiles.")?,
        )
    }

    fn home(&self, id: &str) -> Result<PathBuf, String> {
        let parsed = uuid::Uuid::parse_str(id).map_err(|_| "Invalid profile ID.")?;
        if parsed.to_string() != id {
            return Err("Invalid profile ID.".into());
        }
        let path = self.root.join(id);
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Profile directories must not be symbolic links.".into());
        }
        Ok(path)
    }

    fn switch_error(&self) -> Option<String> {
        let config = match fs::read_to_string(self.codex_home.join("config.toml")) {
            Ok(config) => match toml::from_str::<toml::Value>(&config) {
                Ok(config) => config,
                Err(_) => return Some(
                    "Cannot read Codex configuration. Fix config.toml before switching accounts."
                        .into(),
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                toml::Value::Table(Default::default())
            }
            Err(_) => {
                return Some("Cannot read Codex configuration. Check file permissions.".into())
            }
        };
        match config.get("cli_auth_credentials_store").and_then(toml::Value::as_str).unwrap_or("file") {
            "file" => {},
            _ => return Some("Account switching currently requires Codex file-based login storage. Set cli_auth_credentials_store = \"file\" in Codex settings and sign in again.".into()),
        }
        if config
            .get("forced_login_method")
            .and_then(toml::Value::as_str)
            .is_some_and(|s| s != "chatgpt")
        {
            return Some("Codex is configured to require API-key login. ChatGPT profiles cannot be activated.".into());
        }
        None
    }

    fn current(&self) -> Result<Option<Login>, String> {
        read_login(&self.codex_home.join("auth.json"))
    }

    fn login(&self, profile: &Profile) -> Result<Option<Login>, String> {
        read_login(&self.home(&profile.id)?.join("auth.json"))
    }

    fn sync_current(&self, index: &Index, current: Option<&Login>) -> Result<(), String> {
        if let Some(current) = current {
            for profile in &index.profiles {
                if profile.needs_activation {
                    continue;
                }
                if self.login(profile).ok().flatten().is_some_and(|saved| {
                    saved.identity == current.identity && saved.raw != current.raw
                }) {
                    private_dir(&self.home(&profile.id)?)?;
                    write_private(&self.home(&profile.id)?.join("auth.json"), &current.raw)?;
                }
            }
        }
        Ok(())
    }

    fn state(&self) -> Result<ProfilesState, String> {
        let index = self.index()?;
        let switch_error = self.switch_error();
        let current = if switch_error.is_none() {
            self.current().ok().flatten()
        } else {
            None
        };
        // A fresh CLI token rotation wins over a saved snapshot of that account.
        self.sync_current(&index, current.as_ref())?;
        let mut profiles = Vec::new();
        let mut identities: Vec<(Identity, String)> = Vec::new();
        for p in &index.profiles {
            let login = self.login(p).ok().flatten();
            let connection_error = login.as_ref().and_then(|login| identities.iter().find(|(identity, _)| *identity == login.identity))
                .map(|(_, name)| format!("This account is already saved as {name}. Sign in with a different account."));
            let connected = login.is_some() && connection_error.is_none();
            let active = connected
                && !p.needs_activation
                && login
                    .as_ref()
                    .zip(current.as_ref())
                    .is_some_and(|(a, b)| a.identity == b.identity);
            profiles.push(ProfileView {
                id: p.id.clone(),
                name: p.name.clone(),
                display_name: login.as_ref().and_then(|l| crate::codex::profile_display_name(&l.raw)),
                created_at: p.created_at.clone(),
                email: login.as_ref().and_then(|l| l.email.clone()),
                plan: login.as_ref().and_then(|l| l.plan.clone()),
                subscription: login.as_ref().map(|l| l.subscription.clone()),
                connected,
                connection_error,
                active,
                needs_activation: p.needs_activation,
                login_command: format!(
                    "CODEX_HOME={} codex -c 'cli_auth_credentials_store=\"file\"' login",
                    quote_shell(&self.home(&p.id)?.to_string_lossy())
                ),
            });
            if let Some(login) = login {
                identities.push((login.identity, p.name.clone()));
            }
        }
        let active_profile_id = profiles.iter().find(|p| p.active).map(|p| p.id.clone());
        Ok(ProfilesState {
            profiles,
            active_profile_id,
            can_import: current.is_some(),
            switch_error,
        })
    }

    fn unique_name(
        &self,
        index: &Index,
        raw: &str,
        except: Option<&str>,
    ) -> Result<String, String> {
        let name = if raw.trim().is_empty() && except.is_none() {
            let adjectives = ["Curious", "Quiet", "Swift", "Bright", "Cosmic", "Gentle", "Lucky", "Nimble"];
            let animals = ["Otter", "Axolotl", "Narwhal", "Fox", "Panda", "Owl", "Lynx", "Finch"];
            let bytes = *uuid::Uuid::new_v4().as_bytes();
            format!("{} {} {}", adjectives[bytes[0] as usize % 8], animals[bytes[1] as usize % 8], u16::from_le_bytes([bytes[2], bytes[3]]))
        } else { name(raw)? };
        if index
            .profiles
            .iter()
            .any(|p| Some(p.id.as_str()) != except && p.name.to_lowercase() == name.to_lowercase())
        {
            return Err("A profile with this name already exists.".into());
        }
        Ok(name)
    }

    fn create(&self, raw_name: &str, import_current: bool) -> Result<ProfilesState, String> {
        let mut index = self.index()?;
        let name = self.unique_name(&index, raw_name, None)?;
        let login = if import_current {
            if let Some(error) = self.switch_error() {
                return Err(error);
            }
            let login = self
                .current()?
                .ok_or("Sign in to Codex with ChatGPT first.")?;
            for p in &index.profiles {
                if self
                    .login(p)
                    .ok()
                    .flatten()
                    .is_some_and(|l| l.identity == login.identity)
                {
                    return Err(format!("This account is already saved as {}.", p.name));
                }
            }
            Some(login)
        } else {
            None
        };
        let profile = Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            needs_activation: false,
        };
        private_dir(&self.root)?;
        let home = self.home(&profile.id)?;
        private_dir(&home)?;
        if let Some(login) = login {
            write_private(&home.join("auth.json"), &login.raw)?;
        }
        index.profiles.push(profile);
        self.save(&index)?;
        self.state()
    }

    fn import_file(
        &self,
        raw_name: &str,
        source: &Path,
        replace_id: Option<&str>,
    ) -> Result<ProfilesState, String> {
        let mut index = self.index()?;
        const MAX_AUTH_BYTES: u64 = 256 * 1024;
        let metadata =
            fs::metadata(source).map_err(|_| "Cannot read the selected auth.json file.")?;
        if !metadata.is_file() || metadata.len() > MAX_AUTH_BYTES {
            return Err("Choose an auth.json file smaller than 256 KB.".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(source)
            .map_err(|_| "Cannot open the selected auth.json file.")?
            .take(MAX_AUTH_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read the selected auth.json file.")?;
        if bytes.len() as u64 > MAX_AUTH_BYTES {
            return Err("Choose an auth.json file smaller than 256 KB.".into());
        }
        let raw = serde_json::from_slice(&bytes).map_err(|_| {
            "This file is not valid Codex auth.json. Choose the original login file."
        })?;
        let login = parse_login(raw)?;
        let inferred = if raw_name.trim().is_empty() {
            replace_id.and_then(|id| index.profiles.iter().find(|p| p.id == id)).map(|p| p.name.clone()).unwrap_or_else(|| crate::codex::imported_account_label(&login.raw))
        } else { raw_name.to_owned() };
        let mut label = inferred.clone();
        if raw_name.trim().is_empty() && replace_id.is_none() {
            let mut suffix = 2;
            while index.profiles.iter().any(|p| p.name.to_lowercase() == label.to_lowercase()) {
                label = format!("{} ({suffix})", inferred.chars().take(54).collect::<String>()); suffix += 1;
            }
        }
        let name = self.unique_name(&index, &label, replace_id)?;
        let target = if let Some(id) = replace_id {
            index
                .profiles
                .iter()
                .find(|p| p.id == id)
                .ok_or("Profile no longer exists.")?
                .clone()
        } else {
            Profile {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.clone(),
                created_at: Some(chrono::Utc::now().to_rfc3339()),
                needs_activation: true,
            }
        };

        for p in &index.profiles {
            if p.id != target.id
                && self
                    .login(p)
                    .ok()
                    .flatten()
                    .is_some_and(|l| l.identity == login.identity)
            {
                return Err(format!("This account is already saved as {}. Replace its login from that profile instead.", p.name));
            }
        }
        private_dir(&self.root)?;
        let home = self.home(&target.id)?;
        private_dir(&home)?;
        // Back up an existing saved login before replacing it; source is never changed.
        if let Some(previous) = self.login(&target).ok().flatten() {
            write_private(&home.join("previous-auth.json"), &previous.raw)?;
        }
        if let Some(p) = index.profiles.iter_mut().find(|p| p.id == target.id) {
            p.name = name;
            p.needs_activation = true;
        } else {
            index.profiles.push(target);
        }
        self.save(&index)?;
        write_private(&home.join("auth.json"), &login.raw)?;
        self.state()
    }

    fn rename(&self, id: &str, raw_name: &str) -> Result<ProfilesState, String> {
        let mut index = self.index()?;
        let name = self.unique_name(&index, raw_name, Some(id))?;
        index
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Profile no longer exists.")?
            .name = name;
        self.save(&index)?;
        self.state()
    }

    fn remove(&self, id: &str) -> Result<ProfilesState, String> {
        let mut index = self.index()?;
        if !index.profiles.iter().any(|p| p.id == id) {
            return Err("Profile no longer exists.".into());
        }
        // Remove from the index first. Deleting a profile never logs out the CLI.
        index.profiles.retain(|p| p.id != id);
        self.save(&index)?;
        let home = self.home(id)?;
        if home.exists() {
            fs::remove_dir_all(home)
                .map_err(|_| "Profile removed, but its saved login could not be deleted.")?;
        }
        self.state()
    }

    fn activate(&self, id: &str) -> Result<ProfilesState, String> {
        if let Some(error) = self.switch_error() {
            return Err(error);
        }
        let mut index = self.index()?;
        let profile = index
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or("Profile no longer exists.")?
            .clone();
        let current = self.current()?;
        self.sync_current(&index, current.as_ref())?;
        let target = self
            .login(&profile)?
            .ok_or("Sign in to this profile first, then check its connection.")?;
        if let Some(error) = self
            .state()?
            .profiles
            .iter()
            .find(|p| p.id == id)
            .and_then(|p| p.connection_error.clone())
        {
            return Err(error);
        }
        let config = fs::read_to_string(self.codex_home.join("config.toml")).unwrap_or_default();
        if toml::from_str::<toml::Value>(&config)
            .ok()
            .and_then(|c| {
                c.get("forced_chatgpt_workspace_id")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            })
            .is_some_and(|required| required != target.identity.account)
        {
            return Err(
                "This profile does not match the workspace required by Codex settings.".into(),
            );
        }
        if !profile.needs_activation
            && current
                .as_ref()
                .is_some_and(|c| c.identity == target.identity)
        {
            return self.state();
        }
        // Preserve an unregistered current account before replacing its cache.
        if let Some(current) = &current {
            let known = current.identity == target.identity
                || index.profiles.iter().any(|p| {
                    self.login(p)
                        .ok()
                        .flatten()
                        .is_some_and(|l| l.identity == current.identity)
                });
            if !known {
                let mut label = "Previous account".to_string();
                let mut suffix = 2;
                while self.unique_name(&index, &label, None).is_err() {
                    label = format!("Previous account {suffix}");
                    suffix += 1;
                }
                let previous = Profile {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: label,
                    created_at: Some(chrono::Utc::now().to_rfc3339()),
            needs_activation: false,
                };
                let home = self.home(&previous.id)?;
                private_dir(&home)?;
                write_private(&home.join("auth.json"), &current.raw)?;
                index.profiles.push(previous);
                self.save(&index)?;
            }
        }
        // Detect another CLI changing accounts while the backup was being saved.
        let latest = self.current()?;
        if latest.as_ref().map(|l| &l.raw) != current.as_ref().map(|l| &l.raw) {
            return Err("Codex login changed during the switch. Refresh and try again.".into());
        }
        if let Some(current) = &current {
            private_dir(&self.root)?;
            write_private(&self.root.join("previous-active-auth.json"), &current.raw)?;
        }
        if let Some(p) = index.profiles.iter_mut().find(|p| p.id == id) {
            p.needs_activation = false;
        }
        self.save(&index)?;
        if let Err(error) = write_private(&self.codex_home.join("auth.json"), &target.raw) {
            if let Some(p) = index.profiles.iter_mut().find(|p| p.id == id) {
                p.needs_activation = profile.needs_activation;
            }
            self.save(&index).map_err(|_| "The switch failed and profile metadata could not be restored. Your active Codex login was not changed.")?;
            return Err(error);
        }
        let state = self.state()?;
        if state.active_profile_id.as_deref() != Some(id) {
            return Err("Codex changed accounts after the switch. Reopen running Codex sessions and refresh.".into());
        }
        Ok(state)
    }
}

fn with_repository<T>(
    action: impl FnOnce(&Repository) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = LOCK
        .lock()
        .map_err(|_| "Profile storage is busy. Restart Keyturn.")?;
    action(&Repository::system())
}

/// Read profile metadata and reconcile the active account with Codex's cache.
pub fn list() -> Result<ProfilesState, String> {
    with_repository(Repository::state)
}
/// Create a login directory, optionally copying the current ChatGPT login.
pub fn create(name: &str, import_current: bool) -> Result<ProfilesState, String> {
    with_repository(|r| r.create(name, import_current))
}
/// Rename without changing identity, credentials or the active account.
pub fn rename(id: &str, name: &str) -> Result<ProfilesState, String> {
    with_repository(|r| r.rename(id, name))
}
/// Forget the saved profile while leaving the active CLI login intact.
pub fn remove(id: &str) -> Result<ProfilesState, String> {
    crate::profile_login::ensure_not_running(id)?;
    with_repository(|r| r.remove(id))
}
/// Preserve the previous account and atomically activate the selected login.
pub fn activate(id: &str) -> Result<ProfilesState, String> {
    crate::profile_login::ensure_not_running(id)?;
    with_repository(|r| r.activate(id))
}

/// Import a user-selected file without changing the active Codex credentials.
pub fn import_file(
    name: &str,
    source: &Path,
    replace_id: Option<&str>,
) -> Result<ProfilesState, String> {
    if let Some(id) = replace_id {
        crate::profile_login::ensure_not_running(id)?;
    }
    with_repository(|r| r.import_file(name, source, replace_id))
}

/// Serialize usage renewal with switching/import so rotated tokens cannot be overwritten.
pub fn quota_for(id: &str) -> Option<crate::quota::QuotaSet> {
    let raw = {
        let _guard = LOCK.lock().ok()?;
        let repo = Repository::system();
        repo.profile_login_for_quota(id).ok()?.raw
    };
    let quota = crate::quota::codex_quota_from_auth(&raw)?;
    if !needs_quota_renewal(&quota) { return Some(quota); }
    // Re-read after locking: another reader may already have renewed the session.
    let _guard = LOCK.lock().ok()?;
    Repository::system().profile_quota_with(id, crate::codex_refresh::refresh, crate::quota::codex_quota_from_auth)
}

fn needs_quota_renewal(quota: &crate::quota::QuotaSet) -> bool {
    quota.error.as_deref().is_some_and(|e| e.starts_with("session expired") || e.starts_with("session rejected"))
}

/// A saved JSON file is not proof of a live session. Check/renew the destination
/// before closing the desktop app, leaving the working active cache in place.
pub(crate) fn check_switch_login(id: &str, allow_limited: bool) -> Result<(), String> {
    let _guard = LOCK.lock().map_err(|_| "Profile storage is busy. Retry switching.")?;
    Repository::system().check_switch_login_with(
        id, allow_limited, crate::codex_refresh::refresh, crate::quota::codex_quota_from_auth,
    )
}

impl Repository {
    fn check_switch_login_with(
        &self,
        id: &str,
        allow_limited: bool,
        refresh: impl FnOnce(&Path) -> Result<(), String>,
        fetch: impl Fn(&Value) -> Option<crate::quota::QuotaSet>,
    ) -> Result<(), String> {
        let quota = self.profile_quota_with(id, refresh, fetch)
            .ok_or("Cannot verify the saved login. Reconnect this account before switching. Your current login has not changed.")?;
        if let Some(error) = quota.error {
            return Err(format!("Cannot switch accounts: {error}. Your current login has not changed."));
        }
        if !allow_limited && quota.windows.iter().flatten().any(|window| crate::settings::canonical_account_window(&window.window).is_some() && window.percent.is_finite() && window.percent >= 100.0) {
            return Err("This account has no remaining capacity. Choose another account or wait for its limits to reset. Your current login has not changed.".into());
        }
        Ok(())
    }

    fn profile_login_for_quota(&self, id: &str) -> Result<Login, String> {
        let index = self.index()?;
        let profile = index.profiles.iter().find(|p| p.id == id).ok_or("Profile no longer exists.")?;
        let current = self.current().ok().flatten();
        self.sync_current(&index, current.as_ref())?;
        self.login(profile)?.ok_or("No saved login for this account.".into())
    }

    fn profile_quota_with(
        &self,
        id: &str,
        refresh: impl FnOnce(&Path) -> Result<(), String>,
        fetch: impl Fn(&Value) -> Option<crate::quota::QuotaSet>,
    ) -> Option<crate::quota::QuotaSet> {
        let index = self.index().ok()?;
        let profile = index.profiles.iter().find(|p| p.id == id)?;
        let current = self.current().ok().flatten();
        self.sync_current(&index, current.as_ref()).ok()?;
        let login = self.login(profile).ok()??;
        let mut quota = fetch(&login.raw)?;
        if !needs_quota_renewal(&quota) {
            return Some(quota);
        }
        let active = !profile.needs_activation && current.as_ref().is_some_and(|c| c.identity == login.identity);
        let home = if active { self.codex_home.clone() } else { self.home(id).ok()? };
        let renewed = refresh(&home).and_then(|()| {
            let renewed = read_login(&home.join("auth.json"))?.ok_or("No renewed login was saved.")?;
            if renewed.identity != login.identity {
                return Err("Account changed during session renewal. Refresh and retry.".into());
            }
            if active { self.sync_current(&index, Some(&renewed))?; }
            Ok(renewed)
        });
        match renewed {
            Ok(renewed) => fetch(&renewed.raw),
            Err(error) => { quota.error = Some(error); Some(quota) }
        }
    }
}

/// Import a pasted login through the same native validator and private-file path.
pub fn import_json(name: &str, raw: &str, replace_id: Option<&str>) -> Result<ProfilesState, String> {
    if raw.len() > 256 * 1024 { return Err("Use an auth.json smaller than 256 KB.".into()); }
    let value: Value = serde_json::from_str(raw).map_err(|_| "This is not valid auth.json JSON.")?;
    parse_login(value.clone())?;
    // The temporary copy is private and removed even when import validation fails.
    let path = std::env::temp_dir().join(format!("keyturn-import-{}.json", uuid::Uuid::new_v4()));
    write_private(&path, &value)?;
    let result = import_file(name, &path, replace_id);
    let _ = fs::remove_file(path);
    result
}

/// Resolve an existing inactive profile for a CLI-owned browser sign-in.
pub(crate) fn login_home(id: &str) -> Result<PathBuf, String> {
    let _guard = LOCK.lock().map_err(|_| "Profile storage is busy.")?;
    let repository = Repository::system();
    let state = repository.state()?;
    let profile = state
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or("Profile no longer exists.")?;
    if profile.active {
        return Err("This profile is active. Add another profile to sign in separately.".into());
    }
    repository.home(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejected_pending_replacement_cannot_displace_a_valid_current_session() {
        let f = Fixture::new();
        f.current("personal", "valid-current");
        let state = f.repository.create("Personal", true).unwrap();
        let id = &state.profiles[0].id;
        // Reproduce a previously imported, now rejected replacement for the
        // same account. Pending replacements intentionally bypass sync_current.
        let mut index = f.repository.index().unwrap();
        index.profiles[0].needs_activation = true;
        f.repository.save(&index).unwrap();
        f.connect(id, "personal", "rejected-import");
        let before = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        let result = f.repository.check_switch_login_with(id, false,
            |home| {
                assert_eq!(home, f.repository.home(id).unwrap());
                Err("session could not be renewed — reconnect this account".into())
            },
            |raw| {
                assert_eq!(raw["tokens"]["access_token"], "rejected-import");
                Some(crate::quota::QuotaSet { provider: "codex".into(), windows: None,
                    error: Some("session rejected — sign in again".into()), as_of: None })
            });
        assert!(result.unwrap_err().contains("reconnect this account"));
        assert_eq!(fs::read(f.repository.codex_home.join("auth.json")).unwrap(), before);
        assert!(f.repository.index().unwrap().profiles[0].needs_activation);
    }

    #[test]
    fn switch_preflight_rejects_exhausted_destination_without_changing_current_login() {
        let f = Fixture::new(); f.current("personal", "valid-current");
        let state = f.repository.create("Work", false).unwrap(); let id = &state.profiles[0].id;
        f.connect(id, "work", "work-token");
        let before = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        for window in ["5-hour", "weekly"] {
            let result = f.repository.check_switch_login_with(id, false, |_| panic!("valid login needs no renewal"), |_| Some(crate::quota::QuotaSet { provider: "codex".into(), error: None, as_of: None, windows: Some(vec![crate::quota::QuotaWindow { window: window.into(), percent: 100.0, resets_at: None, status: None }]) }));
            assert!(result.unwrap_err().contains("no remaining capacity"));
            assert_eq!(fs::read(f.repository.codex_home.join("auth.json")).unwrap(), before);
        }
    }

    #[test]
    fn capacity_override_is_per_check_and_does_not_bypass_login_errors() {
        let f = Fixture::new(); f.current("personal", "valid-current");
        let state = f.repository.create("Work", false).unwrap(); let id = &state.profiles[0].id;
        f.connect(id, "work", "work-token");
        let before = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        let exhausted = |_: &Value| Some(crate::quota::QuotaSet { provider: "codex".into(), error: None, as_of: None, windows: Some(vec![crate::quota::QuotaWindow { window: "weekly".into(), percent: 100.0, resets_at: None, status: None }]) });
        assert!(f.repository.check_switch_login_with(id, true, |_| Ok(()), exhausted).is_ok());
        assert!(f.repository.check_switch_login_with(id, false, |_| Ok(()), exhausted).unwrap_err().contains("no remaining capacity"));
        let error = f.repository.check_switch_login_with(id, true, |_| Ok(()), |_| Some(crate::quota::QuotaSet { provider: "codex".into(), error: Some("usage unavailable".into()), as_of: None, windows: None }));
        assert!(error.unwrap_err().contains("usage unavailable"));
        assert_eq!(fs::read(f.repository.codex_home.join("auth.json")).unwrap(), before);
    }

    #[test]
    fn switch_preflight_uses_the_renewed_destination_without_activating_it() {
        let f = Fixture::new();
        f.current("personal", "valid-current");
        let state = f.repository.create("Work", false).unwrap();
        let id = &state.profiles[0].id;
        f.connect(id, "work", "expired");
        let before = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        f.repository.check_switch_login_with(id, false,
            |home| write_private(&home.join("auth.json"), &login("work", "workspace", "renewed")),
            |raw| Some(crate::quota::QuotaSet { provider: "codex".into(), windows: Some(vec![]),
                error: (raw["tokens"]["access_token"] != "renewed").then(|| "session expired".into()), as_of: None })
        ).unwrap();
        assert_eq!(fs::read(f.repository.codex_home.join("auth.json")).unwrap(), before);
        assert_eq!(f.repository.login(&f.repository.index().unwrap().profiles[0]).unwrap().unwrap().raw["tokens"]["access_token"], "renewed");
    }

    #[test]
    fn expired_saved_account_refreshes_without_activating_it() {
        let f = Fixture::new();
        f.current("personal", "personal-token");
        let state = f.repository.create("Work", false).unwrap();
        let id = &state.profiles[0].id;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"exp":1}"#);
        f.connect(id, "work", &format!("header.{payload}.signature"));
        let active_before = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        let result = f.repository.profile_quota_with(id, |home| {
            assert_eq!(home, f.repository.home(id).unwrap());
            write_private(&home.join("auth.json"), &login("work", "workspace", "renewed"))
        }, |raw| {
            if raw["tokens"]["access_token"] == "renewed" {
                Some(crate::quota::QuotaSet { provider: "codex".into(), windows: Some(vec![crate::quota::QuotaWindow { window: "Weekly".into(), percent: 12.0, resets_at: None, status: None }]), error: None, as_of: None })
            } else { crate::quota::codex_quota_from_auth(raw) }
        }).unwrap();
        assert!(result.windows.is_some(), "expired saved accounts must recover usage: {:?}", result.error);
        assert_eq!(fs::read(f.repository.codex_home.join("auth.json")).unwrap(), active_before);
        assert_eq!(f.repository.login(&f.repository.index().unwrap().profiles[0]).unwrap().unwrap().raw["tokens"]["access_token"], "renewed");
    }

    #[test]
    fn active_session_renewal_updates_the_cli_and_saved_snapshot() {
        let f = Fixture::new();
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"exp":1}"#);
        f.current("personal", &format!("header.{payload}.signature"));
        let state = f.repository.create("Personal", true).unwrap();
        let id = &state.profiles[0].id;
        let calls = std::cell::Cell::new(0);
        let result = f.repository.profile_quota_with(id, |home| {
            assert_eq!(home, f.repository.codex_home);
            f.current("personal", "renewed"); Ok(())
        }, |raw| {
            calls.set(calls.get() + 1);
            if raw["tokens"]["access_token"] == "renewed" {
                Some(crate::quota::QuotaSet { provider: "codex".into(), windows: Some(vec![]), error: None, as_of: None })
            } else { crate::quota::codex_quota_from_auth(raw) }
        }).unwrap();
        assert!(result.error.is_none());
        assert_eq!(calls.get(), 2);
        assert_eq!(f.repository.login(&f.repository.index().unwrap().profiles[0]).unwrap().unwrap().raw, f.repository.current().unwrap().unwrap().raw);
    }

    #[test]
    fn renewal_failure_preserves_credentials_and_non_auth_errors_do_not_renew() {
        let f = Fixture::new();
        f.current("personal", "current");
        let state = f.repository.create("Work", false).unwrap();
        let id = &state.profiles[0].id;
        f.connect(id, "work", "old");
        let saved = f.repository.home(id).unwrap().join("auth.json");
        let before = fs::read(&saved).unwrap();
        for error in ["session rejected — sign in again", "HTTP 403", "request timed out"] {
            let renewed = std::cell::Cell::new(false);
            let result = f.repository.profile_quota_with(id, |_| {
                renewed.set(true); Err("reconnect this account".into())
            }, |_| Some(crate::quota::QuotaSet { provider: "codex".into(), windows: None, error: Some(error.into()), as_of: None })).unwrap();
            assert_eq!(renewed.get(), error.starts_with("session rejected"));
            assert_eq!(result.error.as_deref(), Some(if renewed.get() { "reconnect this account" } else { error }));
            assert_eq!(fs::read(&saved).unwrap(), before);
        }
    }

    #[test]
    fn unnamed_profiles_get_distinct_names_and_creation_timestamps() {
        let f=Fixture::new();
        let first=f.repository.create("",false).unwrap();
        let second=f.repository.create("",false).unwrap();
        assert!(!first.profiles[0].name.is_empty());
        assert_ne!(second.profiles[0].name,second.profiles[1].name);
        assert!(second.profiles.iter().all(|p|p.created_at.is_some()));
    }

    /// Opt-in local check with user-supplied credentials. No network, CLI or real
    /// desktop control: only the filesystem transaction and lifecycle driver run.
    #[test]
    #[ignore = "requires KEYTURN_TEST_AUTH_DIR with personal-auth.json and work-auth.json"]
    fn supplied_files_round_trip_in_isolated_storage() {
        use crate::desktop_switch::{restart_with, DesktopDriver};
        use std::cell::Cell;
        struct TestDesktop(Cell<Option<i32>>);
        impl DesktopDriver for TestDesktop {
            fn ensure_available(&self) -> Result<(), String> { Ok(()) }
            fn pid(&self) -> Result<Option<i32>, String> { Ok(self.0.get()) }
            fn quit(&self) -> Result<(), String> { self.0.set(None); Ok(()) }
            fn reopen(&self) -> Result<(), String> { self.0.set(Some(2)); Ok(()) }
        }
        let source_dir = PathBuf::from(std::env::var("KEYTURN_TEST_AUTH_DIR").expect("set the test directory"));
        let personal_file = source_dir.join("personal-auth.json");
        let work_file = source_dir.join("work-auth.json");
        let personal_raw = fs::read(&personal_file).expect("read personal file");
        let work_raw = fs::read(&work_file).expect("read work file");
        let personal_login = parse_login(serde_json::from_slice(&personal_raw).unwrap()).unwrap();
        let work_login = parse_login(serde_json::from_slice(&work_raw).unwrap()).unwrap();
        assert!(personal_login.identity != work_login.identity, "test accounts must differ");
        let f = Fixture::new();
        write_private(&f.repository.codex_home.join("auth.json"), &personal_login.raw).unwrap();
        let personal_id = f.repository.create("Personal", true).unwrap().profiles[0].id.clone();
        let imported = f.repository.import_file("Work", &work_file, None).unwrap();
        let work_id = imported.profiles.iter().find(|p| p.name == "Work").unwrap().id.clone();
        assert!(imported.active_profile_id.as_deref() == Some(personal_id.as_str()));
        assert!(f.repository.state().unwrap().profiles.iter().find(|p| p.id == work_id).unwrap().needs_activation);
        let driver = TestDesktop(Cell::new(Some(1)));
        restart_with(&driver, |_| {}, || {
            assert!(driver.0.get().is_none(), "credentials must change after exit");
            f.repository.activate(&work_id).map(|_| ())
        }, || {
            assert!(f.repository.current().unwrap().unwrap().identity == work_login.identity);
            Ok(())
        }).unwrap();
        assert!(f.repository.state().unwrap().active_profile_id.as_deref() == Some(work_id.as_str()));
        // A new file for this same account must survive refresh before activation.
        f.repository.import_file("Work", &work_file, Some(&work_id)).unwrap();
        assert!(f.repository.state().unwrap().profiles.iter().find(|p| p.id == work_id).unwrap().needs_activation);
        f.repository.activate(&personal_id).unwrap();
        assert!(f.repository.current().unwrap().unwrap().identity == personal_login.identity);
        assert!(fs::read(&personal_file).unwrap() == personal_raw, "personal source changed");
        assert!(fs::read(&work_file).unwrap() == work_raw, "work source changed");
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            let saved = f.repository.home(&work_id).unwrap().join("auth.json");
            assert_eq!(fs::metadata(saved).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    struct Fixture {
        repository: Repository,
        dir: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("keyturn-profiles-{}", uuid::Uuid::new_v4()));
            let repository = Repository {
                root: dir.join("profiles"),
                codex_home: dir.join("codex"),
            };
            fs::create_dir_all(&repository.codex_home).unwrap();
            Self { repository, dir }
        }
        fn current(&self, subject: &str, token: &str) {
            write_private(
                &self.repository.codex_home.join("auth.json"),
                &login(subject, "workspace", token),
            )
            .unwrap();
        }
        fn connect(&self, id: &str, subject: &str, token: &str) {
            write_private(
                &self.repository.home(id).unwrap().join("auth.json"),
                &login(subject, "workspace", token),
            )
            .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn login(subject: &str, account: &str, token: &str) -> Value {
        let claims = json!({"sub":subject,"email":"same@example.com","https://api.openai.com/auth":{"chatgpt_account_id":account,"chatgpt_plan_type":"plus"}});
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        json!({"auth_mode":"chatgpt","tokens":{"id_token":format!("header.{payload}.signature"),"access_token":token,"refresh_token":format!("refresh-{token}"),"account_id":account},"last_refresh":"2026-10-14T00:00:00Z"})
    }

    #[test]
    fn empty_list_does_not_create_files() {
        let f = Fixture::new();
        let state = f.repository.state().unwrap();
        assert!(state.profiles.is_empty());
        assert!(!state.can_import);
        assert!(!f.repository.root.exists());
    }

    #[test]
    fn imported_profile_exposes_its_own_display_name_without_replacing_the_saved_label() {
        for (label, expected) in [("Work backup", "Work backup"), ("", "Emma Brooks")] {
        let f = Fixture::new();
        f.current("personal", "personal-token");
        let mut imported = login("work", "workspace", "work-token");
        let claims = json!({"sub":"work", "name":"Emma Brooks", "email":"work@example.com", "https://api.openai.com/auth":{"chatgpt_account_id":"workspace","chatgpt_plan_type":"plus"}});
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        imported["tokens"]["id_token"] = format!("header.{payload}.signature").into();
        let source = f.dir.join("other-auth.json");
        write_private(&source, &imported).unwrap();
        let current_before = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        let state = f.repository.import_file(label, &source, None).unwrap();
        let account = &state.profiles[0];
        assert_eq!(account.name, expected);
        assert_eq!(account.display_name.as_deref(), Some("Emma Brooks"));
        assert_eq!(account.email.as_deref(), Some("work@example.com"));
        assert!(!account.active);
        let reread = f.repository.state().unwrap();
        assert_eq!(reread.profiles[0].display_name.as_deref(), Some("Emma Brooks"));
        if label.is_empty() {
            let mut other_claims = claims.clone(); other_claims["sub"] = "other-user".into();
            let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(other_claims.to_string());
            imported["tokens"]["id_token"] = format!("header.{payload}.signature").into();
            write_private(&source, &imported).unwrap();
            let second = f.repository.import_file("", &source, None).unwrap();
            assert_eq!(second.profiles[1].name, "Emma Brooks (2)");
            assert_eq!(second.profiles[0].name, "Emma Brooks");
            let replaced = f.repository.import_file("", &source, Some(&second.profiles[1].id)).unwrap();
            assert_eq!(replaced.profiles[1].name, "Emma Brooks (2)");
        }
        assert_eq!(fs::read(f.repository.codex_home.join("auth.json")).unwrap(), current_before);
        }
    }

    #[test]
    fn imported_work_file_is_copied_without_changing_current_login_or_source() {
        let f = Fixture::new();
        f.current("personal", "personal-token");
        let source = f.dir.join("work-auth.json");
        write_private(&source, &login("work", "workspace", "work-token")).unwrap();
        let original_source = fs::read(&source).unwrap();
        let original_active = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        let state = f.repository.import_file("Work", &source, None).unwrap();
        assert!(state.profiles[0].connected);
        assert!(state.profiles[0].display_name.is_none());
        assert!(state.profiles[0].needs_activation);
        assert!(!state.profiles[0].active);
        assert_eq!(fs::read(&source).unwrap(), original_source);
        assert_eq!(
            fs::read(f.repository.codex_home.join("auth.json")).unwrap(),
            original_active
        );
        let id = state.profiles[0].id.clone();
        let active = f.repository.activate(&id).unwrap();
        assert_eq!(active.active_profile_id, Some(id));
        assert!(!active.profiles[0].needs_activation);
        assert_eq!(
            f.repository.current().unwrap().unwrap().identity.subject,
            "work"
        );
    }

    #[test]
    fn reimported_fresh_tokens_are_not_overwritten_by_the_old_active_snapshot() {
        let f = Fixture::new();
        f.current("work", "old-token");
        let id = f.repository.create("Work", true).unwrap().profiles[0]
            .id
            .clone();
        let source = f.dir.join("new-work-auth.json");
        write_private(&source, &login("work", "workspace", "new-token")).unwrap();
        f.repository
            .import_file("Work", &source, Some(&id))
            .unwrap();
        for _ in 0..3 {
            f.repository.state().unwrap();
        }
        let profile = f.repository.index().unwrap().profiles.remove(0);
        assert_eq!(
            f.repository.login(&profile).unwrap().unwrap().raw["tokens"]["access_token"],
            "new-token"
        );
        assert_eq!(
            f.repository.current().unwrap().unwrap().raw["tokens"]["access_token"],
            "old-token"
        );
        assert!(f.repository.activate(&id).unwrap().profiles[0].active);
        assert_eq!(
            f.repository.current().unwrap().unwrap().raw["tokens"]["access_token"],
            "new-token"
        );
        let backup: Value = serde_json::from_slice(
            &fs::read(f.repository.root.join("previous-active-auth.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(backup["tokens"]["access_token"], "old-token");
    }

    #[test]
    fn import_rejects_invalid_oversized_api_and_duplicate_logins_before_changes() {
        let f = Fixture::new();
        f.current("personal", "p");
        let source = f.dir.join("auth.json");
        fs::write(&source, "invalid secret-token").unwrap();
        assert!(!f
            .repository
            .import_file("Work", &source, None)
            .err()
            .unwrap()
            .contains("secret-token"));
        fs::write(&source, vec![b' '; 256 * 1024 + 1]).unwrap();
        assert!(f.repository.import_file("Work", &source, None).is_err());
        write_private(
            &source,
            &serde_json::json!({"OPENAI_API_KEY":"secret-api-key"}),
        )
        .unwrap();
        assert!(f.repository.import_file("Work", &source, None).is_err());
        assert!(f.repository.state().unwrap().profiles.is_empty());
        let id = f.repository.create("Personal", true).unwrap().profiles[0]
            .id
            .clone();
        write_private(&source, &login("personal", "workspace", "p-new")).unwrap();
        assert!(f
            .repository
            .import_file("Duplicate", &source, None)
            .is_err());
        assert!(f
            .repository
            .import_file("Personal", &source, Some(&id))
            .is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn failed_imported_activation_keeps_the_new_file_pending_and_old_login_intact() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        f.current("work", "old-token");
        let id = f.repository.create("Work", true).unwrap().profiles[0]
            .id
            .clone();
        let source = f.dir.join("new-work-auth.json");
        write_private(&source, &login("work", "workspace", "new-token")).unwrap();
        f.repository
            .import_file("Work", &source, Some(&id))
            .unwrap();
        let active = f.repository.codex_home.join("auth.json");
        let backing = f.repository.codex_home.join("backing.json");
        fs::rename(&active, &backing).unwrap();
        symlink(&backing, &active).unwrap();
        assert!(f.repository.activate(&id).is_err());
        let state = f.repository.state().unwrap();
        assert!(state.profiles[0].needs_activation);
        assert_eq!(
            f.repository.current().unwrap().unwrap().raw["tokens"]["access_token"],
            "old-token"
        );
        let profile = f.repository.index().unwrap().profiles.remove(0);
        assert_eq!(
            f.repository.login(&profile).unwrap().unwrap().raw["tokens"]["access_token"],
            "new-token"
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_metadata_save_does_not_replace_active_or_saved_imported_tokens() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        f.current("work", "old-token");
        let id = f.repository.create("Work", true).unwrap().profiles[0]
            .id
            .clone();
        let source = f.dir.join("new-work-auth.json");
        write_private(&source, &login("work", "workspace", "new-token")).unwrap();
        let index = f.repository.root.join("profiles.json");
        let backing = f.repository.root.join("index-backing.json");
        fs::rename(&index, &backing).unwrap();
        symlink(&backing, &index).unwrap();
        assert!(f
            .repository
            .import_file("Work", &source, Some(&id))
            .is_err());
        assert_eq!(
            f.repository.current().unwrap().unwrap().raw["tokens"]["access_token"],
            "old-token"
        );
        let profile = f.repository.index().unwrap().profiles.remove(0);
        assert_eq!(
            f.repository.login(&profile).unwrap().unwrap().raw["tokens"]["access_token"],
            "old-token"
        );
    }

    #[test]
    fn import_rename_and_remove_leave_cli_login_intact() {
        let f = Fixture::new();
        f.current("alice", "alice-access");
        let state = f.repository.create("  Personal  ", true).unwrap();
        let profile = &state.profiles[0];
        assert_eq!(profile.name, "Personal");
        assert!(profile.active);
        assert_eq!(
            state.active_profile_id.as_deref(),
            Some(profile.id.as_str())
        );
        assert!(f.repository.create("Duplicate", true).is_err());
        assert_eq!(
            f.repository.rename(&profile.id, "Work").unwrap().profiles[0].name,
            "Work"
        );
        let original = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        assert!(f
            .repository
            .remove(&profile.id)
            .unwrap()
            .profiles
            .is_empty());
        assert_eq!(
            fs::read(f.repository.codex_home.join("auth.json")).unwrap(),
            original
        );
        assert!(!f.repository.home(&profile.id).unwrap().exists());
    }

    #[test]
    fn separate_login_and_switch_preserve_latest_rotated_tokens_and_config() {
        let f = Fixture::new();
        f.current("alice", "old");
        let alice = f.repository.create("Personal", true).unwrap().profiles[0]
            .id
            .clone();
        let state = f.repository.create("Work", false).unwrap();
        let bob = &state.profiles[1];
        assert!(!bob.connected);
        assert_eq!(
            f.repository.current().unwrap().unwrap().identity.subject,
            "alice"
        );
        f.connect(&bob.id, "bob", "bob-access");
        f.current("alice", "rotated");
        let config = "model = \"gpt-test\"\n[projects.test]\ntrust_level = \"trusted\"\n";
        fs::write(f.repository.codex_home.join("config.toml"), config).unwrap();
        let fresh = f.repository.activate(&bob.id).unwrap();
        assert_eq!(fresh.active_profile_id.as_deref(), Some(bob.id.as_str()));
        assert_eq!(
            f.repository.current().unwrap().unwrap().identity.subject,
            "bob"
        );
        assert_eq!(
            f.repository
                .login(&Profile {
                    id: alice.clone(),
                    name: "Personal".into(),
                    created_at: Some(chrono::Utc::now().to_rfc3339()),
            needs_activation: false,
                })
                .unwrap()
                .unwrap()
                .raw["tokens"]["access_token"],
            "rotated"
        );
        f.repository.activate(&alice).unwrap();
        assert_eq!(
            f.repository.current().unwrap().unwrap().raw["tokens"]["access_token"],
            "rotated"
        );
        assert_eq!(
            fs::read_to_string(f.repository.codex_home.join("config.toml")).unwrap(),
            config
        );
    }

    #[test]
    fn external_account_changes_determine_active_profile_without_using_email() {
        let f = Fixture::new();
        f.current("alice", "a");
        let a = f.repository.create("Personal", true).unwrap().profiles[0]
            .id
            .clone();
        f.current("bob", "b");
        assert!(f.repository.state().unwrap().active_profile_id.is_none());
        let b = f.repository.create("Work", true).unwrap().profiles[1]
            .id
            .clone();
        f.current("alice", "a-new");
        let state = f.repository.state().unwrap();
        assert_eq!(state.active_profile_id, Some(a));
        assert!(!state.profiles.iter().find(|p| p.id == b).unwrap().active);
    }

    #[test]
    fn switching_preserves_an_unsaved_previous_account() {
        let f = Fixture::new();
        f.current("alice", "a");
        let target = f.repository.create("Work", false).unwrap().profiles[0]
            .id
            .clone();
        f.connect(&target, "bob", "b");
        let state = f.repository.activate(&target).unwrap();
        let previous = state
            .profiles
            .iter()
            .find(|p| p.name == "Previous account")
            .unwrap();
        f.repository.activate(&previous.id).unwrap();
        assert_eq!(
            f.repository.current().unwrap().unwrap().identity.subject,
            "alice"
        );
    }

    #[test]
    fn invalid_names_and_unknown_ids_do_not_change_the_active_login() {
        let f = Fixture::new();
        f.current("alice", "a");
        for name in ["bad\nname", &"a".repeat(65)] {
            assert!(f.repository.create(name, false).is_err());
        }
        // Blank labels now use generated names; they still leave active login intact.
        for name in ["", " \n "] {
            let state = f.repository.create(name, false).unwrap();
            let generated = state.profiles.last().unwrap();
            assert!(!generated.name.trim().is_empty());
            assert!(state.profiles.iter().filter(|p| p.name == generated.name).count() == 1);
        }
        f.repository.create("Personal", false).unwrap();
        assert!(f.repository.create("personal", false).is_err());
        assert!(f.repository.activate("../../auth").is_err());
        assert!(f.repository.rename("unknown", "New").is_err());
        assert!(f.repository.remove("unknown").is_err());
        assert_eq!(
            f.repository.current().unwrap().unwrap().identity.subject,
            "alice"
        );
    }

    #[test]
    fn incomplete_and_corrupt_logins_cannot_replace_current_credentials() {
        let f = Fixture::new();
        f.current("alice", "a");
        let id = f.repository.create("Work", false).unwrap().profiles[0]
            .id
            .clone();
        let original = fs::read(f.repository.codex_home.join("auth.json")).unwrap();
        assert!(f.repository.activate(&id).is_err());
        fs::write(
            f.repository.home(&id).unwrap().join("auth.json"),
            b"{incomplete secret-access-token",
        )
        .unwrap();
        let error = f.repository.activate(&id).err().unwrap();
        assert!(!error.contains("secret-access-token"));
        assert!(!f.repository.state().unwrap().profiles[0].connected);
        assert_eq!(
            fs::read(f.repository.codex_home.join("auth.json")).unwrap(),
            original
        );
        let mut invalid = login("bob", "workspace", "b");
        invalid["tokens"]["refresh_token"] = json!("");
        assert!(parse_login(invalid).is_err());
    }

    #[test]
    fn keyring_and_workspace_restrictions_are_checked_before_activation() {
        let f = Fixture::new();
        f.current("alice", "a");
        let id = f.repository.create("Work", false).unwrap().profiles[0]
            .id
            .clone();
        f.connect(&id, "bob", "b");
        for mode in ["keyring", "auto", "ephemeral"] {
            fs::write(
                f.repository.codex_home.join("config.toml"),
                format!("cli_auth_credentials_store = \"{mode}\"\n"),
            )
            .unwrap();
            assert!(f.repository.state().unwrap().switch_error.is_some());
            assert!(f.repository.activate(&id).is_err());
        }
        fs::write(
            f.repository.codex_home.join("config.toml"),
            "forced_chatgpt_workspace_id = \"another-workspace\"",
        )
        .unwrap();
        assert!(f.repository.activate(&id).is_err());
        assert_eq!(
            f.repository.current().unwrap().unwrap().identity.subject,
            "alice"
        );
    }

    #[test]
    fn ipc_and_metadata_never_contain_credentials() {
        let f = Fixture::new();
        f.current("alice", "secret-access-token");
        let state = f.repository.create("Personal", true).unwrap();
        let ipc = serde_json::to_string(&state).unwrap();
        let index = fs::read_to_string(f.repository.root.join("profiles.json")).unwrap();
        for raw in [ipc, index] {
            assert!(!raw.contains("secret-access-token"));
            assert!(!raw.contains("refresh_token"));
            assert!(!raw.contains("id_token"));
        }
    }

    #[test]
    fn duplicate_browser_login_is_identified_and_cannot_be_activated_twice() {
        let f = Fixture::new();
        f.current("alice", "a");
        f.repository.create("Personal", true).unwrap();
        let id = f.repository.create("Work", false).unwrap().profiles[1]
            .id
            .clone();
        f.connect(&id, "alice", "same-account");
        let state = f.repository.state().unwrap();
        assert_eq!(state.profiles.iter().filter(|p| p.active).count(), 1);
        assert!(!state.profiles[1].connected);
        assert!(state.profiles[1]
            .connection_error
            .as_ref()
            .unwrap()
            .contains("Personal"));
        assert!(f.repository.activate(&id).is_err());
        f.connect(&id, "bob", "b");
        assert!(f.repository.activate(&id).is_ok());
    }

    #[test]
    fn same_user_in_different_workspaces_has_distinct_profiles() {
        let f = Fixture::new();
        f.current("alice", "a");
        f.repository.create("Personal", true).unwrap();
        let id = f.repository.create("Team", false).unwrap().profiles[1]
            .id
            .clone();
        write_private(
            &f.repository.home(&id).unwrap().join("auth.json"),
            &login("alice", "team-workspace", "team"),
        )
        .unwrap();
        let state = f.repository.activate(&id).unwrap();
        assert!(state.profiles[1].active);
        assert!(!state.profiles[0].active);
    }

    #[test]
    fn failed_active_file_replacement_preserves_original_credentials() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let f = Fixture::new();
            f.current("alice", "a");
            let current = f.repository.codex_home.join("auth.json");
            let backing = f.repository.codex_home.join("backing.json");
            fs::rename(&current, &backing).unwrap();
            symlink(&backing, &current).unwrap();
            let id = f.repository.create("Work", false).unwrap().profiles[0]
                .id
                .clone();
            f.connect(&id, "bob", "b");
            assert!(f.repository.activate(&id).is_err());
            assert_eq!(
                f.repository.current().unwrap().unwrap().identity.subject,
                "alice"
            );
            assert!(fs::symlink_metadata(current)
                .unwrap()
                .file_type()
                .is_symlink());
        }
    }

    #[test]
    fn login_commands_quote_paths_with_shell_metacharacters() {
        let mut f = Fixture::new();
        f.repository.root = f.dir.join("person's $(touch injected) `cmd` profiles");
        let state = f.repository.create("Work", false).unwrap();
        assert!(state.profiles[0]
            .login_command
            .contains("person'\\''s $(touch injected) `cmd` profiles"));
        assert!(!f.dir.join("injected").exists());
    }

    #[cfg(unix)]
    #[test]
    fn credentials_and_directories_are_owner_only_and_symlinks_are_rejected() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let f = Fixture::new();
        f.current("alice", "a");
        let id = f.repository.create("Personal", true).unwrap().profiles[0]
            .id
            .clone();
        let home = f.repository.home(&id).unwrap();
        assert_eq!(
            fs::metadata(&home).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(home.join("auth.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let target = f.dir.join("linked-auth.json");
        symlink(f.repository.codex_home.join("auth.json"), &target).unwrap();
        assert!(write_private(&target, &login("bob", "workspace", "b")).is_err());
        assert_eq!(
            f.repository.current().unwrap().unwrap().identity.subject,
            "alice"
        );
    }
}

/// Validate in a private staging home. Files alone never prove live paid access.
pub(crate) fn inspect_staged(home: &Path) -> Result<(String, Option<String>, crate::subscriptions::Subscription), String> {
    let server = crate::codex_refresh::account(home)?;
    let login = read_login(&home.join("auth.json"))?.ok_or("Sign-in did not save an auth.json file.")?;
    if server["email"].as_str().zip(login.email.as_deref()).is_some_and(|(a,b)| a != b) {
        return Err("Account identity changed while checking the login. Sign in again.".into());
    }
    let mut subscription = login.subscription;
    subscription.plan = server["planType"].as_str().map(str::to_owned);
    if !subscription.is_paid() { return Err("A confirmed paid ChatGPT plan is required. Check your subscription and retry.".into()); }
    Ok((format!("{}:{}", login.identity.subject, login.identity.account), login.email, subscription))
}

pub(crate) fn saved_auth_path(id: &str) -> Result<PathBuf, String> {
    with_repository(|repo| {
        let index = repo.index()?;
        let profile = index.profiles.iter().find(|p| p.id == id).ok_or("This saved account no longer exists.")?;
        Ok(repo.home(&profile.id)?.join("auth.json"))
    })
}

/// Reuse a matching saved identity; adding membership must not duplicate credentials.
pub(crate) fn import_or_reuse(name: &str, source: &Path) -> Result<(String, bool), String> {
    with_repository(|repo| {
        let login = read_login(source)?.ok_or("Choose an auth.json file.")?;
        let before = repo.index()?;
        if let Some(existing) = before.profiles.iter().find(|p| repo.login(p).ok().flatten().is_some_and(|old| old.identity == login.identity)) {
            let id = existing.id.clone();
            repo.import_file(&existing.name, source, Some(&id))?;
            return Ok((id, false));
        }
        let mut label = name.to_owned();
        let mut suffix = 2;
        while before.profiles.iter().any(|p| p.name.to_lowercase() == label.to_lowercase()) {
            label = format!("{} ({suffix})", name.chars().take(54).collect::<String>()); suffix += 1;
        }
        let after = repo.import_file(&label, source, None)?;
        let profile = after.profiles.iter().find(|p| !before.profiles.iter().any(|old| old.id == p.id)).ok_or("Cannot save this account.")?;
        Ok((profile.id.clone(), true))
    })
}

pub(crate) fn inspect_saved(id: &str) -> Result<(String, Option<String>, crate::subscriptions::Subscription), String> {
    with_repository(|repo| inspect_staged(&repo.home(id)?))
}
