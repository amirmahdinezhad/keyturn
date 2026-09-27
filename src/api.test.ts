import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  checkUpdates,
  cliStatus,
  exportHistory,
  getState,
  history,
  prettyError,
  profileCreate,
  profileDelete,
  profileRename,
  profileSwitch,
  quitApp,
  setAppSettings,
  setSettings,
} from "./api";

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue("ok");
});

describe("api invoke wrappers", () => {
  it("sends the exact command name and payload the backend expects", async () => {
    const cases: Array<[Promise<unknown>, string, unknown]> = [
      [getState(), "get_state", undefined],
      [setSettings(true), "set_settings", { openAtLogin: true }],
      [setAppSettings({ billingDay: 15 }), "set_app_settings", { settings: { billingDay: 15 } }],
      [profileCreate("Work"), "profile_create", { name: "Work" }],
      [profileRename("p1", "Home"), "profile_rename", { id: "p1", name: "Home" }],
      [profileSwitch("p1"), "profile_switch", { id: "p1" }],
      [profileDelete("p1"), "profile_delete", { id: "p1" }],
      [history("7d"), "history", { range: "7d" }],
      [exportHistory("30d", "csv"), "export_history", { range: "30d", format: "csv" }],
      [checkUpdates(), "check_updates", undefined],
      [cliStatus(), "cli_status", undefined],
      [quitApp(), "quit_app", undefined],
    ];

    for (const [promise] of cases) {
      await promise;
    }

    expect(invokeMock.mock.calls.map(([command]) => command)).toEqual(
      cases.map(([, command]) => command),
    );
    cases.forEach(([, , payload], index) => {
      const [, actual] = invokeMock.mock.calls[index];
      expect(actual).toEqual(payload);
    });
  });

  it("resolves with whatever the backend returned", async () => {
    invokeMock.mockResolvedValue({ available: true, version: "9.9.9" });
    await expect(checkUpdates()).resolves.toEqual({ available: true, version: "9.9.9" });
  });
});

describe("prettyError", () => {
  it("strips Tauri's invoke prefix", () => {
    expect(
      prettyError(
        new Error("Error invoking remote method 'get_state': Error: disk is full"),
      ),
    ).toBe("disk is full");
  });

  it("strips the prefix without the nested Error marker", () => {
    expect(prettyError(new Error("Error invoking remote method 'get_state': disk is full"))).toBe(
      "disk is full",
    );
  });

  it("leaves plain messages alone and accepts non-errors", () => {
    expect(prettyError(new Error("nothing to strip"))).toBe("nothing to strip");
    expect(prettyError("raw string")).toBe("raw string");
  });
});
