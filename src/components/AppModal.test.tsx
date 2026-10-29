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

  it("blocks closing while a save is in flight", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<AppModal open onClose={onClose} title="Saving account" busy>Draft</AppModal>);
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(onClose).not.toHaveBeenCalled();
  });

  it("confirms a dirty back path and lets Escape keep the draft", async () => {
    const user = userEvent.setup();
    const back = vi.fn();
    render(<AppModal open onClose={() => {}} title="Account" onBack={back} hasUnsavedChanges>Draft</AppModal>);
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("alertdialog", { name: "Discard changes?" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(back).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Back" }));
    await user.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(back).toHaveBeenCalledOnce();
  });
});
