import { useState } from "react";
import { importCodexProfile, importCodexProfileJson } from "../api";
import type { CodexProfile, CodexProfilesState } from "../types";
import { AppModal } from "./AppModal";
import { Button } from "./ui/button";
import { WorkspaceAccountForm, type DraftStatus } from "./WorkspaceSetup";
import type { ToastKind } from "./Toast";

/** Reconnect from an account's usage popup without a workspace Accounts tab. */
export function ReconnectAccount({ profile, onClose, onProfilesChanged, notify }: {
  profile: CodexProfile; onClose: () => void; onProfilesChanged: (profiles: CodexProfilesState) => void;
  notify: (message: string, kind?: ToastKind) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<DraftStatus>({ dirty: false, busy: false });
  return <AppModal open title="Reconnect account" onClose={onClose} hasUnsavedChanges={draft.dirty} busy={draft.busy}
    detail={<div className="usage-identity"><strong>{profile.name}</strong>{profile.email ? <span>{profile.email}</span> : null}</div>}>
    {editing ? <WorkspaceAccountForm submitLabel="Save login" initialDraft={{ accountName: profile.name }} onStatusChange={setDraft} onSave={async (account) => {
      const fresh = account.credentials != null ? await importCodexProfileJson(account.accountName, account.credentials, profile.id)
        : await importCodexProfile(account.accountName, account.path!, profile.id);
      onProfilesChanged(fresh); setEditing(false); setDraft({ dirty: false, busy: false }); notify("Account login replaced", "ok");
    }} /> : <section className="settings-section"><p className="setting-note">Import a fresh auth.json for this account to restore usage limits.</p><Button size="sm" onClick={() => setEditing(true)}>Replace login</Button></section>}
  </AppModal>;
}
