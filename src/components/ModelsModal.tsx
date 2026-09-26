import { Chip } from "@heroui/react";
import type { Bucket } from "../types";
import { moneyPrecise } from "../format";
import { ModelMark } from "../logos";
import { AppModal } from "./AppModal";

interface ModelsModalProps {
  keyName: string | null;
  bucket: Bucket | null;
  onClose: () => void;
}

/** "Cost detail" modal (key named in the subtitle) with brand logos. */
export function ModelsModal({ keyName, bucket, onClose }: ModelsModalProps) {
  const rows = bucket
    ? Object.entries(bucket.models).sort((a, b) => b[1] - a[1])
    : [];
  return (
    <AppModal
      open={keyName !== null}
      onClose={onClose}
      title="Cost detail"
      detail={
        keyName ? <Chip className="detail-chip">{keyName}</Chip> : undefined
      }
      cardClass="detail-card"
      bodyId="detail-body"
    >
      <div className="mlist">
        {rows.length === 0 ? (
          <p className="m-empty">No usage found for this key in the selected month.</p>
        ) : (
          rows.map(([model, cost]) => (
            <div className="mrow" key={model}>
              <ModelMark model={model} />
              <span className="m-name" title={model}>
                {model}
              </span>
              <span className="m-cost">{moneyPrecise(cost)}</span>
            </div>
          ))
        )}
      </div>
    </AppModal>
  );
}
