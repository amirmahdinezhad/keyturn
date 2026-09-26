import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { AppModal } from "./AppModal";

// React Aria emits this when a PressResponder has no pressable child. Modal.Root
// used to wrap every modal in a DialogTrigger, which produced one warning per
// mounted modal that never had a trigger.
const PRESS_RESPONDER_WARNING = "A PressResponder was rendered without a pressable child";

function pressResponderWarnings(spy: { mock: { calls: unknown[][] } }): unknown[][] {
  return spy.mock.calls.filter((args) =>
    args.some((arg) => String(arg).includes(PRESS_RESPONDER_WARNING)),
  );
}

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

  it("renders without React Aria's childless PressResponder warning", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    renderModal(false);
    expect(pressResponderWarnings(warn)).toHaveLength(0);
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
