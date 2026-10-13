import { describe, expect, it, beforeEach } from "vitest";
import {
  applyDisplayPrefs,
  DEFAULT_DISPLAY_PREFS,
  DISPLAY_PREFS_KEY,
  loadDisplayPrefs,
  resetDisplayPrefs,
  saveDisplayPrefs,
} from "./displayPrefs";

beforeEach(() => {
  localStorage.clear();
});

describe("displayPrefs", () => {
  it("returns defaults when nothing is stored", () => {
    expect(loadDisplayPrefs()).toEqual(DEFAULT_DISPLAY_PREFS);
  });

  it("round-trips prefs through localStorage", () => {
    saveDisplayPrefs({ ...DEFAULT_DISPLAY_PREFS, compactRows: true });
    expect(loadDisplayPrefs().compactRows).toBe(true);
  });

  it("ignores malformed payloads and falls back to defaults", () => {
    localStorage.setItem(DISPLAY_PREFS_KEY, "{not json");
    expect(loadDisplayPrefs()).toEqual(DEFAULT_DISPLAY_PREFS);
  });

  it("resets to defaults and clears storage", () => {
    saveDisplayPrefs({ ...DEFAULT_DISPLAY_PREFS, compactRows: true });
    expect(resetDisplayPrefs()).toEqual(DEFAULT_DISPLAY_PREFS);
    expect(localStorage.getItem(DISPLAY_PREFS_KEY)).toBeNull();
  });

  it("migrates legacy prefs without theme/accent to defaults", () => {
    localStorage.setItem(
      DISPLAY_PREFS_KEY,
      JSON.stringify({ showDetails: false, compactRows: true }),
    );
    const prefs = loadDisplayPrefs();
    expect(prefs.showDetails).toBe(false);
    expect(prefs.theme).toBe("dark");
    expect(prefs.accent).toBe(DEFAULT_DISPLAY_PREFS.accent);
  });

  it("rejects invalid theme/accent values", () => {
    localStorage.setItem(
      DISPLAY_PREFS_KEY,
      JSON.stringify({ ...DEFAULT_DISPLAY_PREFS, theme: "neon", accent: "red" }),
    );
    const prefs = loadDisplayPrefs();
    expect(prefs.theme).toBe(DEFAULT_DISPLAY_PREFS.theme);
    expect(prefs.accent).toBe(DEFAULT_DISPLAY_PREFS.accent);
  });

  it("paints data-theme and accent vars onto <html>", () => {
    applyDisplayPrefs({ theme: "light", accent: "#2f9e6e" });
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#2f9e6e");
    expect(document.documentElement.style.getPropertyValue("--accent-hover")).not.toBe("");
    expect(document.documentElement.style.getPropertyValue("--accent-bg")).toContain("rgba(");
  });
});
