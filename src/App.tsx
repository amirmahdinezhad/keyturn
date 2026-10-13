import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "./components/ui/button";
import type { AppState } from "./types";
import {
  cliStatus,
  getState,
  prettyError,
  quitApp,
  type ProviderCliStatus,
} from "./api";
import { loadDisplayPrefs, applyDisplayPrefs, type DisplayPrefs } from "./displayPrefs";
import { AppLogo } from "./logos";
import { LogoutIcon, RefreshIcon, SettingIcon } from "./icons";
import { CliProviders } from "./components/CliProviders";
import { AddAccountModal } from "./components/AddAccountModal";
import { UsageModal } from "./components/UsageModal";
import { SettingsModal } from "./components/SettingsModal";
import { EmptyState } from "./components/EmptyState";
import { Toast, type ToastData, type ToastKind } from "./components/Toast";
import { LoadingSkeleton } from "./components/LoadingSkeleton";
import { formatAgo } from "./components/CliProviders";

/** Keep the tray panel mounted and coordinate status refresh, setup and popups. */
export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [cli, setCli] = useState<ProviderCliStatus[] | null>(null);
  const [cliError, setCliError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [usageFor, setUsageFor] = useState<ProviderCliStatus | null>(null);
  const [toast, setToast] = useState<ToastData | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);
  const [display, setDisplay] = useState<DisplayPrefs>(() => loadDisplayPrefs());
  const [lastUpdated, setLastUpdated] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const toastTimer = useRef(0);

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

  const refreshProviders = useCallback(async () => {
    const fresh = await cliStatus();
    setCli(fresh);
    setCliError(null);
    return fresh;
  }, []);

  const load = useCallback(async () => {
    setRefreshing(true);
    try {
      applyState(await getState());
    } catch (e) {
      setState(null);
      setLoadError(`Couldn't load settings: ${prettyError(e)}`);
    }
    try {
      await refreshProviders();
      setLastUpdated(new Date().toISOString());
    } catch (e) {
      setCli(null);
      setCliError(`Couldn't read CLI status: ${prettyError(e)}`);
    } finally {
      setRefreshing(false);
    }
  }, [applyState, refreshProviders]);

  useEffect(() => {
    void load();
  }, [load]);

  // Theme + accent paint <html> instantly and persist via displayPrefs.
  useEffect(() => {
    applyDisplayPrefs(display);
  }, [display]);

  // When following the OS, re-resolve on OS changes without a reload.
  useEffect(() => {
    if (display.theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const onChange = () => applyDisplayPrefs(display);
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [display]);
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

  return (
    <>
      <header className="app-header">
        <div className="brand">
          <AppLogo />
          <span className="brand-text">
            <span>Keyturn</span>
            {lastUpdated ? (
              <span className="brand-updated" title={lastUpdated}>
                Updated {formatAgo(lastUpdated)}
              </span>
            ) : null}
          </span>
        </div>
        <div className="header-actions">
          <Button
            size="icon-sm"
            className={`gear-btn${refreshing ? " spinning" : ""}`}
            aria-label="Refresh data"
            onClick={() => void load()}
          >
            <RefreshIcon size={20} />
          </Button>
          <Button
            size="icon-sm"
            className="gear-btn"
            aria-label="Settings"
            onClick={() => setSettingsOpen(true)}
          >
            <SettingIcon size={20} />
          </Button>
          <Button
            size="icon-sm"
            className="quit-header-btn"
            aria-label="Quit Keyturn"
            onClick={() => {
              void quitApp();
            }}
          >
            <LogoutIcon size={20} />
          </Button>
        </div>
      </header>

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
          <EmptyState message={loadError} />
        ) : !state ? (
          <LoadingSkeleton />
        ) : cliError ? (
          <EmptyState message={cliError} />
        ) : (
          <CliProviders
            statuses={cli}
            showDetails={display.showDetails}
            compactRows={display.compactRows}
            notify={notify}
            onAddAccount={() => setAddOpen(true)}
            onShowUsage={(s) => setUsageFor(s)}
          />
        )}
      </main>
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        state={state}
        onChanged={applyState}
        notify={notify}
        display={display}
        onDisplayChanged={setDisplay}
      />
      <AddAccountModal
        open={addOpen}
        candidates={(cli ?? []).filter((c) => !(c.binaryFound && c.loggedIn === true))}
        onClose={() => setAddOpen(false)}
        onRefresh={refreshProviders}
        notify={notify}
      />
      <UsageModal
        status={usageFor}
        showRemaining={state?.app?.usageShowRemaining ?? false}
        onClose={() => setUsageFor(null)}
        notify={notify}
      />
      <Toast toast={toast} />
    </>
  );
}
