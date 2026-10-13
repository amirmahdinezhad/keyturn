import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

const { mockQuotas } = vi.hoisted(() => ({ mockQuotas: vi.fn() }));

vi.mock("../api", () => ({ cliQuotas: mockQuotas }));

import { UsageModal } from "./UsageModal";
import type { ProviderCliStatus } from "../api";

const notify = vi.fn();

function status(over: Partial<ProviderCliStatus> = {}): ProviderCliStatus {
  return {
    id: "opencode",
    label: "OpenCode",
    binary: "opencode",
    binaryFound: true,
    binaryPath: "/opt/homebrew/bin/opencode",
    version: "1.2.3",
    loggedIn: true,
    authMethod: null,
    email: null,
    account: null,
    detail: "2 connected",
    loginCmd: "opencode auth login",
    installUrl: "https://opencode.ai",
    authFile: null,
    connected: ["opencode-go", "anthropic"],
    ...over,
  };
}

function renderModal(over: Partial<ProviderCliStatus> = {}, showRemaining = false) {
  return render(
    <UsageModal
      status={status(over)}
      showRemaining={showRemaining}
      onClose={() => {}}
      notify={notify}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(cleanup);

describe("UsageModal", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <UsageModal status={null} showRemaining={false} onClose={() => {}} notify={notify} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows percent windows with bars and resets", async () => {
    mockQuotas.mockResolvedValue([
      {
        provider: "opencode-go",
        windows: [
          { window: "rolling", percent: 40, resetsAt: new Date(Date.now() + 3600000).toISOString(), status: "ok" },
          { window: "weekly", percent: 12.5, resetsAt: null, status: "ok" },
        ],
        error: null,
      },
    ]);
    renderModal();

    expect(await screen.findByText("5-hour")).toBeInTheDocument();
    expect(screen.getByText("Weekly")).toBeInTheDocument();
    expect(screen.getByText("40%")).toBeInTheDocument();
    expect(screen.getByText("12.5%")).toBeInTheDocument();
    expect(mockQuotas).toHaveBeenCalledTimes(1);
  });

  it("shows backend quota errors honestly instead of bars", async () => {
    mockQuotas.mockResolvedValue([
      { provider: "opencode-go", windows: null, error: "no quota on this key (no active plan?)" },
    ]);
    renderModal();

    expect(await screen.findByText(/no quota on this key/)).toBeInTheDocument();
  });

  it("inverts percents to remaining when the setting is on", async () => {
    mockQuotas.mockResolvedValue([
      {
        provider: "opencode-go",
        windows: [{ window: "rolling", percent: 40, resetsAt: null, status: "ok" }],
        error: null,
      },
    ]);
    renderModal({}, true);

    expect(await screen.findByText("60%")).toBeInTheDocument();
    expect(screen.queryByText("40%")).not.toBeInTheDocument();
    expect(screen.queryByText(/% left|% used/)).not.toBeInTheDocument();
  });

  it("replays the Claude snapshot with its age", async () => {
    mockQuotas.mockResolvedValue([
      {
        provider: "claude",
        windows: [
          { window: "5-hour", percent: 50, resetsAt: "2026-09-26T21:19:59Z", status: null },
          { window: "Weekly", percent: 16, resetsAt: "2026-09-27T16:59:59Z", status: null },
        ],
        error: null,
        asOf: new Date(Date.now() - 3 * 3600000).toISOString(),
      },
    ]);
    renderModal({ id: "claude", label: "Claude" });

    expect(await screen.findByText("5-hour")).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();
    expect(screen.getByText("Updated 3h ago")).toBeInTheDocument();
  });

  it("says so when Claude never reported usage", async () => {
    mockQuotas.mockResolvedValue([]);
    renderModal({ id: "claude", label: "Claude" });

    expect(await screen.findByText(/No snapshots yet/)).toBeInTheDocument();
  });

  it("shows Codex windows from the quota endpoint", async () => {
    mockQuotas.mockResolvedValue([
      {
        provider: "codex",
        windows: [
          { window: "Weekly", percent: 10, resetsAt: "2026-10-04T00:00:00Z", status: null },
          { window: "gpt-reserve", percent: 0, resetsAt: null, status: null },
        ],
        error: null,
      },
    ]);
    renderModal({ id: "codex", label: "OpenAI Codex" });

    expect(await screen.findByText("Weekly")).toBeInTheDocument();
    expect(screen.getByText("10%")).toBeInTheDocument();
    expect(screen.queryByText("gpt-reserve")).not.toBeInTheDocument();
  });
});
