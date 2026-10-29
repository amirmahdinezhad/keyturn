import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "./ui/toggle-group";
import { Progress } from "./ui/progress";
import {
  check,
  type DownloadEvent,
  type Update,
} from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { prettyError, setAppSettings, setSettings } from "../api";
import { DEFAULT_APP_SETTINGS, type AppSettings, type AppState } from "../types";
import {
  applyDisplayPrefs,
  DEFAULT_DISPLAY_PREFS,
  loadDisplayPrefs,
  resetDisplayPrefs,
  saveDisplayPrefs,
  type DisplayPrefs,
  type ThemeMode,
} from "../displayPrefs";
import { AppModal } from "./AppModal";
import type { ToastKind } from "./Toast";
import { AppLogo } from "../logos";
import { SpinnerIcon, TickIcon } from "../icons";

/** localStorage key remembering the version the user asked to skip. */
export const SKIPPED_UPDATE_KEY = "keyturn.skippedUpdateVersion";

function skippedVersion(): string | null {
  try {
    return localStorage.getItem(SKIPPED_UPDATE_KEY);
  } catch {
    return null;
  }
}

function setSkippedVersion(version: string | null) {
  try {
    if (version === null) localStorage.removeItem(SKIPPED_UPDATE_KEY);
    else localStorage.setItem(SKIPPED_UPDATE_KEY, version);
  } catch {
    // Private mode etc: skipping just won't persist.
  }
}

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
  state: AppState | null;
  onChanged: (s: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
  /** Frontend-only display prefs, lifted to App so the list reacts instantly. */
  display?: DisplayPrefs;
  onDisplayChanged?: (d: DisplayPrefs) => void;
}

type SettingsTab = "general" | "updates";
const SETTINGS_TABS: { id: SettingsTab; label: string }[] = [
  { id: "general", label: "General" },
  { id: "updates", label: "Updates" },
];

/**
 * Global behavior, theme and updates; account usage and alerts remain workspace-scoped.
 * General owns the single reset action; Updates shows one primary action
 * for the current phase, while keeping download and restart separate.
 */
