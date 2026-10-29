import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { beginAccountSetup, discardAccountSetup, loadSetupAccounts, startSetupLogin, pollSetupLogin, cancelSetupLogin, submitSetupCode, importSetupAccount, connectSetupConfig, connectCurrentGoogle, commitAccountSetup, prettyError, type AccountProvider, type SetupAccount, type SetupLogin } from "../api";
import { AuthFileInput, type AuthFileSelection } from "./AuthFileInput";
import { ProviderLogo } from "../logos";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { AppModal } from "./AppModal";
import { PlanBadge } from "./PlanBadge";
import { KeyIcon, NextIcon, PrevIcon, RefreshIcon, SpinnerIcon } from "../icons";

export const PROVIDER_LABELS: Record<AccountProvider, string> = { codex: "ChatGPT / Codex", claude: "Claude", antigravity: "Antigravity" };

/** One native staging session is shared by discovery and all new-account methods. */
export function useAccountEnrollment() {
  const session = useRef<Promise<string> | null>(null);
  const mounted = useRef(false);
  const [accounts, setAccounts] = useState<SetupAccount[]>([]);
  const [manualIds, setManualIds] = useState<string[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [login, setLogin] = useState<SetupLogin | null>(null);
  const [error, setError] = useState<string | null>(null);
  const loadedIds = useRef<string[]>([]);
  const [revision, setRevision] = useState(0);
  const [editing, setEditing] = useState(false);
  const getSession = useCallback(() => {
    if (!session.current) session.current = beginAccountSetup().catch((error) => { session.current = null; throw error; });
    return session.current;
  }, []);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      // StrictMode may immediately remount; an actual leave discards the native draft.
      queueMicrotask(() => { if (!mounted.current && session.current) void session.current.then(discardAccountSetup).catch(() => {}); });
    };
  }, []);
  const add = useCallback((account: SetupAccount) => {
    setAccounts((previous) => [...previous.filter((old) => old.id !== account.id), account]);
    setSelected((previous) => [...new Set([...previous, account.id])]);
    setManualIds((previous) => [...new Set([...previous, account.id])]);
    setEditing(false); setRevision((value) => value + 1);
  }, []);
  useEffect(() => {
    if (!login || login.status !== "pending") return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const fresh = await pollSetupLogin(await getSession(), login.attemptId);
        if (!active) return;
        setLogin(fresh);
        if (fresh.account) add(fresh.account);
        if (fresh.error) setError(fresh.error);
        if (fresh.status === "pending") timer = setTimeout(poll, 1000);
      } catch (error) { if (active) { setLogin(null); setError(prettyError(error)); } }
    };
    timer = setTimeout(poll, 500);
    return () => { active = false; clearTimeout(timer); };
  }, [login?.attemptId, login?.status, getSession, add]);
  const load = async () => {
    if (loading || busy) return;
    setLoading(true); setError(null);
    try {
      const fresh = await loadSetupAccounts(await getSession());
      if (!mounted.current) return;
      const previousLoaded = loadedIds.current;
      setAccounts((previous) => [...previous.filter((old) => !previousLoaded.includes(old.id) && !fresh.some((next) => next.id === old.id)), ...fresh]);
      setSelected((previous) => previous.filter((id) => !previousLoaded.includes(id) || fresh.some((next) => next.id === id)));
      loadedIds.current = fresh.map((account) => account.id);
      setLoaded(true);
    } catch (error) { if (mounted.current) setError(prettyError(error)); }
    finally { if (mounted.current) setLoading(false); }
  };
  const run = async (action: (session: string) => Promise<SetupAccount>) => {
    if (busy || login?.status === "pending") return;
    setBusy(true); setError(null);
    try { const account = await action(await getSession()); if (mounted.current) { setLogin(null); add(account); } }
    catch (error) { if (mounted.current) setError(prettyError(error)); }
    finally { if (mounted.current) setBusy(false); }
  };
  const start = async (provider: AccountProvider, method: string, name: string) => {
    if (busy || login?.status === "pending") return;
    setBusy(true); setError(null);
    try {
      const login = await startSetupLogin(await getSession(), provider, method, name);
      if (!mounted.current) return;
      setLogin(login);
      if (login.authUrl) void openUrl(login.authUrl).catch(() => setError("Couldn’t open your browser. Use Open sign-in page below."));
    } catch (error) { if (mounted.current) setError(prettyError(error)); }
    finally { if (mounted.current) setBusy(false); }
  };
  const cancel = async () => {
    setBusy(true);
    try { await cancelSetupLogin(await getSession()); setLogin(null); setError(null); }
    catch (error) { setError(prettyError(error)); }
    finally { setBusy(false); }
  };
  const commit = async (name: string, accent: string, workspaceId?: string) => {
    setBusy(true); setError(null);
    try { return await commitAccountSetup(await getSession(), name, accent, selected, workspaceId ?? null); }
    catch (error) { setError(prettyError(error)); return null; }
    finally { if (mounted.current) setBusy(false); }
  };
  return { revision, accounts, manualAccounts: accounts.filter((account) => manualIds.includes(account.id)), selected, loaded, loading, busy, login, error, editing, setEditing, setError, getSession, add, load, run, start, cancel, commit,
    pending: login?.status === "pending",
    toggle: (id: string) => setSelected((previous) => previous.includes(id) ? previous.filter((old) => old !== id) : [...previous, id]),
  };
}
export type Enrollment = ReturnType<typeof useAccountEnrollment>;

