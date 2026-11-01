import { activeProviderAccountId } from "./accountActivity";
import { codexAccountPreferences, codexUsageAppearance } from "./accountPrefs";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "./components/ui/button";
import { Tooltip } from "./components/ui/tooltip";
import { DEFAULT_APP_SETTINGS, type AppState, type ClaudeProfile, type CodexProfilesState } from "./types";
import {
  cliStatus,
  editClaudeProfile,
  renameCodexProfile,
  codexProfiles,
  claudeProfiles,
  codexProfileQuota,
  getState,
  prettyError,
  quitApp,
  setMenuBarAccent,
  selectWorkspace,
  includeWorkspaceProvider,
  type ProviderCliStatus,
} from "./api";
import { loadDisplayPrefs, applyDisplayPrefs, type DisplayPrefs } from "./displayPrefs";
import { AppLogo } from "./logos";
import { LogoutIcon, RefreshIcon, SettingIcon } from "./icons";
import { CliProviders } from "./components/CliProviders";
import { AddAccountsPanel } from "./components/AddAccountsPanel";
import { AntigravityAccounts } from "./components/AntigravityAccounts";
import { UsageModal } from "./components/UsageModal";
import { SettingsModal } from "./components/SettingsModal";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@tauri-apps/api/core";
import { ClaudeProfilesModal } from "./components/ClaudeProfilesModal";
import { WorkspaceSetup, WorkspaceImportAccount, type DraftStatus } from "./components/WorkspaceSetup";
import { AccountSettings } from "./components/AccountSettings";
import { ReconnectAccount } from "./components/ReconnectAccount";
import { WorkspaceSettings } from "./components/WorkspaceSettings";
import { WorkspaceAccounts, useWorkspaceQuotas } from "./components/WorkspaceAccounts";
import { CodexRestartBanner, useCodexRestart } from "./components/CodexRestartBanner";
import { AppModal } from "./components/AppModal";
import { Select } from "./components/ui/select";
import { AddIcon } from "./icons";
import { accountLimitText, nextUsableAccount } from "./accountLimits";
import { CodexUsageModal } from "./components/CodexUsageModal";
import { EmptyState } from "./components/EmptyState";
import { Toast, type ToastData, type ToastKind } from "./components/Toast";
import { LimitedSwitchWarning, type LimitedSwitchTarget } from "./components/LimitedSwitchWarning";
import { LoadingSkeleton } from "./components/LoadingSkeleton";
import { formatAgo } from "./components/CliProviders";

