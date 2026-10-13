import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";

import { AddAccountModal } from "./AddAccountModal";
import type { ProviderCliStatus } from "../api";

const notify = vi.fn();
const onClose = vi.fn();

function candidate(over: Partial<ProviderCliStatus> = {}): ProviderCliStatus {
  return {
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
    authFile: null,
    connected: [],
    ...over,
  };
}

function renderModal(
  over: Partial<ComponentProps<typeof AddAccountModal>> = {},
) {
  return render(
    <AddAccountModal
      open
      candidates={[candidate()]}
      onClose={onClose}
      onRefresh={async () => []}
      notify={notify}
      {...over}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(cleanup);

describe("AddAccountModal", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <AddAccountModal
        open={false}
        candidates={[candidate()]}
        onClose={onClose}
        onRefresh={async () => []}
        notify={notify}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("starts on provider pick without a stepper, then shows it after pick", async () => {
    const user = userEvent.setup();
    renderModal();

    expect(screen.queryByLabelText("Setup progress")).not.toBeInTheDocument();
    expect(screen.getByText("OpenAI Codex")).toBeInTheDocument();
    expect(screen.getByText("Not installed")).toBeInTheDocument();

    await user.click(screen.getByText("OpenAI Codex").closest("button")!);

    expect(screen.getByLabelText("Setup progress")).toBeInTheDocument();

    // Install step validates before continuing: no sign-in command yet.
    expect(screen.getByText("curl -fsSL https://chatgpt.com/codex/install.sh | sh")).toBeInTheDocument();
    expect(screen.queryByText("codex login")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check installation" })).toBeInTheDocument();
  });

  it("blocks install until the CLI is found, then advances to sign in", async () => {
    const user = userEvent.setup();
    const installed = candidate({ binaryFound: true, binaryPath: "/usr/local/bin/codex" });
    renderModal({ onRefresh: async () => [installed] });
    await user.click(screen.getByText("OpenAI Codex").closest("button")!);

    await user.click(screen.getByRole("button", { name: "Check installation" }));

    expect(await screen.findByText("codex login")).toBeInTheDocument();
    const steps = within(screen.getByLabelText("Setup progress"));
    expect(steps.getByText("Install").closest("li")).toHaveClass("is-done");
    expect(steps.getByText("Sign in").closest("li")).toHaveAttribute("aria-current", "step");
    expect(
      screen.getByRole("button", { name: "I've signed in — check" }),
    ).toBeInTheDocument();
  });

  it("stays on install with an error when the CLI is still missing", async () => {
    const user = userEvent.setup();
    renderModal({ onRefresh: async () => [candidate()] });

    await user.click(screen.getByText("OpenAI Codex").closest("button")!);
    await user.click(screen.getByRole("button", { name: "Check installation" }));

    expect(await screen.findByText(/still not found/)).toBeInTheDocument();
    expect(screen.queryByText("codex login")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("skips install when the binary exists", async () => {
    const user = userEvent.setup();
    renderModal({
      candidates: [candidate({ binaryFound: true, binaryPath: "/usr/local/bin/codex" })],
    });

    await user.click(screen.getByText("OpenAI Codex").closest("button")!);

    expect(screen.queryByText("curl -fsSL https://chatgpt.com/codex/install.sh | sh")).not.toBeInTheDocument();
    expect(screen.getByText("codex login")).toBeInTheDocument();
  });

  it("finishes with a Done pane once the account verifies", async () => {
    const user = userEvent.setup();
    const loggedIn = candidate({ binaryFound: true, loggedIn: true });
    renderModal({
      candidates: [candidate({ binaryFound: true })],
      onRefresh: async () => [loggedIn],
    });

    await user.click(screen.getByText("OpenAI Codex").closest("button")!);
    await user.click(screen.getByRole("button", { name: "I've signed in — check" }));

    expect(await screen.findByRole("button", { name: "Done" })).toBeInTheDocument();
    expect(notify).toHaveBeenCalledWith("OpenAI Codex connected", "ok");
    // Once configured, the sign-in command + Copy button are gone.
    expect(screen.queryByText("codex login")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Copy" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("stays open with a hint when sign-in verification fails", async () => {
    const user = userEvent.setup();
    renderModal({
      candidates: [candidate({ binaryFound: true })],
      onRefresh: async () => [candidate({ binaryFound: true })],
    });

    await user.click(screen.getByText("OpenAI Codex").closest("button")!);
    await user.click(screen.getByRole("button", { name: "I've signed in — check" }));

    expect(await screen.findByText(/Still not signed in/)).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("says so when everything is already connected", () => {
    renderModal({ candidates: [] });

    expect(screen.getByText("Everything is connected.")).toBeInTheDocument();
  });

  it.each([
    ["claude", "Claude", "claude auth login", "Anthropic Console", "https://code.claude.com/docs/en/authentication"],
    ["opencode", "OpenCode", "opencode auth login", "Select your provider", "https://opencode.ai/docs/cli/#auth"],
    ["gemini", "Gemini", "gemini", "Sign in with Google", "https://geminicli.com/docs/get-started/authentication/"],
  ])("shows official guidance for %s", async (id, label, loginCmd, instruction, href) => {
    const user = userEvent.setup();
    renderModal({ candidates: [candidate({ id, label, loginCmd, binaryFound: true })] });
    await user.click(screen.getByText(label, { selector: ".row-title" }).closest("button")!);
    expect(screen.getByText(loginCmd, { selector: "code" })).toBeInTheDocument();
    expect(screen.getByText(new RegExp(instruction), { selector: "li" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: `Official ${label} guide` })).toHaveAttribute("href", href);
    expect(screen.queryByText(/Step .* of/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Run the command in your own terminal, then check/)).not.toBeInTheDocument();
  });
});
