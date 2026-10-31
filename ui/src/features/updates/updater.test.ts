import { describe, expect, it } from "vitest";
import { progressLabel } from "./updater";

describe("progressLabel", () => {
  it("shows a percent when the length is known", () => {
    expect(progressLabel({ downloaded: 50, contentLength: 200 })).toBe("Downloading update… 25%");
  });

  it("falls back to bytes while the length is unknown", () => {
    expect(progressLabel({ downloaded: 2048, contentLength: null })).toBe(
      "Downloading update… 2 KB",
    );
  });

  it("names the step before the first chunk", () => {
    expect(progressLabel({ downloaded: 0, contentLength: null })).toBe("Downloading update…");
  });
});
