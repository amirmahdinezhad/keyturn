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
directories. Codex profile tests cover identity/workspace matching, isolated
login, active switching, rotated-token preservation, external sign-ins,
private file permissions, rejected corrupt/symlink credentials and storage
restrictions. Real user credentials are not changed by those tests. The native
entry point remains thin so integration tests can use `keyturn_lib`.

## Live native verification

Saved Codex usage renews expired/rejected access tokens once through the CLI's
managed app-server. `profiles` tests cover inactive-account isolation, active
snapshot synchronization and failure preservation; `codex_refresh` tests cover
the bounded auth-only RPC, timeout cleanup and sanitized errors. The App recovery
test follows Reconnect account → Replace login → fresh account-specific usage,
without activation. A reused/revoked refresh token needs a fresh auth.json; opening
the unrelated active Codex account cannot repair that saved session.

Start the dev stack on port 5174 and test the **menu-bar panel**:

1. Open, hide and reopen using the tray icon. The existing panel should appear
   immediately with its mounted state. Click outside to verify dismissal.
2. Confirm connected providers match real CLI status. Missing or signed-out
   CLIs should appear only in Add account.
3. Check the wizard's installation and sign-in validation. A missing CLI must
   remain at Install; a signed-out CLI must remain at Sign in. Run installation
   or login in your own terminal only if you intend to change that CLI.
4. Open available usage popups. Verify percentage/reset display or the source
   error. Claude popups must load live usage even without transcript snapshots;
   verify inactive profiles use their own login and session failures remain explicit.
   Both Claude and Codex require usable OAuth sessions. Close and reopen a popup to request fresh data.
   For a connected Codex ChatGPT account, also check its public reset outlook:
   a scheduled announcement shows total hours/minutes and local hours/minutes
   without GMT or a source link. Close it and verify it stays closed after
   refresh/restart; a new announcement must appear again. Banked credits retain
   their explanation. Passed deadlines and completed resets, including legacy
   saved completions, stay hidden. A current probability-only watch shows Possible
   reset with its probability/window when enabled. Toggle Show possible resets
   under Settings → General → Codex resets; announcements must remain visible.
   Change Reset refetch interval and verify timer replacement and saved settings.
   Expired watches and failed/stale outlooks leave no placeholder. For 93.5% used, both native
   menu-bar and popup must show 7% remaining. Time markers are neutral without an
   elapsed-time description below the bars.

5. Change a reversible preference, close/reopen Settings and verify persistence,
   then restore its original value. Check keyboard tab navigation and Escape.
   Under Add account → Import file, choose a synthetic auth.json and verify its
   filename and Ready to import state. Choose another file, then Cancel: the
   panel and previous selection must remain visible. The native file chooser
   taking focus must not trigger the panel's ordinary focus-loss dismissal.
6. Check the displayed running version. Updater download/install/relaunch is
   covered by mocks and requires a separate signed-release validation.
7. Open Codex profiles and verify the name form, custom auth.json path form and
   isolated browser login. Going back before submitting must create nothing; importing
   must preserve the source and active account. Replacing a file for an existing
   account must keep the new tokens pending across refreshes until activation.
   Import and Set active must show a 30-second countdown with Cancel restart
   and Restart now. Cancel leaves active credentials intact. Timer tests use
   a synthetic desktop driver: expiry/manual close/Restart now must quit first,
   activate after exit, reopen, then verify. Missing app/refused quit cannot
   replace credentials; a failed file write reopens the prior login.
   A worker whose process ancestry includes the desktop being closed must reject
   the operation before requesting quit. On macOS start dev with `pnpm run run`
   and confirm `pnpm run dev:status` reports a launchd-owned supervisor.
   Use temporary app data and synthetic Codex credentials for switching checks.
   Verify active-name changes, rename/removal and preservation of the previous
   login. A real second-account browser login requires the user's participation;
   do not claim desktop-app account synchronization from UI mocks alone.

User-supplied files can be tested without changing the real Codex home or
controlling the desktop app. This opt-in test runs import → isolated activation
with a synthetic lifecycle → same-account replacement → Personal restoration,
checks source preservation/0600 copies, and makes no network/model requests:

```bash
cd src-tauri
KEYTURN_TEST_AUTH_DIR=/path/to/files cargo test --offline supplied_files_round_trip_in_isolated_storage -- --ignored
```

`pnpm run test:native-restart` builds a small temporary AppKit fixture, then uses
the production native driver code to quit it, reach the activation boundary,
reopen the same app path and verify a new PID. Under Rust unit-test compilation
the driver's bundle ID is fixed to the fixture. It cannot target real Codex or
use account credentials. The runner removes its fixture after testing.

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

