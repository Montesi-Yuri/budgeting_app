import { useCallback } from "react";
import TransactionForm from "./TransactionForm";

/** Add or edit a Transaction in a modal dialog; Escape or Cancel closes it. */
export default function TransactionDialog(props: React.ComponentProps<typeof TransactionForm>) {
  const ref = useCallback((dialog: HTMLDialogElement | null) => {
    if (dialog && !dialog.open) dialog.showModal();
  }, []);
  return (
    <dialog
      ref={ref}
      className="modal"
      onCancel={(e) => {
        e.preventDefault();
        props.onCancel();
      }}
    >
      <TransactionForm {...props} />
    </dialog>
  );
}
