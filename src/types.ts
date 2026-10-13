// IPC contract types — mirrors CONTRACT.md. Keep in sync with src-tauri.
//
// CLI-only contract: auth lives in the provider CLIs. Settings carry
// refresh cadence and quota wording; the panel shows CLI status and limits.

/** Persisted backend preferences shared by the panel and settings modal. */
export interface AppSettings {
  refreshIntervalSecs: number;
  usageShowRemaining: boolean;
}

/** Backend preferences plus the OS launch-at-login state. */
export interface AppState {
  settings: { openAtLogin: boolean };
  app: AppSettings;
}
