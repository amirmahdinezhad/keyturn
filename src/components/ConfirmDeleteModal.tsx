import { Button } from "@heroui/react";
import { deleteKey, prettyError } from "../api";
import type { AppState } from "../types";
import { prettyProvider } from "../format";
import { AppModal } from "./AppModal";
import type { ToastKind } from "./Toast";

export interface ConfirmTarget {
  provider: string;
  name: string;
}

interface ConfirmDeleteModalProps {
  target: ConfirmTarget | null;
  onClose: () => void;
  onDeleted: (s: AppState) => void;
  notify: (message: string, kind?: ToastKind) => void;
}

/** In-app confirmation — names both the key and the provider. */
export function ConfirmDeleteModal({ target, onClose, onDeleted, notify }: ConfirmDeleteModalProps) {
  const remove = async () => {
    if (!target) return;
    onClose();
    try {
      onDeleted(await deleteKey(target.provider, target.name));
      notify("Key deleted");
    } catch (e) {
      notify(prettyError(e), "err");
    }
  };

  return (
    <AppModal open={target !== null} onClose={onClose} title="Delete key" cardClass="confirm-card">
      <p className="confirm-ask">
        Are you sure you want to delete <b>{target ? `"${target.name}"` : ""}</b> token for{" "}
        <b>{target ? prettyProvider(target.provider) : ""}</b>?
      </p>
      <div className="confirm-actions">
        <Button onPress={onClose}>Cancel</Button>
        <Button className="danger" onPress={() => void remove()}>
          Delete
        </Button>
      </div>
    </AppModal>
  );
}
