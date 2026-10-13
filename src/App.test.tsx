import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  setSettings: vi.fn(),
  setAppSettings: vi.fn(),
  cliStatus: vi.fn(),
  cliQuotas: vi.fn(async (): Promise<QuotaSet[]> => []),
  quitApp: vi.fn(),
  prettyError: vi.fn((e: unknown) => (e instanceof Error ? e.message : String(e))),
}));

vi.mock("./api", () => api);

import App from "./App";
import { makeState } from "./test/fixtures";
import type { QuotaSet } from "./api";

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
  localStorage.clear();
  api.getState.mockResolvedValue(makeState());
  api.cliStatus.mockResolvedValue(cliFixture());
});

afterEach(cleanup);

describe("App", () => {
  it("shows the loading skeleton until the first state arrives", () => {
    api.getState.mockReturnValue(new Promise(() => {}));
    render(<App />);
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument();
  });

  it("lists only connected providers plus the floating add button", async () => {
    const { container } = render(<App />);

    // Floating plus button only; no overview hero, no needs-attention text.
    expect(
      await screen.findByRole("button", { name: "Add account" }),
    ).toBeInTheDocument();
    expect(screen.queryByLabelText("Connection overview")).not.toBeInTheDocument();
    expect(screen.queryByText("1 of 2 ready")).not.toBeInTheDocument();
    // SVG brand marks carry their own <title>, so read the card headers.
    const titles = Array.from(
      container.querySelectorAll(".cli-card .row-title"),
      (el) => el.textContent,
    );
    expect(titles).toHaveLength(1);
    expect(titles[0]).toContain("Claude");
    // The missing CLI is hidden from the list, reachable via plus.
    expect(screen.queryByText("OpenAI Codex")).not.toBeInTheDocument();
    expect(screen.queryByText("Add account")).not.toBeInTheDocument();
  });

  it("opens the guided setup for a missing provider", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Add account" });

    await user.click(screen.getByRole("button", { name: "Add account" }));
    const pick = await screen.findByText("OpenAI Codex");
    await user.click(pick.closest("button")!);

    expect(await screen.findByText("curl -fsSL https://chatgpt.com/codex/install.sh | sh")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Check installation" }),
    ).toBeInTheDocument();
  });

  it("opens usage limits in a popup from the card icon", async () => {
    const user = userEvent.setup();
    api.cliQuotas.mockResolvedValue([
      {
        provider: "claude",
        windows: [{ window: "5-hour", percent: 50, resetsAt: null, status: null }],
        error: null,
      },
    ]);
    render(<App />);
    await screen.findByRole("button", { name: "Add account" });

    await user.click(screen.getByRole("button", { name: "Usage limits for Claude" }));

    expect(await screen.findByText("5-hour")).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();
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
    await screen.findByRole("button", { name: "Add account" });

    expect(api.getState).toHaveBeenCalledTimes(1);
    expect(api.cliStatus).toHaveBeenCalledTimes(1);
    await user.click(screen.getByLabelText("Refresh data"));

    await waitFor(() => {
      expect(api.getState).toHaveBeenCalledTimes(2);
      expect(api.cliStatus).toHaveBeenCalledTimes(2);
    });
  });

  it("does not refresh when the panel regains focus by default", async () => {
    render(<App />);
    await screen.findByRole("button", { name: "Add account" });

    expect(api.getState).toHaveBeenCalledTimes(1);
    window.dispatchEvent(new Event("focus"));

    await waitFor(() => {
      expect(api.getState).toHaveBeenCalledTimes(1);
      expect(api.cliStatus).toHaveBeenCalledTimes(1);
    });
  });
});
