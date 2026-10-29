import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CliProviders, formatAgo, formatReset } from "./CliProviders";
import type { ProviderCliStatus } from "../api";

const { profileQuota } = vi.hoisted(() => ({ profileQuota: vi.fn() }));
vi.mock("../api", async (original) => ({ ...await original<typeof import("../api")>(), claudeProfileQuota: profileQuota }));

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
  profileQuota.mockImplementation(async (id: string) => ({ provider: "claude", windows: [{ window: "weekly", percent: id === "two" ? 63 : 21 }] }));
  cleanup();
});

describe("CliProviders", () => {
  const props = {
    notify,
    onAddAccount: noop,
    onShowUsage: noop,
  };

  it("omits the entire other-providers section when none are connected", () => {
    const { container } = render(<CliProviders statuses={[status({ loggedIn: false })]} sectionLabel="Other providers" {...props} />);
    expect(container.querySelector(".other-providers")).toBeNull();
    expect(screen.queryByText(/Other providers/)).not.toBeInTheDocument();
  });

  it("counts saved Claude accounts without duplicating the active CLI login", () => {
    const account = { id: "one", name: "Personal", email: null, configDir: "/tmp/test", createdAt: "2026-10-01", active: true, connected: true, subscription: null, loginCommand: "claude auth login", launchCommand: "claude", quota: null };
    render(<CliProviders statuses={[status()]} claudeAccounts={[account, { ...account, id: "two", name: "Work", active: false }]} sectionLabel="Other providers" {...props} />);
    expect(screen.getByLabelText("2 Claude accounts")).toHaveTextContent("2");
    expect(screen.getByRole("article", { name: "Personal Claude account" })).toBeInTheDocument();
    expect(screen.getByRole("article", { name: "Work Claude account" })).toBeInTheDocument();
    expect(document.querySelectorAll(".cli-card")).toHaveLength(2);
  });

  it("keeps Claude expanded and excludes it from the other-provider count", () => {
    const { container } = render(<CliProviders statuses={[status(), status({ id: "gemini", label: "Gemini" })]} sectionLabel="Other providers" {...props} />);
    const claude = screen.getByRole("region", { name: "Claude accounts" });
    const other = container.querySelector(".other-providers")!;
    expect(other.contains(claude)).toBe(false);
    expect(other.querySelector("summary")).toHaveTextContent("Other providers 1");
    expect(within(other as HTMLElement).getByRole("region", { name: "Gemini accounts" })).toBeInTheDocument();
  });

  it("loads each saved Claude profile's own usage, including inactive profiles and session errors", async () => {
    const user = userEvent.setup();
    const account = { id: "one", name: "Personal", email: null, configDir: "/tmp/test", createdAt: "2026-10-01", active: true, connected: true, subscription: null, loginCommand: "claude auth login", launchCommand: "claude", quota: { provider: "claude", windows: [{ window: "weekly", percent: 21 }] } };
    const accounts = [account, { ...account, id: "two", name: "Work", active: false, quota: { provider: "claude", windows: [{ window: "weekly", percent: 63 }] } }, { ...account, id: "empty", name: "New", active: false, quota: null }];
    const view = render(<CliProviders statuses={null} claudeAccounts={accounts} sectionLabel="Other providers" showRemaining={false} {...props} />);
    expect(view.container.querySelector(".other-providers")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Usage limits for Work" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("Work");
    expect(await screen.findByText("63%")).toBeInTheDocument();
    expect(screen.queryByText("21%")).not.toBeInTheDocument();
    view.rerender(<CliProviders statuses={null} claudeAccounts={accounts} sectionLabel="Other providers" showRemaining {...props} />);
    expect(screen.getByRole("dialog")).toHaveTextContent("37%");
    await user.click(screen.getByRole("button", { name: "Close" }));
    profileQuota.mockResolvedValue({ provider: "claude", error: "Claude session expired. Sign in again." });
    await user.click(screen.getByRole("button", { name: "Usage limits for New" }));
    expect(await screen.findByText(/Claude session expired/)).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("loads live Claude usage when a connected profile has no transcript snapshot", async () => {
    profileQuota.mockResolvedValue({ provider: "claude", windows: [{ window: "5-hour", percent: 42 }] });
    const account = { id: "live", name: "Team", email: null, configDir: "/tmp/test", createdAt: "2026-10-01", active: true, connected: true, subscription: null, loginCommand: "claude auth login", launchCommand: "claude", quota: null };
    render(<CliProviders statuses={[]} claudeAccounts={[account]} sectionLabel="Other providers" showRemaining={false} {...props} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Usage limits for Team" }));
    expect(await screen.findByText("42%")).toBeInTheDocument();
    expect(profileQuota).toHaveBeenCalledWith("live");
    expect(screen.queryByText(/No snapshots yet/)).not.toBeInTheDocument();
  });

  it("lists only connected providers plus the floating add button", () => {
    const { container } = render(
      <CliProviders
        statuses={[
          status(),
          status({
            id: "codex",
            label: "Codex",
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
    expect(screen.queryByText("Codex")).not.toBeInTheDocument();
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
            label: "Codex",
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

  it("hides CLI versions and connection dots", () => {
    const { container } = render(
      <CliProviders
        statuses={[status({ version: "codex-cli 0.52.0" })]}
        notify={notify}
        onAddAccount={noop}
        onShowUsage={noop}
      />,
    );

    expect(screen.queryByText("CLI v0.52.0")).not.toBeInTheDocument();
    expect(screen.queryByText("codex-cli 0.52.0")).not.toBeInTheDocument();
    expect(screen.queryByText("codexCli 0.52.0")).not.toBeInTheDocument();
    expect(screen.queryByText("/opt/homebrew/bin/claude")).not.toBeInTheDocument();
    expect(container.querySelector(".cli-version")).toBeNull();
    expect(container.querySelector(".cli-connection i")).toBeNull();
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
    const services = screen.getAllByLabelText("Connected services").find((element) => element.textContent?.includes("OpenCode Go"))!;
    expect(services).toHaveTextContent("Anthropic");
    expect(services).toHaveTextContent("OpenAI");
    expect(services).toHaveTextContent("OpenCode Go");
    expect(screen.getByText("3 services")).toBeInTheDocument();
  });

  it("places a text-only plan badge below the CLI provider without a price", () => {
    const { container } = render(<CliProviders statuses={[status({ id: "codex", label: "Codex", authMethod: "API key",
      subscriptions: [{ id: "chatgpt", service: "ChatGPT", plan: "prolite" }],
    })]} {...props} />);
    expect(container.querySelector(".cli-head")).toHaveTextContent("Codex");
    const footer = within(container.querySelector(".cli-footer") as HTMLElement);
    expect(footer.getByText("ChatGPT")).toBeInTheDocument();
    const badge = footer.getByText("Pro");
    expect(badge).toHaveAttribute("data-tier", "pro");
    expect(badge.childElementCount).toBe(0);
    expect(badge).not.toHaveAttribute("aria-label");
    expect(footer.queryByText(/\$|\/mo/)).not.toBeInTheDocument();
  });

  it("removes ended accounts and only expired services on multi-service cards", () => {
    const { container } = render(<CliProviders statuses={[
      status({ id: "codex", label: "Codex", subscriptions: [{ id: "chatgpt", service: "ChatGPT", plan: "plus", expiresAt: "2000-01-01T00:00:00Z" }] }),
      status({ id: "opencode", label: "OpenCode", subscriptions: [
        { id: "opencode-go", service: "OpenCode Go", plan: "go", status: "expired" },
        { id: "openai", service: "OpenAI", plan: "API" },
      ] }),
    ]} {...props} />);
    expect(container.querySelectorAll(".cli-card")).toHaveLength(1);
    expect(container.querySelector(".cli-head")).toHaveTextContent("OpenCode");
    expect(screen.queryByText("OpenCode Go")).not.toBeInTheDocument();
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
    expect(container.querySelector(".provider-section-label")).toHaveTextContent("1");
  });

  it("removes a subscription when it ends while the panel stays open", () => {
    vi.useFakeTimers();
    try {
      vi.setSystemTime(new Date("2026-10-15T00:00:00Z"));
      render(<CliProviders statuses={[status({ subscriptions: [{ id: "claude", service: "Claude", plan: "pro", expiresAt: "2026-10-15T00:00:01Z" }] })]} {...props} />);
      expect(screen.queryByText("No connected accounts")).not.toBeInTheDocument();
      act(() => { vi.advanceTimersByTime(1000); });
      expect(screen.getByText("No connected accounts")).toBeInTheDocument();
    } finally { vi.useRealTimers(); }
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

it("removes a failed saved Claude account on refresh without resurrecting its ambient login", () => {
  const account = { id: "one", name: "Personal", email: "dev@example.com", configDir: "/tmp/test", createdAt: "2026-10-01", active: true, connected: true, subscription: null, loginCommand: "", launchCommand: "", quota: { provider: "claude", windows: [{ window: "weekly", percent: 100 }] } };
  const view = render(<CliProviders statuses={[status()]} claudeAccounts={[account]} sectionLabel="Other providers" notify={notify} onAddAccount={noop} onShowUsage={noop} />);
  expect(screen.getByRole("article", { name: "Personal Claude account" })).toBeInTheDocument();
  view.rerender(<CliProviders statuses={[status()]} claudeAccounts={[{ ...account, connected: false, quota: { provider: "claude", error: "Session expired" } }]} sectionLabel="Other providers" notify={notify} onAddAccount={noop} onShowUsage={noop} />);
  expect(screen.queryByRole("article", { name: "Personal Claude account" })).not.toBeInTheDocument();
  expect(screen.queryByRole("article", { name: "Claude account" })).not.toBeInTheDocument();
  view.rerender(<CliProviders statuses={[status()]} claudeAccounts={[account]} sectionLabel="Other providers" notify={notify} onAddAccount={noop} onShowUsage={noop} />);
  expect(screen.getByRole("article", { name: "Personal Claude account" })).toBeInTheDocument();
});
