import { describe, expect, it } from "vitest";
import { usageColor, usagePace } from "./usage";
const now = Date.parse("2026-10-15T08:00:00Z");
describe("usage pace", () => {
  it("projects at half a session and half a week", () => {
    for (const [window, duration] of [["5-hour", 18_000_000], ["Weekly", 604_800_000]] as const) {
      expect(usagePace({ window, percent: 60, resetsAt: new Date(now + duration / 2).toISOString() }, now)).toEqual({ elapsed: 0.5, projected: 120 });
    }
  });
  it("does not fabricate a marker for absent, expired or unsupported windows", () => {
    for (const quota of [{ window: "Monthly", percent: 50, resetsAt: new Date(now+1000).toISOString() }, { window: "5-hour", percent: 50 }, { window: "5-hour", percent: 50, resetsAt: new Date(now).toISOString() }]) expect(usagePace(quota,now)).toBeNull();
  });
  it("keeps status based on consumed capacity and supports projected pace", () => {
    expect(usageColor(10)).toBe("#30b86b");
    expect(usageColor(95)).toBe("#ff453a");
    expect(usageColor(30,150)).toBe("#bf5af2");
  });
});
