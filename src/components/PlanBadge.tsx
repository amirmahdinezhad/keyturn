import { describePlan, type Subscription } from "../subscriptions";

/** Shared plan treatment for account cards and saved profiles. */
export function PlanBadge({ subscription }: { subscription: Subscription }) {
  const { label, tier } = describePlan(subscription);
  return (
    <span className="plan-badge" data-tier={tier}>
      {label}
    </span>
  );
}
