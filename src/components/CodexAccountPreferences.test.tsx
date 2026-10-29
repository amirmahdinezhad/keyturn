import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { DEFAULT_CODEX_ACCOUNT_PREFERENCES, type AppState, type CodexAccountPreferences as Preferences, type CodexProfile } from "../types";
import { makeState } from "../test/fixtures";

const api = vi.hoisted(() => ({ codexProfileQuota: vi.fn(), setCodexAccountPreferences: vi.fn(), prettyError: (e: unknown) => String(e) }));
vi.mock("../api", () => api);
import { CodexAccountPreferences } from "./CodexAccountPreferences";

const work: CodexProfile = { id: "work", name: "Work", email: "work@example.com", plan: "pro", connected: true, connectionError: null, active: true, needsActivation: false, loginCommand: "codex login" };
const personal: CodexProfile = { ...work, id: "personal", name: "Personal", active: false };
const notify = vi.fn();
let persisted: AppState;
beforeEach(() => {
  vi.resetAllMocks();
  persisted = makeState();
  api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "Weekly", percent: 20 }, { window: "gpt-reserve", percent: 0 }] });
  api.setCodexAccountPreferences.mockImplementation(async (id: string, prefs: Preferences) => {
    persisted = { ...persisted, app: { ...persisted.app, codexAccounts: { ...persisted.app.codexAccounts, [id]: prefs } } };
    return persisted;
  });
});
afterEach(cleanup);
function setup(initial = makeState()) {
  persisted = initial;
  function Harness() {
    const [state, setState] = useState(initial);
    return <CodexAccountPreferences profile={work} profiles={[work, personal]} app={state.app} onChanged={setState} notify={notify} />;
  }
  return { user: userEvent.setup(), ...render(<Harness />) };
}

it("offers reported windows for this account without inferring five-hour support from its plan", async () => {
  const { user } = setup();
  const weekly = await screen.findByRole("switch", { name: "Show Weekly limit" });
  expect(weekly).not.toBeChecked();
  expect(screen.queryByRole("switch", { name: "Show 5-hour limit" })).not.toBeInTheDocument();
  expect(screen.queryByText("gpt-reserve")).not.toBeInTheDocument();
  await user.click(weekly);
  expect(api.setCodexAccountPreferences).toHaveBeenLastCalledWith("work", expect.objectContaining({ menuBarWindows: ["weekly"], knownWindows: ["weekly"] }));
  expect(weekly).toBeChecked();
});

it("changes only this account and keeps warning colors independent of native notifications", async () => {
  const sibling = { ...DEFAULT_CODEX_ACCOUNT_PREFERENCES, color: "#2f9e6e" };
  const { user } = setup(makeState({ app: { ...makeState().app, codexAccounts: { personal: sibling } } }));
  await screen.findByRole("switch", { name: "Show Weekly limit" });
  await user.click(screen.getByRole("button", { name: "Account color Orchid" }));
  expect(persisted.app.codexAccounts.work.color).toBe("#7c6cf0");
  expect(persisted.app.codexAccounts.personal).toEqual(sibling);
  await user.click(screen.getByRole("switch", { name: "Warning colors" }));
  expect(persisted.app.codexAccounts.work.warningColors).toBe(true);
  expect(screen.queryByRole("switch", { name: "System notifications" })).not.toBeInTheDocument();
  expect(screen.getByRole("switch", { name: "Warning colors" })).toBeChecked();
  expect(persisted.app.menuBarEnabled).toBe(false);
});

it("preserves selected windows and configuration during an outage", async () => {
  api.codexProfileQuota.mockRejectedValueOnce(new Error("Offline"));
  setup(makeState({ app: { ...makeState().app, codexAccounts: { work: { ...DEFAULT_CODEX_ACCOUNT_PREFERENCES, knownWindows: ["weekly"], menuBarWindows: ["weekly"] } } } }));
  expect(await screen.findByRole("switch", { name: "Show Weekly limit" })).toBeChecked();
  expect(await screen.findByText(/Temporarily unavailable/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Check limits again" })).toBeEnabled();
  expect(api.setCodexAccountPreferences).not.toHaveBeenCalled();
});

it("leaves saved choices intact after a failed save", async () => {
  api.setCodexAccountPreferences.mockRejectedValueOnce(new Error("Cannot save"));
  const { user } = setup();
  const weekly = await screen.findByRole("switch", { name: "Show Weekly limit" });
  await user.click(weekly);
  await waitFor(() => expect(notify).toHaveBeenCalledWith(expect.stringContaining("Cannot save"), "err"));
  expect(weekly).not.toBeChecked();
});

it("edits switch-cycle membership later without enrolling other accounts", async () => {
  const { user } = setup();
  await screen.findByRole("switch", { name: "Show Weekly limit" });
  const membership = screen.getByRole("switch", { name: "Include in switch cycle" });
  await user.click(membership);
  expect(persisted.app.codexAccounts.work.autoSwitch).toBe(true);
  expect(persisted.app.codexAccounts.personal).toBeUndefined();
  await user.click(membership);
  expect(persisted.app.codexAccounts.work.autoSwitch).toBe(false);
});

