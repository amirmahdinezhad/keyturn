import { exactPercent, quotaLevel, resetText, type PaceInfo } from "../format";
import type { ResetDisplay, TimeFormat } from "../types";

export type PercentMode = "used" | "remaining" | "both";

export interface UsageRow {
  label: string;
  percent: number;
  /** Window id ("rolling" | "weekly" | "monthly") when known. */
  window?: string;
  resetsAt?: string | null;
  /** Precomputed reset wording (site-provided for Go windows). */
  reset?: string;
  pace?: PaceInfo | null;
}

export interface UsageDisplay {
  mode: PercentMode;
  reset: ResetDisplay;
  time: TimeFormat;
  showPace: boolean;
}

export const DEFAULT_USAGE_DISPLAY: UsageDisplay = {
  mode: "used",
  reset: "remaining",
  time: "system",
  showPace: true,
};

function percentText(percent: number, mode: PercentMode): string {
  const used = exactPercent(percent);
  const remaining = exactPercent(100 - percent);
  if (mode === "remaining") return `${remaining}% left`;
  if (mode === "both") return `${used}% used · ${remaining}% left`;
  return `${used}%`;
}

/**
 * Quota rows: label, percent, a thin bar with an optional pace marker, and the
 * reset line. These carry the exact per-window detail and live inside the
 * usage limits modal.
 */
export function UsageRows({
  rows,
  display = DEFAULT_USAGE_DISPLAY,
}: {
  rows: UsageRow[];
  display?: UsageDisplay;
}) {
  if (rows.length === 0) {
    return <p className="m-empty">No usage limits reported for this account yet.</p>;
  }
  return (
    <div className="usage-list">
      {rows.map((row) => {
        const level = quotaLevel(row.percent);
        const reset =
          row.reset ??
          (row.resetsAt ? resetText(row.resetsAt, display.reset, display.time) : "");
        const pace = display.showPace ? row.pace : null;
        return (
          <div className="usage-row" key={row.label}>
            <div className="usage-top">
              <span className="usage-name">{row.label}</span>
              <span className={`usage-pct ${level}`}>
                {percentText(row.percent, display.mode)}
              </span>
            </div>
            <div className="usage-bar">
              <span
                className={`usage-fill ${level}`}
                style={{ width: `${Math.min(100, Math.max(0, row.percent))}%` }}
              />
              {pace ? (
                <span
                  className={`pace-marker pace-${pace.level}`}
                  style={{ left: `${Math.min(100, pace.marker)}%` }}
                  title={`Projected ${exactPercent(pace.projected)}% by reset`}
                />
              ) : null}
            </div>
            <div className="usage-meta">
              {reset ? <span className="usage-reset">{reset}</span> : <span />}
              {pace ? (
                <span className={`pace-label pace-${pace.level}`}>
                  {pace.label} · {exactPercent(pace.projected)}%
                </span>
              ) : null}
            </div>
          </div>
        );
      })}
    </div>
  );
}
