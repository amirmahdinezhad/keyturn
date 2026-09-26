import { Button, Chip } from "@heroui/react";
import { AddIcon, InfoCircleIcon, LogoutIcon, NextIcon, SpinnerIcon, UsageIcon } from "../icons";
import { moneyPrecise, prettyProvider } from "../format";
import { ProviderLogo } from "../logos";

export interface ProviderOption {
  id: string;
  label: string;
  /** Sign-in flow available today. */
  ready: boolean;
}

/** Providers we can sign in to (ready) or manage through a local CLI file. */
export const PROVIDER_OPTIONS: ProviderOption[] = [
  { id: "opencode-go", label: "OpenCode", ready: true },
  { id: "anthropic", label: "Claude", ready: true },
  { id: "openai", label: "OpenAI", ready: true },
  { id: "google", label: "Gemini", ready: true },
  { id: "codex", label: "OpenAI Codex", ready: false },
];

interface ProviderPickerProps {
  /** Start a fresh sign-in (a new account) for this provider. */
  onPick: (id: string) => void;
  busyId: string | null;
  /** Providers the user added by hand (managed with manual keys). */
  customProviders: string[];
  /** Signed-in accounts, newest last. */
  accounts: { id: string; provider: string; label?: string | null }[];
  /** Account currently driving the desktop client. */
  activeId?: string | null;
  onResume: (accountId: string) => void;
  onPickCustom: (id: string) => void;
  onAddProvider: () => void;
}

/**
 * Account list: signed-in accounts first, then the providers you can connect,
 * then any hand-added provider. One row per thing, action on the right.
 */
export function ProviderPicker({
  onPick,
  busyId,
  customProviders,
  accounts,
  activeId,
  onResume,
  onPickCustom,
  onAddProvider,
}: ProviderPickerProps) {
  return (
    <div className="picker">
      {accounts.length > 0 && (
        <section className="stack-group">
          <p className="section-label">Signed in</p>
          <div className="stack">
            {accounts.map((a) => (
              <button
                type="button"
                key={a.id}
                className="row"
                onClick={() => onResume(a.id)}
              >
                <ProviderLogo provider={a.provider} />
                <span className="row-main">
                  <span className="row-title">
                    {PROVIDER_OPTIONS.find((p) => p.id === a.provider)?.label ??
                      prettyProvider(a.provider)}
                  </span>
                  <span className="row-sub">{a.label ?? "Signed in"}</span>
                </span>
                {activeId === a.id && <Chip className="mini-chip on">Active</Chip>}
                <NextIcon size={18} className="row-chevron" />
              </button>
            ))}
          </div>
        </section>
      )}

      <section className="stack-group">
        <p className="section-label">
          {accounts.length > 0 ? "Add account" : "Connect an account"}
        </p>
        <div className="stack">
          {PROVIDER_OPTIONS.filter((p) => p.ready).map((p) => {
            const connected = accounts.some((a) => a.provider === p.id);
            const busy = busyId === p.id;
            return (
              <button
                type="button"
                key={p.id}
                className="row"
                disabled={busyId !== null}
                onClick={() => onPick(p.id)}
              >
                <ProviderLogo provider={p.id} />
                <span className="row-main">
                  <span className="row-title">{p.label}</span>
                  <span className="row-sub">
                    {busy
                      ? "Signing in…"
                      : connected
                        ? "Connect another account"
                        : "Sign in to connect"}
                  </span>
                </span>
                {busy ? (
                  <span className="row-busy" role="status" aria-label="Signing in">
                    <SpinnerIcon size={18} />
                  </span>
                ) : (
                  <AddIcon size={18} className="row-action" />
                )}
              </button>
            );
          })}
        </div>
      </section>

      {PROVIDER_OPTIONS.some((p) => !p.ready) && (
        <section className="stack-group">
          <p className="section-label">Local CLI accounts</p>
          <div className="stack">
            {PROVIDER_OPTIONS.filter((p) => !p.ready).map((p) => (
              <button
                type="button"
                key={p.id}
                className="row"
                onClick={() => onPickCustom(p.id)}
              >
                <ProviderLogo provider={p.id} />
                <span className="row-main">
                  <span className="row-title">{p.label}</span>
                  <span className="row-sub">
                    Reads and patches ~/.codex/auth.json
                  </span>
                </span>
                <NextIcon size={18} className="row-chevron" />
              </button>
            ))}
          </div>
        </section>
      )}

      <section className="stack-group">
        <p className="section-label">Manual keys</p>
        <div className="stack">
            {customProviders.map((id) => (
              <button
                type="button"
                key={id}
                className="row"
                onClick={() => onPickCustom(id)}
              >
                <ProviderLogo provider={id} />
                <span className="row-main">
                  <span className="row-title">{prettyProvider(id)}</span>
                  <span className="row-sub">Keys stored on this Mac</span>
                </span>
                <NextIcon size={18} className="row-chevron" />
              </button>
            ))}
            <button type="button" className="row dashed" onClick={onAddProvider}>
              <span className="row-glyph">
                <AddIcon size={18} />
              </span>
              <span className="row-main">
                <span className="row-title">Add provider</span>
                <span className="row-sub">Track a provider by hand</span>
              </span>
            </button>
        </div>
      </section>
    </div>
  );
}

