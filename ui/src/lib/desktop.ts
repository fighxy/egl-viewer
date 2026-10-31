// Desktop behaviour in the WebView: no browser context menu (anywhere, D-068), reload, printing,
// find, page zoom or history navigation (docs/architecture.md §8, D-029).

/** Browser shortcuts that do nothing in the app. Some become app commands later (Ctrl+F). */
const BLOCKED_KEYS = new Set([
  "F5",
  "Ctrl+R",
  "Ctrl+Shift+R",
  "Ctrl+F5",
  "Ctrl+P",
  "Ctrl+Shift+P",
  "Ctrl+F",
  "Ctrl+G",
  "Ctrl+Shift+G",
  "Ctrl+U",
  "Ctrl+S",
  "Ctrl+=",
  "Ctrl+-",
  "Ctrl+0",
  "Ctrl++",
  "Alt+ArrowLeft",
  "Alt+ArrowRight",
  "BrowserBack",
  "BrowserForward",
  "BrowserRefresh",
]);

/** The layout-independent letter for KeyA–KeyZ, so shortcuts work on non-Latin layouts. */
function letterOf(code: string): string | null {
  return /^Key[A-Z]$/.test(code) ? code.slice(3) : null;
}

export function keyName(event: KeyboardEvent): string {
  const parts: string[] = [];
  if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  const key =
    event.key === " "
      ? "Space"
      : event.key.length === 1
        ? (letterOf(event.code) ?? event.key.toUpperCase())
        : event.key;
  parts.push(key);
  return parts.join("+");
}

export function isEditable(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || target.closest("input, textarea, [contenteditable]") !== null)
  );
}

/** Space and Enter press a focused button, link, field or menu item instead of a shortcut. */
export function isInteractive(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest(
      "button, a[href], input, select, textarea, summary, [contenteditable], [role=button], [role=menuitem], [role=menuitemradio], [role=menuitemcheckbox], [role=checkbox], [role=switch], [role=radio], [role=tab], [role=slider], [role=option]",
    ) !== null
  );
}

/** Installs the handlers on `target` and returns a function that removes them. */
export function installDesktopBehaviour(target: Window = window): () => void {
  const onKeyDown = (event: KeyboardEvent) => {
    if (BLOCKED_KEYS.has(keyName(event))) event.preventDefault();
  };
  const onContextMenu = (event: MouseEvent) => {
    // Always block the WebView/Chromium menu, including on text fields (no emoji / password /
    // cut-copy-paste chrome). App menus (Radix) still open: they handle the event on the
    // target before this bubbling listener runs (D-042, D-068).
    event.preventDefault();
  };
  // The browser menu is blocked after the page saw the event (bubbling, not capture): a menu
  // of the app, such as the folder tree's, ignores an event that was already prevented.
  const onWheel = (event: WheelEvent) => {
    if (event.ctrlKey) event.preventDefault();
  };
  const onMouseUp = (event: MouseEvent) => {
    // The back and forward buttons of a mouse.
    if (event.button === 3 || event.button === 4) event.preventDefault();
  };
  target.addEventListener("keydown", onKeyDown, { capture: true });
  target.addEventListener("contextmenu", onContextMenu);
  target.addEventListener("wheel", onWheel, { capture: true, passive: false });
  target.addEventListener("mouseup", onMouseUp, { capture: true });
  return () => {
    target.removeEventListener("keydown", onKeyDown, { capture: true });
    target.removeEventListener("contextmenu", onContextMenu);
    target.removeEventListener("wheel", onWheel, { capture: true });
    target.removeEventListener("mouseup", onMouseUp, { capture: true });
  };
}
