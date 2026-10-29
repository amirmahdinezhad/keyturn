import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { makeState } from "../test/fixtures";
import type { AppSettings } from "../types";
const api = vi.hoisted(() => ({ setAppSettings: vi.fn(), prettyError: (e: unknown) => String(e) }));
vi.mock("../api", () => api);
import { CodexDisplaySettings } from "./CodexDisplaySettings";
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

it("toggles the menu bar and chooses active or selected accounts", async () => {
  function Harness() {
    const [state, setState] = useState(makeState());
    api.setAppSettings.mockImplementation(async (patch: Partial<AppSettings>) => ({ ...state, app: { ...state.app, ...patch } }));
    return <CodexDisplaySettings app={state.app} onChanged={setState} notify={() => {}} />;
  }
  const user = userEvent.setup(); render(<Harness />);
  expect(screen.queryByRole("switch", { name: "Alerts module" })).not.toBeInTheDocument();
  expect(screen.getByRole("switch", { name: "Menu bar module" })).not.toBeChecked();
  await user.click(screen.getByRole("switch", { name: "Menu bar module" }));
  expect(screen.getByRole("switch", { name: "Menu bar module" })).toBeChecked();
  await user.click(screen.getByRole("combobox", { name: "Codex accounts to display" }));
  await user.click(screen.getByRole("option", { name: "Selected accounts" }));
  expect(api.setAppSettings).toHaveBeenLastCalledWith({ codexDisplayMode: "selected" });
});
