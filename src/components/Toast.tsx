export type ToastKind = "" | "ok" | "err";

export interface ToastData {
  message: string;
  kind: ToastKind;
}

/** Bottom-center toast — same show/ok/err classes and 2200ms timing as Electron. */
export function Toast({ toast }: { toast: ToastData | null }) {
  return (
    <div className={`ak-toast${toast ? ` show ${toast.kind}`.trimEnd() : ""}`} aria-live="polite">
      {toast?.message ?? ""}
    </div>
  );
}
