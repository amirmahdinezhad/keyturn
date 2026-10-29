import { activeProviderAccountId } from "../accountActivity";
import { useEffect, useState } from "react";
import { antigravityAccounts, antigravityAccountQuota, prettyError, renameAntigravityAccount, type SetupAccount } from "../api";
import type { AppState, Workspace } from "../types";
import { ProviderLogo } from "../logos";
import { Button } from "./ui/button";
import { PlanBadge } from "./PlanBadge";
import { AppModal } from "./AppModal";
import { SettingIcon, UsageIcon } from "../icons";
import { AccountSettings } from "./AccountSettings";
import { QuotaContent } from "./CliProviders";

/** Account-scoped Google usage never falls back to another ambient Google login. */
export function AntigravityAccounts({ workspace, refreshKey, onChanged }: { workspace: Workspace; refreshKey: number; onChanged: (state: AppState) => void }) {
  const [accounts, setAccounts] = useState<SetupAccount[]>([]);
  const [selected, setSelected] = useState<SetupAccount | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [settingsId, setSettingsId] = useState<string | null>(null);
  const settingsAccount = accounts.find((a) => a.id === settingsId && workspace.antigravityAccountIds?.includes(a.id));
  useEffect(() => { let active = true; void antigravityAccounts().then((accounts) => { if (active) setAccounts(accounts); }).catch((e) => { if (active) setError(prettyError(e)); }); return () => { active = false; }; }, [workspace.id, refreshKey]);
  const activeId = activeProviderAccountId("antigravity", workspace, accounts);
  return <section className="provider-account-groups" aria-label="Antigravity accounts">
    <p className="provider-group-title"><ProviderLogo provider="antigravity" />Antigravity <span>Experimental</span></p>
    {error ? <p className="workspace-error" role="alert">{error}</p> : null}
    {accounts.filter((a) => workspace.antigravityAccountIds?.includes(a.id)).map((a) => <div key={a.id} className="enrollment-saved-row" ><span className="row-main"><strong>{a.name}</strong>{a.id === activeId ? <span className="workspace-active-badge">Active</span> : null}<span className="row-sub">{a.email}</span></span><PlanBadge subscription={a.subscription} /><div className="workspace-account-actions"><Button size="icon-sm" variant="ghost" aria-label={`Usage limits for ${a.name}`} title="Usage limits" onClick={() => setSelected(a)}><UsageIcon size={17} /></Button><Button size="icon-sm" variant="ghost" aria-label={`Manage ${a.name}`} title="Account settings" onClick={() => setSettingsId(a.id)}><SettingIcon size={17} /></Button></div></div>)}
    <AppModal open={!!selected} onClose={() => setSelected(null)} title={selected?.name ?? "Usage"}>{selected ? <QuotaContent key={selected.id} ids={["antigravity"]} loadQuota={() => antigravityAccountQuota(selected.id)} refreshKey={refreshKey} emptyHint="No measured quota is available for this account." showRemaining={true} notify={() => {}} /> : null}</AppModal>
    {settingsAccount ? <AccountSettings key={`${workspace.id}:${settingsAccount.id}`} workspace={workspace} provider="antigravity" profile={settingsAccount} onClose={() => setSettingsId(null)} onChanged={onChanged} onRename={async (name) => {
      await renameAntigravityAccount(settingsAccount.id, name);
      setAccounts((old) => old.map((a) => a.id === settingsAccount.id ? { ...a, name } : a));
    }} /> : null}
  </section>;
}
