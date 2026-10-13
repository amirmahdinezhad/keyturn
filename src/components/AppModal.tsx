import { useRef, type ReactNode } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "./ui/dialog";
import { Button } from "./ui/button";
import { CloseIcon } from "../icons";

interface AppModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  /** Centered header inside the body: which key/account this is about. */
  detail?: ReactNode;
  /** Fixed controls between the header and the scrollable body. */
  navigation?: ReactNode;
  cardClass?: string;
  bodyId?: string;
  /** When set, a back chevron renders top-left in the header row. */
  onBack?: () => void;
  backLabel?: string;
  children: ReactNode;
}

/** Shadcn dialog with fixed navigation and a scrollable panel body. */
export function AppModal({
  open,
  onClose,
  title,
  detail,
  navigation,
  cardClass,
  bodyId,
  onBack,
  backLabel = "Back",
  children,
}: AppModalProps) {
  const returnFocus = useRef<HTMLElement | null>(null);
  return (
    <Dialog
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      <DialogContent
        className={`card${cardClass ? ` ${cardClass}` : ""}`}
        aria-describedby={undefined}
        onOpenAutoFocus={() => {
          returnFocus.current =
            document.activeElement instanceof HTMLElement
              ? document.activeElement
              : null;
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          returnFocus.current?.focus();
        }}
      >
        <div className="modal-head">
          {onBack ? (
            <Button
              className="modal-back"
              onClick={onBack}
              aria-label={backLabel}
            >
              <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path d="M19 12H5" />
                <path d="m12 19-7-7 7-7" />
              </svg>
            </Button>
          ) : null}
          <DialogTitle>{title}</DialogTitle>
          <DialogClose asChild>
            <Button size="icon-sm" className="icon" aria-label="Close">
              <CloseIcon size={20} />
            </Button>
          </DialogClose>
        </div>
        {navigation}
        <div id={bodyId} className="app-modal-body">
          {detail ? <div className="modal-detail">{detail}</div> : null}
          {children}
        </div>
      </DialogContent>
    </Dialog>
  );
}
