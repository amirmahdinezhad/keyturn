import type { AppSettings, AppState } from "../types";

export const DEFAULT_APP_SETTINGS: AppSettings = {
  refreshIntervalSecs: 60,
  usageShowRemaining: false,
};

export function makeState(over: Partial<AppState> = {}): AppState {
  return {
    settings: { openAtLogin: false },
    app: DEFAULT_APP_SETTINGS,
    ...over,
  };
}
