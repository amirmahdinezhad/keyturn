# Workspaces

Setup has three steps. The first collects only the workspace name and accent.
In the second, **Load accounts** probes saved accounts and provider CLIs on demand. Only signed-in
accounts with a confirmed paid subscription and no known billing expiry appear.
Free, expired and unknown plans are omitted. Nothing is selected by default;
checkboxes explicitly choose the individual accounts saved to this workspace. Reload keeps
only still-available selections and never selects new accounts. The backend
rechecks paid eligibility and the selected account’s email before creating the workspace.

The workspace can be created in step two. **Add new account** opens optional step
three: choose a provider, then a method. Codex offers official browser login,
device code and auth.json import; Claude offers official CLI browser login in an
isolated configuration or connection to an existing configuration directory.
Antigravity offers experimental Google browser login, reading the current local
AGY login, and credentials JSON import. It requires a compatible installed AGY
OAuth client (or a matching client ID/secret environment pair) and server-confirmed
identity and paid tier. Gemini CLI credentials are not Antigravity credentials.
API-key accounts are outside this subscription-only flow.

File import accepts a file chooser, drag and drop, or a full file path. Selected files
are checked for valid JSON and a 256 KB size limit; Finder drops pass the path
to the native validator. File drafts and selections survive Back within setup.
Several new accounts can be verified before one final save. Native drafts hold
temporary isolated login homes; leaving asks to discard unsaved work, stops login
processes and removes app-owned temporary credentials. Claude configurations keep
their stable absolute paths because their Keychain service depends on that path.
Provider browser login runs in the system browser, initiated from the panel.
Imports never activate Codex. The primary actions and tiny privacy footer are outside the content scroller
in both inline setup and the create-workspace modal.

The macOS file chooser temporarily takes focus from the tray panel. The panel
stays visible while that native modal is open, so choosing or cancelling returns
to the same account draft. Ordinary focus loss still dismisses the panel.

Existing saved Codex profiles migrate once into **Personal**, preserving account
names, preferences and credentials. An unsaved CLI login waits for the user to load and select it in step two.

Saved accounts can be shared by multiple workspaces without copying credentials.
Codex account display and switch preferences belong to each workspace.
Each provider has an independent active default in `usage.activeAccounts`.
Codex and Claude can both show Active simultaneously. Load accounts prioritizes
the current login of each provider; adding a backup retains the existing default.
When a default is removed, a remaining account of that provider becomes its
fallback, without clearing any other provider’s selection. Codex credential
activation still uses its real CLI identity and the existing restart safeguards.
Each removal offers removing the membership here or forgetting the saved account
in all workspaces. Workspace selection changes the visible
accounts, accent and native monitoring policy; activation remains explicit.
**Account settings** opens from the gear on each saved Codex, Claude or Antigravity
account and contains only rename and removal. Names are saved in the provider’s
account repository. **Workspace settings → Accounts** lists accounts by provider with Edit and Trash
actions. Trash opens an animated confirmation popup before removal. Edit opens an animated popup over the unchanged list with the saved account
name, Cancel and Save name; all
three providers use their own rename command. Account identity rows are not
navigation buttons. Login recovery remains available through the usage reconnect
action. Moving is an explicit reassignment; loading the
same account into another workspace shares it.
Claude accounts appear in their own expanded group with per-account usage buttons.
Claude usage popups fetch live subscription limits using that profile’s login,
including inactive profiles. If the request fails, an existing local snapshot
retains its timestamp; otherwise a specific connection or session error is shown.
OpenCode and Gemini retain their setup in the collapsed **Other providers** section. Accounts share one compact surface; the Accounts heading's gear opens
Workspace settings. Account accents remain visible on their identity marks.
Account management keeps accent controls first and folds Menu bar, Alerts and
Usage bars into separate disclosures.

The active workspace accent colors the whole native menu bar: logo, initials,
limit labels, percentages and progress fills/tracks. Changing the accent repaints
cached metrics immediately. Enabled crossed warning levels override that accent
for the whole bar. Account/provider colors remain independently saved for their
other surfaces; the workspace override never rewrites them. Legacy rendering
without an active workspace retains its saved account/provider colors.

