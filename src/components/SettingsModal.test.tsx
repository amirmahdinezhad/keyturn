import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type { AppSettings } from "../types";

const { mockCheck, mockRelaunch, mockGetVersion } = vi.hoisted(() => ({
  mockCheck: vi.fn(),
  mockRelaunch: vi.fn(async () => {}),
  mockGetVersion: vi.fn(async () => "0.1.0"),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({ check: mockCheck }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mockRelaunch }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: mockGetVersion }));

vi.mock("../api", () => ({
  setAppSettings: vi.fn(async (settings: Partial<AppSettings>) =>
    makeState({ app: { ...makeState().app, ...settings } }),
  ),
  setSettings: vi.fn(),
  prettyError: (e: unknown) => (e instanceof Error ? e.message : String(e)),
}));

import { SettingsModal, SKIPPED_UPDATE_KEY } from "./SettingsModal";
import { makeState } from "../test/fixtures";

function renderModal(initialState = makeState()) {
  function SettingsHarness() {
    const [state, setState] = useState(initialState);
    return (
      <SettingsModal
        open
        onClose={() => {}}
        state={state}
        onChanged={setState}
        notify={() => {}}
      />
    );
  }
  return render(<SettingsHarness />);
}

async function openUpdatesTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("tab", { name: "Updates" }));
}

function fakeUpdate() {
  return {
    version: "9.9.9",
    currentVersion: "0.1.0",
    body: "Bug fixes.",
    download: vi.fn(async (onEvent: (e: unknown) => void) => {
      onEvent({ event: "Started", data: { contentLength: 100 } });
      onEvent({ event: "Progress", data: { chunkLength: 100 } });
      onEvent({ event: "Finished" });
    }),
    install: vi.fn(async () => {}),
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
});

afterEach(cleanup);

describe("SettingsModal updates", () => {
  it("shows the running version and reports when on latest", async () => {
    const user = userEvent.setup();
    mockCheck.mockResolvedValue(null);
    renderModal();
    await openUpdatesTab(user);

    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check now" }));

    expect(
      await screen.findByText("You're on the latest version."),
    ).toBeInTheDocument();
  });

  it("offers download and skip with release notes when an update exists", async () => {
    const user = userEvent.setup();
    mockCheck.mockResolvedValue(fakeUpdate());
    renderModal();
    await openUpdatesTab(user);

    await user.click(screen.getByRole("button", { name: "Check now" }));
    expect(
      await screen.findByText("Version 9.9.9 is available"),
    ).toBeInTheDocument();
    expect(screen.getByText("Bug fixes.")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Download" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Skip this version" }),
    ).toBeInTheDocument();
  });

  it("skipping persists and later checks respect it", async () => {
    const user = userEvent.setup();
    mockCheck.mockResolvedValue(fakeUpdate());
    renderModal();
    await openUpdatesTab(user);

    await user.click(screen.getByRole("button", { name: "Check now" }));
    await user.click(
      await screen.findByRole("button", { name: "Skip this version" }),
    );

    expect(localStorage.getItem(SKIPPED_UPDATE_KEY)).toBe("9.9.9");
    expect(
      await screen.findByText("You're on the latest version."),
    ).toBeInTheDocument();

    // Same version again: still treated as current without asking twice.
    await user.click(screen.getByRole("button", { name: "Check now" }));
    expect(
      await screen.findByText("You're on the latest version."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Version 9.9.9 is available"),
    ).not.toBeInTheDocument();
  });

  it("downloads with progress, then installs and relaunches", async () => {
    const user = userEvent.setup();
    const update = fakeUpdate();
    mockCheck.mockResolvedValue(update);
    renderModal();
    await openUpdatesTab(user);

    await user.click(screen.getByRole("button", { name: "Check now" }));
    await user.click(await screen.findByRole("button", { name: "Download" }));

    expect(
      await screen.findByText("Version 9.9.9 downloaded"),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Install & relaunch" }),
    );
    expect(update.install).toHaveBeenCalledTimes(1);
    expect(mockRelaunch).toHaveBeenCalledTimes(1);
  });

  it("surfaces check failures inline", async () => {
    const user = userEvent.setup();
    mockCheck.mockRejectedValue(new Error("offline"));
    renderModal();
    await openUpdatesTab(user);

    await user.click(screen.getByRole("button", { name: "Check now" }));

    expect(await screen.findByText("offline")).toBeInTheDocument();
  });
});

