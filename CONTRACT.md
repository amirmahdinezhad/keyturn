# Keyturn — frontend/backend contract (v5, workspaces)

Backend: Tauri v2 Rust commands. Frontend: React 19, TypeScript, Tailwind v4
and local Shadcn Studio components backed by Radix. Variants use
`class-variance-authority` and `clsx`. Icons use the iconsax Linear set plus
local brand/usage marks. The tray uses the transparent alpha mask from
`src-tauri/icons/tray-template.png`, tinted with the selected panel accent.

## Workspaces

The workspace selector leads the panel. First-run setup collects name/accent,
then explicitly loads and selects paid accounts. Optional step three chooses a
provider and sign-in method; multiple verified drafts are committed once without
activation. Saved accounts can be shared across workspaces, with independent
Codex account preferences. Existing Codex accounts
migrate into Personal. Each workspace shows its own saved account cards, including
inactive accounts. Workspace settings have General / Usage / Menu bar tabs;
machine settings retain global settings. Workspace Menu bar chooses Codex, Claude or
Antigravity before configuring each account’s reported limits. The global Codex
account section is removed. See [Workspaces](docs/WORKSPACES.md) for ownership,
migration, personal limit countdowns, quick switching and five workspace commands.

Workspace usage adds `menuBarProvider` (default `"codex"`), `accountWindows`
(account id → selected limit keys), `warningColors` (default false), and
`autoSwitchRemaining` (0–100, default 2). All three Display switches default on.
The old `displayMode` field remains readable for compatibility; the workspace UI
uses per-limit toggles. Warning colors independently tint both metrics and the
tray mark at the enabled consumed-capacity levels (default 75/90/95%). Automatic switching is Codex-only and both
source and destination must be enrolled in the cycle.

Setup account discovery includes optional `displayName` for the Codex profile
name in the local ID token (`name`, with the namespaced profile claim as a
fallback). The load-account list displays it above the email, falling back to
`name` when absent. Newly discovered ambient Codex accounts use that profile
name as their initial saved label. Existing custom labels and identity matching
are unchanged; no extra profile request or credential disclosure is introduced.
Codex auth.json import has no name input: the native validator chooses the
profile name, then email, then `ChatGPT account` as its initial label (64-character
limit). Duplicate labels receive a numeric suffix. Confirmation shows the label,
profile name (when different), email and plan. Replacing a saved login preserves
its current label without asking for a name. Saved Codex profiles also expose optional `displayName`, derived
from their own stored login on every read, so it survives a panel/app restart
without being confused with the current CLI account or overwriting custom labels.

Workspace `usage.activeAccounts` stores independent defaults keyed by `codex`,
`claude` and `antigravity`; there is no cross-provider active-account ID. Setup
accounts expose `active`, resolved per provider from its current loaded login,
then its saved default, then its first available account. Commit translates draft
IDs to saved IDs and preserves existing valid defaults when adding backups.
Removal repairs only missing defaults. Current provider login metadata takes
priority over a saved default. Codex’s `activeProfileId` remains Codex-only and
continues to track the real CLI cache for credential switching and auto-switch
safeguards. Loading/selecting defaults does not rewrite credentials.

## Authentication and status

Provider CLIs own Codex and Claude authentication. Keyturn starts their official
login flows only on request, with isolated native configurations. Codex supports
browser, device code and auth.json import; Claude supports browser/CLI login or
an existing configuration. Experimental Antigravity uses a loopback Google OAuth
flow modeled on CodexBar, with state checking, bounded requests and server-side
identity/paid-plan verification. No embedded sign-in webview is opened. Explicit
Codex profile actions save private login files and activate a selected account;
manual import accepts credential JSON only in the frontend-to-native direction.
Anthropic profiles track separate CLI configuration directories. Confirmed ended
OpenCode subscriptions are removed from OpenCode's saved connections; the
keychain is never modified.

| Provider | Binary | User runs | Keyturn probes |
| --- | --- | --- | --- |
| Claude | `claude` | `claude auth login` | `claude auth status --json` |
| OpenAI Codex | `codex` | `codex login` | `codex login status`, read-only auth-file identity |
| OpenCode | `opencode` | `opencode auth login` | `opencode auth list`, auth-file service IDs |
| Gemini | `gemini` | `gemini` | OAuth file or environment credentials |

