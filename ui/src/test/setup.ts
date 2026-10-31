import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";
import { setAppLocale } from "@/i18n";

// The first render of the whole app in a test file loads every feature module and the
// locale catalogs; on a cold Windows CI runner that alone can outlast the 1 s default of
// `findBy*`/`waitFor`, leaving the app on the Welcome screen when the query gives up.
configure({ asyncUtilTimeout: 5000 });

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
  setAppLocale("en");
});