## Usage preferences regression checks

`SettingsModal.test.tsx`, `ProfileDisplaySettings.test.tsx`,
`CodexAccountModal.test.tsx`, `ClaudeProfilesModal.test.tsx` and `usage.test.ts`
cover percent-only controls, appearance/threshold persistence, custom alerts,
daily background downloads, isolated profile creation, manual credential import,
account tests and known-window pace calculations. Rust tests cover threshold
history/reset re-arming, private Claude config references, random profile names,
percent wording, auto-switch candidate selection and native text rendering.

For a native percentage image for visual inspection, run this opt-in test from
`src-tauri` (the synthetic image contains no account data):

```bash
KEYTURN_TEST_TRAY_RGBA=/tmp/keyturn-percent.rgba cargo test --offline tray_text::tests
```

It writes straight RGBA plus `[width,height]` in the adjacent `.json` file.
The original mark, system-font percentages, profile caption and active underline
must render on a transparent 36-pixel-high retina image.

For OpenCode Go, a stored key alone is not evidence of an active subscription.
An explicit EntitlementError / OpenCode Go subscription required response must
hide Go (or its entire provider card if no other services remain). Generic
403/WAF errors and exhausted rolling/weekly/monthly limits must not establish
expiry. A confirmed ended state must survive a transient source failure; real
usage windows should restore visibility after renewal.

## Start onboarding again (macOS)

Run `pnpm run reset:data`. It stops Keyturn and dev, deletes Keyturn workspace
settings, saved profiles, alert history and native WebKit/cache data, then starts
dev again on port 5174. Saved Keyturn profiles are removed permanently.
Provider CLI logins (`~/.codex`, `~/.claude`, Gemini and OpenCode) and unrelated
files in OpenCode's shared application directory remain intact.
`KEYTURN_OPENCODE_USERDATA` is honored when set in the invoking shell.

The reset writes an empty workspace state with migration already completed, so
an existing Codex CLI login is shown by discovery instead of automatically
creating a legacy Personal workspace. Browser preview storage is separate.

Verify the reset's deletion boundaries with `node --test scripts/reset-data.test.mjs`.

### Single-account menu bar and dark account popups (2026-10-29)

- TypeScript: `pnpm exec tsc --noEmit` passed.
- Vitest: 27 files, 288 tests passed. The menu-bar regression covers workspace
  provider/account filtering, one-account quota reads, account selection saving,
  and replacement of limit controls when the selection changes.
- Rust: `cargo check --offline` passed; library suite: 160 passed, 2 ignored.
  Workspace coverage checks invalid/removed menu-bar selections and provider
  fallbacks. The Tauri watcher completed its dev rebuild; native process PID
  75364 started at 17:24:28.
- Live source on port 5174 contains the account selector. Browser main page
  shows the expected missing-IPC state. A temporary component preview changed
  Emma’s 5-hour/weekly controls to Zoe’s monthly control after selecting Zoe.
  The popup surface computed to rgb(35,38,43), with rgba(0,0,0,.66) scrim.
- After moving the preview to about:blank, WebKit PID 76707 retained an
  established connection to port 5174. Touching WorkspaceMenuBar.tsx produced
  the 17:25:33 HMR update in /tmp/keyturn-vite-5174.log. Temporary preview files
  were removed; no real account credentials or selections were changed.

### Workspace alerts, initials and compact global settings (2026-10-29)

- TypeScript passed. Vitest: 27 files, 260 tests passed. Obsolete global controls
  and updater UI tests were replaced by Theme-only coverage; workspace tests
  cover alert thresholds, reset notifications, custom thresholds, initials
  placement and workspace-scoped saves.
- `cargo check --offline` passed. Rust library tests: 163 passed, 2 ignored.
  The warning-color regression initially failed because toggling workspace
  colors discarded cached samples. It now passes with immediate repaint data.
  Native image assertions cover warning pixels in both metrics and the icon,
  disabled-threshold fallback, initials and tooltip identity. Settings tests
  cover legacy alert fallbacks and independent workspace overrides.
- Live component preview on 5174 showed only Theme in Global settings. Workspace
  Usage showed initials directly after account selection, workspace alert
  levels and a 58×32px switching input. Preview used synthetic identities;
  no credentials or real saved account settings were changed.
