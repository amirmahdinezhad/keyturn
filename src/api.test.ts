import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  beginAccountSetup, discardAccountSetup, loadSetupAccounts, startSetupLogin, pollSetupLogin, cancelSetupLogin, submitSetupCode, importSetupAccount, connectSetupConfig, connectCurrentGoogle, commitAccountSetup, removeWorkspaceAccount,
  cliStatus,
  renameAntigravityAccount,
  claudeProfileQuota,
  codexProfiles,
  createCodexProfile,
  renameCodexProfile,
  removeCodexProfile,
  activateCodexProfile,
  importCodexProfile, startCodexProfileLogin, codexProfileLoginStatus, cancelCodexProfileLogin,
  scheduleCodexRestart, codexRestartStatus, cancelCodexRestart, runCodexRestartNow,
  codexResetStatus,
  getState,
  prettyError,
  quitApp,
  setAppSettings,
  setCodexAccountPreferences,
  setMenuBarAccent,
  setSettings,
  includeWorkspaceProvider, discoverWorkspaceAccounts, createDiscoveredWorkspace, createWorkspace, importWorkspaceAccount, updateWorkspace, selectWorkspace, moveWorkspaceAccount,
} from "./api";
import { DEFAULT_CODEX_ACCOUNT_PREFERENCES, DEFAULT_WORKSPACE_USAGE } from "./types";

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue("ok");
});

describe("api invoke wrappers", () => {
  it("sends the exact command name and payload the backend expects", async () => {
    const cases: Array<[Promise<unknown>, string, unknown]> = [
      [renameAntigravityAccount("agy-id", "Office"), "rename_antigravity_account", { id: "agy-id", name: "Office" }],
      [getState(), "get_state", undefined],
      [beginAccountSetup(), "begin_account_setup", undefined],
      [discardAccountSetup("draft"), "discard_account_setup", {sessionId:"draft"}],
      [loadSetupAccounts("draft"), "load_setup_accounts", {sessionId:"draft"}],
      [startSetupLogin("draft","codex","device","Work"), "start_setup_login", {sessionId:"draft",provider:"codex",method:"device",name:"Work"}],
      [pollSetupLogin("draft","attempt"), "poll_setup_login", {sessionId:"draft",attemptId:"attempt"}],
      [cancelSetupLogin("draft"), "cancel_setup_login", {sessionId:"draft"}],
      [submitSetupCode("draft","code"), "submit_setup_code", {sessionId:"draft",code:"code"}],
      [importSetupAccount("draft","codex","Work",{path:"/tmp/auth.json"}), "import_setup_account", {sessionId:"draft",provider:"codex",name:"Work",path:"/tmp/auth.json"}],
      [connectSetupConfig("draft","Claude","/tmp/config"), "connect_setup_config", {sessionId:"draft",name:"Claude",path:"/tmp/config"}],
      [connectCurrentGoogle("draft","Google"), "connect_current_google", {sessionId:"draft",name:"Google"}],
      [commitAccountSetup("draft","Studio","#007aff",["one"],"studio"), "commit_account_setup", {sessionId:"draft",name:"Studio",accent:"#007aff",accountIds:["one"],workspaceId:"studio"}],
      [removeWorkspaceAccount("studio","codex","one",false), "remove_workspace_account", {workspaceId:"studio",provider:"codex",id:"one",everywhere:false}],
      [includeWorkspaceProvider("studio", "claude"), "include_workspace_provider", { workspaceId: "studio", providerId: "claude" }],
      [discoverWorkspaceAccounts(), "discover_workspace_accounts", undefined],
      [createDiscoveredWorkspace("Studio", "#007aff", ["claude"], { claude: "work@example.com" }), "create_discovered_workspace", { name: "Studio", accent: "#007aff", providerIds: ["claude"], accountEmails: { claude: "work@example.com" } }],
      [claudeProfileQuota("team"), "claude_profile_quota", { id: "team" }],
      [claudeProfileQuota(), "claude_profile_quota", { id: null }],
      [createWorkspace("Studio", "#7c6cf0", { accountName: "Personal", credentials: "fixture" }), "create_workspace", { name: "Studio", accent: "#7c6cf0", accountName: "Personal", credentials: "fixture" }],
      [importWorkspaceAccount("studio", { accountName: "Work", path: "~/auth.json" }), "import_workspace_account", { workspaceId: "studio", accountName: "Work", path: "~/auth.json" }],
      [updateWorkspace({ id: "studio", name: "Studio", accent: "#007aff", accountIds: [], usage: DEFAULT_WORKSPACE_USAGE }), "update_workspace", { id: "studio", name: "Studio", accent: "#007aff", usage: DEFAULT_WORKSPACE_USAGE }],
      [selectWorkspace("studio"), "select_workspace", { id: "studio" }],
      [moveWorkspaceAccount("personal", "studio"), "move_workspace_account", { id: "personal", targetId: "studio" }],
      [setSettings(true), "set_settings", { openAtLogin: true }],
      [setAppSettings({ refreshIntervalSecs: 120 }), "set_app_settings", { settings: { refreshIntervalSecs: 120 } }],
      [setCodexAccountPreferences("second", DEFAULT_CODEX_ACCOUNT_PREFERENCES, "first"), "set_codex_account_preferences", { id: "second", preferences: DEFAULT_CODEX_ACCOUNT_PREFERENCES, startCycleWith: "first" }],
      [setMenuBarAccent("#7c6cf0"), "set_menu_bar_accent", { accent: "#7c6cf0" }],
      [cliStatus(), "cli_status", undefined],
      [codexProfiles(), "codex_profiles", undefined],
      [createCodexProfile("Work", false), "create_codex_profile", { name: "Work", importCurrent: false }],
      [renameCodexProfile("profile-id", "Personal"), "rename_codex_profile", { id: "profile-id", name: "Personal" }],
      [removeCodexProfile("profile-id"), "remove_codex_profile", { id: "profile-id" }],
      [activateCodexProfile("profile-id"), "activate_codex_profile", { id: "profile-id" }],
      [importCodexProfile("Work", "~/Downloads/auth.json"), "import_codex_profile", { name: "Work", path: "~/Downloads/auth.json", replaceId: null }],
      [importCodexProfile("Work", "/tmp/auth.json", "profile-id"), "import_codex_profile", { name: "Work", path: "/tmp/auth.json", replaceId: "profile-id" }],
      [startCodexProfileLogin("profile-id"), "start_codex_profile_login", { id: "profile-id" }],
      [codexProfileLoginStatus("profile-id"), "codex_profile_login_status", { id: "profile-id" }],
      [cancelCodexProfileLogin("profile-id"), "cancel_codex_profile_login", { id: "profile-id" }],
      [scheduleCodexRestart("profile-id"), "schedule_codex_restart", { id: "profile-id" }],
      [scheduleCodexRestart("limited-profile", true), "schedule_codex_restart", { id: "limited-profile", allowLimited: true }],
      [codexRestartStatus(), "codex_restart_status", undefined],
      [cancelCodexRestart(), "cancel_codex_restart", undefined],
      [runCodexRestartNow(), "run_codex_restart_now", undefined],
      [codexResetStatus(), "codex_reset_status", undefined],
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
