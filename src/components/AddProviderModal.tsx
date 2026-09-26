import { useEffect, useState } from "react";
import { Button, Input, Label, TextField } from "@heroui/react";
import { addProvider, prettyError } from "../api";
import type { AppState } from "../types";
import { prettyProvider } from "../format";
import { ProviderLogo } from "../logos";
import { AppModal } from "./AppModal";
import { SpinnerIcon } from "../icons";
import type { ToastKind } from "./Toast";

/** Common OpenCode provider ids the user may want to manage by hand. */
const SUGGESTIONS = [
  "mistral",
  "groq",
  "deepseek",
  "xai",
  "openrouter",
  "togetherai",
  "cerebras",
  "fireworks-ai",
];

interface AddProviderModalProps {
  open: boolean;
  onClose: () => void;
  onAdded: (s: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
}

/** "Add provider" — registers a provider that has no console we can read. */
export function AddProviderModal({ open, onClose, onAdded, notify }: AddProviderModalProps) {
  const [id, setId] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) setId("");
  }, [open]);

  const save = async () => {
    const value = id.trim();
    if (!value) return;
    setBusy(true);
    try {
      const next = await addProvider(value);
      onAdded(next);
      notify(`${prettyProvider(value)} added`, "ok");
      onClose();
    } catch (e) {
      notify(prettyError(e), "err");
    } finally {
      setBusy(false);
    }
  };

  return (
    <AppModal
      open={open}
      onClose={onClose}
      title="Add provider"
      bodyId="add-provider-body"
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <TextField className="field" value={id} onChange={setId}>
          <Label>Provider id</Label>
          <Input
            id="add-provider-id"
            placeholder="mistral"
            spellCheck={false}
            autoComplete="off"
          />
        </TextField>
        <p className="field-note">
          Use the id OpenCode knows (its auth.json key). Keys for it are managed by
          hand — no console sign-in.
        </p>
        <div className="suggest">
          {SUGGESTIONS.map((s) => (
            <button key={s} type="button" className="suggest-chip" onClick={() => setId(s)}>
              <ProviderLogo provider={s} />
              <span>{prettyProvider(s)}</span>
            </button>
          ))}
        </div>
        <Button
          className={`primary save-btn${busy ? " busy" : ""}`}
          type="submit"
          isDisabled={!id.trim() || busy}
        >
          {busy ? (
            <>
              <SpinnerIcon size={15} />
              Adding…
            </>
          ) : (
            "Add provider"
          )}
        </Button>
      </form>
    </AppModal>
  );
}
