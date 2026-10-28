import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { DuplicateAnswer, ImportProgress } from "@/ipc";
import { fileName } from "./importText";

type Props = {
  /** The import whose first duplicate waits; the dialog is closed without one. */
  waiting: ImportProgress | undefined;
  onAnswer: (id: number, answer: DuplicateAnswer) => void;
};

/** The batch asks once what to do with duplicates (docs/spec.md §4). */
export function DuplicateDialog({ waiting, onAnswer }: Props) {
  const duplicate = waiting?.duplicate;
  const answer = (choice: DuplicateAnswer) => {
    if (waiting) onAnswer(waiting.id, choice);
  };
  return (
    <Dialog open={duplicate !== undefined && duplicate !== null}>
      <DialogContent
        showCloseButton={false}
        onEscapeKeyDown={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>Some files are already in this library</DialogTitle>
          <DialogDescription>
            {duplicate
              ? `“${fileName(duplicate.path)}” is already in the library${duplicate.inTrash ? ", in the trash" : ""}. `
              : ""}
            Your choice applies to every duplicate in this batch.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => answer("cancel")}>
            Cancel import
          </Button>
          <Button variant="outline" onClick={() => answer("addAnyway")}>
            Add anyway
          </Button>
          <Button autoFocus onClick={() => answer("skip")}>
            Skip duplicates
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
