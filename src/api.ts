import { invoke } from "@tauri-apps/api/core";
import type { Subscription } from "./subscriptions";
import type { AppSettings, AppState, CodexProfilesState, CodexProfileLoginStatus, CodexRestartStatus } from "./types";

/** Typed invoke wrappers for the backend commands.
 *
 * Provider CLIs own login. Codex profile actions explicitly save/activate
 * native credentials; IPC returns metadata and login instructions only.
 */

/** Read preferences and reconcile launch-at-login with the OS setting. */
export function getState(): Promise<AppState> {
  return invoke<AppState>("get_state");
}

/** Request launch-at-login and return the resulting OS-backed state. */
export function setSettings(openAtLogin: boolean): Promise<AppState> {
  return invoke<AppState>("set_settings", { openAtLogin });
}

/** Merge panel and menu-bar preferences into persisted backend settings. */
export function setAppSettings(
  settings: Partial<AppSettings>,
): Promise<AppState> {
  return invoke<AppState>("set_app_settings", { settings });
}

/** Credentials travel only to the native validator; imported accounts stay inactive. */
export interface WorkspaceImport {
  accountName: string;
  credentials?: string | null;
  path?: string | null;
}

/** Commit a workspace only after its first auth.json has been validated and saved. */
export function createWorkspace(name: string, accent: string, account: WorkspaceImport): Promise<AppState> {
  return invoke("create_workspace", { name, accent, ...account });
}

/** Save an account directly into one workspace, without activation or restart. */
export function importWorkspaceAccount(workspaceId: string, account: WorkspaceImport): Promise<AppState> {
  return invoke("import_workspace_account", { workspaceId, ...account });
}

/** Persist this workspace's identity and usage policy atomically. */
export function updateWorkspace(workspace: import("./types").Workspace): Promise<AppState> {
  return invoke("update_workspace", { id: workspace.id, name: workspace.name, accent: workspace.accent, usage: workspace.usage });
}

/** Change the visible workspace without changing the CLI's active credentials. */
export function selectWorkspace(id: string): Promise<AppState> {
  return invoke("select_workspace", { id });
}

/** Move membership; account preferences and credentials remain intact. */
export function moveWorkspaceAccount(id: string, targetId: string): Promise<AppState> {
  return invoke("move_workspace_account", { id, targetId });
}

/** Apply the saved panel accent to the native menu-bar mark and single-color metrics. */
export function setMenuBarAccent(accent: string): Promise<void> {
  return invoke("set_menu_bar_accent", { accent });
}

/** Save one account; an explicitly named partner starts its first two-account cycle. */
export function setCodexAccountPreferences(
  id: string,
  preferences: import("./types").CodexAccountPreferences,
  startCycleWith: string | null = null,
): Promise<AppState> {
  return invoke("set_codex_account_preferences", { id, preferences, startCycleWith });
}

/** Exit the native application, including its tray icon. */
export function quitApp(): Promise<void> {
  return invoke("quit_app");
}

/** CLI-only provider status (T3-style, read-only). Backend never logs in. */
export interface ProviderCliStatus {
  id: string;
  label: string;
  binary: string;
  binaryFound: boolean;
  binaryPath?: string | null;
  version?: string | null;
  loggedIn?: boolean | null;
  authMethod?: string | null;
  email?: string | null;
  account?: string | null;
  detail?: string | null;
  loginCmd: string;
  installUrl: string;
  authFile?: string | null;
  connected?: string[];
  subscriptions?: Subscription[];
}

/** Probe every supported CLI without running installation or sign-in commands. */
export function cliStatus(): Promise<ProviderCliStatus[]> {
  return invoke<ProviderCliStatus[]>("cli_status");
}

/** Reconcile saved profiles with the active Codex account and refreshed tokens. */
export function codexProfiles(): Promise<CodexProfilesState> {
  return invoke<CodexProfilesState>("codex_profiles");
}

/** Create a named isolated login, or save the current ChatGPT account. */
export function createCodexProfile(name: string, importCurrent: boolean): Promise<CodexProfilesState> {
  return invoke<CodexProfilesState>("create_codex_profile", { name, importCurrent });
}

/** Change a profile's label without changing its login. */
export function renameCodexProfile(id: string, name: string): Promise<CodexProfilesState> {
  return invoke<CodexProfilesState>("rename_codex_profile", { id, name });
}

/** Forget a saved profile without signing out the active CLI account. */
export function removeCodexProfile(id: string): Promise<CodexProfilesState> {
  return invoke<CodexProfilesState>("remove_codex_profile", { id });
}

/** Preserve the previous account and activate the selected Codex login. */
export function activateCodexProfile(id: string): Promise<CodexProfilesState> {
  return invoke<CodexProfilesState>("activate_codex_profile", { id });
}

/** Import auth.json from an absolute or ~/ path without opening a system dialog. */
export function importCodexProfile(name: string, path: string, replaceId: string | null = null): Promise<CodexProfilesState> {
  return invoke<CodexProfilesState>("import_codex_profile", { name, path, replaceId });
}

