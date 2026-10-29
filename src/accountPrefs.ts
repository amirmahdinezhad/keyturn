import type { QuotaSet } from "./api";
import { DEFAULT_CODEX_ACCOUNT_PREFERENCES, type AppSettings, type CodexAccountPreferences, type UsageAppearance } from "./types";

/** Accept provider main windows, excluding model-specific and reserve buckets. */
export function canonicalAccountWindow(value: string): string | null {
  const key = value.toLowerCase();
  if (key === "rolling" || key === "5-hour") return "5-hour";
  return ["weekly", "monthly"].includes(key) || /^[1-9]\d{0,9}[mhd]$/.test(key) ? key : null;
}

/** Preserve learned windows during outages; prices never imply window support. */
export function accountWindows(quota: QuotaSet | null, prefs: CodexAccountPreferences): string[] {
  const reported = quota?.error ? [] : (quota?.windows ?? []).map((w) => canonicalAccountWindow(w.window)).filter((w): w is string => w !== null);
  return [...new Set([...prefs.knownWindows, ...prefs.menuBarWindows, ...reported])];
}

/** Missing account preferences are fresh opt-in settings, never legacy shared values. */
export function codexAccountPreferences(app: AppSettings, id: string): CodexAccountPreferences {
  const workspace = app.workspaces?.find((w) => w.id === app.activeWorkspaceId);
  return (workspace?.accountPreferences != null ? workspace.accountPreferences[id] : app.codexAccounts?.[id]) ?? { ...DEFAULT_CODEX_ACCOUNT_PREFERENCES, color: app.providerAccents?.codex ?? DEFAULT_CODEX_ACCOUNT_PREFERENCES.color };
}

/** Readable labels for the actual windows returned by the provider. */
export function accountWindowLabel(window: string): string {
  return ({ "5-hour": "5-hour", weekly: "Weekly", monthly: "Monthly" } as Record<string, string>)[window] ?? window;
}

/** Codex follows the CLI's remaining-capacity convention on every surface. */
export function codexUsageAppearance(prefs: CodexAccountPreferences, base: UsageAppearance): UsageAppearance {
  return { ...base, showRemaining: true, showTimeMarker: prefs.showTimeMarker, colorMarkerByPace: prefs.colorMarkerByPace, paceAwareColors: prefs.paceAwareColors };
}
