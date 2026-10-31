import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";

export type Confirmation = {
  title: string;
  description: string;
  confirm: string;
  cancel: string;
  /** A "don't ask again" checkbox with a hint where to change the choice later. */
  dontAskAgain?: { label: string; hint: string };
  /** Receives the checkbox state; confirmations without the checkbox get `false`. */
  onConfirm: (dontAskAgain: boolean) => void;
};

/** Asks before an irreversible or interrupting action; only the trash dialog can opt out for good. */
export function ConfirmDialog({
  confirmation,
  onClose,
}: {
  confirmation: Confirmation | null;
  onClose: () => void;
}) {
  const [dontAskAgain, setDontAskAgain] = useState(false);
  // Bumps when a new confirmation opens so a late onOpenChange from the previous
  // dialog cannot clear the next one (Delete permanently → Empty Trash).
  const generation = useRef(0);
  useEffect(() => {
    if (confirmation) {
      generation.current += 1;
      setDontAskAgain(false);
    }
  }, [confirmation]);

  // Remount when the prompt changes so a late close from the previous dialog cannot
  // clear the next one.
  const dialogKey = confirmation?.title ?? "closed";

  return (
    <Dialog
      key={dialogKey}
      open={confirmation !== null}
      onOpenChange={(open) => {
        if (open) return;
        const atClose = generation.current;
        queueMicrotask(() => {
          if (generation.current === atClose) onClose();
        });
      }}
    >
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{confirmation?.title}</DialogTitle>
          <DialogDescription>{confirmation?.description}</DialogDescription>
        </DialogHeader>
        {confirmation?.dontAskAgain && (
          <div className="grid gap-1">
            <div className="flex items-center gap-2">
              <Checkbox
                id="dont-ask-again"
                checked={dontAskAgain}
                onCheckedChange={(checked) => setDontAskAgain(checked === true)}
              />
              <Label htmlFor="dont-ask-again" className="font-normal">
                {confirmation.dontAskAgain.label}
              </Label>
            </div>
            <p className="text-xs text-muted-foreground">{confirmation.dontAskAgain.hint}</p>
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" autoFocus onClick={onClose}>
            {confirmation?.cancel}
          </Button>
          <Button
            variant="destructive"
            onClick={() => {
              const action = confirmation?.onConfirm;
              onClose();
              action?.(dontAskAgain);
            }}
          >
            {confirmation?.confirm}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
