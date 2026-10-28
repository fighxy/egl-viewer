import { useEffect } from "react";
import { isEditable, keyName } from "@/lib/desktop";
import {
  availability,
  COMMANDS,
  type Command,
  type CommandActions,
  type CommandState,
  commandForShortcut,
} from "./registry";

/** Text fields keep their own cut, copy, paste, select all and undo. */
const TEXT_KEYS = new Set(["Ctrl+A", "Ctrl+C", "Ctrl+V", "Ctrl+X", "Ctrl+Z", "Ctrl+Y"]);

/** Runs registry commands from the keyboard while `enabled` (off while a dialog is open). */
export function useShortcuts(
  state: CommandState,
  actions: CommandActions,
  enabled: boolean,
  commands: readonly Command[] = COMMANDS,
) {
  useEffect(() => {
    if (!enabled) return;
    const onKeyDown = (event: KeyboardEvent) => {
      const name = keyName(event);
      if (TEXT_KEYS.has(name) && isEditable(event.target)) return;
      const command = commandForShortcut(name, commands);
      if (!command?.run || availability(command, state) !== true) return;
      event.preventDefault();
      command.run(actions);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [state, actions, enabled, commands]);
}
