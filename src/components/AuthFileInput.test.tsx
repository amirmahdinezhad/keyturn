import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Event } from "@tauri-apps/api/event";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import { AuthFileInput } from "./AuthFileInput";

const native = vi.hoisted(() => ({ enabled: false, listen: vi.fn(), stop: vi.fn(), invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => native.enabled, invoke: native.invoke }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: native.listen }) }));
const credentials = '{"tokens":{"synthetic":"test-only"}}';
const file = () => new File([credentials], "auth.json", { type: "application/json" });
let handler: (event: Event<DragDropEvent>) => void;
const props = () => ({ value: null, disabled: false, onChange: vi.fn(), onReadingChange: vi.fn() });
beforeEach(() => {
  vi.clearAllMocks(); native.enabled = false;
  native.invoke.mockResolvedValue(undefined);
  native.listen.mockImplementation((next) => { handler = next; return Promise.resolve(native.stop); });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("opens the chooser and reads one selected JSON file", async () => {
  const user = userEvent.setup(); const callbacks = props(); render(<AuthFileInput {...callbacks} />);
  const input = screen.getByLabelText("Choose auth.json file");
  const click = vi.spyOn(input, "click");
  await user.click(screen.getByRole("button", { name: "Choose file" }));
  expect(click).toHaveBeenCalledTimes(1);
  await user.upload(input, file());
  await waitFor(() => expect(callbacks.onChange).toHaveBeenLastCalledWith({ name: "auth.json", credentials }));
  expect(callbacks.onReadingChange.mock.calls).toEqual([[true], [false]]);
});

it("accepts a browser file drop and resets its hover state", async () => {
  const callbacks = props(); render(<AuthFileInput {...callbacks} />);
  const zone = screen.getByRole("group", { name: "Auth file upload" });
  fireEvent.dragEnter(zone, { dataTransfer: { types: ["Files"] } });
  expect(zone).toHaveClass("is-dragging");
  fireEvent.dragLeave(zone, { relatedTarget: document.body });
  expect(zone).not.toHaveClass("is-dragging");
  fireEvent.drop(zone, { dataTransfer: { files: [file()] } });
  await waitFor(() => expect(callbacks.onChange).toHaveBeenLastCalledWith({ name: "auth.json", credentials }));
  expect(native.listen).not.toHaveBeenCalled();
});

it.each([
  ["multiple files", () => [file(), file()], "Choose one auth.json file at a time."],
  ["a non-JSON file", () => [new File([credentials], "auth.txt")], "Choose a JSON file"],
  ["an oversized file", () => [new File(["x".repeat(256 * 1024 + 1)], "auth.json")], "smaller than 256 KB"],
  ["malformed JSON", () => [new File(["not JSON"], "auth.json")], "not valid auth.json JSON"],
] as const)("rejects %s without retaining an earlier selection", async (_label, files, message) => {
  const callbacks = props(); render(<AuthFileInput {...callbacks} value={{ name: "earlier.json", credentials }} />);
  fireEvent.drop(screen.getByRole("group"), { dataTransfer: { files: files() } });
  expect(await screen.findByRole("alert")).toHaveTextContent(message);
  expect(callbacks.onChange).toHaveBeenCalledExactlyOnceWith(null);
});

it("allows the same file to be selected again and reports read failures", async () => {
  const callbacks = props(); render(<AuthFileInput {...callbacks} />);
  const read = vi.spyOn(FileReader.prototype, "readAsText").mockImplementation(function (this: FileReader) {
    this.dispatchEvent(new ProgressEvent("error"));
    this.dispatchEvent(new ProgressEvent("loadend"));
  });
  const input = screen.getByLabelText("Choose auth.json file");
  await userEvent.setup().upload(input, file());
  expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't read this file");
  expect(input).toHaveValue("");
  expect(callbacks.onReadingChange).toHaveBeenLastCalledWith(false);
  read.mockRestore();
  await userEvent.setup().upload(input, file());
  await waitFor(() => expect(callbacks.onChange).toHaveBeenLastCalledWith({ name: "auth.json", credentials }));
});

it("ignores file drops and disables the chooser during save", () => {
  const callbacks = props(); render(<AuthFileInput {...callbacks} disabled />);
  fireEvent.drop(screen.getByRole("group"), { dataTransfer: { files: [file()] } });
  expect(callbacks.onChange).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Choose file" })).toBeDisabled();
});

it("aborts a pending file read when the form is discarded", async () => {
  const callbacks = props();
  vi.spyOn(FileReader.prototype, "readAsText").mockImplementation(() => {});
  const abort = vi.spyOn(FileReader.prototype, "abort");
  const { unmount } = render(<AuthFileInput {...callbacks} />);
  await userEvent.setup().upload(screen.getByLabelText("Choose auth.json file"), file());
  expect(screen.getByRole("group")).toHaveAttribute("aria-busy", "true");
  unmount();
  expect(abort).toHaveBeenCalledTimes(1);
  expect(callbacks.onChange).toHaveBeenCalledExactlyOnceWith(null);
});

describe("native Finder drops", () => {
  it("keeps the panel open until file import is unmounted", async () => {
    native.enabled = true;
    const { rerender, unmount } = render(<AuthFileInput {...props()} />);
    await waitFor(() => expect(native.invoke).toHaveBeenCalledExactlyOnceWith("set_panel_file_import", { active: true }));
    rerender(<AuthFileInput {...props()} disabled />);
    expect(native.invoke).toHaveBeenCalledTimes(1);
    unmount();
    await waitFor(() => expect(native.invoke).toHaveBeenLastCalledWith("set_panel_file_import", { active: false }));
  });

  it("orders cleanup before a replacement import even while IPC is pending", async () => {
    native.enabled = true;
    let finish!: () => void;
    native.invoke.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
    const first = render(<AuthFileInput {...props()} />);
    await waitFor(() => expect(native.invoke).toHaveBeenCalledTimes(1));
    first.unmount();
    const second = render(<AuthFileInput {...props()} />);
    expect(native.invoke).toHaveBeenCalledTimes(1);
    finish();
    await waitFor(() => expect(native.invoke.mock.calls.map(([, args]) => args.active)).toEqual([true, false, true]));
    second.unmount();
    await waitFor(() => expect(native.invoke).toHaveBeenLastCalledWith("set_panel_file_import", { active: false }));
  });
  async function setup() {
    native.enabled = true; const callbacks = props();
    const view = render(<AuthFileInput {...callbacks} />);
    await waitFor(() => expect(native.listen).toHaveBeenCalled());
    vi.spyOn(screen.getByRole("group"), "getBoundingClientRect").mockReturnValue({ left: 10, top: 20, right: 210, bottom: 220 } as DOMRect);
    vi.spyOn(window, "devicePixelRatio", "get").mockReturnValue(2);
    return { ...view, callbacks };
  }
  function emit(payload: object) { act(() => handler({ event: "test", id: 1, payload } as Event<DragDropEvent>)); }

  it("scales native coordinates and accepts a single path only inside the drop area", async () => {
    const { callbacks } = await setup();
    emit({ type: "enter", paths: ["/tmp/Work Account/auth.json"], position: { x: 100, y: 100 } });
    expect(screen.getByRole("group")).toHaveClass("is-dragging");
    emit({ type: "drop", paths: ["/tmp/Work Account/auth.json"], position: { x: 100, y: 100 } });
    expect(callbacks.onChange).toHaveBeenLastCalledWith({ name: "auth.json", path: "/tmp/Work Account/auth.json" });
    expect(screen.getByRole("group")).not.toHaveClass("is-dragging");
    callbacks.onChange.mockClear();
    emit({ type: "drop", paths: ["/tmp/other.json"], position: { x: 500, y: 500 } });
    expect(callbacks.onChange).not.toHaveBeenCalled();
  });

  it("rejects multiple files and non-JSON native paths", async () => {
    const { callbacks } = await setup();
    emit({ type: "drop", paths: ["/tmp/a.json", "/tmp/b.json"], position: { x: 100, y: 100 } });
    expect(screen.getByRole("alert")).toHaveTextContent("Choose one auth.json file at a time.");
    emit({ type: "drop", paths: ["/tmp/auth.txt"], position: { x: 100, y: 100 } });
    expect(screen.getByRole("alert")).toHaveTextContent("Choose a JSON file");
    expect(callbacks.onChange.mock.calls).toEqual([[null], [null]]);
  });

  it("uses current disabled state and removes native listeners on unmount", async () => {
    const { callbacks, rerender, unmount } = await setup();
    rerender(<AuthFileInput {...callbacks} disabled />);
    emit({ type: "drop", paths: ["/tmp/auth.json"], position: { x: 100, y: 100 } });
    expect(callbacks.onChange).not.toHaveBeenCalled();
    unmount();
    expect(native.stop).toHaveBeenCalledTimes(1);
  });

  it("cleans up a listener that finishes registering after unmount", async () => {
    native.enabled = true;
    let resolve!: (stop: () => void) => void;
    native.listen.mockReturnValue(new Promise<() => void>((done) => { resolve = done; }));
    const { unmount } = render(<AuthFileInput {...props()} />);
    unmount();
    await act(async () => resolve(native.stop));
    expect(native.stop).toHaveBeenCalledTimes(1);
  });
});
