import { useState } from "react";
import { Button, Chip } from "@heroui/react";
import type { AppState } from "../types";
import { mask, moneyPrecise, prettyProvider } from "../format";
import { ProviderLogo } from "../logos";
import {
  AddIcon,
  CopyIcon,
  EyeIcon,
  EyeOffIcon,
  InfoCircleIcon,
  SpinnerIcon,
  TickIcon,
  TrashIcon,
} from "../icons";

interface ManualKeysProps {
  provider: string;
  state: AppState;
  browsing: boolean;
  busyName: string | null;
  onUse: (name: string) => void;
  onAdd: () => void;
  onDelete: (name: string) => void;
  onShowModels: (name: string) => void;
  onRemoveProvider: () => void;
  onBack: () => void;
  /** Hand-added providers can be removed; built-in ones cannot. */
  removable?: boolean;
}

/** Keys for a hand-added provider: reveal/copy secrets, activate, delete. */
export function ManualKeys({
  provider,
  state,
  browsing,
  busyName,
  onUse,
  onAdd,
  onDelete,
  onShowModels,
  onRemoveProvider,
  onBack,
  removable = true,
}: ManualKeysProps) {
  const entry = state.providers[provider];
  const names = entry ? Object.keys(entry.keys).sort() : [];
  const usage = state.usage.byKey[provider] ?? {};
  const active = state.applied[provider] ?? null;
  const [revealed, setRevealed] = useState<Record<string, boolean>>({});
  const [copied, setCopied] = useState<string | null>(null);

  const copyKey = async (name: string, value: string) => {
    try {
      await navigator.clipboard?.writeText(value);
      setCopied(name);
      window.setTimeout(() => setCopied((c) => (c === name ? null : c)), 1400);
    } catch {
      /* clipboard unavailable */
    }
  };

  return (
    <div className="picker">
      <div className="account-panel">
        <div className="account-head">
          <ProviderLogo provider={provider} />
          <div className="account-info">
            <span className="account-name">{prettyProvider(provider)}</span>
            <span className="account-email">
              {names.length === 1 ? "1 key" : `${names.length} keys`} · managed by hand
            </span>
          </div>
          <Button size="sm" className="disc" onPress={onBack}>
            Back
          </Button>
        </div>
        {names.length === 0 && (
          <p className="account-hint">
            No keys yet — add one to use this provider with the desktop client.
          </p>
        )}
        <div className="account-keys">
          {names.map((name) => {
            const cred = entry?.keys[name];
            const bucket = usage[name];
            const isActive = active === name;
            const isRevealed = revealed[name] === true;
            return (
              <div
                key={name}
                className="ledger-row manual"
                role="button"
                tabIndex={0}
                aria-label={`Cost detail for ${name}`}
                onClick={() => onShowModels(name)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onShowModels(name);
                  }
                }}
              >
                <div className="ledger-top">
                  <span className="ledger-title">{name}</span>
                  <span className="ledger-cost">
                    {bucket ? moneyPrecise(bucket.total) : "—"}
                  </span>
                  <button
                    type="button"
                    className="info-ghost"
                    aria-label={`Cost detail for ${name}`}
                    title="Cost detail"
                    onClick={(e) => {
                      e.stopPropagation();
                      onShowModels(name);
                    }}
                  >
                    <InfoCircleIcon size={18} />
                  </button>
                  <button
                    type="button"
                    className="info-ghost danger"
                    aria-label={`Delete ${name}`}
                    title="Delete key"
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete(name);
                    }}
                  >
                    <TrashIcon size={18} />
                  </button>
                </div>
                <div className="ledger-bottom">
                  <span className="secret" title={isRevealed ? undefined : "Hidden"}>
                    {cred
                      ? isRevealed
                        ? cred.key
                        : mask(cred.key)
                      : "•••"}
                  </span>
                  <button
                    type="button"
                    className="info-ghost"
                    aria-label={isRevealed ? "Hide key" : "Reveal key"}
                    title={isRevealed ? "Hide key" : "Reveal key"}
                    onClick={(e) => {
                      e.stopPropagation();
                      setRevealed((r) => ({ ...r, [name]: !isRevealed }));
                    }}
                  >
                    {isRevealed ? <EyeOffIcon size={18} /> : <EyeIcon size={18} />}
                  </button>
                  <button
                    type="button"
                    className="info-ghost"
                    aria-label={`Copy ${name}`}
                    title="Copy key"
                    onClick={(e) => {
                      e.stopPropagation();
                      if (cred) void copyKey(name, cred.key);
                    }}
                  >
                    {copied === name ? <TickIcon size={18} /> : <CopyIcon size={18} />}
                  </button>
                  <span
                    className="row-tail"
                    onClick={(e) => e.stopPropagation()}
                    onKeyDown={(e) => e.stopPropagation()}
                  >
                    {browsing &&
                      (isActive ? (
                        <Chip className="mini-chip on">Active</Chip>
                      ) : (
                        <Button
                          size="sm"
                          className={`primary use-btn${busyName === name ? " busy" : ""}`}
                          isDisabled={busyName !== null}
                          onPress={() => onUse(name)}
                        >
                          {busyName === name ? (
                            <>
                              <SpinnerIcon size={14} />
                              Applying…
                            </>
                          ) : (
                            "Use"
                          )}
                        </Button>
                      ))}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      </div>
      <div className="manual-actions">
        <Button size="sm" className="disc" onPress={onAdd}>
          <AddIcon size={17} />
          Add key
        </Button>
        {removable ? (
          <button type="button" className="link-btn danger" onClick={onRemoveProvider}>
            Remove provider
          </button>
        ) : null}
      </div>
      <p className="picker-note">
        Costs come from your local OpenCode usage database for the selected month.
      </p>
    </div>
  );
}
