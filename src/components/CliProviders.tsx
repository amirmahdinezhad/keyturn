import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";
import { Tooltip } from "./ui/tooltip";
import { claudeProfileQuota, cliQuotas, type ProviderCliStatus, type QuotaSet } from "../api";
import { ProviderLogo } from "../logos";
import { AddIcon, CopyIcon, UsageIcon, SettingIcon } from "../icons";
import type { ToastKind } from "./Toast";
import { PROVIDER_NAMES } from "../providerSetup";
import { DEFAULT_APP_SETTINGS, type UsageAppearance } from "../types";
import { usageColor, usagePace } from "../usage";
import { PlanBadge } from "./PlanBadge";
import { accountSubscription, isSubscriptionExpired, type Subscription } from "../subscriptions";
import { accentStyle, DEFAULT_ACCENT } from "../displayPrefs";
import type { AppSettings, AccentProvider, ClaudeProfile } from "../types";
import { Skeleton } from "./ui/skeleton";
import { AppModal } from "./AppModal";

interface CliProvidersProps {
  statuses: ProviderCliStatus[] | null;
  notify: (message: string, kind?: ToastKind) => void;
  /** Compact provider-list spacing. */
  compactRows?: boolean;
  /** Open the add-account flow (plus row). */
  onAddAccount: () => void;
  /** Open the usage popup for a connected provider. */
  onShowUsage: (status: ProviderCliStatus) => void;
  sectionLabel?: string;
  onManageClaude?: (id?: string) => void;
  claudeAccounts?: ClaudeProfile[];
  showRemaining?: boolean;
  usageAppearance?: UsageAppearance;
  refreshKey?: number;
  providerAccents?: AppSettings["providerAccents"];
}

