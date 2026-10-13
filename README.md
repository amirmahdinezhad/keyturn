# Keyturn

A macOS menu-bar panel for AI coding CLI accounts and usage limits. Keyturn
checks Claude, OpenAI Codex, OpenCode and Gemini, then shows the CLIs that are
installed and signed in.

Authentication belongs to each provider's CLI. Keyturn displays installation
and sign-in instructions; it does not run them, store keys or patch CLI
credentials.

Built with Tauri v2, Rust, React 19, TypeScript and Tailwind v4. Local UI
components follow Shadcn Studio composition with Radix interaction primitives.

## Features

- Connected accounts with identity, sign-in method, plan when available,
  optional CLI version and named OpenCode services.
- An Add account wizard: choose a provider, install if needed, sign in in your
  own terminal, then verify with a fresh CLI status check.
- Usage limits in a separate popup, with percentage bars and reset countdowns:

  | CLI | Source used by Keyturn |
  | --- | --- |
  | OpenCode | Live usage requests for `opencode` and `opencode-go` entries in its auth file. |
  | OpenAI Codex | Usage request with the CLI's stored OAuth access token. API-key logins do not provide this quota source. |
  | Claude | Newest matching rate-limit snapshot in local JSONL transcripts, with its observation age. No snapshot means no data. |
  | Gemini | No usage popup. |

- Settings for launch at login, refresh cadence, used/remaining percentages,
  compact rows, CLI versions, dark/light/system appearance and four accents.
- Manual update checks, release notes, download progress, skip a version, and
  install/relaunch. An optional automatic check runs when the Updates tab opens.
- A static monochrome menu-bar icon. Clicking it toggles the existing panel
  without recreating the webview. Refresh on focus is off by default.

Status refresh runs on startup, on request, at the configured interval while
the panel has focus, and when interval drift suggests wake from sleep. The
default is 60 seconds; the backend accepts 10–300 seconds. Usage requests run
when a usage popup mounts. The backend reads all available quota sources and
the popup filters the result for its provider.

## Development

```bash
pnpm install --frozen-lockfile
pnpm run run
```

This starts Vite on **http://localhost:5174/** and the native Tauri dev app.
Use the menu-bar icon to open the actual panel. A browser preview has no Tauri
IPC and is expected to show a settings-load error.

If Vite is already running:

```bash
pnpm exec tauri dev --config '{"build":{"devUrl":"http://localhost:5174","beforeDevCommand":""}}'
```

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

See [Testing](docs/TESTING.md) for native panel and HMR checks.

## Local data

| Location on macOS | Purpose |
| --- | --- |
| `~/Library/Application Support/ai.opencode.desktop/agent-rtl.keys` | Launch-at-login flag; the OS is the source of truth. |
| `…/agent-rtl.settings.json` | Refresh cadence and used/remaining preference. |
| Webview localStorage: `keyturn.displayPrefs` | Appearance, density, version display, focus refresh and automatic update-check preference. |
| Webview localStorage: `keyturn.skippedUpdateVersion` | Skipped updater version. |
| `~/.local/share/opencode/auth.json` | Read-only OpenCode credentials and connected services. Honors `XDG_DATA_HOME`. |
| `~/.codex/auth.json` | Read-only Codex identity and OAuth access token. Honors `CODEX_HOME`. |
| `~/.claude/projects/**/*.jsonl` | Read-only Claude quota snapshots. Honors `CLAUDE_CONFIG_DIR`. |
| `~/.gemini/oauth_creds.json` / Gemini environment variables | Read-only Gemini authentication detection. |

`KEYTURN_OPENCODE_USERDATA` overrides Keyturn's application-data directory for
isolated testing. On startup, recognized legacy keys/accounts/profile-era
preferences are removed from Keyturn's own store/settings files. Provider
credential files are never written by Keyturn.

## Docs

- [Frontend/backend contract](CONTRACT.md)
- [Adding a provider](docs/ADDING_A_PROVIDER.md)
- [Testing](docs/TESTING.md)
- [Release workflow and packaging templates](docs/RELEASING.md)
