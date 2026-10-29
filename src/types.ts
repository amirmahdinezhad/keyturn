// IPC contract types — mirrors CONTRACT.md. Keep in sync with src-tauri.
//
// Provider CLIs own sign-in; named Codex profiles have separate IPC. Settings carry
// refresh cadence, quota wording and menu-bar usage display.

/** Quota sources that can report remaining capacity in the menu bar. */
export type MenuBarProvider = "codex" | "claude" | "opencode" | "opencode-go";

export type AccentProvider = MenuBarProvider | "gemini";

/** Auto selects the supported window with the least remaining capacity. */
export type MenuBarWindow = "auto" | "5-hour" | "weekly" | "monthly";

/** Percent-only menu-bar metrics and usage-bar appearance. */
export interface UsageAppearance {
  monochrome: boolean;
  showLabels: boolean;
  showRemaining: boolean;
  sessionEnabled: boolean;
  weeklyEnabled: boolean;
  showTimeMarker: boolean;
  /** Retained for saved settings; elapsed-time ticks now use a neutral color. */
  colorMarkerByPace: boolean;
  paceAwareColors: boolean;
}

/** Selected saved accounts shown together as percentages in the native tray. */
export interface MultiProfileSettings {
  enabled: boolean;
  selectedIds: string[];
  showWeekly: boolean;
  showProfileLabel: boolean;
  monochrome: boolean;
  showRemaining: boolean;
  showActiveIndicator: boolean;
}

/** A saved account owns its display and explicit switch-cycle membership. */
export interface CodexAccountPreferences {
  menuBarEnabled: boolean;
  menuBarWindows: string[];
  knownWindows: string[];
  color: string;
  showLabels: boolean;
  showProfileLabel: boolean;
  showActiveIndicator: boolean;
  showTimeMarker: boolean;
  /** Retained for saved settings; elapsed-time ticks now use a neutral color. */
  colorMarkerByPace: boolean;
  paceAwareColors: boolean;
  warningColors: boolean;
  autoSwitch: boolean;
}

/** Fresh accounts show no percentages until the user chooses their windows. */
export const DEFAULT_CODEX_ACCOUNT_PREFERENCES: CodexAccountPreferences = {
  menuBarEnabled: true,
  menuBarWindows: [],
  knownWindows: [],
  color: "#007aff",
  showLabels: true,
  showProfileLabel: true,
  showActiveIndicator: true,
  showTimeMarker: true,
  colorMarkerByPace: true,
  paceAwareColors: false,
  warningColors: false,
  autoSwitch: false,
};

/** Public reset forecasts are optional and refresh independently of CLI usage. */
export interface CodexResetSettings {
  showProbableResets: boolean;
  refetchIntervalSecs: number;
}

export const DEFAULT_CODEX_RESET_SETTINGS: CodexResetSettings = {
  showProbableResets: true,
  refetchIntervalSecs: 300,
};

/** Workspace policy is independent of machine-wide refresh/theme preferences. */
export interface WorkspaceUsage {
  menuBarEnabled: boolean;
  displayMode: "active" | "selected";
  menuBarProvider?: "codex" | "claude" | "antigravity";
  /** Exactly one workspace account supplies menu-bar metrics. */
  menuBarAccountId?: string | null;
  menuBarStyle?: "percentages" | "progress";
  accountWindows?: Record<string, string[]>;
  warningColors?: boolean;
  /** Enabled menu-bar color levels, in used percent; an empty list disables all levels. */
  warningThresholds?: number[] | null;
  showAccountName?: boolean | null;
  autoSwitchRemaining?: number;
  /** Independent defaults selected from loaded accounts for each provider. */
  activeAccounts?: Partial<Record<"codex" | "claude" | "antigravity", string>>;
  showRemaining: boolean;
  autoSwitchEnabled: boolean;
  showCodexResets: boolean;
  showProbableResets: boolean;
}

export interface Workspace {
  id: string;
  name: string;
  accent: string;
  accountIds: string[];
  /** Missing on legacy workspaces; new workspaces show only selected CLI providers. */
  providerIds?: string[] | null;
  claudeAccountIds?: string[] | null;
  antigravityAccountIds?: string[];
  accountPreferences?: Record<string, CodexAccountPreferences> | null;
  usage: WorkspaceUsage;
}

