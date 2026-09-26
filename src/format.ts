// Pure formatting helpers — ported 1:1 from the Electron ui.js.

import type { ResetDisplay, TimeFormat } from "./types";

export function mask(key: string): string {
  return key.length > 10 ? `${key.slice(0, 5)}…${key.slice(-4)}` : "•••";
}

/** Exact decimal text: up to 4 places, no rounding below that, and no
 * float-noise tails (37.4 stays "37.4", 0.0725 stays "0.0725"). */
export function exactDecimal(v: number): string {
  if (!Number.isFinite(v)) return "0";
  // Cap at 4 decimal places, then strip trailing zeros so 26.8600 → "26.86".
  return v
    .toFixed(4)
    .replace(/(\.\d*?)0+$/, "$1")
    .replace(/\.$/, "");
}

/** Percent with its real decimals (37.4% never becomes 37%). */
export function exactPercent(v: number): string {
  return exactDecimal(v);
}

export function money(v: number): string {
  return `$${exactDecimal(v)}`;
}

export function moneyPrecise(v: number): string {
  if (v <= 0) return "$0";
  return money(v);
}

export const QUOTA_LABELS: Record<string, string> = {
  rolling: "5-hour Usage",
  weekly: "Weekly Usage",
  monthly: "Monthly Usage",
};

export function plural(n: number, unit: string): string {
  return `${n} ${unit}${n === 1 ? "" : "s"}`;
}

// Same wording as the OpenCode panel: "4 hours 58 minutes" / "5 days 14 hours".
export function resetsIn(iso: string | null): string {
  if (!iso) return "";
  const ms = new Date(iso).getTime() - Date.now();
  if (!Number.isFinite(ms)) return "";
  if (ms <= 0) return "now";
  const totalMinutes = Math.floor(ms / 60000);
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  if (days >= 1) return `${plural(days, "day")} ${plural(hours, "hour")}`;
  if (hours >= 1) return `${plural(hours, "hour")} ${plural(minutes, "minute")}`;
  return plural(minutes, "minute");
}

export function formatClock(iso: string, format: TimeFormat): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const options: Intl.DateTimeFormatOptions =
    format === "24h"
      ? { hour: "2-digit", minute: "2-digit", hour12: false }
      : format === "12h"
        ? { hour: "numeric", minute: "2-digit", hour12: true }
        : { hour: "numeric", minute: "2-digit" };
  return date.toLocaleTimeString("en-US", options);
}

/** "Resets at 14:30", "Resets in 4 hours 58 minutes", or both. */
export function resetText(
  iso: string | null,
  mode: ResetDisplay,
  format: TimeFormat,
): string {
  if (!iso) return "";
  const remaining = resetsIn(iso);
  const clock = formatClock(iso, format);
  if (mode === "clock") return clock ? `Resets at ${clock}` : "";
  if (mode === "both") {
    return clock ? `Resets at ${clock} · ${remaining}` : `Resets in ${remaining}`;
  }
  return `Resets in ${remaining}`;
}

export function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) {
    const value = tokens / 1_000_000;
    return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)}M`;
  }
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(0)}K`;
  return `${Math.round(tokens)}`;
}

export type QuotaLevel = "hot" | "warm" | "ok";

export function quotaLevel(percent: number): QuotaLevel {
  if (percent >= 90) return "hot";
  if (percent >= 70) return "warm";
  return "ok";
}

export type PaceLevel =
  | "comfortable"
  | "light"
  | "steady"
  | "elevated"
  | "critical"
  | "runaway";

export interface PaceInfo {
  /** Where the window lands if usage keeps its current pace. */
  projected: number;
  level: PaceLevel;
  label: string;
  /** Bar position for the pace marker (clamped to 130%). */
  marker: number;
}

const WINDOW_SECONDS: Record<string, number> = {
  rolling: 5 * 3600,
  weekly: 7 * 24 * 3600,
  monthly: 30 * 24 * 3600,
};

export const PACE_LABELS: Record<PaceLevel, string> = {
  comfortable: "Comfortable",
  light: "Light",
  steady: "Steady",
  elevated: "Elevated",
  critical: "Critical",
  runaway: "Runaway",
};

export function paceLevel(projected: number): PaceLevel {
  if (projected >= 120) return "runaway";
  if (projected >= 100) return "critical";
  if (projected >= 90) return "elevated";
  if (projected >= 70) return "steady";
  if (projected >= 50) return "light";
  return "comfortable";
}

/**
 * Linear projection from the window's elapsed fraction. Mirrors
 * `tray::projected` in the backend.
 */
export function paceInfo(
  percent: number,
  resetsAt: string | null,
  window: string,
): PaceInfo | null {
  const duration = WINDOW_SECONDS[window];
  if (!duration || !resetsAt) return null;
  const resets = new Date(resetsAt).getTime();
  if (!Number.isFinite(resets)) return null;
  const remaining = (resets - Date.now()) / 1000;
  if (remaining <= 0 || remaining >= duration) return null;
  const elapsed = duration - remaining;
  if (elapsed < duration * 0.05) return null;
  const projected = Math.max(percent, percent / (elapsed / duration));
  const level = paceLevel(projected);
  return {
    projected,
    level,
    label: PACE_LABELS[level],
    marker: Math.min(130, projected),
  };
}

export function prettyProvider(providerID: string): string {
  if (/^opencode$/i.test(providerID)) return "OpenCode";
  if (/^opencode[-_]?go$/i.test(providerID)) return "OpenCode Go";
  return providerID
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

export function monthLabel(m: Month): string {
  return new Date(m.year, m.month, 1).toLocaleString("en-US", {
    month: "long",
    year: "numeric",
  });
}

export interface Month {
  year: number;
  month: number;
}

export type ModelMarkKey =
  | "kimi"
  | "zhipu"
  | "deepseek"
  | "openai"
  | "claude"
  | "qwen"
  | "gemini"
  | "grok"
  | "mistral"
  | "minimax";

// Brand matchers — same order and patterns as the Electron modelLogo().
export function modelLogoKey(model: string): ModelMarkKey | null {
  const lower = model.toLowerCase();
  if (/kimi|moonshot/.test(lower)) return "kimi";
  if (/glm|zhipu/.test(lower)) return "zhipu";
  if (/deepseek/.test(lower)) return "deepseek";
  if (/gpt|openai|^o\d/.test(lower)) return "openai";
  if (/claude/.test(lower)) return "claude";
  if (/qwen|tongyi/.test(lower)) return "qwen";
  if (/gemini/.test(lower)) return "gemini";
  if (/grok/.test(lower)) return "grok";
  if (/mistral/.test(lower)) return "mistral";
  if (/minimax/.test(lower)) return "minimax";
  return null;
}

/** Letter-avatar background so the same id always gets the same hue. */
export function hashHue(s: string): number {
  let hash = 0;
  for (const ch of s) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return hash % 360;
}