async function copyText(text: string): Promise<boolean> {
  try {
    if (!navigator.clipboard) return false;
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** Display a command and report clipboard success or failure without executing it. */
export function CopyRow({
  label,
  text,
  copiedMessage,
  notify,
}: {
  label: string;
  text: string;
  copiedMessage: string;
  notify: (message: string, kind?: ToastKind) => void;
}) {
  return (
    <div className="cmd-row">
      <div className="cmd-text">
        <span className="cmd-label">{label}</span>
        <code>{text}</code>
      </div>
      <Button
        type="button"
        className="cmd-copy"
        aria-label="Copy"
        title={`Copy ${label}`}
        onClick={() => {
          void copyText(text).then((ok) =>
            notify(
              ok ? copiedMessage : "Copy failed — select the text manually",
              ok ? "ok" : "err",
            ),
          );
        }}
      >
        <CopyIcon size={17} />
        <span>Copy</span>
      </Button>
    </div>
  );
}

/**
 * Connected CLI accounts, with identity and connection details from the
 * provider's local status. Setup remains behind the add-account button.
 */
export function CliProviders({
  statuses,
  notify,
  compactRows = false,
  onAddAccount,
  onShowUsage,
  sectionLabel = "Connected accounts",
  onManageClaude,
  providerAccents,
  claudeAccounts = [],
  showRemaining = DEFAULT_APP_SETTINGS.usageShowRemaining,
  usageAppearance = DEFAULT_APP_SETTINGS.usageAppearance,
  refreshKey,
}: CliProvidersProps) {
  const [claudeUsageId, setClaudeUsageId] = useState<string | null>(null);
  const claudeUsage = claudeAccounts.find((profile) => profile.id === claudeUsageId);
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  if (!statuses && !claudeAccounts.length) return null;
  const ready = (statuses ?? []).filter((s) => {
    if (!s.binaryFound || s.loggedIn !== true) return false;
    const subscriptions = providerSubscriptions(s);
    return !subscriptions.length || subscriptions.some((subscription) => !isSubscriptionExpired(subscription, now));
  });

  if (sectionLabel === "Other providers") {
    const savedClaude = claudeAccounts.filter((p) => p.connected && !p.quota?.error && !isSubscriptionExpired(p.subscription, now));
    const ids = [...new Set([...ready.map((s) => s.id), ...(savedClaude.length ? ["claude"] : [])])];
    if (!ids.length) return null;
    const groups = ids.map((id) => {
        const status = ready.find((s) => s.id === id);
        const saved = id === "claude" ? savedClaude : [];
        // A tracked current configuration already represents the default CLI login.
        const showCurrent = !!status && !claudeAccounts.some((p) => p.active);
        const count = saved.length + (showCurrent ? 1 : 0);
        if (!count) return null;
        const label = status?.label ?? "Claude";
        return <section className="provider-account-group" aria-label={`${label} accounts`} key={id}>
          <h3 className="provider-group-title"><ProviderLogo provider={id} />{label} <span className="provider-count" aria-label={`${count} ${label} accounts`}>{count}</span></h3>
          <div className="stack">
            {showCurrent && status ? <ProviderCard accountLayout={id === "claude"} status={status} now={now} onShowUsage={onShowUsage}
              accent={providerAccents?.[id as AccentProvider]} /> : null}
            {saved.map((profile) => <article className={`cli-card saved-provider-account workspace-account account-card${profile.active ? " is-active" : ""}`} aria-label={`${profile.name} Claude account`} key={profile.id} style={accentStyle(providerAccents?.claude ?? DEFAULT_ACCENT)}>
              <div className="workspace-account-row">
                <div className="workspace-account-info">
                  <div className="workspace-account-title"><span className="row-title" title={profile.name}>{profile.name}</span><PlanBadge subscription={profile.subscription ?? { id: "claude", service: "Claude" }} />{profile.active ? <span className="workspace-active-badge">Active</span> : null}</div>
                  {profile.email ? <p className="workspace-account-email" title={profile.email}>{profile.email}</p> : null}

                </div>
                <div className="workspace-account-actions">
                  <Button size="icon-sm" variant="ghost" aria-label={`Usage limits for ${profile.name}`} title="Usage limits" disabled={!profile.connected} onClick={() => setClaudeUsageId(profile.id)}><UsageIcon size={17} /></Button>
                  {onManageClaude ? <Button size="icon-sm" variant="ghost" aria-label={`Manage ${profile.name}`} title="Account settings" onClick={() => onManageClaude(profile.id)}><SettingIcon size={17} /></Button> : null}
                </div>
              </div>
              {!profile.connected ? <p className="workspace-error">Sign-in needed</p> : null}
            </article>)}
          </div>
        </section>;
      });
    const otherGroups = groups.filter((group) => group && group.key !== "claude");
    return <>
      <div className="provider-account-groups">{groups.filter((group) => group?.key === "claude")}</div>
      {otherGroups.length ? <details className="other-providers"><summary>Other providers <span>{otherGroups.length}</span></summary>
        <div className="provider-account-groups">{otherGroups}</div>
        <Button className="add-provider-quiet" variant="ghost" size="sm" onClick={onAddAccount}><AddIcon size={16} />Add provider</Button>
      </details> : null}
      <AppModal open={!!claudeUsage} onClose={() => setClaudeUsageId(null)} title="Usage limits" cardClass="usage-card"
        detail={claudeUsage ? <div className="usage-identity"><strong>{claudeUsage.name}</strong>{claudeUsage.email ? <span>{claudeUsage.email}</span> : null}</div> : undefined}>
        {claudeUsage ? <QuotaContent key={claudeUsage.id} ids={["claude"]} emptyHint="No usage data for this Claude account."
          showRemaining={showRemaining} appearance={usageAppearance} loadQuota={() => claudeProfileQuota(claudeUsage.id)} refreshKey={refreshKey} notify={notify} /> : null}
      </AppModal>
    </>;
  }

  const content = (
    <div className={`picker${compactRows ? " compact" : ""}`}>
      {ready.length > 0 ? (
        <section className="stack-group">
          <p className="section-label provider-section-label">{sectionLabel} <span>{ready.length}</span></p>
          <div className="stack">
            {ready.map((s) => (
              <ProviderCard
                key={s.id}
                status={s}
                now={now}
                onShowUsage={onShowUsage}
                onManage={s.id === "claude" && onManageClaude ? () => onManageClaude() : undefined}
                accent={providerAccents?.[s.id as AccentProvider]}
              />
            ))}
          </div>
        </section>
      ) : sectionLabel === "Connected accounts" ? (
        <div className="provider-empty">
          <p>No connected accounts</p>
          <span>Add a provider to see its account and usage limits here.</span>
        </div>
      ) : null}

      <Button
        type="button"
        className="fab"
        onClick={onAddAccount}
        aria-label="Add account"
      >
        <AddIcon size={22} />
      </Button>
    </div>
  );
  return content;
}

/** Provider ids with a quota source behind the usage popup. */
const QUOTA_IDS = ["opencode", "opencode-go", "claude", "codex"];

function providerSubscriptions(status: ProviderCliStatus): Subscription[] {
  if (status.subscriptions?.length) return status.subscriptions;
  if (status.id === "opencode") return [...new Set(status.connected ?? [])].map((id) => ({
    id, service: PROVIDER_NAMES[id] ?? id,
    // An API key alone cannot distinguish Go from Go Plus or prove billing status.
    plan: id === "opencode-go" ? "Subscription" : id === "opencode" ? "Pay as you go" : "API",
  }));
  const subscription = accountSubscription(status.id, status.authMethod, status.account);
  return subscription ? [subscription] : [];
}

function ProviderCard({
  status: s,
  onShowUsage,
  now,
  onManage,
  accent,
  accountLayout = false,
}: {
  status: ProviderCliStatus;
  onShowUsage: (status: ProviderCliStatus) => void;
  now: number;
  onManage?: () => void;
  accent?: string;
  accountLayout?: boolean;
}) {
  const method = s.authMethod === "claude.ai" ? "Claude account"
    : s.authMethod === "Google OAuth" ? "Google account"
    : s.authMethod?.startsWith("env:") ? "Environment credentials"
    : s.authMethod;
  const subscriptions = providerSubscriptions(s).filter((subscription) => !isSubscriptionExpired(subscription, now));
  const account = s.account && !subscriptions.some((subscription) => subscription.plan?.toLowerCase() === s.account?.toLowerCase()) && !/^(logged in|user_|account_|[a-f0-9]{8}-)/i.test(s.account)
    ? s.account : null;
  if (accountLayout) return <article className="cli-card workspace-account account-card is-active" aria-label={`${s.label} account`} style={accentStyle(accent ?? DEFAULT_ACCENT)}>
    <div className="workspace-account-row">
      <div className="workspace-account-info">
        <div className="workspace-account-title"><span className="row-title">{s.label}</span>{subscriptions.map((subscription) => <span className="workspace-account-plan" key={subscription.id}><PlanBadge subscription={subscription} /></span>)}<span className="workspace-active-badge">Active</span></div>
        {s.email ? <p className="workspace-account-email" title={s.email}>{s.email}</p> : null}

      </div>
      <div className="workspace-account-actions">
        <Button size="icon-sm" variant="ghost" aria-label={`Usage limits for ${s.label}`} title="Usage limits" onClick={() => onShowUsage(s)}><UsageIcon size={17} /></Button>
        {onManage ? <Button size="icon-sm" variant="ghost" aria-label="Manage Claude profiles" title="Account settings" onClick={onManage}><SettingIcon size={17} /></Button> : null}
      </div>
    </div>
  </article>;
  return (
    <div className="cli-card" style={accentStyle(accent ?? DEFAULT_ACCENT)}>
      <div className="cli-head">
        <ProviderLogo provider={s.id} />
        <span className="row-main">
          <span className="row-title">{s.label}</span>
        </span>
        {QUOTA_IDS.includes(s.id) ? (
          <Button
            size="icon-sm"
            className="disc icon-only usage-icon-btn"
            aria-label={`Usage limits for ${s.label}`}
            onClick={() => onShowUsage(s)}
          >
            <UsageIcon size={20} />
          </Button>
        ) : null}
        {onManage ? <Button size="icon-sm" aria-label="Manage Claude profiles" onClick={onManage}><SettingIcon size={18} /></Button> : null}
      </div>

      {s.email ? <Tooltip content={s.email}><p className="cli-identity">{s.email}</p></Tooltip> : null}
      <div className="cli-footer">
        {subscriptions.length ? (
          <div className="cli-subscriptions" aria-label="Connected services">
            {subscriptions.map((subscription) => (
              <div className="cli-subscription" key={subscription.id}>
                <span className="cli-connection">{subscription.service}</span>
                <PlanBadge subscription={subscription.id === "opencode-go" && !subscription.plan ? { ...subscription, plan: "Subscription" } : subscription} />
              </div>
            ))}
            {subscriptions.length > 1 ? <span className="cli-service-count">{subscriptions.length} services</span> : null}
          </div>
        ) : <span className="cli-connection">{method || "Connected"}</span>}
        {account ? <Tooltip content={account}><span className="cli-account">{account}</span></Tooltip> : null}
      </div>
    </div>
  );
}

const WINDOW_LABELS: Record<string, string> = {
  rolling: "5-hour",
  weekly: "Weekly",
  monthly: "Monthly",
};

/** "Resets in 3h 12m" wording for an ISO reset timestamp. Pure + tested. */
export function formatReset(iso: string | null | undefined): string {
  if (!iso) return "";
  const ms = new Date(iso).getTime() - Date.now();
  if (!Number.isFinite(ms)) return "";
  if (ms <= 0) return "Resets now";
  const totalMinutes = Math.floor(ms / 60000);
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  if (days >= 1) return `Resets in ${days}d ${hours}h`;
  if (hours >= 1) return `Resets in ${hours}h ${minutes}m`;
  return `Resets in ${minutes}m`;
}

function quotaTone(percent: number): string {
  if (percent >= 90) return "hot";
  if (percent >= 70) return "warm";
  return "";
}

/**
 * Quota windows for given auth/provider ids, refreshed with the app's data cycle.
 * OpenCode reads live from the CLI's own quota endpoint; Claude's account loader
 * fetches live limits with a timestamped local snapshot fallback;
 * Codex reads its usage endpoint; saved-account loaders may renew through the CLI.
 *
 * The backend reads all supported sources; this component filters by ids.
 * Rows show the window name, exact used/remaining percent and reset countdown.
 */
export function QuotaContent({
  ids,
  emptyHint,
  showRemaining,
  refreshKey,
  appearance = DEFAULT_APP_SETTINGS.usageAppearance,
  notify,
  providedSets,
  loadQuota,
  onReconnect,
}: {
  ids: string[];
  emptyHint: string;
  showRemaining: boolean;
  refreshKey?: number;
  appearance?: UsageAppearance;
  notify: (message: string, kind?: ToastKind) => void;
  providedSets?: QuotaSet[];
  loadQuota?: () => Promise<QuotaSet>;
  onReconnect?: () => void;
}) {
  const loader = useRef(loadQuota);
  loader.current = loadQuota;
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, []);
  const [sets, setSets] = useState<QuotaSet[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (providedSets) return;
    let cancelled = false;
    void (loader.current ? loader.current().then((set) => [set]) : cliQuotas())
      .then((result) => {
        if (cancelled) return;
        setSets(result.filter((set) => ids.includes(set.provider)));
        setError(null);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
        notify("Couldn't read usage limits", "err");
      });
    return () => {
      cancelled = true;
    };
  }, [ids.join(","), notify, refreshKey, providedSets]);

  const displayedSets = providedSets ?? sets;

  if (displayedSets === null && error === null) {
    return (
      <div className="quota-skeleton" role="status" aria-label="Loading usage"><span className="sr-only">Loading…</span>{[0, 1].map((row) => <div key={row} aria-hidden="true"><Skeleton className="quota-skeleton-label" /><Skeleton className="quota-skeleton-bar" /></div>)}</div>
    );
  }
  if (!providedSets && error !== null) {
    return <p className="cli-hint">{error}</p>;
  }
  if (displayedSets !== null && displayedSets.length === 0) {
    return <p className="cli-hint">{emptyHint}</p>;
  }
  return (
    <>
      {(displayedSets ?? []).map((set) =>
        set.windows ? (
          <div key={set.provider} className="quota-windows">
            {set.windows
              // Internal spare capacity, not a user-facing limit — hide it.
              .filter((w) => w.window.toLowerCase() !== "gpt-reserve")
              .map((w) => {
                  const remaining = showRemaining;
                  const capacity = remaining
                    ? Math.min(100, Math.max(0, 100 - w.percent))
                    : w.percent;
                  const shown = set.provider === "codex" ? Math.round(capacity) : capacity;
                  const text = shown % 1 === 0 ? shown.toFixed(0) : shown.toFixed(1);
                  const label = WINDOW_LABELS[w.window] ?? w.window;
                  const tone = quotaTone(w.percent);
                  const pace = usagePace(w, now);
                  const color = usageColor(w.percent, appearance.paceAwareColors ? pace?.projected : null);
                  const resetText = formatReset(w.resetsAt);
                  return (
                    <div key={w.window} className="usage-row">
                      <div className="usage-top">
                        <span className="usage-name">{label}</span>
                        <span className={`usage-pct${tone ? ` ${tone}` : ""}`} style={{ "--usage-value": color } as CSSProperties}>{text}%</span>
                      </div>
                      <div className="usage-track" style={{ "--usage-source": color } as CSSProperties}>
                        <Progress className="usage-bar" value={shown}
                          indicatorClassName={`usage-fill${tone ? ` ${tone}` : ""}`} aria-label={`${label} usage`} />
                        {appearance.showTimeMarker && pace ? <span className="usage-time-marker" role="img"
                          aria-label={`${Math.round(pace.elapsed * 100)}% of ${label} period elapsed`}
                          style={{ left: `${(remaining ? 1 - pace.elapsed : pace.elapsed) * 100}%`, background: "var(--muted)" }} /> : null}
                      </div>
                      {resetText ? (
                        <div className="usage-meta">
                          <span className="usage-reset">{resetText}</span>
                        </div>
                      ) : null}
                    </div>
                  );
                })}
                {set.asOf ? (
                  <span className="quota-sub">Updated {formatAgo(set.asOf)}</span>
                ) : null}
              </div>
            ) : (
              <div key={set.provider} className="quota-error-state">
                <p className="cli-hint">{set.provider === "codex" && /^session could not be renewed/i.test(set.error ?? "")
                  ? "This account’s saved session is no longer valid. Reconnect it with a fresh auth.json to load usage. Inactive accounts can show usage without switching Codex accounts."
                  : `${set.provider}: ${set.error ?? "No quota data"}`}</p>
                {onReconnect && set.provider === "codex" && /^session (expired|rejected|could not be renewed)/i.test(set.error ?? "")
                  ? <Button onClick={onReconnect}>Reconnect account</Button> : null}
              </div>
            ),
          )
        }
    </>
  );
}

/** "3h ago" wording for an ISO timestamp. Pure + tested. */
export function formatAgo(iso: string | null | undefined): string {
  if (!iso) return "";
  const ms = Date.now() - new Date(iso).getTime();
  if (!Number.isFinite(ms)) return "";
  if (ms < 0) return "just now";
  const minutes = Math.floor(ms / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}
