import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { codexProfileQuota, prettyError, setCodexAccountPreferences, type QuotaSet } from "../api";
import { accountWindows, accountWindowLabel, canonicalAccountWindow, codexAccountPreferences } from "../accountPrefs";
import { ACCENT_PRESETS } from "../displayPrefs";
import type { AppSettings, AppState, CodexAccountPreferences as Preferences, CodexProfile } from "../types";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";
import type { ToastKind } from "./Toast";

interface Props {
  profile: CodexProfile;
  profiles: CodexProfile[];
  app: AppSettings;
  onChanged: (state: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
  workspaceScope?: boolean;
}

function Row({ name, desc, children }: { name: string; desc?: string; children: ReactNode }) {
  return <div className="setting-row"><div className="setting-text"><span className="setting-name">{name}</span>{desc ? <span className="setting-desc">{desc}</span> : null}</div>{children}</div>;
}

function PreferenceSection({ title, label, collapsed, children }: { title: string; label: string; collapsed: boolean; children: ReactNode }) {
  return collapsed ? <details className="account-preference-section" aria-label={label}><summary>{title}</summary><div>{children}</div></details>
    : <section className="settings-section" aria-label={label}><h3 className="settings-subtitle">{title}</h3>{children}</section>;
}

/** Account-local display; quota discovery never activates its credentials. */
export function CodexAccountPreferences({ profile, profiles, app, onChanged, notify, workspaceScope = false }: Props) {
  const prefs = codexAccountPreferences(app, profile.id);
  const [quota, setQuota] = useState<QuotaSet | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [busy, setBusy] = useState(false);
  const [customColor, setCustomColor] = useState(prefs.color);
  useEffect(() => { setCustomColor(prefs.color); }, [profile.id, prefs.color]);
  useEffect(() => {
    let cancelled = false;
    setQuota(null); setLoading(true); setError(null);
    if (!profile.connected) { setLoading(false); return; }
    void codexProfileQuota(profile.id).then((fresh) => {
      if (cancelled) return;
      setQuota(fresh); setError(fresh.error ?? null);
    }).catch((e) => { if (!cancelled) setError(prettyError(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [profile.id, profile.connected, retry]);
  const windows = accountWindows(quota, prefs);
  const save = async (patch: Partial<Preferences>) => {
    if (busy) return;
    setBusy(true);
    try {
      onChanged(await setCodexAccountPreferences(profile.id, { ...prefs, knownWindows: windows, ...patch }));
    } catch (e) { notify(prettyError(e), "err"); }
    finally { setBusy(false); }
  };
  const toggle = (name: string, value: boolean, patch: (value: boolean) => Partial<Preferences>, desc?: string) =>
    <Row name={name} desc={desc}><Switch aria-label={name} checked={value} disabled={busy} onCheckedChange={(v) => void save(patch(v))} /></Row>;
  return <div className="account-preferences" aria-label={`${profile.name} preferences`}>
    <section className="settings-section" aria-label="Account appearance">
      <h3 className="settings-subtitle">Accent</h3>
      <div className="settings-group">
        <Row name="Account color"><div className="accent-grid" role="group" aria-label="Account color">{ACCENT_PRESETS.map((preset) => <Button key={preset.value} className={`accent-dot${prefs.color === preset.value ? " on" : ""}`} style={{ "--dot": preset.value } as CSSProperties} aria-label={`Account color ${preset.name}`} aria-pressed={prefs.color === preset.value} disabled={busy} onClick={() => void save({ color: preset.value })}><span aria-hidden="true">{prefs.color === preset.value ? "✓" : ""}</span></Button>)}</div></Row>
        <form className="threshold-form" onSubmit={(e) => { e.preventDefault(); if (/^#[\da-f]{6}$/i.test(customColor)) void save({ color: customColor.toLowerCase() }); }}>
          <label htmlFor={`account-color-${profile.id}`}>Custom color</label><input id={`account-color-${profile.id}`} aria-label="Custom account color" value={customColor} maxLength={7} placeholder="#007aff" spellCheck={false} disabled={busy} onChange={(e) => setCustomColor(e.target.value)} /><Button type="submit" size="sm" disabled={busy || !/^#[\da-f]{6}$/i.test(customColor) || customColor.toLowerCase() === prefs.color}>Apply color</Button>
        </form>
      </div>
    </section>
    <PreferenceSection title="Menu bar" label="Account menu bar" collapsed={workspaceScope}>
      {!app.menuBarEnabled ? <p className="setting-note">Enable menu bar usage in {workspaceScope ? "Workspace settings → Menu bar" : "Settings → Display"}.</p> : null}
      <div className="settings-group">
        {toggle("Show account in menu bar", prefs.menuBarEnabled, (menuBarEnabled) => ({ menuBarEnabled }))}
        {loading ? <p className="setting-note" role="status">Reading this account’s limits…</p> : null}
        {windows.map((kind) => {
          const available = !quota?.error && quota?.windows?.some((w) => canonicalAccountWindow(w.window) === kind);
          return <div key={kind}>{toggle(`Show ${accountWindowLabel(kind)} limit`, prefs.menuBarWindows.includes(kind), (enabled) => ({ menuBarWindows: enabled ? [...prefs.menuBarWindows, kind] : prefs.menuBarWindows.filter((w) => w !== kind) }), !loading && !available ? "Temporarily unavailable · your selection is preserved" : undefined)}</div>;
        })}
      </div>
      {error || (!loading && windows.length === 0) ? <div className="account-usage-status"><p className="setting-note">{error ?? "No limits have been reported for this account yet."}</p><Button size="sm" disabled={loading} onClick={() => setRetry((v) => v + 1)}>Check limits again</Button></div> : null}
      <div className="settings-group">
        {toggle("Show limit labels", prefs.showLabels, (showLabels) => ({ showLabels }))}
        {toggle("Show account name", prefs.showProfileLabel, (showProfileLabel) => ({ showProfileLabel }))}
        {toggle("Mark active account", prefs.showActiveIndicator, (showActiveIndicator) => ({ showActiveIndicator }))}
      </div>
    </PreferenceSection>
    <PreferenceSection title="Limit colors" label="Account limit colors" collapsed={workspaceScope}>
      <div className="settings-group">{toggle("Warning colors", prefs.warningColors, (warningColors) => ({ warningColors }))}</div>
    </PreferenceSection>
    <PreferenceSection title="Usage bars" label="Account usage bars" collapsed={workspaceScope}>
      <div className="settings-group">
        {toggle("Show time marker", prefs.showTimeMarker, (showTimeMarker) => ({ showTimeMarker }))}
        {toggle("Pace-aware colors", prefs.paceAwareColors, (paceAwareColors) => ({ paceAwareColors }))}
      </div>
    </PreferenceSection>
    {!workspaceScope && (profiles.length >= 2 || prefs.autoSwitch) ? <section className="settings-section" aria-label="Account switch cycle">
      <h3 className="settings-subtitle">Automatic switching</h3>
      <div className="settings-group">{toggle("Include in switch cycle", prefs.autoSwitch, (autoSwitch) => ({ autoSwitch }), "Only enrolled Codex accounts can switch automatically. At least two members are required; switches start immediately and ask before closing a running thread.")}</div>
    </section> : null}
  </div>;
}
