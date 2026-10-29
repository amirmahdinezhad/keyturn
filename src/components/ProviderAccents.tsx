import { useState } from "react";
import { prettyError, setAppSettings } from "../api";
import { DEFAULT_ACCENT } from "../displayPrefs";
import type { AccentProvider, AppSettings, AppState } from "../types";
import { WorkspaceAccent } from "./WorkspaceSetup";
import { Select } from "./ui/select";
import { Button } from "./ui/button";
import type { ToastKind } from "./Toast";

const PROVIDERS = [
  { value: "codex", label: "Codex" }, { value: "claude", label: "Claude" },
  { value: "opencode", label: "OpenCode Zen" }, { value: "opencode-go", label: "OpenCode Go" },
  { value: "gemini", label: "Gemini" },
] as const;

/** Provider identity stays independent of workspace chrome and account overrides. */
export function ProviderAccents({ app, onChanged, notify }: {
  app: AppSettings; onChanged: (state: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
}) {
  const [provider, setProvider] = useState<AccentProvider>("codex");
  const [busy, setBusy] = useState(false);
  return <details className="provider-accents">
    <summary>Provider accents</summary>
    <div className="provider-accent-editor">
      <Select aria-label="Accent provider" value={provider} options={PROVIDERS} disabled={busy} onValueChange={(value) => setProvider(value as AccentProvider)} />
      <ProviderColor key={`${provider}:${app.providerAccents?.[provider]}`} provider={provider} app={app} onChanged={onChanged} notify={notify} busy={busy} setBusy={setBusy} />
    </div>
  </details>;
}

function ProviderColor({ provider, app, onChanged, notify, busy, setBusy }: {
  provider: AccentProvider; app: AppSettings; onChanged: (state: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
  busy: boolean; setBusy: (value: boolean) => void;
}) {
  const saved = app.providerAccents?.[provider] ?? DEFAULT_ACCENT;
  const [color, setColor] = useState(saved);
  return <form className="workspace-form provider-color-form" onSubmit={(event) => {
    event.preventDefault();
    if (busy || !/^#[\da-f]{6}$/i.test(color)) return;
    setBusy(true);
    void setAppSettings({ providerAccents: { ...app.providerAccents, [provider]: color.toLowerCase() } })
      .then(onChanged).catch((error) => notify(prettyError(error), "err")).finally(() => setBusy(false));
  }}>
    <WorkspaceAccent label={`${PROVIDERS.find((p) => p.value === provider)?.label} accent`} value={color} onChange={setColor} disabled={busy} />
    {provider === "codex" ? <p className="setting-note">New accounts use this color. Change a saved account’s accent in Workspace settings.</p> : null}
    <Button type="submit" size="sm" disabled={busy || color.toLowerCase() === saved || !/^#[\da-f]{6}$/i.test(color)}>Apply accent</Button>
  </form>;
}
