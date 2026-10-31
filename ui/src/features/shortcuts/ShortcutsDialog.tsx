import { useTranslation } from "react-i18next";
import { COMMANDS, type Command, WIDGET_KEYS } from "@/commands/registry";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { translateCommandLabel, translateMenuLabel } from "@/i18n/labels";
import { DEFAULT_PREFERENCES } from "@/ipc";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  commands?: readonly Command[];
};

const idleState: import("@/commands/registry").CommandState = {
  library: null,
  recent: [],
  theme: DEFAULT_PREFERENCES.theme,
  fullscreen: false,
  leftOpen: true,
  rightOpen: true,
  browse: DEFAULT_PREFERENCES.browse,
  sort: DEFAULT_PREFERENCES.browse.sort,
  ascending: DEFAULT_PREFERENCES.browse.ascending,
  folderOpen: false,
  unlockedFolders: 0,
  items: 0,
  selected: 0,
  inTrash: false,
  undo: null,
};

const WIDGET_LABELS = [
  "moveSelection",
  "extendSelection",
  "firstLast",
  "goDetails",
  "saveRevert",
  "thumbSize",
  "folderNav",
  "folderEdit",
  "prevNext",
  "zoom",
  "rotate",
  "close",
] as const;

const WIDGET_AREAS = [
  "grid",
  "grid",
  "grid",
  "grid",
  "details",
  "grid",
  "folders",
  "folders",
  "preview",
  "preview",
  "preview",
  "preview",
] as const;

const WIDGET_KEY_IDS = [
  "arrows",
  "shiftArrows",
  "homeEnd",
  "enter",
  "enterEsc",
  "ctrlWheel",
  "folderKeys",
  "f2Delete",
  "leftRight",
  "zoomKeys",
  "r",
  "esc",
] as const;

/** Every command with a shortcut, read from the command registry (docs/spec.md §7.4). */
export function ShortcutsDialog({ open, onOpenChange, commands = COMMANDS }: Props) {
  const { t } = useTranslation();
  const withShortcuts = commands.filter((command) => command.shortcut);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t("shortcuts.title")}</DialogTitle>
          <DialogDescription>{t("shortcuts.description")}</DialogDescription>
        </DialogHeader>
        <table className="w-full text-[13px]">
          <tbody>
            {withShortcuts.map((command) => (
              <tr key={command.id} className="border-b last:border-0">
                <td className="py-1.5 text-muted-foreground">
                  {translateMenuLabel(t, command.menu)}
                </td>
                <td className="py-1.5">
                  {translateCommandLabel(t, command, idleState).replace("…", "")}
                </td>
                <td className="py-1.5 text-right">
                  <kbd className="rounded border bg-muted px-1.5 py-0.5 font-sans text-xs">
                    {command.shortcut}
                  </kbd>
                </td>
              </tr>
            ))}
            {WIDGET_KEYS.map((key, index) => (
              <tr key={`${key.area}:${key.keys}`} className="border-b last:border-0">
                <td className="py-1.5 text-muted-foreground">
                  {t(`widgetKeys.${WIDGET_AREAS[index]}`)}
                </td>
                <td className="py-1.5">{t(`widgetKeys.${WIDGET_LABELS[index]}`)}</td>
                <td className="py-1.5 text-right">
                  <kbd className="rounded border bg-muted px-1.5 py-0.5 font-sans text-xs">
                    {t(`widgetKeys.${WIDGET_KEY_IDS[index]}`)}
                  </kbd>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </DialogContent>
    </Dialog>
  );
}
