import { describe, expect, it } from "vitest";
import {
  hashHue,
  modelLogoKey,
} from "./format";

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