export function SettingsModal({
  open,
  onClose,
  state,
  onChanged,
  notify,
  display: displayProp,
  onDisplayChanged,
}: SettingsModalProps) {
  const app = state?.app;
  const [busy, setBusy] = useState(false);
  const [phase, setPhase] = useState<UpdatePhase>({ kind: "idle" });
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);
  const [tab, setTab] = useState<SettingsTab>("general");
  const updateBusy = useRef(false);
  const lastAttempt = useRef(0);
  const [keyboardNavigation, setKeyboardNavigation] = useState(false);
  const [displayState, setDisplayState] = useState<DisplayPrefs>(
    () => displayProp ?? loadDisplayPrefs(),
  );

  useLayoutEffect(() => {
    const body = document.getElementById("settings-body");
    if (body) body.scrollTop = 0;
  }, [tab]);

  const display = displayProp ?? displayState;
  const setDisplay = useCallback(
    (next: DisplayPrefs) => {
      saveDisplayPrefs(next);
      setDisplayState(next);
      onDisplayChanged?.(next);
    },
    [onDisplayChanged],
  );

  // Instant preview even when the modal owns its own state (tests/stories).
  useEffect(() => {
    applyDisplayPrefs(display);
  }, [display]);

  useEffect(() => {
    if (!open) return;
    setTab("general");
    getVersion()
      .then((v) => setCurrentVersion(v))
      .catch(() => setCurrentVersion(null));
  }, [open]);

  const updatePrefs = useCallback(
    async (partial: Partial<AppSettings>) => {
      if (busy) return false;
      setBusy(true);
      try {
        onChanged(await setAppSettings(partial));
        return true;
      } catch (e) {
        notify(prettyError(e), "err");
        return false;
      } finally {
        setBusy(false);
      }
    },
    [busy, notify, onChanged],
  );

  const toggleLogin = useCallback(
    async (next: boolean) => {
      try {
        onChanged(await setSettings(next));
      } catch (e) {
        notify(prettyError(e), "err");
      }
    },
    [notify, onChanged],
  );

  const checkNow = useCallback((automatic = false) => {
    if (updateBusy.current) return;
    updateBusy.current = true;
    lastAttempt.current = Date.now();
    setPhase({ kind: "checking" });
    void check().then(async (found) => {
      try { localStorage.setItem("keyturn.lastUpdateCheck", new Date().toISOString()); } catch { /* Optional history. */ }
      if (!found || (automatic && found.version === skippedVersion())) { setPhase({ kind: "current" }); return; }
      setPhase({ kind: "available", update: found });
      notify(`Version ${found.version} is available`, "ok");
      if (automatic) {
        setPhase({ kind: "downloading", progress: null });
        await downloadUpdate(found, (progress) => setPhase({ kind: "downloading", progress }));
        setPhase({ kind: "downloaded", update: found });
      }
    }).catch((e) => {
      const message = prettyError(e); setPhase({ kind: "failed", message }); notify(message, "err");
    }).finally(() => { updateBusy.current = false; });
  }, [notify]);

  // This component remains mounted while Settings and the tray are closed.
  useEffect(() => {
    if (!display.checkUpdatesOnLaunch) return;
    const dailyCheck = () => {
      if (updateBusy.current || phase.kind === "available" || phase.kind === "downloaded" || phase.kind === "installing") return;
      let checked = 0;
      try { checked = Date.parse(localStorage.getItem("keyturn.lastUpdateCheck") ?? "") || 0; } catch { /* Optional history. */ }
      if (Date.now() - Math.max(checked, lastAttempt.current) >= 86_400_000) checkNow(true);
    };
    dailyCheck(); const timer = window.setInterval(dailyCheck, 60_000);
    return () => window.clearInterval(timer);
  }, [display.checkUpdatesOnLaunch, checkNow, phase.kind]);

  const skipVersion = useCallback(
    (version: string) => {
      setSkippedVersion(version);
      setPhase({ kind: "current" });
      notify(`Version ${version} skipped`, "ok");
    },
    [notify],
  );

  const downloadOnly = useCallback((update: Update) => {
    if (updateBusy.current) return;
    updateBusy.current = true; setPhase({ kind: "downloading", progress: null });
    void downloadUpdate(update, (progress) => setPhase({ kind: "downloading", progress }))
      .then(() => setPhase({ kind: "downloaded", update }))
      .catch((e) => { const message = prettyError(e); setPhase({ kind: "failed", message }); notify(message, "err"); })
      .finally(() => { updateBusy.current = false; });
  }, [notify]);

  const installAndRelaunch = useCallback((update: Update) => {
    if (updateBusy.current) return;
    updateBusy.current = true; setPhase({ kind: "installing" });
    void update.install().then(() => relaunch())
      .catch((e) => { const message = prettyError(e); setPhase({ kind: "failed", message }); notify(message, "err"); })
      .finally(() => { updateBusy.current = false; });
  }, [notify]);

  const resetAll = useCallback(() => {
    // Reset global preferences without replacing workspace membership or policies.
    void updatePrefs({ refreshIntervalSecs: DEFAULT_APP_SETTINGS.refreshIntervalSecs,
      usageShowRemaining: DEFAULT_APP_SETTINGS.usageShowRemaining }).then((saved) => {
      if (!saved) return;
      setDisplay(resetDisplayPrefs()); setDisplayState({ ...DEFAULT_DISPLAY_PREFS });
      notify("Settings reset to defaults", "ok");
    });
  }, [notify, setDisplay, updatePrefs]);

  return (
    <Tabs
      value={tab}
      onValueChange={(value) => {
        const next = value as SettingsTab;
        setTab(next);
      }}
      className="settings-tabs-root"
    >
      <AppModal
        open={open}
        onClose={onClose}
        title="Global settings"
        cardClass="settings-card"
        bodyId="settings-body"
        navigation={
          app && state ? (
            <TabsList
              className="settings-tabs ios-segments"
              aria-label="Settings sections"
              style={segmentStyle(
                SETTINGS_TABS.length,
                SETTINGS_TABS.findIndex((item) => item.id === tab),
                keyboardNavigation,
              )}
              onPointerDownCapture={() => setKeyboardNavigation(false)}
              onKeyDownCapture={() => setKeyboardNavigation(true)}
            >
              {SETTINGS_TABS.map(({ id, label }) => (
                <TabsTrigger key={id} value={id} className="settings-tab">
                  {label}
                </TabsTrigger>
              ))}
            </TabsList>
          ) : null
        }
      >
        {!app || !state ? (
          <p className="m-empty">Settings are still loading…</p>
        ) : (
          <div
            className={`settings-body${keyboardNavigation ? " no-motion" : ""}`}
          >
            <TabsContent value="general" className="settings-panel settings-general">
              <section
                className="settings-section settings-group"
                aria-label="App behavior"
              >
                <Row name="Launch at login">
                  <Toggle
                    label="Launch at login"
                    selected={state.settings.openAtLogin}
                    onChange={(v) => void toggleLogin(v)}
                  />
                </Row>
                <Row name="Refresh on focus">
                  <Toggle
                    label="Refresh on focus"
                    selected={display.refreshOnFocus}
                    onChange={(v) =>
                      setDisplay({ ...display, refreshOnFocus: v })
                    }
                  />
                </Row>
                <Row name="Refresh interval">
                  <RefreshInterval
                    value={app.refreshIntervalSecs}
                    disabled={busy}
                    onCommit={(value) =>
                      void updatePrefs({ refreshIntervalSecs: value })
                    }
                  />
                </Row>
              </section>
              <p className="setting-note">
                Checks connected CLIs while the panel is open.
              </p>
              <section
                className="settings-section settings-group"
                aria-label="Appearance"
              >
                <Row name="Theme">
                  <ToggleGroup
                    type="single"
                    className="segmented ios-segments theme-segments"
                    style={segmentStyle(
                      3,
                      ["dark", "light", "system"].indexOf(display.theme),
                    )}
                    aria-label="Color theme"
                    value={display.theme}
                    onValueChange={(mode) => {
                      if (mode)
                        setDisplay({ ...display, theme: mode as ThemeMode });
                    }}
                  >
                    {(["dark", "light", "system"] as ThemeMode[]).map(
                      (mode) => (
                        <ToggleGroupItem
                          key={mode}
                          value={mode}
                          className="segment"
                        >
                          {mode === "dark"
                            ? "Dark"
                            : mode === "light"
                              ? "Light"
                              : "System"}
                        </ToggleGroupItem>
                      ),
                    )}
                  </ToggleGroup>
                </Row>
              </section>
              <div className="settings-footer">
                <Button
                  type="button"
                  className="link-btn reset-settings"
                  disabled={busy}
                  onClick={resetAll}
                >
                  Reset to defaults
                </Button>
              </div>
            </TabsContent>

            <TabsContent value="updates" className="settings-panel">
              <section className="settings-section">
                <UpdateRows
                  phase={phase}
                  currentVersion={currentVersion}
                  onCheck={() => checkNow()}
                  onSkip={skipVersion}
                  onDownload={downloadOnly}
                  onInstallRelaunch={installAndRelaunch}
                />
              </section>
              <section className="settings-section settings-group">
                <Row name="Automatic updates">
                  <Toggle
                    label="Automatic updates"
                    selected={display.checkUpdatesOnLaunch}
                    onChange={(v) =>
                      setDisplay({ ...display, checkUpdatesOnLaunch: v })
                    }
                  />
                </Row>
              </section>
              <p className="setting-note updates-note">
                Checks and downloads once a day, including while Settings is closed. Installation waits for you.
              </p>
            </TabsContent>
          </div>
        )}
      </AppModal>
    </Tabs>
  );
}

