import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const api = vi.hoisted(() => ({
  beginAccountSetup: vi.fn(), discardAccountSetup: vi.fn(), loadSetupAccounts: vi.fn(),
  startSetupLogin: vi.fn(), pollSetupLogin: vi.fn(), cancelSetupLogin: vi.fn(), submitSetupCode: vi.fn(),
  importSetupAccount: vi.fn(), connectCurrentGoogle: vi.fn(), connectSetupConfig: vi.fn(), commitAccountSetup: vi.fn(), removeWorkspaceAccount: vi.fn(), antigravityAccounts: vi.fn(), antigravityAccountQuota: vi.fn(),
  includeWorkspaceProvider: vi.fn(), getState: vi.fn(), setSettings: vi.fn(), setAppSettings: vi.fn(), setMenuBarAccent: vi.fn(),
  codexProfileQuota: vi.fn(), setCodexAccountPreferences: vi.fn(), cliStatus: vi.fn(), codexProfiles: vi.fn(), claudeProfiles: vi.fn(),
  discoverWorkspaceAccounts: vi.fn(), createDiscoveredWorkspace: vi.fn(), createWorkspace: vi.fn(), importWorkspaceAccount: vi.fn(), selectWorkspace: vi.fn(), updateWorkspace: vi.fn(), moveWorkspaceAccount: vi.fn(),
  editClaudeProfile: vi.fn(), renameAntigravityAccount: vi.fn(), renameCodexProfile: vi.fn(), removeCodexProfile: vi.fn(), activateCodexProfile: vi.fn(),
  importCodexProfile: vi.fn(), importCodexProfileJson: vi.fn(),
  scheduleCodexRestart: vi.fn(), codexRestartStatus: vi.fn(), cancelCodexRestart: vi.fn(), runCodexRestartNow: vi.fn(),
  cliQuotas: vi.fn(), codexResetStatus: vi.fn(), quitApp: vi.fn(),
  prettyError: vi.fn((e: unknown) => e instanceof Error ? e.message : String(e)),
}));
vi.mock("./api", () => api);
const nativeEvents = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => nativeEvents);
import App from "./App";
import type { SetupAccount } from "./api";
const opener = vi.hoisted(() => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@tauri-apps/plugin-opener", () => opener);
import { makeState } from "./test/fixtures";
import { DEFAULT_CODEX_ACCOUNT_PREFERENCES, DEFAULT_WORKSPACE_USAGE, type AppState, type CodexProfile, type Workspace } from "./types";

const personal: CodexProfile = { id: "personal", name: "Personal account", email: "personal@example.com", plan: "plus", connected: true, active: true, needsActivation: false, connectionError: null, loginCommand: "" };
const work: CodexProfile = { ...personal, id: "work", name: "Work account", email: "work@example.com", active: false };
const studio: Workspace = { id: "studio", name: "Studio", accent: "#7c6cf0", accountIds: [personal.id, work.id], usage: { ...DEFAULT_WORKSPACE_USAGE } };
function savedState(workspaces = [studio], activeWorkspaceId = workspaces[0]?.id ?? null): AppState {
  return makeState({ app: { ...makeState().app, workspaceVersion: 1, workspaces, activeWorkspaceId } });
}
const claude = { id: "claude", label: "Claude", binary: "claude", binaryFound: true, loggedIn: true, authMethod: "claude.ai", email: "claude@example.com", loginCmd: "claude auth login", installUrl: "https://claude.com" };
const stagedClaude: SetupAccount = { id: "staged-claude", provider: "claude", name: "Claude", email: "claude@example.com", subscription: { id: "claude", service: "Claude", plan: "pro" } };
const stagedCodex: SetupAccount = { id: "staged-codex", provider: "codex", name: "Backup", email: "backup@example.com", subscription: { id: "chatgpt", service: "ChatGPT", plan: "plus" } };
beforeEach(() => {
  vi.resetAllMocks(); localStorage.clear();
  nativeEvents.listen.mockResolvedValue(vi.fn());
  api.beginAccountSetup.mockResolvedValue("draft"); api.discardAccountSetup.mockResolvedValue(undefined);
  api.loadSetupAccounts.mockResolvedValue([stagedClaude]); api.commitAccountSetup.mockResolvedValue(savedState());
  api.importSetupAccount.mockResolvedValue(stagedCodex); api.connectSetupConfig.mockResolvedValue(stagedClaude);
  api.cancelSetupLogin.mockResolvedValue(undefined); api.submitSetupCode.mockResolvedValue(undefined); opener.openUrl.mockResolvedValue(undefined);
  api.startSetupLogin.mockResolvedValue({ attemptId: "attempt", status: "pending", authUrl: "https://auth.openai.com/authorize", userCode: null, error: null, account: null });
  api.pollSetupLogin.mockResolvedValue({ attemptId: "attempt", status: "completed", authUrl: null, userCode: null, error: null, account: stagedCodex });
  api.prettyError.mockImplementation((e) => e instanceof Error ? e.message : String(e));
  api.getState.mockResolvedValue(savedState()); api.setMenuBarAccent.mockResolvedValue(undefined);
  api.cliStatus.mockResolvedValue([claude]);
  api.discoverWorkspaceAccounts.mockResolvedValue([{ ...claude, subscriptions: [{ id: "claude", service: "Claude", plan: "pro" }] }]);
  api.claudeProfiles.mockResolvedValue([]);
  api.antigravityAccounts.mockResolvedValue([]);
  api.codexProfiles.mockResolvedValue({ profiles: [personal, work], activeProfileId: personal.id, canImport: true, switchError: null });
  api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "5-hour", percent: 20 }] });
  api.codexRestartStatus.mockResolvedValue(null);
  api.codexResetStatus.mockResolvedValue({ latestReset: null, scheduledReset: null, activeWatch: null, generatedAt: new Date().toISOString() });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

