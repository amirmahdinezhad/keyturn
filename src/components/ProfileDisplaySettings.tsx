import { useState } from "react";
import { prettyError, setAppSettings } from "../api";
import type {
  AppSettings,
  AppState,
  CodexProfile,
  MultiProfileSettings,
} from "../types";
import { Switch } from "./ui/switch";
import { Checkbox } from "./ui/checkbox";
import type { ToastKind } from "./Toast";

interface Props {
  profiles: Pick<CodexProfile, "id" | "name" | "active">[];
  app: AppSettings;
  allowAutoSwitch?: boolean;
  onChanged: (state: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
}

/** Preferences for percent-only multi-account rendering and native auto-switch. */
export function ProfileDisplaySettings({
  profiles,
  app,
  onChanged,
  notify,
  allowAutoSwitch = true,
}: Props) {
  const [busy, setBusy] = useState(false);
  const save = async (settings: Partial<AppSettings>) => {
    if (busy) return;
    setBusy(true);
    try {
      onChanged(await setAppSettings(settings));
    } catch (e) {
      notify(prettyError(e), "err");
    } finally {
      setBusy(false);
    }
  };
  const multi = app.multiProfile;
  const update = (settings: Partial<MultiProfileSettings>) =>
    void save({ multiProfile: { ...multi, ...settings } });
  const choices: Array<{
    key: keyof Pick<
      MultiProfileSettings,
      | "showWeekly"
      | "showProfileLabel"
      | "monochrome"
      | "showRemaining"
      | "showActiveIndicator"
    >;
    name: string;
    desc: string;
  }> = [
    {
      key: "showWeekly",
      name: "Show week usage",
      desc: "Include the weekly limit beside each session",
    },
    {
      key: "showProfileLabel",
      name: "Show profile label",
      desc: "Show the account name below its percentages",
    },
    {
      key: "monochrome",
      name: "Monochrome profiles",
      desc: "Use the selected accent for percentages",
    },
    {
      key: "showRemaining",
      name: "Show remaining capacity",
      desc: "Show capacity left instead of usage",
    },
    {
      key: "showActiveIndicator",
      name: "Show active profile indicator",
      desc: "Underline the currently active account",
    },
  ];
  return (
    <div className="profile-preferences">
      <section aria-label="Multi-profile display" className="settings-section">
        <h3 className="settings-subtitle">Multi-profile display</h3>
        <div className="settings-group">
          <div className="setting-row">
            <div className="setting-text">
              <span className="setting-name">Show multiple profiles</span>
              <span className="setting-desc">
                Display selected accounts side by side
              </span>
            </div>
            <Switch
              aria-label="Show multiple profiles"
              checked={multi.enabled}
              disabled={busy}
              onCheckedChange={(enabled) => update({ enabled })}
            />
          </div>
          {multi.enabled ? (
            <>
              <fieldset className="profile-visibility" disabled={busy}>
                <legend>Profiles in the menu bar</legend>
                {profiles.length ? (
                  profiles.map((p) => (
                    <label key={p.id}>
                      <Checkbox
                        aria-label={p.name}
                        disabled={busy}
                        checked={multi.selectedIds.includes(p.id)}
                        onCheckedChange={(checked) =>
                          update({
                            selectedIds: checked === true
                              ? [...multi.selectedIds, p.id]
                              : multi.selectedIds.filter((id) => id !== p.id),
                          })
                        }
                      />
                      <span>{p.name}</span>
                      {p.active ? (
                        <span className="profile-active">Active</span>
                      ) : null}
                    </label>
                  ))
                ) : (
                  <p className="setting-note">
                    Create a profile to display its usage here.
                  </p>
                )}
              </fieldset>
              <div className="fixed-icon-style">
                <span>Icon style</span>
                <strong>Percentage</strong>
              </div>
              {choices.map(({ key, name, desc }) => (
                <div className="setting-row" key={key}>
                  <div className="setting-text">
                    <span className="setting-name">{name}</span>
                    <span className="setting-desc">{desc}</span>
                  </div>
                  <Switch
                    aria-label={name}
                    checked={multi[key]}
                    disabled={busy}
                    onCheckedChange={(v) => update({ [key]: v })}
                  />
                </div>
              ))}
              {!multi.selectedIds.some((id) =>
                profiles.some((p) => p.id === id),
              ) ? (
                <p className="setting-note">
                  Select at least one profile to show percentages.
                </p>
              ) : null}
            </>
          ) : null}
        </div>
      </section>
      {allowAutoSwitch ? (
        <section aria-label="Auto-switch profile" className="settings-section">
          <h3 className="settings-subtitle">Auto-switch profile</h3>
          <div className="settings-group">
            <div className="setting-row">
              <div className="setting-text">
                <span className="setting-name">Enable auto-switch</span>
                <span className="setting-desc">
                  At 100% session usage, choose the next available Codex account
                </span>
              </div>
              <Switch
                aria-label="Enable auto-switch"
                checked={app.autoSwitchEnabled}
                disabled={busy}
                onCheckedChange={(enabled) =>
                  void save({ autoSwitchEnabled: enabled })
                }
              />
            </div>
          </div>
          {app.autoSwitchEnabled ? (
            <p className="setting-note">
              Starts a cancellable 30-second Codex restart countdown. Accounts
              with unavailable or exhausted limits are skipped. Save your work
              before enabling this.
            </p>
          ) : null}
        </section>
      ) : null}
    </div>
  );
}
