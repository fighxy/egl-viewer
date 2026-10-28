import { describe, expect, it } from "vitest";
import { describeError } from "./errors";

describe("describeError", () => {
  it("explains library errors in plain English", () => {
    expect(describeError({ kind: "notALibrary", detail: { eagle: false } })).toBe(
      "This folder is not an egl-viewer library.",
    );
    expect(describeError({ kind: "notALibrary", detail: { eagle: true } })).toContain(
      "Eagle library",
    );
    expect(describeError({ kind: "newerFormat", detail: { version: 3 } })).toContain("format 3");
    expect(describeError({ kind: "unsupportedLocation", detail: "syncFolder" })).toContain(
      "OneDrive",
    );
    expect(describeError({ kind: "unsupportedLocation", detail: "networkPath" })).toContain(
      "network drive",
    );
    expect(describeError({ kind: "alreadyExists" })).toContain("already exists");
    expect(describeError({ kind: "invalid", detail: "Enter a name." })).toBe("Enter a name.");
  });

  it("passes other errors through", () => {
    expect(describeError(new Error("boom"))).toBe("boom");
    expect(describeError("plain")).toBe("plain");
  });
});
