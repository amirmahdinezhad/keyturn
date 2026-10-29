# Usage preferences

Keyturn renders menu-bar usage as numeric percentages or horizontal progress
bars, selected per workspace. The existing tray panel remains mounted between
clicks; no embedded release assets or new port are used during development.

## Workspace controls

Workspace settings has General, Usage and Menu bar tabs. Account cards provide separate
rename/removal settings; connection recovery opens its own popup from Usage.
Add account handles new saved logins. Removing the Accounts tab retains every
saved credential and workspace membership.

Display switches default to on. Menu bar usage starts with a provider
picker restricted to providers with accounts in the current workspace, then a
single-account picker. There is no all-accounts option. `usage.menuBarAccountId`
persists that choice; a missing or removed choice falls back to the provider’s
active workspace member, then its first member. Limit toggles belong only to
that account and use its reported or previously learned limit buckets.
Workspace `usage.accountWindows` overrides older account-level window selections.
Empty selections display no percentages. A provider outage preserves selections
and renders missing values as unavailable rather than borrowing another login.

Global settings has General and Updates tabs. General contains the dark/light/system Theme choice and launch/focus/refresh controls. Menu-bar controls live in Workspace settings → Menu bar. The account picker is
followed by Show account initials: `usage.showAccountName` toggles the caption
on the left of percentages or bars, formatted as first/last initials (Emma Brooks → EB).
The tooltip retains the full name.

`usage.menuBarStyle` selects `percentages` (the backwards-compatible default)
or `progress`. Progress renders one horizontal bar per selected limit, stacked
vertically within the same native menu-bar height. A single enabled limit uses a larger 96×22px bar at 2× native scale;
multiple limits use 72px-wide bars stacked within the same menu-bar height. Fill uses the workspace’s remaining/used preference;
warning colors still follow consumed capacity. Missing data renders an
unavailable dash on its track, distinct from a genuine zero. Full values and
limit names remain in the tooltip. Initials stay left-aligned in both styles. No underline is drawn in either mode.

The normal menu-bar tint comes from the active workspace accent, including
initials, limit labels, all available values, progress fills/tracks and the logo.
An accent edit repaints the cached sample immediately. It overrides account
colors for menu-bar presentation only; saved account colors are not rewritten.
Unavailable values keep their muted treatment, and time markers stay neutral.

`usage.warningColors` independently enables consumed-capacity colors for the
whole menu bar, using the most consumed selected valid limit. Enabling it
reveals per-level switches, colored dots and a compact custom-percent input.
`usage.warningThresholds` stores enabled levels per workspace; null/missing uses
75/90/95%, and an empty list disables all levels. Custom integer levels 1–100
are supported; saving normalizes invalid/duplicate levels. The highest crossed
enabled level determines the hue (under 75 green, 75 yellow, 90 orange, 95 red).
Toggling the master preserves the saved levels.
Initials, values, window labels, tracks/fills and the tray mark share the hue.
Usage threshold notifications, session-reset alerts and their persisted history
have been removed. Old notification settings are ignored and omitted on save.
Changing colors or captions preserves the cached sample for immediate repaint.
Automatic switching remains
Codex-only, requires explicit source and destination membership, and accepts a
whole-number `usage.autoSwitchRemaining` from 0–100, defaulting to 2. A target
must have more remaining capacity than that threshold in every current window.

The browser success page for CLI-owned Codex login is supplied by Codex itself;
its HTML/title is not an asset of Keyturn.

## Persisted settings

The native settings file retains the existing `refreshIntervalSecs`,
`usageShowRemaining`, `menuBarEnabled`, `menuBarProvider` and `menuBarWindow` fields.
New nested fields are filled from defaults when reading older files.

All percentage displays default to remaining capacity: 100% at the start,
decreasing toward 0% as capacity is consumed. Workspace Usage → Show capacity
remaining controls the workspace menu-bar wording. Legacy popup wording
preferences remain in storage. Existing saved choices are preserved; missing `usageShowRemaining`
values default to `true`. The menu-bar and multi-profile remaining switches
also default to enabled and can be changed independently.

| Object | Fields and defaults |
| --- | --- |
| `usageAppearance` | `monochrome: true`, `showLabels: true`, `showRemaining: true`, `sessionEnabled: true`, `weeklyEnabled: false`, `showTimeMarker: true`, `colorMarkerByPace: true`, `paceAwareColors: false` |
| `multiProfile` | `enabled: false`, `selectedIds: []`, `showWeekly: false`, `showProfileLabel: true`, `monochrome: true`, `showRemaining: true`, `showActiveIndicator: true` |
| `autoSwitchEnabled` | `false` |
| `codexReset` | `showProbableResets: true`, `refetchIntervalSecs: 300` (30–3600 seconds) |

Thresholds are sorted, deduplicated and limited to integers from 1 to 100.
Profile selections retain valid UUIDs. Refresh cadence remains 10–300 seconds.
The existing popup wording preference is independent of menu-bar wording. Codex
always shows remaining capacity, rounded to the nearest whole percent with
half points rounded up (6.5% becomes 7%) in both the popup and native tray.

## Usage bars and pace

Known 5-hour and weekly windows derive elapsed time from their reset timestamp
and duration. Projected usage is consumed percentage divided by the elapsed
fraction. During the first 1% of a period, projection is unavailable. Missing,
expired, malformed or out-of-duration resets have no marker/projection.

