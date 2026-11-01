import { useCallback, useEffect, useRef, useState } from "react";
import { codexBankedResets, prepareCodexReset, consumeCodexReset, prettyError, type BankedResetInventory, type BankedResetReview, type BankedResetCredit, type QuotaSet } from "../api";
import type { CodexProfile, UsageAppearance } from "../types";
import { AppModal } from "./AppModal";
import { QuotaContent } from "./CliProviders";
import { Button } from "./ui/button";
import { Select } from "./ui/select";
import { RefreshIcon } from "../icons";
import type { ToastKind } from "./Toast";
import { Skeleton } from "./ui/skeleton";
import { bankedResetExpiry } from "../bankedResets";

/** One placeholder for the usage windows and the reset that arrive together. */
function UsageLoading() {
  return <div className="codex-usage-loading" role="status" aria-label="Loading usage and resets">
    <span className="sr-only">Loading usage and resets…</span>
    <div className="quota-skeleton" aria-hidden="true">{[0, 1].map((row) => <div key={row}><Skeleton className="quota-skeleton-label" /><Skeleton className="quota-skeleton-bar" /></div>)}</div>
    <div className="banked-reset-card banked-reset-skeleton" aria-hidden="true"><div><Skeleton className="banked-reset-skeleton-title" /><Skeleton className="banked-reset-skeleton-expiry" /></div><Skeleton className="banked-reset-skeleton-action" /></div>
  </div>;
}

interface Props {
  profile: CodexProfile;
  quota?: QuotaSet | null;
  showRemaining: boolean;
  appearance?: UsageAppearance;
  refreshKey: number;
  onClose: () => void;
  onReconnect: () => void;
  onUsed: () => void;
  notify: (message: string, kind?: ToastKind) => void;
}

