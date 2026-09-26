import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Input, Switch } from "@heroui/react";
import {
  checkUpdates,
  codexRefresh,
  codexStatus,
  exportHistory,
  listSounds,
  playSound,
  prettyError,
  profileCreate,
  profileDelete,
  profileRename,
  profileSwitch,
  setAppSettings,
  setSettings,
} from "../api";
import type {
  AppSettings,
  AppState,
  CodexOut,
  ProfileRef,
  TrayPrimary,
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

type Tab = "general" | "alerts" | "tray" | "automation" | "advanced";

const TABS: { id: Tab; label: string }[] = [
  { id: "general", label: "General" },
  { id: "alerts", label: "Alerts" },
  { id: "tray", label: "Menu bar" },
  { id: "automation", label: "Automation" },
  { id: "advanced", label: "Advanced" },
];

const TRAY_STYLES: { id: AppSettings["trayStyle"]; label: string; hint: string }[] = [
  { id: "battery", label: "Battery", hint: "Battery outline filled to the primary metric" },
  { id: "bar", label: "Progress Bar", hint: "One stacked bar per selected metric" },
  { id: "percent", label: "Percentage", hint: "Key icon with the percent in the menu bar" },
  { id: "iconbar", label: "Icon with Bar", hint: "Key icon above a thin bar" },
  { id: "compact", label: "Compact", hint: "Text only: 42|18" },
];

const METRICS: { id: string; label: string }[] = [
  { id: "rolling", label: "5-hour" },
  { id: "weekly", label: "Weekly" },
  { id: "monthly", label: "Monthly" },
];

/** Settings: profile management, display modes, alerts, tray and automation. */
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
          onChanged={onChanged}
          notify={notify}
          onToggleLogin={(v) => void toggleLogin(v)}
          onUpdate={(p) => void update(p)}
        />
      ) : tab === "alerts" ? (
        <AlertsTab app={app} onUpdate={(p) => void update(p)} notify={notify} />
      ) : tab === "tray" ? (
        <TrayTab state={state} app={app} onUpdate={(p) => void update(p)} />
      ) : tab === "automation" ? (
        <AutomationTab app={app} onUpdate={(p) => void update(p)} />
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

function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { id: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="segmented">
      {options.map((option) => (
        <button
          key={option.id}
          type="button"
          className={`segment${value === option.id ? " on" : ""}`}
          onClick={() => onChange(option.id)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function Check({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
}) {
  return (
    <label className="check">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{label}</span>
    </label>
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
  onChanged,
  notify,
  onToggleLogin,
  onUpdate,
}: {
  state: AppState;
  app: AppSettings;
  onChanged: (s: AppState) => void;
  notify: (m: string, k?: ToastKind) => void;
  onToggleLogin: (v: boolean) => void;
  onUpdate: (p: Partial<AppSettings>) => void;
}) {
  const [newName, setNewName] = useState("");
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const act = async (fn: () => Promise<AppState>) => {
    try {
      onChanged(await fn());
    } catch (e) {
      notify(prettyError(e), "err");
    }
  };

  return (
    <div className="settings-body">
      <section className="settings-section">
        <p className="section-label">Profiles</p>
        <div className="profile-list">
          {state.profiles.map((profile: ProfileRef) => (
            <div
              key={profile.id}
              className={`profile-row${profile.id === state.activeProfile ? " on" : ""}`}
            >
              {renaming?.id === profile.id ? (
                <Input
                  className="ak-input"
                  value={renaming.name}
                  autoFocus
                  onChange={(e) => setRenaming({ id: profile.id, name: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      void act(async () => {
                        const next = await profileRename(profile.id, renaming.name);
                        setRenaming(null);
                        return next;
                      });
                    }
                    if (e.key === "Escape") setRenaming(null);
                  }}
                />
              ) : (
                <span className="profile-name">{profile.name}</span>
              )}
              {profile.id === state.activeProfile ? (
                <span className="mini-chip on">Active</span>
              ) : (
                <Button
                  size="sm"
                  className="disc"
                  onPress={() => void act(() => profileSwitch(profile.id))}
                >
                  Use
                </Button>
              )}
              <button
                type="button"
                className="link-btn"
                onClick={() =>
                  setRenaming(
                    renaming?.id === profile.id
                      ? null
                      : { id: profile.id, name: profile.name },
                  )
                }
              >
                Rename
              </button>
              {confirmDelete === profile.id ? (
                <button
                  type="button"
                  className="link-btn danger"
                  onClick={() =>
                    void act(async () => {
                      const next = await profileDelete(profile.id);
                      setConfirmDelete(null);
                      return next;
                    })
                  }
                >
                  Confirm delete
                </button>
              ) : (
                <button
                  type="button"
                  className="link-btn danger"
                  disabled={state.profiles.length <= 1}
                  onClick={() => setConfirmDelete(profile.id)}
                >
                  Delete
                </button>
              )}
            </div>
          ))}
        </div>
        <div className="profile-create">
          <Input
            className="ak-input"
            placeholder="New profile name"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && newName.trim()) {
                void act(async () => {
                  const next = await profileCreate(newName.trim());
                  setNewName("");
                  return next;
                });
              }
            }}
          />
          <Button
            size="sm"
            className="primary"
            isDisabled={!newName.trim()}
            onPress={() =>
              void act(async () => {
                const next = await profileCreate(newName.trim());
                setNewName("");
                return next;
              })
            }
          >
            Add
          </Button>
        </div>
        <p className="setting-note">
          Every profile keeps its own keys, signed-in accounts, switch log and history.
          App-wide display and alert settings stay shared.
        </p>
      </section>

      <section className="settings-section">
        <p className="section-label">App</p>
        <Row name="Launch at login" desc="Open Keyturn when you sign in.">
          <Toggle
            label="Launch at login"
            selected={state.settings.openAtLogin}
            onChange={onToggleLogin}
          />
        </Row>
        <Row name="Theme" desc="More themes coming soon.">
          <span className="theme-current">
            {state.settings.theme === "light" ? "Light" : "Dark"}
          </span>
        </Row>
        <Row name="Reset countdown" desc="How reset times are worded.">
          <Segmented
            value={app.resetDisplay}
            onChange={(v) => onUpdate({ resetDisplay: v })}
            options={[
              { id: "clock", label: "Clock" },
              { id: "remaining", label: "Remaining" },
              { id: "both", label: "Both" },
            ]}
          />
        </Row>
        <Row name="Usage percentages" desc="Show used, remaining, or both.">
          <Segmented
            value={app.usagePercentMode}
            onChange={(v) => onUpdate({ usagePercentMode: v })}
            options={[
              { id: "used", label: "Used" },
              { id: "remaining", label: "Left" },
              { id: "both", label: "Both" },
            ]}
          />
        </Row>
        <Row name="Time format" desc="Clock display in reset times.">
          <Segmented
            value={app.timeFormat}
            onChange={(v) => onUpdate({ timeFormat: v })}
            options={[
              { id: "system", label: "System" },
              { id: "12h", label: "12h" },
              { id: "24h", label: "24h" },
            ]}
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

// ── alerts ─────────────────────────────────────────────────────────────────

function AlertsTab({
  app,
  onUpdate,
  notify,
}: {
  app: AppSettings;
  onUpdate: (p: Partial<AppSettings>) => void;
  notify: (m: string, k?: ToastKind) => void;
}) {
  const [sounds, setSounds] = useState<string[]>([]);

  useEffect(() => {
    listSounds()
      .then(setSounds)
      .catch(() => setSounds([]));
  }, []);

  const toggleThreshold = (threshold: number, on: boolean) => {
    const next = on
      ? [...app.notifyThresholds, threshold].sort((a, b) => a - b)
      : app.notifyThresholds.filter((t) => t !== threshold);
    onUpdate({ notifyThresholds: next.length ? next : [75, 90, 95] });
  };

  return (
    <div className="settings-body">
      <section className="settings-section">
        <p className="section-label">Usage alerts</p>
        <Row name="Notifications" desc="Notify when a key crosses a threshold.">
          <Toggle
            label="Notifications"
            selected={app.notifyEnabled}
            onChange={(v) => onUpdate({ notifyEnabled: v })}
          />
        </Row>
        <Row name="Thresholds" desc="Preset crossings, plus one custom value.">
          <div className="chip-row">
            {[75, 90, 95].map((threshold) => (
              <Check
                key={threshold}
                label={`${threshold}%`}
                checked={app.notifyThresholds.includes(threshold)}
                onChange={(on) => toggleThreshold(threshold, on)}
              />
            ))}
          </div>
        </Row>
        <Row name="Custom threshold" desc="Optional extra crossing (1–100).">
          <NumberInput
            value={app.notifyCustom}
            min={1}
            max={100}
            placeholder="—"
            onCommit={(value) => onUpdate({ notifyCustom: value })}
          />
        </Row>
      </section>

      <section className="settings-section">
        <p className="section-label">Sound</p>
        <Row name="Play a sound" desc="Uses the system sound library.">
          <Toggle
            label="Sound"
            selected={app.soundEnabled}
            onChange={(v) => onUpdate({ soundEnabled: v })}
          />
        </Row>
        <Row name="Sound" desc={sounds.length ? `${sounds.length} sounds available` : "No sound library found"}>
          <div className="sound-row">
            <select
              className="ak-select"
              value={app.soundName ?? ""}
              onChange={(e) => onUpdate({ soundName: e.target.value || null })}
            >
              <option value="">Default</option>
              {sounds.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
            <Button
              size="sm"
              className="disc"
              isDisabled={!app.soundName}
              onPress={() =>
                void playSound(app.soundName ?? "").catch((e) =>
                  notify(prettyError(e), "err"),
                )
              }
            >
              Preview
            </Button>
          </div>
        </Row>
      </section>
    </div>
  );
}

// ── tray ───────────────────────────────────────────────────────────────────

function TrayTab({
  state,
  app,
  onUpdate,
}: {
  state: AppState;
  app: AppSettings;
  onUpdate: (p: Partial<AppSettings>) => void;
}) {
  const toggleMetric = (id: string, on: boolean) => {
    const next = on
      ? [...app.trayMetrics, id]
      : app.trayMetrics.filter((m) => m !== id);
    onUpdate({ trayMetrics: next.length ? next : ["rolling"] });
  };
  const toggleProfile = (id: string, on: boolean) => {
    const next = on
      ? [...app.trayProfiles, id]
      : app.trayProfiles.filter((p) => p !== id);
    onUpdate({ trayProfiles: next });
  };

  return (
    <div className="settings-body">
      <section className="settings-section">
        <p className="section-label">Icon</p>
        <div className="style-grid">
          {TRAY_STYLES.map((style) => (
            <button
              key={style.id}
              type="button"
              className={`style-card${app.trayStyle === style.id ? " on" : ""}`}
              onClick={() => onUpdate({ trayStyle: style.id })}
            >
              <span className="style-name">{style.label}</span>
              <span className="style-hint">{style.hint}</span>
            </button>
          ))}
        </div>
        <Row name="Colour mode" desc="Multi-colour uses thresholds, mono follows the menu bar tint.">
          <Segmented
            value={app.trayColorMode}
            onChange={(v) => onUpdate({ trayColorMode: v })}
            options={[
              { id: "multi", label: "Multi" },
              { id: "mono", label: "Mono" },
              { id: "custom", label: "Custom" },
            ]}
          />
        </Row>
        {app.trayColorMode === "custom" ? (
          <Row name="Custom colour" desc="Hex value used for every fill.">
            <input
              type="color"
              className="ak-color"
              value={app.trayCustomColor}
              onChange={(e) => onUpdate({ trayCustomColor: e.target.value })}
            />
          </Row>
        ) : null}
      </section>

      <section className="settings-section">
        <p className="section-label">Numbers</p>
        <Row name="Value" desc="Percent of the window, or recorded tokens.">
          <Segmented
            value={app.trayUsedMode}
            onChange={(v) => onUpdate({ trayUsedMode: v })}
            options={[
              { id: "percent", label: "Percent" },
              { id: "tokens", label: "Tokens" },
            ]}
          />
        </Row>
        <Row name="Show remaining" desc="Display 100 − used instead of used.">
          <Toggle
            label="Show remaining"
            selected={app.trayShowRemaining}
            onChange={(v) => onUpdate({ trayShowRemaining: v })}
          />
        </Row>
        <Row
          name="Reported window"
          desc="What the battery, percentage and key styles speak for. Worst window follows whichever selected window is closest to its limit."
        >
          <select
            className="ak-select"
            aria-label="Reported window"
            value={app.trayPrimary}
            onChange={(e) => onUpdate({ trayPrimary: e.target.value as TrayPrimary })}
          >
            <option value="auto">First selected</option>
            <option value="rolling">5-hour</option>
            <option value="weekly">Weekly</option>
            <option value="monthly">Monthly</option>
            <option value="worst">Worst window</option>
          </select>
        </Row>
        <Row name="Show label" desc="Prefix the metric (W18%, M7%).">
          <Toggle
            label="Show label"
            selected={app.trayShowLabels}
            onChange={(v) => onUpdate({ trayShowLabels: v })}
          />
        </Row>
        <Row name="Pace marker" desc="Dotted marker and ▲ when the window projects over 100%.">
          <Toggle
            label="Pace marker"
            selected={app.trayShowPace}
            onChange={(v) => onUpdate({ trayShowPace: v })}
          />
        </Row>
        <Row name="Metrics" desc="Windows drawn by the bar styles and used for the title.">
          <div className="chip-row">
            {METRICS.map((metric) => (
              <Check
                key={metric.id}
                label={metric.label}
                checked={app.trayMetrics.includes(metric.id)}
                onChange={(on) => toggleMetric(metric.id, on)}
              />
            ))}
          </div>
        </Row>
      </section>

      <section className="settings-section">
        <p className="section-label">Multiple profiles</p>
        <Row name="Show several profiles" desc="Draw one cell per selected profile.">
          <Toggle
            label="Multi profile"
            selected={app.trayMulti}
            onChange={(v) => onUpdate({ trayMulti: v })}
          />
        </Row>
        {app.trayMulti ? (
          <>
            <Row name="Profiles" desc="Up to five, in the order listed here.">
              <div className="chip-row">
                {state.profiles.map((profile) => (
                  <Check
                    key={profile.id}
                    label={profile.name}
                    checked={app.trayProfiles.includes(profile.id)}
                    onChange={(on) => toggleProfile(profile.id, on)}
                  />
                ))}
              </div>
            </Row>
            <Row name="Active indicator" desc="● marks the active profile.">
              <Toggle
                label="Active indicator"
                selected={app.trayActiveIndicator}
                onChange={(v) => onUpdate({ trayActiveIndicator: v })}
              />
            </Row>
          </>
        ) : null}
      </section>
    </div>
  );
}

// ── automation ─────────────────────────────────────────────────────────────

function AutomationTab({
  app,
  onUpdate,
}: {
  app: AppSettings;
  onUpdate: (p: Partial<AppSettings>) => void;
}) {
  return (
    <div className="settings-body">
      <section className="settings-section">
        <p className="section-label">Keys</p>
        <Row
          name="Auto-switch exhausted keys"
          desc="When a key hits 100%, a non-ok status or a 429, switch to the least-used healthy key."
        >
          <Toggle
            label="Auto-switch"
            selected={app.autoSwitch}
            onChange={(v) => onUpdate({ autoSwitch: v })}
          />
        </Row>
        <p className="setting-note">
          Only keys with quota data are compared. Every switch is written to the profile's
          switch log, so history stays accurate, and you get a notification.
        </p>
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
  const [codex, setCodex] = useState<CodexOut | null>(null);
  const [update, setUpdate] = useState<UpdateOut | null>(null);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    codexStatus()
      .then(setCodex)
      .catch(() => setCodex(null));
  }, []);

  const codexSummary = useMemo(() => {
    if (!codex?.status.present) return "No ~/.codex/auth.json found";
    const parts: string[] = [];
    if (codex.status.hasApiKey) parts.push("API key stored");
    if (codex.status.hasTokens) parts.push("OAuth tokens stored");
    if (codex.status.email) parts.push(codex.status.email);
    if (codex.status.plan) parts.push(codex.status.plan);
    return parts.join(" · ") || "File present";
  }, [codex]);

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
        <p className="section-label">Codex CLI</p>
        <Row name="Credential file" desc={codexSummary}>
          <Button
            size="sm"
            className="disc"
            isDisabled={!codex?.status.hasTokens}
            onPress={() =>
              void codexRefresh()
                .then((next) => {
                  setCodex(next);
                  notify("Codex tokens refreshed", "ok");
                })
                .catch((e) => notify(prettyError(e), "err"))
            }
          >
            Refresh tokens
          </Button>
        </Row>
        <p className="setting-note">
          Keys applied to the Codex provider are patched into <code>~/.codex/auth.json</code>{" "}
          atomically with 0600 permissions; OAuth tokens are preserved.
        </p>
      </section>

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
