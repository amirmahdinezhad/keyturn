//! Anthropic profiles use Claude Code's documented CLAUDE_CONFIG_DIR isolation.
//! Claude owns the directory and Keychain login. Keyturn stores only metadata.
use crate::{
    paths,
    profiles::{private_dir, write_private},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeProfile {
    pub id: String,
    pub name: String,
    pub config_dir: PathBuf,
    pub created_at: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub subscription: Option<crate::subscriptions::Subscription>,
}
#[derive(Default, Serialize, Deserialize)]
struct Index {
    profiles: Vec<ClaudeProfile>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    #[serde(flatten)]
    pub profile: ClaudeProfile,
    pub active: bool,
    pub login_command: String,
    pub launch_command: String,
    pub quota: Option<crate::quota::QuotaSet>,
}
fn root() -> PathBuf {
    paths::opencode_user_data().join("keyturn-claude-profiles")
}
fn read(root: &Path) -> Result<Index, String> {
    let path = root.join("profiles.json");
    match std::fs::read(path) {
        Ok(raw) => serde_json::from_slice(&raw)
            .map_err(|_| "Cannot read Anthropic profiles. Restore the profile index.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Index::default()),
        Err(_) => Err("Cannot read Anthropic profiles. Check permissions.".into()),
    }
}
fn save(root: &Path, index: &Index) -> Result<(), String> {
    private_dir(root)?;
    write_private(
        &root.join("profiles.json"),
        &serde_json::to_value(index).map_err(|_| "Cannot encode profiles.")?,
    )
}
fn shell(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}
fn views(index: Index) -> Vec<ProfileView> {
    std::thread::scope(|scope| {
        let jobs: Vec<_> = index.profiles.into_iter().map(|mut p| scope.spawn(move || {
            let prefix = format!("CLAUDE_CONFIG_DIR={} claude", shell(&p.config_dir));
            let quota = crate::claude_usage::current_quota_in(&p.config_dir);
            p.connected = quota.error.is_none() && quota.windows.iter().flatten().any(|w| w.percent.is_finite() && (0.0..=100.0).contains(&w.percent));
            ProfileView { active: p.config_dir == paths::claude_home(), profile: p, login_command: format!("{prefix} auth login"), launch_command: prefix, quota: Some(quota) }
        })).collect();
        jobs.into_iter().filter_map(|job| job.join().ok()).collect()
    })
}
/// Recheck each saved session independently; a cached success never masks a live failure.
pub fn list() -> Result<Vec<ProfileView>, String> {
    let index = { let _guard = LOCK.lock().map_err(|_| "Profile storage is busy.")?; read(&root())? };
    Ok(views(index))
}
/// Resolve a saved profile by id before reading its own usage credentials.
/// An omitted id selects the default CLI configuration.
pub fn quota(id: Option<&str>) -> Result<crate::quota::QuotaSet, String> {
    let config = if let Some(id) = id {
        let _guard = LOCK.lock().map_err(|_| "Profile storage is busy.")?;
        read(&root())?.profiles.into_iter().find(|p| p.id == id)
            .ok_or("Profile no longer exists.")?.config_dir
    } else { paths::claude_home() };
    Ok(crate::claude_usage::live_quota_in(&config))
}

fn create_in(root: &Path, current: &Path, raw: &str, use_current: bool) -> Result<(), String> {
    let mut index = read(root)?;
    let id = uuid::Uuid::new_v4().to_string();
    let name = if raw.trim().is_empty() {
        format!("Curious Otter {}", &id[..4])
    } else {
        raw.trim().to_owned()
    };
    validate_name(&name)?;
    if index
        .profiles
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&name))
    {
        return Err("A profile with this name already exists.".into());
    }
    let config = if use_current {
        current.to_path_buf()
    } else {
        root.join(&id)
    };
    if index.profiles.iter().any(|p| p.config_dir == config) {
        return Err("The current Claude configuration is already saved.".into());
    }
    if !use_current {
        private_dir(&config)?;
    }
    index.profiles.push(ClaudeProfile {
        id,
        name,
        config_dir: config,
        created_at: chrono::Utc::now().to_rfc3339(),
        email: None,
        connected: false,
        subscription: None,
    });
    save(root, &index)
}
fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        Err("Use a profile name with 1–64 characters.".into())
    } else {
        Ok(())
    }
}
/// Add an isolated config or track the current CLI config without copying secrets.
pub fn create(name: &str, use_current: bool) -> Result<Vec<ProfileView>, String> {
    let _guard = LOCK.lock().map_err(|_| "Profile storage is busy.")?;
    create_in(&root(), &paths::claude_home(), name, use_current)?;
    Ok(views(read(&root())?))
}
/// Probe only the requested CLI configuration; credentials remain with Claude.
pub fn rescan(id: &str) -> Result<Vec<ProfileView>, String> {
    let profile = {
        let _guard = LOCK.lock().map_err(|_| "Profile storage is busy.")?;
        read(&root())?
            .profiles
            .into_iter()
            .find(|p| p.id == id)
            .ok_or("Profile no longer exists.")?
    };
    let binary = crate::cli::which_binary("claude")
        .ok_or("Install Claude Code before checking this profile.")?;
    let (connected, _, email, _, subscription) =
        crate::cli::probe_claude_at(&binary, Some(&profile.config_dir));
    let connected = connected.ok_or("Claude did not return a connection status. Try again.")?;
    let _guard = LOCK.lock().map_err(|_| "Profile storage is busy.")?;
    let mut index = read(&root())?;
    let target = index
        .profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or("Profile no longer exists.")?;
    target.connected = connected;
    target.email = email;
    target.subscription = subscription;
    save(&root(), &index)?;
    Ok(views(index))
}
/// Rename or forget a tracked profile; its CLI-owned login/history are preserved.
pub fn edit(id: &str, name: Option<&str>) -> Result<Vec<ProfileView>, String> {
    edit_profile(id, name, false)
}

