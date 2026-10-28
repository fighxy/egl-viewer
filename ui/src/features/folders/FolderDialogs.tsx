import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { plural } from "@/features/imports/importText";
import type { Folder } from "@/ipc";
import { FolderPicker } from "./FolderPicker";
import { findFolder, subtreeIds } from "./tree";

/** Which folder dialog is open. */
export type FolderDialog =
  | { kind: "create"; parent: string | null }
  | { kind: "move"; id: string }
  | { kind: "delete"; id: string };

type Props = {
  dialog: FolderDialog | null;
  folders: Folder[];
  onClose: () => void;
  onCreate: (name: string, parent: string | null) => void;
  onMove: (id: string, parent: string | null) => void;
  onDelete: (id: string, trashItems: boolean) => void;
};

/** New folder, Move To and Delete (docs/spec.md §7.4, item 4). */
export function FolderDialogs({ dialog, folders, onClose, onCreate, onMove, onDelete }: Props) {
  const close = (open: boolean) => {
    if (!open) onClose();
  };
  return (
    <>
      <Dialog open={dialog?.kind === "create"} onOpenChange={close}>
        {dialog?.kind === "create" && (
          <CreateFolder
            parent={dialog.parent ? findFolder(folders, dialog.parent) : undefined}
            onCreate={(name) => onCreate(name, dialog.parent)}
          />
        )}
      </Dialog>
      <Dialog open={dialog?.kind === "move"} onOpenChange={close}>
        {dialog?.kind === "move" && (
          <MoveFolder
            folders={folders}
            id={dialog.id}
            onMove={(parent) => onMove(dialog.id, parent)}
          />
        )}
      </Dialog>
      <Dialog open={dialog?.kind === "delete"} onOpenChange={close}>
        {dialog?.kind === "delete" && (
          <DeleteFolder
            folder={findFolder(folders, dialog.id)}
            onCancel={onClose}
            onDelete={(trashItems) => onDelete(dialog.id, trashItems)}
          />
        )}
      </Dialog>
    </>
  );
}

function CreateFolder({ parent, onCreate }: { parent?: Folder; onCreate: (name: string) => void }) {
  const [name, setName] = useState("");
  return (
    <DialogContent>
      <form
        className="grid gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (name.trim()) onCreate(name.trim());
        }}
      >
        <DialogHeader>
          <DialogTitle>{parent ? "New subfolder" : "New folder"}</DialogTitle>
          <DialogDescription>
            {parent ? `Inside “${parent.name}”.` : "At the top of the folder tree."}
          </DialogDescription>
        </DialogHeader>
        <input
          aria-label="Name"
          // biome-ignore lint/a11y/noAutofocus: the dialog asks for this one field.
          autoFocus
          value={name}
          onChange={(event) => setName(event.target.value)}
          className="h-8 rounded-md border bg-background px-2 text-[13px] outline-none focus:ring-2 focus:ring-ring/50"
        />
        <DialogFooter>
          <Button type="submit" disabled={!name.trim()}>
            Create
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  );
}

function MoveFolder({
  folders,
  id,
  onMove,
}: {
  folders: Folder[];
  id: string;
  onMove: (parent: string | null) => void;
}) {
  const folder = findFolder(folders, id);
  // The cycle guard: not into itself or its own subfolders.
  const disabled = folder ? subtreeIds(folder) : new Set<string>();
  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Move “{folder?.name}”</DialogTitle>
        <DialogDescription>Choose the folder it goes into.</DialogDescription>
      </DialogHeader>
      <FolderPicker
        label="Folders"
        folders={folders}
        disabled={disabled}
        topLabel="Top level"
        onPickTop={() => onMove(null)}
        onPick={onMove}
      />
    </DialogContent>
  );
}

function DeleteFolder({
  folder,
  onCancel,
  onDelete,
}: {
  folder?: Folder;
  onCancel: () => void;
  onDelete: (trashItems: boolean) => void;
}) {
  const [trashItems, setTrashItems] = useState(false);
  if (!folder) return null;
  const subfolders = subtreeIds(folder).size - 1;
  const items = folder.descendantImageCount ?? 0;
  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Delete “{folder.name}”?</DialogTitle>
        <DialogDescription>
          {subfolders === 1 ? "Its subfolder goes too. " : ""}
          {subfolders > 1 ? `Its ${subfolders} subfolders go too. ` : ""}
          {items > 0
            ? `${plural(items, "item")} ${items === 1 ? "is" : "are"} in it; they stay in the library.`
            : "It has no items."}
        </DialogDescription>
      </DialogHeader>
      {items > 0 && (
        <label className="flex items-center gap-2 text-[13px]">
          <input
            type="checkbox"
            checked={trashItems}
            onChange={(event) => setTrashItems(event.target.checked)}
          />
          Also move items that are only in these folders to the trash
        </label>
      )}
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button variant="destructive" onClick={() => onDelete(trashItems)}>
          Delete
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
