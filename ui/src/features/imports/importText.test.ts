import { describe, expect, it } from "vitest";
import { progress } from "@/test/backend";
import { importSummary, percent, plural } from "./importText";

describe("import text", () => {
  it("counts in plain English", () => {
    expect(plural(1, "file")).toBe("1 file");
    expect(plural(1020, "file")).toBe("1,020 files");
  });

  it("says what an import is doing", () => {
    expect(importSummary(progress({ state: "scanning" }))).toBe("Looking for files…");
    expect(importSummary(progress())).toBe("Adding files: 1 of 3");
    expect(importSummary(progress({ state: "waitingForDecision" }))).toContain("duplicates");
    expect(importSummary(progress({ state: "previews", previewsDone: 2, previewsTotal: 3 }))).toBe(
      "Making previews: 2 of 3",
    );
    expect(
      importSummary(
        progress({ state: "done", added: 40, duplicates: 2, failed: 1, processed: 43 }),
      ),
    ).toBe("Added 40 files · 2 duplicates skipped · 1 problem");
    expect(importSummary(progress({ state: "cancelled", added: 1 }))).toBe(
      "Stopped · added 1 file",
    );
    expect(importSummary(progress({ kind: "previews", state: "done", previewsDone: 5 }))).toBe(
      "Made 5 previews",
    );
  });

  it("measures files first, then previews", () => {
    expect(percent(progress({ processed: 1, total: 4 }))).toBe(25);
    expect(percent(progress({ state: "previews", previewsDone: 1, previewsTotal: 2 }))).toBe(50);
    expect(percent(progress({ state: "done" }))).toBe(100);
  });
});
