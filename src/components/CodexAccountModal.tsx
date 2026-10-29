import { useEffect, useState } from "react";
import {
  codexAccountQuota,
  codexAccountStatus,
  importCodexProfileJson,
  prettyError,
} from "../api";
import type { CodexProfilesState } from "../types";
import { ProviderLogo } from "../logos";
import { AppModal } from "./AppModal";
import { Button } from "./ui/button";
import { formatAgo } from "./CliProviders";
import type { ToastKind } from "./Toast";

interface Props {
  open: boolean;
  onClose: () => void;
  onProfilesChanged: (s: CodexProfilesState) => void;
  notify: (s: string, kind?: ToastKind) => void;
}
type Status = Awaited<ReturnType<typeof codexAccountStatus>>;

/** Inspect the current CLI account and import a manually supplied login privately. */
export function CodexAccountModal({
  open,
  onClose,
  onProfilesChanged,
  notify,
}: Props) {
  const [status, setStatus] = useState<Status | null>(null);
  const expired = !!status?.expiresAt && status.expiresAt * 1000 <= Date.now();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const [credentials, setCredentials] = useState("");
  const [name, setName] = useState("");
  useEffect(() => {
    setCredentials("");
    setResult(null);
    setError(null);
    if (!open) return;
    let cancelled = false;
    setBusy(true);
    void codexAccountStatus()
      .then((s) => {
        if (!cancelled) setStatus(s);
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
  }, [open]);
  const rescan = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setStatus(await codexAccountStatus());
      notify("Codex CLI rescanned", "ok");
    } catch (e) {
      setError(prettyError(e));
    } finally {
      setBusy(false);
    }
  };
  const test = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const quota = await codexAccountQuota();
      if (!quota || quota.error || !quota.windows?.length)
        throw new Error(
          quota?.error ??
            "No usage data. Sign in to Codex with ChatGPT, then rescan.",
        );
      setResult("Connection verified. Usage limits are available.");
    } catch (e) {
      setError(prettyError(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <AppModal
      open={open}
      onClose={busy ? () => {} : onClose}
      title="Codex account"
      cardClass="profiles-card"
      detail={
        <div className="modal-provider">
          <ProviderLogo provider="codex" />
          <span>Track your Codex usage limits</span>
        </div>
      }
    >
      {error ? (
        <p className="wizard-error" role="alert">
          {error}
        </p>
      ) : null}
      {result ? (
        <p className="profile-switch-note" role="status">
          {result}
        </p>
      ) : null}
      <section className="account-connection" aria-label="Connection status">
        <span
          className={`connection-dot${status?.hasTokens && !expired ? " connected" : ""}`}
        />
        <div>
          <strong>
            {expired
              ? "Login expired"
              : status?.hasTokens
                ? "Connected"
                : status?.hasApiKey
                  ? "API key login"
                  : busy
                    ? "Checking…"
                    : "Not connected"}
          </strong>
          <p>
            {status?.email
              ? `Signed in as ${status.email}`
              : status?.hasApiKey
                ? "Usage limits require a ChatGPT login."
                : "Sign in with the Codex CLI or import a saved login."}
          </p>
        </div>
        <Button
          disabled={busy || !status?.hasTokens}
          onClick={() => void test()}
        >
          Test connection
        </Button>
      </section>
      <section className="account-section">
        <h3>Codex CLI</h3>
        <p className="setting-note">
          Credentials are read live from the CLI's auth.json.
        </p>
        {status?.lastRefresh ? (
          <p className="setting-note">
            Tokens refreshed {formatAgo(status.lastRefresh)}
          </p>
        ) : null}
        <p className="setting-note">
          Signing in or out with Codex is reflected when you rescan.
        </p>
        <Button disabled={busy} onClick={() => void rescan()}>
          {busy ? "Checking…" : "Rescan"}
        </Button>
      </section>
      <section className="account-section">
        <h3>Manual credentials</h3>
        <p className="setting-note">
          Paste auth.json to save it as a profile on this Mac.
        </p>
        <form noValidate
          className="profile-form"
          onSubmit={(e) => {
            e.preventDefault();
            if (!credentials.trim() || credentials.length > 256 * 1024) return;
            setBusy(true);
            setError(null);
            setResult(null);
            void importCodexProfileJson(name, credentials)
              .then((fresh) => {
                onProfilesChanged(fresh);
                setCredentials("");
                setName("");
                setResult(
                  "Credentials saved as a profile. Open Profiles and choose Set active to use this account.",
                );
              })
              .catch((e) => setError(prettyError(e)))
              .finally(() => setBusy(false));
          }}
        >
          <label htmlFor="manual-profile-name">Profile name (optional)</label>
          <input
            id="manual-profile-name"
            maxLength={64}
            value={name}
            onChange={(e) => setName(e.target.value)}
            disabled={busy}
            placeholder="Leave empty for a random name"
          />
          <label htmlFor="manual-codex-auth">Contents of auth.json</label>
          <textarea
            id="manual-codex-auth"
            value={credentials}
            onChange={(e) => setCredentials(e.target.value)}
            disabled={busy}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
          />
          {credentials.length > 256 * 1024 ? (
            <p className="wizard-error">
              Use an auth.json smaller than 256 KB.
            </p>
          ) : null}
          <div className="wizard-foot is-end">
            <Button
              className="primary"
              type="submit"
              disabled={
                busy || !credentials.trim() || credentials.length > 256 * 1024
              }
            >
              Save credentials
            </Button>
          </div>
        </form>
      </section>
    </AppModal>
  );
}
