// Pure formatting helpers for brand marks and avatar hues.

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
