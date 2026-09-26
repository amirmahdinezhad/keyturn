import { useEffect, useState } from "react";
import { Button, ComboBox, Input, Label, ListBox, ListBoxItem, TextField } from "@heroui/react";
import { prettyError, saveKey } from "../api";
import type { AppState } from "../types";
import { prettyProvider } from "../format";
import { ProviderLogo } from "../logos";
import { AppModal } from "./AppModal";
import type { ToastKind } from "./Toast";

interface AddKeyModalProps {
  open: boolean;
  onClose: () => void;
  providerIds: string[];
  /** Pre-selected provider/key name (e.g. picked from the signed-in account). */
  prefill?: { provider: string; name: string } | null;
  onSaved: (s: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
}

export function AddKeyModal({ open, onClose, providerIds, prefill, onSaved, notify }: AddKeyModalProps) {
  // providerId = picked provider (drives selectedKey); providerText = what the
  // input shows (drives filtering and custom typed ids).
  const [providerId, setProviderId] = useState("");
  const [providerText, setProviderText] = useState("");
  const [name, setName] = useState("");
  const [account, setAccount] = useState("");
  const [apiKey, setApiKey] = useState("");

  // Seed from the account picker when it opens with a prefill.
  useEffect(() => {
    if (!open || !prefill) return;
    setProviderId(prefill.provider);
    setProviderText(prettyProvider(prefill.provider));
    setName(prefill.name);
  }, [open, prefill]);

  const close = () => {
    setApiKey("");
    setName("");
    setAccount("");
    setProviderId("");
    setProviderText("");
    onClose();
  };

  const save = async () => {
    try {
      const next = await saveKey({
        provider: (providerId || providerText).trim(),
        name: name.trim(),
        apiKey: apiKey.trim(),
        account: account.trim(),
      });
      close();
      onSaved(next);
      notify("Key saved", "ok");
    } catch (e) {
      notify(prettyError(e), "err");
    }
  };

  return (
    <AppModal open={open} onClose={close} title="Add key">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <div className="field">
          <Label htmlFor="add-provider">Provider</Label>
          <ComboBox
            className="ak-select"
            aria-label="Provider"
            menuTrigger="focus"
            allowsCustomValue
            selectedKey={providerId || null}
            inputValue={providerText}
            onSelectionChange={(key) => {
              const id = key == null ? "" : String(key);
              setProviderId(id);
              setProviderText(id ? prettyProvider(id) : "");
            }}
            onInputChange={(text) => {
              setProviderText(text);
              // Typing means "custom id" unless it still matches the pick.
              if (providerId && text !== prettyProvider(providerId)) setProviderId("");
            }}
          >
            <ComboBox.InputGroup className="ak-select-trigger">
              {providerId && <ProviderLogo provider={providerId} />}
              <Input
                id="add-provider"
                className="ak-select-input"
                placeholder="Choose a provider"
                spellCheck={false}
                autoComplete="off"
              />
              <ComboBox.Trigger className="ak-select-indicator" aria-label="Toggle providers" />
            </ComboBox.InputGroup>
            <ComboBox.Popover className="ak-select-popover">
              <ListBox>
                {providerIds.map((id) => (
                  <ListBoxItem key={id} id={id} textValue={prettyProvider(id)}>
                    <span className="sel-content">
                      <ProviderLogo provider={id} />
                      <span className="sel-label">{prettyProvider(id)}</span>
                    </span>
                  </ListBoxItem>
                ))}
              </ListBox>
            </ComboBox.Popover>
          </ComboBox>
        </div>
        <TextField className="field" value={name} onChange={setName}>
          <Label>Name</Label>
          <Input id="add-name" placeholder="personal / work" spellCheck={false} />
        </TextField>
        <TextField className="field" value={account} onChange={setAccount}>
          <Label>Account</Label>
          <Input
            id="add-account"
            placeholder="you@example.com — optional"
            spellCheck={false}
          />
        </TextField>
        <TextField className="field" value={apiKey} onChange={setApiKey}>
          <Label>API key</Label>
          <Input
            id="add-key"
            type="password"
            placeholder="sk-…"
            spellCheck={false}
            autoComplete="off"
          />
        </TextField>
        <Button className="primary save-btn" type="submit">
          Save
        </Button>
      </form>
    </AppModal>
  );
}
