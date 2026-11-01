# Keyturn

A macOS menu-bar panel for AI coding CLI accounts and usage limits. Keyturn
checks Claude, OpenAI Codex, OpenCode and Gemini, then shows the CLIs that are
installed and signed in.

Add account starts official Codex or Claude Code browser sign-in from the panel
using an isolated configuration. Codex also supports device codes and auth.json
upload. Claude can connect an existing signed-in configuration. Experimental
Antigravity support offers Google sign-in, current AGY login and credentials JSON.
Only verified paid subscription accounts can enter the new workspace flow.

Codex browser and device-code sign-in start directly from the method picker,
without an intermediate Continue/Get code screen. Cancelling returns to the
picker; failed starts show the error there and allow another explicit attempt.
Both methods use the verified identity name automatically,
falling back to email or ChatGPT account when no name is present. Account names
remain editable later in Account settings.

Saved Codex accounts renew expired usage sessions through the CLI automatically.
If a copied session can no longer be renewed, use **Reconnect account → Replace
login** to import a fresh auth.json without switching the active account.

Built with Tauri v2, Rust, React 19, TypeScript and Tailwind v4. Local UI
components follow Shadcn Studio composition with Radix interaction primitives.

## Features

- Workspaces start with name/accent, then explicitly load and select accounts.
  Optional step three adds accounts by provider and sign-in method. Multiple new
  accounts remain drafts until one final save. Saved accounts can be shared by
  workspaces; Codex display and switch preferences are independent.
  Each workspace has General / Usage / Menu bar settings.
  Switching workspaces leaves the active Codex login unchanged. Existing accounts
  migrate into Personal. See [Workspaces](WORKSPACES.md).
- Limited active accounts show total hours/minutes until all exhausted main
  windows reset. The adjacent switch icon selects the next usable account in
  the workspace and starts the desktop switch immediately.
- Named Codex accounts: import auth.json, rename/remove accounts, move them
  between workspaces and explicitly activate a saved login.
  Switching preserves the previous account and updated CLI tokens. It supports
  Codex's file-based credential store; Keychain/auto/ephemeral stores are reported
  as unsupported rather than silently changing storage settings.
- Connected accounts with identity, sign-in method, plan when available,
  short provider names and named OpenCode services. Expired subscriptions are hidden.
- The Codex section shows one reset update above its account cards: upcoming
  announcements, recent confirmed resets, or optional probability forecasts.
  Workspace Usage has a Show reset updates master toggle; dismissal persists.
- An Add account panel: choose a provider and method, finish sign-in in the browser
  or import a file, then verify identity and paid plan before saving. Closing an
  unfinished draft asks to discard it and cleans its temporary login.
- Usage limits in a separate popup, with percentage bars and reset countdowns:

  | CLI | Source used by Keyturn |
  | --- | --- |
  | OpenCode | Live usage requests for `opencode` and `opencode-go` entries in its auth file. |
  | Codex | Usage request with the CLI's stored OAuth access token. API-key logins do not provide this quota source. |
  | Claude | Live subscription limits using the selected profile’s Claude login. On failure, a local snapshot retains its observation age, or a specific error is shown. |
  | Gemini | No usage popup. |

- Global settings has General and Updates tabs: app launch/refresh
  behavior and theme in General, and update checks/downloads/installation in Updates.
  Workspace settings owns usage, switching, reset visibility and workspace accent.
- Optional percentages or horizontal progress bars beside the menu-bar icon. Workspace settings → Menu bar
  offers providers with accounts in that workspace, then exactly one account,
  followed by its available limits. First/last name initials can appear to
  the left. Enabled progress limits stack vertically. Missing limits show `—`; usage keeps refreshing with the panel closed.
  Enabling warning colors reveals individual 75/90/95% levels and custom thresholds; the highest enabled crossed level colors all metrics and the menu-bar mark.
- Usage bars support a neutral elapsed-time marker and optional projected pace
  colors for known 5-hour and weekly windows. Missing/expired reset times have
  no marker.
