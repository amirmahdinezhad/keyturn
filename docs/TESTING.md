# Testing

Four layers, cheapest first. Everything runs locally with no network and no
signed-in account.

| Layer | Where | Command |
| --- | --- | --- |
| Frontend unit | `src/*.test.ts` | `pnpm test` |
| Frontend component | `src/components/*.test.tsx` | `pnpm test` |
| Frontend integration | `src/App.test.tsx` | `pnpm test` |
| Rust unit | `#[cfg(test)]` in `src-tauri/src/*.rs` | `cargo test` |
| Rust integration | `src-tauri/tests/integration.rs` | `cargo test` |

Before committing, all of these must pass:

```bash
pnpm test                 # frontend, 77 tests
npx tsc --noEmit          # types
npm run build             # tsc + production bundle

cd src-tauri
cargo test                # 61 unit + 8 integration
```

## Frontend

Harness: **Vitest** with `jsdom`, configured in `vitest.config.ts`. Setup lives
in `src/test/setup.ts` (adds `@testing-library/jest-dom` matchers and pins `TZ`
to UTC so date assertions are deterministic). Tests are colocated with the code
they cover.

**Unit** (`src/format.test.ts`, `src/api.test.ts`)

- `format.ts` is pure logic and is covered exhaustively: masking, exact decimals,
  reset wording, pace projection and its edge cases (window not started, already
  reset, opening 5%, marker clamping), brand matching, provider pretty-printing.
  Anything time-dependent runs under `vi.useFakeTimers()` with a fixed instant.
- `api.ts` is the IPC boundary, so the test pins the **exact command name and
  payload** for every wrapper. A renamed command or a changed argument shape
  fails here instead of at runtime in the app.

**Component** (`src/components/*.test.tsx`)

Render the real component and assert on what the user sees. Interaction uses
`@testing-library/user-event` (never raw `fireEvent` for presses — React Aria
presses need the full pointer sequence). Messages are matched through
`@testing-library/dom` queries; when a brand SVG carries a `<title>` that also
matches the label (Claude, OpenAI, Gemini), scope the query to a container
rather than using a global `getByText`.

**Integration** (`src/App.test.tsx`)

`App` is rendered with `./api` replaced by a `vi.mock` factory built from
`vi.hoisted`. A `beforeEach` resets every mock and installs a default
`getState`/`accountStatus` so each test only states what it overrides. Fixtures
come from `src/test/fixtures.ts`. This layer covers the orchestration the
components cannot: skeleton → loaded panel, load failure, refresh with
`refresh = true`, sign-in → account keys, and month navigation including the
"cannot go past the current month" rule.

Router/date expectations are derived from `new Date()` inside the test, not
hard-coded, so the suite does not rot as the calendar moves.

## Rust

Unit tests sit next to the code in `#[cfg(test)] mod tests`. They cover the
parsers, the store/profiles round trips, tray pixel rendering, notification
latches, usage attribution and the account callback.

Integration tests live in `src-tauri/tests/` and reach the code through the
`keyturn_lib` public API — the same surface an outside caller sees. They exist
because the inline tests cannot cover two things:

- **The IPC contract.** `app_state_serializes_to_the_frontend_contract` and
  `app_settings_keys_are_camel_case_for_the_ui` serialize the real IPC structs
  and assert the exact field names. `src/types.ts` and `CONTRACT.md` describe
  the same shape from the frontend side; these tests make a Rust-side rename
  fail loudly instead of silently breaking the UI.
- **The filesystem surface.** `atomic_writes_are_owner_only_and_round_trip`
  checks that credential writes are `0600` and leave no temp file behind.
  `keys_survive_a_round_trip_...` and `profiles_round_trip_...` drive the real
  files through the public API, including the "delete the active key re-points
  it" and "the last profile is protected" rules.

Tests are hermetic: temp dirs are created and removed per test, and
`isolated_user_data()` points `KEYTURN_OPENCODE_USERDATA` at a temp dir for the
binary so profile store files never land in real application data.

`main.rs` is a thin entry point that calls `keyturn_lib::run()`; all app code
lives in the lib so integration tests can link against it. Keep it that way.

## Not covered

- **End-to-end / UI automation.** The menubar panel and tray icon need the app
  running and a real display; there is no `tauri-driver`/WebDriver harness yet.
  The pieces with logic were extracted behind seams instead (see `cli.rs`)
  so unit and integration tests carry the weight.
- **Live CLI probing** against real provider CLIs. `cli::all_status` runs real
  subprocesses; tests cover parsing with synthetic inputs instead.

## Adding a test

1. Pure function → unit test beside it, same file's `.test.ts`.
2. React component → component test; mock only the `./api` module, never
   internal child components.
3. Anything crossing store ↔ profiles ↔ filesystem → `src-tauri/tests/`.
4. Tray/notification/usage parsing → `#[cfg(test)]` in the module.
5. Anything touching the IPC shape → `src-tauri/tests/integration.rs`.

## Troubleshooting

- **`failed to read plugin permissions: ... /some-other-project/...`** — a stale
  `src-tauri/target/` copied from another repo. Remove it and rebuild:
  `rm -rf src-tauri/target`.
- **`ERR_PNPM_UNEXPECTED_STORE`** — this project's `node_modules` was linked by
  pnpm 11 while the `pnpm` on `PATH` is older. Install with
  `npx pnpm@11 add ...` (or `pnpm config set store-dir` to match).
