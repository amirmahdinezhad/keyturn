import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CliProviders, formatAgo, formatCliTitle, formatCliVersion, formatReset } from "./CliProviders";
import type { ProviderCliStatus } from "../api";

const notify = vi.fn();
const noop = vi.fn();

function status(over: Partial<ProviderCliStatus> = {}): ProviderCliStatus {
  return {
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
    ...over,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  cleanup();
});

describe("CliProviders", () => {
  const props = {
    notify,
    onAddAccount: noop,
    onShowUsage: noop,
  };

  it("lists only connected providers plus the floating add button", () => {
    const { container } = render(
      <CliProviders
        statuses={[
          status(),
          status({
            id: "codex",
            label: "OpenAI Codex",
            binary: "codex",
            binaryFound: false,
            binaryPath: null,
            version: null,
            loggedIn: null,
            loginCmd: "codex login",
            installUrl: "https://developers.openai.com/codex/cli",
          }),
        ]}
        {...props}
      />,
    );

    // SVG brand marks carry their own <title>, so read the card headers.
    const titles = Array.from(
      container.querySelectorAll(".cli-card .row-title"),
      (el) => el.textContent,
    );
    expect(titles).toHaveLength(1);
    // No "Logged in" badge on connected cards — the title is just the label.
    expect(titles[0]).toContain("Claude");
    expect(titles[0]).not.toContain("Logged in");
    expect(container.querySelector(".cli-card .mini-chip")).toBeNull();
    // The missing CLI is hidden from the list…
    expect(screen.queryByText("OpenAI Codex")).not.toBeInTheDocument();
    // …and reachable through the floating plus button instead (icon only).
    expect(
      screen.getByRole("button", { name: "Add account" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Add account")).not.toBeInTheDocument();
  });

  it("shows no overview hero and no per-card setup links", () => {
    render(
      <CliProviders
        statuses={[
          status(),
          status({
            id: "codex",
            label: "OpenAI Codex",
            binary: "codex",
            binaryFound: false,
            binaryPath: null,
            version: null,
            loggedIn: null,
            loginCmd: "codex login",
            installUrl: "https://developers.openai.com/codex/cli",
          }),
        ]}
        {...props}
      />,
    );

    // No progress hero, no per-card setup links on the main list.
    expect(screen.queryByLabelText("Connection overview")).not.toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.queryByText("Show sign-in command")).not.toBeInTheDocument();
    expect(screen.queryByText("codex login")).not.toBeInTheDocument();
    // The pending count lives on the plus row instead.
    // Only the floating plus button, without any explanatory text.
    expect(
      screen.getByRole("button", { name: "Add account" }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/needs attention/)).not.toBeInTheDocument();
  });

  it("shows only the floating add button once everything is connected", () => {
    render(<CliProviders statuses={[status()]} {...props} />);

    expect(
      screen.getByRole("button", { name: "Add account" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Connect another provider CLI")).not.toBeInTheDocument();
    expect(screen.queryByText("Add account")).not.toBeInTheDocument();
    expect(screen.queryByText(/needs attention/)).not.toBeInTheDocument();
  });

  it("opens the add flow from the floating button", async () => {
    const user = userEvent.setup();
    const onAddAccount = vi.fn();
    render(
      <CliProviders
        statuses={[status()]}
        notify={notify}
        onAddAccount={onAddAccount}
        onShowUsage={noop}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Add account" }));

    expect(onAddAccount).toHaveBeenCalledTimes(1);
  });

  it("opens the usage popup from the card icon button", async () => {
    const user = userEvent.setup();
    const onShowUsage = vi.fn();
    render(
      <CliProviders
        statuses={[status()]}
        notify={notify}
        onAddAccount={noop}
        onShowUsage={onShowUsage}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Usage limits for Claude" }));

    expect(onShowUsage).toHaveBeenCalledTimes(1);
    expect(onShowUsage).toHaveBeenCalledWith(expect.objectContaining({ id: "claude" }));
  });

  it("shows no usage icon where no quota source exists", () => {
    render(
      <CliProviders
        statuses={[status({ id: "gemini", label: "Gemini" })]}
        notify={notify}
        onAddAccount={noop}
        onShowUsage={noop}
      />,
    );

    expect(screen.queryByRole("button", { name: /Usage limits/ })).not.toBeInTheDocument();
  });

  it("hides the login command when already logged in", () => {
    const { container } = render(<CliProviders statuses={[status()]} notify={notify} onAddAccount={noop} onShowUsage={noop} />);

    expect(container.querySelector(".cmd-row")).toBeNull();
    expect(screen.queryByText("claude auth login")).not.toBeInTheDocument();
  });

  it("shows a readable sign-in method once", () => {
    const { container } = render(<CliProviders statuses={[status()]} notify={notify} onAddAccount={noop} onShowUsage={noop} />);

    expect(screen.getByText("Claude account")).toBeInTheDocument();
    expect(container.querySelector(".cli-connection")?.textContent).toBe("Claude account");
    expect(screen.queryByText("Logged in via claude.ai")).not.toBeInTheDocument();
  });

  it("shows the CLI version as a quiet pill and never the binary path", () => {
    const { container } = render(
      <CliProviders
        statuses={[status({ version: "codex-cli 0.52.0" })]}
        notify={notify}
        onAddAccount={noop}
        onShowUsage={noop}
      />,
    );

    expect(screen.getByText("CLI v0.52.0")).toBeInTheDocument();
    expect(screen.getByText("CLI v0.52.0")).toHaveAttribute("title", "codexCli 0.52.0");
    expect(screen.queryByText("codex-cli 0.52.0")).not.toBeInTheDocument();
    expect(screen.queryByText("codexCli 0.52.0")).not.toBeInTheDocument();
    expect(screen.queryByText("/opt/homebrew/bin/claude")).not.toBeInTheDocument();
    expect(container.querySelector(".cli-version")).not.toBeNull();
  });

  it("shows account identity without internal IDs or credential paths", () => {
    const { container } = render(
      <CliProviders
        statuses={[
          status({
            email: "dev@example.com",
            account: "user_123",
            binaryPath: "/Users/dev/.local/bin/claude",
            authFile: "/Users/dev/.claude/.credentials.json (+ macOS Keychain)",
          }),
        ]}
        notify={notify}
        onAddAccount={noop}
        onShowUsage={noop}
      />,
    );

    // The title is just the label — no status badge of any kind.
    expect(container.querySelector(".cli-card .row-title")?.textContent).toBe("Claude");
    const cardText = container.querySelector(".cli-card")?.textContent ?? "";
    expect(cardText).not.toContain("/Users/");
    expect(cardText).not.toContain("user_123");
    expect(cardText).toContain("dev@example.com");
    expect(cardText).not.toContain("credentials.json");
  });

  it("shows the email separately from the aligned provider heading", () => {
    const { container } = render(
      <CliProviders statuses={[status()]} notify={notify} onAddAccount={noop} onShowUsage={noop} />,
    );

    expect(container.querySelector(".cli-card .row-sub")).toBeNull();
    expect(screen.getByText("dev@example.com")).toHaveClass("cli-identity");
    expect(screen.queryByText(/Logged in/)).not.toBeInTheDocument();
  });

  it("renders nothing before the first status arrives", () => {
    const { container } = render(<CliProviders statuses={null} notify={notify} onAddAccount={noop} onShowUsage={noop} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows available plans and named OpenCode connections", () => {
    render(<CliProviders statuses={[
      status({ account: "Max" }),
      status({ id: "opencode", label: "OpenCode", email: null, authMethod: null, connected: ["anthropic", "openai", "opencode-go"] }),
    ]} {...props} />);
    expect(screen.getByText("Max")).toBeInTheDocument();
    const services = screen.getByLabelText("Connected services");
    expect(services).toHaveTextContent("Anthropic");
    expect(services).toHaveTextContent("OpenAI");
    expect(services).toHaveTextContent("OpenCode Go");
    expect(screen.getByText("3 services")).toBeInTheDocument();
  });
});

describe("formatAgo", () => {
  // Frozen clock: impl and expectations share one instant, so minute/hour
  // boundaries can never tick between the two Date.now() calls.
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(Date.parse("2026-09-23T12:00:00.000Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("words snapshot age like a status bar", () => {
    const now = Date.now();
    expect(formatAgo(null)).toBe("");
    expect(formatAgo("nope")).toBe("");
    expect(formatAgo(new Date(now - 30 * 1000).toISOString())).toBe("just now");
    expect(formatAgo(new Date(now - 45 * 60000).toISOString())).toBe("45m ago");
    expect(formatAgo(new Date(now - 3 * 3600000).toISOString())).toBe("3h ago");
    expect(formatAgo(new Date(now - 2 * 86400000).toISOString())).toBe("2d ago");
  });
});

describe("formatReset", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(Date.parse("2026-09-23T12:00:00.000Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("words the countdown like the old panel", () => {
    const now = Date.now();
    expect(formatReset(null)).toBe("");
    expect(formatReset("nope")).toBe("");
    expect(formatReset(new Date(now - 1000).toISOString())).toBe("Resets now");
    expect(formatReset(new Date(now + 59 * 60000).toISOString())).toBe("Resets in 59m");
    expect(formatReset(new Date(now + (4 * 60 + 58) * 60000).toISOString())).toBe(
      "Resets in 4h 58m",
    );
    expect(formatReset(new Date(now + (5 * 1440 + 14 * 60) * 60000).toISOString())).toBe(
      "Resets in 5d 14h",
    );
  });
});

describe("formatCliTitle", () => {
  it("camelCases the binary-name prefix for the tooltip", () => {
    expect(formatCliTitle(null)).toBe("");
    expect(formatCliTitle("  ")).toBe("");
    expect(formatCliTitle("codex-cli 0.52.0")).toBe("codexCli 0.52.0");
    expect(formatCliTitle("claude 2.1.220")).toBe("claude 2.1.220");
    expect(formatCliTitle("opencode 1.5.3")).toBe("opencode 1.5.3");
  });
});

describe("formatCliVersion", () => {
  it("strips the binary-name prefix from --version output", () => {
    expect(formatCliVersion(null)).toBe("");
    expect(formatCliVersion("codex-cli 0.52.0")).toBe("v0.52.0");
    expect(formatCliVersion("claude 2.1.220")).toBe("v2.1.220");
    expect(formatCliVersion("opencode 1.5.3")).toBe("v1.5.3");
    expect(formatCliVersion("2.1.220")).toBe("v2.1.220");
  });
});
