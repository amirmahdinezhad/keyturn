import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  exactDecimal,
  exactPercent,
  formatClock,
  formatTokens,
  hashHue,
  mask,
  modelLogoKey,
  money,
  moneyPrecise,
  monthLabel,
  paceInfo,
  paceLevel,
  plural,
  prettyProvider,
  quotaLevel,
  resetText,
  resetsIn,
} from "./format";

// A fixed instant so every relative-time assertion is deterministic.
const NOW = Date.parse("2026-09-23T12:00:00.000Z");
const at = (offsetSeconds: number) => new Date(NOW + offsetSeconds * 1000).toISOString();

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("mask", () => {
  it("keeps the head and tail of a long key", () => {
    expect(mask("sk-abcdefghijklmnop")).toBe("sk-ab…mnop");
  });

  it("hides keys that are too short to mask usefully", () => {
    expect(mask("short")).toBe("•••");
    expect(mask("0123456789")).toBe("•••");
  });
});

describe("exactDecimal", () => {
  it("drops float-noise tails", () => {
    expect(exactDecimal(37.4)).toBe("37.4");
    expect(exactDecimal(26.86)).toBe("26.86");
    expect(exactDecimal(0.0725)).toBe("0.0725");
  });

  it("caps at four decimal places", () => {
    expect(exactDecimal(1.234567)).toBe("1.2346");
  });

  it("strips trailing zeros and the bare point", () => {
    expect(exactDecimal(5)).toBe("5");
    expect(exactDecimal(5.0)).toBe("5");
  });

  it("is safe on non-finite input", () => {
    expect(exactDecimal(Number.NaN)).toBe("0");
    expect(exactDecimal(Number.POSITIVE_INFINITY)).toBe("0");
  });
});

describe("money", () => {
  it("prefixes a dollar sign", () => {
    expect(money(12.5)).toBe("$12.5");
  });

  it("collapses zero and negatives to a plain $0", () => {
    expect(moneyPrecise(0)).toBe("$0");
    expect(moneyPrecise(-1)).toBe("$0");
    expect(moneyPrecise(9.99)).toBe("$9.99");
  });
});

describe("exactPercent", () => {
  it("keeps real decimals", () => {
    expect(exactPercent(37.4)).toBe("37.4");
    expect(exactPercent(100)).toBe("100");
  });
});

describe("plural", () => {
  it("singularises one only", () => {
    expect(plural(1, "hour")).toBe("1 hour");
    expect(plural(0, "hour")).toBe("0 hours");
    expect(plural(2, "day")).toBe("2 days");
  });
});

describe("resetsIn", () => {
  it("is empty without a timestamp", () => {
    expect(resetsIn(null)).toBe("");
    expect(resetsIn("not-a-date")).toBe("");
  });

  it("reads now for an elapsed reset", () => {
    expect(resetsIn(at(-1))).toBe("now");
  });

  it("counts days and hours past a day", () => {
    expect(resetsIn(at(5 * 86400 + 14 * 3600))).toBe("5 days 14 hours");
  });

  it("counts hours and minutes under a day", () => {
    expect(resetsIn(at(4 * 3600 + 58 * 60))).toBe("4 hours 58 minutes");
  });

  it("falls back to minutes under an hour", () => {
    expect(resetsIn(at(59))).toBe("0 minutes");
    expect(resetsIn(at(60))).toBe("1 minute");
    expect(resetsIn(at(2 * 60))).toBe("2 minutes");
  });
});

describe("formatClock", () => {
  it("is empty for an invalid date", () => {
    expect(formatClock("nope", "24h")).toBe("");
  });

  it("uses a 24-hour clock without a meridiem", () => {
    expect(formatClock(at(0), "24h")).toMatch(/^\d{2}:\d{2}$/);
  });

  it("uses a meridiem on the 12-hour clocks", () => {
    expect(formatClock(at(0), "12h")).toMatch(/(AM|PM)/);
  });
});

describe("resetText", () => {
  it("speaks in clocks, remaining, or both", () => {
    expect(resetText(at(3600), "clock", "24h")).toMatch(/^Resets at \d{2}:\d{2}$/);
    expect(resetText(at(3600), "remaining", "24h")).toBe("Resets in 1 hour 0 minutes");
    expect(resetText(at(3600), "both", "24h")).toMatch(/^Resets at \d{2}:\d{2} · 1 hour 0 minutes$/);
  });

  it("is empty without a timestamp", () => {
    expect(resetText(null, "both", "24h")).toBe("");
  });
});

describe("formatTokens", () => {
  it("scales into K and M", () => {
    expect(formatTokens(500)).toBe("500");
    expect(formatTokens(1500)).toBe("2K");
    expect(formatTokens(1_500_000)).toBe("1.5M");
    expect(formatTokens(25_000_000)).toBe("25M");
  });
});

