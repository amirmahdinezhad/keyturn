import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { AppModal } from "./AppModal";

function renderModal(open: boolean, onClose: () => void = () => {}) {
  return render(
    <AppModal open={open} onClose={onClose} title="Cost detail">
      <span>the-body</span>
    </AppModal>,
  );
}

describe("AppModal", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("closes on Escape and returns focus to the button that opened it", async () => {
    const user = userEvent.setup();
    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>Show detail</button>
          <AppModal
            open={open}
            onClose={() => setOpen(false)}
            title="Cost detail"
          >
            the-body
          </AppModal>
        </>
      );
    }
    render(<Harness />);
    const trigger = screen.getByRole("button", { name: "Show detail" });
    await user.click(trigger);
    expect(
      screen.getByRole("dialog", { name: "Cost detail" }),
    ).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("shows its title and body when open", () => {
    renderModal(true);
    expect(screen.getByText("Cost detail")).toBeInTheDocument();
    expect(screen.getByText("the-body")).toBeInTheDocument();
  });

  it("renders nothing when closed", () => {
    renderModal(false);
    expect(screen.queryByText("the-body")).not.toBeInTheDocument();
  });

  it("closes through the close trigger", () => {
    const onClose = vi.fn();
    renderModal(true, onClose);
    fireEvent.click(screen.getByLabelText("Close"));
    expect(onClose).toHaveBeenCalled();
  });
});
