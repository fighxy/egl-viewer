import { COMMANDS, type Command, MENUS, WIDGET_KEYS } from "@/commands/registry";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  commands?: readonly Command[];
};

/** Every command with a shortcut, read from the command registry (docs/spec.md §7.4). */
export function ShortcutsDialog({ open, onOpenChange, commands = COMMANDS }: Props) {
  const withShortcuts = commands.filter((command) => command.shortcut);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Keyboard shortcuts</DialogTitle>
          <DialogDescription>What the keyboard does in this version.</DialogDescription>
        </DialogHeader>
        <table className="w-full text-[13px]">
          <tbody>
            {withShortcuts.map((command) => (
              <tr key={command.id} className="border-b last:border-0">
                <td className="py-1.5 text-muted-foreground">
                  {MENUS.find((menu) => menu.id === command.menu)?.label}
                </td>
                <td className="py-1.5">{command.label.replace("…", "")}</td>
                <td className="py-1.5 text-right">
                  <kbd className="rounded border bg-muted px-1.5 py-0.5 font-sans text-xs">
                    {command.shortcut}
                  </kbd>
                </td>
              </tr>
            ))}
            {WIDGET_KEYS.map((key) => (
              <tr key={`${key.area}:${key.keys}`} className="border-b last:border-0">
                <td className="py-1.5 text-muted-foreground">{key.area}</td>
                <td className="py-1.5">{key.label}</td>
                <td className="py-1.5 text-right">
                  <kbd className="rounded border bg-muted px-1.5 py-0.5 font-sans text-xs">
                    {key.keys}
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
