import {
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
} from "@/components/ui/context-menu";
import type { ItemActions } from "./DetailsPanel";

type Props = {
  actions: ItemActions;
  /** Selected items. */
  count: number;
  /** `true`, or why items cannot change (read-only library). */
  writable: true | string;
  inTrash: boolean;
  onPreview: () => void;
};

/** The right-click menu of the grid and the list: the same actions as the right bar. */
export function ItemMenu({ actions, count, writable, inTrash, onPreview }: Props) {
  const canWrite = writable === true;
  if (inTrash) {
    return (
      <>
        <ContextMenuItem disabled={!canWrite} onSelect={actions.onRestore}>
          Restore
        </ContextMenuItem>
        <ContextMenuItem disabled={!canWrite} onSelect={actions.onDeletePermanently}>
          Delete Permanently…
          <ContextMenuShortcut>Delete</ContextMenuShortcut>
        </ContextMenuItem>
      </>
    );
  }
  return (
    <>
      <ContextMenuItem onSelect={onPreview}>
        Preview
        <ContextMenuShortcut>Space</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuItem disabled={count !== 1} onSelect={actions.onOpenOriginal}>
        Open with Default App
      </ContextMenuItem>
      <ContextMenuItem onSelect={actions.onReveal}>Show in Explorer</ContextMenuItem>
      <ContextMenuItem onSelect={actions.onCopy}>
        {count === 1 ? "Copy File" : "Copy Files"}
        <ContextMenuShortcut>Ctrl+C</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem disabled={!canWrite} onSelect={actions.onTrash}>
        Move to Trash
        <ContextMenuShortcut>Delete</ContextMenuShortcut>
      </ContextMenuItem>
    </>
  );
}
