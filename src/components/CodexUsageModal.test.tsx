import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CodexUsageModal } from "./CodexUsageModal";
import type { BankedResetCredit, BankedResetInventory } from "../api";
import type { CodexProfile } from "../types";
const api = vi.hoisted(() => ({ inventory: vi.fn(), prepare: vi.fn(), consume: vi.fn() }));
vi.mock("../api", async (original) => ({ ...await original<typeof import("../api")>(), codexBankedResets: api.inventory, prepareCodexReset: api.prepare, consumeCodexReset: api.consume }));
const profile: CodexProfile = { id: "inactive", name: "Backup", email: "backup@example.com", plan: "plus", connected: true, active: false, needsActivation: false, connectionError: null, loginCommand: "" };
const credit: BankedResetCredit = { id: "first", title: "Full reset (Weekly + 5 hr)", expiresAt: "2099-11-30T12:00:00Z", coveredWindows: ["5-hour", "Weekly"], canUse: true, unavailableReason: null };
const inventory = (changes: Partial<BankedResetInventory> = {}): BankedResetInventory => ({ availableCount: 1, credits: [credit], pendingCredit: null, usage: { provider: "codex", windows: [{ window: "Weekly", percent: 100 }] }, ...changes });
const notify = vi.fn(), used = vi.fn(), close = vi.fn();
const props = () => ({ profile, refreshKey: 0, showRemaining: true, onClose: close, onReconnect: vi.fn(), onUsed: used, notify });
beforeEach(() => { vi.clearAllMocks(); api.inventory.mockResolvedValue(inventory()); api.prepare.mockResolvedValue({ token: "review-1", credit, retry: false }); api.consume.mockResolvedValue({ code: "reset", windows_reset: 2 }); });
afterEach(cleanup);
it("reads an inactive account but spends nothing until the separate explicit confirmation", async () => {
  const user=userEvent.setup(); const base=props(); const view=render(<CodexUsageModal {...base} />);
  expect(await screen.findByText("Full reset (Weekly + 5 hr)")).toBeInTheDocument();
  expect(api.inventory).toHaveBeenCalledWith("inactive"); expect(api.consume).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button",{name:"Use reset"}));
  expect(await screen.findByRole("heading",{name:"Use 1 reset?"})).toBeInTheDocument();
  expect(screen.getByText("backup@example.com")).toBeInTheDocument(); expect(screen.getByText(/Affected refresh times/)).toBeInTheDocument();
  expect(api.prepare).toHaveBeenCalledWith("inactive","first"); expect(api.consume).not.toHaveBeenCalled();
  api.inventory.mockResolvedValue(inventory({ availableCount: 0, credits: [], usage: { provider:"codex", windows:[{window:"Weekly",percent:0}] } }));
  await user.click(screen.getByRole("button",{name:"Use 1 reset"}));
  expect(api.consume).toHaveBeenCalledWith("inactive","review-1"); expect(await screen.findByText("Reset used")).toBeInTheDocument(); expect(used).toHaveBeenCalledTimes(1);
  view.rerender(<CodexUsageModal {...base} refreshKey={1} />);
  await waitFor(() => expect(screen.queryByLabelText("Loading usage and resets")).not.toBeInTheDocument()); expect(screen.queryByLabelText("Banked resets")).not.toBeInTheDocument(); expect(screen.getByText("100%")).toBeInTheDocument(); expect(close).not.toHaveBeenCalled();
});
it("Cancel and Back never consume a reset", async () => {
  const user=userEvent.setup();render(<CodexUsageModal {...props()} />);
  await user.click(await screen.findByRole("button",{name:"Use reset"}));
  await user.click(await screen.findByRole("button",{name:"Cancel"}));
  expect(await screen.findByRole("heading",{name:"Usage limits"})).toBeInTheDocument();expect(api.consume).not.toHaveBeenCalled();
});
it("shows a server-ineligible reset card with only its disabled action",async()=>{
  api.inventory.mockResolvedValue(inventory({credits:[{...credit,canUse:false,unavailableReason:"Codex says this reset is not applicable."}]}));render(<CodexUsageModal {...props()} />);
  expect(await screen.findByText("Full reset (Weekly + 5 hr)")).toBeInTheDocument();expect(screen.getByRole("button",{name:"Use reset"})).toBeDisabled();expect(screen.queryByText(/not applicable/)).not.toBeInTheDocument(); expect(screen.queryByText(/Restores/)).not.toBeInTheDocument();expect(api.prepare).not.toHaveBeenCalled();
});
it("shows a read failure as unknown with Retry, never zero",async()=>{
  api.inventory.mockRejectedValueOnce(new Error("Network unavailable")); const user=userEvent.setup();render(<CodexUsageModal {...props()} />);
  expect(await screen.findByRole("alert")).toHaveTextContent("Network unavailable");expect(screen.queryByText("No resets available")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button",{name:"Retry"}));expect(await screen.findByText("Full reset (Weekly + 5 hr)")).toBeInTheDocument();
});
it("blocks dismissal and repeated clicks while confirming, then retries the same review after an unknown result",async()=>{
  const user=userEvent.setup();let reject!: (e:Error)=>void;api.consume.mockImplementationOnce(()=>new Promise((_,no)=>{reject=no;}));render(<CodexUsageModal {...props()} />);
  await user.click(await screen.findByRole("button",{name:"Use reset"}));await user.click(await screen.findByRole("button",{name:"Use 1 reset"}));
  expect(screen.getByRole("button",{name:"Using reset…"})).toBeDisabled();expect(screen.getByRole("button",{name:"Close"})).toBeDisabled();await user.keyboard("{Escape}");expect(close).not.toHaveBeenCalled();
  await act(async()=>reject(new Error("Reset result is unknown")));await user.click(await screen.findByRole("button",{name:"Retry same reset"}));
  expect(api.consume.mock.calls).toEqual([["inactive","review-1"],["inactive","review-1"]]);
});
it("recovers a pending attempt after reopening and blocks new credits",async()=>{
  api.inventory.mockResolvedValue(inventory({pendingCredit:credit}));api.prepare.mockResolvedValue({token:"recovered",credit,retry:true});const user=userEvent.setup();render(<CodexUsageModal {...props()} />);
  await user.click(await screen.findByRole("button",{name:"Check previous attempt"}));
  expect(await screen.findByText(/without spending a second reset/)).toBeInTheDocument();expect(screen.getByRole("button",{name:"Retry same reset"})).toBeInTheDocument();expect(api.consume).not.toHaveBeenCalled();
});
it("rejects a delayed read from an unmounted identity",async()=>{
  let finish!:(data:BankedResetInventory)=>void;api.inventory.mockReturnValueOnce(new Promise(resolve=>{finish=resolve;}));const view=render(<CodexUsageModal {...props()} />);view.unmount();
  render(<CodexUsageModal {...props()} profile={{...profile,id:"other",name:"Other"}} />);await screen.findByText("Full reset (Weekly + 5 hr)");
  await act(async()=>finish(inventory({availableCount:99})));expect(screen.queryByText("99 resets available")).not.toBeInTheDocument();
});
it("doesn't call a no-op or already-used response a newly used reset",async()=>{
  api.consume.mockResolvedValue({code:"nothing_to_reset",windows_reset:0});const user=userEvent.setup();render(<CodexUsageModal {...props()} />);
  await user.click(await screen.findByRole("button",{name:"Use reset"}));await user.click(await screen.findByRole("button",{name:"Use 1 reset"}));
  await waitFor(()=>expect(screen.getByText(/No reset was used/)).toBeInTheDocument());expect(screen.queryByText("Reset used")).not.toBeInTheDocument();
});
it("lets the user choose a different reset before review",async()=>{
  api.inventory.mockResolvedValue(inventory({availableCount:2,credits:[credit,{...credit,id:"later",title:"Later reset",expiresAt:null}]}));
  const user=userEvent.setup();render(<CodexUsageModal {...props()} />);
  await user.click(await screen.findByRole("combobox",{name:"Choose reset"}));
  await user.click(screen.getByRole("option",{name:/Later reset/}));
  await user.click(screen.getByRole("button",{name:"Use reset"}));
  expect(api.prepare).toHaveBeenCalledWith("inactive","later");expect(api.consume).not.toHaveBeenCalled();
});
it("reveals usage and resets together after a shared loading state, even with cached quota",async()=>{
  let finish!:(data:BankedResetInventory)=>void;
  api.inventory.mockReturnValueOnce(new Promise(resolve=>{finish=resolve;}));
  render(<CodexUsageModal {...props()} quota={{provider:"codex",windows:[{window:"Weekly",percent:12}]}} />);
  expect(screen.getByLabelText("Loading usage and resets")).toBeInTheDocument();
  expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();expect(screen.queryByText("Full reset (Weekly + 5 hr)")).not.toBeInTheDocument();
  await act(async()=>finish(inventory()));
  expect(screen.queryByLabelText("Loading usage and resets")).not.toBeInTheDocument();
  expect(screen.getByText("0%")).toBeInTheDocument();expect(screen.getByText("Full reset (Weekly + 5 hr)")).toBeInTheDocument();expect(screen.getByText(/^Expires in/)).toBeInTheDocument();
});
it("shows no reset section or zero-balance message when the account has none",async()=>{
  api.inventory.mockResolvedValue(inventory({availableCount:0,credits:[]}));render(<CodexUsageModal {...props()} />);
  await waitFor(()=>expect(screen.queryByLabelText("Loading usage and resets")).not.toBeInTheDocument());
  expect(screen.getByText("0%")).toBeInTheDocument();expect(screen.queryByLabelText("Banked resets")).not.toBeInTheDocument();
  expect(screen.queryByText(/No resets available|Available resets/)).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:"Use reset"})).not.toBeInTheDocument();
});
it("reloads both usage and resets behind one placeholder",async()=>{
  const user=userEvent.setup();render(<CodexUsageModal {...props()} />);await screen.findByText("Full reset (Weekly + 5 hr)");
  let finish!:(data:BankedResetInventory)=>void;api.inventory.mockReturnValueOnce(new Promise(resolve=>{finish=resolve;}));
  await user.click(screen.getByRole("button",{name:"Refresh usage and resets"}));
  expect(screen.getByLabelText("Loading usage and resets")).toBeInTheDocument();expect(screen.queryByText("Full reset (Weekly + 5 hr)")).not.toBeInTheDocument();expect(screen.queryByText("0%")).not.toBeInTheDocument();
  await act(async()=>finish(inventory({availableCount:0,credits:[],usage:{provider:"codex",windows:[{window:"Weekly",percent:25}]}})));
  expect(screen.getByText("75%")).toBeInTheDocument();expect(screen.queryByLabelText("Banked resets")).not.toBeInTheDocument();
});