Colors use consumed capacity: green below 75%, yellow at 75%, orange at 90%, red
at 95%. Pace-aware bars substitute projected end-of-period usage and add purple
at 125%. Remaining wording inverts the displayed percentage, while status colors
continue to describe consumed capacity. The marker follows the same direction
as the bar and always uses a neutral color; no elapsed-time description is
shown below the bar.
`colorMarkerByPace` remains in saved settings for compatibility but no longer
colors the tick. Menu-bar percentage colors use current consumed capacity.

## Background monitoring

The native monitor polls while menu-bar display or an explicitly enrolled
automatic switch cycle needs quota. No alert-only quota probes or usage-alert
history remain. Auto-switch status notifications are separate and retained.

Auto-switch requires the active Codex session to reach the workspace’s remaining-capacity threshold (default 2% left, or 98% used) with a current reset
timestamp. It walks saved accounts in order, skips unavailable/exhausted session
limits and exhausted known weekly limits, then schedules the existing graceful
immediate Codex restart with active-work confirmation. It attempts at most once per active-account/reset period
in the running monitor, including when the user cancels. No suitable account
means no change. Restart, cancellation and failures retain the existing profile
activation safeguards. Anthropic profile selection uses its CLI launch command.

## Manual login import

Global settings → Updates restores manual checks, release notes, skipping,
progress and install/relaunch. Automatic updates opt into daily checks and
signed downloads, including while Settings is closed. Downloaded updates wait
for explicit installation; failed checks wait until the next daily opportunity
unless retried manually. `keyturn.lastUpdateCheck` records successful checks.
Global reset changes app/display defaults without replacing workspaces or their
account membership, menu-bar settings or alert policies.

The Codex account screen reads current CLI metadata, rescans it, tests only the
Codex usage source, and accepts pasted auth.json. Manual import applies the same
native validator/private-file storage as file import. It saves a profile without
changing the active login. The pasted text is cleared after success and when the
screen closes; credentials are never returned over IPC or written to browser
storage. New profiles may omit their name; renames require a nonempty name.

## Public reset estimates

Workspace Usage has a `Show reset updates` master (`usage.showCodexResets`, default
true). Turning it off unmounts the provider notice and stops its requests.
One notice appears below the Codex heading, never inside each account card.
Upcoming announcements take priority, then confirmed resets from the last 24
hours, then optional possible-reset watches. Completion requires fresh source
metadata; an elapsed countdown alone never confirms a reset.
The dependent `Include reset predictions` toggle adds estimated chances and explicitly explains that predictions are unconfirmed. The independent saved
refetch interval defaults to five minutes; visibility defaults to enabled. The saved
interval controls both the frontend timer and shared native cache, including
failed requests. Announced resets remain visible when estimates are disabled.
Possible resets show the source probability and remaining forecast window, for
example `within 23h, tomorrow at 08:22`. The clock and relative day use the
device's timezone; the deadline is the source's watch expiry, not a confirmed
execution time. The countdown updates while open and disappears at expiry.
The close button persists dismissal of that watch across refetch/restart; an
odds-only update does not reopen it. A different watch can appear again.


## Switching to a limited account

A manual home-card Switch reads the destination's current quota before scheduling
a restart. Exhausted main limits open an Account limit reached warning with the
account name, email and reset countdown (or unavailable/checking-reset wording).
Cancel dismisses without changing anything. Switch anyway explicitly permits
that destination to exceed the capacity check for this restart job only, then
starts an immediate manual switch without a countdown. It is never a saved policy.

Scheduling errors remain inline and can be retried. Pending requests disable the
actions. Changing workspace or removing the destination dismisses the warning;
confirmation validates membership again. Quota errors still prevent scheduling.
The native preflight retains credential/session validation even with the override.
Automatic switches always reject exhausted targets and never pass this override.
Model/reserve buckets do not block the whole account.

After successful manual or automatic switching, a menu bar already displaying
that provider follows the newly active account. Other providers and inactive
workspaces keep their chosen display account. Explicit destination limit choices
remain intact; only unconfigured destinations inherit compatible enabled limits.
Initials use the same font size as percentage digits, retaining their left position.

## Running threads during switching

Both manual and automatic switches have zero grace. After validating the target
login, immediately before quit, Keyturn checks local desktop execution lifecycle
markers in rollout files held open by Codex’s own backend. Saved closed chats,
terminal-only sessions and completed/aborted turns do not count as running work.
A running turn opens a dark Thread in progress popup with Close and switch / Cancel.
An unverified probe instead explains that activity could not be confirmed. Both
pause indefinitely for the user’s decision; no account or app change happens
while waiting. Cancelling does not repeat the same automatic attempt that period.
The confirmation closes Codex gracefully and may interrupt work; it never force
kills a refusing app. Detection covers local desktop rollouts and is dependent on
the installed Codex version; it is not remote/cloud execution monitoring.

Warning yellow adapts to native system appearance: bright yellow (#FFD60A) for
dark surfaces, deeper amber (#8A5300) for light surfaces. Panel CSS theme remains
independent. Both variants exceed 4.5:1 contrast against their tested solid dark
and white backgrounds; translucent wallpaper-dependent surfaces can vary.

Native progress bars now include the same time-position marker as Usage limits
for known 5-hour/rolling and weekly periods with valid future reset timestamps.
The marker follows elapsed time in used mode and remaining time in remaining
mode, independently for each row. Its neutral dark/light outline remains legible
on warning colors and on the unfilled track. It appears in single and stacked
progress layouts; numeric rendering stays unchanged. Missing/expired/malformed
resets, resets farther away than their known period, and unknown durations
(including monthly) do not fabricate a marker, matching the Usage popup.
