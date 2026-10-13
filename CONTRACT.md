# Keyturn — frontend/backend contract (v3, CLI-only)

Backend: Tauri v2 Rust commands. Frontend: React 19, TypeScript, Tailwind v4
and local Shadcn Studio components backed by Radix. Variants use
`class-variance-authority` and `clsx`. Icons use the iconsax Linear set plus
local brand/usage marks. The tray uses `src-tauri/icons/tray-template.png`
as a macOS template icon.

## Authentication and status

Provider CLIs own authentication. Keyturn never runs login/install commands,
opens a sign-in webview, patches credentials or writes to the keychain. It
shows copyable instructions and probes installed CLIs read-only.

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
  refreshIntervalSecs: number // 10..300, default 60
  usageShowRemaining: boolean // default false
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
```

Rust omits absent optional status fields and empty `connected` arrays. The
frontend tolerates missing/null optionals. These types mirror `src/types.ts`
and `src/api.ts`; integration tests pin the status/settings field names.

| Invoke command | Result / behavior |
| --- | --- |
| `get_state()` | `AppState`; reconciles launch-at-login with the OS and compacts legacy app data. |
| `cli_status()` | `ProviderCliStatus[]` for all four CLIs. |
| `cli_quotas()` | `QuotaSet[]` from all available supported sources, sorted by provider. |
| `set_settings({ openAtLogin })` | Applies the OS preference and returns reconciled `AppState`. |
| `set_app_settings({ settings })` | Merges partial app preferences, normalizes cadence and returns `AppState`. |
| `quit_app()` | Exits the native app. |

## Usage sources

A usage popup requests `cli_quotas` when mounted. The backend reads **all**
available sources; `QuotaContent` filters by the popup's provider IDs. There
is no periodic quota fetch, per-provider IPC argument or quota cache.

- **OpenCode:** `quota.rs` reads `opencode` and `opencode-go` keys from the CLI
  auth file and requests the corresponding Zen `/usage` endpoint. It parses
  rolling, weekly and monthly windows. 401 means the key was rejected; 403
  shows the no-quota error. Requests time out after 10 seconds.
- **Codex:** `codex.rs` reads a stored OAuth access token/account ID;
  `quota.rs` requests `https://chatgpt.com/backend-api/wham/usage`. No OAuth
  token means no source. Expired/rejected sessions return instructions rather
  than refreshing or writing credentials. Window labels derive from duration;
  `gpt-reserve` is hidden. Requests time out after 10 seconds.
- **Claude:** `claude_usage.rs` recursively scans JSONL transcripts under the
  Claude projects directory for matching tool-result JSON. It chooses the
  newest record timestamp, parses 5-hour/weekly and optional Opus/Sonnet
  weekly windows, and reports `asOf`. Missing snapshots produce no source.
- **Gemini:** no quota source or usage button.

The UI shows exact percentages and reset countdowns. Remaining mode inverts
used percentages and clamps them to 0–100; warning colors still derive from
used percentages. Missing data and source errors are shown explicitly.

## Persistence and panel lifecycle

- `agent-rtl.settings.json` stores refresh cadence and quota wording.
  Reading it normalizes values and rewrites legacy unknown preferences.
- `agent-rtl.keys` stores only launch-at-login. The OS is the source of truth;
  recognized legacy key/account/theme sections are compacted on startup.
- `displayPrefs.ts` stores frontend preferences in `keyturn.displayPrefs`:
  CLI version visibility, compact rows, focus refresh, automatic update check
  on opening the Updates tab, theme and accent. Defaults use dark/blue, visible versions
  and disabled focus/update checks. Skipped versions use
  `keyturn.skippedUpdateVersion`.
- Paths are defined in `paths.rs`; `KEYTURN_OPENCODE_USERDATA` overrides app
  data, `CODEX_HOME` overrides Codex auth, `XDG_DATA_HOME` overrides OpenCode
  auth and `CLAUDE_CONFIG_DIR` overrides Claude transcript home.
- The tray toggles the existing main webview and anchors it under the icon,
  clamped to the monitor. Losing focus hides it. Opening never reloads it.
- Status refresh reads settings and all CLI statuses on mount/manual refresh,
  at the configured interval while focused, or after detected timer drift.
  Refresh on focus is optional and off by default. Vite HMR updates dev UI.

## UI behavior

- Header: Keyturn brand, last status refresh age, Refresh, Settings and Quit.
- Main list: connected accounts only (`binaryFound && loggedIn === true`),
  count, identity, readable auth/plan/services and optional CLI version. Paths
  and internal account IDs stay out of cards. Add account is a floating button.
- Add account: pick a provider, then Install if missing, Sign in and Done.
  The stepper begins after provider selection. Fresh status validates each
  stage; the wizard never executes installation or login commands.
- Usage: a card icon opens `UsageModal`; limits do not render inline.
- Settings tabs: General / Display / Updates. Cadence offers 30/60/120-second
  presets plus a selected custom saved value, with controls disabled while
  saving. General contains the reset action. Display contains used/left
  selection, appearance presets and detail/density controls. Tab motion respects
  keyboard interaction and reduced-motion preferences.
- Loading skeleton until settings arrive, explicit load/probe errors and toast
  feedback. A plain browser has no Tauri IPC and shows the expected load error.
- Updates use `@tauri-apps/plugin-updater` and `plugin-process` in the frontend:
  check, release notes, download/skip, progress, install and relaunch. Current
  version comes from `core:app:allow-version`; there are no backend update
  commands. An optional automatic check runs when the Updates tab opens.
- The release workflow expects the matching passwordless signing private key,
  creates a draft universal macOS release and includes `latest.json`.

## Structure

`src/App.tsx` coordinates status refresh and modals. `src/api.ts` owns invoke
wrappers; `src/types.ts` mirrors backend preferences. `src/components/ui/`
contains locally owned Radix components with source/license notes. CSS tokens
and component styles live in `src/index.css`. Rust modules own CLI probing,
quota sources, local settings/store persistence, paths and the tray shell.

Removed functionality includes stored key switching, profiles, console login,
history/export, threshold notifications, sounds and automatic key switching.
The old `account`, `bridge`, `auth`, `providers`, `profiles`, `notify`, `history`
and `usage` modules are absent.

## Development workflow (standing rules)

- **Test locally, always.** Verification happens on the dev build
  (`pnpm tauri dev`) — never on the installed app.
- **Never ship a release build unless asked.** No `pnpm tauri build`, no DMG,
  no copy to Desktop or `/Applications`, until the user explicitly says
  "take an output" (خروجی بگیر). Release builds are slow, flaky on this
  toolchain, and overwrite what the user is testing.
- **After every task, restart dev** without asking (kill the old
  `target/debug/keyturn` + vite first; single-instance would otherwise
  shadow the new launch).