- Codex automatic switching uses a workspace threshold (2% remaining by default)
  and explicitly enrolled accounts. It starts immediately, asking before closing
  Codex when a local thread is running or activity cannot be confirmed.
- Account cards own rename/removal; Add account owns auth.json import.
  Reconnect stays available from the account usage popup. Imports save accounts
  without activation.
- A transparent menu-bar mark with percentage or progress usage metrics. Clicking it toggles the existing panel
  without recreating the webview. Refresh on focus is off by default.

Status refresh runs on startup, on request, at the configured interval while
the panel has focus, and when interval drift suggests wake from sleep. The
default is 60 seconds; the backend accepts 10–300 seconds. Usage requests run
when a usage popup mounts. The backend reads all available quota sources and
the popup filters the result for its provider. Enabled menu-bar usage also
reads its selected source at the configured interval and on status refresh.

Reset outlook requests retain an independent saved refetch interval of
30 seconds to one hour, default five minutes. Workspace Usage controls visibility. The public
API receives no account details or credentials. Successful responses and failures
share the selected cache lifetime. Possible resets are enabled by default and
can be hidden separately from announcements. Expired forecasts disappear;
source freshness accommodates the selected interval.

## Banked resets

Each Codex card shows its account-owned reset balance when nonzero. Usage limits
reviews expiry, affected allowances and eligibility before a separate confirmation.
Inactive accounts work without activation. See [Banked resets](banked-resets.md)
for the API contract, persisted retry IDs and verification boundaries.

## Development

```bash
pnpm install --frozen-lockfile
pnpm run run
```

This starts Vite on **http://localhost:5174/** and the native Tauri dev app.
Use the menu-bar icon to open the actual panel. A browser preview has no Tauri
IPC and is expected to show a settings-load error.

On macOS, `pnpm run run` starts a launchd-owned supervisor so closing Codex
cannot also close Keyturn or Vite during an account restart. The command returns
while dev continues. `pnpm run dev:status` checks that ownership;
`pnpm run dev:stop` stops both services. Logs remain in
`/tmp/keyturn-vite-5174.log` and `/tmp/keyturn-tauri-dev.log`.

If Vite is already running:

```bash
pnpm exec tauri dev --config '{"build":{"devUrl":"http://localhost:5174","beforeDevCommand":""}}'
```

For desktop account-restart testing from Codex, use the independent combined
startup. A dev instance owned by Codex refuses to close its own ancestor.

Keep port 5174 and `strictPort`. Leave `custom-protocol` out of permanent Tauri
features so dev uses Vite and HMR. Do not create a release bundle unless the
user explicitly requests one.

```bash
pnpm exec tsc --noEmit
pnpm exec vitest run
cd src-tauri
cargo check --offline
cargo test --offline
```

See [Testing](TESTING.md) for native panel and HMR checks.

## Local data

| Location on macOS | Purpose |
| --- | --- |
| `~/Library/Application Support/ai.opencode.desktop/agent-rtl.keys` | Launch-at-login flag; the OS is the source of truth. |
| `…/agent-rtl.settings.json` | Refresh cadence, percentage display, menu-bar appearance, multi-profile selection and auto-switch. |
| Webview localStorage: `keyturn.displayPrefs` | Theme, focus refresh and daily automatic update preference; legacy density fields. |
| Webview localStorage: `keyturn.lastUpdateCheck` | Timestamp of the latest successful update check. |
| `…/keyturn-claude-profiles/profiles.json` | Anthropic profile names and Claude configuration-directory references. |
| Webview localStorage: `keyturn.skippedUpdateVersion` | Skipped updater version. |
| `~/.local/share/opencode/auth.json` | Read-only OpenCode credentials and connected services. Honors `XDG_DATA_HOME`. |
| `~/.codex/auth.json` | Codex identity and OAuth access token. Explicit profile activation replaces this cache atomically. Honors `CODEX_HOME`. |
| `…/keyturn-codex-profiles/profiles.json` | Profile IDs and names; independent of legacy app-data cleanup. |
| `…/keyturn-codex-profiles/<id>/auth.json` | Each profile's private ChatGPT login cache. Credentials never cross IPC. |
| `~/.claude/projects/**/*.jsonl` | Read-only Claude quota snapshots. Honors `CLAUDE_CONFIG_DIR`. |
| `~/.gemini/oauth_creds.json` / Gemini environment variables | Read-only Gemini authentication detection. |

