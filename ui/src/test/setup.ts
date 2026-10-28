import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// jsdom lacks what Radix menus and dialogs call.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;
Element.prototype.hasPointerCapture ??= () => false;
Element.prototype.releasePointerCapture ??= () => {};
Element.prototype.setPointerCapture ??= () => {};
Element.prototype.scrollIntoView ??= () => {};

afterEach(() => {
  cleanup();
  clearMocks();
  // clearMocks keeps the object; remove it so the next test starts outside Tauri.
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  document.documentElement.classList.remove("dark");
});
