import { useEffect, useState } from "react";
import { Button } from "./ui/button";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ProviderCliStatus } from "../api";
import { ProviderLogo } from "../logos";
import { AddIcon } from "../icons";
import { AppModal } from "./AppModal";
import { CopyRow } from "./CliProviders";
import { PROVIDER_SETUP } from "../providerSetup";
import type { ToastKind } from "./Toast";

interface AddAccountModalProps {
  open: boolean;
  /** Providers that are not connected (missing binary or not logged in). */
  candidates: ProviderCliStatus[];
  onClose: () => void;
  /** Re-read CLI status; resolves with the fresh list. */
  onRefresh: () => Promise<ProviderCliStatus[]>;
  notify: (message: string, kind?: ToastKind) => void;
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
  notify,
}: AddAccountModalProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [step, setStep] = useState<Step>("pick");
  const [live, setLive] = useState<ProviderCliStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [includeInstall, setIncludeInstall] = useState(false);

  // Fresh open resets the wizard.
  useEffect(() => {
    if (open) {
      setSelectedId(null);
      setStep("pick");
      setLive(null);
      setChecking(false);
      setError(null);
      setIncludeInstall(false);
    }
  }, [open]);

  const close = () => {
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
    const c = candidates.find((x) => x.id === id) ?? null;
    setSelectedId(id);
    setLive(c);
    setError(null);
    setIncludeInstall(c ? !c.binaryFound : false);
    setStep(c && !c.binaryFound ? "install" : "login");
  };

  const backToPick = () => {
    setStep("pick");
    setError(null);
  };

  const checkInstall = () => {
    if (!current) return;
    setChecking(true);
    setError(null);
    void onRefresh()
      .then((fresh) => {
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
      .catch(() => setError("Couldn't re-check — try again."))
      .finally(() => setChecking(false));
  };

  const checkLogin = () => {
    if (!current) return;
    setChecking(true);
    setError(null);
    void onRefresh()
      .then((fresh) => {
        const now = fresh.find((c) => c.id === current.id);
        if (now) setLive(now);
        if (now?.binaryFound && now.loggedIn === true) {
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
      .catch(() => setError("Couldn't re-check — try again."))
      .finally(() => setChecking(false));
  };

  return (
    <AppModal
      open={open}
      onClose={close}
      title="Add account"
      cardClass="setup-card"
      onBack={step !== "pick" ? backToPick : undefined}
      backLabel="All providers"
    >
      {candidates.length === 0 && step === "pick" ? (
        <p className="m-empty">Everything is connected.</p>
      ) : (
        <>
          {step !== "pick" ? (
            <ol className="wizard-steps" aria-label="Setup progress">
              {steps.map((s, i) => (
                <li
                  key={s.id}
                  className={`wizard-step${i < stepIndex ? " is-done" : ""}${i === stepIndex ? " is-now" : ""}`}
                  aria-current={i === stepIndex ? "step" : undefined}
                >
                  <span className="wizard-dot" aria-hidden="true">
                    {i < stepIndex ? "✓" : `${i + 1}`}
                  </span>
                  <span className="wizard-label">{s.label}</span>
                  {i < steps.length - 1 ? (
                    <span className="wizard-line" aria-hidden="true" />
                  ) : null}
                </li>
              ))}
            </ol>
          ) : null}

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
                      {!c.binaryFound ? "Not installed" : "Not logged in"}
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
