import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
  const duplicate = waiting?.duplicate;
  const target = duplicate?.targetFolder ? findFolder(folders, duplicate.targetFolder) : undefined;
  const answer = (choice: DuplicateAnswer) => {
    if (waiting) onAnswer(waiting.id, choice);
  };
  const name = duplicate ? fileName(duplicate.path) : "";
  return (
    <Dialog open={duplicate !== undefined && duplicate !== null}>
      <DialogContent
        showCloseButton={false}
        onEscapeKeyDown={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>{t("imports.dupeTitle")}</DialogTitle>
          <DialogDescription>
            {duplicate
              ? `${
                  duplicate.inTrash
                    ? t("imports.dupeAlreadyInTrash", { name })
                    : t("imports.dupeAlready", { name })
                } `
              : ""}
            {t("imports.dupeApplies")}
            {target && ` ${t("imports.dupeFolderHint", { name: target.name })}`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => answer("cancel")}>
            {t("imports.cancelImport")}
          </Button>
          <Button variant="outline" onClick={() => answer("addAnyway")}>
            {t("imports.addAnyway")}
          </Button>
          <Button
            variant={target ? "outline" : "default"}
            autoFocus={!target}
            onClick={() => answer("skip")}
          >
            {t("imports.skip")}
          </Button>
          {target && (
            <Button autoFocus onClick={() => answer("addToFolder")}>
              {t("imports.addToFolder", { name: target.name })}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
