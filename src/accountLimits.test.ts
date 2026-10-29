import { describe, expect, it } from "vitest";
import { accountLimitText, nextUsableAccount } from "./accountLimits";
import type { CodexProfile } from "./types";

const now = Date.parse("2026-10-17T10:00:00Z");
const profile = (id: string): CodexProfile => ({ id, name: id, connected: true, active: id === "a", email: null, plan: null, connectionError: null, needsActivation: false, loginCommand: "" });
describe("account limits", () => {
  it("counts to the last exhausted main window in total hours and minutes", () => {
    expect(accountLimitText({ provider: "codex", windows: [
      { window: "5-hour", percent: 100, resetsAt: "2026-10-17T11:00:00Z" },
      { window: "weekly", percent: 100, resetsAt: "2026-10-18T14:15:00Z" },
      { window: "gpt-reserve", percent: 100, resetsAt: "2027-01-01T00:00:00Z" },
    ] }, now)).toBe("Resets in 28h 15m");
  });
  it("keeps exhaustion explicit until a fresh response confirms reset", () => {
    expect(accountLimitText({ provider: "codex", windows: [{ window: "5-hour", percent: 100, resetsAt: "2026-10-17T09:00:00Z" }] }, now)).toContain("Checking reset");
    expect(accountLimitText({ provider: "codex", windows: [{ window: "weekly", percent: 100 }] }, now)).toContain("Reset time unavailable");
    expect(accountLimitText({ provider: "codex", error: "offline", windows: [{ window: "weekly", percent: 100 }] }, now)).toBeNull();
  });
  it("skips failed, exhausted, stale, incomplete and disconnected targets, then wraps in saved order", () => {
    const profiles = ["a", "b", "c", "d", "e"].map(profile);
    const quotas = {
      a: { provider: "codex", windows: [{ window: "5-hour", percent: 10 }] },
      b: { provider: "codex", error: "offline" },
      c: { provider: "codex", windows: [{ window: "weekly", percent: 100 }] },
      d: { provider: "codex", windows: [{ window: "5-hour", percent: 2, resetsAt: "2026-10-17T09:00:00Z" }] },
      e: { provider: "codex", windows: [{ window: "5-hour", percent: 5 }, { window: "weekly", percent: 20 }] },
    };
    expect(nextUsableAccount(profiles, "a", quotas, now)?.id).toBe("e");
    expect(nextUsableAccount(profiles, "e", quotas, now)?.id).toBe("a");
    expect(nextUsableAccount(profiles, "a", quotas, now, { e: ["monthly"] })).toBeNull();
    profiles[4].connected = false;
    expect(nextUsableAccount(profiles, "a", quotas, now)).toBeNull();
  });
});
