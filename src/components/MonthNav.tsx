import { Button } from "@heroui/react";
import type { Month } from "../types";
import { money, monthLabel } from "../format";
import { NextIcon, PrevIcon } from "../icons";

interface MonthNavProps {
  selected: Month;
  isCurrentMonth: boolean;
  onShift: (delta: -1 | 1) => void;
  /** Spend for the selected month, shown as the panel's hero figure. */
  total?: number | null;
  /** Costs are still being fetched for the selected month. */
  loading?: boolean;
}

/**
 * ‹ September 2026 › with the month's spend underneath — the one figure the
 * whole panel is built around.
 */
export function MonthNav({
  selected,
  isCurrentMonth,
  onShift,
  total,
  loading = false,
}: MonthNavProps) {
  const hasTotal = typeof total === "number";
  return (
    <div className="month-nav">
      <div className="month-box">
        <Button
          className="icon nav"
          aria-label="Previous month"
          onPress={() => onShift(-1)}
        >
          <PrevIcon size={16} />
        </Button>
        <span id="month-label">{monthLabel(selected)}</span>
        <Button
          className="icon nav"
          aria-label="Next month"
          isDisabled={isCurrentMonth}
          onPress={() => onShift(1)}
        >
          <NextIcon size={16} />
        </Button>
      </div>
      <div className="month-total" aria-live="polite">
        {loading && !hasTotal ? (
          <span className="month-total-value pending">—</span>
        ) : (
          <span className="month-total-value">{money(total ?? 0)}</span>
        )}
        <span className="month-total-label">
          {isCurrentMonth ? "spent this month" : "month total"}
        </span>
      </div>
    </div>
  );
}
