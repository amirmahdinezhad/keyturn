//! Provider registry: one place that describes every provider we can talk to.
//!
//! Adding a provider is a data change here plus (optionally) a docs update —
//! see `docs/ADDING_A_PROVIDER.md`. The rest of the app asks this module for
//! capabilities instead of hard-coding provider ids.

use std::path::PathBuf;

/// Where a provider keeps the credential file the local CLI reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthTarget {
    /// `~/.local/share/opencode/auth.json`, entry `{ "<provider>": { type, key } }`.
    OpenCode,
    /// `~/.codex/auth.json`, key `OPENAI_API_KEY` (or OAuth `tokens`).
    Codex,
    /// No local credential file — the key is only stored for reference.
    #[allow(dead_code)]
    None,
}

impl AuthTarget {
    /// JSON path to the field that holds the API key inside the auth file.
    #[allow(dead_code)]
    pub fn key_path(self) -> &'static [&'static str] {
        match self {
            AuthTarget::OpenCode => &["key"],
            AuthTarget::Codex => &["OPENAI_API_KEY"],
            AuthTarget::None => &[],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub host: &'static str,
    pub login_url: &'static str,
    pub home_url: &'static str,
    /// Path on `host` that lists API keys (console providers).
    pub keys_path: &'static str,
    /// Path on `host` that shows usage (console providers).
    pub usage_path: Option<&'static str>,
    /// Base URL of a JSON quota endpoint (`GET {base}/usage`, Bearer key).
    pub usage_api: Option<&'static str>,
    pub auth: AuthTarget,
    /// URL fragments proving the console webview is signed in.
    pub signed_in_markers: &'static [&'static str],
    /// URL fragments that mean "still on a login page".
    pub login_markers: &'static [&'static str],
    /// Shown in the picker as a provider the user can connect.
    pub connectable: bool,
}

/// Registry API used by tests and by callers that need to branch on what a
/// provider can do (kept even when no runtime caller exists yet).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub console: bool,
    pub usage_api: bool,
    pub auth_patch: bool,
    pub token_refresh: bool,
}

impl ProviderSpec {
    #[allow(dead_code)]
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            console: !self.host.is_empty() && self.connectable,
            usage_api: self.usage_api.is_some(),
            auth_patch: self.auth != AuthTarget::None,
            token_refresh: self.auth == AuthTarget::Codex,
        }
    }
}

const SPECS: &[ProviderSpec] = &[
    ProviderSpec {
        id: "opencode-go",
        label: "OpenCode Go",
        host: "opencode.ai",
        login_url: "https://opencode.ai/auth",
        home_url: "https://opencode.ai/",
        keys_path: "",
        usage_path: None,
        usage_api: Some("https://opencode.ai/zen/go/v1"),
        auth: AuthTarget::OpenCode,
        signed_in_markers: &["/workspace/"],
        login_markers: &["/auth", "/login"],
        connectable: true,
    },
    ProviderSpec {
        id: "opencode",
        label: "OpenCode",
        host: "opencode.ai",
        login_url: "https://opencode.ai/auth",
        home_url: "https://opencode.ai/",
        keys_path: "",
        usage_path: None,
        usage_api: Some("https://opencode.ai/zen/v1"),
        auth: AuthTarget::OpenCode,
        signed_in_markers: &["/workspace/"],
        login_markers: &["/auth", "/login"],
        connectable: true,
    },
    ProviderSpec {
        id: "anthropic",
        label: "Claude",
        host: "console.anthropic.com",
        login_url: "https://console.anthropic.com/login",
        home_url: "https://console.anthropic.com/",
        keys_path: "/settings/keys",
        usage_path: Some("/settings/usage"),
        usage_api: None,
        auth: AuthTarget::OpenCode,
        signed_in_markers: &["console.anthropic.com"],
        login_markers: &["/login", "/oauth"],
        connectable: true,
    },
    ProviderSpec {
        id: "openai",
        label: "OpenAI",
        host: "platform.openai.com",
        login_url: "https://platform.openai.com/login",
        home_url: "https://platform.openai.com/",
        keys_path: "/api-keys",
        usage_path: Some("/usage"),
        usage_api: None,
        auth: AuthTarget::OpenCode,
        signed_in_markers: &["platform.openai.com"],
        login_markers: &["/login", "/auth"],
        connectable: true,
    },
    ProviderSpec {
        id: "google",
        label: "Gemini",
        host: "aistudio.google.com",
        login_url: "https://aistudio.google.com/",
        home_url: "https://aistudio.google.com/",
        keys_path: "/apikey",
        usage_path: None,
        usage_api: None,
        auth: AuthTarget::OpenCode,
        signed_in_markers: &["aistudio.google.com"],
        login_markers: &["/signin", "/login"],
        connectable: true,
    },
    ProviderSpec {
        id: "codex",
        label: "OpenAI Codex",
        host: "chatgpt.com",
        login_url: "https://chatgpt.com/",
        home_url: "https://chatgpt.com/",
        keys_path: "",
        usage_path: None,
        usage_api: None,
        auth: AuthTarget::Codex,
        signed_in_markers: &[],
        login_markers: &[],
        connectable: false,
    },
];