/** Save a pasted login in native private storage, leaving the active account intact. */
export function importCodexProfileJson(name: string, credentials: string, replaceId: string | null = null): Promise<CodexProfilesState> {
  return invoke("import_codex_profile_json", { name, credentials, replaceId });
}

/** Active CLI login metadata; never includes credential values. */
export function codexAccountStatus(): Promise<{ present: boolean; hasApiKey: boolean; hasTokens: boolean; email: string | null; lastRefresh: string | null; expiresAt: number | null }> {
  return invoke("codex_account_status");
}

/** Verify only the current Codex usage source. */
export function codexAccountQuota(): Promise<QuotaSet> { return invoke("codex_account_quota"); }

/** Test a saved account's usage without activating it. */
export function codexProfileQuota(id: string): Promise<QuotaSet> {
  return invoke("codex_profile_quota", { id });
}

/** Start the CLI's browser login in this profile's isolated home. */
export function startCodexProfileLogin(id: string): Promise<CodexProfileLoginStatus> {
  return invoke<CodexProfileLoginStatus>("start_codex_profile_login", { id });
}

/** Poll the isolated sign-in process without reading its output. */
export function codexProfileLoginStatus(id: string): Promise<CodexProfileLoginStatus> {
  return invoke<CodexProfileLoginStatus>("codex_profile_login_status", { id });
}

/** Cancel sign-in without signing out the active account. */
export function cancelCodexProfileLogin(id: string): Promise<void> {
  return invoke<void>("cancel_codex_profile_login", { id });
}

/** Start an immediate manual restart; allowLimited acknowledges capacity only for this job. */
export function scheduleCodexRestart(id: string, allowLimited = false): Promise<CodexRestartStatus> {
  return invoke<CodexRestartStatus>("schedule_codex_restart", { id, ...(allowLimited ? { allowLimited: true } : {}) });
}
/** Read native switch progress or an active-thread confirmation after the panel reopens. */
export function codexRestartStatus(): Promise<CodexRestartStatus | null> { return invoke<CodexRestartStatus | null>("codex_restart_status"); }
/** Cancel before app quit begins; credentials remain unchanged. */
export function cancelCodexRestart(): Promise<CodexRestartStatus | null> { return invoke<CodexRestartStatus | null>("cancel_codex_restart"); }
/** Confirm closing Codex and continue a switch paused for active work. */
export function runCodexRestartNow(): Promise<CodexRestartStatus | null> { return invoke<CodexRestartStatus | null>("run_codex_restart_now"); }

/** A quota bucket whose percent always represents used capacity. */
export interface QuotaWindow {
  window: string;
  percent: number;
  resetsAt?: string | null;
  status?: string | null;
}

/** Provider limits or a source error, with observation time for local snapshots. */
export interface QuotaSet {
  provider: string;
  windows?: QuotaWindow[] | null;
  error?: string | null;
  asOf?: string | null;
  resetCredits?: { availableCount: number; applicableAvailableCount?: number | null; planType: string } | null;
}

export interface BankedResetCredit {
  id: string;
  title: string;
  expiresAt: string | null;
  coveredWindows: string[];
  canUse: boolean;
  unavailableReason: string | null;
}

export interface BankedResetInventory {
  availableCount: number;
  credits: BankedResetCredit[];
  usage: QuotaSet;
  pendingCredit: BankedResetCredit | null;
}

export interface BankedResetReview { token: string; credit: BankedResetCredit; retry: boolean }
export interface BankedResetResult { code: "reset" | "nothing_to_reset" | "no_credit" | "already_redeemed"; windows_reset: number }

/** Read saved-account inventory and eligibility without activating or redeeming. */
export function codexBankedResets(id: string): Promise<BankedResetInventory> {
  return invoke("codex_banked_resets", { id });
}

/** Recheck a selected reset and pin its account/impact for a separate confirmation. */
export function prepareCodexReset(id: string, creditId: string): Promise<BankedResetReview> {
  return invoke("prepare_codex_reset", { id, creditId });
}

/** Spend only the reviewed reset after the user explicitly confirms in Usage. */
export function consumeCodexReset(id: string, reviewToken: string): Promise<BankedResetResult> {
  return invoke("consume_codex_reset", { id, reviewToken });
}

/** Read all available quota sources; callers filter the result for their popup. */
export function cliQuotas(): Promise<QuotaSet[]> {
  return invoke<QuotaSet[]>("cli_quotas");
}

/** A reset the public source has confirmed, rather than a passed deadline. */
export interface CodexCompletedReset {
  id: string;
  resetType: "regular" | "banked";
  /** When the source announced or first observed completion. */
  announcedAt: string;
}

/** Public completed resets, announcements and forecasts from Codex Resets. */
export interface CodexResetStatus {
  latestReset: CodexCompletedReset | null;
  scheduledReset: {
    resetType: "regular" | "banked";
    announcedAt: string;
    scheduledFor: string | null;
  } | null;
  activeWatch: {
    level: "elevated" | "strong";
    resetChancePercent: number | null;
    forecastWindow: string;
    observedAt: string;
    expiresAt: string;
  } | null;
  generatedAt: string;
}

