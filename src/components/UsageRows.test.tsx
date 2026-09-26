import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DEFAULT_USAGE_DISPLAY, UsageRows, type UsageRow } from "./UsageRows";
import type { PaceInfo } from "../format";

const NOW = Date.parse("2026-09-23T12:00:00.000Z");
const at = (offsetSeconds: number) => new Date(NOW + offsetSeconds * 1000).toISOString();

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const row = (over: Partial<UsageRow> = {}): UsageRow => ({
  label: "5-hour Usage",
  percent: 40,
  window: "rolling",
  resetsAt: at(4 * 3600),
  ...over,
});

describe("UsageRows", () => {
  it("shows an empty state when nothing was reported", () => {
    render(<UsageRows rows={[]} />);
    expect(screen.getByText(/No usage limits reported/i)).toBeInTheDocument();
  });

  it("renders used percent by default", () => {
    render(<UsageRows rows={[row({ percent: 37.4 })]} />);
    expect(screen.getByText("37.4%")).toBeInTheDocument();
  });

  it("renders remaining and both modes from the same data", () => {
    const { unmount } = render(
      <UsageRows rows={[row({ percent: 37.4 })]} display={{ ...DEFAULT_USAGE_DISPLAY, mode: "remaining" }} />,
    );
    expect(screen.getByText("62.6% left")).toBeInTheDocument();
    unmount();

    render(
      <UsageRows rows={[row({ percent: 37.4 })]} display={{ ...DEFAULT_USAGE_DISPLAY, mode: "both" }} />,
    );
    expect(screen.getByText("37.4% used · 62.6% left")).toBeInTheDocument();
  });

  it("colours the percentage and fill by threshold band", () => {
    const { container } = render(<UsageRows rows={[row({ percent: 95 })]} />);
    expect(screen.getByText("95%")).toHaveClass("usage-pct", "hot");
    expect(container.querySelector(".usage-fill")).toHaveClass("hot");
  });

  it("clamps the fill width into 0..100", () => {
    const { container, unmount } = render(<UsageRows rows={[row({ percent: 140 })]} />);
    expect(container.querySelector<HTMLElement>(".usage-fill")?.style.width).toBe("100%");
    unmount();

    const second = render(<UsageRows rows={[row({ percent: -20 })]} />);
    expect(second.container.querySelector<HTMLElement>(".usage-fill")?.style.width).toBe("0%");
  });

  it("prefers a precomputed reset string over the timestamp", () => {
    render(<UsageRows rows={[row({ reset: "Resets in 3 hours" })]} />);
    expect(screen.getByText("Resets in 3 hours")).toBeInTheDocument();
  });

  it("derives the reset line from the timestamp in the selected mode", () => {
    render(
      <UsageRows
        rows={[row({ resetsAt: at(3600) })]}
        display={{ ...DEFAULT_USAGE_DISPLAY, reset: "remaining" }}
      />,
    );
    expect(screen.getByText("Resets in 1 hour 0 minutes")).toBeInTheDocument();
  });

  it("omits pace entirely when the display disables it", () => {
    const pace: PaceInfo = { projected: 80, level: "steady", label: "Steady", marker: 80 };
    const { container, unmount } = render(
      <UsageRows
        rows={[row({ pace })]}
        display={{ ...DEFAULT_USAGE_DISPLAY, showPace: false }}
      />,
    );
    expect(container.querySelector(".pace-marker")).toBeNull();
    unmount();

    const shown = render(<UsageRows rows={[row({ pace })]} />);
    expect(shown.container.querySelector(".pace-marker")).toHaveClass("pace-steady");
    expect(screen.getByText("Steady · 80%")).toBeInTheDocument();
  });

  it("clamps the pace marker to 100% of the bar", () => {
    const pace: PaceInfo = { projected: 150, level: "runaway", label: "Runaway", marker: 130 };
    const { container } = render(<UsageRows rows={[row({ pace })]} />);
    expect(container.querySelector<HTMLElement>(".pace-marker")?.style.left).toBe("100%");
  });

  it("leaves the reset slot empty (but present) without any reset info", () => {
    const { container } = render(
      <UsageRows rows={[row({ resetsAt: null, reset: undefined })]} />,
    );
    expect(container.querySelector(".usage-reset")).toBeNull();
    expect(container.querySelector(".usage-meta")?.children).toHaveLength(1);
  });
});
