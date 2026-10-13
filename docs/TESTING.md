# Testing

## Required checks

Run after every task:

```bash
pnpm exec tsc --noEmit
pnpm exec vitest run
```

For Rust changes, also run from `src-tauri/`:

```bash
cargo check --offline
cargo test --offline
```

`--offline` prevents Cargo dependency downloads; dependencies must already be
cached. It does not prevent a CLI subprocess from making its own requests.
The `cli_status_covers_the_four_supported_clis` integration test invokes real
installed CLIs, so its duration and probe behavior depend on the machine.

Before committing frontend changes, `pnpm run build` verifies the production
frontend bundle. This does not package the native app. A Tauri release build
requires an explicit user request.

## Automated coverage

| Layer | Files | Coverage |
| --- | --- | --- |
| Frontend unit | `src/api.test.ts`, `src/format.test.ts`, `src/displayPrefs.test.ts` | IPC names/payloads, formatting, validated preference storage and appearance tokens. |
| Frontend component | `src/components/*.test.tsx` | Connected cards, setup validation, usage windows/errors/snapshot age, dialog focus, settings and updater phases. |
| Frontend integration | `src/App.test.tsx` | Loading, load/probe failures, connected providers, setup/usage popups, manual refresh and default focus behavior. |
| Rust unit | `#[cfg(test)]` modules under `src-tauri/src/` | CLI parsers, local Claude snapshots, quota parsing, Codex claims, settings/store normalization, tray pixels and panel placement. |
| Rust integration | `src-tauri/tests/integration.rs` | IPC serialization, real CLI registry probes, settings/store round trips, atomic file permissions and live dev frontend mode. |

Vitest uses jsdom and `src/test/setup.ts`. Frontend tests replace native IPC
and updater calls with mocks; they do not prove that the tray panel or a live
quota endpoint works. Use `user-event` for interaction and scope text queries
when logo titles overlap provider names. Time-dependent assertions use fixed
clocks where needed.

Rust parser and filesystem tests use synthetic payloads or temporary
directories. The atomic-write test verifies the shared JSON writer in a
temporary file; it does not imply the app writes credentials. The native entry
point remains thin so integration tests can use `keyturn_lib`.

## Live native verification

Start the dev stack on port 5174 and test the **menu-bar panel**:

1. Open, hide and reopen using the tray icon. The existing panel should appear
   immediately with its mounted state. Click outside to verify dismissal.
2. Confirm connected providers match real CLI status. Missing or signed-out
   CLIs should appear only in Add account.
3. Check the wizard's installation and sign-in validation. A missing CLI must
   remain at Install; a signed-out CLI must remain at Sign in. Run installation
   or login in your own terminal only if you intend to change that CLI.
4. Open available usage popups. Verify percentage/reset display or the source
   error. Claude may have no matching transcript snapshot; Codex requires a
   usable OAuth session. Close and reopen a popup to request fresh data.
5. Change a reversible preference, close/reopen Settings and verify persistence,
   then restore its original value. Check keyboard tab navigation and Escape.
6. Check the displayed running version. Updater download/install/relaunch is
   covered by mocks and requires a separate signed-release validation.

For any `src/` change, also verify served source, HMR log and browser:

```bash
curl -s 'http://localhost:5174/src/api.ts' | grep -c 'Read all available quota sources'
tail -n 3 /tmp/keyturn-vite-5174.log
```

Inspect a preview tab on 5174 with a snapshot or DOM evaluation. Its lack of
Tauri IPC is expected; it cannot prove the native panel is current.

To prove **native HMR**, move all preview tabs off port 5174, keep the panel
open, confirm an established WebKit connection, then touch a frontend file:

```bash
lsof -nP -iTCP:5174
touch src/components/CliProviders.tsx
tail -n 3 /tmp/keyturn-vite-5174.log
```

Require a fresh update/reload and confirm the panel still renders current
content. `CliProviders.tsx` exports formatting helpers alongside components,
so Fast Refresh invalidation followed by a full reload is expected.

After Rust changes, confirm the `tauri dev` watcher finishes rebuilding and
starts a new `target/debug/keyturn` PID. Use `/tmp/keyturn-tauri-dev.log` when
startup output is redirected there. Restart dev after the task, stopping the
old debug app and Vite first to avoid single-instance shadowing.

## Limits and troubleshooting

There is no automated tray/WebDriver harness. Live provider availability,
authentication changes, network failures, quota endpoints and signed updates
need native checks; passing mocks is insufficient.

- Plugin permission errors mentioning another project indicate a stale
  `src-tauri/target/` copied from that project; rebuild from a clean target.
- `ERR_PNPM_UNEXPECTED_STORE` indicates a pnpm store/version mismatch. The
  release workflow uses pnpm 11; use a matching version/store for this checkout.
