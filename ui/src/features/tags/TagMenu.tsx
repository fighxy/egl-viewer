import type { ReactElement } from "react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";

/** Rename, merge and delete a tag everywhere; each opens a dialog that confirms. */
export type TagActions = {
  onRename: (name: string) => void;
  onMerge: (name: string) => void;
  onDelete: (name: string) => void;
};

/** The context menu of a tag row; without `actions` (read-only library) the row has none. */
export function TagMenu({
  name,
  actions,
  children,
}: {
  name: string;
  actions?: TagActions;
  children: ReactElement;
}) {
  if (!actions) return children;
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => actions.onRename(name)}>Rename…</ContextMenuItem>
        <ContextMenuItem onSelect={() => actions.onMerge(name)}>Merge Into…</ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={() => actions.onDelete(name)}>Delete…</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
