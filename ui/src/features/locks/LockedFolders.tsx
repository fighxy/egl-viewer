import { ChevronDown, ChevronRight, Lock, LockOpen } from "lucide-react";
import { useState } from "react";
import { FOLDER_COLORS } from "@/features/folders/tree";
import type { Folder } from "@/ipc";

/** Every locked folder of the tree, in tree order. */
export function lockedFolders(tree: readonly Folder[]): Folder[] {
  return tree.flatMap((folder) => [
    ...(folder.isLocked ? [folder] : []),
    ...lockedFolders(folder.children),
  ]);
}

type Props = {
  folders: Folder[];
  /** The open folder. */
  selectedId: string | null;
  /** A locked row was chosen: ask for that folder's password. */
  onUnlock: (id: string) => void;
  /** An unlocked row was chosen: open the folder. */
  onOpen: (id: string) => void;
};

/**
 * The Locked folders section of the left bar (docs/spec.md §7.3, D-018): each locked folder
 * with its name and its tag (a label and a colour). Choosing a locked row asks for that
 * folder's password only.
 */
export function LockedFolders({ folders, selectedId, onUnlock, onOpen }: Props) {
  const [open, setOpen] = useState(true);
  const locked = lockedFolders(folders);
  if (locked.length === 0) return null;
  return (
    <section aria-label="Locked folders" className="grid gap-px">
      <button
        type="button"
        aria-expanded={open}
        className="flex h-7 items-center gap-1 rounded-md px-1 text-left text-xs font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
        onClick={() => setOpen(!open)}
      >
        {open ? (
          <ChevronDown aria-hidden className="size-3.5" />
        ) : (
          <ChevronRight aria-hidden className="size-3.5" />
        )}
        Locked folders
      </button>
      {open && (
        <ul className="grid gap-px">
          {locked.map((folder) => {
            const Icon = folder.isUnlocked ? LockOpen : Lock;
            const color = folder.lockColor ? FOLDER_COLORS[folder.lockColor] : undefined;
            return (
              <li key={folder.id}>
                <button
                  type="button"
                  aria-current={folder.id === selectedId ? "page" : undefined}
                  title={folder.isUnlocked ? "Open the folder" : "Unlock the folder"}
                  className="flex h-7 w-full items-center gap-2 rounded-md px-2 text-left text-[13px] outline-none hover:bg-sidebar-accent focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:bg-sidebar-accent aria-[current=page]:font-medium"
                  onClick={() => (folder.isUnlocked ? onOpen(folder.id) : onUnlock(folder.id))}
                >
                  <Icon
                    aria-label={folder.isUnlocked ? "Unlocked" : "Locked"}
                    className="size-4 shrink-0 text-muted-foreground"
                  />
                  <span className="min-w-0 flex-1 truncate">{folder.name}</span>
                  {folder.lockLabel && (
                    <span
                      className="flex max-w-24 items-center gap-1 rounded bg-muted px-1.5 text-xs text-muted-foreground"
                      title={folder.lockLabel}
                    >
                      {color && (
                        <span
                          aria-hidden
                          className="size-2 shrink-0 rounded-full"
                          style={{ background: color }}
                        />
                      )}
                      <span className="truncate">{folder.lockLabel}</span>
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
