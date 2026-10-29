import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { CodexRestartBanner, useCodexRestart } from "./CodexRestartBanner";

const api = vi.hoisted(() => ({ codexRestartStatus: vi.fn(), scheduleCodexRestart: vi.fn(), cancelCodexRestart: vi.fn(), runCodexRestartNow: vi.fn() }));
vi.mock("../api", async original => ({ ...await original<typeof import("../api")>(), ...api }));
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.clearAllMocks(); });

it("starts manual switching immediately without countdown controls", async () => {
  const start = Date.parse("2026-10-25T10:00:00Z");
  const clock = vi.spyOn(Date, "now").mockReturnValue(start);
  api.codexRestartStatus.mockResolvedValue(null);
  api.scheduleCodexRestart.mockImplementation(async () => {
    clock.mockReturnValue(start + 500);
    return { profileId: "work", profileName: "Work", deadlineAt: new Date(start + 500).toISOString(), phase: "quitting", error: null, processId: null, credentialsVerified: false };
  });
  const notify = vi.fn(), completed = vi.fn();
  function Harness() {
    const restart = useCodexRestart(notify, completed);
    return <><button onClick={() => void restart.schedule("work")}>Switch</button><CodexRestartBanner restart={restart} /></>;
  }
  render(<Harness />);
  fireEvent.click(screen.getByRole("button", { name: /^Switch$/ }));
  expect(await screen.findByRole("status")).toHaveTextContent("Switching to Work…");
  expect(screen.queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Switch now" })).not.toBeInTheDocument();
});

it("reports the confirmed destination immediately when the native switch is verified", async () => {
  api.codexRestartStatus.mockResolvedValue({ profileId: "new", profileName: "New account", deadlineAt: "2026-10-29T10:00:00Z", phase: "verified", error: null, credentialsVerified: true, processId: 123 });
  const completed = vi.fn();
  function Harness() { useCodexRestart(vi.fn(), completed); return null; }
  render(<Harness />);
  const { waitFor } = await import("@testing-library/react");
  await waitFor(() => expect(completed).toHaveBeenCalledWith("new"));
});

it("asks before closing active threads and cancels without completion", async () => {
  const status = { profileId: "work", profileName: "Work", deadlineAt: new Date().toISOString(), phase: "awaiting_confirmation", error: null, processId: null, credentialsVerified: false, runningThreads: 2, activityUncertain: false };
  api.codexRestartStatus.mockResolvedValue(status); api.cancelCodexRestart.mockResolvedValue({ ...status, phase: "cancelled" });
  const completed = vi.fn();
  function Harness() { const restart = useCodexRestart(vi.fn(), completed); return <CodexRestartBanner restart={restart} />; }
  render(<Harness />);
  expect(await screen.findByRole("dialog", { name: "Thread in progress" })).toHaveTextContent("2 Codex threads are still running");
  expect(screen.queryByText(/Switching in/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  const { waitFor } = await import("@testing-library/react");
  await waitFor(() => expect(api.cancelCodexRestart).toHaveBeenCalled());
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(completed).not.toHaveBeenCalled();
});
it("labels unknown activity honestly and resumes only after Close and switch", async () => {
  const status = { profileId: "work", profileName: "Work", deadlineAt: new Date().toISOString(), phase: "awaiting_confirmation", error: null, processId: null, credentialsVerified: false, runningThreads: 0, activityUncertain: true };
  api.codexRestartStatus.mockResolvedValue(status); api.runCodexRestartNow.mockResolvedValue({ ...status, phase: "quitting" });
  function Harness() { const restart = useCodexRestart(vi.fn(), vi.fn()); return <CodexRestartBanner restart={restart} />; }
  render(<Harness />);
  expect(await screen.findByRole("dialog", { name: "Check running threads" })).toHaveTextContent("couldn’t confirm");
  expect(api.runCodexRestartNow).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Close and switch" }));
  const { waitFor } = await import("@testing-library/react");
  await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Switching to Work"));
  expect(api.runCodexRestartNow).toHaveBeenCalled();
});
