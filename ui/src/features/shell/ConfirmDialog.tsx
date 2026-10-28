import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export type Confirmation = {
  title: string;
  description: string;
  confirm: string;
  cancel: string;
  onConfirm: () => void;
};

/** Asks before an action that would stop running imports. */
export function ConfirmDialog({
  confirmation,
  onClose,
}: {
  confirmation: Confirmation | null;
  onClose: () => void;
}) {
  return (
    <Dialog open={confirmation !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{confirmation?.title}</DialogTitle>
          <DialogDescription>{confirmation?.description}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" autoFocus onClick={onClose}>
            {confirmation?.cancel}
          </Button>
          <Button
            variant="destructive"
            onClick={() => {
              const action = confirmation?.onConfirm;
              onClose();
              action?.();
            }}
          >
            {confirmation?.confirm}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
