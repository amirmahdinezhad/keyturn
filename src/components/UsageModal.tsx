import { Chip } from "@heroui/react";
import type { QuotaWindow } from "../types";
import { QUOTA_LABELS, paceInfo, resetText } from "../format";
import { AppModal } from "./AppModal";
import { ProviderLogo } from "../logos";
import { DEFAULT_USAGE_DISPLAY, UsageRows, type UsageDisplay, type UsageRow } from "./UsageRows";

export interface UsageDetail {
  /** Which account/provider these limits belong to. */
  label: string;
  providerID: string;
  windows: QuotaWindow[];
  /** Exact Go-panel windows (label, decimal percent, site reset text). */
  go?: { label: string; percent: number; reset: string }[];
}

/** "Usage limits" modal (account named in the subtitle) with exact percents,
 * reset wording per the user's display mode, pace projection and level bars. */
export function UsageModal({
  detail,
  onClose,
  display = DEFAULT_USAGE_DISPLAY,
}: {
  detail: UsageDetail | null;
  onClose: () => void;
  display?: UsageDisplay;
}) {
  const rows: UsageRow[] = detail?.go?.length
    ? detail.go.map((g) => ({
        label: g.label,
        percent: g.percent,
        reset: g.reset,
      }))
    : (detail?.windows ?? []).map((w) => ({
        label: QUOTA_LABELS[w.window] || w.window,
        percent: w.percent,
        window: w.window,
        resetsAt: w.resetsAt,
        reset: w.resetsAt
          ? resetText(w.resetsAt, display.reset, display.time)
          : undefined,
        pace: paceInfo(w.percent, w.resetsAt, w.window),
      }));

  return (
    <AppModal
      open={detail !== null}
      onClose={onClose}
      title="Usage limits"
      detail={
        detail ? (
          <Chip className="detail-chip">
            <ProviderLogo provider={detail.providerID} />
            <span>{detail.label}</span>
          </Chip>
        ) : undefined
      }
      cardClass="detail-card"
      bodyId="detail-body"
    >
      <UsageRows rows={rows} display={display} />
    </AppModal>
  );
}
