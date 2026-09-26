import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  accountStatus,
  activateAccountKey,
  addProvider,
  applyKey,
  checkUpdates,
  codexRefresh,
  codexStatus,
  connectAccount,
  consoleKeys,
  consoleUsage,
  deleteKey,
  disconnectAccount,
  exportHistory,
  getState,
  history,
  listSounds,
  playSound,
  prettyError,
  profileCreate,
  profileDelete,
  profileRename,
  profileSwitch,
  quitApp,
  removeProvider,
  saveKey,
  setActiveAccount,
  setAppSettings,
  setSettings,
} from "./api";

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue("ok");
});

describe("api invoke wrappers", () => {
  it("sends the exact command name and payload the backend expects", async () => {
    const month = { year: 2026, month: 8 };
    const cases: Array<[Promise<unknown>, string, unknown]> = [
      [getState(null), "get_state", { month: null, refresh: false }],
      [getState(month, true), "get_state", { month, refresh: true }],
      [
        saveKey({ provider: "opencode", name: "main", apiKey: "sk-1", account: "a@b.c" }),
        "save_key",
        { provider: "opencode", name: "main", apiKey: "sk-1", account: "a@b.c" },
      ],
      [deleteKey("opencode", "main"), "delete_key", { provider: "opencode", name: "main" }],
      [applyKey("opencode", "main"), "apply_key", { provider: "opencode", name: "main" }],
      [addProvider("custom"), "add_provider", { provider: "custom" }],
      [removeProvider("custom"), "remove_provider", { provider: "custom" }],
      [setSettings(true), "set_settings", { openAtLogin: true }],
      [setAppSettings({ autoSwitch: true }), "set_app_settings", { settings: { autoSwitch: true } }],
      [profileCreate("Work"), "profile_create", { name: "Work" }],
      [profileRename("p1", "Home"), "profile_rename", { id: "p1", name: "Home" }],
      [profileSwitch("p1"), "profile_switch", { id: "p1" }],
      [profileDelete("p1"), "profile_delete", { id: "p1" }],
      [listSounds(), "list_sounds", undefined],
      [playSound("Glass"), "play_sound", { name: "Glass" }],
      [codexStatus(), "codex_status", undefined],
      [codexRefresh(), "codex_refresh", undefined],
      [history("7d"), "history", { range: "7d", key: null }],
      [history("7d", "main"), "history", { range: "7d", key: "main" }],
      [exportHistory("30d", "csv"), "export_history", { range: "30d", format: "csv", key: null }],
      [checkUpdates(), "check_updates", undefined],
      [accountStatus(), "account_status", undefined],
      [connectAccount("opencode"), "connect_account_webview", { provider: "opencode", account: undefined }],
      [connectAccount("opencode", "a@b.c"), "connect_account_webview", { provider: "opencode", account: "a@b.c" }],
      [disconnectAccount("acc"), "disconnect_account", { account: "acc" }],
      [setActiveAccount("acc"), "set_active_account_cmd", { account: "acc" }],
      [consoleKeys("acc", month), "console_keys", { account: "acc", month, refresh: false }],
      [consoleUsage("acc", month, true), "console_usage", { account: "acc", month, refresh: true }],
      [activateAccountKey("k1", "acc"), "activate_account_key", { keyId: "k1", account: "acc" }],
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
        new Error("Error invoking remote method 'save_key': Error: a key with that name already exists"),
      ),
    ).toBe("a key with that name already exists");
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
