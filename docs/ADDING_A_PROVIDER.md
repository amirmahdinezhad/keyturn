# Adding a provider

The provider registry lives in `src-tauri/src/providers.rs`. Adding a provider
is a data change plus, when the provider is new to the app, a logo and (if it
has a console) a sign-in flow. The rest of the backend asks the registry for
capabilities instead of branching on ids.

## 1. Describe the provider

Append a `ProviderSpec` to `SPECS`:

```rust
ProviderSpec {
    id: "acme",
    label: "Acme AI",
    host: "console.acme.ai",
    login_url: "https://console.acme.ai/login",
    home_url: "https://console.acme.ai/",
    keys_path: "/keys",
    usage_path: Some("/usage"),
    usage_api: Some("https://api.acme.ai/v1"), // GET {base}/usage with Bearer key
    auth: AuthTarget::OpenCode,                // or Codex / None
    signed_in_markers: &["console.acme.ai"],
    login_markers: &["/login", "/oauth"],
    connectable: true,
}
```

Field notes:

| Field | Meaning |
| --- | --- |
| `usage_api` | Base URL for `GET {base}/usage`. Response shape is `{ "usage": { "rolling": { "percent", "resetsAt", "status" }, "weekly": …, "monthly": … } }` — see `usage::parse_usage`. |
| `usage_path` / `keys_path` | Pages the console webview walks for scraping. Only used when the console has no JSON API. |
| `auth` | `OpenCode` patches `~/.local/share/opencode/auth.json` under the provider id with `{ type, key }`; `Codex` patches `OPENAI_API_KEY` in `~/.codex/auth.json`; `None` stores keys locally only. |
| `signed_in_markers` / `login_markers` | URL fragments used by `providers::is_signed_in` to decide whether the console webview still needs a login. |

Capabilities are derived: `console`, `usage_api`, `auth_patch`, `token_refresh`.

## 2. Frontend

- Add the id to `PROVIDER_OPTIONS` in `src/components/ProviderPicker.tsx`
  (`ready: true` when console sign-in works, `false` for local-CLI providers
  such as Codex).
- Add a brand mark to `src/logos.tsx` if the fallback letter avatar is not
  good enough, and a model matcher to `modelLogoKey` in `src/format.ts` when the
  provider's model ids have a distinctive prefix.

## 3. Token refresh (optional)

Providers with OAuth tokens implement `refresh_tokens()` in a module like
`codex.rs`, and the settings Advanced tab calls it. Keep writes atomic and 0600
by using `paths::atomic_write_json`.

## 4. Tests

- `cargo test providers::` covers registry lookups and sign-in markers.
- Add a `parse_usage` case in `usage.rs` when the payload shape differs.
- The live smoke tests (`KEYTURN_LIVE_SMOKE=1 cargo test`) exercise a
  real store and quota fetch.
