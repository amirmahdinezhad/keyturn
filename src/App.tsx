import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@heroui/react";
import type { AppState, Month } from "./types";
import { accountStatus, activateAccountKey, applyKey, connectAccount, consoleKeys, consoleUsage, disconnectAccount, getState, prettyError, profileSwitch, quitApp, removeProvider, setActiveAccount, type AccountRef, type ConsoleKey, type ConsoleUsage } from "./api";
import { prettyProvider } from "./format";
import { AppLogo } from "./logos";
import { LogoutIcon, RefreshIcon, SettingIcon } from "./icons";
import { MonthNav } from "./components/MonthNav";
import { AccountKeyList, PROVIDER_OPTIONS, ProviderPicker } from "./components/ProviderPicker";
import { ModelsModal } from "./components/ModelsModal";
import { SettingsModal } from "./components/SettingsModal";
import { ProfileMenu } from "./components/ProfileMenu";
import { HistoryModal } from "./components/HistoryModal";
import { AddKeyModal } from "./components/AddKeyModal";
import { AddProviderModal } from "./components/AddProviderModal";
import {
  ConfirmDeleteModal,
  type ConfirmTarget,
} from "./components/ConfirmDeleteModal";
import { ManualKeys } from "./components/ManualKeys";
import { UsageModal, type UsageDetail } from "./components/UsageModal";
import { DEFAULT_USAGE_DISPLAY, type UsageDisplay } from "./components/UsageRows";
import { EmptyState } from "./components/EmptyState";
import { Toast, type ToastData, type ToastKind } from "./components/Toast";
import { LoadingSkeleton } from "./components/LoadingSkeleton";

function nowMonth(): Month {
  const d = new Date();
  return { year: d.getFullYear(), month: d.getMonth() };
}

function isCurrentMonth(m: Month, now: Month): boolean {
  return m.year === now.year && m.month === now.month;
}

function accountProvider(accountId: string): string {
  return accountId.split("~")[0];
}

