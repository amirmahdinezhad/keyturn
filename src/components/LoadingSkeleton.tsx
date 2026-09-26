import { Skeleton, Spinner } from "@heroui/react";

/** Shown until the first backend state arrives. */
export function LoadingSkeleton() {
  return (
    <div className="loading" role="status" aria-label="Loading">
      <Spinner size="lg" className="loading-spinner" aria-label="Loading keys" />
      <Skeleton className="skel" />
      <Skeleton className="skel" />
      <Skeleton className="skel" />
    </div>
  );
}
