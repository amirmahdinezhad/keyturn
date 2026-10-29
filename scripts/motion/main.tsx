import React from "react";
import { createRoot } from "react-dom/client";
import { DEFAULT_APP_SETTINGS, DEFAULT_WORKSPACE_USAGE, type AppState, type CodexProfile, type ClaudeProfile, type CodexRestartStatus } from "../../src/types";
import { applyDisplayPrefs, DEFAULT_DISPLAY_PREFS } from "../../src/displayPrefs";
import { installMotionProbe } from "./probe";
import { setFixtureHandler } from "./bridge";
import "../../src/index.css";
import "../../src/panel.css";

// Only this explicit QA entry intercepts IPC. No real account commands escape it.
const native = (window as any).__TAURI_INTERNALS__;
const nativeVersion = native ? "optimized-qa" : null;
let state: AppState;
let profiles: CodexProfile[];
let claude: ClaudeProfile[];
let cliReads = 0;
let restart: CodexRestartStatus | null = null;
let scenario = "main";
let epoch = 0;
const reset = (next: string) => {
  scenario = next;
  cliReads = 0;
  restart = null;
  profiles = [
    { id: "personal", name: "Personal", email: "alex@example.com", plan: "plus", active: true },
    { id: "studio", name: "Studio", email: "alex@studio.example", plan: "pro", active: false },
    { id: "weekend", name: "Weekend projects", email: "alex+projects@example.com", plan: "plus", active: false },
  ].map(p => ({ ...p, connected: true, needsActivation: false, connectionError: null, loginCommand: "codex login" }));
  state = { settings: { openAtLogin: false }, app: structuredClone({ ...DEFAULT_APP_SETTINGS, workspaceVersion: 1, activeWorkspaceId: next === "setup" ? null : "personal", workspaces: next === "setup" ? [] : [
    { id: "personal", name: "Personal", accent: "#007aff", accountIds: next === "empty" ? [] : profiles.map(p => p.id), usage: { ...DEFAULT_WORKSPACE_USAGE } },
    { id: "team", name: "Team", accent: "#2f9e6e", accountIds: [], usage: { ...DEFAULT_WORKSPACE_USAGE } },
  ] }) };
  claude = [{ id: "claude-personal", name: "Personal", email: "alex@example.com", configDir: "/tmp/keyturn-motion/claude", createdAt: "2026-10-01T10:00:00Z", active: true, connected: true, subscription: { id: "claude", service: "Claude", plan: "max" }, loginCommand: "claude auth login", launchCommand: "claude", quota: null }];
};
reset("main");
const wait = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const snapshot = () => structuredClone(state);
const profileState = () => ({ profiles: structuredClone(profiles), activeProfileId: profiles.find(p => p.active)?.id ?? null, canImport: true, switchError: null });
const quota = () => ({ provider: "codex", asOf: new Date().toISOString(), windows: [
  { window: "5-hour", percent: scenario === "limited" ? 100 : 32, resetsAt: new Date(Date.now() + 7_200_000).toISOString() },
  { window: "weekly", percent: 61, resetsAt: new Date(Date.now() + 259_200_000).toISOString() },
] });
const addProfile = (name: string) => {
  const p: CodexProfile = { ...profiles[0], id: `qa-${profiles.length}`, name, email: `${name.toLowerCase().replace(/\W/g, "")}@example.com`, active: false };
  profiles.push(p);
  return p;
};
setFixtureHandler(async (cmd: string, args: any = {}) => {
  if (cmd === "get_state") {
    if (scenario === "error") throw new Error("Saved settings are temporarily unavailable. Try again.");
    await wait(scenario === "loading" ? 5000 : 80);
    return snapshot();
  }
  if (cmd === "cli_status") {
    cliReads++;
    return [{ id: "claude", label: "Claude Code", binary: "claude", binaryFound: true, loggedIn: true, email: "alex@example.com", authMethod: "claude.ai", loginCmd: "claude auth login", installUrl: "https://claude.com", subscriptions: [{ id: "claude", service: "Claude", plan: "max" }] },
      { id: "gemini", label: "Gemini", binary: "gemini", binaryFound: cliReads > 1, loggedIn: cliReads > 2, loginCmd: "gemini", installUrl: "https://geminicli.com" }];
  }
  if (cmd === "codex_profiles") return profileState();
  if (cmd === "codex_profile_quota" || cmd === "codex_account_quota") {
    await wait(120);
    if (scenario === "quota-error") throw new Error("Usage is temporarily unavailable. Try refreshing.");
    return quota();
  }
  if (cmd === "cli_quotas") { await wait(120); return [{ ...quota(), provider: "claude" }]; }
  if (cmd === "codex_reset_status") return { latestReset: null, scheduledReset: null, activeWatch: null, generatedAt: new Date().toISOString() };
  if (cmd === "set_app_settings") { await wait(400); state.app = { ...state.app, ...args.settings }; return snapshot(); }
  if (cmd === "set_settings") { state.settings.openAtLogin = args.openAtLogin; return snapshot(); }
  if (cmd === "select_workspace") { state.app.activeWorkspaceId = args.id; return snapshot(); }
  if (cmd === "update_workspace") { await wait(400); state.app.workspaces = state.app.workspaces.map(w => w.id === args.id ? { ...w, name: args.name, accent: args.accent, usage: args.usage } : w); return snapshot(); }
  if (cmd === "create_workspace" || cmd === "import_workspace_account") {
    await wait(600);
    if (args.credentials && !JSON.parse(args.credentials).fixture) throw new Error("Use synthetic JSON: {\"fixture\":true}");
    const p = addProfile(args.accountName);
    if (cmd === "create_workspace") {
      const id = `workspace-${state.app.workspaces.length}`;
      state.app.workspaces.push({ id, name: args.name, accent: args.accent, accountIds: [p.id], usage: { ...DEFAULT_WORKSPACE_USAGE } }); state.app.activeWorkspaceId = id;
    } else state.app.workspaces.find(w => w.id === args.workspaceId)!.accountIds.push(p.id);
    return snapshot();
  }
  if (cmd === "rename_codex_profile") { await wait(400); profiles.find(p => p.id === args.id)!.name = args.name; return profileState(); }
  if (cmd === "remove_codex_profile") { await wait(400); profiles = profiles.filter(p => p.id !== args.id); state.app.workspaces.forEach(w => { w.accountIds = w.accountIds.filter(id => id !== args.id); }); return profileState(); }
  if (cmd === "move_workspace_account") { state.app.workspaces.forEach(w => { w.accountIds = w.accountIds.filter(id => id !== args.id); if (w.id === args.targetId) w.accountIds.push(args.id); }); return snapshot(); }
  if (cmd === "set_codex_account_preferences") { await wait(400); state.app.codexAccounts[args.id] = args.preferences; return snapshot(); }
  if (cmd === "claude_profiles") { await wait(200); return structuredClone(claude); }
  if (cmd === "create_claude_profile") { await wait(400); claude.push({ ...claude[0], id: `claude-${claude.length}`, name: args.name || "New profile", active: false }); return structuredClone(claude); }
  if (cmd === "edit_claude_profile") { await wait(400); if (args.name === null) claude = claude.filter(p => p.id !== args.id); else claude.find(p => p.id === args.id)!.name = args.name; return structuredClone(claude); }
  if (cmd === "rescan_claude_profile") { await wait(400); return structuredClone(claude); }
  if (cmd === "codex_restart_status") return restart;
  if (cmd === "schedule_codex_restart") { restart = { profileId: args.id, profileName: profiles.find(p => p.id === args.id)!.name, deadlineAt: new Date(Date.now() + 30_000).toISOString(), phase: "countdown", error: null, processId: null, credentialsVerified: false }; return restart; }
  if (cmd === "cancel_codex_restart") { restart = restart ? { ...restart, phase: "cancelled" } : null; return restart; }
  if (cmd === "run_codex_restart_now") { profiles.forEach(p => { p.active = p.id === restart?.profileId; }); restart = restart ? { ...restart, phase: "verified", credentialsVerified: true } : null; return restart; }
  if (cmd === "plugin:app|version") return nativeVersion ?? "0.1.0";
  if (cmd.includes("updater")) return null;
  if (cmd.startsWith("plugin:event|") || cmd === "set_menu_bar_accent" || cmd === "quit_app") return null;
  throw new Error(`QA fixture has no handler for ${cmd}`);
});

localStorage.removeItem("keyturn.displayPrefs");
applyDisplayPrefs(DEFAULT_DISPLAY_PREFS);
installMotionProbe({ native: !!native, nativeVersion, production: import.meta.env.PROD, fixture: true, scenario: () => scenario });
const { default: App } = await import("../../src/App");
const root = createRoot(document.getElementById("root")!);
const render = () => root.render(<App key={epoch++} />);
// Fixture-only keyboard controls: Ctrl+Alt+1..6 = main/setup/empty/error/loading/limited.
window.addEventListener("keydown", event => {
  if (!event.ctrlKey || !event.altKey) return;
  const next = ["main", "setup", "empty", "error", "loading", "limited"][Number(event.code.replace("Digit", "")) - 1];
  if (next) { event.preventDefault(); reset(next); render(); }
});
render();
