import { useState } from "react";
import { prettyError, setAppSettings } from "../api";
import type { AppSettings, AppState } from "../types";
import { Select } from "./ui/select";
import { Switch } from "./ui/switch";
import type { ToastKind } from "./Toast";

/** Independent module switches and the account-selection policy, shared across Codex. */
export function CodexDisplaySettings({ app, onChanged, notify }: { app: AppSettings; onChanged: (state: AppState) => void; notify: (message: string, kind?: ToastKind) => void }) {
  const [busy, setBusy] = useState(false);
  const save = async (patch: Partial<AppSettings>) => {
    if (busy) return;
    setBusy(true);
    try { onChanged(await setAppSettings(patch)); }
    catch (e) { notify(prettyError(e), "err"); }
    finally { setBusy(false); }
  };
  return <section className="settings-section profile-preferences" aria-label="Codex modules">
    <h3 className="settings-subtitle">Modules</h3>
    <div className="settings-group">
      <div className="setting-row"><div className="setting-text"><span className="setting-name">Menu bar module</span><span className="setting-desc">Uses each account’s chosen limits and color</span></div><Switch aria-label="Menu bar module" checked={app.menuBarEnabled && app.menuBarProvider === "codex"} disabled={busy} onCheckedChange={(enabled) => void save({ menuBarEnabled: enabled, ...(enabled ? { menuBarProvider: "codex" } : {}) })} /></div>
      <div className="setting-row"><span className="setting-name">Accounts to display</span><Select aria-label="Codex accounts to display" value={app.codexDisplayMode ?? "active"} disabled={busy} onValueChange={(value) => void save({ codexDisplayMode: value as AppSettings["codexDisplayMode"] })} options={[{ value: "active", label: "Active account" }, { value: "selected", label: "Selected accounts" }]} /></div>
    </div>
    <p className="setting-note">Manage an account to choose its limits, color and switch-cycle membership.</p>
  </section>;
}
