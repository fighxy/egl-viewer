import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";

/** A tag row's context menu: export is always there; the edits need a writable library. */
export type TagActions = {
  onExport: (name: string) => void;
  onRename?: (name: string) => void;
  onMerge?: (name: string) => void;
  onDelete?: (name: string) => void;
};

/** Export, and rename, merge and delete where the library can be written. */
export function TagMenu({
  name,
  actions,
  children,
}: {
  name: string;
  actions: TagActions;
  children: ReactElement;
}) {
  const { t } = useTranslation();
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => actions.onExport(name)}>
          {t("tagMenu.export")}
        </ContextMenuItem>
        {actions.onRename && (
          <>
            <ContextMenuSeparator />
            <ContextMenuItem onSelect={() => actions.onRename?.(name)}>
              {t("tagMenu.rename")}
            </ContextMenuItem>
            <ContextMenuItem onSelect={() => actions.onMerge?.(name)}>
              {t("tagMenu.merge")}
            </ContextMenuItem>
            <ContextMenuItem onSelect={() => actions.onDelete?.(name)}>
              {t("tagMenu.delete")}
            </ContextMenuItem>
          </>
        )}
      </ContextMenuContent>
    </ContextMenu>
  );
}
