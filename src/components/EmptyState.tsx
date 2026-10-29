import type { ReactNode } from "react";
import { InfoCircleIcon } from "../icons";

/** Keep recovery guidance beside the state that needs attention. */
export function EmptyState({ message, title, action }: {
  message: string;
  title?: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty-state">
      <span className="empty-state-symbol" aria-hidden="true"><InfoCircleIcon size={24} /></span>
      {title ? <h3>{title}</h3> : null}
      <p>{message}</p>
      {action}
    </div>
  );
}
