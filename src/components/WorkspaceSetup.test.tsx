import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { Event } from "@tauri-apps/api/event";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import { WorkspaceAccountForm } from "./WorkspaceSetup";

const native = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: vi.fn() }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: native.listen }) }));
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("submits a Finder drop through the native path import and preserves it after validation errors", async () => {
  let handler!: (event: Event<DragDropEvent>) => void;
  native.listen.mockImplementation((callback) => { handler = callback; return Promise.resolve(vi.fn()); });
  const save = vi.fn().mockRejectedValueOnce(new Error("This login has expired.")).mockResolvedValueOnce(undefined);
  const user = userEvent.setup();
  render(<WorkspaceAccountForm onSave={save} submitLabel="Add account" />);
  expect(screen.queryByLabelText("Account name")).not.toBeInTheDocument();
  const zone = screen.getByRole("group", { name: "Auth file upload" });
  vi.spyOn(zone, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, right: 200, bottom: 200 } as DOMRect);
  act(() => handler({ event: "test", id: 1, payload: { type: "drop", paths: ["/tmp/Work Account/auth.json"], position: { x: 100, y: 100 } } } as Event<DragDropEvent>));
  expect(screen.getByText("auth.json")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Add account" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("This login has expired.");
  expect(screen.getByText("Ready to import")).toBeInTheDocument();
  expect(save).toHaveBeenCalledWith({ accountName: "", path: "/tmp/Work Account/auth.json" });
  await user.click(screen.getByRole("button", { name: "Add account" }));
  expect(await screen.findByText("Drop auth.json here")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Add account" })).toBeDisabled();
});
