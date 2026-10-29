import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { CodexProfile, CodexProfilesState } from "../types";

const api = vi.hoisted(() => ({
  codexProfiles: vi.fn(), createCodexProfile: vi.fn(), renameCodexProfile: vi.fn(),
  removeCodexProfile: vi.fn(), activateCodexProfile: vi.fn(),
  importCodexProfile: vi.fn(), startCodexProfileLogin: vi.fn(), codexProfileLoginStatus: vi.fn(), cancelCodexProfileLogin: vi.fn(),
  scheduleCodexRestart: vi.fn(), codexRestartStatus: vi.fn(), cancelCodexRestart: vi.fn(), runCodexRestartNow: vi.fn(),
  prettyError: (e: unknown) => e instanceof Error ? e.message : String(e),
  codexProfileQuota: vi.fn(), setCodexAccountPreferences: vi.fn(), setAppSettings: vi.fn(),
}));
vi.mock("../api", () => api);
import { CodexProfilesModal } from "./CodexProfilesModal";
import { makeState } from "../test/fixtures";
import { DEFAULT_CODEX_ACCOUNT_PREFERENCES } from "../types";

const personal: CodexProfile = {
  id: "personal", name: "Personal", email: "personal@example.com", plan: "plus", connected: true, active: true,
  loginCommand: "CODEX_HOME='/private/personal' codex login",
  connectionError: null,
  needsActivation: false,
};
const work: CodexProfile = { ...personal, id: "work", name: "Work", email: "work@example.com", active: false };
function state(profiles: CodexProfile[] = []): CodexProfilesState {
  return { profiles, activeProfileId: profiles.find((p) => p.active)?.id ?? null, canImport: true, switchError: null };
}
function setup() {
  const props = { open: true, onClose: vi.fn(), onChanged: vi.fn(), onActivated: vi.fn(async () => {}), notify: vi.fn() };
  return { user: userEvent.setup(), props, ...render(<CodexProfilesModal {...props} />) };
}
beforeEach(() => {
  vi.resetAllMocks(); api.codexProfiles.mockResolvedValue(state());
  api.cancelCodexProfileLogin.mockResolvedValue(undefined);
  api.startCodexProfileLogin.mockRejectedValue(new Error("Use Terminal instead."));
  api.codexRestartStatus.mockResolvedValue(null);
  api.scheduleCodexRestart.mockImplementation(async (id: string) => {
    const job = { profileId: id, profileName: "Work", deadlineAt: new Date(Date.now()+30000).toISOString(), phase: "countdown", error: null, processId: null, credentialsVerified: false };
    api.codexRestartStatus.mockResolvedValue(job); return job;
  });
  api.cancelCodexRestart.mockResolvedValue(null);
  api.codexProfileQuota.mockResolvedValue({ provider: "codex", windows: [{ window: "Weekly", percent: 0 }] });
});
afterEach(cleanup);