/** Three-dot pulse + label used while a value is still loading. */
function ThreeDots({ label = "Loading…" }: { label?: string }) {
  return (
    <span className="loading-dots">
      <span className="dots" role="status" aria-label={label}>
        <i />
        <i />
        <i />
      </span>
      <span className="dots-label">{label}</span>
    </span>
  );
}

interface AccountKeyListProps {
  keys: { id: string; name?: string }[];
  provider: string;
  providerLabel: string;
  /** Signed-in account email, read from the console payload. */
  email?: string | null;
  /** cost per console key id for the selected month */
  costByKey?: Record<string, number>;
  /** Costs are still being fetched for the selected month. */
  loading?: boolean;
  activeName?: string | null;
  busyId: string | null;
  /** True while the current month is selected (only then can keys change). */
  browsing: boolean;
  /** Quota windows exist for this account; opens the limits modal. */
  usageAvailable: boolean;
  onUse: (keyId: string) => void;
  onShowModels: (keyId: string) => void;
  onShowUsage: () => void;
  /** Back to the account list. */
  onSwitch: () => void;
  onLogout: () => void;
}

/** After signing in: the account's keys for the selected month. */
export function AccountKeyList({
  keys,
  provider,
  providerLabel,
  email,
  costByKey = {},
  loading = false,
  activeName,
  busyId,
  browsing,
  usageAvailable,
  onUse,
  onShowModels,
  onShowUsage,
  onSwitch,
  onLogout,
}: AccountKeyListProps) {
  return (
    <div className="picker">
      <div className="account-panel">
        <div className="account-head">
          <ProviderLogo provider={provider} />
          <div className="account-info">
            <span className="account-name">{providerLabel}</span>
            {email && <span className="account-email">{email}</span>}
          </div>
          {usageAvailable && (
            <Button
              isIconOnly
              size="sm"
              className="disc icon-only"
              aria-label="Usage limits"
              onPress={onShowUsage}
            >
              <UsageIcon size={18} />
            </Button>
          )}
          <Button size="sm" className="disc" onPress={onSwitch}>
            Switch
          </Button>
          <Button
            isIconOnly
            size="sm"
            className="disc logout-btn icon-only"
            aria-label="Log out"
            onPress={onLogout}
          >
            <LogoutIcon size={18} />
          </Button>
        </div>
        {keys.length === 0 && (
          <p className="account-hint">
            No keys in this account yet — create one in the OpenCode console to
            get started.
          </p>
        )}
        <div className="account-keys">
          {keys.map((k) => {
            const label = k.name ?? k.id;
            const cost = costByKey[k.id];
            const isActive = activeName === label;
            const idTail =
              k.id.length > 16 ? `${k.id.slice(0, 12)}…` : k.id;
            return (
              <div
                key={k.id}
                className="ledger-row"
                role="button"
                tabIndex={0}
                aria-label={`Cost detail for ${label}`}
                onClick={() => onShowModels(k.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onShowModels(k.id);
                  }
                }}
              >
                <span className="ledger-main">
                  <span className="ledger-title">{label}</span>
                  <span className="ledger-sub">{idTail}</span>
                </span>
                <span className="ledger-cost">
                  {loading ? (
                    <ThreeDots label="Loading…" />
                  ) : cost !== undefined ? (
                    moneyPrecise(cost)
                  ) : (
                    <span className="ledger-cost none">—</span>
                  )}
                </span>
                <button
                  type="button"
                  className="info-ghost"
                  aria-label={`Cost detail for ${label}`}
                  title="Cost detail"
                  onClick={(e) => {
                    e.stopPropagation();
                    onShowModels(k.id);
                  }}
                >
                  <InfoCircleIcon size={18} />
                </button>
                {browsing &&
                  (isActive ? (
                    <Chip className="mini-chip on">Active</Chip>
                  ) : (
                    <span
                      onClick={(e) => e.stopPropagation()}
                      onKeyDown={(e) => e.stopPropagation()}
                    >
                      <Button
                        size="sm"
                        className={`primary use-btn${busyId === k.id ? " busy" : ""}`}
                        isDisabled={busyId !== null}
                        onPress={() => onUse(k.id)}
                      >
                        {busyId === k.id ? (
                          <>
                            <SpinnerIcon size={14} />
                            Applying…
                          </>
                        ) : (
                          "Use"
                        )}
                      </Button>
                    </span>
                  ))}
              </div>
            );
          })}
        </div>
      </div>
      <p className="picker-note">
        {browsing
          ? "Pick the key the desktop client should use. Usage is read from your account for the selected month."
          : "Past months are read-only — go back to the current month to change the active key."}
      </p>
    </div>
  );
}