describe("quotaLevel", () => {
  it("splits at 70 and 90", () => {
    expect(quotaLevel(0)).toBe("ok");
    expect(quotaLevel(69.9)).toBe("ok");
    expect(quotaLevel(70)).toBe("warm");
    expect(quotaLevel(89.9)).toBe("warm");
    expect(quotaLevel(90)).toBe("hot");
    expect(quotaLevel(130)).toBe("hot");
  });
});

describe("paceLevel", () => {
  it("maps projections onto the six levels", () => {
    expect(paceLevel(49)).toBe("comfortable");
    expect(paceLevel(50)).toBe("light");
    expect(paceLevel(70)).toBe("steady");
    expect(paceLevel(90)).toBe("elevated");
    expect(paceLevel(100)).toBe("critical");
    expect(paceLevel(120)).toBe("runaway");
  });
});

describe("paceInfo", () => {
  it("is null without a window duration or reset time", () => {
    expect(paceInfo(50, at(3600), "unknown")).toBeNull();
    expect(paceInfo(50, null, "rolling")).toBeNull();
    expect(paceInfo(50, "nope", "rolling")).toBeNull();
  });

  it("is null when the window has not started or is already over", () => {
    // resetting further out than the window is long: impossible, ignore it.
    expect(paceInfo(50, at(6 * 3600), "rolling")).toBeNull();
    expect(paceInfo(50, at(0), "rolling")).toBeNull();
  });

  it("is null in the opening 5% of a window", () => {
    expect(paceInfo(50, at(17999), "rolling")).toBeNull();
  });

  it("extrapolates usage to the end of the window", () => {
    // rolling = 5h; halfway through, so 40% used projects to 80%.
    const info = paceInfo(40, at(9000), "rolling");
    expect(info).not.toBeNull();
    expect(info!.projected).toBeCloseTo(80, 5);
    expect(info!.level).toBe("steady");
    expect(info!.label).toBe("Steady");
    expect(info!.marker).toBeCloseTo(80, 5);
  });

  it("never projects below the usage already recorded", () => {
    // One minute left of the window: the projection settles onto the used value.
    const info = paceInfo(95, at(60), "rolling");
    expect(info!.projected).toBeGreaterThanOrEqual(95);
    expect(info!.projected).toBeLessThan(96);
    expect(info!.level).toBe("elevated");
  });

  it("clamps the bar marker at 130%", () => {
    const info = paceInfo(100, at(17099), "rolling");
    expect(info!.marker).toBe(130);
    expect(info!.level).toBe("runaway");
  });
});

describe("prettyProvider", () => {
  it("matches the two OpenCode brands exactly", () => {
    expect(prettyProvider("opencode")).toBe("OpenCode");
    expect(prettyProvider("OpenCode")).toBe("OpenCode");
    expect(prettyProvider("opencode-go")).toBe("OpenCode Go");
    expect(prettyProvider("opencode_go")).toBe("OpenCode Go");
  });

  it("title-cases unknown ids across separators", () => {
    expect(prettyProvider("my-custom_provider")).toBe("My Custom Provider");
  });
});

describe("monthLabel", () => {
  it("renders month and year", () => {
    expect(monthLabel({ year: 2026, month: 8 })).toBe("September 2026");
    expect(monthLabel({ year: 2026, month: 0 })).toBe("January 2026");
  });
});

describe("modelLogoKey", () => {
  it("matches each brand's own pattern", () => {
    expect(modelLogoKey("moonshot-v1")).toBe("kimi");
    expect(modelLogoKey("glm-4")).toBe("zhipu");
    expect(modelLogoKey("deepseek-chat")).toBe("deepseek");
    expect(modelLogoKey("gpt-4o")).toBe("openai");
    expect(modelLogoKey("o3-mini")).toBe("openai");
    expect(modelLogoKey("claude-sonnet-4")).toBe("claude");
    expect(modelLogoKey("qwen-max")).toBe("qwen");
    expect(modelLogoKey("gemini-2.5-pro")).toBe("gemini");
    expect(modelLogoKey("grok-3")).toBe("grok");
    expect(modelLogoKey("mistral-large")).toBe("mistral");
    expect(modelLogoKey("minimax-01")).toBe("minimax");
  });

  it("returns null for unrecognised models", () => {
    expect(modelLogoKey("llama-3")).toBeNull();
  });
});

describe("hashHue", () => {
  it("is deterministic and inside the colour wheel", () => {
    expect(hashHue("opencode")).toBe(hashHue("opencode"));
    expect(hashHue("opencode")).toBeGreaterThanOrEqual(0);
    expect(hashHue("opencode")).toBeLessThan(360);
  });
});