`KEYTURN_OPENCODE_USERDATA` overrides Keyturn's application-data directory for
isolated testing. On startup, recognized legacy keys/accounts/profile-era
preferences are removed from Keyturn's own store/settings files. Named Codex
profiles use their own directory and survive that cleanup. Profile directories
use mode 0700 and credential files use 0600 on macOS. Files contain tokens in
plaintext, like Codex's own file store; keep them out of source control and logs.

## Codex profiles

Open **Codex profile** at the top of the panel (also reachable through Add
account). Choose Add profile and enter a name. Then choose:

- **Import auth.json file:** paste its full path in the in-app form (absolute or starting with `~/`). Keyturn
  validates it and saves a private copy; the source and active account stay intact.
- **Sign in to another account:** Continue with browser launches the installed
  Codex CLI's sign-in for this isolated profile. Keyturn detects completion.
  Cancel stops the child process. Terminal and device-code instructions remain
  available as fallbacks.
- **Save current Codex account:** keep the existing login as a named profile.

Manual Switch starts the graceful desktop replacement immediately on macOS,
without a countdown. Keyturn quits Codex gracefully, installs the selected login
after exit and reopens it. Automatic switching also has no countdown. If
a local thread is running, a confirmation offers **Close and switch** and
**Cancel**; uncertain activity gets a separate explanation and the same choice. No force quit is used; refusal to close aborts replacement.
A missing desktop app blocks restart.
The running app's bundle path is captured before closing and reopened directly.
**Activate for CLI only** is available under Manage for a file-only switch.
New CLI processes use the selected cache; existing sessions should be reopened.

Manage an existing profile to **Replace login from file** when a manager sends
updated credentials. Imported replacements remain pending until activation;
refreshing the list cannot overwrite their new tokens with the old active cache.

Activation changes only `auth.json`, preserving Codex configuration and chat
history. A current account that has not yet been saved is preserved as a
Previous account profile. Removing a profile forgets its saved login without
logging out the active CLI. External CLI sign-ins are reflected on refresh;
profile identity uses the token's subject and workspace, not its email.

The close/replace/reopen workflow is based on this user's observed desktop
setup, not a universal desktop switching API. Restart verification checks that
the active login file matches and a new desktop process is running. The account
displayed inside Codex still needs to be checked; no remote token validity or
in-memory desktop identity is claimed. Versions using a separate sign-in may
require their own login. If replacement fails after quitting, Keyturn reopens
Codex with the old login. A reopen failure leaves a recovery profile available.
ChatGPT API-key logins
and non-file credential stores cannot currently be imported or activated.

See [Account-method research](research/codex-account-methods.md) for the
comparison of browser/device-code login, cache import, Keychain, API keys,
enterprise tokens, app-server and Sign in with ChatGPT integrations.

## Anthropic profiles

Choose **Anthropic profiles** inside Codex profiles, or select Anthropic in the
new-profile form. Track the current Claude configuration or create an isolated
one. Run the supplied sign-in command in Terminal, then choose **Test connection /
Rescan**. Start Claude with the profile's **Use this account** command.

Isolation uses the CLI's documented `CLAUDE_CONFIG_DIR` workflow. Claude owns
credentials, Keychain entries, settings and conversation history. Keyturn stores
metadata and reads subscription usage with the selected profile’s existing login;
it does not replace Claude’s global credentials. Forgetting a tracked profile preserves its CLI directory and login.
Codex auto-switch applies only to Codex profiles.

See [usage preferences](USAGE_PREFERENCES.md) for defaults and alert/pace
behavior, and [Claude authentication](https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts)
for the configuration-directory workflow.

## Docs

- [Frontend/backend contract](../CONTRACT.md)
- [Adding a provider](ADDING_A_PROVIDER.md)
- [Testing](TESTING.md)
- [Release workflow and packaging templates](RELEASING.md)
