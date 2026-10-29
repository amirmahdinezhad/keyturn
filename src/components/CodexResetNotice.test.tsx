import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CodexResetStatus, ProviderCliStatus } from "../api";
import { RESET_DISMISSAL_KEY, resetForecastTiming } from "../codexResets";
import { DEFAULT_CODEX_RESET_SETTINGS, type CodexResetSettings } from "../types";

const mocks = vi.hoisted(() => ({ read: vi.fn() }));
vi.mock("../api", async (importOriginal) => ({
  ...await importOriginal<typeof import("../api")>(), codexResetStatus: mocks.read,
}));
import { CodexResetNotice } from "./CodexResetNotice";
import { CliProviders } from "./CliProviders";

const NOW = Date.parse("2026-10-13T04:00:00Z");
const account: ProviderCliStatus = {
  id: "codex", label: "Codex", binary: "codex", binaryFound: true,
  loggedIn: true, authMethod: "ChatGPT", loginCmd: "codex login", installUrl: "",
};
function outlook(over: Partial<CodexResetStatus> = {}): CodexResetStatus {
  return { latestReset: null, scheduledReset: null, activeWatch: null, generatedAt: new Date(NOW).toISOString(), ...over };
}
function scheduled(over: Partial<NonNullable<CodexResetStatus["scheduledReset"]>> = {}) {
  return outlook({ scheduledReset: { resetType: "regular", announcedAt: "2026-10-13T03:00:00Z", scheduledFor: "2026-10-13T07:00:00Z", ...over } });
}
const completed = { id: "reset-1", resetType: "regular" as const, announcedAt: "2026-10-13T03:24:00Z" };
async function mount(data: CodexResetStatus, settings?: CodexResetSettings) {
  mocks.read.mockResolvedValue(data);
  let view!: ReturnType<typeof render>;
  await act(async () => { view = render(<CodexResetNotice refreshKey={account} settings={settings} />); });
  return view;
}
async function refresh(view: ReturnType<typeof render>, data: CodexResetStatus) {
  mocks.read.mockResolvedValue(data);
  await act(async () => view.rerender(<CodexResetNotice refreshKey={{ ...account }} />));
}
beforeEach(() => {
  vi.useFakeTimers(); vi.setSystemTime(NOW); vi.clearAllMocks(); localStorage.clear();
  mocks.read.mockResolvedValue(outlook());
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.useRealTimers(); });

