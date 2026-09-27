// Pure formatting helpers for costs, tokens and brand marks.

/** Exact decimal text: up to 4 places, no rounding below that, and no
 * float-noise tails (37.4 stays "37.4", 0.0725 stays "0.0725"). */
export function exactDecimal(v: number): string {
  if (!Number.isFinite(v)) return "0";
  // Cap at 4 decimal places, then strip trailing zeros so 26.8600 → "26.86".
  return v
    .toFixed(4)
    .replace(/(\.\d*?)0+$/, "$1")
    .replace(/\.$/, "");
}

/** Percent with its real decimals (37.4% never becomes 37%). */
export function exactPercent(v: number): string {
  return exactDecimal(v);
}

export function money(v: number): string {
  return `$${exactDecimal(v)}`;
}

export function moneyPrecise(v: number): string {
  if (v <= 0) return "$0";
  return money(v);
}

export function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) {
    const value = tokens / 1_000_000;
    return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)}M`;
  }
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(0)}K`;
  return `${Math.round(tokens)}`;
}

export type ModelMarkKey =
  | "kimi"
  | "zhipu"
  | "deepseek"
  | "openai"
  | "claude"
  | "qwen"
  | "gemini"
  | "grok"
  | "mistral"
  | "minimax";

// Brand matchers — same order and patterns as the Electron modelLogo().
export function modelLogoKey(model: string): ModelMarkKey | null {
  const lower = model.toLowerCase();
  if (/kimi|moonshot/.test(lower)) return "kimi";
  if (/glm|zhipu/.test(lower)) return "zhipu";
  if (/deepseek/.test(lower)) return "deepseek";
  if (/gpt|openai|^o\d/.test(lower)) return "openai";
  if (/claude/.test(lower)) return "claude";
  if (/qwen|tongyi/.test(lower)) return "qwen";
  if (/gemini/.test(lower)) return "gemini";
  if (/grok/.test(lower)) return "grok";
  if (/mistral/.test(lower)) return "mistral";
  if (/minimax/.test(lower)) return "minimax";
  return null;
}

/** Letter-avatar background so the same id always gets the same hue. */
export function hashHue(s: string): number {
  let hash = 0;
  for (const ch of s) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return hash % 360;
}
