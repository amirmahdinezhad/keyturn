import type { ReactNode } from "react";
import { Modal } from "@heroui/react";
import { CloseIcon } from "../icons";

interface AppModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  /** Left-aligned header inside the body: which key/account this is about. */
  detail?: ReactNode;
  cardClass?: string;
  bodyId?: string;
  children: ReactNode;
}

/** HeroUI v3 modal mapped onto the Electron modal-card look. */
export function AppModal({ open, onClose, title, detail, cardClass, bodyId, children }: AppModalProps) {
  // State lives on Modal.Backdrop rather than Modal.Root: Root wraps its
  // children in a DialogTrigger, whose PressResponder expects a pressable
  // trigger child. These modals open from state, never from a trigger, so that
  // childless PressResponder only produced a React Aria warning.
  return (
    <Modal.Backdrop
      className="ak-backdrop"
      isOpen={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      <Modal.Container className="ak-container" placement="center">
        <Modal.Dialog className={`card ak-dialog${cardClass ? ` ${cardClass}` : ""}`}>
          <Modal.Header className="modal-head">
            <Modal.Heading className="ak-heading">{title}</Modal.Heading>
            <Modal.CloseTrigger className="icon" aria-label="Close">
              <CloseIcon size={18} />
            </Modal.CloseTrigger>
          </Modal.Header>
          <Modal.Body id={bodyId}>
            {detail ? <div className="modal-detail">{detail}</div> : null}
            {children}
          </Modal.Body>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
