import { useEffect, useRef, useState } from "react";
import { Button } from "./ui/button";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ProviderCliStatus } from "../api";
import { ProviderLogo } from "../logos";
import { AddIcon } from "../icons";
import { AppModal } from "./AppModal";
import { CopyRow } from "./CliProviders";
import { PROVIDER_SETUP } from "../providerSetup";
import type { ToastKind } from "./Toast";
import { SetupSteps } from "./SetupSteps";

interface AddAccountModalProps {
  open: boolean;
  /** Supported providers, including connected providers that accept more accounts. */
  candidates: ProviderCliStatus[];
  onClose: () => void;
  /** Re-read CLI status; resolves with the fresh list. */
  onRefresh: () => Promise<ProviderCliStatus[]>;
  onConnected?: (providerId: string) => Promise<void>;
  notify: (message: string, kind?: ToastKind) => void;
  /** Named accounts stay reachable even when Codex is already connected. */
  onCodexProfiles?: () => void;
  onClaudeProfiles?: () => void;
  title?: string;
}

type Step = "pick" | "install" | "login" | "done";

/**
 * Stepped setup for one more provider CLI. The sequence is real
 * (pick → install when needed → sign in → done), so a stepper is
 * appropriate. Every step validates before advancing:
 * pick needs a selection, install needs `binaryFound`, login needs
 * `loggedIn`. Keyturn never signs in; it only shows the commands
 * and re-probes via `onRefresh`.
 */
