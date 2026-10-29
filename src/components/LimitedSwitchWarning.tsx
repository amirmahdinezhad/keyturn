import { RefreshIcon, UsageIcon } from "../icons";
import { AppModal } from "./AppModal";
import { Button } from "./ui/button";

/** A warning acknowledges only this destination in this workspace. */
export interface LimitedSwitchTarget {
  id: string;
  workspaceId: string;
  name: string;
  email: string | null;
  reset: string;
}

/** Capacity is advisory; continuing still requires confirmation for running threads. */
export function LimitedSwitchWarning({ target, busy, error, onClose, onContinue }: {
  target: LimitedSwitchTarget | null;
  busy: boolean;
  error: string | null;
  onClose: () => void;
  onContinue: () => void;
}) {
  return <AppModal open={!!target} onClose={onClose} title="Account limit reached" cardClass="limit-switch-card" busy={busy} busyMessage="Starting account switch…">
    {target ? <div className="limit-switch-content">
      <div className="limit-switch-account"><span className="limit-switch-symbol"><UsageIcon size={20} /></span><div><strong>{target.name}</strong>{target.email ? <span>{target.email}</span> : null}</div></div>
      <p className="limit-switch-copy">This account has no remaining capacity. You can still switch, but Codex may stay limited until it resets.</p>
      <div className="limit-switch-reset"><RefreshIcon size={15} /><span>{target.reset}</span></div>
      {error ? <p className="limit-switch-error" role="alert">{error}</p> : null}
      <div className="limit-switch-actions"><Button variant="secondary" disabled={busy} onClick={onClose}>Cancel</Button><Button variant="default" disabled={busy} onClick={onContinue}>{busy ? "Starting switch…" : "Switch anyway"}</Button></div>
    </div> : null}
  </AppModal>;
}