describe("upcoming Codex resets", () => {
  it("shows a current probable reset when the source has no scheduled announcement", async () => {
    await mount(outlook({ activeWatch: {
      level: "elevated", resetChancePercent: 40, forecastWindow: "within 24h",
      observedAt: "2026-10-13T03:00:00Z", expiresAt: "2026-10-14T05:00:00Z",
    } }));
    expect(screen.getByText("Possible reset")).toBeInTheDocument();
    expect(screen.getByText(/40% chance/)).toBeInTheDocument();
  });
  it("shows only countdown, local hours/minutes and a close button without a source link", async () => {
    const view = await mount(scheduled({ scheduledFor: "2026-10-13T07:12:00Z" }));
    expect(screen.getByText("Resets in 3h 12m")).toBeInTheDocument();
    expect(view.container.querySelector("time")).toHaveTextContent(/^At 07:12$/);
    expect(view.container.querySelector("time")).toHaveAttribute("dateTime", "2026-10-13T07:12:00Z");
    expect(screen.getByRole("button", { name: "Dismiss upcoming reset" })).toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(view.container).not.toHaveTextContent(/GMT|UTC|codex-resets/);
  });
  it("uses the device's local timezone", async () => {
    // @ts-expect-error Node types are not installed in this frontend package.
    const previous = process.env.TZ;
    try {
      // @ts-expect-error Node types are not installed in this frontend package.
      process.env.TZ = "Asia/Tehran";
      await mount(scheduled());
      expect(screen.getByText("At 10:30")).toBeInTheDocument();
    } finally {
      // @ts-expect-error Node types are not installed in this frontend package.
      process.env.TZ = previous;
    }
  });
  it("keeps minute precision beyond a day and updates without a status refresh", async () => {
    await mount(scheduled({ scheduledFor: "2026-10-14T07:12:00Z" }));
    expect(screen.getByText("Resets in 27h 12m")).toBeInTheDocument();
    await act(async () => vi.advanceTimersByTime(60_000));
    expect(screen.getByText("Resets in 27h 11m")).toBeInTheDocument();
    expect(mocks.read).toHaveBeenCalledTimes(1);
  });
  it("disappears at the deadline instead of showing a completed or waiting state", async () => {
    const view = await mount(scheduled({ scheduledFor: "2026-10-13T04:01:00Z" }));
    expect(screen.getByText("Resets in 0h 01m")).toBeInTheDocument();
    await act(async () => vi.advanceTimersByTime(1000));
    expect(screen.getByText("Resets in <1m")).toBeInTheDocument();
    await act(async () => vi.advanceTimersByTime(59_000));
    expect(view.container).toBeEmptyDOMElement();
  });
  it("shows a fresh confirmed reset and gives the next announcement priority", async () => {
    localStorage.setItem("keyturn.codexResetHistory", JSON.stringify({ latestReset: completed, dismissedResetId: null }));
    const view = await mount(outlook({ latestReset: completed }));
    expect(screen.getByText("Reset confirmed")).toBeInTheDocument();
    await refresh(view, { ...scheduled(), latestReset: completed });
    expect(screen.getByText("Resets in 3h 00m")).toBeInTheDocument();
    expect(screen.queryByText("Reset completed")).not.toBeInTheDocument();
  });
  it("keeps the next announcement when the completed event belongs to an earlier reset", async () => {
    await mount({ ...scheduled({ announcedAt: "2026-10-13T03:30:00Z" }), latestReset: completed });
    expect(screen.getByText("Resets in 3h 00m")).toBeInTheDocument();
  });
  it("persists closing across refreshes/restarts and displays a new announcement", async () => {
    const view = await mount(scheduled());
    fireEvent.click(screen.getByRole("button", { name: "Dismiss upcoming reset" }));
    expect(view.container).toBeEmptyDOMElement();
    await refresh(view, scheduled());
    expect(view.container).toBeEmptyDOMElement();
    view.unmount();
    const reopened = await mount(scheduled());
    expect(reopened.container).toBeEmptyDOMElement();
    await refresh(reopened, scheduled({ announcedAt: "2026-10-13T03:59:00Z", scheduledFor: "2026-10-13T08:00:00Z" }));
    expect(screen.getByText("Resets in 4h 00m")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss upcoming reset" }));
    await refresh(reopened, scheduled());
    expect(reopened.container).toBeEmptyDOMElement();
  });
  it("renders nothing for no reset, stale data or failure", async () => {
    const view = await mount(outlook());
    expect(view.container).toBeEmptyDOMElement();
    await refresh(view, { ...scheduled(), generatedAt: "2026-10-13T03:44:59Z" });
    expect(view.container).toBeEmptyDOMElement();
    mocks.read.mockRejectedValue(new Error("offline"));
    await act(async () => view.rerender(<CodexResetNotice refreshKey={{ ...account }} />));
    expect(view.container).toBeEmptyDOMElement();
  });
  it("hides a formerly fresh announcement after the freshness window", async () => {
    const view = await mount(scheduled());
    await act(async () => vi.advanceTimersByTime(15 * 60_000 + 1000));
    expect(view.container).toBeEmptyDOMElement();
  });
  it("handles undated announcements and hides invalid or passed deadlines", async () => {
    const view = await mount(scheduled({ scheduledFor: null }));
    expect(screen.getByText("Time not announced")).toBeInTheDocument();
    for (const scheduledFor of ["invalid", "2026-10-13T03:59:00Z"]) {
      await refresh(view, scheduled({ scheduledFor }));
      expect(view.container).toBeEmptyDOMElement();
    }
  });
  it("distinguishes extra credit from an automatic reset", async () => {
    await mount(scheduled({ resetType: "banked" }));
    expect(screen.getByText(/current usage may stay unchanged/)).toBeInTheDocument();
  });
  it.each(["focus", "online"])("refreshes upcoming resets on %s", async (event) => {
    await mount(outlook()); mocks.read.mockResolvedValue(scheduled());
    await act(async () => window.dispatchEvent(new Event(event)));
    expect(screen.getByText("Resets in 3h 00m")).toBeInTheDocument();
  });
  it("ignores an older request after a newer focus refresh", async () => {
    let finish!: (data: CodexResetStatus) => void;
    mocks.read.mockReturnValueOnce(new Promise<CodexResetStatus>((resolve) => { finish = resolve; }));
    const view = render(<CodexResetNotice refreshKey={account} />);
    mocks.read.mockResolvedValue(outlook({ latestReset: completed }));
    await act(async () => window.dispatchEvent(new Event("focus")));
    await act(async () => finish(scheduled()));
    expect(view.container).toHaveTextContent("Reset confirmed");
    expect(screen.queryByText("Resets in 3h 00m")).not.toBeInTheDocument();
  });
  it.each(["invalid JSON", JSON.stringify({ bad: true })])("recovers from corrupt dismissal storage: %s", async (value) => {
    localStorage.setItem(RESET_DISMISSAL_KEY, value);
    await mount(scheduled());
    expect(screen.getByText("Resets in 3h 00m")).toBeInTheDocument();
  });
  it("still closes when local storage is unavailable", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("unavailable"); });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("unavailable"); });
    const view = await mount(scheduled());
    fireEvent.click(screen.getByRole("button", { name: "Dismiss upcoming reset" }));
    expect(view.container).toBeEmptyDOMElement();
  });
  it("never fetches per CLI account card", async () => {
    const props = { notify: vi.fn(), onAddAccount: vi.fn(), onShowUsage: vi.fn() };
    await act(async () => render(<CliProviders statuses={[{ ...account, authMethod: "API key" }]} {...props} />));
    expect(mocks.read).not.toHaveBeenCalled();
    cleanup(); mocks.read.mockResolvedValue(scheduled());
    await act(async () => render(<CliProviders statuses={[account]} {...props} />));
    expect(mocks.read).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("Codex reset outlook")).not.toBeInTheDocument();
  });
});

