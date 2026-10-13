# Adding a provider

Keyturn reads CLI status and usage sources. Authentication remains in the
provider's CLI; the app displays instructions and never runs login commands
or patches credential files.

## Backend status

Add a `CliSpec` to `SPECS` in `src-tauri/src/cli.rs`:

```rust
CliSpec {
    id: "acme",
    label: "Acme AI",
    binary: "acme",
    install_url: "https://acme.example/cli",
    login_cmd: "acme login",
}
```

Implement a read-only probe and wire it into `status_for`. Prefer a CLI status
subcommand through `run_status` with a bounded timeout; otherwise inspect its
credential file/environment without writing. Binary lookup includes `PATH`,
Homebrew, `/usr/local/bin`, system directories and `~/.local/bin`.

Return the existing `ProviderCliStatus` shape. Unknown login state is `None`;
optional `None` values and empty `connected` arrays are omitted during
serialization. `connected` contains service IDs for a CLI such as OpenCode.
Do not put raw credentials in any IPC field.

## Setup and cards

- Add instructions, copyable installation command, verification command when
  available, and official guide links to `PROVIDER_SETUP` in
  `src/providerSetup.ts`. Add readable service names to `PROVIDER_NAMES` if needed.
- Add the provider mark to `ProviderLogo` in `src/logos.tsx`; otherwise it uses
  its existing fallback.
- `CliProviders.tsx` displays a provider only when `binaryFound` and
  `loggedIn === true`. The Add account wizard receives the other statuses.
- Wizard selection advances directly to Install or Sign in. Installation and
  sign-in checks use fresh status; success requires both the binary and
  a confirmed login.

## Optional quota source

Add a read-only source returning `quota::QuotaSet`, and include it in
`cli_quotas` in `src-tauri/src/lib.rs`. `percent` represents used capacity;
`as_of` is the observation timestamp for a local snapshot. Missing data and
source failures must remain distinguishable.

Register the card's usage button in `QUOTA_IDS` in `CliProviders.tsx`, and add
its source mapping and empty message in `UsageModal.tsx`. Opening any usage
popup currently reads every backend quota source, then filters by provider
in the frontend. It does not start a background quota refresh loop.

## Verification

- Add parser tests beside the Rust probe/source. Update registry and IPC
  expectations in `src-tauri/tests/integration.rs` when the provider set changes.
- Cover card rendering, setup validation and usage behavior in the relevant
  frontend tests; update fixtures as needed.
- Run typecheck, Vitest, Rust checks/tests and native panel/HMR checks from
  [Testing](TESTING.md).
- Update the provider/source tables in `README.md` and `CONTRACT.md`.
