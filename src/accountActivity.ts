import type { AccountProvider } from "./api";
import type { Workspace } from "./types";

/** Codex activity reflects its live login; other providers retain workspace defaults. */
export function activeProviderAccountId(provider: AccountProvider, workspace: Workspace, accounts: readonly { id: string; active?: boolean }[]): string | null {
  const ids = provider === "codex" ? workspace.accountIds : provider === "claude" ? workspace.claudeAccountIds : workspace.antigravityAccountIds;
  const members = accounts.filter((account) => ids == null || ids.includes(account.id));
  const current = members.find((account) => account.active);
  if (provider === "codex") return current?.id ?? null;
  const saved = workspace.usage.activeAccounts?.[provider];
  return current?.id ?? members.find((account) => account.id === saved)?.id ?? members[0]?.id ?? null;
}