export const DEFAULT_WORKSPACE_USAGE: WorkspaceUsage = {
  menuBarEnabled: true, displayMode: "active", showRemaining: true,
  menuBarProvider: "codex", menuBarStyle: "percentages", accountWindows: {}, warningColors: false, autoSwitchRemaining: 2,
  autoSwitchEnabled: false, showCodexResets: true, showProbableResets: true,
};

/** Initial settings also serve as the reset payload. */
export const DEFAULT_APP_SETTINGS: AppSettings = {
  refreshIntervalSecs: 60,
  usageShowRemaining: true,
  menuBarEnabled: false,
  menuBarProvider: "codex",
  menuBarWindow: "5-hour",
  providerAccents: {},
  usageAppearance: {
    monochrome: true,
    showLabels: true,
    showRemaining: true,
    sessionEnabled: true,
    weeklyEnabled: false,
    showTimeMarker: true,
    colorMarkerByPace: true,
    paceAwareColors: false,
  },
  multiProfile: {
    enabled: false,
    selectedIds: [],
    showWeekly: false,
    showProfileLabel: true,
    monochrome: true,
    showRemaining: true,
    showActiveIndicator: true,
  },
  autoSwitchEnabled: false,
  codexDisplayMode: "active",
  codexAccounts: {},
  codexReset: DEFAULT_CODEX_RESET_SETTINGS,
  workspaceVersion: 0,
  activeWorkspaceId: null,
  workspaces: [],
};

/** Persisted backend preferences shared by the panel and settings modal. */
export interface AppSettings {
  refreshIntervalSecs: number;
  /** Default: capacity counts down from 100%; false shows consumed capacity. */
  usageShowRemaining: boolean;
  menuBarEnabled: boolean;
  menuBarProvider: MenuBarProvider;
  menuBarWindow: MenuBarWindow;
  /** Provider identity colors, independent of workspace and saved account colors. */
  providerAccents: Partial<Record<AccentProvider, string>>;
  usageAppearance: UsageAppearance;
  multiProfile: MultiProfileSettings;
  autoSwitchEnabled: boolean;
  codexDisplayMode: "active" | "selected";
  codexAccounts: Record<string, CodexAccountPreferences>;
  codexReset: CodexResetSettings;
  workspaceVersion: number;
  activeWorkspaceId: string | null;
  workspaces: Workspace[];
}

/** Backend preferences plus the OS launch-at-login state. */
export interface AppState {
  settings: { openAtLogin: boolean };
  app: AppSettings;
}

/** A saved ChatGPT login; credentials never cross the native bridge. */
export interface CodexProfile {
  id: string;
  name: string;
  /** Profile name from the saved login, independently of its editable label. */
  displayName?: string | null;
  email: string | null;
  plan: string | null;
  subscription?: import("./subscriptions").Subscription | null;
  connected: boolean;
  connectionError: string | null;
  active: boolean;
  needsActivation: boolean;
  loginCommand: string;
  createdAt?: string | null;
}

/** Codex-only active identity, independent of every other provider’s current login. */
export interface CodexProfilesState {
  profiles: CodexProfile[];
  activeProfileId: string | null;
  canImport: boolean;
  switchError: string | null;
}

/** CLI lifecycle only; no browser auth URL, codes or credentials in IPC. */
export interface CodexProfileLoginStatus {
  profileId: string;
  status: "idle" | "pending" | "completed" | "failed";
  error: string | null;
}

/** Native restart job survives a hidden panel and never makes AI requests. */
export interface CodexRestartStatus {
  profileId: string;
  profileName: string;
  deadlineAt: string;
  phase:
    | "checking"
    | "awaiting_confirmation"
    | "countdown"
    | "quitting"
    | "activating"
    | "reopening"
    | "verified"
    | "failed"
    | "cancelled";
  error: string | null;
  processId: number | null;
  credentialsVerified: boolean;
  runningThreads?: number;
  activityUncertain?: boolean;
}

/** Claude Code owns sign-in and credentials in each isolated config directory. */
export interface ClaudeProfile {
  id: string;
  name: string;
  configDir: string;
  active?: boolean;
  createdAt: string;
  email: string | null;
  connected: boolean;
  subscription: import("./subscriptions").Subscription | null;
  loginCommand: string;
  launchCommand: string;
  quota: import("./api").QuotaSet | null;
}
