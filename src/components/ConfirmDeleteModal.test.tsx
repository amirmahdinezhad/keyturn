import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const { deleteKeyMock, prettyErrorMock } = vi.hoisted(() => ({
  deleteKeyMock: vi.fn(),
  prettyErrorMock: vi.fn(),
}));

vi.mock("../api", () => ({
  deleteKey: deleteKeyMock,
  prettyError: prettyErrorMock,
}));

import { ConfirmDeleteModal } from "./ConfirmDeleteModal";
import type { AppState } from "../types";

const state = { month: { year: 2026, month: 8 } } as unknown as AppState;

beforeEach(() => {
  deleteKeyMock.mockReset();
  prettyErrorMock.mockReset();
  prettyErrorMock.mockImplementation((e: unknown) => String(e));
});

afterEach(cleanup);

function setup(over: Partial<Parameters<typeof ConfirmDeleteModal>[0]> = {}) {
  const onClose = vi.fn();
  const onDeleted = vi.fn();
  const notify = vi.fn();
  render(
    <ConfirmDeleteModal
      target={{ provider: "opencode-go", name: "side-project" }}
      onClose={onClose}
      onDeleted={onDeleted}
      notify={notify}
      {...over}
    />,
  );
  return { onClose, onDeleted, notify };
}

describe("ConfirmDeleteModal", () => {
  it("stays closed without a target", () => {
    render(
      <ConfirmDeleteModal target={null} onClose={() => {}} onDeleted={() => {}} notify={() => {}} />,
    );
    expect(screen.queryByText("Delete key")).not.toBeInTheDocument();
  });

  it("names the key and the pretty provider in the question", () => {
    setup();
    expect(screen.getByText("Delete key")).toBeInTheDocument();
    expect(screen.getByText(/"side-project"/)).toBeInTheDocument();
    expect(screen.getByText("OpenCode Go")).toBeInTheDocument();
  });

  it("cancels without touching the backend", async () => {
    const user = userEvent.setup();
    const { onClose, onDeleted } = setup();

    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(deleteKeyMock).not.toHaveBeenCalled();
    expect(onDeleted).not.toHaveBeenCalled();
  });

  it("deletes the key, pushes the new state and toasts", async () => {
    const user = userEvent.setup();
    deleteKeyMock.mockResolvedValue(state);
    const { onClose, onDeleted, notify } = setup();

    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(deleteKeyMock).toHaveBeenCalledWith("opencode-go", "side-project");
    expect(onDeleted).toHaveBeenCalledWith(state);
    expect(notify).toHaveBeenCalledWith("Key deleted");
    expect(onClose).toHaveBeenCalled();
  });

  it("surfaces a backend failure as an error toast instead of updating state", async () => {
    const user = userEvent.setup();
    deleteKeyMock.mockRejectedValue(new Error("that key is in use"));
    const { onDeleted, notify } = setup();

    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(onDeleted).not.toHaveBeenCalled();
    expect(notify).toHaveBeenCalledWith("Error: that key is in use", "err");
  });
});
