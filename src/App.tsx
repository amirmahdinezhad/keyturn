import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@heroui/react";
import type { AppState } from "./types";
import {
  cliStatus,
  getState,
  prettyError,
  quitApp,
  type ProviderCliStatus,
} from "./api";
import { AppLogo } from "./logos";
import { LogoutIcon, RefreshIcon, SettingIcon } from "./icons";
import { CliProviders } from "./components/CliProviders";
import { SettingsModal } from "./components/SettingsModal";
import { HistoryModal } from "./components/HistoryModal";
import { EmptyState } from "./components/EmptyState";
import { Toast, type ToastData, type ToastKind } from "./components/Toast";
import { LoadingSkeleton } from "./components/LoadingSkeleton";

export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [cli, setCli] = useState<ProviderCliStatus[] | null>(null);
  const [cliError, setCliError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
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
    setLoadError(null);
    setState(s);
  }, []);

  const load = useCallback(async () => {
    try {
      applyState(await getState());
    } catch (e) {
      setState(null);
      setLoadError(`Couldn't load settings: ${prettyError(e)}`);
    }
    try {
      setCli(await cliStatus());
      setCliError(null);
    } catch (e) {
      setCli(null);
      setCliError(`Couldn't read CLI status: ${prettyError(e)}`);
    }
  }, [applyState]);

  useEffect(() => {
    void load();
  }, [load]);

  // Opening the panel refreshes it (the window gets focus again), and while it
  // stays open the numbers catch up on the configured interval. The backend
  // loop keeps the tray fresh when the panel is closed, including after wake.
  const refreshIntervalSecs = state?.app?.refreshIntervalSecs ?? 60;
  useEffect(() => {
    const onFocus = () => {
      void load();
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
  }, [load, refreshIntervalSecs]);

  return (
    <>
      <header className="app-header">
        <div className="brand">
          <AppLogo />
          <span>Keyturn</span>
        </div>
        <div className="header-actions">
          <Button
            isIconOnly
            size="sm"
            className="gear-btn"
            aria-label="Refresh data"
            onPress={() => void load()}
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
        ) : cliError ? (
          <EmptyState message={cliError} />
        ) : (
          <CliProviders statuses={cli} notify={notify} />
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
      <Toast toast={toast} />
    </>
  );
}
