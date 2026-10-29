import { expect, it } from "vitest";
import { activeProviderAccountId } from "./accountActivity";
import { DEFAULT_WORKSPACE_USAGE, type Workspace } from "./types";
const workspace: Workspace = { id: "studio", name: "Studio", accent: "#007aff", accountIds: ["cx-loaded", "cx-backup"], claudeAccountIds: ["cl-loaded", "cl-backup"], antigravityAccountIds: ["google"], usage: { ...DEFAULT_WORKSPACE_USAGE, activeAccounts: { codex: "cx-loaded", claude: "cl-loaded", antigravity: "google" } } };
it("uses live Codex activity and independent defaults for other providers", () => {
  expect(activeProviderAccountId("codex", workspace, [{ id: "cx-backup" }, { id: "cx-loaded" }])).toBeNull();
  expect(activeProviderAccountId("claude", workspace, [{ id: "cl-backup" }, { id: "cl-loaded" }])).toBe("cl-loaded");
  expect(activeProviderAccountId("antigravity", workspace, [{ id: "google" }])).toBe("google");
});
it("prefers a current login and rejects accounts outside the provider membership", () => {
  expect(activeProviderAccountId("claude", workspace, [{ id: "cx-loaded", active: true }, { id: "cl-loaded" }, { id: "cl-backup", active: true }])).toBe("cl-backup");
  expect(activeProviderAccountId("antigravity", workspace, [{ id: "cl-loaded", active: true }])).toBeNull();
});
it("marks only the live Codex login across workspaces, including shared membership", () => {
  const accounts = [{ id: "cx-loaded", active: true }, { id: "cx-backup", active: false }];
  const other = { ...workspace, id: "other", accountIds: ["cx-backup"], usage: { ...workspace.usage, activeAccounts: { codex: "cx-backup" } } };
  expect(activeProviderAccountId("codex", workspace, accounts)).toBe("cx-loaded");
  expect(activeProviderAccountId("codex", other, accounts)).toBeNull();
  expect(activeProviderAccountId("codex", { ...other, accountIds: ["cx-backup", "cx-loaded"] }, accounts)).toBe("cx-loaded");
  expect(activeProviderAccountId("codex", workspace, accounts.map((a) => ({ ...a, active: false })))).toBeNull();
});
