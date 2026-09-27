import { Chip } from "@heroui/react";
import type { ProviderCliStatus } from "../api";
import { ProviderLogo } from "../logos";
import type { ToastKind } from "./Toast";

interface CliProvidersProps {
  statuses: ProviderCliStatus[] | null;
  notify: (message: string, kind?: ToastKind) => void;
}

type StatusKind = "missing" | "logged-in" | "logged-out" | "unknown";

interface StatusView {
  kind: StatusKind;
  pill: string;
  pillClass: string;
  subtitle: string;
}

/// One cascade from CLI state to everything the card header shows, so the
/// pill and the subtitle can never disagree with each other.
function describe(status: ProviderCliStatus): StatusView {
  if (!status.binaryFound) {
    return {
      kind: "missing",
      pill: "Not installed",
      pillClass: "mini-chip off",
      subtitle: "CLI not found on PATH",
    };
  }
  if (status.loggedIn === true) {
    return {
      kind: "logged-in",
      pill: "Logged in",
      pillClass: "mini-chip on",
      subtitle:
        status.detail ?? status.email ?? status.version ?? status.binaryPath ?? "Signed in",
    };
  }
  if (status.loggedIn === false) {
    return {
      kind: "logged-out",
      pill: "Not logged in",
      pillClass: "mini-chip warn",
      subtitle: status.detail ?? "Not logged in",
    };
  }
  return {
    kind: "unknown",
    pill: "Unknown",
    pillClass: "mini-chip off",
    subtitle: status.version ?? status.binaryPath ?? "Status unknown",
  };
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard?.writeText(text);
    return true;
  } catch {
    return false;
  }
}

function CopyRow({
  text,
  copiedMessage,
  notify,
}: {
  text: string;
  copiedMessage: string;
  notify: (message: string, kind?: ToastKind) => void;
}) {
  return (
    <div className="cmd-row">
      <code>{text}</code>
      <button
        type="button"
        className="link-btn"
        onClick={() => {
          void copyText(text).then((ok) =>
            notify(
              ok ? copiedMessage : "Copy failed — select the text manually",
              ok ? "ok" : "err",
            ),
          );
        }}
      >
        Copy
      </button>
    </div>
  );
}

/**
 * CLI-only provider list (T3-style): Keyturn never signs in. Each card shows
 * the CLI's own status and the exact command the user runs in their terminal.
 */
export function CliProviders({ statuses, notify }: CliProvidersProps) {
  if (!statuses) return null;
  return (
    <div className="picker">
      <section className="stack-group">
        <p className="section-label">Providers</p>
        <div className="stack">
          {statuses.map((s) => {
            const view = describe(s);
            return (
              <div className="cli-card" key={s.id}>
                <div className="cli-head">
                  <ProviderLogo provider={s.id} />
                  <span className="row-main">
                    <span className="row-title">
                      {s.label} <Chip className={view.pillClass}>{view.pill}</Chip>
                    </span>
                    <span className="row-sub">{view.subtitle}</span>
                  </span>
                </div>

                <div className="cli-meta">
                  {s.version && <span className="cli-meta-line">{s.version}</span>}
                  {s.binaryPath && <span className="cli-meta-line">{s.binaryPath}</span>}
                  {s.email && s.loggedIn && <span className="cli-meta-line">{s.email}</span>}
                  {s.authMethod && s.loggedIn && (
                    <span className="cli-meta-line">via {s.authMethod}</span>
                  )}
                  {s.connected && s.connected.length > 0 && (
                    <span className="cli-connected">
                      {s.connected.map((c) => (
                        <Chip key={c} className="mini-chip on">
                          {c}
                        </Chip>
                      ))}
                    </span>
                  )}
                </div>

                {view.kind === "missing" ? (
                  <>
                    <p className="cli-hint">
                      Install the {s.label} CLI first, then sign in with:
                    </p>
                    <CopyRow text={s.installUrl} copiedMessage="Install link copied" notify={notify} />
                    <CopyRow
                      text={s.loginCmd}
                      copiedMessage="Login command copied — run it after installing"
                      notify={notify}
                    />
                  </>
                ) : (
                  <CopyRow
                    text={s.loginCmd}
                    copiedMessage="Login command copied — run it in your terminal"
                    notify={notify}
                  />
                )}
              </div>
            );
          })}
        </div>
      </section>
      <p className="picker-note">
        Keyturn never signs in for you — run the command in your own terminal,
        then press Refresh to update the status.
      </p>
      <p className="picker-note">
        Anthropic, OpenAI and Google keys without their own CLI are managed
        through OpenCode: <code>opencode auth login anthropic</code>,{" "}
        <code>opencode auth login openai</code>,{" "}
        <code>opencode auth login google</code>.
      </p>
    </div>
  );
}
