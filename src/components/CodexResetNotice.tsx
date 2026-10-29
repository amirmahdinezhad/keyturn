import { useEffect, useState } from "react";
import { codexResetStatus, type CodexResetStatus } from "../api";
import { CloseIcon, RefreshIcon } from "../icons";
import { loadDismissedResets, resetAnnouncementId, resetWatchId, resetCountdown, resetForecastTiming, resetLocalTime, saveDismissedResets } from "../codexResets";
import { DEFAULT_CODEX_RESET_SETTINGS, type CodexResetSettings } from "../types";
import { Button } from "./ui/button";

const MAX_AGE_MS = 15 * 60_000;

/** Provider-level reset updates; completion requires fresh source confirmation. */
export function CodexResetNotice({ refreshKey, settings = DEFAULT_CODEX_RESET_SETTINGS }: {
  refreshKey: unknown;
  settings?: CodexResetSettings;
}) {
  const [data, setData] = useState<CodexResetStatus | null>(null);
  const [dismissed, setDismissed] = useState(loadDismissedResets);
  const [now, setNow] = useState(Date.now);
  const intervalMs = settings.refetchIntervalSecs * 1000;

  useEffect(() => saveDismissedResets(dismissed), [dismissed]);

  useEffect(() => {
    let cancelled = false;
    let generation = 0;
    const refresh = () => {
      const ticket = ++generation;
      setNow(Date.now());
      void codexResetStatus().then(
        (result) => {
          if (cancelled || ticket !== generation) return;
          setData(result);
          setNow(Date.now());
        },
        () => {
          if (!cancelled && ticket === generation) setData(null);
        },
      );
    };
    refresh();
    const polling = window.setInterval(refresh, intervalMs);
    window.addEventListener("focus", refresh);
    window.addEventListener("online", refresh);
    return () => {
      cancelled = true;
      window.clearInterval(polling);
      window.removeEventListener("focus", refresh);
      window.removeEventListener("online", refresh);
    };
  }, [refreshKey, intervalMs]);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const generated = data ? Date.parse(data.generatedAt) : NaN;
  const maxAge = Math.max(MAX_AGE_MS, intervalMs + 60_000);
  const fresh = Number.isFinite(generated) && now - generated <= maxAge && generated - now <= 60_000;
  if (!fresh) return null;
  const announcement = data?.scheduledReset;
  const announcedAt = Date.parse(announcement?.announcedAt ?? "");
  const scheduledEnd = Date.parse(announcement?.scheduledFor ?? "");
  const scheduled = announcement && Number.isFinite(announcedAt) && announcedAt <= now + 60_000
    && (announcement.scheduledFor === null || (Number.isFinite(scheduledEnd) && scheduledEnd > now)) ? announcement : null;
  const completedAt = Date.parse(data?.latestReset?.announcedAt ?? "");
  const completed = !scheduled && data?.latestReset && Number.isFinite(completedAt)
    && completedAt <= now && now - completedAt < 24 * 60 * 60_000 ? data.latestReset : null;
  const forecast = !scheduled && !completed && settings.showProbableResets ? data?.activeWatch : null;
  const observedAt = Date.parse(forecast?.observedAt ?? "");
  const expiresAt = Date.parse(forecast?.expiresAt ?? "");
  const chance = forecast?.resetChancePercent;
  const watch = forecast && Number.isFinite(observedAt) && observedAt <= now + 60_000
    && Number.isFinite(expiresAt) && expiresAt > now && observedAt < expiresAt
    && (chance == null || (Number.isFinite(chance) && chance >= 0 && chance <= 100)) ? forecast : null;
  if (!scheduled && !completed && !watch) return null;
  const id = scheduled ? resetAnnouncementId(scheduled) : completed ? `completed:${completed.id}` : resetWatchId(watch!);
  if (dismissed.includes(id)) return null;
  const scheduledDate = scheduled?.scheduledFor;

  return (
    <div className="codex-reset-notice" aria-label="Codex reset outlook" data-kind={completed ? "completed" : watch ? "possible" : "scheduled"}>
      <div className="codex-reset-head">
        <RefreshIcon size={13} />
        <strong>{completed ? "Reset confirmed" : watch ? "Possible reset" : scheduledDate ? resetCountdown(scheduledDate, now) : "Reset announced"}</strong>
        <Button size="icon-sm" className="codex-reset-close" aria-label={completed ? "Dismiss confirmed reset" : watch ? "Dismiss possible reset" : "Dismiss upcoming reset"}
          onClick={() => setDismissed((previous) => [...previous, id].slice(-100))}>
          <CloseIcon size={13} />
        </Button>
      </div>
      {completed ? <div className="codex-reset-meta"><time dateTime={completed.announcedAt}>Confirmed at {resetLocalTime(completed.announcedAt)}</time></div> : watch ? <>
        {chance != null ? <p className="codex-reset-description">{chance}% chance</p> : null}
        <div className="codex-reset-meta"><time dateTime={watch.expiresAt}>{resetForecastTiming(watch.expiresAt, now)}</time></div>
      </>
        : scheduledDate ? <div className="codex-reset-meta"><time dateTime={scheduledDate}>At {resetLocalTime(scheduledDate)}</time></div> : <p className="codex-reset-description">Time not announced</p>}
      {(scheduled?.resetType ?? completed?.resetType) === "banked" ? <p className="codex-reset-description">Extra reset credit; current usage may stay unchanged.</p> : null}
    </div>
  );
}