/** Keep the tray panel mounted and coordinate status refresh, setup and popups. */
export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [limitedTarget, setLimitedTarget] = useState<LimitedSwitchTarget | null>(null);
  const checkingSwitch = useRef(false);
  const [limitSwitchError, setLimitSwitchError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [cli, setCli] = useState<ProviderCliStatus[] | null>(null);
  const [cliError, setCliError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [claudeProfilesOpen, setClaudeProfilesOpen] = useState(false);
  const [claudeCreate, setClaudeCreate] = useState(false);
  const [claudeManagedId, setClaudeManagedId] = useState<string | undefined>();
  const [claudeAccounts, setClaudeAccounts] = useState<ClaudeProfile[]>([]);
  const [workspaceSettingsOpen, setWorkspaceSettingsOpen] = useState(false);
  const [claudeSettingsId, setClaudeSettingsId] = useState<string | null>(null);
  const [accountSettingsId, setAccountSettingsId] = useState<string | null>(null);
  const [reconnectId, setReconnectId] = useState<string | null>(null);
  const [workspaceSection, setWorkspaceSection] = useState("general");
  const [createOpen, setCreateOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [setupStep, setSetupStep] = useState(0);
  const [createStep, setCreateStep] = useState(0);
  const [, setSetupDraft] = useState<DraftStatus>({ dirty: false, busy: false });
  const [createDraft, setCreateDraft] = useState<DraftStatus>({ dirty: false, busy: false });
  const [importDraft, setImportDraft] = useState<DraftStatus>({ dirty: false, busy: false });
  const [previewAccent, setPreviewAccent] = useState<string | null>(null);
  const [workspaceBusy, setWorkspaceBusy] = useState(false);
  const [accountUsageId, setAccountUsageId] = useState<string | null>(null);
  const [profilesError, setProfilesError] = useState<string | null>(null);
  const [profiles, setProfiles] = useState<CodexProfilesState | null>(null);
  const [usageFor, setUsageFor] = useState<ProviderCliStatus | null>(null);
  const [toast, setToast] = useState<ToastData | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);
  const [display, setDisplay] = useState<DisplayPrefs>(() => loadDisplayPrefs());
  const [lastUpdated, setLastUpdated] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [usageRefreshKey, setUsageRefreshKey] = useState(0);
  const toastTimer = useRef(0);
  const refreshGeneration = useRef(0);
  const profilesGeneration = useRef(0);
  const loadGeneration = useRef(0);
  const workspace = state?.app.workspaces?.find((w) => w.id === state.app.activeWorkspaceId);
  const latestWorkspace = useRef(workspace);
  latestWorkspace.current = workspace;
  const workspaceProfiles = useMemo(() => (workspace?.accountIds ?? []).flatMap((id) => profiles?.profiles.find((p) => p.id === id) ?? []), [workspace, profiles]);
  const quotas = useWorkspaceQuotas(workspaceProfiles, workspace?.id, usageRefreshKey);
  const claudeSettingsAccount = claudeAccounts.find((p) => p.id === claudeSettingsId && (workspace?.claudeAccountIds == null || workspace.claudeAccountIds.includes(p.id)));
  const reconnectAccount = workspaceProfiles.find((p) => p.id === reconnectId);
  const settingsAccount = workspaceProfiles.find((p) => p.id === accountSettingsId);
  const accountUsage = workspaceProfiles.find((p) => p.id === accountUsageId);
  const effectiveDisplay = useMemo(() => ({ ...display, accent: previewAccent ?? workspace?.accent ?? display.accent }), [display, previewAccent, workspace?.accent]);

  const notify = useCallback((message: string, kind: ToastKind = "") => {
    if (kind === "err") setLastError(message);
    else if (kind === "ok") setLastError(null);
    setToast({ message, kind });
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2200);
  }, []);

  const applyState = useCallback((s: AppState) => {
    setLoadError(null);
    setState(s);
  }, []);

  const applyProfiles = useCallback((fresh: CodexProfilesState) => {
    const ticket = ++profilesGeneration.current;
    setProfiles(fresh);
    setUsageRefreshKey((key) => key + 1);
    setProfilesError(null);
    void getState().then((fresh) => { if (ticket === profilesGeneration.current) applyState(fresh); }).catch((e) => notify(prettyError(e), "err"));
  }, [applyState, notify]);

  const refreshProviders = useCallback(async () => {
    const ticket = ++refreshGeneration.current;
    const fresh = await cliStatus();
    if (ticket !== refreshGeneration.current) return fresh;
    setCli(fresh);
    setCliError(null);
    return fresh;
  }, []);

  const load = useCallback(async () => {
    const loadTicket = ++loadGeneration.current;
    setRefreshing(true);
    setUsageRefreshKey((previous) => previous + 1);
    let hasWorkspace = false;
    try {
      const fresh = await getState();
      hasWorkspace = fresh.app.workspaces.length > 0;
      if (loadTicket === loadGeneration.current) applyState(fresh);
    } catch (e) {
      if (loadTicket === loadGeneration.current) {
        setState(null);
        setLoadError(`Couldn't load settings: ${prettyError(e)}`);
      }
    }
    if (loadTicket !== loadGeneration.current) return;
    if (!hasWorkspace) { setRefreshing(false); return; }
    try {
      await refreshProviders();
    } catch (e) {
      if (loadTicket === loadGeneration.current) {
        setCli(null);
        setCliError(`Couldn't read CLI status: ${prettyError(e)}`);
      }
    }
    if (loadTicket !== loadGeneration.current) return;
    // Saved Claude configurations can exist independently of the default CLI login.
    void claudeProfiles().then((fresh) => { if (loadTicket === loadGeneration.current) setClaudeAccounts(fresh); })
      .catch(() => { if (loadTicket === loadGeneration.current) setClaudeAccounts([]); });
    const ticket = ++profilesGeneration.current;
    try {
      const fresh = await codexProfiles();
      if (ticket === profilesGeneration.current) { setProfiles(fresh); setProfilesError(null); }
      setLastUpdated(new Date().toISOString());
    } catch (error) {
      if (ticket === profilesGeneration.current) { setProfiles(null); setProfilesError(prettyError(error)); }
    } finally {
      if (loadTicket === loadGeneration.current) setRefreshing(false);
    }
  }, [applyState, refreshProviders]);

  const completedSwitch = useCallback((id: string) => {
    profilesGeneration.current++;
    setState((current) => current ? { ...current, app: { ...current.app, workspaces: current.app.workspaces.map((w) => {
      if (!w.accountIds.includes(id)) return w;
      return { ...w, usage: { ...w.usage, activeAccounts: { ...w.usage.activeAccounts, codex: id },
        ...(w.id === current.app.activeWorkspaceId && (w.usage.menuBarProvider ?? "codex") === "codex" ? { menuBarAccountId: id } : {}) } };
    }) } } : current);
    setProfiles((current) => current ? { ...current, activeProfileId: id, profiles: current.profiles.map((p) => ({ ...p, active: p.id === id, needsActivation: p.id === id ? false : p.needsActivation })) } : current);
    void load();
  }, [load]);
  const restart = useCodexRestart(notify, completedSwitch);
  const changeState = useCallback((fresh: AppState) => {
    loadGeneration.current++;
    refreshGeneration.current++;
    profilesGeneration.current++;
    setRefreshing(false);
    applyState(fresh);
    setUsageFor(null); setAccountUsageId(null);
  }, [applyState]);

  const imported = useCallback((fresh: AppState) => {
    changeState(fresh); setCreateOpen(false); setImportOpen(false); setAddOpen(false); setPreviewAccent(null);
    void refreshProviders().catch((e) => setCliError(prettyError(e)));
    void claudeProfiles().then(setClaudeAccounts).catch(() => {});
    void codexProfiles().then(applyProfiles).catch((e) => notify(prettyError(e), "err"));
    setUsageRefreshKey((key) => key + 1);
  }, [changeState, applyProfiles, notify, refreshProviders]);

  const activate = async (id: string) => {
    if (restart.busy || workspaceBusy || checkingSwitch.current || !workspace?.accountIds.includes(id)) return;
    checkingSwitch.current = true;
    setWorkspaceBusy(true);
    const workspaceId = workspace.id;
    const name = profiles?.profiles.find((p) => p.id === id)?.name ?? "This account";
    try {
      const fresh = await codexProfileQuota(id);
      if (latestWorkspace.current?.id !== workspaceId || !latestWorkspace.current.accountIds.includes(id)) return;
      if (fresh.error) throw new Error(`Couldn’t check ${name}’s limits. ${fresh.error}`);
      const reset = accountLimitText(fresh, Date.now());
      if (reset) { setLimitSwitchError(null); setLimitedTarget({ id, workspaceId, name, email: profiles?.profiles.find((p) => p.id === id)?.email ?? null, reset }); return; }
      await restart.schedule(id);
    } catch (e) { notify(prettyError(e), "err"); }
    finally { checkingSwitch.current = false; setWorkspaceBusy(false); }
  };

  const continueLimitedSwitch = async () => {
    if (!limitedTarget || restart.busy || checkingSwitch.current || workspaceBusy) return;
    if (latestWorkspace.current?.id !== limitedTarget.workspaceId || !latestWorkspace.current.accountIds.includes(limitedTarget.id)) {
      setLimitedTarget(null); return;
    }
    checkingSwitch.current = true; setWorkspaceBusy(true); setLimitSwitchError(null);
    try {
      await restart.schedule(limitedTarget.id, true);
      setLimitedTarget(null);
    } catch (e) { setLimitSwitchError(prettyError(e)); }
    finally { checkingSwitch.current = false; setWorkspaceBusy(false); }
  };

  useEffect(() => {
    if (limitedTarget && (workspace?.id !== limitedTarget.workspaceId || !workspace.accountIds.includes(limitedTarget.id))) {
      setLimitedTarget(null); setLimitSwitchError(null);
    }
  }, [workspace, limitedTarget]);

  const quickSwitch = async (activeId: string) => {
    if (restart.busy || workspaceBusy || !workspace || !state) return;
    setWorkspaceBusy(true);
    try {
      const freshProfiles = await codexProfiles();
      if (freshProfiles.activeProfileId !== activeId) throw new Error("The active account changed. Refresh and try again.");
      const accounts = workspace.accountIds.flatMap((id) => freshProfiles.profiles.find((p) => p.id === id) ?? []);
      const freshQuotas = Object.fromEntries(await Promise.all(accounts.filter((p) => p.connected).map(async (p) => [p.id, await codexProfileQuota(p.id).catch(() => null)])));
      if (!accountLimitText(freshQuotas[activeId], Date.now())) throw new Error("This account is no longer limited. Refresh to check its usage.");
      const target = nextUsableAccount(accounts, activeId, freshQuotas, Date.now(), Object.fromEntries(accounts.map((p) => [p.id, codexAccountPreferences(state.app, p.id).knownWindows])));
      if (!target) throw new Error("No other account has available capacity.");
      if (latestWorkspace.current?.id !== workspace.id || !latestWorkspace.current.accountIds.includes(activeId) || !latestWorkspace.current.accountIds.includes(target.id)) throw new Error("Workspace accounts changed. Try again.");
      await restart.schedule(target.id);
    } catch (e) { notify(prettyError(e), "err"); }
    finally { setWorkspaceBusy(false); }
  };

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void listen<string>("codex-profile-switched", (event) => { setUsageFor(null); if (typeof event.payload === "string") completedSwitch(event.payload); else void load(); })
      .then((unlisten) => { if (disposed) unlisten(); else stop = unlisten; })
      .catch(() => { /* Browser previews have no native event bridge. */ });
    return () => { disposed = true; stop?.(); };
  }, [load, completedSwitch]);

  // Theme + accent paint <html> instantly and persist via displayPrefs.
  useEffect(() => {
    applyDisplayPrefs(effectiveDisplay);
  }, [effectiveDisplay]);

  // Restore the saved accent on startup, including while the panel is hidden.
  useEffect(() => {
    void setMenuBarAccent(effectiveDisplay.accent).catch((error) => {
      if (isTauri()) notify(`Couldn't apply menu bar color: ${prettyError(error)}`, "err");
    });
  }, [effectiveDisplay.accent, notify]);

  // When following the OS, re-resolve on OS changes without a reload.
  useEffect(() => {
    if (effectiveDisplay.theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const onChange = () => applyDisplayPrefs(effectiveDisplay);
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [effectiveDisplay]);
  // Show the mounted panel immediately. Refresh on focus is opt-in; while
  // the panel is open, keep data current at the configured interval.
  const refreshIntervalSecs = state?.app?.refreshIntervalSecs ?? 60;
  const refreshOnFocus = display.refreshOnFocus;
  useEffect(() => {
    const onFocus = () => {
      if (refreshOnFocus) void load();
    };
    window.addEventListener("focus", onFocus);
    let last = Date.now();
    const timer = window.setInterval(() => {
      const now = Date.now();
      const drift = now - last - refreshIntervalSecs * 1000;
      last = now;
      const woke = drift > 10_000;
      if (document.hasFocus() || woke) void load();
    }, refreshIntervalSecs * 1000);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.clearInterval(timer);
    };
  }, [load, refreshIntervalSecs, refreshOnFocus]);

  const includeProvider = async (providerId: string) => {
    if (!workspace || workspace.providerIds == null || workspace.providerIds.includes(providerId)) return;
    changeState(await includeWorkspaceProvider(workspace.id, providerId));
  };

  return (
    <>
      <header className="app-header">

        <div className="brand">
          <AppLogo />
          <div className="brand-text workspace-brand">
            <span className="brand-caption">{workspace ? "Workspace" : "Account manager"}</span>
            {workspace ? <Select aria-label="Workspace" value={workspace.id} disabled={workspaceBusy} options={(state?.app.workspaces ?? []).map((w) => ({ value: w.id, label: w.name }))} onValueChange={(id) => {
              setWorkspaceBusy(true);
              void selectWorkspace(id).then(changeState).catch((e) => notify(prettyError(e), "err")).finally(() => setWorkspaceBusy(false));
            }} /> : <span className="profile-picker-name">Keyturn</span>}
            {workspace && lastUpdated ? (
              <Tooltip content={lastUpdated}><span className="brand-updated">
                Updated {formatAgo(lastUpdated)}
              </span></Tooltip>
            ) : null}
          </div>
        </div>
        <div className="header-actions">
          {workspace ? <Button size="icon-sm" aria-label="Create workspace" title="Create workspace" onClick={() => { setCreateStep(0); setCreateOpen(true); }}><AddIcon size={20} /></Button> : null}
          {workspace && ((profiles?.profiles.length ?? 0) > 0 || claudeAccounts.length > 0 || cli?.some((status) => status.loggedIn === true)) ? <><Button
            size="icon-sm"
            className={`gear-btn${refreshing ? " spinning" : ""}`}
            aria-label="Refresh data"
            title="Refresh data"
            disabled={refreshing}
            onClick={() => void load()}
          >
            <RefreshIcon size={20} />
          </Button>
          <Button
            size="icon-sm"
            className="gear-btn"
            aria-label="Settings"
            title="Global settings"
            onClick={() => setSettingsOpen(true)}
          >
            <SettingIcon size={20} />
          </Button></> : null}
          <Button
            size="icon-sm"
            className="quit-header-btn"
            aria-label="Quit Keyturn"
            title="Quit Keyturn"
            onClick={() => {
              void quitApp();
            }}
          >
            <LogoutIcon size={20} />
          </Button>
        </div>
      </header>

      <CodexRestartBanner restart={restart} />

      {lastError && (
        <div className="error-panel" role="alert">
          <div className="error-head">
            <span className="error-title">{lastError.split("\n")[0]}</span>
            <span className="error-actions">
              <Button
                type="button"
                className="link-btn"
                onClick={() => {
                  void navigator.clipboard?.writeText(lastError);
                }}
              >
                Copy
              </Button>
              <Button
                type="button"
                className="link-btn"
                onClick={() => setLastError(null)}
              >
                Dismiss
              </Button>
            </span>
          </div>
          {lastError.includes("\n") && (
            <details className="error-details">
              <summary>Details</summary>
              <pre className="error-text">{lastError}</pre>
            </details>
          )}
        </div>
      )}

      <main className="providers">
        {loadError ? (
          <EmptyState title="Unable to load accounts" message={loadError} action={<Button onClick={() => void load()} disabled={refreshing}>Try again</Button>} />
        ) : !state ? (
          <LoadingSkeleton />
        ) : !workspace ? <WorkspaceSetup onAccountAdded={() => notify("Account added", "ok")} step={setupStep} onStepChange={setSetupStep} onCreated={imported} onAccentPreview={setPreviewAccent} onStatusChange={setSetupDraft} /> : (
          <>
          {profilesError ? <p className="workspace-error" role="alert">Couldn’t load accounts: {profilesError}. Refresh to retry.</p> : <WorkspaceAccounts
            workspace={workspace} profiles={profiles} app={state.app} quotas={quotas} busy={restart.busy || workspaceBusy}
            onAdd={() => setAddOpen(true)} onManage={setAccountSettingsId}
            onSettings={() => { setWorkspaceSection("general"); setWorkspaceSettingsOpen(true); }}
            onActivate={(id) => void activate(id)} onQuickSwitch={(id) => void quickSwitch(id)} onUsage={(p) => setAccountUsageId(p.id)} />}
          {workspace.antigravityAccountIds?.length ? <AntigravityAccounts workspace={workspace} refreshKey={usageRefreshKey} onChanged={changeState} /> : null}
          {cliError ? <p className="workspace-error" role="alert">{cliError}</p> : null}
          <CliProviders
            statuses={(cli ?? []).filter((s) => s.id !== "codex" && !(s.id === "claude" && workspace.claudeAccountIds != null) && (workspace.providerIds == null || workspace.providerIds.includes(s.id)))}
            claudeAccounts={workspace.providerIds == null || workspace.providerIds.includes("claude") ? claudeAccounts.filter((account) => workspace.claudeAccountIds == null || workspace.claudeAccountIds.includes(account.id)).map((account) => ({ ...account, active: account.id === activeProviderAccountId("claude", workspace, claudeAccounts) })) : []}
            refreshKey={usageRefreshKey}
            showRemaining={state.app.usageShowRemaining}
            usageAppearance={state.app.usageAppearance}
            sectionLabel="Other providers"
            providerAccents={state.app.providerAccents}
            onManageClaude={(id) => { if (id) setClaudeSettingsId(id); else { setClaudeCreate(false); setClaudeManagedId(undefined); setClaudeProfilesOpen(true); } }}
            compactRows={display.compactRows}
            notify={notify}
            onAddAccount={() => setAddOpen(true)}
            onShowUsage={(s) => setUsageFor(s)}
          />
          </>
        )}
      </main>
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        state={state}
        onChanged={changeState}
        notify={notify}
        display={effectiveDisplay}
        onDisplayChanged={setDisplay}
      />
      <AppModal open={addOpen && !!workspace} onClose={() => setAddOpen(false)} title="Add account" cardClass="workspace-setup-card" hasUnsavedChanges={importDraft.dirty} busy={importDraft.busy}>
        {addOpen && workspace ? <AddAccountsPanel onAccountAdded={() => notify("Account added", "ok")} workspace={workspace} onSaved={imported} onStatusChange={setImportDraft} /> : null}
      </AppModal>
      <LimitedSwitchWarning target={limitedTarget} busy={workspaceBusy || restart.pending} error={limitSwitchError} onClose={() => { setLimitedTarget(null); setLimitSwitchError(null); }} onContinue={() => void continueLimitedSwitch()} />
      <ClaudeProfilesModal open={claudeProfilesOpen} onClose={() => setClaudeProfilesOpen(false)}
        initialCreate={claudeCreate} initialProfileId={claudeManagedId} onProfilesChanged={(fresh) => { setClaudeAccounts(fresh); if (claudeCreate && fresh.length) void includeProvider("claude").catch((e) => notify(prettyError(e), "err")); }}
        onCodex={() => {setClaudeProfilesOpen(false);setImportOpen(true);}} appState={state} onAppChanged={changeState} notify={notify} />
      {workspace && claudeSettingsAccount ? <AccountSettings key={`${workspace.id}:claude:${claudeSettingsAccount.id}`} workspace={workspace} provider="claude" profile={claudeSettingsAccount} onClose={() => setClaudeSettingsId(null)} onChanged={changeState} onRename={async (name) => setClaudeAccounts(await editClaudeProfile(claudeSettingsAccount.id, name))} /> : null}
      {workspace && settingsAccount ? <AccountSettings key={`${workspace.id}:${settingsAccount.id}`} workspace={workspace} provider="codex" profile={settingsAccount} onClose={() => setAccountSettingsId(null)} onChanged={changeState} onRename={async (name) => applyProfiles(await renameCodexProfile(settingsAccount.id, name))} /> : null}
      {workspace && state ? <WorkspaceSettings claudeAccounts={claudeAccounts} open={workspaceSettingsOpen} workspace={workspace} state={state} profiles={profiles} initialSection={workspaceSection} onClose={() => setWorkspaceSettingsOpen(false)} onChanged={changeState} notify={notify} /> : null}
      {reconnectAccount ? <ReconnectAccount key={`${workspace?.id}:${reconnectAccount.id}`} profile={reconnectAccount} onClose={() => setReconnectId(null)} onProfilesChanged={applyProfiles} notify={notify} /> : null}
      <AppModal open={createOpen} hasUnsavedChanges={createDraft.dirty} busy={createDraft.busy} onClose={() => { setCreateOpen(false); setPreviewAccent(null); }} title="Create workspace" cardClass="workspace-setup-card">{createOpen ? <WorkspaceSetup onAccountAdded={() => notify("Account added", "ok")} step={createStep} onStepChange={setCreateStep} onCreated={imported} onAccentPreview={setPreviewAccent} onStatusChange={setCreateDraft} /> : null}</AppModal>
      <AppModal open={importOpen && !!workspace} hasUnsavedChanges={importDraft.dirty} busy={importDraft.busy} onClose={() => setImportOpen(false)} onBack={() => { setImportOpen(false); setAddOpen(true); }} backLabel="All providers" title="Add Codex account">{importOpen && workspace ? <WorkspaceImportAccount workspace={workspace} onImported={imported} onStatusChange={setImportDraft} /> : null}</AppModal>
      {accountUsage && workspace ? <CodexUsageModal key={`${workspace.id}:${accountUsage.id}`} profile={accountUsage} quota={quotas[accountUsage.id]} showRemaining={workspace.usage.showRemaining}
        refreshKey={usageRefreshKey} appearance={state ? codexUsageAppearance(codexAccountPreferences(state.app, accountUsage.id), state.app.usageAppearance) : undefined} notify={notify}
        onClose={() => setAccountUsageId(null)} onUsed={() => setUsageRefreshKey((key) => key + 1)}
        onReconnect={() => { setAccountUsageId(null); setReconnectId(accountUsage.id); }} /> : null}
      <UsageModal
        status={usageFor}
        refreshKey={usageRefreshKey}
        showRemaining={usageFor?.id === "codex" ? true : state?.app?.usageShowRemaining ?? DEFAULT_APP_SETTINGS.usageShowRemaining}
        appearance={usageFor?.id === "codex" && state ? codexUsageAppearance(codexAccountPreferences(state.app, profiles?.activeProfileId ?? ""), state.app.usageAppearance) : state?.app?.usageAppearance}
        onClose={() => setUsageFor(null)}
        notify={notify}
      />
      <Toast toast={toast} />
    </>
  );
}