describe("SettingsModal display prefs", () => {
  it("keeps reset in General and removes custom color and icon-credit controls", async () => {
    const user = userEvent.setup();
    renderModal();
    expect(
      screen.getAllByRole("button", { name: "Reset to defaults" }),
    ).toHaveLength(1);
    await user.click(screen.getByRole("tab", { name: "Display" }));
    expect(
      screen.queryByLabelText("Custom accent color"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Reset to defaults" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Updates" }));
    expect(screen.queryByText(/Limit icon|Flaticon/)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Reset to defaults" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Check now" }),
    ).toBeInTheDocument();
  });

  it("persists a preset accent immediately", async () => {
    const user = userEvent.setup();
    renderModal();
    await user.click(screen.getByRole("tab", { name: "Display" }));
    await user.click(screen.getByRole("button", { name: "Accent Orchid" }));
    expect(
      screen.getByRole("button", { name: "Accent Orchid" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      JSON.parse(localStorage.getItem("keyturn.displayPrefs")!),
    ).toMatchObject({ accent: "#7c6cf0" });
  });

  it("moves through the settings tabs with arrow keys and links the panel", async () => {
    const user = userEvent.setup();
    renderModal();
    const general = screen.getByRole("tab", { name: "General" });
    general.focus();
    await user.keyboard("{ArrowRight}");
    const display = screen.getByRole("tab", { name: "Display" });
    expect(display).toHaveFocus();
    expect(display).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel")).toHaveAttribute(
      "aria-labelledby",
      display.id,
    );
    expect(general).toHaveAttribute("tabindex", "-1");
    await user.keyboard("{End}");
    expect(screen.getByRole("tab", { name: "Updates" })).toHaveFocus();
    await user.keyboard("{Home}");
    expect(general).toHaveFocus();
  });

  it("toggles remaining mode through setAppSettings", async () => {
    const user = userEvent.setup();
    const { setAppSettings } = await import("../api");
    renderModal();

    await user.click(screen.getByRole("tab", { name: "Display" }));
    await user.click(screen.getByRole("radio", { name: "Left" }));

    expect(setAppSettings).toHaveBeenCalledWith({ usageShowRemaining: true });
  });

  it("selects refresh intervals with the keyboard and keeps one selected", async () => {
    const user = userEvent.setup();
    const { setAppSettings } = await import("../api");
    renderModal();
    const selected = screen.getByRole("radio", { name: "60s" });
    expect(selected).toBeChecked();
    selected.focus();
    await user.keyboard("{ArrowRight} ");
    expect(screen.getByRole("radio", { name: "120s" })).toHaveFocus();
    expect(setAppSettings).toHaveBeenLastCalledWith({
      refreshIntervalSecs: 120,
    });
    expect(screen.getByRole("radio", { name: "120s" })).toBeChecked();
    expect(selected).not.toBeChecked();
    await user.keyboard(" ");
    expect(screen.getByRole("radio", { name: "120s" })).toBeChecked();
    expect(setAppSettings).toHaveBeenCalledTimes(1);
  });

  it("disables refresh intervals until the selected cadence is saved", async () => {
    const user = userEvent.setup();
    const { setAppSettings } = await import("../api");
    let finishSave!: (state: ReturnType<typeof makeState>) => void;
    vi.mocked(setAppSettings).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishSave = resolve;
        }),
    );
    renderModal();
    await user.click(screen.getByRole("radio", { name: "120s" }));
    expect(setAppSettings).toHaveBeenCalledWith({ refreshIntervalSecs: 120 });
    for (const option of screen.getAllByRole("radio")) {
      expect(option).toBeDisabled();
    }
    expect(screen.getByRole("radio", { name: "60s" })).toBeChecked();
    finishSave(makeState({ app: { ...makeState().app, refreshIntervalSecs: 120 } }));
    await waitFor(() =>
      expect(screen.getByRole("radio", { name: "120s" })).toBeEnabled(),
    );
    expect(screen.getByRole("radio", { name: "120s" })).toBeChecked();
  });

  it("preserves a saved custom cadence without silently changing settings", async () => {
    const { setAppSettings } = await import("../api");
    renderModal(makeState({ app: { ...makeState().app, refreshIntervalSecs: 45 } }));
    expect(screen.getByRole("radio", { name: "45s" })).toBeChecked();
    expect(setAppSettings).not.toHaveBeenCalled();
  });

  it("keeps the saved cadence selected when saving fails", async () => {
    const user = userEvent.setup();
    const { setAppSettings } = await import("../api");
    vi.mocked(setAppSettings).mockRejectedValueOnce(new Error("Save failed"));
    renderModal();
    await user.click(screen.getByRole("radio", { name: "30s" }));
    expect(screen.getByRole("radio", { name: "60s" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "30s" })).not.toBeChecked();
    expect(screen.getByRole("radio", { name: "30s" })).toBeEnabled();
  });

  it("persists a display switch changed with the keyboard", async () => {
    const user = userEvent.setup();
    renderModal();
    await user.click(screen.getByRole("tab", { name: "Display" }));
    const compact = screen.getByRole("switch", { name: "Compact rows" });
    compact.focus();
    await user.keyboard(" ");
    expect(compact).toBeChecked();
    expect(
      JSON.parse(localStorage.getItem("keyturn.displayPrefs")!),
    ).toMatchObject({ compactRows: true });
  });
});
