import { Skeleton } from "./ui/skeleton";

/** Shown until the first backend state arrives. */
export function LoadingSkeleton({ variant = "list", count = 3 }: { variant?: "list" | "cards"; count?: number }) {
  if (variant === "cards") return <div className="account-card-skeletons" role="status" aria-label="Loading accounts">
    <span className="sr-only">Loading accounts…</span>
    {Array.from({ length: count }, (_, row) => <div className="account-card-skeleton" aria-hidden="true" key={row}>
      <div className="account-card-skeleton-row"><div><Skeleton className="account-skeleton-name" /><Skeleton className="account-skeleton-email" /></div><Skeleton className="account-card-skeleton-actions" /></div>
      <Skeleton className="account-card-skeleton-reset" />
    </div>)}
  </div>;
  return (
    <div className="account-skeletons" role="status" aria-label="Loading">
      <span className="sr-only">Loading accounts…</span>
      {[0, 1, 2].map((row) => (
        <div className="account-skeleton" key={row} aria-hidden="true">
          <Skeleton className="account-skeleton-mark" />
          <div><Skeleton className="account-skeleton-name" /><Skeleton className="account-skeleton-email" /></div>
        </div>
      ))}
    </div>
  );
}
