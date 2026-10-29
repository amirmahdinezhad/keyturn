# Panel design

Keyturn is a macOS tray utility. Apply the Appllama principles to the existing
Tauri/React panel: desktop typography, grouped preferences, keyboard focus and
short sheets. Mobile navigation, safe areas and simulator APIs do not apply.

## Shared rules

- System UI font; tabular numerals for usage and time. Names lead, email and
  subscription metadata sit below. Long names wrap; emails truncate and remain
  selectable, with the full value available through their title.
- Cool graphite neutrals in dark mode and matching ink neutrals in light mode.
  Workspace accent drives global actions; existing account/provider colors remain
  scoped to identity. Semantic warning colors describe usage, not navigation.
- Spacing uses a 4px rhythm. Controls use 8px corners, groups 12px, sheets 16px.
  Circular swatches and switches are intentional exceptions.
- Primary actions derive a darker shade when necessary to maintain 4.5:1
  contrast with white labels. The saved identity color stays unchanged.
- Main buttons, tabs and icon actions use 44px targets. Switches and checkboxes
  retain compact desktop visuals with enlarged pointer areas.
- Sheet headers and tab navigation stay above independently scrolling content.
  Every sheet has Close and Escape; nested account settings retain All accounts.
- Peer tabs change without sliding. Sheets use the shared short fade/scale;
  frequent rows have background feedback instead of scaling. Reduce Motion
  removes spatial motion, repeated skeleton motion and long transitions.
- Loading placeholders match account/usage rows. Empty and error states explain
  the next action. A completed provider setup cannot return to installation.

`src/index.css` owns base tokens and legacy component styles. `src/panel.css`
consolidates the current shared panel design and is imported after the base
stylesheet. Keep theme fixes and responsive rules together there.

## Reference patterns

Thirty real screens were inspected across preferences, account switching and
usage dashboards using Mobbin because Appllama MCP was unavailable. Patterns
were adapted to the 430×600 desktop panel, without copying a screen.

- Grouped label/control rows and quiet descriptions:
  [Linear preferences](https://mobbin.com/screens/332e33ea-6a11-476e-9442-06d1ec5a49bb),
  [Graphite preferences](https://mobbin.com/screens/d72b17de-7e8c-4a18-86a8-b4cd8bfa13d3).
- Identity first, active state beside the name, explicit add action:
  [PayPal account switching](https://mobbin.com/screens/b708dfb4-49db-4c39-844d-09d11d82e822),
  [Remote profile switching](https://mobbin.com/screens/f473e6d8-03ee-494d-8297-89af8e99f209).
- Readable headline values and secondary period/reset details:
  [Webflow site usage](https://mobbin.com/screens/60e0b500-55f9-4e21-a5ff-0f8f1f62f70b),
  [Exa usage](https://mobbin.com/screens/cfec730d-d7a7-4a8c-8fd1-d742bf18dd9f).


## Provider account cards (2026-10-29)

Each provider logo appears once in its group header beside the label and count.
Account cards use a compact two-line identity block: Figtree name, plan and
Active state on top, email below, with actions centered at the right. Regular
cards measure 70px high with 12px padding and 44px action targets. One provider-local account receives the text Active
badge; inactive Codex cards offer a text Switch action instead of arrow glyphs.
The active card resolves from current account metadata, then the workspace's
loaded default, then its first visible account. Native credential restart and
automatic-switch safeguards continue to use the actual Codex login metadata.

The design retains the existing semantic palette and has been reviewed with a
synthetic two-Codex/one-Claude fixture in dark and light themes, including a
360px-wide container. The fixture entry was removed after review.


## Limit warning (2026-10-29)

The limit dialog is advisory. Its left-aligned title precedes a compact identity
block (name/email), a short explanation, an inset reset-time row and equal-width
Cancel / Switch anyway buttons. Uses the existing semantic palette, Figtree,
16px outer corners and 8px action corners. The warning tone belongs to a small
usage glyph; the main action keeps the workspace blue. Busy actions are disabled
and scheduling errors stay inline for retry. Both themes and long identity text
were reviewed using the actual component in a temporary synthetic fixture.

Workspace account action popups have an opaque dark surface and darker scrim,
including in the light theme. Menu bar settings progress from workspace-owned
provider to one account to that account’s available limit switches.

Global settings has General and Updates peer tabs. Theme is in General. Workspace Usage contains reset updates and optional predictions. Menu bar owns limit colors. Account initials visibility sits directly
below the account picker. Percentage inputs are 58×32px with a separate percent
suffix, keeping the setting label on one line at the standard panel width.

Menu-bar initials are centered vertically to the left of usage. Workspace Menu bar
offers Percentages / Progress bars. Multiple bars form a compact vertical stack, one
row per enabled limit, with rounded fills that share the highest active warning hue. Dense rows use
compact limit labels; full labels and values stay in the tooltip. Numeric and progress modes have no active underline.

When a selected limit reaches an enabled warning threshold, the entire menu-bar
identity and all selected metrics share that warning hue. Progress tracks retain
their lower opacity while following the same hue.

Workspace settings has General, Usage and Menu bar tabs. Account-card management and
usage-popup recovery are separate modals. Manual switching begins immediately;
automatic switching also starts immediately. Running or unknown thread activity
opens a dark confirmation with Close and switch / Cancel before any quit. Menu-bar initials use
the same 25px native font size as percentage digits. A successful switch follows
the new account when its provider is already displayed in the current workspace.

Warning yellow is brighter in dark native appearance and darker amber in light
appearance, retaining one shared hue across the entire menu-bar display. No
underline appears beneath initials, values or bars in either rendering mode.

A single progress limit has a wider, thicker bar. Known reset periods show a
neutral vertical time marker on every single/stacked row, matching Usage limits
and remaining/used direction. The small light halo and dark core distinguish
time from the warning-colored fill without an active underline.

Provider reset update: retain graphite #191B1F, card #23262B, text #F1F3F5,
muted #A8B0BD and action #007AFF. Figtree follows the existing 13px title /
11px metadata scale. The layout is left aligned: Codex heading → one quiet
reset strip → account rows. Keeping it outside account cards avoids implying
that a public reset announcement belongs to a particular login. The strip has
no decorative accent, and its close action retains keyboard focus styling.

In Menu bar, Change colors near limits reveals its own detail group: colored
dots, used-percent labels, short severity captions and individual switches.
A 58px percent input below the group adds custom levels. The master collapses
these controls without discarding their selection. Notification UI stays absent.

Native menu-bar identity follows the workspace accent across logo, initials,
values and progress, with warning colors taking priority. Account-card accents
remain independently saved. Unavailable values and time ticks keep semantic
muted/neutral styling.