function probable(over: Partial<NonNullable<CodexResetStatus["activeWatch"]>> = {}): CodexResetStatus {
  return outlook({ activeWatch: {
    level: "elevated", resetChancePercent: 40, forecastWindow: "within 24h",
    observedAt: "2026-10-13T03:00:00Z", expiresAt: "2026-10-14T05:00:00Z", ...over,
  } });
}

describe("probable reset controls and polling", () => {
  it("turns estimates off immediately while keeping announced resets visible", async () => {
    const view = await mount(probable());
    expect(screen.getByText("Possible reset")).toBeInTheDocument();
    const settings = { ...DEFAULT_CODEX_RESET_SETTINGS, showProbableResets: false };
    await act(async () => view.rerender(<CodexResetNotice refreshKey={account} settings={settings} />));
    expect(view.container).toBeEmptyDOMElement();
    mocks.read.mockResolvedValue({ ...scheduled(), activeWatch: probable().activeWatch });
    await act(async () => window.dispatchEvent(new Event("focus")));
    expect(screen.getByText("Resets in 3h 00m")).toBeInTheDocument();
    expect(screen.queryByText("Possible reset")).not.toBeInTheDocument();
  });

  it("prefers an announced reset over a forecast and never invents a probability", async () => {
    const view = await mount({ ...scheduled(), activeWatch: probable().activeWatch });
    expect(screen.getByText("Resets in 3h 00m")).toBeInTheDocument();
    expect(screen.queryByText("Possible reset")).not.toBeInTheDocument();
    await refresh(view, probable({ resetChancePercent: null }));
    expect(screen.getByText("Possible reset")).toBeInTheDocument();
    expect(screen.getByText("within 25h, tomorrow at 05:00")).toBeInTheDocument();
    expect(view.container).not.toHaveTextContent(/% chance/);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });

  it("expires a forecast while open and rejects invalid or future watches", async () => {
    const view = await mount(probable({ expiresAt: "2026-10-13T04:00:01Z" }));
    expect(screen.getByText("Possible reset")).toBeInTheDocument();
    await act(async () => vi.advanceTimersByTime(1000));
    expect(view.container).toBeEmptyDOMElement();
    for (const patch of [{ observedAt: "invalid" }, { observedAt: "2026-10-14T04:00:00Z" }, { resetChancePercent: 101 }]) {
      await refresh(view, probable(patch));
      expect(view.container).toBeEmptyDOMElement();
    }
  });

  it("persists a dismissed watch across odds changes/restarts, then shows a new watch", async () => {
    const view = await mount(probable());
    fireEvent.click(screen.getByRole("button", { name: "Dismiss possible reset" }));
    await refresh(view, probable({ resetChancePercent: 80 }));
    expect(view.container).toBeEmptyDOMElement();
    view.unmount();
    const reopened = await mount(probable());
    expect(reopened.container).toBeEmptyDOMElement();
    await refresh(reopened, probable({ observedAt: "2026-10-13T03:59:00Z" }));
    expect(screen.getByText("Possible reset")).toBeInTheDocument();
  });

  it("polls on the chosen interval and replaces the timer when settings change", async () => {
    const view = await mount(probable(), { ...DEFAULT_CODEX_RESET_SETTINGS, refetchIntervalSecs: 60 });
    await act(async () => vi.advanceTimersByTime(59_000));
    expect(mocks.read).toHaveBeenCalledTimes(1);
    await act(async () => vi.advanceTimersByTime(1000));
    expect(mocks.read).toHaveBeenCalledTimes(2);
    await act(async () => view.rerender(<CodexResetNotice refreshKey={account} settings={{ ...DEFAULT_CODEX_RESET_SETTINGS, refetchIntervalSecs: 30 }} />));
    expect(mocks.read).toHaveBeenCalledTimes(3);
    await act(async () => vi.advanceTimersByTime(30_000));
    expect(mocks.read).toHaveBeenCalledTimes(4);
    view.unmount();
    await act(async () => vi.advanceTimersByTime(60_000));
    expect(mocks.read).toHaveBeenCalledTimes(4);
  });

  it("keeps a valid forecast between longer configured refreshes", async () => {
    await mount(probable(), { ...DEFAULT_CODEX_RESET_SETTINGS, refetchIntervalSecs: 3600 });
    await act(async () => vi.advanceTimersByTime(30 * 60_000));
    expect(screen.getByText("Possible reset")).toBeInTheDocument();
    expect(mocks.read).toHaveBeenCalledTimes(1);
  });
});