Binary lookup uses `PATH` and common installation directories. Version
subprocesses have an 8-second timeout; status subprocesses use 10 seconds.
They run on a blocking worker, outside the async runtime.

## IPC shapes

```ts
interface AppSettings {
  workspaceVersion: number // 1 after the migration check
  activeWorkspaceId: string | null
  workspaces: Workspace[] // metadata only; docs/WORKSPACES.md
  refreshIntervalSecs: number // 10..300, default 60
  usageShowRemaining: boolean // default true; remaining counts down from 100%
  menuBarEnabled: boolean // default false; percentage metrics beside tray mark
  menuBarProvider: "codex" | "claude" | "opencode" | "opencode-go" // default codex
  menuBarWindow: "auto" | "5-hour" | "weekly" | "monthly" // default 5-hour
  providerAccents: Partial<Record<"codex" | "claude" | "opencode" | "opencode-go" | "gemini", string>> // optional #rrggbb identity colors
  usageAppearance: UsageAppearance
  multiProfile: MultiProfileSettings
  autoSwitchEnabled: boolean // default false; Codex-only immediate switch with active-thread confirmation
  codexReset: { showProbableResets: boolean; refetchIntervalSecs: number } // true, 300; 30..3600
}
interface AppState {
  settings: { openAtLogin: boolean }
  app: AppSettings
}
interface ProviderCliStatus {
  id: string
  label: string
  binary: string
  binaryFound: boolean
  binaryPath?: string | null
  version?: string | null
  loggedIn?: boolean | null // absent/unknown if the probe cannot determine it
  authMethod?: string | null
  email?: string | null
  account?: string | null
  detail?: string | null
  loginCmd: string
  installUrl: string
  authFile?: string | null
  connected?: string[] // OpenCode services; omitted when empty
  subscriptions?: Subscription[] // service/plan metadata; omitted when empty
}
interface Subscription {
  id: string // chatgpt | claude | OpenCode service id
  service: string // subscription product, separate from the CLI provider label
  plan?: string | null
  monthlyPriceUsd?: number | null
  expiresAt?: string | null // billing expiry only; never OAuth/session expiry
  status?: "active" | "expired" | "unknown" | null
}
interface QuotaWindow {
  window: string
  percent: number // USED, independent of the UI preference
  resetsAt?: string | null
  status?: string | null
}
interface QuotaSet {
  provider: string // opencode | opencode-go | claude | codex
  windows?: QuotaWindow[] | null
  error?: string | null
  asOf?: string | null // observation time for a local Claude snapshot
}
interface CodexResetStatus {
  latestReset: {
    id: string
    resetType: "regular" | "banked"
    announcedAt: string // source announcement / first observed completion
  } | null
  scheduledReset: {
    resetType: "regular" | "banked"
    announcedAt: string
    scheduledFor: string | null
  } | null
  activeWatch: {
    level: "elevated" | "strong"
    resetChancePercent: number | null
    forecastWindow: string
    observedAt: string
    expiresAt: string
  } | null
  generatedAt: string
}
```

Rust omits absent optional status fields and empty `connected` arrays. The
frontend tolerates missing/null optionals. These types mirror `src/types.ts`
and `src/api.ts`; integration tests pin the status/settings field names.

