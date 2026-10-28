import { describe, expect, it } from "vitest";
import {
  cellRect,
  GAP,
  gridLayout,
  indicesInRect,
  LIST_ROW,
  listLayout,
  moveFocus,
  NAME_HEIGHT,
  PADDING,
} from "./geometry";

describe("grid geometry", () => {
  it("fits as many cells as the width allows and stretches them", () => {
    const layout = gridLayout(800, 160, true);
    expect(layout.columns).toBe(4);
    expect(layout.cell * 4 + GAP * 3).toBeLessThanOrEqual(800 - 2 * PADDING);
    expect(layout.cell).toBeGreaterThanOrEqual(160);
    expect(layout.rowHeight).toBe(layout.cell + NAME_HEIGHT + GAP);
    expect(gridLayout(800, 160, false).rowHeight).toBe(layout.cell + GAP);
    expect(gridLayout(100, 320, true).columns).toBe(1);
    expect(listLayout(600)).toMatchObject({ columns: 1, rowHeight: LIST_ROW });
  });

  it("finds the cells a rubber band touches", () => {
    const layout = gridLayout(800, 160, false);
    const second = cellRect(layout, 1);
    const sixth = cellRect(layout, 5);
    const band = {
      left: second.left + 5,
      top: second.top + 5,
      right: sixth.left + 5,
      bottom: sixth.top + 5,
    };
    expect(indicesInRect(layout, 100, band)).toEqual([1, 5]);
    // The gap between cells touches nothing.
    const gap = {
      left: second.right + 1,
      top: second.top,
      right: second.right + GAP - 1,
      bottom: second.bottom,
    };
    expect(indicesInRect(layout, 100, gap)).toEqual([]);
    // Positions past the end do not exist.
    const everything = { left: 0, top: 0, right: 800, bottom: 10_000 };
    expect(indicesInRect(layout, 6, everything)).toEqual([0, 1, 2, 3, 4, 5]);
  });

  it("moves the keyboard focus within the items", () => {
    expect(moveFocus("ArrowRight", 0, 10, 4, 2)).toBe(1);
    expect(moveFocus("ArrowDown", 1, 10, 4, 2)).toBe(5);
    expect(moveFocus("ArrowDown", 8, 10, 4, 2)).toBe(9);
    expect(moveFocus("ArrowUp", 1, 10, 4, 2)).toBe(0);
    expect(moveFocus("ArrowDown", null, 10, 4, 2)).toBe(0);
    expect(moveFocus("End", 0, 10, 4, 2)).toBe(9);
    expect(moveFocus("Home", 7, 10, 4, 2)).toBe(0);
    expect(moveFocus("PageDown", 0, 100, 4, 3)).toBe(12);
    // The list has one column: left and right do not move.
    expect(moveFocus("ArrowRight", 3, 10, 1, 2)).toBe(3);
    expect(moveFocus("Tab", 3, 10, 4, 2)).toBeNull();
    expect(moveFocus("ArrowDown", 0, 0, 4, 2)).toBeNull();
  });
});