describe("Codex profiles", () => {
  it("offers opt-in from the second account and explicitly enrolls the first pair", async () => {
    const first = { ...personal, active: false };
    const second = { ...work, active: true };
    api.codexProfiles.mockResolvedValue(state([first]));
    api.createCodexProfile.mockResolvedValue(state([first, second]));
    api.setCodexAccountPreferences.mockResolvedValue(makeState({ app: { ...makeState().app, codexAccounts: { personal: { ...DEFAULT_CODEX_ACCOUNT_PREFERENCES, autoSwitch: true }, work: { ...DEFAULT_CODEX_ACCOUNT_PREFERENCES, autoSwitch: true } } } }));
    const onAppChanged = vi.fn(); const user = userEvent.setup();
    render(<CodexProfilesModal open onClose={() => {}} onChanged={() => {}} onActivated={async () => {}} notify={() => {}} appState={makeState()} onAppChanged={onAppChanged} />);
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    const join = screen.getByRole("switch", { name: "Join automatic switch cycle" });
    expect(join).not.toBeChecked();
    expect(screen.getByText("Switch automatically between Personal and this account")).toBeInTheDocument();
    await user.click(join);
    await user.click(screen.getByRole("button", { name: "Save profile" }));
    await waitFor(() => expect(api.setCodexAccountPreferences).toHaveBeenCalledWith("work", expect.objectContaining({ autoSwitch: true, menuBarWindows: [] }), "personal"));
    expect(onAppChanged).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("switch", { name: "Show Weekly limit" })).not.toBeChecked();
  });
  it("leaves credentials untouched when backing out of the custom import form", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    await user.click(screen.getByLabelText("Import auth.json file"));
    expect(screen.getByRole("button", { name: "Import profile" })).toBeDisabled();
    await user.type(screen.getByLabelText("auth.json path"), "~/Downloads/auth.json");
    await user.click(screen.getByRole("button", { name: "All profiles" }));
    expect(api.importCodexProfile).not.toHaveBeenCalled();
    expect(api.scheduleCodexRestart).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Add profile" })).toBeInTheDocument();
  });

  it("shows saved profile names with service and tier badges", async () => {
    api.codexProfiles.mockResolvedValue(state([personal, { ...work, plan: "pro" }]));
    setup();
    const personalCard = await screen.findByRole("button", { name: "Manage Personal" });
    const workCard = screen.getByRole("button", { name: "Manage Work" });
    expect(personalCard).toHaveTextContent("ChatGPTPlus");
    expect(workCard).toHaveTextContent("ChatGPTPro");
    expect(personalCard).not.toHaveTextContent(/\$|\/mo/);
    expect(workCard).not.toHaveTextContent(/\$|\/mo/);
  });

  it("hides expired subscriptions throughout the saved profile list", async () => {
    api.codexProfiles.mockResolvedValue(state([personal, { ...work, subscription: { id: "chatgpt", service: "ChatGPT", plan: "pro", status: "expired" } }]));
    setup();
    await screen.findByRole("button", { name: "Manage Personal" });
    expect(screen.queryByRole("button", { name: "Manage Work" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Set Work active" })).not.toBeInTheDocument();
    expect(screen.queryByText("Expired profiles (1)")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Manage expired Work" })).not.toBeInTheDocument();
  });

  it("imports a work file without starting login or activating the account", async () => {
    const imported = { ...work, needsActivation: true };
    api.importCodexProfile.mockResolvedValue(state([imported]));
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    await user.type(screen.getByLabelText("Profile name"), "Work");
    await user.click(screen.getByLabelText("Import auth.json file"));
    await user.type(screen.getByLabelText("auth.json path"), "~/Downloads/auth.json");
    await user.click(screen.getByRole("button", { name: "Import profile" }));
    expect(await screen.findByRole("button", { name: "Cancel restart" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Set Work active" })).toBeDisabled();
    expect(api.scheduleCodexRestart).toHaveBeenCalledWith("work");
    expect(api.importCodexProfile).toHaveBeenCalledWith("Work", "~/Downloads/auth.json");
    expect(api.createCodexProfile).not.toHaveBeenCalled();
    expect(api.startCodexProfileLogin).not.toHaveBeenCalled();
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
    expect(screen.getByText(/Set active starts a 30-second/)).toBeInTheDocument();
  });

  it("retains the custom import form when the file cannot be read", async () => {
    api.importCodexProfile.mockRejectedValue(new Error("Cannot read login file."));
    const { user, props } = setup();
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    await user.type(screen.getByLabelText("Profile name"), "Work");
    await user.click(screen.getByLabelText("Import auth.json file"));
    await user.type(screen.getByLabelText("auth.json path"), "~/Downloads/auth.json");
    await user.click(screen.getByRole("button", { name: "Import profile" }));
    expect(screen.getByLabelText("Profile name")).toHaveValue("Work");
    expect(props.notify).not.toHaveBeenCalled();
    expect(await screen.findByRole("alert")).toHaveTextContent("Cannot read login file.");
    expect(screen.getByLabelText("auth.json path")).toHaveValue("~/Downloads/auth.json");
    expect(api.scheduleCodexRestart).not.toHaveBeenCalled();
  });

  it("replaces a saved login from a new file while waiting for explicit activation", async () => {
    api.codexProfiles.mockResolvedValue(state([work]));
    api.importCodexProfile.mockResolvedValue(state([{ ...work, needsActivation: true }]));
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Manage Work" }));
    await user.click(screen.getByRole("button", { name: "Replace login from file" }));
    expect(api.importCodexProfile).not.toHaveBeenCalled();
    await user.type(screen.getByLabelText("auth.json path"), "/tmp/auth.json");
    await user.click(screen.getByRole("button", { name: "Import profile" }));
    expect(await screen.findByRole("button", { name: "Cancel restart" })).toBeInTheDocument();
    expect(api.importCodexProfile).toHaveBeenCalledWith("Work", "/tmp/auth.json", "work");
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });

  it("cancels the import restart before changing credentials", async () => {
    api.importCodexProfile.mockResolvedValue(state([{ ...work, needsActivation: true }]));
    const { user, props } = setup();
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    await user.type(screen.getByLabelText("Profile name"), "Work");
    await user.click(screen.getByLabelText("Import auth.json file"));
    await user.type(screen.getByLabelText("auth.json path"), "~/Downloads/auth.json");
    await user.click(screen.getByRole("button", { name: "Import profile" }));
    await user.click(await screen.findByRole("button", { name: "Cancel restart" }));
    expect(api.cancelCodexRestart).toHaveBeenCalledOnce();
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
    expect(props.onActivated).not.toHaveBeenCalled();
  });

  it("refreshes account identity after native restart verification", async () => {
    const verified = { profileId: "work", profileName: "Work", deadlineAt: "2026-10-14T00:00:00Z", phase: "verified", error: null, processId: 123, credentialsVerified: true };
    api.codexRestartStatus.mockResolvedValue(verified);
    api.codexProfiles.mockResolvedValue(state([{ ...work, active: true }]));
    const { props } = setup();
    await waitFor(() => expect(props.onActivated).toHaveBeenCalledOnce());
    expect(screen.getByText(/Codex reopened with Work credentials/)).toBeInTheDocument();
    expect(props.onChanged).toHaveBeenCalledWith(state([{ ...work, active: true }]));
  });

  it("finishes identity refresh while the modal is closed and its callback changes", async () => {
    api.importCodexProfile.mockResolvedValue(state([{ ...work, needsActivation: true }]));
    const { user, props, rerender } = setup();
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    await user.type(screen.getByLabelText("Profile name"), "Work");
    await user.click(screen.getByLabelText("Import auth.json file"));
    await user.type(screen.getByLabelText("auth.json path"), "~/Downloads/auth.json");
    await user.click(screen.getByRole("button", { name: "Import profile" }));
    await screen.findByRole("button", { name: "Cancel restart" });
    rerender(<CodexProfilesModal {...props} open={false} />);
    api.codexRestartStatus.mockResolvedValue({ profileId: "work", profileName: "Work", deadlineAt: "unique", phase: "verified", error: null, processId: 123, credentialsVerified: true });
    api.codexProfiles.mockImplementation(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
      return state([{ ...work, active: true }]);
    });
    const callback = vi.fn(async () => {});
    rerender(<CodexProfilesModal {...props} open={false} onActivated={callback} />);
    await waitFor(() => expect(callback).toHaveBeenCalledOnce(), { timeout: 2000 });
    expect(props.onChanged).toHaveBeenLastCalledWith(state([{ ...work, active: true }]));
  });

  it("adds a personal account through browser login and updates automatically on completion", async () => {
    const draft = { ...work, connected: false, email: null };
    api.createCodexProfile.mockResolvedValue(state([draft]));
    api.startCodexProfileLogin.mockResolvedValue({ profileId: "work", status: "pending", error: null });
    api.codexProfileLoginStatus.mockResolvedValue({ profileId: "work", status: "completed", error: null });
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    await user.type(screen.getByLabelText("Profile name"), "Work");
    await user.click(screen.getByLabelText("Sign in to another account"));
    await user.click(screen.getByRole("button", { name: "Continue with browser" }));
    await screen.findByRole("button", { name: "Cancel sign-in" });
    api.codexProfiles.mockResolvedValue(state([work]));
    expect(await screen.findByRole("button", { name: "Set Work active" })).toBeInTheDocument();
    expect(api.startCodexProfileLogin).toHaveBeenCalledWith("work");
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });

  it("cancels a pending browser sign-in without changing the current account", async () => {
    api.codexProfiles.mockResolvedValue(state([{ ...work, connected: false }]));
    api.startCodexProfileLogin.mockResolvedValue({ profileId: "work", status: "pending", error: null });
    api.codexProfileLoginStatus.mockResolvedValue({ profileId: "work", status: "pending", error: null });
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Manage Work" }));
    await user.click(screen.getByRole("button", { name: "Sign in with browser" }));
    await user.click(await screen.findByRole("button", { name: "Cancel sign-in" }));
    expect(await screen.findByRole("button", { name: "Sign in with browser" })).toBeInTheDocument();
    expect(api.cancelCodexProfileLogin).toHaveBeenCalledWith("work");
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });

  it("imports the current account with a trimmed, custom name", async () => {
    api.createCodexProfile.mockResolvedValue(state([personal]));
    const { user, props } = setup();
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    await user.type(screen.getByLabelText("Profile name"), " Personal ");
    expect(screen.getByLabelText("Save current Codex account")).toBeChecked();
    await user.click(screen.getByRole("button", { name: "Save profile" }));
    await screen.findByRole("button", { name: "Manage Personal" });
    expect(api.createCodexProfile).toHaveBeenCalledWith("Personal", true);
    expect(props.onChanged).toHaveBeenLastCalledWith(state([personal]));
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });

  it("creates a separate login and stays there until a saved login is present", async () => {
    const draft = { ...work, connected: false, email: null };
    api.createCodexProfile.mockResolvedValue(state([draft]));
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Add profile" }));
    await user.type(screen.getByLabelText("Profile name"), "Work");
    await user.click(screen.getByLabelText("Sign in to another account"));
    await user.click(screen.getByRole("button", { name: "Continue with browser" }));
    expect(await screen.findByText(draft.loginCommand)).toBeInTheDocument();
    expect(api.createCodexProfile).toHaveBeenCalledWith("Work", false);
    api.codexProfiles.mockResolvedValue(state([draft]));
    await user.click(screen.getByRole("button", { name: "Check connection" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No saved ChatGPT login found");
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
    api.codexProfiles.mockResolvedValue(state([work]));
    await user.click(screen.getByRole("button", { name: "Check connection" }));
    expect(await screen.findByRole("button", { name: "Set Work active" })).toBeInTheDocument();
  });

  it("schedules desktop activation without replacing credentials before the countdown", async () => {
    api.codexProfiles.mockResolvedValue(state([personal, work]));
    const { user, props } = setup();
    await user.click(await screen.findByRole("button", { name: "Set Work active" }));
    expect(await screen.findByRole("button", { name: "Restart now" })).toBeInTheDocument();
    expect(api.scheduleCodexRestart).toHaveBeenCalledWith("work");
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
    expect(props.onActivated).not.toHaveBeenCalled();
  });

  it("supports explicit CLI-only activation and refreshes account identity", async () => {
    api.codexProfiles.mockResolvedValue(state([personal, work]));
    const switched = state([{ ...personal, active: false }, { ...work, active: true }]);
    api.activateCodexProfile.mockResolvedValue(switched);
    const { user, props } = setup();
    await user.click(await screen.findByRole("button", { name: "Manage Work" }));
    await user.click(screen.getByRole("button", { name: "Activate for CLI only" }));
    await waitFor(() => expect(props.onActivated).toHaveBeenCalledOnce());
    expect(api.activateCodexProfile).toHaveBeenCalledWith("work");
    expect(props.onChanged).toHaveBeenLastCalledWith(switched);
    expect(screen.getByRole("status")).toHaveTextContent("Reopen Codex");
    expect(api.scheduleCodexRestart).not.toHaveBeenCalled();
  });

  it("keeps the existing active account when switching fails", async () => {
    api.codexProfiles.mockResolvedValue(state([personal, work]));
    api.scheduleCodexRestart.mockRejectedValue(new Error("Cannot save profile data."));
    const { user, props } = setup();
    await user.click(await screen.findByRole("button", { name: "Set Work active" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Cannot save profile data");
    expect(screen.getByRole("button", { name: "Set Work active" })).toBeEnabled();
    expect(props.onActivated).not.toHaveBeenCalled();
    expect(props.notify).not.toHaveBeenCalled();
  });

  it("renames and confirms removal without activating another account", async () => {
    api.codexProfiles.mockResolvedValue(state([personal, work]));
    api.renameCodexProfile.mockResolvedValue(state([personal, { ...work, name: "Team" }]));
    api.removeCodexProfile.mockResolvedValue(state([personal]));
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Manage Work" }));
    await user.clear(screen.getByLabelText("Profile name"));
    await user.type(screen.getByLabelText("Profile name"), "Team");
    await user.click(screen.getByRole("button", { name: "Save name" }));
    await user.click(await screen.findByRole("button", { name: "Manage Team" }));
    await user.click(screen.getByRole("button", { name: "Remove profile" }));
    expect(api.removeCodexProfile).not.toHaveBeenCalled();
    expect(screen.getByText(/active Codex account stays signed in/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove profile" }));
    await waitFor(() => expect(api.removeCodexProfile).toHaveBeenCalledWith("work"));
    expect(await screen.findByRole("button", { name: "Manage Personal" })).toBeInTheDocument();
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });

  it("disables activation for unsupported credential storage", async () => {
    api.codexProfiles.mockResolvedValue({ ...state([work]), canImport: false, switchError: "Use file-based storage." });
    setup();
    expect(await screen.findByRole("button", { name: "Set Work active" })).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("Use file-based storage");
  });

  it("explains a duplicate browser login and keeps the profile ready for another sign-in", async () => {
    const duplicate = { ...work, connected: false, connectionError: "This account is already saved as Personal. Sign in with a different account." };
    api.codexProfiles.mockResolvedValue(state([personal, duplicate]));
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Manage Work" }));
    expect(screen.getByRole("alert")).toHaveTextContent("already saved as Personal");
    await user.click(screen.getByText("Use Terminal instead"));
    await user.click(screen.getByRole("button", { name: "Check connection" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Check connection" })).toBeEnabled());
    expect(screen.getByRole("alert")).toHaveTextContent("different account");
    expect(api.activateCodexProfile).not.toHaveBeenCalled();
  });

  it("can retry a failed profile load", async () => {
    api.codexProfiles.mockRejectedValueOnce(new Error("Cannot read saved profiles."));
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: "Add profile" })).toBeInTheDocument();
  });

  it("ignores a profile response after the modal closes", async () => {
    let resolve!: (value: CodexProfilesState) => void;
    api.codexProfiles.mockReturnValue(new Promise<CodexProfilesState>((r) => { resolve = r; }));
    const { props, rerender } = setup();
    rerender(<CodexProfilesModal {...props} open={false} />);
    await act(async () => { resolve(state([personal])); });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(props.onChanged).not.toHaveBeenCalled();
  });
});