export function EnrollmentAccounts({ draft }: { draft: Enrollment }) {
  return <div className="setup-account-content" aria-busy={draft.loading}>
    <div className="setup-selection-summary"><span>Available accounts</span><div className="setup-selection-tools"><span>{draft.selected.length} selected</span><Button size="icon-sm" variant="ghost" aria-label="Load again" title="Load again" aria-busy={draft.loading} disabled={draft.loading || draft.busy} onClick={() => void draft.load()}>{draft.loading ? <SpinnerIcon size={14} /> : <RefreshIcon size={14} />}</Button></div></div>
    {draft.loading ? <div className="setup-account-placeholder" role="status"><SpinnerIcon size={24} /><strong>Checking your accounts</strong><p>Verifying identity and paid subscriptions…</p></div>
      : draft.accounts.length ? <>
        <div className="setup-account-list" role="group" aria-label="Available accounts">{draft.accounts.map((account) => <label key={account.id} className={`setup-account-option${draft.selected.includes(account.id) ? " is-selected" : ""}`}>
          <Checkbox aria-label={`Add ${account.displayName ?? account.name} to workspace`} checked={draft.selected.includes(account.id)} disabled={draft.busy || draft.pending} onCheckedChange={() => draft.toggle(account.id)} />
          <ProviderLogo provider={account.provider} />
          <span className="setup-account-identity"><strong>{account.displayName ?? account.name}</strong><span>{account.email || PROVIDER_LABELS[account.provider]}</span></span>
          {account.active ? <span className="workspace-active-badge">Active</span> : null}
          <PlanBadge subscription={account.subscription} />
        </label>)}</div>
        <p className="setup-account-note">Only verified accounts with a paid subscription can be added.</p>
      </> : <div className="setup-account-placeholder"><KeyIcon size={28} /><strong>{draft.loaded ? "No paid accounts found" : "Find your existing accounts"}</strong><p>{draft.loaded ? "Sign in to a paid account or add a new account in the next step." : "Load signed-in accounts from this Mac and choose which belong here."}</p></div>}
  </div>;
}

