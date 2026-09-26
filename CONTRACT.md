# Keyturn — frontend/backend contract (v2)

Backend = Tauri v2 Rust commands. Frontend = React 19 + TS + Tailwind v4 + HeroUI v3.
No other runtime deps. Icons: iconsax free set, Bold style only (`src/icons.tsx`); brand/model marks stay custom.

## Commands (via `invoke`)

```ts
interface Month { year: number; month: number } // month 0-11, local time
interface Cred { type: string; key: string; account?: string }
interface ProviderEntry { keys: Record<string, Cred>; active: string | null }
interface Bucket { total: number; models: Record<string, number>; tokens: number }
interface QuotaWindow { window: 'rolling' | 'weekly' | 'monthly'; percent: number; resetsAt: string | null; status: string | null }
interface KeyQuota { windows: QuotaWindow[] | null; error: string | null }
interface ProfileRef { id: string; name: string }

interface AppSettings {
  resetDisplay: 'clock' | 'remaining' | 'both'
  usagePercentMode: 'used' | 'remaining' | 'both'
  timeFormat: 'system' | '12h' | '24h'
  refreshIntervalSecs: number            // 10..300
  autoSwitch: boolean
  notifyEnabled: boolean
  notifyThresholds: number[]             // presets, default [75, 90, 95]
  notifyCustom: number | null
  soundEnabled: boolean
  soundName: string | null
  trayStyle: 'battery' | 'bar' | 'percent' | 'iconbar' | 'compact'
  trayColorMode: 'multi' | 'mono' | 'custom'
  trayCustomColor: string                // #rrggbb
  trayUsedMode: 'percent' | 'tokens'
  trayShowRemaining: boolean
  trayShowLabels: boolean
  trayShowPace: boolean
  trayMetrics: ('rolling' | 'weekly' | 'monthly')[]
  trayMulti: boolean
  trayProfiles: string[]                 // profile ids, max 5 rendered
  trayActiveIndicator: boolean
  billingDay: number                     // 1..28
  autoUpdateChecks: boolean
}

interface AppState {
  providers: Record<string, ProviderEntry>
  discovered: string[]
  applied: Record<string, string | null>
  usage: { byKey: Record<string, Record<string, Bucket>>; untracked: Record<string, Bucket>; error: string | null }
  providerUsage: Record<string, Record<string, KeyQuota>>
  settings: { openAtLogin: boolean; theme: string }
  customProviders: string[]
  month: Month
  profiles: ProfileRef[]
  activeProfile: string
  app: AppSettings
}
```

- `get_state(month: Month | null, refresh?: boolean) -> AppState`
- `save_key({ provider, name, apiKey, account }) -> AppState`
- `delete_key({ provider, name }) -> AppState`
- `apply_key({ provider, name }) -> AppState` — writes the provider's credential file
- `set_settings({ openAtLogin }) -> AppState` — launch at login (per profile store)
- `set_app_settings({ settings: Partial<AppSettings> }) -> AppState` — merges + normalizes
- `profile_create({ name }) -> AppState`
- `profile_rename({ id, name }) -> AppState`
- `profile_switch({ id }) -> AppState`
- `profile_delete({ id }) -> AppState`
- `list_sounds() -> string[]`, `play_sound({ name }) -> ()`
- `codex_status() -> { status: CodexStatus, keys: string[] }`
- `codex_refresh() -> { status, keys }` — refreshes `~/.codex/auth.json` OAuth tokens
- `history({ range, key? }) -> HistoryOut` — range: `5h|24h|7d|30d|billing`
- `export_history({ range, format, key? }) -> string` — format: `json|csv`, returns the written path
- `check_updates() -> { available, version?, notes?, error? }`
- `account_status()`, `connect_account_webview({ provider, account? })`, `disconnect_account({ account })`,
  `set_active_account_cmd({ account })`, `console_keys({ account, month, refresh? })`,
  `console_usage({ account, month, refresh? })`, `activate_account_key({ keyId, account })`
- `add_provider({ provider })`, `remove_provider({ provider })`
- `quit_app() -> ()`
- All errors are returned as human-readable strings.

## Semantics (backend owns)

