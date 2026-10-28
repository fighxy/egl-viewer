import { afterEach, describe, expect, it } from "vitest";
import { installDesktopBehaviour, keyName } from "./desktop";

let uninstall: (() => void) | undefined;
afterEach(() => uninstall?.());

function press(init: KeyboardEventInit) {
  const event = new KeyboardEvent("keydown", { cancelable: true, bubbles: true, ...init });
  window.dispatchEvent(event);
  return event.defaultPrevented;
}

describe("desktop behaviour", () => {
  it("names key combinations the way the registry writes them", () => {
    expect(keyName(new KeyboardEvent("keydown", { key: "o", ctrlKey: true }))).toBe("Ctrl+O");
    expect(keyName(new KeyboardEvent("keydown", { key: "N", ctrlKey: true, shiftKey: true }))).toBe(
      "Ctrl+Shift+N",
    );
    expect(keyName(new KeyboardEvent("keydown", { key: ",", ctrlKey: true }))).toBe("Ctrl+,");
    expect(keyName(new KeyboardEvent("keydown", { key: "F11" }))).toBe("F11");
  });

  it("blocks reload, printing, the browser's find and page zoom", () => {
    uninstall = installDesktopBehaviour();
    expect(press({ key: "F5" })).toBe(true);
    expect(press({ key: "r", ctrlKey: true })).toBe(true);
    expect(press({ key: "p", ctrlKey: true })).toBe(true);
    expect(press({ key: "f", ctrlKey: true })).toBe(true);
    expect(press({ key: "=", ctrlKey: true })).toBe(true);
    expect(press({ key: "ArrowLeft", altKey: true })).toBe(true);
    expect(press({ key: "o", ctrlKey: true })).toBe(false);
    expect(press({ key: "a" })).toBe(false);
  });

  it("blocks Ctrl + wheel", () => {
    uninstall = installDesktopBehaviour();
    const zoom = new WheelEvent("wheel", { ctrlKey: true, cancelable: true, bubbles: true });
    window.dispatchEvent(zoom);
    expect(zoom.defaultPrevented).toBe(true);
    const scroll = new WheelEvent("wheel", { cancelable: true, bubbles: true });
    window.dispatchEvent(scroll);
    expect(scroll.defaultPrevented).toBe(false);
  });

  it("replaces the browser context menu but keeps it in text fields", () => {
    uninstall = installDesktopBehaviour();
    const input = document.createElement("input");
    document.body.append(input);
    const onPage = new MouseEvent("contextmenu", { cancelable: true, bubbles: true });
    document.body.dispatchEvent(onPage);
    expect(onPage.defaultPrevented).toBe(true);
    const inField = new MouseEvent("contextmenu", { cancelable: true, bubbles: true });
    input.dispatchEvent(inField);
    expect(inField.defaultPrevented).toBe(false);
    input.remove();
  });

  it("stops after uninstalling", () => {
    installDesktopBehaviour()();
    expect(press({ key: "F5" })).toBe(false);
  });
});
