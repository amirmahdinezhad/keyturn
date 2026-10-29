import { activeProviderAccountId } from "../accountActivity";
import { useEffect, useMemo, useState } from "react";
import { codexProfileQuota, prettyError, type QuotaSet } from "../api";
import { accountLimitText, nextUsableAccount } from "../accountLimits";
import { codexAccountPreferences } from "../accountPrefs";
import { AccountSwitchIcon, AddIcon, SettingIcon, UsageIcon } from "../icons";
import { ProviderLogo } from "../logos";
import { isSubscriptionExpired } from "../subscriptions";
import type { AppSettings, CodexProfile, CodexProfilesState, Workspace } from "../types";
import { Button } from "./ui/button";
import { PlanBadge } from "./PlanBadge";
import { CodexResetNotice } from "./CodexResetNotice";
import { accentStyle } from "../displayPrefs";
import { EmptyState } from "./EmptyState";
import { LoadingSkeleton } from "./LoadingSkeleton";

/** Read saved logins without activation; discard responses from a previous workspace/identity. */
export function useWorkspaceQuotas(profiles: CodexProfile[], workspaceId: string | undefined, refreshKey: number) {
  const [quotas, setQuotas] = useState<Record<string, QuotaSet | null>>({});
  const key = profiles.map((p) => `${p.id}:${p.connected}:${p.active}:${p.needsActivation}`).join(",");
  useEffect(() => {
    let cancelled = false;
    setQuotas({});
    void Promise.all(profiles.filter((p) => p.connected).map(async (profile) => {
      let quota: QuotaSet;
      try { quota = await codexProfileQuota(profile.id); }
      catch (error) { quota = { provider: "codex", error: prettyError(error) }; }
      if (!cancelled) setQuotas((old) => ({ ...old, [profile.id]: quota }));
    }));
    return () => { cancelled = true; };
  }, [key, workspaceId, refreshKey]); // Metadata refreshes alone do not refetch an identical identity.
  return quotas;
}

interface Props {
  workspace: Workspace;
  profiles: CodexProfilesState | null;
  app: AppSettings;
  quotas: Record<string, QuotaSet | null>;
  busy: boolean;
  onAdd: () => void;
  onManage: (id: string) => void;
  onActivate: (id: string) => void;
  onQuickSwitch: (activeId: string) => void;
  onUsage: (profile: CodexProfile) => void;
  onSettings: () => void;
}

/** Each workspace card represents a saved account, including inactive logins. */
export function WorkspaceAccounts({ workspace, profiles, app, quotas, busy, onAdd, onManage, onActivate, onQuickSwitch, onUsage, onSettings }: Props) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => { const timer = window.setInterval(() => setNow(Date.now()), 1000); return () => window.clearInterval(timer); }, []);
  const accounts = useMemo(() => workspace.accountIds.map((id) => profiles?.profiles.find((p) => p.id === id)).filter((p): p is CodexProfile => !!p), [workspace.accountIds, profiles]);
  const visible = accounts.filter((p) => !isSubscriptionExpired(p.subscription, now));
  const expired = accounts.filter((p) => isSubscriptionExpired(p.subscription, now));
  const activeId = activeProviderAccountId("codex", workspace, visible);
  const knownWindows = Object.fromEntries(accounts.map((p) => [p.id, codexAccountPreferences(app, p.id).knownWindows]));
  return <section className="workspace-accounts" aria-label="Workspace accounts">
    <div className="workspace-section-head"><div className="workspace-section-title"><Button size="icon-sm" variant="ghost" aria-label="Workspace settings" title="Workspace settings" onClick={onSettings}><SettingIcon size={16} /></Button></div><Button size="sm" variant="ghost" onClick={onAdd}><AddIcon size={16} />Add account</Button></div>
    {profiles ? <h3 className="provider-group-title"><ProviderLogo provider="codex" />Codex <span className="provider-count" aria-label={`${visible.length} Codex accounts`}>{visible.length}</span></h3> : null}
    {profiles && (workspace.usage.showCodexResets ?? true) ? <CodexResetNotice refreshKey={quotas} settings={{ ...app.codexReset, showProbableResets: workspace.usage.showProbableResets }} /> : null}
    {!profiles ? <LoadingSkeleton /> : !visible.length ? <EmptyState title="No accounts yet" message="Add an account to this workspace." /> : <div className="workspace-account-list">{visible.map((profile) => {
      const isActive = profile.id === activeId;
      const quota = quotas[profile.id];
      const limit = profile.active ? accountLimitText(quota, now) : null;
      const target = profile.active ? nextUsableAccount(accounts, profile.id, quotas, now, knownWindows) : null;
      return <article key={profile.id} style={accentStyle(codexAccountPreferences(app, profile.id).color)} className={`workspace-account account-card${isActive ? " is-active" : ""}`} aria-label={`${profile.name} account`}>
        <div className="workspace-account-row"><div className="workspace-account-info"><div className="workspace-account-title"><span className="row-title" title={profile.name}>{profile.name}</span><PlanBadge subscription={profile.subscription ?? { id: "chatgpt", service: "ChatGPT", plan: profile.plan }} />{isActive ? <span className="workspace-active-badge">Active</span> : null}</div>
          {profile.displayName && profile.displayName !== profile.name ? <p className="workspace-account-email">{profile.displayName}</p> : null}
          {profile.email ? <p className="workspace-account-email" title={profile.email}>{profile.email}</p> : null}

        </div><div className="workspace-account-actions">
          {!isActive ? <Button size="icon-sm" className="account-switch-action" variant="ghost" aria-label={`Activate ${profile.name}`} title={`Switch to ${profile.name}`} disabled={busy || !profile.connected || !!profiles.switchError} onClick={() => onActivate(profile.id)}><AccountSwitchIcon size={17} /></Button> : null}
          <Button size="icon-sm" variant="ghost" aria-label={`Usage limits for ${profile.name}`} title="Usage limits" disabled={!profile.connected} onClick={() => onUsage(profile)}><UsageIcon size={17} /></Button>
          <Button size="icon-sm" variant="ghost" aria-label={`Manage ${profile.name}`} title="Account settings" onClick={() => onManage(profile.id)}><SettingIcon size={17} /></Button>
        </div>
        </div>
        {!profile.connected ? <p className="workspace-error">{profile.connectionError ?? "Reconnect this account"}</p> : null}
        {limit ? <div className="account-limit-notice" aria-label="Account usage limit"><span><span className="account-limit-label">Limit reached</span>{limit}</span>
          <Button size="icon-sm" aria-label="Switch to next available account" title={target ? `Switch to ${target.name}` : "No other account has available capacity"} disabled={busy || !target || !!profiles.switchError} onClick={() => onQuickSwitch(profile.id)}><AccountSwitchIcon size={20} /></Button>
        </div> : null}
      </article>;
    })}</div>}
    {expired.length ? <details className="workspace-expired"><summary>Expired accounts ({expired.length})</summary>{expired.map((p) => <div className="workspace-expired-row" key={p.id}><span>{p.name}</span><Button size="sm" onClick={() => onManage(p.id)}>Manage</Button></div>)}</details> : null}
  </section>;
}
