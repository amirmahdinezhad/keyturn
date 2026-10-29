import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  activateCodexProfile, codexProfiles, createCodexProfile, prettyError,
  removeCodexProfile, renameCodexProfile,
  importCodexProfile, startCodexProfileLogin, codexProfileLoginStatus, cancelCodexProfileLogin,
  scheduleCodexRestart, codexRestartStatus, cancelCodexRestart, runCodexRestartNow,
  setCodexAccountPreferences,
} from "../api";
import type { CodexProfile, CodexProfilesState, CodexRestartStatus, AppState } from "../types";
import { Button } from "./ui/button";
import { RadioGroup, RadioGroupItem } from "./ui/radio-group";
import { AppModal } from "./AppModal";
import { CopyRow } from "./CliProviders";
import { ProviderLogo } from "../logos";
import { AddIcon, NextIcon, TickIcon } from "../icons";
import type { ToastKind } from "./Toast";
import { PlanBadge } from "./PlanBadge";
import { isSubscriptionExpired } from "../subscriptions";

import { CodexAccountPreferences } from "./CodexAccountPreferences";
import { CodexDisplaySettings } from "./CodexDisplaySettings";
import { DEFAULT_CODEX_ACCOUNT_PREFERENCES } from "../types";
import { codexAccountPreferences } from "../accountPrefs";
import { Switch } from "./ui/switch";

interface Props {
  onClaudeProfiles?: (create?: boolean, name?: string) => void;
  appState?: AppState | null;
  onAppChanged?: (state: AppState) => void;
  open: boolean;
  onClose: () => void;
  onChanged: (state: CodexProfilesState) => void;
  onActivated: () => Promise<unknown>;
  notify: (message: string, kind?: ToastKind) => void;
}

type View = "list" | "create" | "edit" | "login" | "import";