- Main browser page showed the expected missing-IPC state. Served source checks
  found Theme and Show account initials. Preview was moved to about:blank;
  WebKit PID 86079 retained a native connection to 5174, followed by a fresh
  17:40:43 HMR update. Tauri completed the dev rebuild (PID 85360, 17:39:34).
  Temporary preview files were deleted.

### Left initials and stacked native progress (2026-10-29)

- `pnpm exec tsc --noEmit` passed; Vitest: 27 files, 261 tests passed.
  Coverage checks the percentage default and workspace-scoped style saving
  while preserving the selected account and both enabled limits.
- `cargo check --offline` passed. Rust library suite: 168 passed, 2 ignored.
  Coverage includes style persistence/defaults, raw used/remaining fractions,
  independent limit colors, left-side initials in both modes and pixel checks
  for two stacked bars. One/two bars retain the same width; missing usage has
  an unavailable dash rather than a zero reading. Active progress underline
  is limited to initials and cannot become a third bar.
- Inspected actual AppKit-rendered percentage/progress image output from synthetic
  metrics. Live component preview on 5174 switched Percentages → Progress bars
  while both 5-hour and Weekly stayed enabled. Temporary preview files removed.
- Main browser page showed the expected no-IPC state; served source contains
  Progress bars. With previews off 5174, native WebKit PID 89857 had an established
  connection; touching WorkspaceMenuBar.tsx generated the 17:46:43 HMR update.
  The Tauri watcher completed its dev rebuild and launched PID 89604 at 17:45:32.

### Uniform menu-bar warning tint (2026-10-29)

- Typecheck passed; Vitest: 27 files, 261 tests passed.
- Offline Cargo check passed; Rust library tests: 170 passed, 2 ignored.
  The regression first failed on the old purple initials during a red weekly
  warning. It now verifies shared label/metric colors and fully opaque native
  image pixels in both percentage and progress styles. Icon and content use
  the same warning resolver. Coverage includes disabled levels, recovery,
  expired/unselected limits, unavailable data and disabling warning colors.
- Served WorkspaceSettings source on 5174 contains the unified-color help text.
  Browser main page showed the expected missing-IPC state. With previews on
  about:blank, native WebKit PID 95550 retained an established connection; a
  WorkspaceSettings touch triggered HMR. The Tauri watcher completed its dev
  rebuild and launched target/debug/keyturn PID 95083.

### Restored global settings tabs (2026-10-29)

- TypeScript passed; Vitest: 27 files, 270 tests passed. Restored tests cover
  General/Display/Updates navigation, Theme-only Display, global cadence and
  launch saves, scoped reset without workspace replacement, update checks,
  skip/download/install/relaunch, failed checks and automatic download while
  closed with installation retained after reopening.
- Live synthetic preview on 5174 showed all three peer tabs, General controls,
  Theme in Display, and the update card/automatic switch in Updates. No update
  was installed or real app behavior changed during visual checking.
- Browser main page showed expected no-IPC state. Served source contained all
  three tab labels. After moving the preview to about:blank, native WebKit PID
  95550 retained a connection to 5174; touching SettingsModal.tsx pushed HMR.
  Temporary preview files were removed. No Rust changes were needed.

### Theme moved into General (2026-10-29)

- TypeScript passed; Vitest: 27 files, 270 tests passed. Existing navigation,
  theme persistence and App integration checks now expect General/Updates,
  with the Theme group inside General and no empty Display tab.
- Live component preview on 5174 confirmed the two-tab layout and Theme under
  General before Reset. Main browser page showed the expected missing IPC state.
  Served source contained the two tab labels. With previews off 5174, WebKit
  PID 95550 retained a native connection and a SettingsModal touch pushed HMR.
  Temporary preview files were removed. No Rust changes.

### Immediate manual switch and workspace tab cleanup (2026-10-29)

- TypeScript passed. Vitest: 27 files, 266 tests passed. Obsolete workspace
  Accounts-tab tests were removed; account-card rename/removal coverage remains.
  Coverage now checks General/Usage only, separate reconnect import, immediate
  manual switch UI without countdown controls, and immediate menu-bar account
  selection after native success before a state refresh returns.
- `cargo check --offline` passed. Rust library tests: 174 passed, 2 ignored.
  Fake desktop tests cover zero-grace manual switching, the retained automatic
  countdown, replacement ordering, failed switches never publishing completion,
  provider-scoped menu-bar selection and retained destination window settings.
  Native pixel coverage verifies equal initials/percentage glyph height.
- Live synthetic WorkspaceSettings preview showed General and Usage without
  Accounts. Actual native renderer output was inspected for equal-size initials
  in percentage/progress modes. The browser main page had the expected no-IPC
  state. Served source on 5174 contained the two workspace tabs.
