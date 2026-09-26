import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@heroui/react";
import { exportHistory, history, prettyError } from "../api";
import { formatTokens, money, moneyPrecise } from "../format";
import type { HistoryOut } from "../types";
import { AppModal } from "./AppModal";
import type { ToastKind } from "./Toast";

const RANGES: { id: string; label: string }[] = [
  { id: "5h", label: "5h" },
  { id: "24h", label: "24h" },
  { id: "7d", label: "7d" },
  { id: "30d", label: "30d" },
  { id: "billing", label: "Billing" },
];

const WIDTH = 320;
const HEIGHT = 110;
const PAD = 6;

/** Cost/token chart over 5h/24h/7d/30d/billing with JSON and CSV export. */
export function HistoryModal({
  open,
  onClose,
  notify,
}: {
  open: boolean;
  onClose: () => void;
  notify: (message: string, kind?: ToastKind) => void;
}) {
  const [range, setRange] = useState("24h");
  const [out, setOut] = useState<HistoryOut | null>(null);
  const [loading, setLoading] = useState(false);
  const [hover, setHover] = useState<number | null>(null);

  const load = useCallback(
    async (nextRange: string) => {
      setLoading(true);
      try {
        setOut(await history(nextRange));
      } catch (e) {
        notify(prettyError(e), "err");
      } finally {
        setLoading(false);
      }
    },
    [notify],
  );

  useEffect(() => {
    if (open) void load(range);
  }, [open, range, load]);

  const chart = useMemo(() => {
    const buckets = out?.buckets ?? [];
    const maxCost = Math.max(0.0001, ...buckets.map((b) => b.cost));
    const maxTokens = Math.max(1, ...buckets.map((b) => b.tokens));
    const slot = (WIDTH - PAD * 2) / Math.max(1, buckets.length);
    const barWidth = Math.max(1, slot - 1);
    const points = buckets
      .map((bucket, index) => {
        const x = PAD + index * slot + slot / 2;
        const y =
          HEIGHT - PAD - (bucket.tokens / maxTokens) * (HEIGHT - PAD * 2);
        return `${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
    return { buckets, maxCost, slot, barWidth, points };
  }, [out]);

  const models = useMemo(
    () =>
      Object.entries(out?.models ?? {})
        .sort((a, b) => b[1] - a[1])
        .slice(0, 6),
    [out],
  );

  const doExport = async (format: "json" | "csv") => {
    try {
      const path = await exportHistory(range, format);
      notify(`Exported to ${path}`, "ok");
    } catch (e) {
      notify(prettyError(e), "err");
    }
  };

  const hovered = hover !== null ? out?.buckets[hover] : null;

  return (
    <AppModal
      open={open}
      onClose={onClose}
      title="History"
      cardClass="detail-card history-card"
      bodyId="detail-body"
    >
      <div className="history-tabs" role="tablist">
        {RANGES.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={range === item.id}
            className={`settings-tab${range === item.id ? " on" : ""}`}
            onClick={() => setRange(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>

      <div className="history-totals">
        <div className="history-total">
          <span className="history-value">{money(out?.totalCost ?? 0)}</span>
          <span className="history-label">{out?.label ?? "Cost"}</span>
        </div>
        <div className="history-total">
          <span className="history-value">
            {formatTokens(out?.totalTokens ?? 0)}
          </span>
          <span className="history-label">tokens</span>
        </div>
      </div>

      {loading && !out ? (
        <p className="m-empty">Loading history…</p>
      ) : !out || out.buckets.every((b) => b.cost === 0 && b.tokens === 0) ? (
        <p className="m-empty">
          No usage recorded in this range
          {out?.error ? ` — ${out.error}` : ""}.
        </p>
      ) : (
        <>
          <svg
            className="history-chart"
            viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
            role="img"
            aria-label={`Cost and tokens over ${out.label}`}
          >
            <line
              className="chart-grid"
              x1={PAD}
              x2={WIDTH - PAD}
              y1={HEIGHT - PAD}
              y2={HEIGHT - PAD}
            />
            {chart.buckets.map((bucket, index) => {
              const height =
                (bucket.cost / chart.maxCost) * (HEIGHT - PAD * 2);
              const x = PAD + index * chart.slot;
              return (
                <rect
                  key={bucket.t}
                  className={`chart-bar${hover === index ? " on" : ""}`}
                  x={x}
                  y={HEIGHT - PAD - height}
                  width={chart.barWidth}
                  height={Math.max(bucket.cost > 0 ? 1.5 : 0, height)}
                  onMouseEnter={() => setHover(index)}
                  onMouseLeave={() => setHover(null)}
                />
              );
            })}
            <polyline className="chart-line" points={chart.points} />
          </svg>
          <div className="history-axis">
            <span>{new Date(out.start).toLocaleString()}</span>
            <span>
              {hovered
                ? `${moneyPrecise(hovered.cost)} · ${formatTokens(hovered.tokens)} tok`
                : new Date(out.end).toLocaleTimeString()}
            </span>
          </div>

          {models.length > 0 ? (
            <div className="history-models">
              {models.map(([model, cost]) => (
                <div className="history-model" key={model}>
                  <span className="m-name" title={model}>
                    {model}
                  </span>
                  <span className="m-cost">{moneyPrecise(cost)}</span>
                </div>
              ))}
            </div>
          ) : null}
        </>
      )}

      <div className="history-actions">
        <Button size="sm" className="disc" onPress={() => void doExport("csv")}>
          Export CSV
        </Button>
        <Button size="sm" className="disc" onPress={() => void doExport("json")}>
          Export JSON
        </Button>
      </div>
    </AppModal>
  );
}
