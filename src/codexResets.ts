import type { CodexResetStatus } from "./api";

type ScheduledReset = NonNullable<CodexResetStatus["scheduledReset"]>;
export const RESET_DISMISSAL_KEY = "keyturn.dismissedCodexAnnouncements";

/** Identity excludes snapshot generation time so refreshes do not reopen a notice. */
export function resetAnnouncementId(reset: ScheduledReset): string {
  return JSON.stringify([reset.resetType, reset.announcedAt, reset.scheduledFor]);
}

/** Probability updates do not reopen the same dismissed forecast. */
export function resetWatchId(watch: NonNullable<CodexResetStatus["activeWatch"]>): string {
  return JSON.stringify(["watch", watch.observedAt, watch.expiresAt]);
}

export function loadDismissedResets(): string[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(RESET_DISMISSAL_KEY) ?? "[]");
    return Array.isArray(value) ? value.filter((id): id is string => typeof id === "string").slice(-100) : [];
  } catch {
    return [];
  }
}

export function saveDismissedResets(ids: string[]): void {
  try {
    localStorage.setItem(RESET_DISMISSAL_KEY, JSON.stringify(ids.slice(-100)));
  } catch {
    // Closing still works for this session if storage is unavailable.
  }
}

/** Total hours preserve minute precision even for resets more than a day away. */
export function resetCountdown(date: string, now: number): string {
  const ms = Date.parse(date) - now;
  if (!Number.isFinite(ms) || ms <= 0) return "";
  if (ms < 60_000) return "Resets in <1m";
  const minutes = Math.ceil(ms / 60_000);
  return `Resets in ${Math.floor(minutes / 60)}h ${String(minutes % 60).padStart(2, "0")}m`;
}

/** Local clock time only; timezone offsets add noise to this short notice. */
export function resetLocalTime(date: string): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit", minute: "2-digit", hour12: false,
  }).format(new Date(date));
}

/** The source uses watch expiry as the forecast's upper bound, not an exact reset time. */
export function resetForecastTiming(date: string, now: number): string {
  const end = new Date(date);
  const remaining = end.getTime() - now;
  if (!Number.isFinite(remaining) || remaining <= 0) return "";
  const duration = remaining < 60_000 ? "<1m"
    : remaining < 3_600_000 ? `${Math.ceil(remaining / 60_000)}m`
      : `${Math.ceil(remaining / 3_600_000)}h`;
  const today = new Date(now);
  const tomorrow = new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1);
  const sameDay = (day: Date) => end.getFullYear() === day.getFullYear()
    && end.getMonth() === day.getMonth() && end.getDate() === day.getDate();
  const day = sameDay(today) ? "today" : sameDay(tomorrow) ? "tomorrow"
    : new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(end);
  return `within ${duration}, ${day} at ${resetLocalTime(date)}`;
}
