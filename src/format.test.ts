import { describe, expect, it } from "vitest";
import {
  exactDecimal,
  exactPercent,
  formatTokens,
  hashHue,
  modelLogoKey,
  money,
  moneyPrecise,
} from "./format";

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

describe("formatTokens", () => {
  it("scales into K and M", () => {
    expect(formatTokens(500)).toBe("500");
    expect(formatTokens(1500)).toBe("2K");
    expect(formatTokens(1_500_000)).toBe("1.5M");
    expect(formatTokens(25_000_000)).toBe("25M");
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
