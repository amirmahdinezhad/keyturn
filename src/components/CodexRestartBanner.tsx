import { useCallback, useEffect, useRef, useState } from "react";
import { cancelCodexRestart, codexRestartStatus, prettyError, runCodexRestartNow, scheduleCodexRestart } from "../api";
import type { CodexRestartStatus } from "../types";
import type { ToastKind } from "./Toast";
import { listen } from "@tauri-apps/api/event";
import { AppModal } from "./AppModal";
import { Button } from "./ui/button";

/** Native switches start immediately and pause only for active-work confirmation. */
export function useCodexRestart(notify: (message: string, kind?: ToastKind) => void, onCompleted: (profileId: string) => void) {
  const [status, setStatus] = useState<CodexRestartStatus | null>(null);
  const [pending, setPending] = useState(false);
  const [now, setNow] = useState(Date.now);
  const observed = useRef("");
  const revision = useRef(0);
  useEffect(() => {
    let cancelled = false;
    let inFlight = false;
    const poll = async () => {
      setNow(Date.now());
      if (inFlight) return;
      inFlight = true;
      const ticket = revision.current;
      try {
        const fresh = await codexRestartStatus();
        if (!cancelled && ticket === revision.current) { setNow(Date.now()); setStatus(fresh); }
      } catch { /* Browser previews have no native restart worker. */ }
      finally { inFlight = false; }
    };
    let unlisten: (() => void) | undefined;
    void listen("codex-switch-attention", () => { void poll(); }).then((stop) => { if (cancelled) stop(); else unlisten = stop; }).catch(() => {});
    void poll();
    const timer = window.setInterval(() => void poll(), 1000);
    return () => { cancelled = true; unlisten?.(); window.clearInterval(timer); };
  }, []);
  useEffect(() => {
    const key = status ? `${status.deadlineAt}:${status.phase}` : "";
    if (key === observed.current) return;
    observed.current = key;
    if (status?.phase === "verified") { notify(`Switched to ${status.profileName}`, "ok"); onCompleted(status.profileId); }
    if (status?.phase === "failed") notify(status.error ?? "Couldn’t switch accounts", "err");
  }, [status, notify, onCompleted]);
  const schedule = useCallback(async (id: string, allowLimited = false) => {
    revision.current++;
    setPending(true);
    try { const fresh = await (allowLimited ? scheduleCodexRestart(id, true) : scheduleCodexRestart(id)); setNow(Date.now()); setStatus(fresh); }
    finally { setPending(false); }
  }, []);
  const action = async (fn: typeof cancelCodexRestart) => {
    revision.current++;
    setPending(true);
    try { const fresh = await fn(); setNow(Date.now()); setStatus(fresh); } catch (e) { notify(prettyError(e), "err"); }
    finally { setPending(false); }
  };
  return { status, now, pending, schedule, busy: pending || !!status && ["checking", "awaiting_confirmation", "countdown", "quitting", "activating", "reopening"].includes(status.phase),
    cancel: () => action(cancelCodexRestart), runNow: () => action(runCodexRestartNow) };
}

export function CodexRestartBanner({ restart }: { restart: ReturnType<typeof useCodexRestart> }) {
  const { status, now, pending } = restart;
  if (status?.phase === "awaiting_confirmation") return <AppModal open onClose={() => void restart.cancel()} title={status.activityUncertain ? "Check running threads" : "Thread in progress"} cardClass="workspace-action-popup" overlayClass="workspace-action-overlay" busy={pending} busyMessage="Starting account switch…">
    <p className="workspace-action-copy">{status.activityUncertain ? "Keyturn couldn’t confirm whether a Codex thread is still running." : `${status.runningThreads && status.runningThreads > 1 ? `${status.runningThreads} Codex threads are` : "A Codex thread is"} still running.`} Closing Codex may interrupt that work.</p>
    <p className="workspace-action-copy">Close Codex and switch to <strong>{status.profileName}</strong> now?</p>
    <div className="workspace-action-buttons"><Button disabled={pending} onClick={() => void restart.cancel()}>Cancel</Button><Button variant="default" disabled={pending} onClick={() => void restart.runNow()}>Close and switch</Button></div>
  </AppModal>;
  if (!status || !["checking", "countdown", "quitting", "activating", "reopening"].includes(status.phase)) return null;
  return <aside className="codex-switch-banner" aria-label="Account switch" role="status">
    <span>{status.phase === "countdown" ? `${status.profileName} · Switching in ${Math.max(0, Math.ceil((Date.parse(status.deadlineAt) - now) / 1000))}s` : `Switching to ${status.profileName}…`}</span>
    {status.phase === "countdown" ? <div><Button size="sm" disabled={pending} onClick={() => void restart.cancel()}>Cancel</Button><Button size="sm" disabled={pending} onClick={() => void restart.runNow()}>Switch now</Button></div> : null}
  </aside>;
}
