import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  cliStatus,
  getState,
  prettyError,
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
      [setAppSettings({ refreshIntervalSecs: 120 }), "set_app_settings", { settings: { refreshIntervalSecs: 120 } }],
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
    invokeMock.mockResolvedValue([{ id: "claude" }]);
    await expect(cliStatus()).resolves.toEqual([{ id: "claude" }]);
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