async function uploadAuth(user: ReturnType<typeof userEvent.setup>, credentials: string) {
  await user.upload(screen.getByLabelText("Choose auth.json file"), new File([credentials], "auth.json", { type: "application/json" }));
  await screen.findByText("Ready to import");
}

async function identity(user: ReturnType<typeof userEvent.setup>, name = "Lab") {
  await user.type(await screen.findByLabelText("Workspace name"), name);
  await user.click(screen.getByRole("button", { name: "Next" }));
}
async function fileMethod(user: ReturnType<typeof userEvent.setup>) {
  if (screen.queryByLabelText("Step 2 of 3")) await user.click(screen.getByRole("button", { name: "Next" }));
  if (!screen.queryByRole("group", { name: "Choose provider" })) await user.click(screen.getByRole("button", { name: "Add account" }));
  await user.click(screen.getByRole("button", { name: "ChatGPT / Codex" }));
  await user.click(screen.getByRole("button", { name: /Import auth.json/ }));
}

describe("workspace flow", () => {
  it("shows an active Codex account and an active Claude account at the same time", async () => {
    const loadedClaude = { id: "claude-loaded", name: "Claude current", configDir: "~/.claude", createdAt: "", email: "claude@example.com", connected: true, active: true, subscription: { id: "claude", service: "Claude", plan: "pro" }, loginCommand: "", launchCommand: "", quota: null };
    api.claudeProfiles.mockResolvedValue([loadedClaude]);
    api.getState.mockResolvedValue(savedState([{ ...studio, claudeAccountIds: [loadedClaude.id] }]));
    render(<App />);
    expect(within(await screen.findByRole("article", { name: "Personal account account" })).getByText("Active")).toBeInTheDocument();
    expect(within(await screen.findByRole("article", { name: "Claude current Claude account" })).getByText("Active")).toBeInTheDocument();
  });

  it("marks the loaded current account independently for each provider", async () => {
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([]));
    api.loadSetupAccounts.mockResolvedValue([{ ...stagedCodex, active: true }, { ...stagedClaude, active: true }]);
    render(<App />); await identity(user);
    const list = await screen.findByRole("group", { name: "Available accounts" });
    expect(within(list).getAllByText("Active")).toHaveLength(2);
  });

  it("shows an imported account's profile name separately from its saved label after reload", async () => {
    api.codexProfiles.mockResolvedValue({ profiles: [{ ...personal, name: "Work backup", displayName: "Emma Brooks", email: "work@example.com" }], activeProfileId: personal.id, canImport: true, switchError: null });
    render(<App />);
    const card = await screen.findByRole("article", { name: "Work backup account" });
    expect(within(card).getByText("Emma Brooks")).toBeInTheDocument();
    expect(within(card).getByText("work@example.com")).toBeInTheDocument();
    expect(within(card).getByText("Work backup")).toBeInTheDocument();
  });

  it("shows the Codex profile name with its email when loading available accounts", async () => {
    const user = userEvent.setup();
    api.getState.mockResolvedValue(savedState([]));
    api.loadSetupAccounts.mockResolvedValue([{ ...stagedCodex, name: "ChatGPT account", displayName: "Emma Brooks" }]);
    render(<App />); await identity(user);
    expect(await screen.findByText("Emma Brooks")).toBeInTheDocument();
    expect(screen.getByText(stagedCodex.email!)).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Add Emma Brooks to workspace" })).toBeInTheDocument();
    expect(screen.queryByText("ChatGPT account")).not.toBeInTheDocument();
  });

  it("automatically loads accounts and lets the optional third step use only existing selections", async () => {
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([])); render(<App />);
    await identity(user, "Studio");
    expect(api.loadSetupAccounts).toHaveBeenCalledTimes(1); expect(api.cliStatus).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Load accounts" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add new account" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Load again" }).textContent).toBe("");
    expect(screen.queryByRole("button", { name: /Import auth.json/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Settings" })).not.toBeInTheDocument();

    expect(await screen.findByText("claude@example.com")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Create workspace" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("checkbox", { name: "Add Claude to workspace" }));
    await user.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByText(/Optional: add another account/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Create workspace" }));
    expect(api.commitAccountSetup).toHaveBeenCalledWith("draft", "Studio", "#007aff", [stagedClaude.id], null);
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });
  it("shows real spinners while automatically loading and refreshing accounts", async () => {
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([]));
    let finish!: (accounts: SetupAccount[]) => void;
    api.loadSetupAccounts.mockImplementation(() => new Promise<SetupAccount[]>((resolve) => { finish = resolve; }));
    render(<App />); await identity(user);
    expect(screen.getByRole("status")).toHaveTextContent("Checking your accounts");
    expect(screen.getByRole("status").querySelector(".ak-spin")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Load again" }).querySelector(".ak-spin")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Load again" })).toBeDisabled();
    await act(async () => finish([stagedClaude]));
    await user.click(screen.getByRole("button", { name: "Load again" }));
    expect(screen.getByRole("button", { name: "Load again" })).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("button", { name: "Load again" }).querySelector(".ak-spin")).toBeInTheDocument();
    await act(async () => finish([stagedClaude]));
    expect(screen.getByRole("button", { name: "Load again" })).toHaveAttribute("aria-busy", "false");
    expect(screen.queryByText(/Saved privately/)).not.toBeInTheDocument();
  });
  it("offers step three even when no existing paid accounts are found", async () => {
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([])); api.loadSetupAccounts.mockResolvedValue([]); render(<App />);
    await identity(user);
    expect(await screen.findByText("No paid accounts found")).toBeInTheDocument();
    await fileMethod(user);
    expect(screen.getByLabelText("Choose auth.json file")).toBeInTheDocument();
    expect(screen.getByLabelText("Step 3 of 3")).toBeInTheDocument();
  });
  it("preserves existing selection across back, and removes unavailable accounts on reload", async () => {
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([])); render(<App />); await identity(user);

    await user.click(await screen.findByRole("checkbox", { name: "Add Claude to workspace" }));
    await user.click(screen.getByRole("button", { name: "Back" })); await user.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("checkbox", { name: "Add Claude to workspace" })).toBeChecked();
    expect(api.loadSetupAccounts).toHaveBeenCalledTimes(1);
    api.loadSetupAccounts.mockResolvedValue([{...stagedClaude,id:"changed",email:"another@example.com"}]);
    await user.click(screen.getByRole("button", { name: "Load again" })); await screen.findByText("another@example.com");
    expect(screen.getByRole("checkbox", { name: "Add Claude to workspace" })).not.toBeChecked();
  });
  it("keeps discovery errors inline and the new-account route reachable", async () => {
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([])); api.loadSetupAccounts.mockRejectedValue(new Error("Cannot read CLI accounts"));
    render(<App />); await identity(user);
    expect(await screen.findByRole("alert")).toHaveTextContent("Cannot read CLI accounts"); await fileMethod(user);
    expect(screen.getByLabelText("Choose auth.json file")).toBeInTheDocument();
  });
  it("keeps a workspace draft until the user explicitly discards it", async () => {
    const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button", { name: "Create workspace" }));
    await user.type(screen.getByLabelText("Workspace name"), "Unsaved"); await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.getByRole("alertdialog", { name: "Discard changes?" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Keep editing" })); expect(screen.getByLabelText("Workspace name")).toHaveValue("Unsaved");
    await user.keyboard("{Escape}"); await user.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(api.commitAccountSetup).not.toHaveBeenCalled();
  });
  it("keeps file enrollment in a modal with workspace save actions unavailable", async () => {
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([])); render(<App />); await identity(user); await fileMethod(user);
    await uploadAuth(user, '{"fixture":true}');
    expect(screen.getByRole("dialog", { name: "Add account" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Create workspace" })).not.toBeInTheDocument();
    expect(screen.queryByText("Enter a file path instead")).not.toBeInTheDocument();
    expect(screen.queryByText("claude@example.com")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(screen.getByRole("button", { name: "Keep editing" }));
    expect(screen.queryByLabelText("Account name")).not.toBeInTheDocument();
    expect(screen.getByText("Ready to import")).toBeInTheDocument();
    expect(api.importSetupAccount).not.toHaveBeenCalled();
  });
  it("verifies a file before allowing final workspace commit and never activates it", async () => {
    api.importSetupAccount.mockResolvedValue({ ...stagedCodex, name: "Emma Brooks", displayName: "Emma Brooks" });
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([])); api.commitAccountSetup.mockImplementation(async () => {api.getState.mockResolvedValue(savedState());return savedState();});
    render(<App />); await user.type(await screen.findByLabelText("Workspace name"), "Studio"); await user.click(screen.getByRole("button", { name: "Accent Orchid" }));
    await user.click(screen.getByRole("button", { name: "Next" })); await fileMethod(user); expect(screen.queryByLabelText("Account name")).not.toBeInTheDocument(); await uploadAuth(user, '{"fixture":true}');
    expect(screen.queryByRole("button", { name: "Create workspace" })).not.toBeInTheDocument(); await user.click(screen.getByRole("button", { name: "Verify account" }));
    expect(api.importSetupAccount).toHaveBeenCalledWith("draft", "codex", "", {credentials:'{"fixture":true}'});
    await screen.findByText("Account added");
    expect(screen.queryByRole("dialog", { name: "Add account" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Done" })).not.toBeInTheDocument();
    expect(screen.queryByText("Account verified and ready to add")).not.toBeInTheDocument();
    expect(within(screen.getByRole("group", { name: "Added accounts" })).getByText("backup@example.com")).toBeInTheDocument();
    expect(within(screen.getByRole("group", { name: "Added accounts" })).getByText("Emma Brooks")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Include Emma Brooks" })).toBeChecked();
    expect(screen.queryByText("claude@example.com")).not.toBeInTheDocument();
    expect(api.commitAccountSetup).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Create workspace" }));
    expect(api.commitAccountSetup).toHaveBeenCalledWith("draft", "Studio", "#7c6cf0", [stagedCodex.id], null);
    expect(await screen.findByRole("article", {name:"Personal account account"})).toBeInTheDocument(); expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });
  it("rejects failed or unknown paid-plan verification without committing a workspace", async () => {
    const user = userEvent.setup(); api.getState.mockResolvedValue(savedState([])); api.importSetupAccount.mockRejectedValue(new Error("A paid plan could not be confirmed"));
    render(<App />); await identity(user); await fileMethod(user); await uploadAuth(user,"{}");
    await user.click(screen.getByRole("button", {name:"Verify account"})); expect(await screen.findByRole("alert")).toHaveTextContent("paid plan");
    expect(screen.queryByRole("button", {name:"Create workspace"})).not.toBeInTheDocument(); expect(api.commitAccountSetup).not.toHaveBeenCalled();
  });
  it("adds a file to an existing workspace through the same staged flow", async () => {
    const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button", {name:"Add account"}));
    expect(screen.getByRole("group", {name:"Choose provider"})).toBeInTheDocument();
    expect(screen.getAllByRole("dialog", {hidden:true})).toHaveLength(1);
    expect(screen.queryByRole("button", {name:"Add accounts"})).not.toBeInTheDocument();
    await fileMethod(user);
    await uploadAuth(user,'{"fixture":true}'); await user.click(screen.getByRole("button",{name:"Verify account"}));
    await screen.findByText("Account added"); await user.click(screen.getByRole("button",{name:"Add accounts"}));
    expect(api.commitAccountSetup).toHaveBeenCalledWith("draft","Studio","#7c6cf0",[stagedCodex.id],"studio");
  });
  it("supports browser sign-in and only adds the verified resulting account", async () => {
    const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button",{name:"Add account"}));
    await user.click(screen.getByRole("button",{name:"ChatGPT / Codex"})); await user.click(screen.getByRole("button",{name:/Sign in with ChatGPT/}));
    expect(screen.queryByLabelText("Account name")).not.toBeInTheDocument(); expect(screen.queryByRole("button",{name:"Continue in browser"})).not.toBeInTheDocument();
    expect(await screen.findByText(/Finish signing in with/)).toBeInTheDocument(); expect(api.commitAccountSetup).not.toHaveBeenCalled();
    expect(opener.openUrl).toHaveBeenCalledWith("https://auth.openai.com/authorize"); await screen.findByText("Account added");
    expect(api.startSetupLogin).toHaveBeenCalledWith("draft","codex","browser","");
  });
  it("cancels a pending device login and discards its draft on confirmed close", async () => {
    const user = userEvent.setup(); api.startSetupLogin.mockResolvedValue({attemptId:"device",status:"pending",authUrl:"https://auth.openai.com/codex/device",userCode:"ABCD-EFGH",error:null,account:null}); api.pollSetupLogin.mockReturnValue(new Promise(()=>{}));
    render(<App />); await user.click(await screen.findByRole("button",{name:"Add account"}));
    await user.click(screen.getByRole("button",{name:"ChatGPT / Codex"})); await user.click(screen.getByRole("button",{name:/Use a device code/}));
    expect(screen.queryByLabelText("Account name")).not.toBeInTheDocument(); expect(screen.queryByRole("button",{name:"Get device code"})).not.toBeInTheDocument(); expect(api.startSetupLogin).toHaveBeenCalledWith("draft","codex","device","");
    expect(await screen.findByText("ABCD-EFGH")).toBeInTheDocument(); await user.click(screen.getByRole("button",{name:"Cancel sign-in"})); expect(api.cancelSetupLogin).toHaveBeenCalledWith("draft");
    await user.click(screen.getByRole("button",{name:"Close"})); await waitFor(()=>expect(api.discardAccountSetup).toHaveBeenCalledWith("draft"));
  });
  it("returns failed direct starts to the method picker and retries only on another choice", async () => {
    const user = userEvent.setup();
    api.startSetupLogin.mockRejectedValueOnce(new Error("CLI unavailable"));
    api.pollSetupLogin.mockReturnValue(new Promise(() => {}));
    render(<App />); await user.click(await screen.findByRole("button", { name: "Add account" }));
    await user.click(screen.getByRole("button", { name: "ChatGPT / Codex" }));
    await user.click(screen.getByRole("button", { name: "Sign in with ChatGPT" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("CLI unavailable");
    expect(screen.getByRole("group", { name: "Choose sign-in method" })).toBeInTheDocument();
    expect(api.startSetupLogin).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "Sign in with ChatGPT" }));
    expect(await screen.findByText(/Finish signing in with/)).toBeInTheDocument();
    expect(api.startSetupLogin).toHaveBeenCalledTimes(2);
    await user.click(screen.getByRole("button", { name: "Cancel sign-in" }));
    expect(screen.getByRole("group", { name: "Choose sign-in method" })).toBeInTheDocument();
  });
  it("blocks repeated direct starts while the first request is in flight", async () => {
    const user = userEvent.setup(); let finish!: (login: unknown) => void;
    api.startSetupLogin.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
    api.pollSetupLogin.mockReturnValue(new Promise(() => {}));
    render(<App />); await user.click(await screen.findByRole("button", { name: "Add account" }));
    await user.click(screen.getByRole("button", { name: "ChatGPT / Codex" }));
    await user.dblClick(screen.getByRole("button", { name: "Use a device code" }));
    expect(api.startSetupLogin).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Starting sign-in…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
    await act(async () => finish({ attemptId: "direct", status: "pending", authUrl: null, userCode: "ABCD", error: null, account: null }));
    expect(await screen.findByText("ABCD")).toBeInTheDocument();
  });
  it("provides official Claude login and config-folder choices rather than a Codex file format", async () => {
    const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button",{name:"Add account"}));
    await user.click(screen.getByRole("button",{name:"Claude"}));
    expect(screen.getByRole("button",{name:/Sign in with Claude Code/})).toBeInTheDocument(); expect(screen.queryByRole("button",{name:/Import auth.json/})).not.toBeInTheDocument();
    await user.click(screen.getByRole("button",{name:/Connect a configuration/})); await user.type(screen.getByLabelText("Account name"),"Work Claude"); await user.type(screen.getByLabelText("Claude configuration folder"),"~/.claude-work");
    await user.click(screen.getByRole("button",{name:"Verify account"})); expect(api.connectSetupConfig).toHaveBeenCalledWith("draft","Work Claude","~/.claude-work");
  });
  it("labels Antigravity experimental and offers its own JSON import", async () => {
    const user = userEvent.setup(); render(<App />); await user.click(await screen.findByRole("button",{name:"Add account"}));
    await user.click(screen.getByRole("button",{name:"Antigravity"}));
    expect(screen.getByRole("button",{name:/Sign in with Google/})).toBeInTheDocument(); await user.click(screen.getByRole("button",{name:/Import credentials JSON/}));
    expect(screen.getByLabelText("Choose credentials.json file")).toBeInTheDocument(); expect(screen.getByText(/Experimental integration/)).toBeInTheDocument();
  });
  it("saves selected existing and new accounts together in one final commit", async () => {
    const user=userEvent.setup();api.getState.mockResolvedValue(savedState([]));render(<App />);await identity(user);
    await user.click(await screen.findByRole("checkbox",{name:"Add Claude to workspace"}));
    await fileMethod(user);await uploadAuth(user,'{"fixture":true}');await user.click(screen.getByRole("button",{name:"Verify account"}));
    await screen.findByText("Account added");expect(api.commitAccountSetup).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button",{name:"Create workspace"}));expect(api.commitAccountSetup).toHaveBeenCalledWith("draft","Lab","#007aff",[stagedClaude.id,stagedCodex.id],null);
  });
  it("keeps the draft open after commit failure and retries without repeating login", async () => {
    const user=userEvent.setup();api.getState.mockResolvedValue(savedState([]));api.commitAccountSetup.mockRejectedValueOnce(new Error("Cannot save settings"));render(<App />);await identity(user);
    await user.click(await screen.findByRole("checkbox",{name:"Add Claude to workspace"}));await user.click(screen.getByRole("button",{name:"Next"}));await user.click(screen.getByRole("button",{name:"Create workspace"}));
    expect(await screen.findByRole("alert")).toHaveTextContent("Cannot save settings");expect(screen.getByText(/Optional: add another account/)).toBeInTheDocument();
    await user.click(screen.getByRole("button",{name:"Create workspace"}));expect(api.commitAccountSetup).toHaveBeenCalledTimes(2);
  });
  it("renders saved accounts and hides unselected providers", async () => {
    api.getState.mockResolvedValue(savedState([{...studio,providerIds:[]}])); render(<App />); await screen.findByRole("article",{name:"Personal account account"}); expect(screen.queryByRole("region",{name:"Claude accounts"})).not.toBeInTheDocument();
  });
  it("switches workspaces without changing the active CLI login", async () => {
    const user=userEvent.setup(); const first={...studio,accountIds:[personal.id]}; const second={...studio,id:"second",name:"Second",accountIds:[work.id]}; api.getState.mockResolvedValue(savedState([first,second])); api.selectWorkspace.mockResolvedValue(savedState([first,second],second.id));
    render(<App />); await screen.findByRole("article",{name:"Personal account account"}); await user.click(screen.getByRole("combobox",{name:"Workspace"})); await user.click(screen.getByRole("option",{name:"Second"}));
    expect(await screen.findByRole("article",{name:"Work account account"})).toBeInTheDocument(); expect(screen.queryByRole("article",{name:"Personal account account"})).not.toBeInTheDocument(); expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });
});

describe("limit countdown and quick switching", () => {
  it("moves Active immediately on native success before the profile refresh returns", async () => {
    let switched!: (event: { payload: string }) => void;
    nativeEvents.listen.mockImplementation(async (name, callback) => { if (name === "codex-profile-switched") switched = callback; return vi.fn(); });
    render(<App />);
    const oldCard = await screen.findByRole("article", { name: "Personal account account" });
    expect(within(oldCard).getByText("Active")).toBeInTheDocument();
    api.codexProfiles.mockImplementation(() => new Promise(() => {}));
    await act(async () => switched({ payload: work.id }));
    expect(within(screen.getByRole("article", { name: "Work account account" })).getByText("Active")).toBeInTheDocument();
    expect(within(oldCard).queryByText("Active")).not.toBeInTheDocument();
  });

  it("selects the switched account for a menu bar using the same provider before refresh finishes", async () => {
    let switched!: (event: { payload: string }) => void;
    nativeEvents.listen.mockImplementation(async (name, callback) => { if (name === "codex-profile-switched") switched = callback; return vi.fn(); });
    api.getState.mockResolvedValue(savedState([{ ...studio, usage: { ...studio.usage, menuBarProvider: "codex", menuBarAccountId: personal.id } }]));
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("article", { name: "Work account account" });
    api.getState.mockImplementation(() => new Promise(() => {}));
    await act(async () => switched({ payload: work.id }));
    await user.click(screen.getByRole("button", { name: "Workspace settings" }));
    await user.click(screen.getByRole("tab", { name: "Menu bar" }));
    expect(screen.getByLabelText("Menu bar active account")).toHaveTextContent(work.name);
  });

  const fixed = Date.parse("2026-10-17T10:00:00Z");
  function exhausted() {
    vi.spyOn(Date, "now").mockReturnValue(fixed);
    api.codexProfileQuota.mockImplementation(async (id) => ({ provider: "codex", windows: id === personal.id ? [
      { window: "5-hour", percent: 100, resetsAt: "2026-10-17T11:00:00Z" },
      { window: "weekly", percent: 100, resetsAt: "2026-10-18T14:15:00Z" },
    ] : [{ window: "5-hour", percent: 20, resetsAt: "2026-10-17T14:00:00Z" }] }));
  }
  it("warns before switching to a freshly limited account and never schedules a restart", async () => {
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("article", { name: "Work account account" });
    api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "weekly", percent: 100, resetsAt: new Date(Date.now() + 3_600_000).toISOString() }] });
    await user.click(screen.getByRole("button", { name: "Activate Work account" }));
    const warning = await screen.findByRole("dialog", { name: "Account limit reached" });
    expect(warning).toHaveTextContent("Work account");
    expect(warning).toHaveTextContent("This account has no remaining capacity");
    expect(warning).toHaveTextContent("Resets in");
    expect(api.scheduleCodexRestart).not.toHaveBeenCalled();
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
    await user.click(within(warning).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog", { name: "Account limit reached" })).not.toBeInTheDocument();
  });
  it("switches anyway only after confirmation and keeps scheduling errors in the warning", async () => {
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("article", { name: "Work account account" });
    api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "weekly", percent: 100 }] });
    const status = { profileId: work.id, profileName: work.name, deadlineAt: new Date(Date.now()).toISOString(), phase: "quitting", error: null, processId: null, credentialsVerified: false };
    api.scheduleCodexRestart.mockRejectedValueOnce(new Error("Codex could not be started. Try again.")).mockResolvedValueOnce(status);
    await user.click(screen.getByRole("button", { name: "Activate Work account" }));
    const warning = await screen.findByRole("dialog", { name: "Account limit reached" });
    expect(warning).toHaveTextContent("Reset time unavailable");
    expect(api.scheduleCodexRestart).not.toHaveBeenCalled();
    await user.click(within(warning).getByRole("button", { name: "Switch anyway" }));
    expect(await within(warning).findByRole("alert")).toHaveTextContent("Codex could not be started");
    expect(api.scheduleCodexRestart).toHaveBeenLastCalledWith(work.id, true);
    await user.click(within(warning).getByRole("button", { name: "Switch anyway" }));
    expect(await screen.findByLabelText("Account switch")).toHaveTextContent("Switching to Work account…");
    expect(screen.queryByRole("dialog", { name: "Account limit reached" })).not.toBeInTheDocument();
    expect(api.scheduleCodexRestart).toHaveBeenCalledTimes(2);
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });
  it("disables repeated confirmation while an override is being scheduled", async () => {
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("article", { name: "Work account account" });
    api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "weekly", percent: 100 }] });
    let finish!: (value: unknown) => void;
    api.scheduleCodexRestart.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    await user.click(screen.getByRole("button", { name: "Activate Work account" }));
    const warning = await screen.findByRole("dialog", { name: "Account limit reached" });
    const confirm = within(warning).getByRole("button", { name: "Switch anyway" });
    fireEvent.click(confirm); fireEvent.click(confirm);
    expect(api.scheduleCodexRestart).toHaveBeenCalledExactlyOnceWith(work.id, true);
    expect(within(warning).getByRole("button", { name: "Starting switch…" })).toBeDisabled();
    expect(within(warning).getByRole("button", { name: "Cancel" })).toBeDisabled();
    expect(within(warning).getByRole("status")).toHaveTextContent("Starting account switch");
    await act(async () => finish({ profileId: work.id, profileName: work.name, deadlineAt: new Date(Date.now()).toISOString(), phase: "quitting", error: null, processId: null, credentialsVerified: false }));
    expect(screen.queryByRole("dialog", { name: "Account limit reached" })).not.toBeInTheDocument();
  });
  it("does not retain the capacity override after dismissing a warning", async () => {
    const user = userEvent.setup(); render(<App />);
    await screen.findByRole("article", { name: "Work account account" });
    api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "5-hour", percent: 100 }] });
    await user.click(screen.getByRole("button", { name: "Activate Work account" }));
    const warning = await screen.findByRole("dialog", { name: "Account limit reached" });
    await user.click(within(warning).getByRole("button", { name: "Cancel" }));
    api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "5-hour", percent: 20 }] });
    api.scheduleCodexRestart.mockResolvedValue({ profileId: work.id, profileName: work.name, deadlineAt: new Date(Date.now()).toISOString(), phase: "quitting", error: null, processId: null, credentialsVerified: false });
    await user.click(screen.getByRole("button", { name: "Activate Work account" }));
    await waitFor(() => expect(api.scheduleCodexRestart).toHaveBeenCalledWith(work.id));
    expect(api.scheduleCodexRestart).toHaveBeenCalledTimes(1);
  });
  it("checks fresh usage and schedules a manual switch when the target still has capacity", async () => {
    const user = userEvent.setup();
    api.scheduleCodexRestart.mockResolvedValue({ profileId: work.id, profileName: work.name, deadlineAt: new Date(Date.now()).toISOString(), phase: "quitting", error: null, processId: null, credentialsVerified: false });
    render(<App />); await screen.findByRole("article", { name: "Work account account" });
    api.codexProfileQuota.mockClear();
    await user.click(screen.getByRole("button", { name: "Activate Work account" }));
    await waitFor(() => expect(api.scheduleCodexRestart).toHaveBeenCalledWith(work.id));
    expect(api.codexProfileQuota).toHaveBeenCalledWith(work.id);
    expect(screen.queryByRole("dialog", { name: "Account limit reached" })).not.toBeInTheDocument();
  });
  it("shows the effective recovery time and starts an immediate native switch with one click", async () => {
    const user = userEvent.setup(); exhausted();
    const status = { profileId: work.id, profileName: work.name, deadlineAt: "2026-10-17T10:00:00Z", phase: "quitting", error: null, processId: null, credentialsVerified: false };
    api.scheduleCodexRestart.mockResolvedValue(status);
    api.cancelCodexRestart.mockResolvedValue({ ...status, phase: "cancelled" });
    render(<App />);
    expect(await screen.findByText("Resets in 28h 15m")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Switch to next available account" }));
    expect(api.scheduleCodexRestart).toHaveBeenCalledWith(work.id);
    expect(await screen.findByLabelText("Account switch")).toHaveTextContent("Switching to Work account…");
    expect(screen.queryByRole("button", { name: "Switch now" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
    expect(api.cancelCodexRestart).not.toHaveBeenCalled();
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });
  it("disables quick switching when no account has usable quota", async () => {
    exhausted(); api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "weekly", percent: 100, resetsAt: "2026-10-18T14:15:00Z" }] });
    render(<App />);
    expect(await screen.findByRole("button", { name: "Switch to next available account" })).toBeDisabled();
    expect(api.scheduleCodexRestart).not.toHaveBeenCalled();
  });
});

