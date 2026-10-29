import type { UsageAppearance } from "../types";
import { claudeProfileQuota } from "../api";
import type { ProviderCliStatus } from "../api";
import { ProviderLogo } from "../logos";
import { AppModal } from "./AppModal";
import { QuotaContent } from "./CliProviders";
import type { ToastKind } from "./Toast";

interface UsageModalProps {
  /** Provider to show limits for, or null when closed. */
  status: ProviderCliStatus | null;
  showRemaining: boolean;
  /** Refresh an open popup with the app's configured data refresh cycle. */
  refreshKey?: number;
  appearance?: UsageAppearance;
  onClose: () => void;
  notify: (message: string, kind?: ToastKind) => void;
}

const QUOTA_IDS: Record<string, string[]> = {
  opencode: ["opencode", "opencode-go"],
  claude: ["claude"],
  codex: ["codex"],
};

const EMPTY_HINTS: Record<string, string> = {
  opencode: "No quota data for the stored keys.",
  claude: "No usage data for this Claude account.",
  codex: "No quota data for this account.",
};

/**
 * Usage limits popup: percent bars with resets for one provider.
 * Read-only throughout — the data comes from the CLI's own sources.
 */
export function UsageModal({ status, showRemaining, refreshKey, appearance, onClose, notify }: UsageModalProps) {
  return (
    <AppModal
      open={status !== null}
      onClose={onClose}
      title="Usage limits"
      cardClass="usage-card"
      detail={
        status ? (
          <div className="modal-provider">
            <ProviderLogo provider={status.id} />
            <span>{status.label}</span>
          </div>
        ) : null
      }
    >
      {status ? (
        <QuotaContent
          key={status.id}
          ids={QUOTA_IDS[status.id] ?? [status.id]}
          emptyHint={EMPTY_HINTS[status.id] ?? "No quota data."}
          showRemaining={status.id === "codex" || showRemaining}
          refreshKey={refreshKey}
          appearance={appearance}
          notify={notify}
          loadQuota={status.id === "claude" ? () => claudeProfileQuota() : undefined}
        />
      ) : null}
    </AppModal>
  );
}
