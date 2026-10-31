import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
  if (!actions) return children;
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => actions.onRename(name)}>
          {t("tagMenu.rename")}
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => actions.onMerge(name)}>
          {t("tagMenu.merge")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={() => actions.onDelete(name)}>
          {t("tagMenu.delete")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