describe("settings and refresh", () => {
  it("keeps account management out of global settings and workspace usage out of global saves", async () => {
    const user = userEvent.setup(); api.updateWorkspace.mockImplementation(async (workspace) => savedState([workspace]));
    render(<App />); await screen.findByRole("button", { name: "Workspace settings" });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Global settings" })).toBeInTheDocument();
    expect(screen.queryByText("Codex account")).not.toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Include reset predictions" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(screen.getByRole("button", { name: "Workspace settings" }));
    for (const name of ["General", "Usage", "Menu bar"]) expect(screen.getByRole("tab", { name })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Usage" }));
    await user.click(screen.getByRole("switch", { name: "Show reset updates" }));
    expect(api.updateWorkspace).toHaveBeenCalledWith(expect.objectContaining({ id: "studio", usage: expect.objectContaining({ showCodexResets: false }) }));
    expect(api.setAppSettings).not.toHaveBeenCalled();
  });

  it("opens account settings with only rename and removal, separately from workspace preferences", async () => {
    const user = userEvent.setup();
    api.renameCodexProfile.mockResolvedValue({ profiles: [personal, { ...work, name: "Office" }], activeProfileId: personal.id, canImport: true, switchError: null });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Manage Work account" }));
    const dialog = screen.getByRole("dialog", { name: "Account settings" });
    expect(within(dialog).queryByRole("tab")).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("switch")).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Replace login" })).not.toBeInTheDocument();
    await user.clear(screen.getByLabelText("Account name"));
    await user.type(screen.getByLabelText("Account name"), "Office");
    await user.click(screen.getByRole("button", { name: "Rename" }));
    await waitFor(() => expect(api.renameCodexProfile).toHaveBeenCalledWith(work.id, "Office"));
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Workspace settings" }));
    expect(screen.queryByRole("tab", { name: "Accounts" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(screen.getByRole("button", { name: "Manage Office" }));
    expect(screen.getByLabelText("Account name")).toHaveValue("Office");
    expect(screen.queryByRole("button", { name: "Account color Moss" })).not.toBeInTheDocument();
  });

  it.each([false, true])("removes an account with forget=%s only after confirmation", async (forget) => {
    const user = userEvent.setup();
    api.removeWorkspaceAccount.mockResolvedValue(savedState([{ ...studio, accountIds: [personal.id] }]));
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Manage Work account" }));
    await user.click(screen.getByRole("button", { name: "Remove account" }));
    expect(api.removeWorkspaceAccount).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: forget ? "Forget in all workspaces" : "Remove from this workspace" }));
    await waitFor(() => expect(api.removeWorkspaceAccount).toHaveBeenCalledWith(studio.id, "codex", work.id, forget));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Work account account" })).not.toBeInTheDocument();
  });

  it.each([["claude", false], ["claude", true], ["antigravity", false], ["antigravity", true]] as const)("uses the same rename/remove settings for %s with forget=%s", async (provider, everywhere) => {
    const user = userEvent.setup();
    const id = `${provider}-saved`;
    const name = `${provider} Personal`;
    const account = { id, name, email: "account@example.com", configDir: "/tmp/claude-test", createdAt: "2026-10-01", active: true, connected: true, subscription: { id: provider, service: provider, plan: "pro" }, loginCommand: "", launchCommand: "", quota: null };
    const workspace = { ...studio, claudeAccountIds: provider === "claude" ? [id] : [], antigravityAccountIds: provider === "antigravity" ? [id] : [] };
    api.getState.mockResolvedValue(savedState([workspace]));
    api.claudeProfiles.mockResolvedValue(provider === "claude" ? [account] : []);
    api.antigravityAccounts.mockResolvedValue([{ ...account, provider: "antigravity" }]);
    api.editClaudeProfile.mockResolvedValue([{ ...account, name: "Office" }]);
    api.renameAntigravityAccount.mockResolvedValue(undefined);
    render(<App />);
    await user.click(await screen.findByRole("button", { name: `Manage ${name}` }));
    const dialog = screen.getByRole("dialog", { name: "Account settings" });
    expect(within(dialog).queryByRole("tab")).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("switch")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Configuration directory")).not.toBeInTheDocument();
    await user.clear(screen.getByLabelText("Account name"));
    await user.type(screen.getByLabelText("Account name"), "Office");
    const rename = provider === "claude" ? api.editClaudeProfile : api.renameAntigravityAccount;
    rename.mockRejectedValueOnce(new Error("Could not save account name"));
    await user.click(screen.getByRole("button", { name: "Rename" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not save account name");
    expect(screen.getByLabelText("Account name")).toHaveValue("Office");
    await user.click(screen.getByRole("button", { name: "Rename" }));
    await waitFor(() => expect(provider === "claude" ? api.editClaudeProfile : api.renameAntigravityAccount).toHaveBeenCalledWith(id, "Office"));
    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(await screen.findByRole("button", { name: "Manage Office" }));
    expect(screen.getByLabelText("Account name")).toHaveValue("Office");
    await user.click(screen.getByRole("button", { name: "Remove account" }));
    expect(api.removeWorkspaceAccount).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Keep account" }));
    expect(screen.queryByRole("button", { name: "Forget in all workspaces" })).not.toBeInTheDocument();
    api.removeWorkspaceAccount.mockResolvedValue(savedState([{ ...workspace, claudeAccountIds: [], antigravityAccountIds: [] }]));
    await user.click(screen.getByRole("button", { name: "Remove account" }));
    await user.click(screen.getByRole("button", { name: everywhere ? "Forget in all workspaces" : "Remove from this workspace" }));
    await waitFor(() => expect(api.removeWorkspaceAccount).toHaveBeenCalledWith(studio.id, provider, id, everywhere));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Manage Office" })).not.toBeInTheDocument();
  });

  it("restores global settings tabs while keeping usage in the workspace", async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Settings" }));
    for (const name of ["General", "Updates"]) expect(screen.getByRole("tab", { name })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Alerts" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Display" })).not.toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: "Color theme" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(screen.getByRole("button", { name: "Workspace settings" }));
    await user.click(screen.getByRole("tab", { name: "Menu bar" }));
    expect(screen.getByRole("switch", { name: "Menu bar usage" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Global settings" })).not.toBeInTheDocument();
  });

  it("applies a saved account accent without recoloring its workspace or sibling", async () => {
    const initial = savedState(); initial.app.codexAccounts = { [work.id]: { ...DEFAULT_CODEX_ACCOUNT_PREFERENCES, color: "#2f9e6e" } };
    api.getState.mockResolvedValue(initial);
    render(<App />);
    await screen.findByRole("article", { name: "Work account account" });
    expect(screen.getByRole("article", { name: "Work account account" }).style.getPropertyValue("--accent")).toBe("#2f9e6e");
    expect(screen.getByRole("article", { name: "Personal account account" }).style.getPropertyValue("--accent")).toBe("#007aff");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe(studio.accent);
  });

  it("shows this saved account's quota and refreshes an open popup", async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole("button", { name: "Usage limits for Work account" }));
    expect(await screen.findByText("80%")).toBeInTheDocument();
    api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "5-hour", percent: 21 }] });
    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(screen.getByRole("button", { name: "Refresh data" }));
    await waitFor(() => expect(api.getState).toHaveBeenCalledTimes(2));
    await user.click(screen.getByRole("button", { name: "Usage limits for Work account" }));
    expect(await screen.findByText("79%")).toBeInTheDocument();
    expect(api.cliQuotas).not.toHaveBeenCalled();
  });

  it("recovers a rejected saved session through the same account and refetches usage after replacement", async () => {
    const user = userEvent.setup();
    api.codexProfileQuota.mockImplementation(async (id) => id === work.id
      ? { provider: "codex", error: "session could not be renewed — reconnect this account or replace its auth.json in account settings" }
      : { provider: "codex", windows: [{ window: "5-hour", percent: 20 }] });
    api.importCodexProfileJson.mockImplementation(async () => {
      api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "Weekly", percent: 12 }] });
      return { profiles: [personal, { ...work, needsActivation: true }], activeProfileId: personal.id, canImport: true, switchError: null };
    });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Usage limits for Work account" }));
    expect(await screen.findByText(/This account’s saved session is no longer valid/)).toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Reconnect account" }));
    expect(screen.queryByRole("dialog", { name: "Usage limits" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Replace login" }));
    await uploadAuth(user, '{"fixture":"renewed"}');
    await user.click(screen.getByRole("button", { name: "Save login" }));
    await waitFor(() => expect(api.importCodexProfileJson).toHaveBeenCalledWith(work.name, '{"fixture":"renewed"}', work.id));
    await screen.findByRole("button", { name: "Replace login" });
    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(screen.getByRole("button", { name: "Usage limits for Work account" }));
    expect(await screen.findByText("88%")).toBeInTheDocument();
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
    expect(api.scheduleCodexRestart).not.toHaveBeenCalled();
  });

  it("updates an open usage popup from 18% to 17% at the configured refresh interval", async () => {
    vi.useFakeTimers();
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    const state = savedState();
    api.getState.mockResolvedValue({ ...state, app: { ...state.app, refreshIntervalSecs: 30 } });
    api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "5-hour", percent: 82 }] });
    try {
      await act(async () => { render(<App />); });
      await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Usage limits for Work account" })); });
      const popup = screen.getByRole("dialog", { name: "Usage limits" });
      expect(within(popup).getByText("18%")).toBeInTheDocument();
      api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "5-hour", percent: 83 }] });
      await act(async () => { vi.advanceTimersByTime(29_000); });
      expect(within(popup).getByText("18%")).toBeInTheDocument();
      await act(async () => { vi.advanceTimersByTime(1000); });
      expect(screen.getByRole("dialog", { name: "Usage limits" })).toBe(popup);
      expect(within(popup).getByText("17%")).toBeInTheDocument();
      expect(within(popup).queryByText("18%")).not.toBeInTheDocument();
      expect(within(popup).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "17");
      expect(api.cliQuotas).not.toHaveBeenCalled();
    } finally {
      cleanup();
      vi.useRealTimers();
    }
  });

  it("loads saved accounts even if another provider probe fails", async () => {
    api.cliStatus.mockRejectedValue(new Error("probe failed")); render(<App />);
    expect(await screen.findByRole("article", { name: "Personal account account" })).toBeInTheDocument();
    expect(screen.getByText("Couldn't read CLI status: probe failed")).toBeInTheDocument();
  });

  it("shows the initial loading skeleton and reports load failures", async () => {
    api.getState.mockReturnValueOnce(new Promise(() => {})); const view = render(<App />);
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument(); view.unmount();
    api.getState.mockRejectedValue(new Error("permission denied")); render(<App />);
    expect(await screen.findByText("Couldn't load settings: permission denied")).toBeInTheDocument();
  });

  it("retains the workspace accent when the native color bridge is unavailable", async () => {
    api.setMenuBarAccent.mockRejectedValue(new Error("unavailable")); render(<App />);
    expect(await screen.findByRole("article", { name: "Personal account account" })).toBeInTheDocument();
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe(studio.accent);
  });
});