/// Explicit global forgetting also clears app-owned login storage, never a connected external config.
pub fn forget(id: &str) -> Result<Vec<ProfileView>, String> {
    edit_profile(id, None, true)
}

fn app_owned_config(home: &Path, managed_roots: &[PathBuf], current: &Path) -> bool {
    managed_roots.iter().any(|parent| home.parent() == Some(parent.as_path()))
        && home.file_name().and_then(|name| name.to_str()).is_some_and(|name| uuid::Uuid::parse_str(name).is_ok())
        && home != current
        && std::fs::symlink_metadata(home).is_ok_and(|metadata| metadata.is_dir())
}

fn edit_profile(id: &str, name: Option<&str>, cleanup: bool) -> Result<Vec<ProfileView>, String> {
    let _guard = LOCK.lock().map_err(|_| "Profile storage is busy.")?;
    let mut index = read(&root())?;
    if !index.profiles.iter().any(|p| p.id == id) {
        return Err("Profile no longer exists.".into());
    }
    let forgotten_home = if cleanup && name.is_none() { index.profiles.iter().find(|p| p.id == id).map(|p| p.config_dir.clone()) } else { None };
    if let Some(name) = name {
        let name = name.trim();
        validate_name(name)?;
        if index
            .profiles
            .iter()
            .any(|p| p.id != id && p.name.eq_ignore_ascii_case(name))
        {
            return Err("A profile with this name already exists.".into());
        }
        index.profiles.iter_mut().find(|p| p.id == id).unwrap().name = name.into();
    } else {
        index.profiles.retain(|p| p.id != id);
    }
    save(&root(), &index)?;
    if let Some(home) = forgotten_home {
        // Forget only our isolated storage. Existing CLI configurations are references.
        if app_owned_config(&home, &[root(), paths::opencode_user_data().join("keyturn-login-homes")], &paths::claude_home())
            && !index.profiles.iter().any(|profile| profile.config_dir == home) {
            crate::account_setup_cleanup::claude(&home);
            let _ = std::fs::remove_dir_all(home);
        }
    }
    Ok(views(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_scope_excludes_external_current_and_symlinked_configurations() {
        let temp=std::env::temp_dir().join(format!("keyturn-claude-cleanup-{}",uuid::Uuid::new_v4()));
        let root=temp.join("managed");let owned=root.join(uuid::Uuid::new_v4().to_string());let external=temp.join("external");
        std::fs::create_dir_all(&owned).unwrap();std::fs::create_dir_all(&external).unwrap();let roots=vec![root.clone()];
        assert!(app_owned_config(&owned,&roots,&external));
        assert!(!app_owned_config(&external,&roots,&external));assert!(!app_owned_config(&owned,&roots,&owned));
        assert!(!app_owned_config(&root,&roots,&external));
        #[cfg(unix)] {let link=root.join(uuid::Uuid::new_v4().to_string());std::os::unix::fs::symlink(&external,&link).unwrap();assert!(!app_owned_config(&link,&roots,&external));}
        std::fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn isolated_profiles_have_private_directories_and_current_profile_is_a_reference() {
        let temp = std::env::temp_dir().join(format!("keyturn-claude-{}", uuid::Uuid::new_v4()));
        let current = temp.join("current");
        let root = temp.join("profiles");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(current.join("settings.json"), "{}").unwrap();
        create_in(&root, &current, "", false).unwrap();
        create_in(&root, &current, "Personal", true).unwrap();
        let index = read(&root).unwrap();
        assert_eq!(index.profiles.len(), 2);
        assert_ne!(index.profiles[0].config_dir, current);
        assert_eq!(index.profiles[1].config_dir, current);
        assert!(create_in(&root, &current, "Duplicate", true).is_err());
        assert_eq!(
            std::fs::read_to_string(current.join("settings.json")).unwrap(),
            "{}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&index.profiles[0].config_dir)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(root.join("profiles.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        std::fs::remove_dir_all(temp).unwrap();
    }
}

/// Adopt a CLI-owned configuration only after sign-in and paid-plan verification.
pub(crate) fn adopt_config(name: &str, config: &Path, email: Option<String>, subscription: crate::subscriptions::Subscription) -> Result<(String, bool), String> {
    let _guard = LOCK.lock().map_err(|_| "Profile storage is busy.")?;
    let mut index = read(&root())?;
    if let Some(profile) = index.profiles.iter().find(|p| p.config_dir == config) { return Ok((profile.id.clone(), false)); }
    let name = name.trim(); validate_name(name)?;
    let id = uuid::Uuid::new_v4().to_string();
    index.profiles.push(ClaudeProfile { id: id.clone(), name: name.into(), config_dir: config.into(), created_at: chrono::Utc::now().to_rfc3339(), email, connected: true, subscription: Some(subscription) });
    save(&root(), &index)?;
    Ok((id, true))
}
