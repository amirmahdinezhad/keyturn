import type { QuotaSet } from "./api";
import { canonicalAccountWindow } from "./accountPrefs";
import { isSubscriptionExpired } from "./subscriptions";
import type { CodexProfile } from "./types";

/** Personal usage limits only; public reset announcements never determine capacity. */
export function accountLimitText(quota: QuotaSet | null | undefined, now: number): string | null {
  if (!quota || quota.error) return null;
  const exhausted = (quota.windows ?? []).filter((w) => canonicalAccountWindow(w.window) && Number.isFinite(w.percent) && w.percent >= 100);
  if (!exhausted.length) return null;
  const resets = exhausted.map((w) => Date.parse(w.resetsAt ?? ""));
  if (resets.some((reset) => !Number.isFinite(reset))) return "Reset time unavailable";
  const latest = Math.max(...resets);
  if (latest <= now) return "Checking reset…";
  const minutes = Math.ceil((latest - now) / 60_000);
  return `Resets in ${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

/** Choose the next saved, connected account with fresh, usable main limits. */
export function nextUsableAccount(
  profiles: CodexProfile[], activeId: string, quotas: Record<string, QuotaSet | null>, now: number,
  knownWindows: Record<string, string[]> = {},
): CodexProfile | null {
  const index = profiles.findIndex((p) => p.id === activeId);
  if (index < 0) return null;
  for (let offset = 1; offset < profiles.length; offset++) {
    const profile = profiles[(index + offset) % profiles.length];
    if (!profile.connected || profile.connectionError || isSubscriptionExpired(profile.subscription, now)) continue;
    const quota = quotas[profile.id];
    if (!quota || quota.error) continue;
    const windows = (quota.windows ?? []).filter((w) => canonicalAccountWindow(w.window));
    if (!windows.length || windows.some((w) => !Number.isFinite(w.percent) || w.percent < 0 || w.percent >= 100
      || (w.resetsAt != null && (!Number.isFinite(Date.parse(w.resetsAt)) || Date.parse(w.resetsAt) <= now)))) continue;
    if ((knownWindows[profile.id] ?? []).some((kind) => !windows.some((w) => canonicalAccountWindow(w.window) === kind))) continue;
    return profile;
  }
  return null;
}
