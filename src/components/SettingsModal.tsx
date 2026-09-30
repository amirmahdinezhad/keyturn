import { useCallback, useEffect, useState } from "react";
import { Button, Input, Switch } from "@heroui/react";
import {
  checkUpdates,
  exportHistory,
  prettyError,
  setAppSettings,
  setSettings,
} from "../api";
import type {
  AppSettings,
  AppState,
  UpdateOut,
} from "../types";
import { AppModal } from "./AppModal";
import type { ToastKind } from "./Toast";

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
  state: AppState | null;
  onChanged: (s: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
  onOpenHistory: () => void;
}

type Tab = "general" | "advanced";

const TABS: { id: Tab; label: string }[] = [
  { id: "general", label: "General" },
  { id: "advanced", label: "Advanced" },
];

/** Settings: refresh cadence, billing day, history and updates. */
export function SettingsModal({
  open,
  onClose,
  state,
  onChanged,
  notify,
  onOpenHistory,
}: SettingsModalProps) {
  const [tab, setTab] = useState<Tab>("general");
  const app = state?.app;
  const [busy, setBusy] = useState(false);

  const update = useCallback(
    async (partial: Partial<AppSettings>) => {
      if (busy) return;
      setBusy(true);
      try {
        onChanged(await setAppSettings(partial));
      } catch (e) {
        notify(prettyError(e), "err");
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

  return (
    <AppModal
      open={open}
      onClose={onClose}
      title="Settings"
      cardClass="settings-card"
      bodyId="settings-body"
    >
      <div className="settings-tabs" role="tablist">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            className={`settings-tab${tab === t.id ? " on" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {!app ? (
        <p className="m-empty">Settings are still loading…</p>
      ) : tab === "general" ? (
        <GeneralTab
          state={state}
          app={app}
          onToggleLogin={(v) => void toggleLogin(v)}
          onUpdate={(p) => void update(p)}
        />
      ) : (
        <AdvancedTab
          app={app}
          onUpdate={(p) => void update(p)}
          notify={notify}
          onOpenHistory={onOpenHistory}
        />
      )}
    </AppModal>
  );
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

function NumberInput({
  value,
  min,
  max,
  placeholder,
  className = "ak-input tiny",
  onCommit,
}: {
  value: number | null;
  min: number;
  max: number;
  placeholder?: string;
  className?: string;
  onCommit: (value: number | null) => void;
}) {
  const [text, setText] = useState(value === null ? "" : String(value));
  useEffect(() => {
    setText(value === null ? "" : String(value));
  }, [value]);
  const commit = () => {
    const raw = text.trim();
    if (!raw) {
      onCommit(null);
      return;
    }
    const parsed = Number(raw);
    if (Number.isFinite(parsed)) {
      onCommit(Math.min(max, Math.max(min, Math.round(parsed))));
    } else {
      setText(value === null ? "" : String(value));
    }
  };
  return (
    <Input
      className={className}
      inputMode="numeric"
      placeholder={placeholder}
      value={text}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
    />
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
    <Switch className="ak-switch" aria-label={label} isSelected={selected} onChange={onChange}>
      <Switch.Content>
        <Switch.Control>
          <Switch.Thumb />
        </Switch.Control>
      </Switch.Content>
    </Switch>
  );
}

// ── general ────────────────────────────────────────────────────────────────

function GeneralTab({
  state,
  app,
  onToggleLogin,
  onUpdate,
}: {
  state: AppState;
  app: AppSettings;
  onToggleLogin: (v: boolean) => void;
  onUpdate: (p: Partial<AppSettings>) => void;
}) {
  return (
    <div className="settings-body">
      <section className="settings-section">
        <p className="section-label">App</p>
        <Row name="Launch at login" desc="Open Keyturn when you sign in.">
          <Toggle
            label="Launch at login"
            selected={state.settings.openAtLogin}
            onChange={onToggleLogin}
          />
        </Row>
        <Row name="Refresh interval" desc="Seconds between background refreshes (10–300).">
          <NumberInput
            value={app.refreshIntervalSecs}
            min={10}
            max={300}
            onCommit={(value) => onUpdate({ refreshIntervalSecs: value ?? 60 })}
          />
        </Row>
        <Row name="Billing cycle day" desc="Day of month used by the billing history chart.">
          <NumberInput
            value={app.billingDay}
            min={1}
            max={28}
            onCommit={(value) => onUpdate({ billingDay: value ?? 1 })}
          />
        </Row>
      </section>
    </div>
  );
}

// ── advanced ───────────────────────────────────────────────────────────────

function AdvancedTab({
  app,
  onUpdate,
  notify,
  onOpenHistory,
}: {
  app: AppSettings;
  onUpdate: (p: Partial<AppSettings>) => void;
  notify: (m: string, k?: ToastKind) => void;
  onOpenHistory: () => void;
}) {
  const [update, setUpdate] = useState<UpdateOut | null>(null);
  const [checking, setChecking] = useState(false);

  const doExport = async (format: "json" | "csv") => {
    try {
      const path = await exportHistory("30d", format);
      notify(`Exported to ${path}`, "ok");
    } catch (e) {
      notify(prettyError(e), "err");
    }
  };

  return (
    <div className="settings-body">
      <section className="settings-section">
        <p className="section-label">History</p>
        <Row name="Charts and export" desc="5h / 24h / 7d / 30d / billing cycle.">
          <div className="sound-row">
            <Button size="sm" className="disc" onPress={onOpenHistory}>
              Open history
            </Button>
            <Button size="sm" className="disc" onPress={() => void doExport("csv")}>
              Export CSV
            </Button>
            <Button size="sm" className="disc" onPress={() => void doExport("json")}>
              Export JSON
            </Button>
          </div>
        </Row>
      </section>

      <section className="settings-section">
        <p className="section-label">Updates</p>
        <Row name="Check for updates" desc="Signed builds update in place.">
          <Button
            size="sm"
            className="disc"
            isDisabled={checking}
            onPress={() => {
              setChecking(true);
              void checkUpdates()
                .then((result) => {
                  setUpdate(result);
                  if (result.available) notify(`Version ${result.version} is available`, "ok");
                })
                .catch((e) => notify(prettyError(e), "err"))
                .finally(() => setChecking(false));
            }}
          >
            {checking ? "Checking…" : "Check now"}
          </Button>
        </Row>
        {update ? (
          <p className="setting-note">
            {update.error
              ? `Updater not configured yet: ${update.error}`
              : update.available
                ? `Version ${update.version} is available.`
                : "You're on the latest version."}
          </p>
        ) : null}
        <Row name="Automatic checks" desc="Ask once a day in the background.">
          <Toggle
            label="Automatic update checks"
            selected={app.autoUpdateChecks}
            onChange={(v) => onUpdate({ autoUpdateChecks: v })}
          />
        </Row>
      </section>
    </div>
  );
}
