import type { AppSettings, AppState } from "../types";

export const DEFAULT_APP_SETTINGS: AppSettings = {
  refreshIntervalSecs: 60,
  billingDay: 1,
  autoUpdateChecks: false,
};

export function makeState(over: Partial<AppState> = {}): AppState {
  return {
    settings: { openAtLogin: false, theme: "dark" },
    profiles: [{ id: "default", name: "Default" }],
    activeProfile: "default",
    app: DEFAULT_APP_SETTINGS,
    ...over,
  };
}
