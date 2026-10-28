import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
  clearMocks();
  // clearMocks keeps the object; remove it so the next test starts outside Tauri.
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
