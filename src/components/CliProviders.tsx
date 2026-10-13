import { useEffect, useState } from "react";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";
import { cliQuotas, type ProviderCliStatus, type QuotaSet } from "../api";
import { ProviderLogo } from "../logos";
import { AddIcon, CopyIcon, UsageIcon } from "../icons";
import type { ToastKind } from "./Toast";
import { PROVIDER_NAMES } from "../providerSetup";

interface CliProvidersProps {
  statuses: ProviderCliStatus[] | null;
  notify: (message: string, kind?: ToastKind) => void;
  /** Frontend-only display prefs (all optional to keep tests + callers simple). */
  showDetails?: boolean;
  compactRows?: boolean;
  /** Open the add-account flow (plus row). */
  onAddAccount: () => void;
  /** Open the usage popup for a connected provider. */
  onShowUsage: (status: ProviderCliStatus) => void;
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
  showDetails = true,
  compactRows = false,
  onAddAccount,
  onShowUsage,
}: CliProvidersProps) {
  if (!statuses) return null;
  const ready = statuses.filter((s) => s.binaryFound && s.loggedIn === true);

  return (
    <div className={`picker${compactRows ? " compact" : ""}`}>
      {ready.length > 0 ? (
        <section className="stack-group">
          <p className="section-label provider-section-label">Connected accounts <span>{ready.length}</span></p>
          <div className="stack">
            {ready.map((s) => (
              <ProviderCard
                key={s.id}
                status={s}
                showDetails={showDetails}
                onShowUsage={onShowUsage}
              />
            ))}
          </div>
        </section>
      ) : (
        <div className="provider-empty">
          <p>No connected accounts</p>
          <span>Add a provider to see its account and usage limits here.</span>
        </div>
      )}

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
}

/** Provider ids with a quota source behind the usage popup. */
const QUOTA_IDS = ["opencode", "opencode-go", "claude", "codex"];

/** "codex-cli 0.52.0" → "codexCli 0.52.0": camelCase the binary-name
 *  prefix for the version pill tooltip, so kebab-case CLI names read as
 *  one identifier while the exact version number stays intact.
 *  Pure + tested. */
export function formatCliTitle(raw: string | null | undefined): string {
  if (!raw) return "";
  const trimmed = raw.trim();
  if (!trimmed) return "";
  const [first, ...rest] = trimmed.split(/\s+/);
  const camel = first.replace(/-([a-zA-Z0-9])/g, (_, c: string) =>
    c.toUpperCase(),
  );
  return [camel, ...rest].join(" ").slice(0, 80);
}

/** "codex-cli 0.52.0" → "v0.52.0": strip the binary-name prefix the
 *  `--version` output carries so the card shows a quiet version pill.
 *  Pure + tested via App tests. */
export function formatCliVersion(raw: string | null | undefined): string {
  if (!raw) return "";
  const trimmed = raw.trim();
  if (!trimmed) return "";
  const semver = trimmed.match(/(\d+\.\d+\.\d+(?:[-+._a-zA-Z0-9]*))/);
  if (semver) return `v${semver[1]}`;
  const short = trimmed.match(/(\d+\.\d+(?:[-+._a-zA-Z0-9]*))/);
  if (short) return `v${short[1]}`;
  return trimmed.slice(0, 32);
}

function ProviderCard({
  status: s,
  showDetails,
  onShowUsage,
}: {
  status: ProviderCliStatus;
  showDetails: boolean;
  onShowUsage: (status: ProviderCliStatus) => void;
}) {
  const method = s.authMethod === "claude.ai" ? "Claude account"
    : s.authMethod === "Google OAuth" ? "Google account"
    : s.authMethod?.startsWith("env:") ? "Environment credentials"
    : s.authMethod;
  const account = s.account && !/^(logged in|user_|account_|[a-f0-9]{8}-)/i.test(s.account)
    ? s.account : null;
  const connections = [...new Set(s.connected ?? [])];
  return (
    <div className="cli-card">
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
      </div>

      {s.email ? <p className="cli-identity" title={s.email}>{s.email}</p> : null}
      {connections.length > 0 ? (
        <div className="cli-services" aria-label="Connected services">
          {connections.map((id) => <span key={id}>{PROVIDER_NAMES[id] ?? id}</span>)}
        </div>
      ) : null}
      <div className="cli-footer">
        <span className="cli-connection"><i aria-hidden="true" />{method || (connections.length ? `${connections.length} services` : "Connected")}</span>
        {account ? <span className="cli-account" title={account}>{account}</span> : null}
        {showDetails && s.version && formatCliVersion(s.version) ? (
          <span className="cli-version" title={formatCliTitle(s.version)}>CLI {formatCliVersion(s.version)}</span>
        ) : null}
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
 * Quota windows for given auth/provider ids, fetched once on mount.
 * OpenCode reads live from the CLI's own quota endpoint; Claude replays
 * the newest snapshot the CLI persisted locally (with its age shown);
 * Codex reads its official usage endpoint. Read-only throughout.
 *
 * The backend reads all supported sources; this component filters by ids.
 * Rows show the window name, exact used/remaining percent and reset countdown.
 */
export function QuotaContent({
  ids,
  emptyHint,
  showRemaining,
  notify,
}: {
  ids: string[];
  emptyHint: string;
  showRemaining: boolean;
  notify: (message: string, kind?: ToastKind) => void;
}) {
  const [sets, setSets] = useState<QuotaSet[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void cliQuotas()
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
  }, [ids.join(","), notify]);

  if (sets === null && error === null) {
    return (
      <span className="loading-dots">
        <span className="dots" role="status" aria-label="Loading usage">
          <i />
          <i />
          <i />
        </span>
        <span className="dots-label">Loading…</span>
      </span>
    );
  }
  if (error !== null) {
    return <p className="cli-hint">{error}</p>;
  }
  if (sets !== null && sets.length === 0) {
    return <p className="cli-hint">{emptyHint}</p>;
  }
  return (
    <>
      {(sets ?? []).map((set) =>
        set.windows ? (
          <div key={set.provider} className="quota-windows">
            {set.windows
              // Internal spare capacity, not a user-facing limit — hide it.
              .filter((w) => w.window.toLowerCase() !== "gpt-reserve")
              .map((w) => {
                  const shown = showRemaining
                    ? Math.min(100, Math.max(0, 100 - w.percent))
                    : w.percent;
                  const text = shown % 1 === 0 ? shown.toFixed(0) : shown.toFixed(1);
                  const label = WINDOW_LABELS[w.window] ?? w.window;
                  const tone = quotaTone(w.percent);
                  const resetText = formatReset(w.resetsAt);
                  return (
                    <div key={w.window} className="usage-row">
                      <div className="usage-top">
                        <span className="usage-name">{label}</span>
                        <span className={`usage-pct${tone ? ` ${tone}` : ""}`}>{text}%</span>
                      </div>
                      <Progress
                        className="usage-bar"
                        value={shown}
                        indicatorClassName={`usage-fill${tone ? ` ${tone}` : ""}`}
                        aria-label={`${label} usage`}
                      />
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
              <p key={set.provider} className="cli-hint">
                {set.provider}: {set.error ?? "No quota data"}
              </p>
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
