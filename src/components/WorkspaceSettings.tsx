import { useEffect, useLayoutEffect, useId, useState, type ReactNode } from "react";
import { prettyError, setCodexAccountPreferences, updateWorkspace, antigravityAccounts, type SetupAccount } from "../api";
import { codexAccountPreferences } from "../accountPrefs";
import type { AppState, CodexProfilesState, Workspace, WorkspaceUsage, ClaudeProfile } from "../types";
import { AppModal } from "./AppModal";
import { WorkspaceAccent } from "./WorkspaceSetup";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "./ui/tabs";
import type { ToastKind } from "./Toast";
import { WorkspaceMenuBar } from "./WorkspaceMenuBar";

const EMPTY_CLAUDE_ACCOUNTS: ClaudeProfile[] = [];

function Row({ name, description, descriptionId, children }: { name: string; description?: string; descriptionId?: string; children: ReactNode }) {
  return <div className="setting-row"><div className="setting-text"><span className="setting-name">{name}</span>{description ? <span className="setting-desc" id={descriptionId}>{description}</span> : null}</div>{children}</div>;
}

interface Props {
  open: boolean; workspace: Workspace; state: AppState; profiles: CodexProfilesState | null;
  onClose: () => void; onChanged: (state: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
  initialSection?: string;
  claudeAccounts?: ClaudeProfile[];
}

/** Workspace identity and usage policy; account management lives on account cards. */
export function WorkspaceSettings({ open, workspace, state, profiles, initialSection = "general", claudeAccounts = EMPTY_CLAUDE_ACCOUNTS, onClose, onChanged, notify }: Props) {
  const [googleAccounts, setGoogleAccounts] = useState<SetupAccount[]>([]);
  useEffect(() => { if (!open) return; let current = true; void antigravityAccounts().then((a) => { if (current) setGoogleAccounts(a); }).catch(() => {}); return () => { current = false; }; }, [open, workspace.id]);
  const descriptionPrefix = useId();
  const [tab, setTab] = useState("general");
  const [name, setName] = useState(workspace.name);
  const [accent, setAccent] = useState(workspace.accent);
  const [busy, setBusy] = useState(false);
  const [customColorThreshold, setCustomColorThreshold] = useState("");
  const [error, setError] = useState<string | null>(null);
  const accounts = workspace.accountIds.map((id) => profiles?.profiles.find((p) => p.id === id)).filter((p) => !!p);
  useLayoutEffect(() => {
    const body = document.getElementById("workspace-settings-body");
    if (body) body.scrollTop = 0;
  }, [tab]);
  useEffect(() => {
    if (!open) return;
    setTab(["usage", "menu-bar"].includes(initialSection) ? initialSection : "general");
    setName(workspace.name); setAccent(workspace.accent); setError(null); setCustomColorThreshold("");
  }, [open, workspace.id, initialSection]);
  const run = async (action: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true); setError(null);
    try { await action(); } catch (e) { setError(prettyError(e)); }
    finally { setBusy(false); }
  };
  const saveUsage = (patch: Partial<WorkspaceUsage>) => run(async () => onChanged(await updateWorkspace({ ...workspace, usage: { ...workspace.usage, ...patch } })));
  const toggle = (label: string, checked: boolean, patch: (value: boolean) => Partial<WorkspaceUsage>, description?: string) => {
    const descriptionId = `${descriptionPrefix}-${label.toLowerCase().replace(/ /g, "-")}`;
    return <Row name={label} description={description} descriptionId={descriptionId}><Switch aria-label={label} aria-describedby={description ? descriptionId : undefined} checked={checked} disabled={busy} onCheckedChange={(value) => void saveUsage(patch(value))} /></Row>;
  };
  const colorThresholds = workspace.usage.warningThresholds ?? [75, 90, 95];
  const colorLevels = [...new Set([75, 90, 95, ...colorThresholds])].sort((a, b) => a - b);
  return <Tabs value={tab} onValueChange={(next) => { setTab(next); setError(null); }}>
    <AppModal open={open} onClose={onClose} busy={busy} hasUnsavedChanges={name.trim() !== workspace.name || accent !== workspace.accent} title="Workspace settings" cardClass="workspace-settings-card" bodyId="workspace-settings-body" navigation={
      <TabsList className="workspace-tabs" aria-label="Workspace settings sections">{[["general", "General"], ["usage", "Usage"], ["menu-bar", "Menu bar"]].map(([value, label]) => <TabsTrigger key={value} value={value} disabled={busy}>{label}</TabsTrigger>)}</TabsList>
    }>
      {error ? <p className="workspace-error" role="alert">{error}</p> : null}
      <TabsContent value="general">
        <div className="page-intro"><h3>Workspace identity</h3><p>Make this space easy to recognize.</p></div>
        <form className="workspace-form" onSubmit={(e) => { e.preventDefault(); void run(async () => { onChanged(await updateWorkspace({ ...workspace, name: name.trim(), accent })); notify("Workspace saved", "ok"); }); }}>
          <label>Workspace name<input aria-label="Workspace name" required maxLength={64} value={name} disabled={busy} onChange={(e) => setName(e.target.value)} /></label>
          <div className="form-field"><span className="form-label">Workspace color</span><WorkspaceAccent value={accent} onChange={setAccent} disabled={busy} /><p className="setting-note">Used for the panel and menu bar icon.</p></div>
          <div className="workspace-form-actions"><Button variant="default" type="submit" disabled={busy || !name.trim() || !/^#[\da-f]{6}$/i.test(accent) || name.trim() === workspace.name && accent === workspace.accent}>Save changes</Button></div>
        </form>
      </TabsContent>
      <TabsContent value="usage">
        <section className="settings-section"><h3 className="settings-subtitle">Display</h3><div className="settings-group">
          {toggle("Show capacity remaining", workspace.usage.showRemaining, (showRemaining) => ({ showRemaining }))}
        </div></section>
        <section className="settings-section" aria-label="Codex resets"><h3 className="settings-subtitle">Codex resets</h3><div className="settings-group">
          {toggle("Show reset updates", workspace.usage.showCodexResets ?? true, (showCodexResets) => ({ showCodexResets }), "Show announced and recently confirmed resets once in the Codex section.")}
          {(workspace.usage.showCodexResets ?? true) ? toggle("Include reset predictions", workspace.usage.showProbableResets, (showProbableResets) => ({ showProbableResets }), "Also show estimated reset chances. Predictions are not confirmed.") : null}
        </div></section>
        <section className="settings-section"><h3 className="settings-subtitle">Automatic switching</h3><div className="settings-group">
          {toggle("Switch when limited", workspace.usage.autoSwitchEnabled, (autoSwitchEnabled) => ({ autoSwitchEnabled }))}
          {workspace.usage.autoSwitchEnabled ? <Row name="Switch at remaining capacity"><span className="workspace-percent-control"><input aria-label="Switch at remaining percent" type="number" min="0" max="100" defaultValue={workspace.usage.autoSwitchRemaining ?? 2} key={workspace.usage.autoSwitchRemaining ?? 2} disabled={busy} onBlur={(e) => { const value = Number(e.target.value); if (e.target.value && Number.isInteger(value) && value >= 0 && value <= 100 && value !== (workspace.usage.autoSwitchRemaining ?? 2)) void saveUsage({ autoSwitchRemaining: value }); }} /><span aria-hidden="true">%</span></span></Row> : null}
          {workspace.usage.autoSwitchEnabled ? accounts.map((p) => { const prefs = codexAccountPreferences(state.app, p.id); return <Row key={p.id} name={p.name}><Switch aria-label={`Include ${p.name} in switch cycle`} checked={prefs.autoSwitch} disabled={busy || !p.connected && !prefs.autoSwitch} onCheckedChange={(autoSwitch) => void run(async () => onChanged(await setCodexAccountPreferences(p.id, { ...prefs, autoSwitch })))} /></Row>; }) : null}
        </div>{workspace.usage.autoSwitchEnabled ? <p className="setting-note">Choose at least two Codex accounts. Switches start immediately. Running threads require confirmation before Codex closes.</p> : null}</section>
      </TabsContent>
      <TabsContent value="menu-bar">
        <section className="settings-section"><h3 className="settings-subtitle">Visibility</h3><div className="settings-group">
          {toggle("Menu bar usage", workspace.usage.menuBarEnabled, (menuBarEnabled) => ({ menuBarEnabled }))}
        </div></section>
        {workspace.usage.menuBarEnabled ? <WorkspaceMenuBar workspace={workspace} app={state.app} accounts={accounts} claudeAccounts={claudeAccounts} googleAccounts={googleAccounts} busy={busy} onSave={saveUsage} /> : null}
        <section className="settings-section" aria-label="Limit colors"><h3 className="settings-subtitle">Limit colors</h3><div className="settings-group">
          {toggle("Change colors near limits", workspace.usage.warningColors ?? false, (warningColors) => ({ warningColors }))}
        </div>
          {workspace.usage.warningColors ? <div className="workspace-limit-colors" role="group" aria-label="Color thresholds">
            <div className="settings-group">
              {colorLevels.map((level) => {
                const tone = level >= 95 ? "critical" : level >= 90 ? "high" : level >= 75 ? "warning" : "early";
                const name = level >= 95 ? "Critical" : level >= 90 ? "High usage" : level >= 75 ? "Warning" : "Early warning";
                const id = `${descriptionPrefix}-color-${level}`;
                return <div className="setting-row" key={level}>
                  <span className={`workspace-color-dot is-${tone}`} aria-hidden="true" />
                  <div className="setting-text"><span className="setting-name">{level}% used</span><span className="setting-desc" id={id}>{name}</span></div>
                  <Switch aria-label={`Change color at ${level}% used`} aria-describedby={id} checked={colorThresholds.includes(level)} disabled={busy} onCheckedChange={(enabled) => void saveUsage({ warningThresholds: enabled ? [...colorThresholds, level].sort((a,b) => a-b) : colorThresholds.filter((n) => n !== level) })} />
                </div>;
              })}
            </div>
            <form className="workspace-color-threshold-form" onSubmit={(event) => {
              event.preventDefault(); const level = Number(customColorThreshold);
              if (!customColorThreshold || !Number.isInteger(level) || level < 1 || level > 100 || colorThresholds.includes(level)) return;
              void saveUsage({ warningThresholds: [...colorThresholds, level].sort((a,b) => a-b) }); setCustomColorThreshold("");
            }}>
              <label htmlFor={`${descriptionPrefix}-custom-color`}>Custom threshold</label>
              <span className="workspace-percent-control"><input id={`${descriptionPrefix}-custom-color`} aria-label="Custom color threshold" type="number" min="1" max="100" value={customColorThreshold} disabled={busy} onChange={(event) => setCustomColorThreshold(event.target.value)} /><span aria-hidden="true">%</span></span>
              <Button size="sm" type="submit" disabled={busy || !customColorThreshold || !Number.isInteger(Number(customColorThreshold)) || Number(customColorThreshold) < 1 || Number(customColorThreshold) > 100 || colorThresholds.includes(Number(customColorThreshold))}>Add</Button>
            </form>
          </div> : null}
          <p className="setting-note">The highest enabled level colors the whole menu bar, including its icon.</p>
        </section>
      </TabsContent>
    </AppModal>
  </Tabs>;
}
