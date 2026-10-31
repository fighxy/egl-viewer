import type { TFunction } from "i18next";
import type { Command, CommandState, MenuId, SubmenuItem } from "@/commands/registry";
import { EDIT_NAMES, labelOf } from "@/commands/registry";
import type { EditKind, SortKey, Theme } from "@/ipc";

/** Translate a disabled-reason string that may already be a catalog key or English copy. */
export function translateAvailability(t: TFunction, available: true | string): true | string {
  if (available === true) return true;
  if (available.startsWith("availability.")) return t(available);
  const byEnglish: Record<string, string> = {
    "Open a library first.": "availability.openLibraryFirst",
    "This library is read-only.": "availability.readOnly",
    "No recent libraries.": "availability.noRecent",
    "Select an item first.": "availability.selectItemFirst",
    "Nothing to select.": "availability.nothingToSelect",
    "Nothing to undo.": "availability.nothingToUndo",
    "Thumbnails have a size in the grid.": "availability.thumbnailsGridOnly",
    "Already the largest.": "availability.alreadyLargest",
    "Already the smallest.": "availability.alreadySmallest",
    "No locked folder is open.": "availability.noLockedOpen",
  };
  const key = byEnglish[available];
  return key ? t(key) : available;
}

export function translateMenuLabel(t: TFunction, id: MenuId): string {
  return t(`menus.${id}`);
}

export function translateThemeLabel(t: TFunction, theme: Theme): string {
  return t(`themes.${theme}`);
}

export function translateSortLabel(t: TFunction, key: SortKey | "manual"): string {
  return t(`sort.${key}`);
}

export function translateEditName(t: TFunction, kind: EditKind): string {
  return t(`edits.${kind}`, { defaultValue: EDIT_NAMES[kind] });
}

/** Menu / shortcuts label for a command in the active locale. */
export function translateCommandLabel(t: TFunction, command: Command, state: CommandState): string {
  if (command.id === "edit.undo" && state.undo) {
    return t("commands.edit.undoNamed", { name: translateEditName(t, state.undo.kind) });
  }
  if (command.id === "edit.delete") {
    return state.inTrash ? t("commands.edit.deletePermanent") : t("commands.edit.delete");
  }
  return t(`commands.${command.id}`, { defaultValue: labelOf(command, state) });
}

/** Translate submenu choice labels (theme, sort, …). */
export function translateSubmenuItem(t: TFunction, item: SubmenuItem): string {
  if (item.id === "system" || item.id === "light" || item.id === "dark") {
    return translateThemeLabel(t, item.id);
  }
  if (item.id === "ascending") return t("sort.ascending");
  if (item.id === "descending") return t("sort.descending");
  const sortKeys = new Set([
    "added",
    "btime",
    "mtime",
    "name",
    "ext",
    "size",
    "resolution",
    "star",
    "tag",
    "random",
    "manual",
  ]);
  if (sortKeys.has(item.id)) return translateSortLabel(t, item.id as SortKey | "manual");
  return item.label;
}