pub fn all() -> &'static [ProviderSpec] {
    SPECS
}

pub fn by_id(id: &str) -> Option<&'static ProviderSpec> {
    SPECS.iter().find(|s| s.id == id)
}

/// Unknown ids (hand-added providers) get the conservative default: nothing
/// but local key storage.
pub fn capabilities(id: &str) -> Capabilities {
    by_id(id)
        .map(ProviderSpec::capabilities)
        .unwrap_or(Capabilities {
            console: false,
            usage_api: false,
            auth_patch: false,
            token_refresh: false,
        })
}

pub fn usage_api(id: &str) -> Option<&'static str> {
    by_id(id).and_then(|s| s.usage_api)
}

pub fn auth_target(id: &str) -> AuthTarget {
    by_id(id).map(|s| s.auth).unwrap_or(AuthTarget::OpenCode)
}

/// Base URL of the console the sign-in window should land on.
pub fn console_home(id: &str) -> Option<String> {
    by_id(id).map(|s| s.home_url.to_string())
}

/// Is the console webview parked on a signed-in page for this provider?
pub fn is_signed_in(id: &str, url: &str) -> bool {
    let Some(spec) = by_id(id) else {
        return false;
    };
    if !url.contains(spec.host) {
        return false;
    }
    if spec.login_markers.iter().any(|m| url.contains(m)) {
        return false;
    }
    spec.signed_in_markers.is_empty() || spec.signed_in_markers.iter().any(|m| url.contains(m))
}

/// The credential file a provider's local CLI reads, if any.
pub fn auth_file(id: &str) -> Option<PathBuf> {
    match auth_target(id) {
        AuthTarget::OpenCode => Some(crate::paths::open_code_auth_file()),
        AuthTarget::Codex => Some(crate::paths::codex_auth_file()),
        AuthTarget::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lookup_and_capabilities() {
        let go = by_id("opencode-go").expect("opencode-go");
        assert!(go.capabilities().usage_api);
        assert!(go.capabilities().auth_patch);
        assert!(!go.capabilities().token_refresh);

        let codex = by_id("codex").expect("codex");
        assert_eq!(codex.auth, AuthTarget::Codex);
        assert_eq!(codex.auth.key_path(), &["OPENAI_API_KEY"]);
        assert!(codex.capabilities().token_refresh);
        assert!(!codex.capabilities().console);

        let unknown = capabilities("acme-llm");
        assert!(!unknown.console && !unknown.usage_api && !unknown.auth_patch);
    }

    #[test]
    fn opencode_auth_entry_uses_the_key_field() {
        assert_eq!(auth_target("anthropic"), AuthTarget::OpenCode);
        assert_eq!(auth_target("anthropic").key_path(), &["key"]);
    }

    #[test]
    fn signed_in_checks_host_and_login_markers() {
        assert!(is_signed_in(
            "anthropic",
            "https://console.anthropic.com/settings/keys"
        ));
        assert!(!is_signed_in(
            "anthropic",
            "https://console.anthropic.com/login"
        ));
        assert!(!is_signed_in("anthropic", "https://example.com/"));
        assert!(is_signed_in(
            "opencode-go",
            "https://opencode.ai/workspace/wrk_1/usage/"
        ));
    }
}
