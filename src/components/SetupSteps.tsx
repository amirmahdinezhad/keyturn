import type { CSSProperties } from "react";
import { TickIcon } from "../icons";

/** A connected progress track that animates in either direction between steps. */
export function SetupSteps({ labels, current, label }: { labels: string[]; current: number; label: string }) {
  return <ol className="setup-steps" aria-label={label} style={{
    "--step-count": labels.length,
    "--step-progress": labels.length > 1 ? current / (labels.length - 1) : 0,
  } as CSSProperties}>
    {labels.map((name, index) => <li key={name}
      className={`wizard-step${index < current ? " is-done" : ""}${index === current ? " is-now" : ""}`}
      aria-current={index === current ? "step" : undefined}>
      <span className="setup-step-dot" aria-hidden="true">{index < current ? <TickIcon size={16} /> : index + 1}</span>
      <span>{name}</span>
    </li>)}
  </ol>;
}
