import { useEffect, useState } from "react";
import type { AppState, Workspace } from "../types";
import { AccountEnrollment, useAccountEnrollment } from "./AccountEnrollment";
import { Button } from "./ui/button";
import type { DraftStatus } from "./WorkspaceSetup";

/** The same staged login flow is used after onboarding, with a single final add. */
export function AddAccountsPanel({ workspace, onSaved, onStatusChange, onAccountAdded }: { workspace: Workspace; onSaved: (state: AppState) => void; onStatusChange: (status: DraftStatus) => void; onAccountAdded?: () => void }) {
  const draft = useAccountEnrollment();
  const [enrolling, setEnrolling] = useState(true);
  const dirty = draft.selected.length > 0 || draft.editing || !!draft.pending;
  useEffect(() => { onStatusChange({ dirty, busy: draft.busy }); }, [dirty, draft.busy, onStatusChange]);
  return <section className="workspace-setup">
    <div className="workspace-setup-scroll">
      {!enrolling ? <p className="workspace-intro">Connect another paid account to {workspace.name}.</p> : null}
      {!enrolling && draft.error ? <p className="workspace-error" role="alert">{draft.error}</p> : null}
      <AccountEnrollment embedded initiallyOpen onOpenChange={setEnrolling} draft={draft} onAdded={onAccountAdded} />
    </div>
    {!enrolling ? <footer className="workspace-setup-footer"><div className="workspace-form-actions"><Button size="sm" variant="default" disabled={draft.busy || draft.pending || draft.editing || !draft.selected.length} onClick={() => void draft.commit(workspace.name, workspace.accent, workspace.id).then((state) => { if (state) onSaved(state); })}>{draft.busy ? "Saving…" : "Add accounts"}</Button></div></footer> : null}
  </section>;
}