/** Shared progress accounting for manual and automatic downloads. */
async function downloadUpdate(update: Update, onProgress: (progress: number | null) => void) {
  let total = 0, done = 0;
  await update.download((event: DownloadEvent) => {
    if (event.event === "Started") { total = event.data.contentLength ?? 0; done = 0; }
    if (event.event === "Progress") done += event.data.chunkLength;
    onProgress(total > 0 ? Math.min(1, done / total) : null);
  });
}

type UpdatePhase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "current" }
  | { kind: "available"; update: Update }
  | { kind: "downloading"; progress: number | null }
  | { kind: "downloaded"; update: Update }
  | { kind: "installing" }
  | { kind: "failed"; message: string };

function UpdateRows({
  phase,
  currentVersion,
  onCheck,
  onSkip,
  onDownload,
  onInstallRelaunch,
}: {
  phase: UpdatePhase;
  currentVersion: string | null;
  onCheck: () => void;
  onSkip: (version: string) => void;
  onDownload: (update: Update) => void;
  onInstallRelaunch: (update: Update) => void;
}) {
  const message =
    phase.kind === "installing" ? "Installing update…" : phase.kind === "checking"
      ? "Checking for updates…"
      : phase.kind === "current"
        ? "You're on the latest version."
        : phase.kind === "available"
          ? `Version ${phase.update.version} is available`
          : phase.kind === "downloading"
            ? "Downloading update"
            : phase.kind === "downloaded"
              ? `Version ${phase.update.version} downloaded`
              : phase.kind === "failed"
                ? phase.message
                : "Keep Keyturn up to date.";

  return (
    <div className="update-card">
      <div className="update-brand">
        <AppLogo />
        <div>
          <h3>Keyturn</h3>
          <p>
            Version <span>{currentVersion ?? "…"}</span>
          </p>
        </div>
      </div>
      <div
        className={`update-status${phase.kind === "failed" ? " failed" : ""}`}
        aria-live="polite"
      >
        {phase.kind === "current" ? <TickIcon size={20} /> : null}
        {phase.kind === "checking" || phase.kind === "downloading" || phase.kind === "installing" ? (
          <SpinnerIcon size={18} />
        ) : null}
        <span>{message}</span>
      </div>
      {(phase.kind === "available" || phase.kind === "downloaded") && phase.update.body ? (
        <details className="update-release-notes">
          <summary>What's new</summary>
          <p className="setting-note release-notes">{phase.update.body}</p>
        </details>
      ) : null}
      {phase.kind === "downloading" ? (
        <>
          <Progress
            aria-label="Download progress"
            className={`update-progress${phase.progress === null ? " indeterminate" : ""}`}
            value={
              phase.progress === null
                ? undefined
                : Math.round(phase.progress * 100)
            }
          />
          <p className="setting-note">Ready to install after the download.</p>
        </>
      ) : phase.kind === "available" ? (
        <>
          <Button
            className="primary update-primary"
            onClick={() => onDownload(phase.update)}
          >
            Download
          </Button>
          <Button
            className="link-btn update-skip"
            onClick={() => onSkip(phase.update.version)}
          >
            Skip this version
          </Button>
        </>
      ) : phase.kind === "downloaded" ? (
        <Button
          className="primary update-primary"
          onClick={() => onInstallRelaunch(phase.update)}
        >
          Install & relaunch
        </Button>
      ) : (
        <Button
          className="update-primary"
          onClick={onCheck}
          disabled={phase.kind === "checking" || phase.kind === "installing"}
        >
          {phase.kind === "installing" ? "Installing…" : phase.kind === "checking" ? "Checking…" : "Check for updates"}
        </Button>
      )}
    </div>
  );
}

