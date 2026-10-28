import { useEffect } from "react";
import { keyName } from "@/lib/desktop";
import {
  availability,
  COMMANDS,
  type Command,
  type CommandActions,
  type CommandState,
  commandForShortcut,
} from "./registry";

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
      const command = commandForShortcut(keyName(event), commands);
      if (!command?.run || availability(command, state) !== true) return;
      event.preventDefault();
      command.run(actions);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [state, actions, enabled, commands]);
}