/** Manage named Codex logins without exposing tokens to the webview. */
export function CodexProfilesModal({ open, onClose, onChanged, onActivated, notify, appState, onAppChanged, onClaudeProfiles }: Props) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!open) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [open]);
  const [data, setData] = useState<CodexProfilesState | null>(null);
  const [view, setView] = useState<View>("list");
  useLayoutEffect(() => {
    const body = document.getElementById("codex-profiles-body");
    if (body) body.scrollTop = 0;
  }, [view]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [filePath, setFilePath] = useState("");
  const [source, setSource] = useState<"current" | "browser" | "file">("browser");
  const [joinCycle, setJoinCycle] = useState(false);
  const [signingInId, setSigningInId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [switched, setSwitched] = useState(false);
  const [restart, setRestart] = useState<CodexRestartStatus | null>(null);
  const activeRestart = !!restart && ["countdown", "quitting", "activating", "reopening"].includes(restart.phase);
  const completedRestart = useRef<string | null>(null);
  const restartGeneration = useRef(0);
  const generation = useRef(0);
  const changedRef = useRef(onChanged);
  changedRef.current = onChanged;
  const activatedRef = useRef(onActivated);
  activatedRef.current = onActivated;
  const notifyRef = useRef(notify);
  notifyRef.current = notify;

  useEffect(() => {
    const ticket = ++generation.current;
    if (!open) return;
    setView("list"); setData(null); setError(null); setBusy(true); setSwitched(false); setSigningInId(null); setFilePath("");
    void codexProfiles().then((fresh) => {
      if (generation.current !== ticket) return;
      setData(fresh); changedRef.current(fresh);
    }).catch((e) => {
      if (generation.current === ticket) setError(prettyError(e));
    }).finally(() => {
      if (generation.current === ticket) setBusy(false);
    });
    return () => { generation.current++; };
  }, [open]);

  useEffect(() => {
    if (!open || !signingInId) return;
    let stopped = false;
    let timer = 0;
    const poll = async () => {
      try {
        const status = await codexProfileLoginStatus(signingInId);
        if (stopped) return;
        if (status.status === "pending") { timer = window.setTimeout(() => void poll(), 1000); return; }
        if (status.status !== "completed") { setSigningInId(null); setError(status.error ?? "Sign-in ended. Try again."); return; }
        const fresh = await codexProfiles();
        if (stopped) return;
        setData(fresh); changedRef.current(fresh);
        const profile = fresh.profiles.find((p) => p.id === signingInId);
        if (profile?.connected) { if (appState && onAppChanged) { setSelectedId(profile.id); setName(profile.name); setView("edit"); } else setView("list"); setError(null); notify("Profile connected", "ok"); }
        else setError(profile?.connectionError ?? "No ChatGPT login was saved. Try again or use the Terminal instructions.");
        setSigningInId(null);
      } catch (e) { if (!stopped) { setSigningInId(null); setError(prettyError(e)); } }
    };
    timer = window.setTimeout(() => void poll(), 500);
    return () => {
      stopped = true; window.clearTimeout(timer);
      void cancelCodexProfileLogin(signingInId).catch(() => {});
    };
  }, [open, signingInId, notify]);

  useEffect(() => {
    if (!open && !activeRestart) return;
    let stopped = false;
    let timer = 0;
    const poll = async () => {
      try {
        const ticket = restartGeneration.current;
        const status = await codexRestartStatus();
        if (stopped) return;
        if (ticket !== restartGeneration.current) {
          timer = window.setTimeout(() => void poll(), 1000); return;
        }
        if (status?.phase === "verified" && completedRestart.current !== status.deadlineAt) {
          const fresh = await codexProfiles();
          if (stopped || ticket !== restartGeneration.current) return;
          setData(fresh); changedRef.current(fresh);
          try { await activatedRef.current(); }
          catch { if (!stopped) setError("Codex restarted, but CLI status could not refresh. Use Refresh data."); }
          if (stopped) return;
          completedRestart.current = status.deadlineAt;
          notifyRef.current(`${status.profileName} login installed. Check the account in Codex.`, "ok");
        }
        if (status?.phase === "failed" && completedRestart.current !== status.deadlineAt) {
          completedRestart.current = status.deadlineAt;
          notifyRef.current(status.error ?? "Codex restart failed. Open profiles to retry.", "err");
        }
        // Finish refresh before the terminal phase tears down this effect.
        setRestart(status);
      } catch (e) { if (!stopped) setError(prettyError(e)); }
      if (!stopped) timer = window.setTimeout(() => void poll(), 1000);
    };
    void poll();
    return () => { stopped = true; window.clearTimeout(timer); };
  }, [open, activeRestart]);

  const selected = data?.profiles.find((p) => p.id === selectedId);
  const visibleProfiles = data?.profiles.filter((p) => !isSubscriptionExpired(p.subscription, now)) ?? [];
  const back = () => { setView("list"); setFilePath(""); setError(null); setConfirmRemove(false); };
  const run = async (action: () => Promise<CodexProfilesState | null>, success?: (fresh: CodexProfilesState) => Promise<void> | void) => {
    const ticket = generation.current;
    setBusy(true); setError(null);
    try {
      const fresh = await action();
      if (ticket !== generation.current || !fresh) return;
      setData(fresh); onChanged(fresh);
      await success?.(fresh);
    } catch (e) {
      if (ticket === generation.current) setError(prettyError(e));
    } finally {
      if (ticket === generation.current) setBusy(false);
    }
  };

  const beginLogin = async (id: string) => {
    const ticket = generation.current;
    setBusy(true); setError(null);
    try {
      const status = await startCodexProfileLogin(id);
      if (ticket !== generation.current) { await cancelCodexProfileLogin(id); return; }
      if (status.status === "failed") setError(status.error ?? "Could not start sign-in.");
      else setSigningInId(id);
    } catch (e) { if (ticket === generation.current) setError(prettyError(e)); }
    finally { if (ticket === generation.current) setBusy(false); }
  };

  const edit = (profile: CodexProfile) => {
    setSelectedId(profile.id); setName(profile.name); setConfirmRemove(false);
    setError(null); setView(profile.connected ? "edit" : "login");
  };

  const configureAdded = (profile: CodexProfile) => {
    if (appState && onAppChanged) { setSelectedId(profile.id); setName(profile.name); setView("edit"); setError(null); }
    else back();
  };

  const activate = (profile: CodexProfile) => void run(() => activateCodexProfile(profile.id), async () => {
    setSwitched(true);
    notify(`${profile.name} credentials activated`, "ok");
    // Close old-account quota content before reading fresh provider identity.
    try { await onActivated(); } catch {
      setError("The account switched, but status could not refresh. Use Refresh data.");
    }
  });

  const schedule = async (profile: CodexProfile) => {
    restartGeneration.current++;
    setRestart(await scheduleCodexRestart(profile.id));
    notify(`Close Codex now. Keyturn will reopen it with ${profile.name} after the countdown.`, "ok");
  };
  const replaceFromFile = () => { setFilePath(""); setError(null); setView("import"); };
  const restartPanel = restart && restart.phase !== "cancelled" ? (
    <div className="profile-switch-note" role="status">
      {restart.phase === "countdown" ? <>
        <strong>{restart.profileName} is ready. Save your work and close Codex.</strong>
        <p>Keyturn will replace its login and reopen it after it closes. If left open, it will be restarted in {Math.max(0, Math.ceil((Date.parse(restart.deadlineAt) - Date.now()) / 1000))} seconds.</p>
        <div className="profile-remove-actions">
          <Button onClick={() => { restartGeneration.current++; void cancelCodexRestart().then(setRestart).catch((e) => setError(prettyError(e))); }}>Cancel restart</Button>
          <Button className="primary" onClick={() => { restartGeneration.current++; void runCodexRestartNow().then(setRestart).catch((e) => setError(prettyError(e))); }}>Restart now</Button>
        </div>
      </> : restart.phase === "verified" ? <>
        <strong>Codex reopened with {restart.profileName} credentials.</strong>
        <p>Check the active account in Codex.</p>
      </> : restart.phase === "failed" ? <p>{restart.error}</p> : <p>Switching to {restart.profileName}… {restart.phase}</p>}
    </div>
  ) : null;

  return (<>
    {!open && activeRestart ? <div className="profile-restart-floating">{restartPanel}</div> : null}
    <AppModal open={open} onClose={busy ? () => {} : onClose} title="Codex profiles"
      cardClass="profiles-card" bodyId="codex-profiles-body" onBack={view !== "list" && !busy && !signingInId ? back : undefined} backLabel="All profiles">
      {data?.switchError ? <p className="wizard-error" role="alert">{data.switchError}</p> : null}
      {restartPanel}
      {switched ? (
        <p className="profile-switch-note" role="status">
          Account credentials updated. Reopen Codex to load the selected account.
          New CLI sessions use it immediately. Desktop versions with a separate sign-in must be checked in the app.
        </p>
      ) : null}
      {error ? <p className="wizard-error" role="alert">{error}</p> : null}
      {!data ? (
        busy ? <p className="setting-note" role="status">Loading profiles…</p> :
          <Button onClick={() => void run(codexProfiles)}>Try again</Button>
      ) : view === "list" ? (
        <>
          {onClaudeProfiles ? <Button className="profile-provider-link" onClick={() => onClaudeProfiles()}>Anthropic profiles</Button> : null}
          <p className="setting-note profile-intro">Import a work login file or add personal ChatGPT accounts.</p>
          <p className="profile-desktop-note">Set active starts a 30-second Codex restart countdown. Save your work before switching.</p>
          {visibleProfiles.length ? (
            <div className="profile-list" aria-label="Saved profiles">
              {visibleProfiles.map((profile) => (
                <div key={profile.id} className={`profile-row${profile.active ? " is-active" : ""}`}>
                  <Button variant="ghost" className="profile-open" aria-label={`Manage ${profile.name}`} onClick={() => edit(profile)} disabled={busy || activeRestart}>
                    <ProviderLogo provider="codex" />
                    <span className="row-main">
                      <span className="row-title">{profile.name}</span>
                      <span className="row-sub">{profile.email ?? (profile.connected ? "ChatGPT account" : "Sign-in needed")}{profile.needsActivation ? " · Ready to activate" : ""}{profile.createdAt ? ` · Created ${new Date(profile.createdAt).toLocaleDateString()}` : ""}</span>
                      {appState && codexAccountPreferences(appState.app, profile.id).autoSwitch ? <span className="row-sub">In switch cycle</span> : null}
                      {profile.connected ? <span className="profile-plan"><span>ChatGPT</span><PlanBadge subscription={profile.subscription ?? { id: "chatgpt", service: "ChatGPT", plan: profile.plan }} /></span> : null}
                    </span>
                  </Button>
                  {profile.active ? <span className="profile-active"><TickIcon size={14} />Active</span> : profile.connected ? (
                    <Button size="sm" disabled={busy || activeRestart || !!data.switchError} onClick={() => void run(async () => { await schedule(profile); return null; })} aria-label={`Set ${profile.name} active`}>Set active</Button>
                  ) : <Button size="icon-sm" disabled={busy} aria-label={`Sign in to ${profile.name}`} onClick={() => edit(profile)}><NextIcon size={16} /></Button>}
                </div>
              ))}
            </div>
          ) : <p className="profile-empty">{data.profiles.length ? "No active subscriptions. Renew a saved account or add another profile." : "No saved profiles yet. Start with your current account or sign in to another."}</p>}
          {data.canImport && !data.activeProfileId ? <p className="setting-note">Your current Codex account has not been saved as a profile.</p> : null}
          <div className="wizard-foot is-end">
            <Button className="primary" disabled={busy || activeRestart} onClick={() => {
              setName(""); setJoinCycle(false); setSource(data.canImport && !data.activeProfileId ? "current" : "browser");
              setError(null); setView("create");
            }}><AddIcon size={16} />Add profile</Button>
          </div>
          {appState && onAppChanged ? <CodexDisplaySettings app={appState.app} onChanged={onAppChanged} notify={notify} /> : null}
        </>
      ) : view === "create" ? (
        <form noValidate className="profile-form" onSubmit={(event) => {
          event.preventDefault();
          if (source === "file" && !filePath.trim()) return;
          void run(() => source === "file" ? importCodexProfile(name.trim(), filePath.trim()) : createCodexProfile(name.trim(), source === "current"), async (fresh) => {
            const added = fresh.profiles.find((p) => !data.profiles.some((old) => old.id === p.id));
            if (added && joinCycle && appState && onAppChanged) {
              onAppChanged(await setCodexAccountPreferences(added.id, { ...DEFAULT_CODEX_ACCOUNT_PREFERENCES, autoSwitch: true }, visibleProfiles.length === 1 ? visibleProfiles[0].id : null));
            }
            if (source === "current") { if (added) configureAdded(added); else back(); notify("Current account saved", "ok"); }
            else if (source === "file" && added) { configureAdded(added); await schedule(added); }
            else if (added) { setSelectedId(added.id); setView("login"); await beginLogin(added.id); }
          });
        }}>
          <label htmlFor="profile-name">Profile name</label>
          <p className="setting-note">Optional. Leave empty for a random name.</p>
          <input id="profile-name" value={name} maxLength={64} autoFocus disabled={busy}
            placeholder="Personal or Work" onChange={(e) => setName(e.target.value)} />
          {onClaudeProfiles ? <div className="profile-provider-choices" role="group" aria-label="Provider">
            <Button aria-pressed="true"><ProviderLogo provider="codex" />Codex</Button>
            <Button onClick={() => onClaudeProfiles(true, name)} disabled={busy}><ProviderLogo provider="claude" />Anthropic</Button>
          </div> : null}
          <fieldset className="profile-source" disabled={busy}>
            <legend>Account</legend>
            <RadioGroup aria-label="Account source" value={source} disabled={busy} onValueChange={(value) => setSource(value as typeof source)}>
              <label><RadioGroupItem value="current" aria-label="Save current Codex account" disabled={busy || !data.canImport || !!data.activeProfileId} />Save current Codex account</label>
              <label><RadioGroupItem value="browser" aria-label="Sign in to another account" />Sign in to another account</label>
              <label><RadioGroupItem value="file" aria-label="Import auth.json file" />Import auth.json file</label>
            </RadioGroup>
          </fieldset>
          {source === "file" ? <>
            <label htmlFor="profile-file-path">auth.json path</label>
            <input id="profile-file-path" value={filePath} disabled={busy} spellCheck={false} autoComplete="off" placeholder="~/Downloads/auth.json" onChange={(e) => setFilePath(e.target.value)} />
          </> : null}
          {visibleProfiles.length > 0 && appState && onAppChanged ? <div className="setting-row"><div className="setting-text"><span className="setting-name">{visibleProfiles.length === 1 ? `Switch automatically between ${visibleProfiles[0].name} and this account` : "Include this account in switch cycle"}</span><span className="setting-desc">Codex accounts only. You can change membership later in Manage account.</span></div><Switch aria-label="Join automatic switch cycle" checked={joinCycle} disabled={busy} onCheckedChange={setJoinCycle} /></div> : null}
          <p className="setting-note">{source === "file" ? "Paste the full path to auth.json. In Finder, use Option–Command–C to copy a file's path. After import, a 30-second restart countdown lets you save your work or cancel." : source === "current" ? "Save the current Codex login with this name. No browser sign-in or account switch is needed." : "Keyturn opens Codex's browser sign-in in a separate profile. Your active account stays unchanged until Set active."}</p>
          <div className="wizard-foot is-end"><Button className="primary" type="submit" disabled={busy || (source === "file" && !filePath.trim())}>
            {busy ? "Saving…" : source === "current" ? "Save profile" : source === "file" ? "Import profile" : "Continue with browser"}
          </Button></div>
        </form>
      ) : selected ? (
        <>
          {view === "import" ? (
            <form noValidate className="profile-form" onSubmit={(event) => {
              event.preventDefault();
              if (!filePath.trim()) return;
              void run(() => importCodexProfile(selected.name, filePath.trim(), selected.id), async (fresh) => {
                back();
                const profile = fresh.profiles.find((p) => p.id === selected.id);
                if (profile) await schedule(profile);
              });
            }}>
              <label htmlFor="replace-file-path">auth.json path</label>
              <input id="replace-file-path" value={filePath} autoFocus disabled={busy} spellCheck={false} autoComplete="off" placeholder="~/Downloads/auth.json" onChange={(e) => setFilePath(e.target.value)} />
              <p className="setting-note">Paste the full path to the login file for {selected.name}. In Finder, use Option–Command–C to copy its path.</p>
              <p className="setting-note">After import, a 30-second restart countdown lets you save your work or cancel.</p>
              <div className="wizard-foot is-end"><Button className="primary" type="submit" disabled={busy || !filePath.trim()}>{busy ? "Importing…" : "Import profile"}</Button></div>
            </form>
          ) : view === "edit" ? (
            <>
            <form noValidate className="profile-form" onSubmit={(e) => {
              e.preventDefault(); void run(() => renameCodexProfile(selected.id, name.trim()), () => { back(); notify("Profile renamed", "ok"); });
            }}>
              <label htmlFor="profile-rename">Profile name</label>
              <input id="profile-rename" value={name} maxLength={64} disabled={busy} onChange={(e) => setName(e.target.value)} />
              <p className="setting-note">{selected.email}</p>
              {isSubscriptionExpired(selected.subscription, now) ? <p className="setting-note">Subscription expired. Sign in again after renewing.</p> : <span className="profile-plan"><span>ChatGPT</span><PlanBadge subscription={selected.subscription ?? { id: "chatgpt", service: "ChatGPT", plan: selected.plan }} /></span>}
              <div className="wizard-foot is-end"><Button type="submit" className="primary" disabled={busy || !name.trim() || name.trim() === selected.name}>Save name</Button></div>
              <Button disabled={busy || (selected.active && !isSubscriptionExpired(selected.subscription, now))} onClick={() => { setView("login"); setError(null); }}>Sign in again</Button>
              <Button disabled={busy || activeRestart} onClick={replaceFromFile}>Replace login from file</Button>
              <Button disabled={busy || activeRestart || !!data.switchError || isSubscriptionExpired(selected.subscription, now)} onClick={() => void run(async () => { await schedule(selected); return null; })}>Activate and restart Codex</Button>
              <Button disabled={busy || activeRestart || !!data.switchError || isSubscriptionExpired(selected.subscription, now)} onClick={() => activate(selected)}>Activate for CLI only</Button>
              {selected.active ? <p className="setting-note">This account is active. Add a new profile to sign in with another account.</p> : null}
            </form>
            {appState && onAppChanged ? <CodexAccountPreferences key={selected.id} profile={selected} profiles={visibleProfiles} app={appState.app} onChanged={onAppChanged} notify={notify} /> : null}
            </>
          ) : (
            <div className="profile-login">
              <p className="row-title">{selected.name}</p>
              {selected.connectionError ? <p className="wizard-error" role="alert">{selected.connectionError}</p> : null}
              {signingInId ? <>
                <p className="setting-note" role="status">Finish signing in with your intended ChatGPT account in the browser, then reopen Keyturn from the menu bar.</p>
                <Button disabled={busy} onClick={async () => {
                  try { await cancelCodexProfileLogin(signingInId); setSigningInId(null); }
                  catch (e) { setError(prettyError(e)); }
                }}>Cancel sign-in</Button>
              </> : <Button className="primary" disabled={busy} onClick={() => void beginLogin(selected.id)}>Sign in with browser</Button>}
              <details className="setup-help" open={!!error}>
                <summary>Use Terminal instead</summary>
              <ol className="setup-instructions">
                <li>Run this command in Terminal.</li>
                <li>Choose the ChatGPT account for this profile in the browser. Use a different account if your browser signs in automatically.</li>
                <li>Return here and check the connection. Your active Codex account stays unchanged until you select Set active.</li>
              </ol>
              <CopyRow label="Sign in" text={selected.loginCommand} copiedMessage="Profile login command copied" notify={notify} />
              <CopyRow label="Device code fallback" text={`${selected.loginCommand} --device-auth`} copiedMessage="Device login command copied" notify={notify} />
              <div className="wizard-foot is-end"><Button className="primary" disabled={busy || !!signingInId} onClick={() => void run(codexProfiles, (fresh) => {
                const profile = fresh.profiles.find((p) => p.id === selected.id);
                if (profile?.connected) { back(); notify("Profile connected", "ok"); }
                else if (!profile?.connectionError) setError("No saved ChatGPT login found. Complete sign-in in Terminal, then check again.");
              })}>{busy ? "Checking…" : "Check connection"}</Button></div>
              </details>
              {!signingInId ? <Button disabled={busy || activeRestart} onClick={replaceFromFile}>Import auth.json instead</Button> : null}
            </div>
          )}
          {!signingInId ? <div className="profile-remove">
            {confirmRemove ? <>
              <p className="setting-note">Remove “{selected.name}” and its saved login? The active Codex account stays signed in.</p>
              <div className="profile-remove-actions">
                <Button disabled={busy} onClick={() => setConfirmRemove(false)}>Keep profile</Button>
                <Button className="danger" disabled={busy} onClick={() => void run(() => removeCodexProfile(selected.id), () => { back(); notify("Profile removed", "ok"); })}>Remove profile</Button>
              </div>
            </> : <Button className="link-btn" disabled={busy} onClick={() => setConfirmRemove(true)}>Remove profile</Button>}
          </div> : null}
        </>
      ) : null}
    </AppModal></>
  );
}
