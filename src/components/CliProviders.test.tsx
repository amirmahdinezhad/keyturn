import { beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CliProviders } from "./CliProviders";
import type { ProviderCliStatus } from "../api";

const notify = vi.fn();

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
  it("renders one card per status with matching pill and subtitle", () => {
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
        notify={notify}
      />,
    );

    const titles = Array.from(
      container.querySelectorAll(".cli-card .row-title"),
      (el) => el.textContent,
    );
    expect(titles).toHaveLength(2);
    expect(titles[0]).toContain("Logged in");
    expect(titles[1]).toContain("Not installed");
    expect(screen.getByText("CLI not found on PATH")).toBeInTheDocument();
  });

  it("shows the install link and the login command when the binary is missing", () => {
    render(
      <CliProviders
        statuses={[
          status({ binaryFound: false, loggedIn: null }),
        ]}
        notify={notify}
      />,
    );

    expect(screen.getByText("https://claude.com/product/claude-code")).toBeInTheDocument();
    expect(screen.getByText("claude auth login")).toBeInTheDocument();
  });

  it("copies the login command and confirms with a toast", async () => {
    const user = userEvent.setup();
    render(<CliProviders statuses={[status()]} notify={notify} />);

    await user.click(screen.getByRole("button", { name: "Copy" }));

    // The clipboard itself is a browser API; what we own is the toast.
    expect(notify).toHaveBeenCalledWith(
      expect.stringContaining("Login command copied"),
      "ok",
    );
  });

  it("renders nothing before the first status arrives", () => {
    const { container } = render(<CliProviders statuses={null} notify={notify} />);
    expect(container).toBeEmptyDOMElement();
  });
});
