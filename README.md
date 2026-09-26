# Keyturn

Menubar key manager for AI coding CLIs. It keeps API keys for OpenCode, Claude,
OpenAI, Gemini and Codex CLI in one place, watches their quotas, and switches
the key your CLI actually uses — automatically when a key runs out.

Tauri v2 (Rust) + React 19 + Tailwind v4 + HeroUI v3.

## Features

**Keys and providers**
- One store per provider; apply a key writes it straight into the credential
  file the CLI reads (`~/.local/share/opencode/auth.json`, `~/.codex/auth.json`)
  with atomic 0600 writes.
- Provider registry with capability flags (`src-tauri/src/providers.rs`):
  console sign-in, quota API, auth patch and token refresh are data, not
  hard-coded branches. See `docs/ADDING_A_PROVIDER.md`.
- Codex: reads `$CODEX_HOME/auth.json`, patches only `OPENAI_API_KEY`, keeps
  OAuth tokens, and can refresh them (public Codex client id, `last_refresh`
  updated).
- Console sign-in for OpenCode / Claude / OpenAI / Gemini in an in-app webview;
  multiple accounts per provider, each with its own cookie jar.

**Profiles**
- Unlimited profiles. Each one has its own keys, signed-in accounts, switch log,
  theme and history; display/alert settings stay app-wide.
- Profile menu in the panel header, multi-profile menu-bar cells with an active
  indicator.

**Monitoring**
- Quota windows (5-hour / weekly / monthly) per key from the provider's usage
  endpoint, cached for 60 s.
- Local usage attribution from the OpenCode SQLite DB: cost and tokens per key
  and per model, scoped to the calendar month, including a `untracked` bucket
  for usage that predates the switch log.
- Pace projection with six levels (Comfortable → Runaway) and a coloured marker
  on every quota bar.
- Reset countdown in three modes: clock, remaining, or both, with system/12h/24h
  clock formatting; quota percentages can read used, remaining, or both.
- History charts for 5h / 24h / 7d / 30d / billing cycle with JSON and CSV
  export (written to your Downloads folder).

**Alerts and automation**
- Threshold notifications at 75/90/95 % plus one custom threshold, with a
  system sound picker and preview. Latches re-arm with 5 % hysteresis.
- Auto-switch: when a key hits 100 %, a non-ok status or a 429, the app moves
  to the least-used healthy key, logs the switch and notifies you.
- Background refresh loop keeps the tray, quotas, alerts and auto-switch fresh
  while the panel is closed, and forces a refresh after the machine wakes.
- Refresh interval configurable from 10 to 300 seconds.

**Menu bar**
- Five icon styles: Battery, Progress Bar, Percentage, Icon with Bar, Compact.
- Three colour modes: threshold colours, monochrome template (adapts to the
  system menu bar), or a custom hex colour.
- Simultaneous metrics, percent or tokens, used or remaining, optional labels
  and pace markers.

## Development

```bash
npm install
npm run tauri dev        # app + HMR
npm run build            # type-check + frontend bundle
pnpm test                # frontend unit / component / integration tests
cargo test               # Rust unit + integration tests (in src-tauri)
```

See `docs/TESTING.md` for what each layer covers and the full pre-commit set.

## Data files

| Path | Contents |
| --- | --- |
| `~/Library/Application Support/ai.opencode.desktop/agent-rtl.keys` | Default profile (same JSON as the Electron app `agent-keyring`) |
| `…/agent-rtl.keys.<profile>` | Extra profiles |
| `…/agent-rtl.profiles.json` | Profile index + app-wide settings |
| `~/.local/share/opencode/auth.json` | OpenCode credential patch target |
| `~/.codex/auth.json` | Codex credential patch target |
| macOS Keychain (`com.7skylab.keyturn`) | Console OAuth tokens and session cookies |

## Docs

- `CONTRACT.md` — frontend/backend IPC contract.
- `docs/ADDING_A_PROVIDER.md` — how to add a provider.
- `docs/RELEASING.md` — signing, notarization, updater, Homebrew and Nix.
- `docs/TESTING.md` — test layers, commands, and what is not covered.