- Previews moved to about:blank, native WebKit PID 3709 retained an established
  connection to 5174. Touching WorkspaceSettings pushed the 18:24:03 HMR update.
  The Tauri watcher completed its dev rebuild and launched PID 2863. Temporary
  preview files were removed. No real account switch or Codex restart was run
  during verification; lifecycle tests use a fake desktop driver.

### Immediate automatic switches with active-thread confirmation (2026-10-29)

- TypeScript passed; Vitest: 27 files, 268 tests passed. Coverage includes the
  running-thread warning, honest unknown-activity wording, Cancel without
  completion, and Close and switch resuming the pending native job.
- Offline Cargo check passed. Rust library suite: 181 passed, 3 ignored.
  Added coverage for zero-grace automatic timing, live process ancestry,
  Desktop-origin rollout lifecycle markers, partial-write uncertainty, active
  work blocking quit, cancellation/opt-out preserving app and credentials, and
  destination revalidation after a potentially long confirmation wait.
- Explicit read-only live detector check against the current Codex desktop
  returned Running(1). No chat text was printed, no real switch was scheduled,
  and Codex was not closed. Lifecycle driver tests use fake desktop processes.
- Live synthetic preview on 5174 showed the dark Thread in progress popup with
  Cancel / Close and switch; the unknown variant explained uncertainty.
  Main browser page showed expected no-IPC state. Served source contained the
  new action text. Temporary preview files were removed and tabs moved off 5174.
- Tauri watcher completed the dev rebuild and launched PID 11532. Native WebKit
  retained an established connection; touching WorkspaceSettings pushed a fresh
  HMR update after the rebuild. Earlier mixed-export hook invalidation correctly
  caused a full reload. Live detection uses local version-dependent rollout
  metadata, not a guaranteed cross-client or remote/cloud execution API.

### Readable yellow and no menu-bar underlines (2026-10-29)

- TypeScript passed; Vitest: 27 files, 268 tests passed.
- Offline Cargo check passed; Rust library suite: 183 passed, 3 ignored.
  New checks assert yellow/amber contrast ≥4.5:1 on tested solid dark/white
  surfaces, one shared tint across initials/text/progress and transparent bottom
  rows in both numeric/progress output. Legacy active preferences cannot draw
  an underline because the renderer no longer has an active-underline field.
- Inspected native AppKit-rendered light/dark warning output for both styles.
  System/native window appearance selects #FFD60A or #8A5300, independently of
  the panel CSS theme. ThemeChanged repaints cached data without a quota read.
  These checks use solid backgrounds; macOS translucent wallpaper can vary.
- Served source on 5174 contains the new warning palette variable. Main browser
  page showed the expected missing-IPC state. Preview moved to about:blank;
  native WebKit PID 16463 retained an established 5174 connection. Touching
  WorkspaceSettings pushed fresh HMR. Tauri completed its dev rebuild and
  launched target/debug/keyturn PID 15310. No account settings or system theme
  were changed during verification.

### Larger single progress and reset-time markers (2026-10-29)

- TypeScript passed; Vitest: 27 files, 268 tests passed.
- Offline Cargo check passed; Rust library suite: 186 passed, 3 ignored.
  Single-limit geometry is wider/thicker than stacked rows. Pixel checks verify
  a time marker on every row of one/two/three-limit progress; numeric pixels
  remain unchanged. Timing tests match popup session/week remaining/used
  orientation and reject missing, malformed, expired and out-of-period resets.
- Inspected actual native AppKit output for one/two/three progress rows. The
  single bar is larger, neutral outlined ticks stay legible over fill/track,
  and no bottom underline returns. Marker duration support matches the Usage
  popup (5-hour/rolling and weekly), without assuming a monthly duration.
- Served source on port 5174 contains the new progress help text. Browser main
  page showed expected no-IPC state. Dev had stopped during final verification;
  confirmed independent launchd supervisor 20878 and native PID 21154 on 5174.
  The Tauri log shows Finished dev profile and Running target/debug/keyturn.
  With previews moved to about:blank, native WebKit PID 21195 retained a live
  connection; touching WorkspaceMenuBar pushed fresh HMR after startup.
  No real account selection, credentials or system theme were changed.

### Remove workspace alerts; provider-level Codex resets (2026-10-30)

- TypeScript passed; Vitest: 27 files, 268 tests passed. Updated settings and app
  integration checks cover absence of alert controls and saving the Codex reset
  master. Two-account coverage proves one notice outside articles and removal
  when disabled. Confirmed-reset coverage includes dismissal, stale/future
  timestamps and newer-response precedence; elapsed deadlines never confirm a reset.