/** Read the cached public reset outlook without sending account information. */
export function codexResetStatus(): Promise<CodexResetStatus> {
  return invoke<CodexResetStatus>("codex_reset_status");
}

/** Strip the Tauri invoke prefix so backend validation messages read cleanly. */
export function prettyError(e: unknown): string {
  const raw = e instanceof Error ? e.message : String(e);
  return raw.replace(/^Error invoking remote method '.*': (Error: )?/, "");
}

/** List tracked Claude configurations and their local usage snapshots. */
export function claudeProfiles(): Promise<import("./types").ClaudeProfile[]> { return invoke("claude_profiles"); }
/** Create a separate configuration or reference the current CLI configuration. */
export function createClaudeProfile(name: string, useCurrent: boolean): Promise<import("./types").ClaudeProfile[]> { return invoke("create_claude_profile", { name, useCurrent }); }
/** Ask the CLI to verify only this profile's sign-in. */
export function rescanClaudeProfile(id: string): Promise<import("./types").ClaudeProfile[]> { return invoke("rescan_claude_profile", { id }); }
/** Rename or forget metadata; Claude's credential/history directory is preserved. */
export function editClaudeProfile(id: string, name: string | null): Promise<import("./types").ClaudeProfile[]> { return invoke("edit_claude_profile", { id, name }); }

/** Read live subscription usage with only the selected Claude profile’s login. */
export function claudeProfileQuota(id?: string): Promise<QuotaSet> { return invoke("claude_profile_quota", { id: id ?? null }); }

/** Save discovered CLI accounts without changing their active credentials. */
export function createDiscoveredWorkspace(name: string, accent: string, providerIds: string[], accountEmails: Record<string, string | null>): Promise<AppState> {
  return invoke("create_discovered_workspace", { name, accent, providerIds, accountEmails });
}

/** On-demand discovery returns only signed-in CLI accounts with confirmed paid plans. */
export function discoverWorkspaceAccounts(): Promise<ProviderCliStatus[]> {
  return invoke("discover_workspace_accounts");
}

/** An explicit add-account flow may extend the current workspace's CLI membership. */
export function includeWorkspaceProvider(workspaceId: string, providerId: string): Promise<AppState> {
  return invoke("include_workspace_provider", { workspaceId, providerId });
}

export type AccountProvider = "codex" | "claude" | "antigravity";
export interface SetupAccount { id: string; provider: AccountProvider; name: string; displayName?: string | null; active?: boolean; email: string | null; subscription: Subscription }
export interface SetupLogin { attemptId: string; status: "pending" | "completed" | "failed"; authUrl: string | null; userCode: string | null; error: string | null; account: SetupAccount | null }
/** Setup credentials live only in native private drafts until one explicit commit. */
export const beginAccountSetup = (): Promise<string> => invoke("begin_account_setup");
export const discardAccountSetup = (sessionId: string): Promise<void> => invoke("discard_account_setup", { sessionId });
export const loadSetupAccounts = (sessionId: string): Promise<SetupAccount[]> => invoke("load_setup_accounts", { sessionId });
export const startSetupLogin = (sessionId: string, provider: AccountProvider, method: string, name: string): Promise<SetupLogin> => invoke("start_setup_login", { sessionId, provider, method, name });
export const pollSetupLogin = (sessionId: string, attemptId: string): Promise<SetupLogin> => invoke("poll_setup_login", { sessionId, attemptId });
export const cancelSetupLogin = (sessionId: string): Promise<void> => invoke("cancel_setup_login", { sessionId });
export const submitSetupCode = (sessionId: string, code: string): Promise<void> => invoke("submit_setup_code", { sessionId, code });
export const importSetupAccount = (sessionId: string, provider: AccountProvider, name: string, file: { credentials: string } | { path: string }): Promise<SetupAccount> => invoke("import_setup_account", { sessionId, provider, name, ...file });
export const connectSetupConfig = (sessionId: string, name: string, path: string): Promise<SetupAccount> => invoke("connect_setup_config", { sessionId, name, path });
export const commitAccountSetup = (sessionId: string, name: string, accent: string, accountIds: string[], workspaceId: string | null = null): Promise<AppState> => invoke("commit_account_setup", { sessionId, name, accent, accountIds, workspaceId });
export const removeWorkspaceAccount = (workspaceId: string, provider: AccountProvider, id: string, everywhere: boolean): Promise<AppState> => invoke("remove_workspace_account", { workspaceId, provider, id, everywhere });
export const antigravityAccounts = (): Promise<SetupAccount[]> => invoke("antigravity_accounts");
export const antigravityAccountQuota = (id: string): Promise<QuotaSet> => invoke("antigravity_account_quota", { id });

/** Explicit opt-in to read and verify the current AGY credential source. */
export const connectCurrentGoogle = (sessionId: string, name: string): Promise<SetupAccount> => invoke("connect_current_google", { sessionId, name });

/** Rename only the saved AGY identity; credentials and workspace membership stay intact. */
export const renameAntigravityAccount = (id: string, name: string): Promise<void> => invoke("rename_antigravity_account", { id, name });