/** Keep identity, review and result in one Usage window; no action runs on open. */
export function CodexUsageModal({ profile, quota, showRemaining, appearance, refreshKey, onClose, onReconnect, onUsed, notify }: Props) {
  const [data, setData] = useState<BankedResetInventory | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [review, setReview] = useState<BankedResetReview | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now);
  const generation = useRef(0);
  const mounted = useRef(true);
  const actionRunning = useRef(false);
  const reviewing = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  reviewing.current = !!review;

  const reload = useCallback(async () => {
    const ticket = ++generation.current;
    setLoading(true); setError(null); setActionError(null);
    try {
      const fresh = await codexBankedResets(profile.id);
      if (!mounted.current || ticket !== generation.current) return;
      setData(fresh);
      setSelectedId((old) => fresh.credits.some((c) => c.id === old) ? old : fresh.credits[0]?.id ?? null);
    } catch (e) {
      if (mounted.current && ticket === generation.current) { setError(prettyError(e)); setData(null); }
    } finally { if (mounted.current && ticket === generation.current) setLoading(false); }
  }, [profile.id]);

  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current++; }; }, []);
  useEffect(() => { if (!actionRunning.current && !reviewing.current) void reload(); }, [reload, refreshKey]);
  useEffect(() => { if (review) cancelButton.current?.focus(); }, [review]);
  useEffect(() => { const timer = window.setInterval(() => setNow(Date.now()), 30_000); return () => window.clearInterval(timer); }, []);

  const prepare = async (credit: BankedResetCredit) => {
    if (actionRunning.current) return;
    actionRunning.current = true; setBusy(true); setActionError(null); setResult(null);
    generation.current++; // Discard a background refresh that predates this review.
    try {
      const fresh = await prepareCodexReset(profile.id, credit.id);
      if (mounted.current) setReview(fresh);
    } catch (e) { if (mounted.current) setActionError(prettyError(e)); }
    finally { actionRunning.current = false; if (mounted.current) { setBusy(false); setLoading(false); } }
  };
  const confirm = async () => {
    if (!review || actionRunning.current) return;
    actionRunning.current = true; setBusy(true); setActionError(null);
    try {
      const outcome = await consumeCodexReset(profile.id, review.token);
      if (!mounted.current) return;
      const message = outcome.code === "reset" ? "Reset used" : outcome.code === "already_redeemed" ? "This reset was already used. Usage is being refreshed." : outcome.code === "nothing_to_reset" ? "Your usage does not need a reset right now. No reset was used." : "This reset is no longer available. No new reset was used.";
      setResult(message); setReview(null); setData(null); setLoading(true);
      notify(message, outcome.code === "reset" ? "ok" : "");
      // Refresh cards and this open Usage window from the server, never predicted values.
      onUsed();
    } catch (e) { if (mounted.current) setActionError(prettyError(e)); }
    finally { actionRunning.current = false; if (mounted.current) setBusy(false); }
  };
  const back = () => { setReview(null); setActionError(null); void reload(); };
  const selected = data?.credits.find((c) => c.id === selectedId);
  const shownQuota = data?.usage ?? (result ? undefined : quota);

  return <AppModal open onClose={onClose} title={review ? "Use 1 reset?" : "Usage limits"} cardClass="usage-card" busy={busy} busyMessage={review ? "Using reset…" : "Checking reset…"}
    onBack={review ? back : undefined} backLabel="Back to usage" detail={<div className="usage-identity"><strong>{profile.name}</strong>{profile.email ? <span>{profile.email}</span> : null}</div>}>
    {review ? <div className="banked-reset-review">
      <div className="banked-reset-impact"><RefreshIcon size={20} /><strong>{review.credit.title}</strong></div>
      <p>{review.retry ? "The previous result is unknown. This checks the same attempt without spending a second reset." : "Use one saved reset for this account."}</p>
      <div className="banked-reset-facts"><span>{bankedResetExpiry(review.credit.expiresAt, now)}</span><span>Covered limits: {review.credit.coveredWindows.join(", ")}</span></div>
      <p className="banked-reset-note">Affected refresh times may change. Allowances already at 100% remaining keep their schedule. This uses one saved reset.</p>
      {actionError ? <p className="banked-reset-error" role="alert">{actionError}</p> : null}
      <div className="banked-reset-actions"><Button ref={cancelButton} disabled={busy} onClick={back}>Cancel</Button><Button variant="default" disabled={busy} onClick={() => void confirm()}>{busy ? "Using reset…" : review.retry || actionError ? "Retry same reset" : "Use 1 reset"}</Button></div>
    </div> : <>
      {result ? <p className="banked-reset-result" role="status">{result}</p> : null}
      {loading ? <UsageLoading /> : <>
      <QuotaContent ids={["codex"]} emptyHint="No usage data for this account." showRemaining={showRemaining} providedSets={shownQuota ? [shownQuota] : []}
        appearance={appearance} notify={notify} onReconnect={onReconnect} />
      {error || data?.pendingCredit || (data && data.availableCount > 0) || actionError ? <section className="banked-resets" aria-label="Banked resets">
        {error ? <div><p className="banked-reset-error" role="alert">{error}</p><Button size="sm" onClick={() => void reload()}>Retry</Button></div> : data ? <>
          {data.pendingCredit ? <div className="banked-reset-pending"><p>A previous reset result is unknown. Check it before using another reset.</p><Button disabled={busy} onClick={() => void prepare(data.pendingCredit!)}>Check previous attempt</Button></div> : null}
          {data.availableCount > 0 && data.credits.length > 1 ? <Select aria-label="Choose reset" disabled={busy || !!data.pendingCredit} value={selectedId ?? undefined} onValueChange={setSelectedId}
            options={data.credits.map((c) => ({ value: c.id, label: `${c.title} · ${bankedResetExpiry(c.expiresAt, now)}` }))} /> : null}
          {data.availableCount > 0 && selected ? <div className="banked-reset-card">
            <div className="banked-reset-card-copy"><strong className="banked-reset-title">{selected.title}</strong><span className="banked-reset-expiry">{bankedResetExpiry(selected.expiresAt, now)}</span></div>
            <Button className="banked-reset-use" variant="default" size="sm" disabled={busy || !selected.canUse || !!data.pendingCredit || selected.expiresAt !== null && Date.parse(selected.expiresAt) <= now} onClick={() => void prepare(selected)}>{busy ? "Checking…" : "Use reset"}</Button>
            <Button className="banked-reset-refresh" variant="ghost" size="icon-sm" aria-label="Refresh usage and resets" disabled={busy} onClick={() => void reload()}><RefreshIcon size={14} /></Button>
          </div> : data.availableCount > 0 ? <div><p className="banked-reset-note">Reset details are unavailable.</p><Button size="sm" onClick={() => void reload()}>Retry</Button></div> : null}
        </> : null}
        {actionError ? <p className="banked-reset-error" role="alert">{actionError}</p> : null}
      </section> : null}
      </>}
    </>}
  </AppModal>;
}
