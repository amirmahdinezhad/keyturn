// IPC contract types — mirrors CONTRACT.md. Keep in sync with src-tauri.
//
// CLI-only contract: auth lives in the provider CLIs, quotas are gone with
// stored keys. Settings carry refresh cadence, billing day and update checks.

export interface AppSettings {
  refreshIntervalSecs: number;
  billingDay: number;
  autoUpdateChecks: boolean;
}

export interface ProfileRef {
  id: string;
  name: string;
}

export interface AppState {
  settings: { openAtLogin: boolean; theme: string };
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
  error: string | null;
}

export interface UpdateOut {
  available: boolean;
  version?: string | null;
  notes?: string | null;
  error?: string | null;
}
