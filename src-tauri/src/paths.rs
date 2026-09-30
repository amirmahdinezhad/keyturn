use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn home_dir() -> PathBuf {
    dirs::home_dir().expect("home directory")
}

pub fn opencode_user_data() -> PathBuf {
    if let Ok(override_dir) = env::var("KEYTURN_OPENCODE_USERDATA") {
        if !override_dir.is_empty() {
            return PathBuf::from(override_dir);
        }
    }
    #[cfg(target_os = "macos")]
    return home_dir().join("Library/Application Support/ai.opencode.desktop");
    #[cfg(target_os = "windows")]
    return home_dir().join("AppData/Roaming/ai.opencode.desktop");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return home_dir().join(".config/ai.opencode.desktop");
}

/// Single per-machine store file (launch-at-login flag only now).
pub fn store_file() -> PathBuf {
    opencode_user_data().join("agent-rtl.keys")
}

/// App-wide settings (refresh cadence, billing day, update checks).
pub fn settings_file() -> PathBuf {
    opencode_user_data().join("agent-rtl.settings.json")
}

pub fn codex_home() -> PathBuf {
    if let Ok(home) = env::var("CODEX_HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    home_dir().join(".codex")
}

pub fn codex_auth_file() -> PathBuf {
    codex_home().join("auth.json")
}

pub fn downloads_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(home_dir)
}

pub fn open_code_auth_file() -> PathBuf {
    if let Ok(data_home) = env::var("XDG_DATA_HOME") {
        if !data_home.is_empty() {
            return PathBuf::from(data_home).join("opencode/auth.json");
        }
    }
    #[cfg(target_os = "windows")]
    return home_dir().join(".opencode/auth.json");
    #[cfg(not(target_os = "windows"))]
    return home_dir().join(".local/share/opencode/auth.json");
}

pub fn open_code_db_file() -> PathBuf {
    open_code_auth_file()
        .parent()
        .map(|p| p.join("opencode.db"))
        .unwrap_or_else(open_code_auth_file)
}

pub fn atomic_write_json(file: &Path, data: &Value) {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).expect("create dir");
    }
    let parent = file.parent().unwrap_or_else(|| Path::new("."));
    let base = file
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "store.json".to_string());
    let tmp = parent.join(format!(
        ".{base}.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    // Tab-indented like the Electron app (`JSON.stringify(data, null, '\t')`).
    let pretty = serde_json::to_string_pretty(data).expect("serialize");
    let mut body = String::with_capacity(pretty.len());
    for (i, line) in pretty.split('\n').enumerate() {
        if i > 0 {
            body.push('\n');
        }
        let spaces = line.len() - line.trim_start_matches(' ').len();
        for _ in 0..spaces / 2 {
            body.push('\t');
        }
        body.push_str(&line[spaces..]);
    }
    fs::write(&tmp, body).expect("write tmp");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, file).expect("rename");
}

pub fn read_json(file: &Path) -> Option<Value> {
    let raw = fs::read_to_string(file).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&raw).ok()
}