- State, month scoping (local calendar month), key attribution, provider quota fetch,
  store file format (`agent-rtl.keys`, identical JSON to the Electron app), auth.json patch,
  openAtLogin via OS — all identical to `/Users/amir/Desktop/Projects/agent-keyring` for the default profile.
- **Profiles**: each profile has its own store file (`agent-rtl.keys.<id>`); the index
  (`agent-rtl.profiles.json`) holds the profile list plus `AppSettings`. History is the
  profile's own switch log + local usage DB attribution.
- **Provider registry** (`providers.rs`) owns host/login/usage paths, the credential file
  and its key path (`["key"]` for OpenCode-style, `["OPENAI_API_KEY"]` for Codex) and
  capability flags. Unknown providers default to key storage only.
- **Auto-switch** runs in `build_state` (and the background loop) when the current month is
  shown: exhausted = any window ≥ 100 %, a non-ok window status, or a 429 error. It picks
  the lowest-scoring healthy key, applies it, logs the switch and notifies.
- **Notifications** fire once per key/window/threshold; latches re-arm 5 % below threshold.
  Sounds come from the OS library (`/System/Library/Sounds` on macOS).
- **Tray** is redrawn on every state build: five styles, three colour modes, pace marker,
  multi-profile cells (other profiles reuse their last known quota snapshot).
- **Background loop** refreshes on the configured interval, detects wake via wall-clock
  drift, and checks for updates every 24 h when enabled.
- Frontend only sends the selected month and renders.

## UI behaviors to replicate 1:1 (read the Electron sources)

Sources: `/Users/amir/Desktop/Projects/agent-keyring/src/ui/{index.html,ui.js,style.css,model-logos.js}`.
New app root: `/Users/amir/Desktop/Projects/agent-keyring-tauri/src`.

- Header: profile chip, gear (settings modal), refresh, quit icon + blue "Add key", all HeroUI `size="sm"`;
  brand is the Keyturn logo (`AppLogo`: the app icon — blue squircle, key with a switch arrow both ways, sparkles) + `Keyturn`.
- Month box: `‹ September 2026 ›`, fixed-width label, next disabled at current month.
- Provider card: logo (OpenCode mark for /opencode/i else letter avatar), pretty name,
  account email in head when all keys share one account; keys grouped under per-account headers otherwise.
- `Usage limits` button on the card/account head, modal with 5-hour/Weekly/Monthly rows, exact percent,
  reset wording per `resetDisplay`, pace marker/label, blue progress bars.
- Month cost is a HeroUI `Chip` badge with the info action inside it (`CostBadge`).
- Key row: name + Active pill, trash icon, masked key, month-cost badge, `Cost detail` info action →
  modal titled `Cost detail — <key>` with brand logos and empty state when total is 0.
- Browsing a past month: read-only — no Apply/Delete/Active/Usage-limits; usage numbers still shown.
- Current month: Apply button on non-applied keys (appends `"<name>" → <Pretty>` toast), delete → custom
  confirm modal: `Are you sure you want to delete "X" token for Y?` + Cancel/Delete(red).
- Add-key modal: provider (+datalist of known+discovered ids), name, API key, optional account.
- Settings modal: tabs (General, Alerts, Menu bar, Automation, Advanced) with profile management,
  display modes, thresholds/sounds, tray styles, auto-switch, Codex card, history/export, updates.
- Loading skeleton until first state; toast bottom-center; white button text; radius 6 controls / 10 cards;
  accent #007aff; danger #ef4444; cost badge soft.

## Frontend structure

- `src/types.ts`, `src/api.ts` (invoke wrappers), `src/format.ts` (money, resets, reset modes, pace, tokens,
  prettyProvider, mask, monthLabel), `src/logos.tsx`, `src/App.tsx` + `src/components/*`
  (`SettingsModal`, `ProfileMenu`, `HistoryModal`, `UsageRows`/`UsageModal`, …).
- `src/index.css`: `@import "@heroui/styles";` then the app tokens; new components append their own block
  at the end of the file.
- `HeroUIProvider` at root. Prefer HeroUI Button/Modal/Input/Switch/Progress/Avatar/Chip/Skeleton with
  className overrides to match the tokens exactly; custom CSS where HeroUI fights you.
