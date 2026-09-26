import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  saveKey: vi.fn(),
  deleteKey: vi.fn(),
  applyKey: vi.fn(),
  addProvider: vi.fn(),
  removeProvider: vi.fn(),
  setSettings: vi.fn(),
  setAppSettings: vi.fn(),
  profileCreate: vi.fn(),
  profileRename: vi.fn(),
  profileSwitch: vi.fn(),
  profileDelete: vi.fn(),
  listSounds: vi.fn(),
  playSound: vi.fn(),
  codexStatus: vi.fn(),
  codexRefresh: vi.fn(),
  history: vi.fn(),
  exportHistory: vi.fn(),
  checkUpdates: vi.fn(),
  accountStatus: vi.fn(),
  connectAccount: vi.fn(),
  disconnectAccount: vi.fn(),
  setActiveAccount: vi.fn(),
  consoleKeys: vi.fn(),
  consoleUsage: vi.fn(),
  activateAccountKey: vi.fn(),
  quitApp: vi.fn(),
  prettyError: vi.fn((e: unknown) => (e instanceof Error ? e.message : String(e))),
}));

vi.mock("./api", () => api);

import App from "./App";
import { makeState } from "./test/fixtures";

const CURRENT_MONTH = (() => {
  const d = new Date();
  return { year: d.getFullYear(), month: d.getMonth() };
})();

const PREVIOUS_MONTH = (() => {
  const d = new Date(CURRENT_MONTH.year, CURRENT_MONTH.month - 1, 1);
  return { year: d.getFullYear(), month: d.getMonth() };
})();

beforeEach(() => {
  vi.clearAllMocks();
  api.getState.mockResolvedValue(makeState({ month: CURRENT_MONTH }));
  api.accountStatus.mockResolvedValue({ accounts: [], activeAccount: null });
  api.listSounds.mockResolvedValue([]);
  api.checkUpdates.mockResolvedValue({ available: false });
  api.codexStatus.mockResolvedValue({ status: { present: false, hasApiKey: false, hasTokens: false }, keys: [] });
});

afterEach(cleanup);

describe("App", () => {
  it("shows the loading skeleton until the first state arrives", () => {
    api.getState.mockReturnValue(new Promise(() => {}));
    render(<App />);
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument();
  });

  it("loads the state and renders the connect list", async () => {
    render(<App />);

    expect(await screen.findByText("Connect an account")).toBeInTheDocument();
    const titles = (section: Element) =>
      Array.from(section.querySelectorAll(".row-title"), (el) => el.textContent);

    const connect = screen.getByText("Connect an account").closest("section")!;
    expect(titles(connect)).toEqual(["OpenCode", "Claude", "OpenAI", "Gemini"]);
    const local = screen.getByText("Local CLI accounts").closest("section")!;
    expect(titles(local)).toEqual(["OpenAI Codex"]);
    expect(screen.getByText("Add provider")).toBeInTheDocument();
  });

  it("surfaces a load failure instead of an empty panel", async () => {
    api.getState.mockRejectedValue(new Error("permission denied"));
    render(<App />);

    expect(await screen.findByText("Couldn't load keys: permission denied")).toBeInTheDocument();
  });

  it("re-fetches the current month with refresh when the refresh button is used", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Connect an account");

    expect(api.getState).toHaveBeenCalledWith(CURRENT_MONTH, false);
    await user.click(screen.getByLabelText("Refresh data"));

    await waitFor(() => {
      expect(api.getState).toHaveBeenCalledWith(CURRENT_MONTH, true);
    });
  });

  it("signs in, then shows that account's keys with the active one marked", async () => {
    const user = userEvent.setup();
    api.connectAccount.mockResolvedValue({
      accounts: [{ id: "opencode-go~a@b.c", provider: "opencode-go", label: "a@b.c" }],
      activeAccount: "opencode-go~a@b.c",
    });
    api.consoleKeys.mockResolvedValue({
      keys: [
        { id: "key_really_long_identifier_1", name: "side-project" },
        { id: "key_really_long_identifier_2", name: "work" },
      ],
      month: CURRENT_MONTH,
      email: "a@b.c",
    });
    api.consoleUsage.mockResolvedValue({
      keys: [],
      usage: [{ model: "claude-sonnet-4", keyId: "key_really_long_identifier_1", cost: 12.5 }],
      go: [],
    });

    render(<App />);
    await screen.findByText("Connect an account");

    await user.click(screen.getByText("OpenCode").closest("button")!);

    expect(api.connectAccount).toHaveBeenCalledWith("opencode-go", null);
    const projectRow = (await screen.findByText("side-project")).closest(".ledger-row")!;
    expect(within(projectRow as HTMLElement).getByText("$12.5")).toBeInTheDocument();
    expect(screen.getByText("work")).toBeInTheDocument();
    expect(screen.getByText("a@b.c")).toBeInTheDocument();
  });

  it("walks back through months from the account view", async () => {
    const user = userEvent.setup();
    api.accountStatus.mockResolvedValue({
      accounts: [{ id: "opencode-go~a@b.c", provider: "opencode-go", label: "a@b.c" }],
      activeAccount: "opencode-go~a@b.c",
    });
    api.consoleKeys.mockResolvedValue({ keys: [], month: CURRENT_MONTH, email: null });
    api.consoleUsage.mockResolvedValue({ keys: [], usage: [], go: [] });

    render(<App />);
    await screen.findByLabelText("Previous month");

    await user.click(screen.getByLabelText("Previous month"));

    await waitFor(() => {
      expect(api.getState).toHaveBeenCalledWith(PREVIOUS_MONTH, false);
    });
  });

  it("blocks moving forward past the current month", async () => {
    api.accountStatus.mockResolvedValue({
      accounts: [{ id: "opencode-go~a@b.c", provider: "opencode-go", label: "a@b.c" }],
      activeAccount: "opencode-go~a@b.c",
    });
    api.consoleKeys.mockResolvedValue({ keys: [], month: CURRENT_MONTH, email: null });
    api.consoleUsage.mockResolvedValue({ keys: [], usage: [], go: [] });

    render(<App />);
    await screen.findByLabelText("Next month");

    expect(screen.getByLabelText("Next month")).toBeDisabled();
    const next = (() => {
      const d = new Date(CURRENT_MONTH.year, CURRENT_MONTH.month + 1, 1);
      return { year: d.getFullYear(), month: d.getMonth() };
    })();
    expect(api.getState).not.toHaveBeenCalledWith(next, expect.anything());
  });
});
