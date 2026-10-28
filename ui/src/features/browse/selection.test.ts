import { describe, expect, it } from "vitest";
import {
  everything,
  isSelected,
  NOTHING,
  only,
  rangeFrom,
  selectedCount,
  singleId,
  toggle,
  withRange,
} from "./selection";

describe("selection", () => {
  it("a click selects one item and anchors ranges there", () => {
    const one = only(4, "d");
    expect(isSelected(one, "d")).toBe(true);
    expect(selectedCount(one, 10)).toBe(1);
    expect(singleId(one)).toBe("d");
    expect(rangeFrom(one, 7)).toEqual([4, 4]);
    expect(rangeFrom(one, 2)).toEqual([2, 3]);
  });

  it("Ctrl toggles and moves the anchor", () => {
    const two = toggle(only(1, "a"), 3, "c");
    expect([...two.ids]).toEqual(["a", "c"]);
    expect(two.anchor).toBe(3);
    expect(singleId(two)).toBeNull();
    expect([...toggle(two, 1, "a").ids]).toEqual(["c"]);
  });

  it("Shift replaces the selection with the range, Ctrl+Shift adds it", () => {
    const start = toggle(only(1, "a"), 5, "e");
    const range = withRange(start, 7, ["e", "f", "g"], false);
    expect([...range.ids]).toEqual(["e", "f", "g"]);
    expect(range.anchor).toBe(5);
    expect(range.focus).toBe(7);
    const added = withRange(start, 7, ["e", "f", "g"], true);
    expect([...added.ids].sort()).toEqual(["a", "e", "f", "g"]);
  });

  it("Ctrl+A selects everything without listing it, and Ctrl excludes from it", () => {
    const all = everything(only(0, "a"));
    expect(selectedCount(all, 1000)).toBe(1000);
    expect(isSelected(all, "zzz")).toBe(true);
    expect(singleId(all)).toBeNull();
    const fewer = toggle(all, 2, "b");
    expect(isSelected(fewer, "b")).toBe(false);
    expect(selectedCount(fewer, 1000)).toBe(999);
    const back = withRange(fewer, 3, ["b", "c"], true);
    expect(back.all).toBe(true);
    expect(isSelected(back, "b")).toBe(true);
  });

  it("nothing is selected at first", () => {
    expect(selectedCount(NOTHING, 5)).toBe(0);
    expect(singleId(NOTHING)).toBeNull();
  });
});