describe("possible reset timing", () => {
  it("uses a fixed forecast deadline and counts down without refetching", async () => {
    const view = await mount(probable({ expiresAt: "2026-10-14T03:01:00Z" }));
    const time = view.container.querySelector("time");
    expect(time).toHaveTextContent(/^within 24h, tomorrow at 03:01$/);
    expect(time).toHaveAttribute("dateTime", "2026-10-14T03:01:00Z");
    await act(async () => vi.advanceTimersByTime(60_000));
    expect(time).toHaveTextContent(/^within 23h, tomorrow at 03:01$/);
    expect(mocks.read).toHaveBeenCalledTimes(1);
  });

  it("counts down in minutes and disappears at the forecast deadline", async () => {
    const view = await mount(probable({ expiresAt: "2026-10-13T04:02:00Z" }));
    expect(screen.getByText("within 2m, today at 04:02")).toBeInTheDocument();
    await act(async () => vi.advanceTimersByTime(60_000));
    expect(screen.getByText("within 1m, today at 04:02")).toBeInTheDocument();
    await act(async () => vi.advanceTimersByTime(1000));
    expect(screen.getByText("within <1m, today at 04:02")).toBeInTheDocument();
    await act(async () => vi.advanceTimersByTime(59_000));
    expect(view.container).toBeEmptyDOMElement();
  });

  it("uses local calendar days across midnight rather than UTC or a 24-hour cutoff", async () => {
    // @ts-expect-error Node types are not installed in this frontend package.
    const previous = process.env.TZ;
    try {
      // @ts-expect-error Node types are not installed in this frontend package.
      process.env.TZ = "Asia/Tehran";
      vi.setSystemTime(Date.parse("2026-10-13T20:00:00Z")); // 23:30 locally.
      await mount({ ...probable({ expiresAt: "2026-10-13T21:00:00Z" }), generatedAt: new Date().toISOString() });
      expect(screen.getByText("within 1h, tomorrow at 00:30")).toBeInTheDocument();
    } finally {
      // @ts-expect-error Node types are not installed in this frontend package.
      process.env.TZ = previous;
    }
  });

  it("updates tomorrow to today at local midnight, including daylight-saving transitions", () => {
    // @ts-expect-error Node types are not installed in this frontend package.
    const previous = process.env.TZ;
    try {
      // @ts-expect-error Node types are not installed in this frontend package.
      process.env.TZ = "America/New_York";
      const end = "2026-11-02T05:30:00Z";
      expect(resetForecastTiming(end, Date.parse("2026-11-01T04:30:00Z"))).toBe("within 25h, tomorrow at 00:30");
      expect(resetForecastTiming(end, Date.parse("2026-11-02T05:00:00Z"))).toBe("within 30m, today at 00:30");
    } finally {
      // @ts-expect-error Node types are not installed in this frontend package.
      process.env.TZ = previous;
    }
  });

  it("uses a calendar date beyond tomorrow and handles invalid/past bounds", () => {
    expect(resetForecastTiming("2026-10-15T04:22:00Z", NOW)).toBe("within 49h, Oct 15 at 04:22");
    expect(resetForecastTiming("invalid", NOW)).toBe("");
    expect(resetForecastTiming(new Date(NOW).toISOString(), NOW)).toBe("");
  });
});

it("dismisses a confirmed reset and ignores old or future completion timestamps", async () => {
  let view = await mount(outlook({latestReset: completed}));
  fireEvent.click(screen.getByRole("button", {name:"Dismiss confirmed reset"}));
  await refresh(view, outlook({latestReset: completed}));
  expect(view.container).toBeEmptyDOMElement();
  view.unmount();
  for (const announcedAt of ["2026-10-11T03:24:00Z", "2026-10-14T03:24:00Z"]) {
    view = await mount(outlook({latestReset: {...completed,id:announcedAt,announcedAt}}));
    expect(view.container).toBeEmptyDOMElement(); view.unmount();
  }
});
