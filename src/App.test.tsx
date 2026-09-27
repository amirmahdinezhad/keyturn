import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  setSettings: vi.fn(),
  setAppSettings: vi.fn(),
  profileCreate: vi.fn(),
  profileRename: vi.fn(),
  profileSwitch: vi.fn(),
  profileDelete: vi.fn(),
  history: vi.fn(),
  exportHistory: vi.fn(),
  checkUpdates: vi.fn(),
  cliStatus: vi.fn(),
  quitApp: vi.fn(),
  prettyError: vi.fn((e: unknown) => (e instanceof Error ? e.message : String(e))),
}));

vi.mock("./api", () => api);

import App from "./App";
import { makeState } from "./test/fixtures";

function cliFixture() {
  return [
    {
      id: "claude",
      label: "Claude",
      binary: "claude",
      binaryFound: true,
      binaryPath: "/opt/homebrew/bin/claude",
      version: "2.1.220",
      loggedIn: true,
      authMethod: "claude.ai",
      email: "dev@example.com",
      account: null,
      detail: "Logged in via claude.ai",
      loginCmd: "claude auth login",
      installUrl: "https://claude.com/product/claude-code",
      authFile: null,
      connected: [],
    },
    {
      id: "codex",
      label: "OpenAI Codex",
      binary: "codex",
      binaryFound: false,
      binaryPath: null,
      version: null,
      loggedIn: null,
      authMethod: null,
      email: null,
      account: null,
      detail: null,
      loginCmd: "codex login",
      installUrl: "https://developers.openai.com/codex/cli",
      authFile: "~/.codex/auth.json",
      connected: [],
    },
  ];
}

beforeEach(() => {
  vi.clearAllMocks();
  api.getState.mockResolvedValue(makeState());
  api.cliStatus.mockResolvedValue(cliFixture());
  api.checkUpdates.mockResolvedValue({ available: false });
});

afterEach(cleanup);

describe("App", () => {
  it("shows the loading skeleton until the first state arrives", () => {
    api.getState.mockReturnValue(new Promise(() => {}));
    render(<App />);
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument();
  });

  it("loads the CLI status and renders one card per provider", async () => {
    const { container } = render(<App />);

    expect(await screen.findByText("Providers")).toBeInTheDocument();
    // SVG brand marks carry their own <title>, so read the card headers.
    const titles = Array.from(
      container.querySelectorAll(".cli-card .row-title"),
      (el) => el.textContent,
    );
    expect(titles).toHaveLength(2);
    expect(titles[0]).toContain("Claude");
    expect(titles[1]).toContain("OpenAI Codex");
    // The exact terminal command is shown for copy-paste, never executed here.
    expect(screen.getByText("claude auth login")).toBeInTheDocument();
    expect(screen.getByText("codex login")).toBeInTheDocument();
  });

  it("marks logged-in and missing CLIs distinctly", async () => {
    render(<App />);

    expect(await screen.findByText("Logged in")).toBeInTheDocument();
    expect(screen.getByText("Not installed")).toBeInTheDocument();
    expect(screen.getByText("CLI not found on PATH")).toBeInTheDocument();
  });

  it("surfaces a load failure instead of an empty panel", async () => {
    api.getState.mockRejectedValue(new Error("permission denied"));
    render(<App />);

    expect(await screen.findByText("Couldn't load settings: permission denied")).toBeInTheDocument();
  });

  it("surfaces a CLI probe failure instead of an empty panel", async () => {
    api.cliStatus.mockRejectedValue(new Error("spawn failed"));
    render(<App />);

    expect(await screen.findByText("Couldn't read CLI status: spawn failed")).toBeInTheDocument();
  });

  it("re-reads state and CLI status when the refresh button is used", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Providers");

    expect(api.getState).toHaveBeenCalledTimes(1);
    expect(api.cliStatus).toHaveBeenCalledTimes(1);
    await user.click(screen.getByLabelText("Refresh data"));

    await waitFor(() => {
      expect(api.getState).toHaveBeenCalledTimes(2);
      expect(api.cliStatus).toHaveBeenCalledTimes(2);
    });
  });
});
