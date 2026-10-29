/** Display metadata only; credential/token expiry is never subscription expiry. */
export interface Subscription {
  id: string;
  service: string;
  plan?: string | null;
  monthlyPriceUsd?: number | null;
  expiresAt?: string | null;
  status?: "active" | "expired" | "unknown" | null;
}

export type PlanTier = "neutral" | "starter" | "plus" | "pro" | "max" | "ultra" | "business";

const PLANS: Record<string, Record<string, [string, number | null, PlanTier]>> = {
  chatgpt: {
    free: ["Free", 0, "neutral"], go: ["Go", null, "starter"], plus: ["Plus", 20, "plus"],
    prolite: ["Pro", 100, "pro"], pro_lite: ["Pro", 100, "pro"], pro_100: ["Pro", 100, "pro"],
    pro: ["Pro", 200, "max"], pro_200: ["Pro", 200, "max"], pro_500: ["Pro", 500, "ultra"],
    team: ["Team", null, "business"], business: ["Business", null, "business"],
    enterprise: ["Enterprise", null, "business"], edu: ["Edu", null, "business"],
  },
  claude: {
    free: ["Free", 0, "neutral"], pro: ["Pro", 20, "plus"], max: ["Max", null, "pro"],
    max_5x: ["Max 5×", 100, "pro"], max_20x: ["Max 20×", 200, "max"],
    team: ["Team", null, "business"], enterprise: ["Enterprise", null, "business"],
  },
  "opencode-go": { go: ["Go", 10, "starter"], go_plus: ["Go Plus", 40, "plus"] },
};

/** Hide only confirmed ended subscriptions; malformed/missing dates stay visible. */
export function isSubscriptionExpired(subscription: Subscription | null | undefined, now = Date.now()): boolean {
  if (!subscription) return false;
  if (subscription.status === "expired") return true;
  const end = subscription.expiresAt ? Date.parse(subscription.expiresAt) : NaN;
  return Number.isFinite(end) && end <= now;
}

/** Known plans get a consistent value tier; new plans preserve the provider's label. */
export function describePlan(subscription: Subscription): { label: string; price: number | null; tier: PlanTier } {
  const raw = subscription.plan?.trim() || undefined;
  const key = raw?.toLowerCase().replace(/[\s-]+/g, "_") ?? "";
  const known = PLANS[subscription.id]?.[key];
  const supplied = subscription.monthlyPriceUsd;
  const price = supplied != null && Number.isFinite(supplied) && supplied >= 0 ? supplied : known?.[1] ?? null;
  const tier = price == null ? known?.[2] ?? "neutral"
    : price >= 500 ? "ultra" : price >= 200 ? "max" : price >= 100 ? "pro"
    : price >= 20 ? "plus" : price > 0 ? "starter" : "neutral";
  return { label: known?.[0] ?? raw ?? "Plan unavailable", price, tier };
}

/** Older status payloads still supply plan names through `account`. */
export function accountSubscription(id: string, method: string | null | undefined, account?: string | null): Subscription | null {
  if (id === "codex" && method === "ChatGPT") {
    const key = account?.toLowerCase().replace(/[\s-]+/g, "_") ?? "";
    return { id: "chatgpt", service: "ChatGPT", plan: PLANS.chatgpt[key] ? account : null };
  }
  if (id === "claude" && method === "claude.ai" && account && PLANS.claude[account.toLowerCase()]) {
    return { id: "claude", service: "Claude", plan: account };
  }
  return null;
}
