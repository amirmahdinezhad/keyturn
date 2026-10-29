import { useEffect, useState } from "react";
import { antigravityAccountQuota, claudeProfileQuota, codexProfileQuota, prettyError, type SetupAccount, type QuotaSet } from "../api";
import { accountWindows, accountWindowLabel, canonicalAccountWindow, codexAccountPreferences } from "../accountPrefs";
import { DEFAULT_CODEX_ACCOUNT_PREFERENCES, type AppSettings, type ClaudeProfile, type CodexProfile, type Workspace, type WorkspaceUsage } from "../types";
import { Select } from "./ui/select";
import { Switch } from "./ui/switch";

/** Follow the active Codex login within this workspace and discover its limits. */
export function WorkspaceMenuBar({ workspace, app, accounts, claudeAccounts, googleAccounts, busy, onSave }: {
  workspace: Workspace; app: AppSettings; accounts: CodexProfile[]; claudeAccounts: ClaudeProfile[]; googleAccounts: SetupAccount[];
  busy: boolean; onSave: (patch: Partial<WorkspaceUsage>) => Promise<unknown>;
}) {
  const groups = [
    { id: "codex" as const, label: "Codex", items: accounts.filter((p) => workspace.accountIds.includes(p.id)) },
    { id: "claude" as const, label: "Claude", items: claudeAccounts.filter((p) => workspace.claudeAccountIds == null || workspace.claudeAccountIds.includes(p.id)) },
    { id: "antigravity" as const, label: "Antigravity", items: googleAccounts.filter((p) => workspace.antigravityAccountIds?.includes(p.id)) },
  ].filter((group) => group.items.length > 0);
  const group = groups.find((g) => g.id === workspace.usage.menuBarProvider) ?? groups[0];
  const provider = group?.id;
  const account = provider === "codex" ? group?.items.find((p) => "active" in p && p.active) : group?.items.find((p) => p.id === workspace.usage.menuBarAccountId)
    ?? group?.items.find((p) => p.id === workspace.usage.activeAccounts?.[group.id]) ?? group?.items[0];
  const [result, setResult] = useState<{ key: string; quota: QuotaSet } | null>(null);
  const key = `${workspace.id}:${provider}:${account?.id}`;
  const quota = result?.key === key ? result.quota : null;
  useEffect(() => {
    let current = true;
    if (!provider || !account) return;
    const read = provider === "codex" ? codexProfileQuota : provider === "claude" ? claudeProfileQuota : antigravityAccountQuota;
    void read(account.id).catch((e) => ({ provider, error: prettyError(e) } as QuotaSet)).then((quota) => { if (current) setResult({ key, quota }); });
    return () => { current = false; };
  }, [key]);
  if (!group) return <section className="settings-section"><h3 className="settings-subtitle">Menu bar</h3><p className="setting-note">Add an account to this workspace first.</p></section>;
  const prefs = provider === "codex" && account ? codexAccountPreferences(app, account.id) : DEFAULT_CODEX_ACCOUNT_PREFERENCES;
  const selected = account ? workspace.usage.accountWindows?.[account.id] ?? prefs.menuBarWindows : [];
  const windows = provider === "codex" ? accountWindows(quota, { ...prefs, menuBarWindows: selected }) : [...new Set([...selected, ...(quota?.error ? [] : quota?.windows ?? []).map((w) => canonicalAccountWindow(w.window) ?? w.window)])];
  return <section className="settings-section workspace-menu-bar" aria-label="Menu bar limits"><h3 className="settings-subtitle">Menu bar</h3>
    <div className="settings-group">
      <div className="setting-row"><span className="setting-name">Provider</span><Select aria-label="Menu bar provider" value={group.id} disabled={busy} options={groups.map((g) => ({ value: g.id, label: g.label }))} onValueChange={(value) => { const next = groups.find((g) => g.id === value)!; void onSave({ menuBarProvider: next.id, menuBarAccountId: next.items.find((p) => p.id === workspace.usage.activeAccounts?.[next.id])?.id ?? next.items[0].id, displayMode: "active" }); }} /></div>
      {provider === "codex" ? <div className="setting-row"><span className="setting-name">Active account</span><span aria-label="Menu bar active account">{account?.name ?? "No active Codex account."}</span></div> : account ? <div className="setting-row"><span className="setting-name">Account</span><Select aria-label="Menu bar account" value={account.id} disabled={busy} options={group.items.map((p) => ({ value: p.id, label: p.name }))} onValueChange={(menuBarAccountId) => void onSave({ menuBarProvider: group.id, menuBarAccountId })} /></div> : null}
      {account ? <div className="setting-row"><span className="setting-name">Show account initials</span><Switch aria-label="Show account initials" checked={workspace.usage.showAccountName ?? prefs.showProfileLabel} disabled={busy} onCheckedChange={(showAccountName) => void onSave({ showAccountName })} /></div> : null}
    </div>
    <div className="settings-group"><div className="setting-row"><span className="setting-name">Usage display</span><Select aria-label="Menu bar display style" value={workspace.usage.menuBarStyle ?? "percentages"} disabled={busy} options={[{ value: "percentages", label: "Percentages" }, { value: "progress", label: "Progress bars" }]} onValueChange={(value) => void onSave({ menuBarStyle: value as "percentages" | "progress" })} /></div></div>
    {account ? <div className="settings-group" key={key}>
      {windows.map((kind) => <div className="setting-row" key={kind}><span className="setting-name">{accountWindowLabel(kind)} limit</span><Switch aria-label={`Show ${account.name} ${accountWindowLabel(kind)} limit`} checked={selected.includes(kind)} disabled={busy} onCheckedChange={(enabled) => void onSave({ menuBarAccountId: account.id, accountWindows: { ...workspace.usage.accountWindows, [account.id]: enabled ? [...selected, kind] : selected.filter((w) => w !== kind) } })} /></div>)}
      {!windows.length ? <p className="setting-note">{quota ? quota.error ?? "No limits reported for this account." : "Reading account limits…"}</p> : null}
    </div> : <p className="setting-note">Only the active account in this workspace appears in the menu bar.</p>}
    {workspace.usage.menuBarStyle === "progress" ? <p className="setting-note">A single limit uses a larger bar. A time marker shows progress toward reset when timing is available.</p> : null}
  </section>;
}