/** Equal-width sliding segments: CSS keeps the indicator interruptible. */
function segmentStyle(
  count: number,
  index: number,
  instant = false,
): CSSProperties {
  return {
    "--segment-count": count,
    "--segment-index": Math.max(0, index),
    ...(instant ? { "--segment-duration": "0ms" } : {}),
  } as CSSProperties;
}

// ── shared bits ────────────────────────────────────────────────────────────

function Row({
  name,
  desc,
  children,
}: {
  name: string;
  desc?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="setting-row">
      <div className="setting-text">
        <span className="setting-name">{name}</span>
        {desc ? <span className="setting-desc">{desc}</span> : null}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

const REFRESH_PRESETS = [30, 60, 120];

/** Match the theme segments while preserving a previously saved custom cadence. */
function RefreshInterval({
  value,
  disabled,
  onCommit,
}: {
  value: number;
  disabled: boolean;
  onCommit: (value: number) => void;
}) {
  const options = REFRESH_PRESETS.includes(value)
    ? REFRESH_PRESETS
    : [...REFRESH_PRESETS, value].sort((a, b) => a - b);

  return (
    <ToggleGroup
      type="single"
      className="segmented ios-segments refresh-segments"
      style={segmentStyle(options.length, options.indexOf(value))}
      aria-label="Refresh interval"
      value={String(value)}
      disabled={disabled}
      onValueChange={(preset) => {
        if (preset) onCommit(Number(preset));
      }}
    >
      {options.map((preset) => (
        <ToggleGroupItem
          key={preset}
          value={String(preset)}
          className="segment"
        >
          {preset}s
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}

function Toggle({
  selected,
  onChange,
  label,
}: {
  selected: boolean;
  onChange: (v: boolean) => void;
  label: string;
}) {
  return (
    <Switch aria-label={label} checked={selected} onCheckedChange={onChange} />
  );
}
