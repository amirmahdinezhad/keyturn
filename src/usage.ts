import type { QuotaWindow } from "./api";

/** Elapsed fraction and end-of-period projection for known session/week windows. */
export function usagePace(
  window: QuotaWindow,
  now = Date.now(),
): { elapsed: number; projected: number | null } | null {
  const duration = ["5-hour", "rolling"].includes(window.window.toLowerCase())
    ? 18_000_000
    : window.window.toLowerCase() === "weekly"
      ? 604_800_000
      : null;
  const reset = Date.parse(window.resetsAt ?? "");
  if (
    !duration ||
    !Number.isFinite(reset) ||
    !Number.isFinite(window.percent) ||
    reset <= now ||
    reset - now > duration
  )
    return null;
  const elapsed = (now - (reset - duration)) / duration;
  return {
    elapsed,
    projected: elapsed > 0.01 ? Math.max(0, window.percent) / elapsed : null,
  };
}

/** Status depends on used capacity; pace can extend beyond 100% into purple. */
export function usageColor(used: number, projected?: number | null): string {
  const percent = projected ?? used;
  return projected != null && percent >= 125
    ? "#bf5af2"
    : percent >= 95
      ? "#ff453a"
      : percent >= 90
        ? "#ff9f0a"
        : percent >= 75
          ? "#e5b92f"
          : "#30b86b";
}
