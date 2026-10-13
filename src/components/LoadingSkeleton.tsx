import { Skeleton } from "./ui/skeleton";
import { SpinnerIcon } from "../icons";

/** Shown until the first backend state arrives. */
export function LoadingSkeleton() {
  return (
    <div className="loading" role="status" aria-label="Loading">
      <SpinnerIcon size={44} className="loading-spinner ak-spin" />
      <Skeleton className="skel" />
      <Skeleton className="skel" />
      <Skeleton className="skel" />
    </div>
  );
}
