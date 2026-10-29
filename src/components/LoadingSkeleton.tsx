import { Skeleton } from "./ui/skeleton";

/** Shown until the first backend state arrives. */
export function LoadingSkeleton() {
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
