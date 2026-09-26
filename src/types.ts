// IPC contract types — mirrors CONTRACT.md. Keep in sync with src-tauri.

export interface Month {
  /** 0-11, local time */
  year: number;
  month: number;
}

export interface Cred {
  type: string;
  key: string;
  account?: string;
}

export interface ProviderEntry {
  keys: Record<string, Cred>;
  active: string | null;
}

export interface Bucket {
  total: number;
  models: Record<string, number>;
  /** Billed tokens for the bucket, when the provider recorded them. */
  tokens?: number;
}

export interface QuotaWindow {
  window: "rolling" | "weekly" | "monthly";
  percent: number;
  resetsAt: string | null;
  status: string | null;
}

export interface KeyQuota {
  windows: QuotaWindow[] | null;
  error: string | null;
}

export type ResetDisplay = "clock" | "remaining" | "both";
export type TimeFormat = "system" | "12h" | "24h";
export type TrayStyle = "battery" | "bar" | "percent" | "iconbar" | "compact";
export type TrayColorMode = "multi" | "mono" | "custom";
export type TrayUsedMode = "percent" | "tokens";
/** Which window the single-value tray styles speak for. */
export type TrayPrimary = "auto" | "rolling" | "weekly" | "monthly" | "worst";

export interface AppSettings {
  resetDisplay: ResetDisplay;
  usagePercentMode: "used" | "remaining" | "both";
  timeFormat: TimeFormat;
  refreshIntervalSecs: number;
  autoSwitch: boolean;
  notifyEnabled: boolean;
  notifyThresholds: number[];
  notifyCustom: number | null;
  soundEnabled: boolean;
  soundName: string | null;
  trayStyle: TrayStyle;
  trayColorMode: TrayColorMode;
  trayCustomColor: string;
  trayUsedMode: TrayUsedMode;
  trayShowRemaining: boolean;
  trayShowLabels: boolean;
  trayShowPace: boolean;
  trayMetrics: string[];
  trayPrimary: TrayPrimary;
  trayMulti: boolean;
  trayProfiles: string[];
  trayActiveIndicator: boolean;
  billingDay: number;
  autoUpdateChecks: boolean;
}

export interface ProfileRef {
  id: string;
  name: string;
}

export interface AppState {
  providers: Record<string, ProviderEntry>;
  discovered: string[];
  applied: Record<string, string | null>;
  usage: {
    byKey: Record<string, Record<string, Bucket>>;
    untracked: Record<string, Bucket>;
    error: string | null;
  };
  providerUsage: Record<string, Record<string, KeyQuota>>;
  settings: { openAtLogin: boolean; theme: string };
  customProviders: string[];
  month: Month;
  profiles: ProfileRef[];
  activeProfile: string;
  app: AppSettings;
}

export interface HistoryBucket {
  t: number;
  cost: number;
  tokens: number;
}

export interface HistoryOut {
  range: string;
  label: string;
  start: number;
  end: number;
  bucketMs: number;
  buckets: HistoryBucket[];
  totalCost: number;
  totalTokens: number;
  models: Record<string, number>;
  key: string | null;
  error: string | null;
}

export interface CodexStatus {
  present: boolean;
  hasApiKey: boolean;
  hasTokens: boolean;
  email?: string | null;
  plan?: string | null;
  accountId?: string | null;
  expiresAt?: number | null;
  lastRefresh?: string | null;
}

export interface CodexOut {
  status: CodexStatus;
  keys: string[];
}

export interface UpdateOut {
  available: boolean;
  version?: string | null;
  notes?: string | null;
  error?: string | null;
}