export default function App() {
  const nowRef = useRef<Month>(nowMonth());
  const [selected, setSelected] = useState<Month>(nowRef.current);
  const [state, setState] = useState<AppState | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [profileBusy, setProfileBusy] = useState(false);
  const [accountKeys, setAccountKeys] = useState<ConsoleKey[] | null>([]);
  const [accountLoading, setAccountLoading] = useState(false);
  const [busyProvider, setBusyProvider] = useState<string | null>(null);
  const [accounts, setAccounts] = useState<AccountRef[]>([]);
  const [activeAccount, setActiveAccountId] = useState<string | null>(null);
  const [view, setView] = useState<"picker" | "account">("picker");
  const [busyKeyId, setBusyKeyId] = useState<string | null>(null);
  const [modelsKeyId, setModelsKeyId] = useState<string | null>(null);
  const [accountUsage, setAccountUsage] = useState<ConsoleUsage | null>(null);
  const [accountEmail, setAccountEmail] = useState<string | null>(null);
  const [customView, setCustomView] = useState<string | null>(null);
  const [addProviderOpen, setAddProviderOpen] = useState(false);
  const [addKeyOpen, setAddKeyOpen] = useState(false);
  const [manualBusy, setManualBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<ConfirmTarget | null>(null);
  const [usageOpen, setUsageOpen] = useState(false);
  const [toast, setToast] = useState<ToastData | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);
  const toastTimer = useRef(0);

  const notify = useCallback((message: string, kind: ToastKind = "") => {
    if (kind === "err") setLastError(message);
    else if (kind === "ok") setLastError(null);
    setToast({ message, kind });
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2200);
  }, []);

  const applyState = useCallback((s: AppState) => {
    if (s.month) setSelected(s.month);
    setLoadError(null);
    setState(s);
  }, []);

  const load = useCallback(
    async (m: Month, refresh = false) => {
      try {
        applyState(await getState(m, refresh));
      } catch (e) {
        setState(null);
        setLoadError(`Couldn't load keys: ${prettyError(e)}`);
      }
    },
    [applyState],
  );

  useEffect(() => {
    void load(nowRef.current);
    accountStatus()
      .then((s) => {
        setAccounts(s.accounts);
        setActiveAccountId(s.activeAccount ?? null);
        setView(s.activeAccount ? "account" : "picker");
      })
      .catch(() => {});
  }, [load]);

  // Account data follows the selected month, and only after the user has
  // connected the account (otherwise a launch would pop the sign-in window).
  useEffect(() => {
    if (!activeAccount || view !== "account") return;
    let cancelled = false;
    setAccountLoading(true);
    Promise.allSettled([
      consoleKeys(activeAccount, selected),
      consoleUsage(activeAccount, selected),
    ]).then(
      ([keysResult, usageResult]) => {
        if (cancelled) return;
        if (keysResult.status === "fulfilled") {
          setAccountKeys(keysResult.value.keys);
          setAccountEmail(keysResult.value.email ?? null);
        }
        setAccountUsage(
          usageResult.status === "fulfilled" ? usageResult.value : null,
        );
        setAccountLoading(false);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [activeAccount, view, selected]);

  // One refresh at a time; the console walk is heavy and serialises server-side.
  const refreshBusy = useRef(false);
  const [refreshing, setRefreshing] = useState(false);

  const refreshAll = useCallback(
    async (force: boolean) => {
      if (refreshBusy.current) return;
      refreshBusy.current = true;
      setRefreshing(true);
      try {
        await Promise.allSettled([
          load(selected, force),
          activeAccount && view === "account"
            ? (async () => {
                const [keysResult, usageResult] = await Promise.allSettled([
                  consoleKeys(activeAccount, selected, force),
                  consoleUsage(activeAccount, selected, force),
                ]);
                if (keysResult.status === "fulfilled") {
                  setAccountKeys(keysResult.value.keys);
                  setAccountEmail(keysResult.value.email ?? null);
                }
                if (usageResult.status === "fulfilled") {
                  setAccountUsage(usageResult.value);
                }
              })()
            : Promise.resolve(),
        ]);
      } finally {
        refreshBusy.current = false;
        setRefreshing(false);
      }
    },
    [load, activeAccount, view, selected],
  );

  // Opening the panel refreshes it (the window gets focus again), and while it
  // stays open the numbers catch up on the configured interval. The backend
  // loop keeps the tray fresh when the panel is closed, including after wake.
  const refreshIntervalSecs = state?.app?.refreshIntervalSecs ?? 60;
  useEffect(() => {
    const onFocus = () => {
      void refreshAll(false);
    };
    window.addEventListener("focus", onFocus);
    let last = Date.now();
    const timer = window.setInterval(() => {
      const now = Date.now();
      const drift = now - last - refreshIntervalSecs * 1000;
      last = now;
      const woke = drift > 10_000;
      if (document.hasFocus() || woke) void refreshAll(woke);
    }, refreshIntervalSecs * 1000);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.clearInterval(timer);
    };
  }, [refreshAll, refreshIntervalSecs]);

  const handleProfileSwitch = async (id: string) => {
    setProfileBusy(true);
    try {
      applyState(await profileSwitch(id));
      setAccounts([]);
      setActiveAccountId(null);
      setAccountKeys([]);
      setAccountUsage(null);
      setView("picker");
      setCustomView(null);
      notify("Profile switched", "ok");
    } catch (e) {
      notify(prettyError(e), "err");
    } finally {
      setProfileBusy(false);
    }
  };

  const shiftMonth = (delta: -1 | 1) => {
    const next = new Date(selected.year, selected.month + delta, 1);
    const candidate: Month = {
      year: next.getFullYear(),
      month: next.getMonth(),
    };
    const now = nowRef.current;
    if (
      candidate.year > now.year ||
      (candidate.year === now.year && candidate.month > now.month)
    )
      return;
    setSelected(candidate);
    void load(candidate);
  };

  const handlePickProvider = async (providerID: string) => {
    setBusyProvider(providerID);
    // Drop the previous account's keys first so they can never linger on screen.
    setAccountKeys(null);
    setAccountUsage(null);
    try {
      const status = await connectAccount(providerID, null);
      setAccounts(status.accounts);
      setActiveAccountId(status.activeAccount ?? null);
      setView("account");
    } catch (e) {
      notify(prettyError(e), "err");
    } finally {
      setBusyProvider(null);
    }
  };

  const handleResumeAccount = async (accountId: string) => {
    setBusyProvider(accountId);
    setAccountKeys(null);
    setAccountUsage(null);
    try {
      const status = await setActiveAccount(accountId);
      setAccounts(status.accounts);
      setActiveAccountId(status.activeAccount ?? accountId);
      setView("account");
    } catch (e) {
      notify(prettyError(e), "err");
    } finally {
      setBusyProvider(null);
    }
  };

  const handleDisconnect = async () => {
    if (!activeAccount) return;
    try {
      const status = await disconnectAccount(activeAccount);
      setAccounts(status.accounts);
      setActiveAccountId(status.activeAccount ?? null);
      setView(status.activeAccount ? "account" : "picker");
    } catch (e) {
      notify(prettyError(e), "err");
    }
    setAccountKeys([]);
    setAccountUsage(null);
    notify("Signed out", "ok");
  };

  const handleUseLocal = async (name: string) => {
    if (!customView) return;
    setManualBusy(name);
    try {
      applyState(await applyKey(customView, name));
      notify("Key activated", "ok");
    } catch (e) {
      notify(prettyError(e), "err");
    } finally {
      setManualBusy(null);
    }
  };

  const handleRemoveProvider = async () => {
    if (!customView) return;
    try {
      applyState(await removeProvider(customView));
      setCustomView(null);
      notify("Provider removed", "ok");
    } catch (e) {
      notify(prettyError(e), "err");
    }
  };

  const handleActivateAccountKey = async (keyId: string) => {
    setBusyKeyId(keyId);
    try {
      applyState(await activateAccountKey(keyId, activeAccount ?? "opencode-go"));
      notify("Key activated", "ok");
    } catch (e) {
      notify(prettyError(e), "err");
    } finally {
      setBusyKeyId(null);
    }
  };

  const modelsBucket = (() => {
    if (!modelsKeyId) return null;
    if (customView) {
      return state?.usage.byKey?.[customView]?.[modelsKeyId] ?? { total: 0, models: {} };
    }
    const models: Record<string, number> = {};
    let total = 0;
    for (const row of accountUsage?.usage ?? []) {
      if (row.keyId !== modelsKeyId || !row.model) continue;
      models[row.model] = (models[row.model] ?? 0) + (row.cost ?? 0);
      total += row.cost ?? 0;
    }
    return { total, models };
  })();
  const modelsKeyLabel =
    accountKeys?.find((k) => k.id === modelsKeyId)?.name ?? modelsKeyId;

  // Exact per-key cost from the account for the selected month.
  const accountCostByKey: Record<string, number> = {};
  for (const row of accountUsage?.usage ?? []) {
    if (!row.keyId) continue;
    accountCostByKey[row.keyId] = (accountCostByKey[row.keyId] ?? 0) + (row.cost ?? 0);
  }

  const provider = activeAccount ? accountProvider(activeAccount) : "opencode-go";
  const activeKeyName = state?.applied?.[provider] ?? null;
  const activeQuota = activeKeyName
    ? state?.providerUsage?.[provider]?.[activeKeyName]
    : undefined;
  const goWindows = provider === "opencode-go" ? accountUsage?.go : undefined;
  const usageAvailable = Boolean(goWindows?.length || activeQuota?.windows?.length);

  const monthTotal = (() => {
    if (customView) {
      const buckets = state?.usage.byKey?.[customView] ?? {};
      return Object.values(buckets).reduce((sum, b) => sum + b.total, 0);
    }
    if (accountUsage) {
      return accountUsage.usage.reduce((sum, row) => sum + (row.cost ?? 0), 0);
    }
    const buckets = state?.usage.byKey?.[provider] ?? {};
    return Object.values(buckets).reduce((sum, b) => sum + b.total, 0);
  })();

  const showUsageLimits = () => {
    setUsageOpen(true);
    // Pull fresh numbers while the modal opens; the modal derives from live
    // state, so whatever lands shows up immediately.
    void refreshAll(false);
  };

  const usageDisplay: UsageDisplay = state?.app
    ? {
        mode: state.app.usagePercentMode,
        reset: state.app.resetDisplay,
        time: state.app.timeFormat,
        showPace: true,
      }
    : DEFAULT_USAGE_DISPLAY;

  const browsing = isCurrentMonth(selected, nowRef.current);
  const providerLabel =
    PROVIDER_OPTIONS.find((p) => p.id === provider)?.label ??
    prettyProvider(provider);

  // Limits modal is derived from live state at render time so background
  // refreshes land in it while it stays open.
  const usageDetail: UsageDetail | null = usageOpen
    ? {
        label: accountEmail ?? activeKeyName ?? providerLabel,
        providerID: provider,
        windows: activeQuota?.windows ?? [],
        go: goWindows,
      }
    : null;

  return (
    <>
      <header className="app-header">
        <div className="brand">
          <AppLogo />
          <span>Keyturn</span>
        </div>
        <div className="header-actions">
          {state ? (
            <ProfileMenu
              profiles={state.profiles}
              active={state.activeProfile}
              busy={profileBusy}
              onSwitch={(id) => void handleProfileSwitch(id)}
              onManage={() => setSettingsOpen(true)}
            />
          ) : null}
          <Button
            isIconOnly
            size="sm"
            className={`gear-btn${refreshing ? " spinning" : ""}`}
            aria-label="Refresh data"
            isDisabled={refreshing}
            onPress={() => void refreshAll(true)}
          >
            <RefreshIcon size={18} />
          </Button>
          <Button
            isIconOnly
            size="sm"
            className="gear-btn"
            aria-label="Settings"
            onPress={() => setSettingsOpen(true)}
          >
            <SettingIcon size={18} />
          </Button>
          <Button
            isIconOnly
            size="sm"
            className="quit-header-btn"
            aria-label="Quit Keyturn"
            onPress={() => {
              void quitApp();
            }}
          >
            <LogoutIcon size={18} />
          </Button>
        </div>
      </header>

      {state && view !== "picker" && (
        <MonthNav
          selected={selected}
          isCurrentMonth={browsing}
          onShift={shiftMonth}
          total={monthTotal}
          loading={view === "account" && accountLoading && !accountUsage}
        />
      )}

      {lastError && (
        <div className="error-panel" role="alert">
          <div className="error-head">
            <span className="error-title">{lastError.split("\n")[0]}</span>
            <span className="error-actions">
              <button
                type="button"
                className="link-btn"
                onClick={() => {
                  void navigator.clipboard?.writeText(lastError);
                }}
              >
                Copy
              </button>
              <button
                type="button"
                className="link-btn"
                onClick={() => setLastError(null)}
              >
                Dismiss
              </button>
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
          <EmptyState message={loadError} />
        ) : !state ? (
          <LoadingSkeleton />
        ) : view === "picker" && customView ? (
          <ManualKeys
            provider={customView}
            state={state}
            browsing={browsing}
            busyName={manualBusy}
            onUse={(name) => void handleUseLocal(name)}
            onAdd={() => setAddKeyOpen(true)}
            onDelete={(name) => setConfirm({ provider: customView, name })}
            onShowModels={(name) => setModelsKeyId(name)}
            onRemoveProvider={() => void handleRemoveProvider()}
            onBack={() => setCustomView(null)}
            removable={!PROVIDER_OPTIONS.some((p) => p.id === customView)}
          />
        ) : view === "picker" || activeAccount === null ? (
          <ProviderPicker
            onPick={(id) => void handlePickProvider(id)}
            busyId={busyProvider}
            customProviders={state.customProviders}
            accounts={accounts}
            activeId={activeAccount}
            onResume={(id) => void handleResumeAccount(id)}
            onPickCustom={(id) => {
              setCustomView(id);
            }}
            onAddProvider={() => setAddProviderOpen(true)}
          />
        ) : accountKeys === null ? (
          <LoadingSkeleton />
        ) : (
          <AccountKeyList
            keys={accountKeys}
            provider={provider}
            providerLabel={providerLabel}
            email={accountEmail}
            costByKey={accountCostByKey}
            loading={accountLoading}
            activeName={state.applied?.[provider] ?? null}
            busyId={busyKeyId}
            browsing={browsing}
            usageAvailable={usageAvailable}
            onUse={(keyId) => void handleActivateAccountKey(keyId)}
            onShowModels={(keyId) => setModelsKeyId(keyId)}
            onShowUsage={showUsageLimits}
            onSwitch={() => setView("picker")}
            onLogout={() => void handleDisconnect()}
          />
        )}
      </main>
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        state={state}
        onChanged={applyState}
        notify={notify}
        onOpenHistory={() => {
          setSettingsOpen(false);
          setHistoryOpen(true);
        }}
      />
      <HistoryModal
        open={historyOpen}
        onClose={() => setHistoryOpen(false)}
        notify={notify}
      />
      <ModelsModal
        keyName={modelsKeyLabel}
        bucket={modelsBucket}
        onClose={() => setModelsKeyId(null)}
      />
      <UsageModal
        detail={usageDetail}
        onClose={() => setUsageOpen(false)}
        display={usageDisplay}
      />
      <ConfirmDeleteModal
        target={confirm}
        onClose={() => setConfirm(null)}
        onDeleted={applyState}
        notify={notify}
      />
      <AddProviderModal
        open={addProviderOpen}
        onClose={() => setAddProviderOpen(false)}
        onAdded={(next) => {
          applyState(next);
          setAddProviderOpen(false);
        }}
        notify={notify}
      />
      <AddKeyModal
        open={addKeyOpen}
        onClose={() => setAddKeyOpen(false)}
        providerIds={[
          ...PROVIDER_OPTIONS.map((p) => p.id),
          ...(state?.customProviders ?? []),
        ]}
        prefill={customView ? { provider: customView, name: "" } : null}
        onSaved={applyState}
        notify={notify}
      />
      <Toast toast={toast} />
    </>
  );
}
