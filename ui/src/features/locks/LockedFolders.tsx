import { ChevronDown, ChevronRight, Lock, LockOpen } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import type { LockAction } from "@/features/folders/FolderTree";
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
  /** `true`, or why the lock cannot change (read-only library). */
  writable: true | string;
  /** A running Eagle: the API can unlock but never set, change or remove a lock. */
  eagle?: boolean;
  /** A locked row was chosen: ask for that folder's password. */
  onUnlock: (id: string) => void;
  /** An unlocked row was chosen: open the folder. */
  onOpen: (id: string) => void;
  /** A lock command from the context menu (D-059). */
  onLock: (id: string, action: LockAction) => void;
};

/**
 * The Locked folders section of the left bar (docs/spec.md §7.3, D-018, D-059): each locked
 * folder with its name and its tag (a label and a colour). Choosing a locked row asks for that
 * folder's password only. Unlock, Lock Now, password and YubiKey commands live here, not in
 * the Folders tree.
 */
export function LockedFolders({
  folders,
  selectedId,
  writable,
  eagle = false,
  onUnlock,
  onOpen,
  onLock,
}: Props) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(true);
  const locked = lockedFolders(folders);
  const canWrite = writable === true;
  if (locked.length === 0) return null;
  return (
    <section aria-label={t("sidebar.lockedFolders")} className="grid gap-px">
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
        {t("sidebar.lockedFolders")}
      </button>
      {open && (
        <ul className="grid gap-px">
          {locked.map((folder) => {
            const Icon = folder.isUnlocked ? LockOpen : Lock;
            const color = folder.lockColor ? FOLDER_COLORS[folder.lockColor] : undefined;
            return (
              <li key={folder.id}>
                <ContextMenu>
                  <ContextMenuTrigger asChild>
                    <button
                      type="button"
                      aria-current={folder.id === selectedId ? "page" : undefined}
                      title={
                        folder.isUnlocked ? t("sidebar.openFolder") : t("sidebar.unlockFolder")
                      }
                      className="flex h-7 w-full items-center gap-2 rounded-md px-2 text-left text-[13px] outline-none hover:bg-sidebar-accent focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:bg-sidebar-accent aria-[current=page]:font-medium"
                      onClick={() => (folder.isUnlocked ? onOpen(folder.id) : onUnlock(folder.id))}
                    >
                      <Icon
                        aria-label={folder.isUnlocked ? t("sidebar.unlocked") : t("sidebar.locked")}
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
                  </ContextMenuTrigger>
                  <ContextMenuContent>
                    {!folder.isUnlocked && (
                      <ContextMenuItem onSelect={() => onLock(folder.id, "unlock")}>
                        Unlock…
                      </ContextMenuItem>
                    )}
                    {eagle ? null : (
                      <>
                        {folder.isUnlocked && (
                          <ContextMenuItem onSelect={() => onLock(folder.id, "relock")}>
                            Lock Now
                          </ContextMenuItem>
                        )}
                        <ContextMenuSeparator />
                        <ContextMenuItem
                          disabled={!canWrite}
                          title={canWrite ? undefined : writable}
                          onSelect={() => onLock(folder.id, "change")}
                        >
                          Change Password…
                        </ContextMenuItem>
                        <ContextMenuItem
                          disabled={!canWrite}
                          title={canWrite ? undefined : writable}
                          onSelect={() => onLock(folder.id, "renew")}
                        >
                          New Recovery Key…
                        </ContextMenuItem>
                        <ContextMenuItem
                          disabled={!canWrite}
                          title={canWrite ? undefined : writable}
                          onSelect={() => onLock(folder.id, "yubikey")}
                        >
                          Add YubiKey…
                        </ContextMenuItem>
                        <ContextMenuItem
                          disabled={!canWrite}
                          title={canWrite ? undefined : writable}
                          onSelect={() => onLock(folder.id, "unyubikey")}
                        >
                          Remove YubiKey…
                        </ContextMenuItem>
                        <ContextMenuItem
                          disabled={!canWrite}
                          title={canWrite ? undefined : writable}
                          onSelect={() => onLock(folder.id, "remove")}
                        >
                          Remove Lock…
                        </ContextMenuItem>
                      </>
                    )}
                  </ContextMenuContent>
                </ContextMenu>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