| Invoke command | Result / behavior |
| --- | --- |
| `get_state()` | `AppState`; reconciles launch-at-login with the OS and compacts legacy app data. |
| `cli_status()` | `ProviderCliStatus[]` for all four CLIs; removes confirmed expired OpenCode connections. |
| `cli_quotas()` | `QuotaSet[]` from all available supported sources, sorted by provider. |
| `codex_reset_status()` | `CodexResetStatus` from the public Codex Resets API; source failures reject. No account data is sent. |
| `codex_account_status()` | Current Codex login metadata, including token expiry and last refresh; no credential values. |
| `codex_account_quota()` | Queries only the active Codex usage source for Test connection. |
| `codex_profile_quota({ id })` | Queries a saved Codex account without activation. |
| `import_codex_profile_json({ name, credentials, replaceId })` | Validates pasted auth.json natively; saves an isolated private copy and returns profile metadata. Active credentials stay intact. |
| `claude_profile_quota({ id? })` | Live read-only Claude subscription usage for a saved config; omitted id selects the default CLI config. Errors or a timestamped local snapshot are returned when unavailable. |
| `claude_profiles()` | Tracked Claude configs, connection metadata, login/launch commands, current-config flag and local quota snapshots. |
| `create_claude_profile({ name, useCurrent })` | Creates an isolated config or tracks the current config. Blank names get random labels. |
| `rescan_claude_profile({ id })` | Runs Claude's status command with that profile's CLAUDE_CONFIG_DIR. |
| `edit_claude_profile({ id, name })` | Renames metadata, or forgets it when name is null; the CLI directory is preserved. |
| `codex_profiles()` | `CodexProfilesState`; reads profiles and saves the active account's latest tokens to its existing profile. |
| `create_codex_profile({ name, importCurrent })` | Creates a named isolated login directory, optionally copying the current ChatGPT login; returns profile state. |
| `rename_codex_profile({ id, name })` | Renames the profile without changing credentials. |
| `remove_codex_profile({ id })` | Deletes the saved profile/login; active CLI credentials remain intact. |
| `activate_codex_profile({ id })` | Preserves the previous account, atomically replaces Codex auth.json, invalidates native quota cache and returns profile state. |
| `import_codex_profile({ name, path, replaceId })` | Imports from an absolute or `~/` path entered in the custom form; returns profile state or a safe error. Validates/copies a supplied file without changing the active cache. |
| `start_codex_profile_login({ id })` | Starts the installed CLI's browser login in isolated CODEX_HOME; returns lifecycle status only. |
| `codex_profile_login_status({ id })` | `profileId`, status `idle/pending/completed/failed`, and sanitized error. |
| `cancel_codex_profile_login({ id })` | Kills/waits for the isolated child; leaves the active account untouched. |
| `schedule_codex_restart({ id, allowLimited? })` | Validates file-backed profile and installed macOS desktop app; starts manual switching immediately without a countdown. Automatic jobs also have zero grace; running or unknown activity pauses before quit for confirmation. Optional `allowLimited: true` acknowledges capacity for this job only; session validation remains required. |
| `codex_restart_status()` | `CodexRestartStatus` or null; survives hidden/closed profile modal. |
| `cancel_codex_restart()` | Cancels checking/awaiting-confirmation jobs before quit, preserving the saved profile and active login. |
| `run_codex_restart_now()` | Triggers a pending restart without waiting for its deadline. |
| `set_settings({ openAtLogin })` | Applies the OS preference and returns reconciled `AppState`. |
| `set_app_settings({ settings })` | Merges partial app preferences, normalizes cadence and returns `AppState`. |
| `set_menu_bar_accent({ accent })` | Validates a three- or six-digit hex color and repaints the native mark and monochrome metrics from cached usage. No quota request or settings write. |
| `quit_app()` | Exits the native app. |

## Usage sources