export function AddAccountModal({
  open,
  candidates,
  onClose,
  onRefresh,
  onConnected,
  notify,
  onCodexProfiles,
  onClaudeProfiles,
  title = "Add account",
}: AddAccountModalProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [step, setStep] = useState<Step>("pick");
  const [live, setLive] = useState<ProviderCliStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [includeInstall, setIncludeInstall] = useState(false);
  const checkGeneration = useRef(0);

  // Fresh open resets the wizard.
  useEffect(() => {
    checkGeneration.current++;
    if (open) {
      setSelectedId(null);
      setStep("pick");
      setLive(null);
      setChecking(false);
      setError(null);
      setIncludeInstall(false);
    }
    return () => { checkGeneration.current++; };
  }, [open]);

  const close = () => {
    checkGeneration.current++;
    onClose();
  };

  const candidate = candidates.find((c) => c.id === selectedId) ?? null;
  // Live status wins once we have re-probed; otherwise the snapshot.
  const current = live ?? candidate;
  const guide = current ? PROVIDER_SETUP[current.id] : undefined;

  // Stepper covers only the post-pick flow (pick screen has no stepper).
  // Provider choice already happened on the previous screen, so it is not
  // a stepper step — just Install (when needed) → Sign in → Done.
  const steps: { id: Exclude<Step, "pick">; label: string }[] = !includeInstall
    ? [
        { id: "login", label: "Sign in" },
        { id: "done", label: "Done" },
      ]
    : [
        { id: "install", label: "Install" },
        { id: "login", label: "Sign in" },
        { id: "done", label: "Done" },
      ];
  const stepIndex = Math.max(
    0,
    steps.findIndex((s) => s.id === step),
  );

  const pick = (id: string) => {
    if (id === "codex" && onCodexProfiles) { onCodexProfiles(); return; }
    if (id === "claude" && onClaudeProfiles) { onClaudeProfiles(); return; }
    checkGeneration.current++;
    setChecking(false);
    const c = candidates.find((x) => x.id === id) ?? null;
    setSelectedId(id);
    setLive(c);
    setError(null);
    setIncludeInstall(c ? !c.binaryFound : false);
    setStep(c && !c.binaryFound ? "install" : "login");
  };

  const backToPick = () => {
    checkGeneration.current++;
    setChecking(false);
    setStep("pick");
    setError(null);
  };

  const checkInstall = () => {
    if (!current) return;
    const ticket = ++checkGeneration.current;
    setChecking(true);
    setError(null);
    void onRefresh()
      .then((fresh) => {
        if (ticket !== checkGeneration.current) return;
        const now = fresh.find((c) => c.id === current.id);
        if (now) setLive(now);
        if (now?.binaryFound) {
          setStep("login");
        } else {
          setError(
            "CLI still not found — finish the install, then check again.",
          );
        }
      })
      .catch(() => { if (ticket === checkGeneration.current) setError("Couldn't re-check — try again."); })
      .finally(() => { if (ticket === checkGeneration.current) setChecking(false); });
  };

  const checkLogin = () => {
    if (!current) return;
    const ticket = ++checkGeneration.current;
    setChecking(true);
    setError(null);
    void onRefresh()
      .then(async (fresh) => {
        if (ticket !== checkGeneration.current) return;
        const now = fresh.find((c) => c.id === current.id);
        if (now) setLive(now);
        if (now?.binaryFound && now.loggedIn === true) {
          await onConnected?.(now.id);
          if (ticket !== checkGeneration.current) return;
          setStep("done");
          notify(`${now.label} connected`, "ok");
        } else if (now && !now.binaryFound) {
          setIncludeInstall(true);
          setStep("install");
          setError("CLI went missing — reinstall it first.");
        } else {
          setError(
            "Still not signed in — run the command above and try again.",
          );
        }
      })
      .catch(() => { if (ticket === checkGeneration.current) setError("Couldn't re-check — try again."); })
      .finally(() => { if (ticket === checkGeneration.current) setChecking(false); });
  };

  return (
    <AppModal
      open={open}
      onClose={close}
      title={title}
      cardClass="setup-card"
      onBack={step !== "pick" && step !== "done" ? backToPick : undefined}
      backLabel="All providers"
    >
      {step === "pick" ? <p className="provider-pick-intro">Choose a provider to add an account.</p> : null}
      {step === "pick" && onCodexProfiles && !candidates.some((c) => c.id === "codex") ? (
        <Button className="row profile-add-entry" onClick={onCodexProfiles}>
          <ProviderLogo provider="codex" />
          <span className="row-main"><span className="row-title">Codex</span><span className="row-sub">Import a saved ChatGPT login</span></span>
          <AddIcon size={20} />
        </Button>
      ) : null}
      {candidates.length === 0 && step === "pick" && !onCodexProfiles ? (
        <p className="m-empty">Everything is connected.</p>
      ) : (
        <>
          {step !== "pick" ? <SetupSteps labels={steps.map((s) => s.label)} current={stepIndex} label="Setup progress" /> : null}

          {step === "pick" ? (
            <div className="stack" role="group" aria-label="Choose a provider">
              {candidates.map((c) => (
                <Button
                  type="button"
                  key={c.id}
                  className={`row${c.id === selectedId ? " is-picked" : ""}`}
                  onClick={() => pick(c.id)}
                >
                  <ProviderLogo provider={c.id} />
                  <span className="row-main">
                    <span className="row-title">{c.label}</span>
                    <span className="row-sub">
                      {c.id === "codex" && onCodexProfiles ? "Import a saved ChatGPT login"
                        : c.id === "claude" && onClaudeProfiles ? "Sign in with a separate Claude profile"
                        : !c.binaryFound ? "Not installed" : c.loggedIn ? "Manage provider sign-in" : "Not logged in"}
                    </span>
                  </span>
                  <span className="row-glyph">
                    <AddIcon size={20} />
                  </span>
                </Button>
              ))}
            </div>
          ) : step === "done" && current ? (
            <div className="wizard-pane" key={step}>
              <div className="cli-head">
                <ProviderLogo provider={current.id} />
                <span className="row-main">
                  <span className="row-title">{current.label}</span>
                </span>
              </div>
              <p className="setting-note">
                {current.label} is now in the list on the main screen.
              </p>
              <div className="wizard-foot">
                <span />
                <Button size="sm" className="primary" onClick={close}>
                  Done
                </Button>
              </div>
            </div>
          ) : current ? (
            <div className="wizard-pane" key={step}>
              <div className="cli-head">
                <ProviderLogo provider={current.id} />
                <span className="row-main">
                  <span className="row-title">{current.label}</span>
                  <span className="row-sub">
                    {step === "install"
                      ? "Install the CLI"
                      : "Connect your account"}
                  </span>
                </span>
              </div>

              {step === "install" ? (
                <>
                  <ol className="setup-instructions">
                    {(
                      guide?.installSteps ?? [
                        "Follow the provider's installation guide, then check the installation here.",
                      ]
                    ).map((instruction) => (
                      <li key={instruction}>{instruction}</li>
                    ))}
                  </ol>
                  <CopyRow
                    label="Install"
                    text={guide?.installCommand ?? current.installUrl}
                    copiedMessage="Install command copied"
                    notify={notify}
                  />
                  {error ? (
                    <p className="wizard-error" role="alert">
                      {error}
                    </p>
                  ) : null}
                  <div className="wizard-foot is-end">
                    <Button
                      size="sm"
                      className="primary"
                      disabled={checking}
                      onClick={checkInstall}
                    >
                      {checking ? "Checking…" : "Check installation"}
                    </Button>
                  </div>
                </>
              ) : (
                <>
                  <ol className="setup-instructions">
                    {(
                      guide?.loginSteps ?? [
                        "Run the command in Terminal and complete the provider's sign-in, then check the connection here.",
                      ]
                    ).map((instruction) => (
                      <li key={instruction}>{instruction}</li>
                    ))}
                  </ol>
                  <CopyRow
                    label="Sign in"
                    text={current.loginCmd}
                    copiedMessage="Login command copied — run it in your terminal"
                    notify={notify}
                  />
                  {guide?.verifyCommand ? (
                    <details className="setup-help">
                      <summary>Check sign-in in Terminal</summary>
                      <CopyRow
                        label="Verify"
                        text={guide.verifyCommand}
                        copiedMessage="Status command copied"
                        notify={notify}
                      />
                    </details>
                  ) : null}
                  {error ? (
                    <p className="wizard-error" role="alert">
                      {error}
                    </p>
                  ) : null}
                  <div className="wizard-foot is-end">
                    <Button
                      size="sm"
                      className="primary"
                      disabled={checking}
                      onClick={checkLogin}
                    >
                      {checking ? "Checking…" : "I've signed in — check"}
                    </Button>
                  </div>
                </>
              )}
              <a
                className="setup-docs"
                href={
                  step === "install"
                    ? (guide?.installDocs ?? current.installUrl)
                    : (guide?.loginDocs ?? current.installUrl)
                }
                target="_blank"
                rel="noreferrer"
                onClick={(event) => {
                  event.preventDefault();
                  void openUrl(event.currentTarget.href).catch(() =>
                    notify(
                      "Couldn't open the guide. Copy its link and open it in your browser.",
                      "err",
                    ),
                  );
                }}
              >
                Official {current.label} guide
                <span aria-hidden="true">↗</span>
              </a>
            </div>
          ) : null}
        </>
      )}
    </AppModal>
  );
}
