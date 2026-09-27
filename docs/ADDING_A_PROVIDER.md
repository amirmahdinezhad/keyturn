# Adding a provider

Keyturn is CLI-only (T3-style): it never authenticates. Adding a provider is
a data change in `src-tauri/src/cli.rs` plus a logo tweak — no sign-in flow,
no credential patching, no console scraping.

## 1. Describe the CLI

Append a `CliSpec` to `SPECS` in `src-tauri/src/cli.rs`:

```rust
CliSpec {
    id: "acme",
    label: "Acme AI",
    binary: "acme",
    install_url: "https://acme.ai/cli",
    login_cmd: "acme login",
}
```

Then add a `probe_*` function that answers "is this CLI logged in?" using
ONLY read-only means:

- preferred: the CLI's own status subcommand
  (e.g. `claude auth status --json`, `codex login status`, `opencode auth list`),
  run via the `run_status` helper (10 s timeout, never a login/write command);
- fallback: a read-only peek at the CLI's credential file or env vars
  (see `probe_gemini`: `~/.gemini/oauth_creds.json`, `GEMINI_API_KEY`, …).

Wire it into `status_for` and return the `(logged_in, auth_method, email,
account, detail, connected, auth_file)` tuple. `connected` is for meta-CLIs
like OpenCode that host sub-providers; leave it empty otherwise.

Field notes:

| Field | Meaning |
| --- | --- |
| `binary` | Default executable looked up on `PATH` (+ Homebrew/`/usr/local/bin` fallbacks). |
| `login_cmd` | Exact command shown to the user to copy into their own terminal. Never executed by the app. |
| `install_url` | Shown (copyable) when the binary is missing. |
| `detail` | One-line human subtitle, e.g. `Logged in using ChatGPT`. |
| `auth_file` | Credential file peeked at read-only for display, if any. |

## 2. Frontend

- Nothing required: `CliProviders.tsx` renders whatever `cli_status` returns.
- Add a brand mark to `src/logos.tsx` (`ProviderLogo`) if the fallback letter
  avatar is not good enough.

## 3. Tests

- `cargo test cli::` covers parsing (Claude JSON, Codex status text,
  `opencode auth list` tokens, Gemini env fallback).
- `cargo test --test integration cli_` covers the spec list and the
  `ProviderCliStatus` IPC field names (they must match `src/api.ts`).
- `npm test -- --run` covers the card rendering in `src/App.test.tsx`.