const METHODS: Record<AccountProvider, { id: string; title: string; detail: string }[]> = {
  codex: [
    { id: "browser", title: "Sign in with ChatGPT", detail: "Continue in your browser with a separate Codex login." },
    { id: "device", title: "Use a device code", detail: "Open the sign-in link and enter a one-time code." },
    { id: "file", title: "Import auth.json", detail: "Choose or drop a saved Codex login file." },
  ],
  claude: [
    { id: "browser", title: "Sign in with Claude Code", detail: "Claude’s CLI opens its official browser sign-in." },
    { id: "config", title: "Connect a configuration", detail: "Use an existing signed-in Claude configuration folder." },
  ],
  antigravity: [
    { id: "browser", title: "Sign in with Google", detail: "Connect a separate Google account in your browser." },
    { id: "current", title: "Use current AGY login", detail: "Read the current local AGY session with your permission." },
    { id: "file", title: "Import credentials JSON", detail: "Use an Antigravity or CodexBar OAuth credentials file." },
  ],
};
/** Provider → method → official sign-in or file; successful additions remain drafts. */
export function AccountEnrollment({ draft, onAdded, initiallyOpen = false, embedded = false, onOpenChange }: { draft: Enrollment; onAdded?: () => void; initiallyOpen?: boolean; embedded?: boolean; onOpenChange?: (open: boolean) => void }) {
  const [open, setOpen] = useState(initiallyOpen);
  useEffect(() => { onOpenChange?.(open); }, [open, onOpenChange]);
  const [provider, setProvider] = useState<AccountProvider | null>(null);
  const [method, setMethod] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [code, setCode] = useState("");
  const [file, setFile] = useState<AuthFileSelection | null>(null);
  const [reading, setReading] = useState(false);
  const locked = draft.busy || draft.pending || reading;
  useEffect(() => { draft.setEditing(!!(name || path || file || draft.pending)); }, [name, path, file, draft.pending, draft.setEditing]);
  const before = useRef(draft.revision);
  useEffect(() => {
    if (draft.revision > before.current) {
      before.current = draft.revision;
      setOpen(false); setName(""); setFile(null); setPath(""); setCode("");
      draft.setEditing(false);
      onAdded?.();
    }
  }, [draft.revision, draft.setEditing, onAdded]);
  const close = () => { setOpen(false); setName(""); setFile(null); setPath(""); setCode(""); draft.setEditing(false); draft.setError(null); };
  const content = <div className="account-enrollment setup-page" key={`${provider ?? "providers"}:${method ?? "methods"}`}>

    {draft.error ? <p className="workspace-error" role="alert">{draft.error}</p> : null}
    {provider ? <Button className="enrollment-back" variant="ghost" disabled={locked} onClick={() => { if (method) setMethod(null); else setProvider(null); draft.setError(null); }}><PrevIcon size={16} />{method ? PROVIDER_LABELS[provider] : "All providers"}</Button> : null}
    {!provider ? <div className="enrollment-choices" role="group" aria-label="Choose provider">{(Object.keys(PROVIDER_LABELS) as AccountProvider[]).map((id) => <Button key={id} variant="ghost" className="enrollment-choice" aria-label={PROVIDER_LABELS[id]} onClick={() => { setProvider(id); }}>
      <ProviderLogo provider={id} /><span><strong>{PROVIDER_LABELS[id]}</strong><span>{id === "antigravity" ? "Google subscriptions · Experimental" : "Subscription accounts"}</span></span><NextIcon size={18} />
    </Button>)}</div>
    : !method ? <div className="enrollment-choices" role="group" aria-label="Choose sign-in method">{METHODS[provider].map((choice) => <Button key={choice.id} variant="ghost" className="enrollment-choice" aria-label={choice.title} onClick={() => { setMethod(choice.id); draft.setError(null); }}><KeyIcon size={20} /><span><strong>{choice.title}</strong><span>{choice.detail}</span></span><NextIcon size={18} /></Button>)}</div>
    : <>
      {provider === "antigravity" ? <p className="enrollment-notice">Experimental integration. Identity and a paid Google plan must be verified before this account is added.</p> : null}
      <>
        {!(provider === "codex" && method === "file") ? <label className="enrollment-name">Account name<input aria-label="Account name" autoFocus value={name} maxLength={64} disabled={locked} placeholder="Personal" onChange={(e) => setName(e.target.value)} /></label> : null}
        {method === "file" ? <>
          <AuthFileInput value={file} fileLabel={provider === "codex" ? "auth.json" : "credentials.json"} disabled={locked} onReadingChange={setReading} onChange={setFile} />

          <Button variant="default" disabled={locked || (provider !== "codex" && !name.trim()) || !file} onClick={() => { if (file) void draft.run((session) => importSetupAccount(session, provider, provider === "codex" ? "" : name.trim(), "credentials" in file ? { credentials: file.credentials } : { path: file.path })); }}>{draft.busy ? "Verifying…" : "Verify account"}</Button>
        </> : method === "current" ? <Button variant="default" disabled={locked || !name.trim()} onClick={() => void draft.run((session) => connectCurrentGoogle(session, name.trim()))}>{draft.busy ? "Verifying…" : "Read and verify current login"}</Button> : method === "config" ? <>
          <label>Configuration folder<input aria-label="Claude configuration folder" value={path} placeholder="~/.claude-work" disabled={locked} onChange={(e) => setPath(e.target.value)} /></label>
          <Button variant="default" disabled={locked || !name.trim() || !path.trim()} onClick={() => void draft.run((session) => connectSetupConfig(session, name.trim(), path.trim()))}>{draft.busy ? "Verifying…" : "Verify account"}</Button>
        </> : draft.pending ? <div className="enrollment-login" role="status">
          <strong>Finish signing in with {PROVIDER_LABELS[provider]}</strong>
          <p>Your active CLI login stays unchanged. Return here after the browser confirms sign-in.</p>
          {draft.login?.userCode ? <div className="enrollment-code"><code>{draft.login.userCode}</code><Button size="sm" onClick={() => void navigator.clipboard.writeText(draft.login!.userCode!)}>Copy code</Button></div> : null}
          {draft.login?.authUrl ? <Button onClick={() => void openUrl(draft.login!.authUrl!).catch((e) => draft.setError(prettyError(e)))}>Open sign-in page</Button> : <p>Waiting for the CLI to open your browser…</p>}
          {provider === "claude" ? <details><summary>Browser shows an authorization code?</summary><label>Authorization code<input aria-label="Authorization code" value={code} onChange={(e) => setCode(e.target.value)} autoComplete="off" /></label><Button size="sm" disabled={!code.trim() || draft.busy} onClick={() => void draft.getSession().then((session) => submitSetupCode(session, code.trim())).then(() => setCode("")).catch((e) => draft.setError(prettyError(e)))}>Send code to Claude Code</Button></details> : null}
          <Button variant="ghost" disabled={draft.busy} onClick={() => void draft.cancel()}>Cancel sign-in</Button>
        </div> : <Button variant="default" disabled={locked || !name.trim()} onClick={() => void draft.start(provider, method, name.trim())}>{draft.busy ? "Starting…" : method === "device" ? "Get device code" : "Continue in browser"}</Button>}
      </>
    </>}
    </div>;
  return <div className="enrollment-overview">
    {(!embedded || !open) ? <>
    <Button size="sm" onClick={() => { setProvider(null); setMethod(null); setOpen(true); }}>Add account</Button>
    {draft.manualAccounts.length ? <div className="enrollment-added-list" role="group" aria-label="Added accounts">{draft.manualAccounts.map((account) => <label key={account.id} className="setup-account-option">
      <Checkbox aria-label={`Include ${account.name}`} checked={draft.selected.includes(account.id)} disabled={draft.busy} onCheckedChange={() => draft.toggle(account.id)} />
      <ProviderLogo provider={account.provider} />
      <span className="setup-account-identity"><strong>{account.name}</strong>{account.displayName && account.displayName !== account.name ? <span>{account.displayName}</span> : null}<span>{account.email || PROVIDER_LABELS[account.provider]}</span></span>
      <PlanBadge subscription={account.subscription} />
    </label>)}</div> : null}
    </> : null}
    {embedded ? open ? content : null : <AppModal open={open} onClose={close} title="Add account" cardClass="account-enrollment-card" busy={locked} hasUnsavedChanges={draft.editing}>{content}</AppModal>}
  </div>;
}
