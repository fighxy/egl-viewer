import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { findFolder } from "@/features/folders/tree";
import type { DuplicateAnswer, Folder, ImportProgress } from "@/ipc";
import { fileName } from "./importText";

type Props = {
  /** The import whose first duplicate waits; the dialog is closed without one. */
  waiting: ImportProgress | undefined;
  /** For the name of the folder the files go into. */
  folders?: Folder[];
  onAnswer: (id: number, answer: DuplicateAnswer) => void;
};

/**
 * The batch asks once what to do with duplicates (docs/spec.md §4). Files going into a folder
 * can instead put the existing items into that folder.
 */
export function DuplicateDialog({ waiting, folders = [], onAnswer }: Props) {
  const duplicate = waiting?.duplicate;
  const target = duplicate?.targetFolder ? findFolder(folders, duplicate.targetFolder) : undefined;
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
            {target && ` “Add to ${target.name}” puts the existing items into that folder.`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => answer("cancel")}>
            Cancel import
          </Button>
          <Button variant="outline" onClick={() => answer("addAnyway")}>
            Add anyway
          </Button>
          <Button
            variant={target ? "outline" : "default"}
            autoFocus={!target}
            onClick={() => answer("skip")}
          >
            Skip duplicates
          </Button>
          {target && (
            <Button autoFocus onClick={() => answer("addToFolder")}>
              Add to {target.name}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
