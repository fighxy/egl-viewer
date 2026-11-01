import { useLayoutEffect, useRef } from "react";
import { isEditable, isInteractive, keyName } from "@/lib/desktop";
import {
  availability,
  COMMANDS,
  type Command,
  type CommandActions,
  type CommandState,
  commandForShortcut,
} from "./registry";

/** Text fields keep their own cut, copy, paste, select all, undo and Delete. */
const TEXT_KEYS = new Set(["Ctrl+A", "Ctrl+C", "Ctrl+V", "Ctrl+X", "Ctrl+Z", "Ctrl+Y", "Delete"]);
/** Buttons, links and fields keep their own Space and Enter. */
const ACTIVATION_KEYS = new Set(["Space", "Enter"]);

/**
 * Runs registry commands from the keyboard while `enabled` (off while a dialog is open). The
 * listener reads the latest state and actions, so a key pressed right after a change never
 * runs a command against the state before it.
 */
export function useShortcuts(
  state: CommandState,
  actions: CommandActions,
  enabled: boolean,
  commands: readonly Command[] = COMMANDS,
) {
  const latest = useRef({ state, actions, commands });
  useLayoutEffect(() => {
    latest.current = { state, actions, commands };
  });
  useLayoutEffect(() => {
    if (!enabled) return;
    const onKeyDown = (event: KeyboardEvent) => {
      const { state, actions, commands } = latest.current;
      const name = keyName(event);
      // The folder tree's Delete deletes its folder, not the selected items. (Other keys arrive
      // prevented on purpose: the browser's Ctrl+F is blocked so the app's can run.)
      if (name === "Delete" && event.defaultPrevented) return;
      if (TEXT_KEYS.has(name) && isEditable(event.target)) return;
      if (ACTIVATION_KEYS.has(name) && isInteractive(event.target)) return;
      // Space hold-vs-toggle for the preview is handled by useSpacePreview (D-073).
      if (name === "Space") return;
      const command = commandForShortcut(name, commands);
      if (!command?.run || availability(command, state) !== true) return;
      event.preventDefault();
      command.run(actions);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [enabled]);
}
