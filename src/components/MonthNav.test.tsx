import { describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach } from "vitest";
import { MonthNav } from "./MonthNav";

afterEach(cleanup);

const september = { year: 2026, month: 8 };

describe("MonthNav", () => {
  it("labels the selected month and the spend", () => {
    render(<MonthNav selected={september} isCurrentMonth total={123.45} onShift={() => {}} />);
    expect(screen.getByText("September 2026")).toBeInTheDocument();
    expect(screen.getByText("$123.45")).toBeInTheDocument();
    expect(screen.getByText("spent this month")).toBeInTheDocument();
  });

  it("calls the month labels differently when browsing history", () => {
    render(<MonthNav selected={september} isCurrentMonth={false} total={10} onShift={() => {}} />);
    expect(screen.getByText("month total")).toBeInTheDocument();
  });

  it("shows a placeholder while the first fetch is in flight", () => {
    render(<MonthNav selected={september} isCurrentMonth loading onShift={() => {}} />);
    expect(screen.getByText("—")).toBeInTheDocument();
    expect(screen.queryByText("$0")).not.toBeInTheDocument();
  });

  it("falls back to $0 once loading finished with no total", () => {
    render(<MonthNav selected={september} isCurrentMonth onShift={() => {}} />);
    expect(screen.getByText("$0")).toBeInTheDocument();
  });

  it("blocks the next month on the current one but never the previous", async () => {
    const user = userEvent.setup();
    render(<MonthNav selected={september} isCurrentMonth onShift={() => {}} />);
    expect(screen.getByLabelText("Next month")).toBeDisabled();
    expect(screen.getByLabelText("Previous month")).toBeEnabled();
    await user.click(screen.getByLabelText("Previous month"));
  });

  it("reports the shift direction", async () => {
    const user = userEvent.setup();
    const onShift = vi.fn();
    render(<MonthNav selected={september} isCurrentMonth={false} onShift={onShift} />);

    await user.click(screen.getByLabelText("Previous month"));
    await user.click(screen.getByLabelText("Next month"));

    expect(onShift.mock.calls).toEqual([[-1], [1]]);
  });
});
