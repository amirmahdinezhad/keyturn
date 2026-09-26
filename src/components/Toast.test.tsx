import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { Toast } from "./Toast";
import { EmptyState } from "./EmptyState";
import { LoadingSkeleton } from "./LoadingSkeleton";

afterEach(cleanup);

describe("Toast", () => {
  it("is hidden but present without a message", () => {
    const { container } = render(<Toast toast={null} />);
    const el = container.querySelector(".ak-toast");
    expect(el).not.toBeNull();
    expect(el).not.toHaveClass("show");
    expect(el).toHaveAttribute("aria-live", "polite");
  });

  it("shows and colour-codes the message it is given", () => {
    const { container, rerender } = render(<Toast toast={{ message: "Key deleted", kind: "ok" }} />);
    expect(container.querySelector(".ak-toast")).toHaveClass("ak-toast", "show", "ok");
    expect(screen.getByText("Key deleted")).toBeInTheDocument();

    rerender(<Toast toast={{ message: "Disk is full", kind: "err" }} />);
    expect(container.querySelector(".ak-toast")).toHaveClass("show", "err");
  });

  it("adds no trailing space for the default kind", () => {
    const { container } = render(<Toast toast={{ message: "Profile switched", kind: "" }} />);
    expect(container.querySelector(".ak-toast")?.className).toBe("ak-toast show");
  });
});

describe("EmptyState", () => {
  it("renders its message", () => {
    render(<EmptyState message="Couldn't load keys" />);
    expect(screen.getByText("Couldn't load keys")).toBeInTheDocument();
  });
});

describe("LoadingSkeleton", () => {
  it("is announced as a busy region", () => {
    render(<LoadingSkeleton />);
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument();
  });
});