The global menu-bar provider picker includes Codex and remains available while
usage is disabled. Selecting Codex exposes the active workspace's menu-bar switch
and a shortcut to its Menu bar tab; it never rewrites that selection to Claude.
Other-provider panel visibility follows the workspace’s `providerIds`; missing/null membership retains the legacy global visibility. Workspace Menu bar shows account selection when enabled; Usage shows enrolled switch-cycle members.

| Scope | Preferences |
| --- | --- |
| Workspace → General | Name, preset/custom accent |
| Account card settings | Account name and removal |
| Workspace → Usage | Remaining/used percentages, automatic switching, reset updates and optional predictions |
| Workspace → Menu bar | Visibility, provider, account, initials, percentages/progress, limit selection and warning colors |
| Global | Theme, refresh cadence/focus, launch at login, reset-outlook polling cadence, updates, display |

The native monitor restricts Codex display and automatic switch candidates
to the selected workspace. Automatic switching also requires its workspace master
and two explicitly enrolled accounts. Selecting another workspace or moving a
member cancels an invalid automatic countdown. Manual jobs retain their lifecycle.

The active card reports personal quota exhaustion separately from public reset
forecasts. Multiple exhausted main windows use the latest reset, with total hours
and minutes (`Resets in 28h 15m`). Missing times remain explicit; elapsed deadlines
await fresh quota data. Model-specific/reserve buckets do not trigger this notice.

The adjacent icon chooses the next usable account in workspace order. Candidates
must be connected, have current main-window data, include learned windows and
have capacity in every main window. A click rereads identity/quota before the
existing cancellable 30-second desktop restart. No candidate disables the button.
The banner offers Cancel and Switch now; the native worker survives a hidden panel.

Metadata shares `agent-rtl.settings.json`: `workspaceVersion`, `activeWorkspaceId`
and `workspaces`. A workspace contains `id`, `name`, `accent`, `accountIds`,
`claudeAccountIds`, `antigravityAccountIds`, `providerIds`, `accountPreferences`, `usage`.
Credentials remain in the private profile repository. Global updates/reset cannot
overwrite workspace metadata. Workspace commands validate names, accents,
membership and destinations under the settings lock.

| Command | Result |
| --- | --- |
| `begin_account_setup()` / `discard_account_setup({sessionId})` | Create or clean a native draft |
| `load_setup_accounts({sessionId})` | Live-verify exact paid accounts; metadata only |
| `start_setup_login` / `poll_setup_login` / `cancel_setup_login` / `submit_setup_code` | Cancellable isolated sign-in, provider-approved URLs and device codes only |
| `import_setup_account` / `connect_setup_config` / `connect_current_google` | Verify and stage an account without permanent membership |
| `commit_account_setup({sessionId, workspaceId?, name, accent, accountIds})` | Revalidate all selected draft accounts, save memberships once; `AppState` |
| `remove_workspace_account({workspaceId, provider, id, everywhere})` | Remove local membership or explicitly forget globally |
| `discover_workspace_accounts()` | Read signed-in CLI accounts with confirmed paid plans; metadata only |
| `create_discovered_workspace({name, accent, providerIds, accountEmails})` | Revalidate and save only selected providers; `AppState` |
| `create_workspace({name, accent, accountName, credentials?, path?})` | Import first account; create/select workspace; `AppState` |
| `import_workspace_account({workspaceId, accountName, credentials?, path?})` | Add account without activation; `AppState` |
| `update_workspace({id, name, accent, usage})` | Update identity/policy; `AppState` |
| `select_workspace({id})` | Change visible workspace; `AppState` |
| `move_workspace_account({id, targetId})` | Reassign membership without changing login; `AppState` |

Imports accept exactly one of `credentials` and `path`. Metadata save failure
rolls back newly created profile indexes. Existing accounts remain saved.
Command results never contain credentials. `pnpm run reset:data` clears Keyturn's
test state and app-owned login homes without deleting ambient provider CLI logins.
