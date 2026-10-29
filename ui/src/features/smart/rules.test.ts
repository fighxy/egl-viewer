import { describe, expect, it } from "vitest";
import { dayStart, dayText, editable, newRule, readConditions, today, withMethod } from "./rules";

describe("smart folder rules", () => {
  it("reads Eagle's conditions and keeps keys it does not know", () => {
    const read = readConditions([
      { match: "OR", rules: [{ property: "color", method: "similar", value: "#f00", extra: 1 }] },
      "not a condition",
    ]);
    expect(read).toEqual([
      { match: "OR", rules: [{ property: "color", method: "similar", value: "#f00", extra: 1 }] },
    ]);
    expect(readConditions(null)).toEqual([]);
    expect(editable(read[0]?.rules[0] ?? newRule("tags"))).toBe(false);
    expect(editable(newRule("width"))).toBe(true);
  });

  it("a new method keeps the value that still fits", () => {
    const size = newRule("fileSize");
    expect(withMethod(size, "between")).toMatchObject({ value: [1, 1], unit: "mb" });
    const within = newRule("createTime");
    expect(within).toMatchObject({ method: "within", value: [7] });
    const before = withMethod(within, "before");
    expect(before.value).toEqual([today()]);
    expect(withMethod(before, "within").value).toEqual([7]);
    expect(withMethod(newRule("tags"), "intersection")).toEqual({
      property: "tags",
      method: "intersection",
      value: [],
    });
  });

  it("dates are day starts in the user's time zone", () => {
    const start = dayStart("2026-10-28");
    expect(start).not.toBeNull();
    expect(dayText(start ?? 0)).toBe("2026-10-28");
    expect(new Date(start ?? 0).getHours()).toBe(0);
    expect(dayStart("")).toBeNull();
  });
});