- Offline Cargo check passed; Rust library suite: 183 passed, 3 ignored;
  integration suite: 8 passed. Removed alert-history module and delivery/probe
  paths. Regression coverage loads old enabled alert settings and verifies
  notification and workspace alert keys are omitted from serialization.
- Synthetic two-account preview on 5174 showed a single compact reset strip
  under the Codex heading. Usage settings contained Codex resets and optional
  possible-reset switches; turning off the master removed the strip and the
  dependent switch. No real account/preferences were changed for this preview.
- Main browser showed the expected missing-Tauri-IPC state. Served settings
  source contained Codex resets. Temporary preview files were removed and all
  preview tabs moved off 5174. Native WebKit PID 30143 retained an established
  connection; touching WorkspaceSettings produced fresh HMR. Tauri watcher
  completed its dev rebuild and launched native PID 30094.

### Workspace Menu bar tab and reset wording (2026-10-30)

- TypeScript passed; Vitest: 27 files, 269 tests passed. Workspace coverage
  verifies General / Usage / Menu bar tabs, direct Menu bar entry, separate
  presentation/usage controls, reset descriptions via aria-describedby and
  persistence of the predictions flag. The app switch integration checks the
  selected account through the new Menu bar tab.
- Live synthetic settings preview on 5174 showed all three tabs. Menu bar owns
  visibility, provider/account, initials, display style, limit selection and
  colors; Usage retains capacity wording, reset updates and switch policy.
  Show reset updates explains announced/recent confirmations. Include reset
  predictions explicitly labels estimates as unconfirmed and disappears when
  the parent is disabled. No real account preferences were changed.
- Served source contained the new Menu bar tab and predictions label. Main
  browser showed the expected missing-Tauri-IPC state. Removed temporary preview
  files and moved all preview tabs off 5174. Native WebKit PID 30143 retained an
  established connection; touching WorkspaceSettings produced a fresh HMR update.
  Rust was unchanged, so no Cargo check/rebuild was required for this task.

### Restore menu-bar color details (2026-10-30)

- TypeScript passed; Vitest: 27 files, 272 tests passed. New coverage verifies
  per-level switches, descriptions, hiding details while the master is off,
  retaining an explicit empty selection, and custom percentage validation/saves.
- Offline Cargo check passed; Rust library suite: 185 passed, 3 ignored;
  integration suite: 8 passed. Saved warningThresholds round-trip, normalize
  invalid/duplicate values, distinguish null from [], and feed both numeric
  and progress rendering. A disabled critical level falls back to orange;
  disabling all levels preserves cached usage and removes warning tint.
- Live synthetic preview on 5174 showed yellow/orange/red switches under the
  enabled master, a compact custom-percent input, and the 55% custom level.
  Turning the master off/on preserved the disabled 95% level and enabled 55%.
  These controls add no usage notifications or alert history.
- Served source contains Color thresholds. Main browser shows expected no-IPC
  state. Temporary previews removed and preview tabs moved off 5174; native
  WebKit PID 47896 retained an established connection. Touching WorkspaceSettings
  produced fresh HMR. Tauri watcher completed its rebuild and launched native
  PID 46870. No real account settings were changed during the synthetic preview.

### Workspace accent across native menu-bar metrics (2026-10-30)

- Reproduced with `cargo test --offline --lib
  workspace_accent_change_recolors_cached_numbers_and_progress_with_the_icon`:
  logo/state accent changed to [124,108,240], but label color stayed [0,122,255].
  Root cause: paint projected workspace windows/warnings/captions into account
  preferences but retained its saved account color for metrics.
- Extracted the actual paint preference projection and made its color follow the
  active workspace accent, without modifying saved account colors. The original
  regression now passes for numeric/progress output in both monochrome modes,
  asserting native rendered pixels on logo and metric sides. Cached samples
  survive the edit; all providers retain warning-color priority, and legacy
  no-workspace preferences preserve the saved account color.
- TypeScript passed; Vitest: 27 files, 272 tests passed. Offline Cargo check
  passed; Rust library suite: 186 passed, 3 ignored; integration: 8 passed.
- Tauri watcher rebuilt and launched native PID 50587 (Finished dev profile /
  Running target/debug/keyturn). Native WebKit PID 51433 retained an established
  connection to port 5174. No frontend source changed, so frontend source/HMR
  checks were not needed. No real account or workspace preference was changed
  during verification; the native rendering assertions use synthetic samples.
