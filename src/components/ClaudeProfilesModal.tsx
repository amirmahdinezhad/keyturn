import { useEffect, useLayoutEffect, useState } from "react";
import {
  claudeProfiles,
  createClaudeProfile,
  editClaudeProfile,
  removeWorkspaceAccount,
  prettyError,
  rescanClaudeProfile,
} from "../api";
import type { AppState, ClaudeProfile } from "../types";
import { ProviderLogo } from "../logos";
import { AppModal } from "./AppModal";
import { CopyRow } from "./CliProviders";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { PlanBadge } from "./PlanBadge";
import { ProfileDisplaySettings } from "./ProfileDisplaySettings";
import { isSubscriptionExpired } from "../subscriptions";
import type { ToastKind } from "./Toast";

interface Props {
  initialCreate?: boolean;
  initialName?: string;
  initialProfileId?: string;
  onProfilesChanged?: (profiles: ClaudeProfile[]) => void;
  open: boolean;
  onClose: () => void;
  onCodex: () => void;
  appState: AppState | null;
  onAppChanged: (s: AppState) => void;
  notify: (s: string, kind?: ToastKind) => void;
}

/** Claude profiles follow the CLI's documented config-directory isolation. */
export function ClaudeProfilesModal({
  open,
  onClose,
  onCodex,
  appState,
  onAppChanged,
  notify,
  initialCreate = false,
  initialName = "",
  initialProfileId,
  onProfilesChanged,
}: Props) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!open) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [open]);
  const workspace = appState?.app.workspaces.find((w) => w.id === appState.app.activeWorkspaceId);
  const [profiles, setProfiles] = useState<ClaudeProfile[]>([]);
  const visibleProfiles = profiles.filter((p) => !isSubscriptionExpired(p.subscription, now) && (workspace?.claudeAccountIds == null || workspace.claudeAccountIds.includes(p.id)));
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [view, setView] = useState<"list" | "create" | "edit">("list");
  useLayoutEffect(() => {
    const body = document.getElementById("claude-profiles-body");
    if (body) body.scrollTop = 0;
  }, [view]);
  const [name, setName] = useState("");
  const [current, setCurrent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const selected = profiles.find((p) => p.id === selectedId);
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setBusy(true);
    setError(null);
    setView(initialCreate ? "create" : "list");
    setName(initialName);
    void claudeProfiles()
      .then((p) => {
        if (!cancelled) {
          setProfiles(p);
          const selected = p.find((profile) => profile.id === initialProfileId);
          if (selected && !initialCreate) {
            setSelectedId(selected.id); setName(selected.name); setView("edit"); setConfirmRemove(false);
          }
        }
      })
      .catch((e) => {
        if (!cancelled) setError(prettyError(e));
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, initialCreate, initialName, initialProfileId]);
  const run = async (
    action: () => Promise<ClaudeProfile[]>,
    done?: (p: ClaudeProfile[]) => void,
  ) => {
    setBusy(true);
    setError(null);
    try {
      const next = await action();
      setProfiles(next);
      onProfilesChanged?.(next);
      done?.(next);
    } catch (e) {
      setError(prettyError(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <AppModal
      open={open}
      onClose={onClose}
      busy={busy && view !== "list"}
      hasUnsavedChanges={view === "create" ? !!name || current : view === "edit" && !!selected && name.trim() !== selected.name}
      title="Claude profiles"
      cardClass="profiles-card"
      bodyId="claude-profiles-body"
      onBack={
        view !== "list"
          ? () => {
              setView("list");
              setError(null);
            }
          : undefined
      }
      backLabel="All profiles"
    >
      {error ? (
        <p className="wizard-error" role="alert">
          {error}
        </p>
      ) : null}
      {view === "list" ? (
        <>
          <p className="setting-note">
            Each account has its own Claude Code settings, sign-in and history.
          </p>
          <Button className="profile-provider-link" onClick={onCodex}>
            Codex profiles
          </Button>
          {busy && !profiles.length ? (
            <p role="status">Loading profiles…</p>
          ) : null}
          <div className="profile-list">
            {visibleProfiles.map((p) => (
              <Button
                className="profile-open"
                key={p.id}
                onClick={() => {
                  setSelectedId(p.id);
                  setName(p.name);
                  setConfirmRemove(false);
                  setView("edit");
                }}
              >
                <ProviderLogo provider="claude" />
                <span className="row-main">
                  <strong>{p.name}</strong>
                  <span className="row-sub">
                    {p.email ?? (p.connected ? "Connected" : "Sign-in needed")}
                  </span>
                  <span className="profile-plan">
                    <span>Claude</span>
                    <PlanBadge
                      subscription={
                        p.subscription ?? { id: "claude", service: "Claude" }
                      }
                    />
                  </span>
                </span>
              </Button>
            ))}
          </div>
          {!visibleProfiles.length && !busy ? (
            <p className="profile-empty">
              Add your current Claude account or sign in to another.
            </p>
          ) : null}
          <div className="wizard-foot is-end">
            <Button
              className="primary"
              disabled={busy}
              onClick={() => {
                setName("");
                setCurrent(false);
                setView("create");
              }}
            >
              Create new profile
            </Button>
          </div>
          {appState ? (
            <ProfileDisplaySettings
              allowAutoSwitch={false}
              profiles={visibleProfiles.map((p) => ({
                id: p.id,
                name: p.name,
                active: p.active ?? false,
              }))}
              app={appState.app}
              onChanged={onAppChanged}
              notify={notify}
            />
          ) : null}
        </>
      ) : view === "create" ? (
        <form noValidate
          className="profile-form"
          onSubmit={(e) => {
            e.preventDefault();
            void run(
              () => createClaudeProfile(name, current),
              (next) => {
                const added = next.find(
                  (p) => !profiles.some((old) => old.id === p.id),
                );
                if (added) {
                  setSelectedId(added.id);
                  setName(added.name);
                  setView("edit");
                }
              },
            );
          }}
        >
          <label htmlFor="claude-profile-name">Profile name (optional)</label>
          <input
            id="claude-profile-name"
            autoFocus
            maxLength={64}
            value={name}
            placeholder="Leave empty for a random name"
            onChange={(e) => setName(e.target.value)}
            disabled={busy}
          />
          <label className="profile-current-choice">
            <Checkbox
              aria-label="Track my current Claude account"
              checked={current}
              onCheckedChange={(checked) => setCurrent(checked === true)}
              disabled={busy}
            />
            Track my current Claude account
          </label>
          <p className="setting-note">
            {current
              ? "Keep using the CLI's current configuration and login."
              : "Create an independent configuration, then sign in with Claude Code in Terminal."}
          </p>
          <div className="wizard-foot is-end">
            <Button className="primary" type="submit" disabled={busy}>
              {busy ? "Creating…" : "Create profile"}
            </Button>
          </div>
        </form>
      ) : selected ? (
        <>
          <form noValidate
            className="profile-form"
            onSubmit={(e) => {
              e.preventDefault();
              void run(
                () => editClaudeProfile(selected.id, name),
                () => notify("Profile renamed", "ok"),
              );
            }}
          >
            <label htmlFor="claude-profile-rename">Profile name</label>
            <input
              id="claude-profile-rename"
              maxLength={64}
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={busy}
            />
            <Button
              type="submit"
              disabled={busy || !name.trim() || name.trim() === selected.name}
            >
              Save name
            </Button>
          </form>
          <p className="setting-note">
            Created {new Date(selected.createdAt).toLocaleDateString()}.{" "}
            {selected.connected
              ? `Connected${selected.email ? ` as ${selected.email}` : ""}.`
              : "Sign in, then check the connection."}
          </p>
          <CopyRow
            label="Sign in to this profile"
            text={selected.loginCommand}
            copiedMessage="Sign-in command copied"
            notify={notify}
          />
          <CopyRow
            label="Use this account"
            text={selected.launchCommand}
            copiedMessage="Profile command copied"
            notify={notify}
          />
          <p className="setting-note">
            Start Claude with this command to use this account. Existing
            terminals keep their own configuration.
          </p>
          <Button
            className="primary"
            disabled={busy}
            onClick={() =>
              void run(
                () => rescanClaudeProfile(selected.id),
                (p) =>
                  notify(
                    p.find((v) => v.id === selected.id)?.connected
                      ? "Profile connected"
                      : "Complete sign-in in Terminal, then rescan.",
                  ),
              )
            }
          >
            {busy ? "Checking…" : "Test connection / Rescan"}
          </Button>
          {selected.quota?.windows ? (
            <div className="claude-profile-usage">
              {selected.quota.windows.map((w) => (
                <div key={w.window}>
                  <span>{w.window}</span>
                  <strong>{Math.round(w.percent)}% used</strong>
                </div>
              ))}
            </div>
          ) : (
            <p className="setting-note">
              Usage appears after Claude records a usage snapshot in this
              profile.
            </p>
          )}
          <div className="profile-remove">
            {confirmRemove ? (
              <>
                <p className="setting-note">
                  Forget this profile? Its Claude login and session history stay
                  in the configuration directory.
                </p>
                {workspace ? [false, true].map((everywhere) => <Button key={String(everywhere)} variant="destructive" disabled={busy} onClick={() => {
                  setBusy(true); setError(null);
                  void removeWorkspaceAccount(workspace.id, "claude", selected.id, everywhere).then((state) => {
                    onAppChanged(state); setView("list");
                    return claudeProfiles().then((profiles) => { setProfiles(profiles); onProfilesChanged?.(profiles); });
                  }).catch((e) => setError(prettyError(e))).finally(() => setBusy(false));
                }}>{everywhere ? "Forget in all workspaces" : "Remove from this workspace"}</Button>) : <Button className="danger" disabled={busy} onClick={() => void run(() => editClaudeProfile(selected.id, null), () => setView("list"))}>Forget profile</Button>}
              </>
            ) : (
              <Button
                className="link-btn danger"
                disabled={busy}
                onClick={() => setConfirmRemove(true)}
              >
                Remove profile
              </Button>
            )}
          </div>
        </>
      ) : null}
    </AppModal>
  );
}
