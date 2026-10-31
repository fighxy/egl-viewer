import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
  const canWrite = writable === true;
  if (inTrash) {
    return (
      <>
        <ContextMenuItem disabled={!canWrite} onSelect={actions.onRestore}>
          {t("itemMenu.restore")}
        </ContextMenuItem>
        <ContextMenuItem disabled={!canWrite} onSelect={actions.onDeletePermanently}>
          {t("itemMenu.deletePermanent")}
          <ContextMenuShortcut>Delete</ContextMenuShortcut>
        </ContextMenuItem>
      </>
    );
  }
  return (
    <>
      <ContextMenuItem onSelect={onPreview}>
        {t("itemMenu.preview")}
        <ContextMenuShortcut>Space</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuItem disabled={count !== 1} onSelect={actions.onOpenOriginal}>
        {t("itemMenu.openDefault")}
      </ContextMenuItem>
      <ContextMenuItem onSelect={actions.onReveal}>{t("itemMenu.showExplorer")}</ContextMenuItem>
      <ContextMenuItem onSelect={actions.onCopy}>
        {count === 1 ? t("itemMenu.copyFile") : t("itemMenu.copyFiles")}
        <ContextMenuShortcut>Ctrl+C</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem disabled={!canWrite} onSelect={actions.onTrash}>
        {t("itemMenu.moveTrash")}
        <ContextMenuShortcut>Delete</ContextMenuShortcut>
      </ContextMenuItem>
    </>
  );
}
