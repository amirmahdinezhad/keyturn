import type {
  AppSettings,
  AppState,
  Bucket,
  KeyQuota,
  Month,
} from "../types";

export const DEFAULT_APP_SETTINGS: AppSettings = {
  resetDisplay: "remaining",
  usagePercentMode: "used",
  timeFormat: "system",
  refreshIntervalSecs: 60,
  autoSwitch: true,
  notifyEnabled: true,
  notifyThresholds: [75, 90, 95],
  notifyCustom: null,
  soundEnabled: true,
  soundName: null,
  trayStyle: "battery",
  trayColorMode: "multi",
  trayCustomColor: "#007aff",
  trayUsedMode: "percent",
  trayShowRemaining: true,
  trayShowLabels: false,
  trayShowPace: true,
  trayMetrics: ["rolling"],
  trayPrimary: "auto",
  trayMulti: false,
  trayProfiles: [],
  trayActiveIndicator: true,
  billingDay: 1,
  autoUpdateChecks: false,
};

export const SEPTEMBER: Month = { year: 2026, month: 8 };

export function makeBucket(over: Partial<Bucket> = {}): Bucket {
  return { total: 0, models: {}, ...over };
}

export function makeQuota(over: Partial<KeyQuota> = {}): KeyQuota {
  return {
    windows: [
      { window: "rolling", percent: 40, resetsAt: null, status: "ok" },
      { window: "weekly", percent: 12, resetsAt: null, status: "ok" },
    ],
    error: null,
    ...over,
  };
}

export function makeState(over: Partial<AppState> = {}): AppState {
  return {
    providers: {},
    discovered: [],
    applied: {},
    usage: { byKey: {}, untracked: {}, error: null },
    providerUsage: {},
    settings: { openAtLogin: false, theme: "dark" },
    customProviders: [],
    month: SEPTEMBER,
    profiles: [{ id: "default", name: "Default" }],
    activeProfile: "default",
    app: DEFAULT_APP_SETTINGS,
    ...over,
  };
}
