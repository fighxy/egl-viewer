import { COMMANDS, type Command, MENUS } from "@/commands/registry";
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
      <DialogContent>
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
          </tbody>
        </table>
      </DialogContent>
    </Dialog>
  );
}