The panel header leads with the selected workspace. Codex cards lead with the
saved account name; other cards keep the CLI provider. The subscription service (for example,
ChatGPT) appears beside a shared plan badge in the footer. Badge color and capacity
marks distinguish entry, Plus, $100, $200 and $500 tiers; free, organization,
usage-based and unknown plans have explicit fallback treatments. Catalog
prices are standard monthly USD prices, not the user's invoice amount:
[ChatGPT Pro tiers](https://help.openai.com/en/articles/9793128-about-chatgpt-pro-tiers),
[Claude plans](https://support.claude.com/en/articles/11049762-choose-a-claude-plan),
[OpenCode Go plans](https://opencode.ai/docs/go/).

Subscription expiry comes from ChatGPT's `chatgpt_subscription_active_until`
claim or explicit provider billing metadata. Expired services leave the main
list, including when their deadline passes while the panel remains open.
Other active services on the same CLI card remain visible. A confirmed expired
OpenCode service is deleted from its auth.json on status refresh, so restarting
Keyturn cannot rediscover the old connection. Cleanup rereads the file, preserves
other entries and skips credentials replaced during the probe. Write failures
are logged and retried on a later refresh. Reconnecting requires adding the
service again. Saved expired Codex/Claude profiles remain hidden.
JWT `exp`, OAuth expiry, quota limits, network errors and generic HTTP 401/403
responses never count as subscription expiry. Go billing metadata is read
from its existing usage endpoint with a four-second timeout, no redirects,
and a per-key 60-second memory cache. Requests identify themselves as Keyturn.
The explicit `EntitlementError` / `OpenCode Go subscription required.` response
means no active subscription and hides that service. Generic transport/WAF
errors preserve the same key’s last confirmed active/expired state; real usage
windows restore active status after renewal. Exhausted usage windows alone do
not expire a subscription. Without billing evidence, plan/expiry stay unknown;
a Go API key alone cannot distinguish Go from Go Plus. Claude plan type and
capacity are separate from its org name.

A provider usage popup requests `cli_quotas` when mounted and on the app's
configured refresh cycle. The backend reads **all** available sources;
`QuotaContent` filters by the popup's provider IDs. Saved Codex account popups
share the workspace's account-specific quota snapshots. Open popups update
without closing/reopening; superseded requests cannot restore older percentages.
When menu-bar usage is enabled, a native worker separately reads only its
selected source on startup, at the refresh interval and on status refresh.
It continues with the panel hidden and caches the latest selected-source
result for immediate window changes. Changing provider or disabling usage
clears that cache and rejects in-flight results from the previous selection.

Menu-bar metrics always use percentages, with configurable used/remaining
wording and labels: `5h 72%`, `1w 19%`, or `1mo 50%`. Session/primary and weekly
metrics can appear together. On macOS, system-font text is drawn at retina
resolution into a transparent tray image; monochrome uses the panel accent,
while color mode uses consumed-capacity thresholds. Template rendering stays
disabled so macOS preserves the accent. Profile initials are optional; neither
numeric nor progress mode renders an active underline. Auto picks the supported main window with the least
capacity left; model-specific and reserve buckets are excluded. Missing,
failed, non-finite or expired-reset windows show `—`. Disabling single-source
usage clears it; independently enabled multi-profile display or
auto-switch keep their required native monitor work running.

`UsageAppearance` and `MultiProfileSettings` mirror
`src/types.ts` and `settings.rs`. See [Usage preferences](docs/USAGE_PREFERENCES.md)
for their exact fields and defaults. Settings loaded from older files receive
new defaults; unknown legacy fields are compacted without dropping new settings.

- **OpenCode:** `quota.rs` reads `opencode` and `opencode-go` keys from the CLI
  auth file and requests the corresponding Zen `/usage` endpoint. It parses
  rolling, weekly and monthly windows. 401 means the key was rejected; 403
  shows the no-quota error. Requests time out after 10 seconds.
- **Codex:** `codex.rs` reads a stored OAuth access token/account ID;
  `quota.rs` requests `https://chatgpt.com/backend-api/wham/usage`. No OAuth
  token means no source. Saved-account usage renews expired/rejected sessions
  once through a short-lived Codex app-server (`account/read`, `refreshToken: true`)
  in the owning login directory, then retries usage. Renewal is serialized with
  profile changes and persists CLI-rotated tokens, including the active account's
  saved snapshot. Rejected renewals offer Reconnect account → Replace login;
  replacement refetches usage without activation. Direct provider probes remain
  read-only. Window labels derive from duration;
  `gpt-reserve` is hidden. Requests time out after 10 seconds.
- **Claude:** Usage popups call `claude_profile_quota` on demand and on the app
  refresh cycle. The backend reads only that config’s Claude OAuth access token
  (directory-scoped macOS Keychain or `.credentials.json`) and requests
  `https://api.anthropic.com/api/oauth/usage`, with a ten-second timeout and no
  redirects. Tokens and provider response bodies never cross IPC; credentials
  are never renewed or modified. Live responses include their observation time.
  A failed request falls back to the newest local transcript snapshot with its
  original `asOf`; otherwise the UI displays the specific failure. Metadata lists
  and menu-bar monitoring retain their local-only snapshot reads.
- **Gemini:** no quota source or usage button.

The UI shows exact percentages and reset countdowns. Remaining mode inverts
used percentages and clamps them to 0–100; warning colors still derive from
used percentages. Missing data and source errors are shown explicitly.

### Public Codex reset outlook

`codex_resets.rs` reads `https://codex-resets.com/api/v1/status` independently
of quota requests. It validates the v1 response, dates and probability bounds;
requests use an eight-second timeout, HTTPS and no redirects or credentials.
A process-local cache shares success/failure results for the saved
`codexReset.refetchIntervalSecs` (30–3600 seconds; default 300). The API contract is documented at
[Codex Resets API](https://codex-resets.com/api/docs).

The workspace Codex section mounts exactly one notice when `usage.showCodexResets`
(default true) is enabled. No per-account card polls or displays reset updates.
Updates arrive on mount, quota refresh, focus, reconnection and an independent
timer. A future announcement takes precedence; otherwise a fresh confirmed reset
from the last 24 hours shows its local confirmation time. An expired countdown
alone never confirms completion. Optional possible-reset watches follow when
neither announcement nor completion is present. Banked credits keep explanatory
wording. Disabling the master unmounts the notice and stops requests.

Closing an announcement saves its type, announcement timestamp and scheduled
timestamp in `keyturn.dismissedCodexAnnouncements`. Refresh/restart does not
reopen that announcement; a new announcement can appear. The bounded history
retains up to 100 dismissed identities. Confirmed-reset dismissal uses `completed:<id>`; old history is ignored. Local-storage failures still allow dismissal for the current session.

Without a current announcement or recent confirmation, `showProbableResets` (default true) enables
active, unexpired watches. They show Possible reset, the nullable probability
and a live countdown to the forecast window's upper bound (`expiresAt`), with
today/tomorrow (or a calendar date) and local hours/minutes. This bounds the
possible window rather than promising an exact execution time. Turning the switch
off hides estimates immediately while keeping announcements. A current
announcement takes precedence. Dismissed watches use observed/expiry timestamps
so changing odds do not reopen them. A new watch can appear.

No reset, failed requests, stale snapshots and expired watches render no notice
or placeholder. Freshness is at least 15 minutes, extended to the selected
interval plus one minute for long cadences. Notices contain no source link.

## Persistence and panel lifecycle

### Codex profile contract

```ts
interface CodexProfile {
  id: string // canonical UUID, never derived from the editable name
  name: string // trimmed, 1–64 characters, unique ignoring case
  email: string | null
  plan: string | null
  subscription?: Subscription | null
  connected: boolean // locally saved, structurally valid ChatGPT login
  connectionError: string | null // duplicate account; choose another sign-in
  active: boolean // identity matches the current file-based Codex login
  needsActivation: boolean // imported/replaced file is waiting for Set active
  loginCommand: string // isolated CODEX_HOME, properly shell-quoted
}
interface CodexProfilesState {
  profiles: CodexProfile[]
  activeProfileId: string | null
  canImport: boolean
  switchError: string | null
}
interface CodexRestartStatus {
  profileId: string
  profileName: string
  deadlineAt: string // ISO timestamp
  phase: "checking" | "awaiting_confirmation" | "countdown" | "quitting" | "activating" | "reopening" | "verified" | "failed" | "cancelled"
  error: string | null
  processId: number | null
  credentialsVerified: boolean // local file match, not remote token validity
}
```

The new `keyturn-codex-profiles` namespace is separate from legacy settings
cleanup. Its metadata index holds names and IDs only. Profile directories
are owner-only (0700); JSON credentials are written atomically via a 0600
temporary file and never returned over IPC or included in errors. They are
plaintext tokens, not encrypted. The CLI owns browser login and refresh.

File selection is native Rust only; no frontend filesystem permission is
granted. Import validates a regular JSON file up to 256 KB before storing
its supported ChatGPT payload. It never changes the source or active cache.
Replacement tokens have a persisted needsActivation flag so the old active
snapshot cannot overwrite them. Set active clears that flag after the file
transaction. Previous saved/active caches are retained as private recovery files.

The browser-login child receives an isolated CODEX_HOME and explicit file-store
override as arguments, without a shell. Its output is discarded, not sent to
IPC. Only one managed login runs at once. Polling detects exit, cancellation
reaps the child, a ten-minute deadline limits attempts, and app exit stops the
callback process. Mutations to that profile are blocked while login is pending.
The UI updates after completion and verifies saved metadata before offering
Set active. Browser authentication itself remains the user's action.

Only file-based ChatGPT logins are currently supported. Non-file storage,
invalid Codex configuration and forced API-key login block activation. Forced
workspace restrictions are checked before any active credential replacement.
The active identity is subject plus workspace, and changes outside Keyturn
are reflected on refresh. Switching saves refreshed current tokens and
preserves an unsaved account as Previous account. It detects a concurrent CLI
credential change before replacing the active file and rejects the switch.
Config/history are not replaced. Deletion does not log out the active CLI.

Activating closes any old-account quota popup and rejects stale provider
responses/native quota probes. New CLI processes use the selected login;
running sessions should be reopened. Manual Switch starts the native job
immediately, as do automatic jobs. Running or unknown local-thread activity
requires Close and switch or Cancel before quit. Cancellation and committing the quit phase share a lock.
Only com.openai.codex is requested to terminate gracefully; no force kill is
used. Credentials are installed after the old app exits, then Codex is reopened.
The native driver retains the running app's bundle URL and reopens that exact
path in the foreground. An ancestry check before scheduling and again before
quitting refuses a restart if Codex owns the Keyturn process; otherwise host
shutdown could kill the worker before activation. This check leaves auth intact.
An activation error reopens the prior login. Missing app/unsupported OS rejects
scheduling; Activate for CLI only remains separate. Verification covers the
active file identity and newly running process, not in-memory desktop account
identity. Other desktop versions may use a separate sign-in. Connection checks
do not prove that a remote token remains accepted. A native completion event
invalidates quota caches; UI polling refreshes identity even if the modal closed.

- `agent-rtl.settings.json` stores refresh cadence, quota wording and menu-bar preferences.
  Reading it normalizes values and rewrites legacy unknown preferences.
- `agent-rtl.keys` stores only launch-at-login. The OS is the source of truth;
  recognized legacy key/account/theme sections are compacted on startup.
- `displayPrefs.ts` stores the global theme, focus refresh and opt-in automatic
  update preference in `keyturn.displayPrefs`, retaining legacy density fields.
  Global settings exposes General and Updates tabs. Workspace metadata owns accent; workspace usage owns menu-bar
  status colors. App mirrors the workspace accent to native state on mount.
- Paths are defined in `paths.rs`; `KEYTURN_OPENCODE_USERDATA` overrides app
  data, `CODEX_HOME` overrides Codex auth, `XDG_DATA_HOME` overrides OpenCode
  auth and `CLAUDE_CONFIG_DIR` overrides Claude transcript home.
- The tray toggles the existing main webview and anchors it under the icon,
  clamped to the monitor. Losing focus hides it. File imports use the custom in-app path form.
  Opening never reloads it.
- Status refresh reads settings and all CLI statuses on mount/manual refresh,
  at the configured interval while focused, or after detected timer drift.
  Refresh on focus is optional and off by default. Vite HMR updates dev UI.

## UI behavior

- Header: workspace selector, create workspace, refresh age, Refresh, Global
  settings and Quit. Workspace settings has its own panel button.
- Codex account cards show the selected workspace's saved logins, active status,
  explicit activation, usage and account management. Exhaustion adds an inline
  personal recovery countdown and a quick-switch icon.
- Main list: connected accounts only (`binaryFound && loggedIn === true`),
  count, identity and readable auth/plan/services. Confirmed expired subscriptions
  are hidden, as are CLI versions and connection dots. Paths
  and internal account IDs stay out of cards. Add account is a floating button.
- Add account: pick a provider, then Install if missing, Sign in and Done.
  The stepper begins after provider selection. Fresh status validates each
  stage; the wizard never executes installation or login commands.
- Usage: a card icon opens account/provider quotas; exhausted main Codex limits
  also render inline. Remaining Codex
  percentages use `round(clamp(100 - used, 0, 100))` in the popup and native tray,
  including half-point cases such as 6.5% remaining becoming 7%.
- Codex section: one compact reset update above account cards. Workspace Usage
  controls its master toggle and optional probability forecasts. Confirmed events
  appear for up to 24 hours; no update is inferred merely from a passed deadline.
- Global settings has General / Updates tabs. General owns Theme, launch/focus/refresh settings and a scoped global reset.
  Workspace settings has General, Usage and Menu bar tabs. Menu bar owns account/limit selection, initials,
  display style and limit colors; Usage owns reset updates/predictions and switching.
- Loading skeleton until settings arrive, explicit load/probe errors and toast
  feedback. A plain browser has no Tauri IPC and shows the expected load error.
- Updates offers checks, release notes, skip, download progress and explicit
  install/relaunch. Opt-in daily checks/downloads run while Settings is closed;
  installation waits for user action. Duplicate update operations are guarded.
  The last successful check is stored as `keyturn.lastUpdateCheck`.
- The release workflow uses the existing updater signing key, creates a draft
  universal macOS release with English/Persian notes and `latest.json`, and
  requires owner approval before publication. See `docs/RELEASING.md`.

## Structure

`src/App.tsx` coordinates status refresh and modals. `src/api.ts` owns invoke
wrappers; `src/types.ts` mirrors backend preferences. `src/components/ui/`
contains locally owned Radix components with source/license notes. CSS tokens
and base component styles live in `src/index.css`; `src/panel.css` consolidates
the shared desktop design described in `docs/DESIGN.md`. Rust modules own CLI probing,
quota sources, local settings/store persistence, paths and the tray shell.

Removed legacy functionality includes general key switching, console login,
history/export, threshold notifications, sounds and automatic key switching.
The old `account`, `bridge`, `auth`, `providers`, `notify`, `history` and `usage`
modules are absent. The new `profiles` and `profile_login` modules manage the
Codex-specific account workflow described above.

## Development workflow (standing rules)

- **Test locally, always.** Verification happens on the dev build
  (`pnpm run run`) — never on the installed app. On macOS the combined dev
  supervisor is owned by launchd, independent of the Codex process that starts
  it. `pnpm run dev:status` verifies parent PID 1; `pnpm run dev:stop` stops it.
- **Never ship a release build unless asked.** No `pnpm tauri build`, no DMG,
  no copy to Desktop or `/Applications`, until the user explicitly says
  "take an output" (خروجی بگیر). Release builds are slow, flaky on this
  toolchain, and overwrite what the user is testing.
- **After every task, restart dev** without asking (kill the old
  `target/debug/keyturn` + vite first; single-instance would otherwise
  shadow the new launch). For supervised dev, use dev:stop, wait for shutdown,
  then run the combined startup again.


After a verified native switch, `codex-profile-switched` carries the destination
profile ID. The mounted panel immediately updates its single Active indicator
before refreshing metadata; countdown/cancel/failure never move it. The native
completion path also saves the Codex default for workspaces containing that ID.
Enrollment validates measured usage for Codex, Claude and Antigravity before
returning a selectable account and again on commit. Missing/failed usage omits
the account from discovery; explicit import shows its validation error. A valid
100% limit is still loadable and is not treated as a data-loading failure.


Claude profile listing rechecks each saved configuration's current usage in
parallel, outside the profile-index lock. Its connected/quota response reflects
that live read, without transcript fallback. Home cards exclude disconnected or
failed Claude profiles and do not resurrect the same default configuration as
an ambient CLI card. On a subsequent successful refresh, the saved card returns.
Credentials and membership are retained for recovery and management. Enrollment
uses the same strict current-usage read; historical usage remains available for
other existing snapshot-capable usage surfaces.


Workspace settings contains General, Usage and Menu bar. Account cards keep their
own rename/removal settings. Saved-session recovery from the quota popup opens
a separate Reconnect account modal with fresh auth.json import and leaves the
current login intact until activation. Removing the workspace Accounts tab does
not remove account credentials or membership.

Workspace menu-bar rendering resolves a provider with workspace members and one
`usage.menuBarAccountId` within that provider. An invalid or absent selection
falls back to the provider’s active member, then its first member. The native
tray filters to that single account even for legacy multi-account settings.
Provider/account selection precedes account-specific limit switches. Workspace
Account recovery is independent of workspace tab navigation.

Global settings has General and Updates; Theme is in General. Workspace Usage
owns Codex reset visibility and `showAccountName` immediately after the menu-bar
account picker. Limit colors expose per-level switches and custom thresholds when enabled.
Native menu-bar captions use the first/last name initials, with the full name in
the tooltip. Metric and mark colors use the highest enabled crossed workspace
threshold. Presentation changes invalidate stale requests while retaining the
current usage sample for an immediate repaint; identity changes clear it.

Workspace `usage.menuBarStyle` persists `percentages` (default for old settings)
or `progress`. Native metrics carry numeric fractions independently of formatted
text. Progress fills follow used/remaining direction, share the highest enabled crossed warning
color and stack one row per enabled window; a single row is larger than a
stacked row. Missing fractions
render as unavailable, not as zero. Account initials are left of both numeric
and progress content; full identity and values remain in the tooltip. Neither
mode draws a bottom line.

Menu-bar warning tint is account-wide: the highest crossed enabled threshold
from a selected, current limit supplies the color for the mark, initials, all numeric/window labels, and all progress tracks/fills. Disabling
warning colors or recovering below all enabled thresholds restores identity
colors. Missing and expired samples never trigger warning tint.

Manual `schedule_codex_restart` requests start in `checking` with zero grace;
automatic requests also use zero grace and start in `checking`. Running or
unknown activity transitions either path to `awaiting_confirmation`.
Successful switching records provider-active defaults and selects that account
in the current workspace’s menu bar only when its provider matches. The native
monitor receives the saved settings immediately. Failed restarts never publish
the attempted account as completed. The destination’s explicit window selection
is retained; a previously unconfigured destination inherits enabled limits that
its learned window support allows. Left-side initials and percentages both use
the native 25px font at 2× scale.

Before quit, after credential validation, the native worker checks lifecycle
markers only in sessions held open by the live Codex desktop backend. It uses
process ancestry, open rollout files and Codex Desktop origin metadata to exclude
stored inactive chats and unrelated CLI sessions. `task_started` without a later
completion/abort counts as active. Parse/probe uncertainty blocks quit and is
labelled honestly. Only count/uncertainty reach the webview; no chat text is sent.
The attention event opens the existing tray panel. Confirmation continues the
same validated job, while cancellation leaves app and credentials unchanged.
Automatic attempts remain deduplicated per source/reset period after cancellation.
These local rollout details are version-dependent, not a public cross-client API.
Official lifecycle reference: https://learn.chatgpt.com/docs/app-server

After a running-thread confirmation, destination login validation runs again
before quit. A session rejected while the popup was open cannot close the
working app or replace its credentials.

Yellow warning uses #FFD60A in dark native appearance and a deeper #8A5300 amber
in light appearance. The native window/system theme, independent of the CSS
panel theme, selects the palette. The mark, initials, text, tracks and fills keep
a uniform warning hue. Native theme changes repaint cached data without a quota
request. Numeric and progress displays never render an active underline, even
when legacy showActiveIndicator is saved as true.

Progress metrics carry `time_marker: Option<f64>` separately from consumed/
remaining capacity. Native calculations match `usagePace` for 5-hour/rolling and
weekly reset periods. Valid markers render on every progress row, inverting
with remaining wording. Unknown durations and invalid reset data have no marker.
Single-limit bars are 96×22 physical pixels at 2× scale; stacked bars retain
72px width. Neutral outlined time ticks are independent of warning tint and do
not restore the removed active underline. Numeric metrics ignore marker data.

### Removal of usage alerts

Usage threshold notifications and session-reset notifications are removed from
workspace, account and app contracts. Native history persistence, alert-only
probes and delivery are deleted. Serde discards old notification/alert keys and
subsequent writes omit them. Menu-bar warning colors use independently saved `usage.warningThresholds`;
automatic-switch status notifications are retained.

### Menu-bar color details

`usage.warningThresholds: number[] | null` stores enabled color levels per
workspace. Missing/null defaults to [75,90,95]; [] keeps identity colors at all
usage levels. Values normalize to sorted unique integers 1–100. When Change
colors near limits is on, Menu bar shows level switches, colored dots and a
compact custom threshold form. Turning the master off preserves selections.
Native rendering uses these same levels for labels, metrics and icon, retaining
the current usage sample on level changes for immediate repaint. This adds no
system notifications or alert history.

### Workspace accent across native metrics

The active workspace accent overrides the selected account's color only in its
resolved native menu-bar presentation. Logo, initials, window labels, available
percentages and progress fills/tracks share that accent. Edits repaint cached
usage immediately without waiting for a quota request. Enabled warning levels
still override the whole bar; time-marker ticks stay neutral and unavailable
values remain muted. Saved account/provider colors and legacy no-workspace
rendering retain their previous behavior.
