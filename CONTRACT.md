# Keyturn — frontend/backend contract (v3, CLI-only)

Backend = Tauri v2 Rust commands. Frontend = React 19 + TS + Tailwind v4 + HeroUI v3.
No other runtime deps. Icons: iconsax free set, Bold style only (`src/icons.tsx`); brand/model marks stay custom.

## Auth model (T3-style)

Keyturn does NOT authenticate. No login flow, no sign-in window, no OAuth,
no keychain writes, no credential-file patching. Each provider's own CLI owns
its auth; the user runs the login command in their own terminal. The app only
runs each CLI's read-only status command and shows the install/login guidance.

| Provider | CLI binary | User runs | We run (read-only) |
| -------- | ---------- | --------- | ------------------ |
| Claude | `claude` | `claude auth login` | `claude auth status --json` |
| OpenAI Codex | `codex` | `codex login` | `codex login status` |
| OpenCode | `opencode` | `opencode auth login` | `opencode auth list` + auth.json keys |
| Gemini | `gemini` | `gemini` (Sign in with Google) | file/env probe (no status cmd) |

Anthropic / OpenAI / Google keys without their own CLI are managed through
OpenCode (`opencode auth login anthropic|openai|google`).

## Commands (via `invoke`)

```ts
interface AppSettings {
  refreshIntervalSecs: number            // 10..300
  billingDay: number                     // 1..28
  autoUpdateChecks: boolean
}

interface AppState {
  settings: { openAtLogin: boolean }
  app: AppSettings
}

interface ProviderCliStatus {
  id: string                 // claude | codex | opencode | gemini
  label: string
  binary: string             // default binary name, e.g. `claude`
  binaryFound: boolean
  binaryPath?: string | null
  version?: string | null
  loggedIn?: boolean | null  // null = unknown (CLI missing / probe failed)
  authMethod?: string | null
  email?: string | null
  account?: string | null
  detail?: string | null     // human subtitle, e.g. "Logged in using ChatGPT"
  loginCmd: string           // exact command the user runs in their terminal
  installUrl: string
  authFile?: string | null   // credential file peeked at (read-only)
  connected?: string[]       // OpenCode only: provider ids in auth.json
}
```

- `get_state() -> AppState`
- `cli_status() -> ProviderCliStatus[]` — runs the CLI status probes, read-only
- `set_settings({ openAtLogin }) -> AppState` — launch at login
- `set_app_settings({ settings: Partial<AppSettings> }) -> AppState` — merges + normalizes
- `history({ range }) -> HistoryOut` — range: `5h|24h|7d|30d|billing`
- `export_history({ range, format }) -> string` — format: `json|csv`, returns the written path
- `check_updates() -> { available, version?, notes?, error? }`
- `quit_app() -> ()`
- All errors are returned as human-readable strings.

Removed (auth lived here): `save_key`, `delete_key`, `apply_key`,
`add_provider`, `remove_provider`, `account_status`, `connect_account_webview`,
`disconnect_account`, `set_active_account_cmd`, `console_keys`, `console_usage`,
`activate_account_key`, `codex_status`, `codex_refresh`, `console_payload`,
`list_sounds`, `play_sound`.
Removed (profiles/themes lived here): `profile_create`, `profile_rename`,
`profile_switch`, `profile_delete`; the `theme` setting.
Deleted modules: `account` (OAuth/keychain/console scraping), `bridge`
(console hash-bridge), `auth` (auth.json patching), `providers` (console
registry; replaced by the `cli` status registry), `profiles` (replaced by
the single-file `settings` module).

## Semantics (backend owns)

- **Settings**: one file (`agent-rtl.settings.json`) holds refresh cadence,
  billing day and update checks; one store file (`agent-rtl.keys`) holds the
  launch-at-login flag. History is the local usage DB aggregation. Legacy
  sections (stored keys, switch log, accounts, themes, profiles) are
  compacted out of the files on launch.
- **CLI status** (`cli.rs`) never writes: binary lookup on `PATH` (+ Homebrew /
  `/usr/local/bin` fallbacks), one short-lived status subprocess per provider
  (10 s timeout), read-only peeks at the CLI auth files for display.
- **Tray** (`tray.rs`) is a static bundled icon. Quota rendering, pace
  markers and multi-profile cells are gone with stored keys/quotas.
- **History** is read from the local usage DB, aggregated per time bucket with
  no key attribution (no key filter).
- **Notifications** are one-shot notices only (e.g. update available).
- **Background loop** checks for updates every 24 h when enabled. Wake
  detection lives in the frontend panel, which refreshes on focus.
- Frontend only renders; refresh re-runs `get_state` + `cli_status`.

## UI behaviors

- Header: refresh (re-reads state + CLI status), gear (settings modal),
  quit icon; brand is the Keyturn logo (`AppLogo`) + `Keyturn`.
- Main view: one `Providers` section with a card per CLI — logo, name, status
  pill (`Logged in` green / `Not logged in` amber / `Not installed` gray),
  version + binary path + account detail, connected chips for OpenCode
  sub-providers, and the login command (or install link when the binary is
  missing) in a copyable code row. A note states Keyturn never signs in and
  the command must run in the user's own terminal, plus the OpenCode
  sub-provider hint (`opencode auth login anthropic|openai|google`).
- Loading skeleton until first state; `EmptyState` on load/CLI-probe failure;
  toast bottom-center; radius 6 controls / 10 cards; accent #007aff.
- Settings modal: tabs (General, Advanced) with
  refresh cadence, billing day, history/export, updates.

## Frontend structure

- `src/types.ts`, `src/api.ts` (invoke wrappers), `src/format.ts` (money, tokens,
  exact decimals, model marks, avatar hues), `src/logos.tsx`, `src/App.tsx` + `src/components/*`
  (`CliProviders`, `SettingsModal`, `HistoryModal`, …).
- `src/index.css`: `@import "@heroui/styles";` then the app tokens; new components append their own block
  at the end of the file.
- `HeroUIProvider` at root. Prefer HeroUI Button/Modal/Input/Switch/Progress/Avatar/Chip/Skeleton with
  className overrides to match the tokens exactly; custom CSS where HeroUI fights you.
