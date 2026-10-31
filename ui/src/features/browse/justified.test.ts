import { describe, expect, it } from "vitest";
import { GAP, NAME_HEIGHT, PADDING } from "./geometry";
import {
  aspectOf,
  itemAtScrollTop,
  justifiedCellRect,
  justifiedIndicesInRect,
  moveFocusJustified,
  packJustified,
} from "./justified";

describe("justified packing", () => {
  it("treats missing sizes as square", () => {
    expect(aspectOf(0, 0)).toBe(1);
    expect(aspectOf(200, 100)).toBe(2);
  });

  it("fills the row width with equal-height cells from aspect ratios", () => {
    // Three landscape 2:1 items at 800×160 → two per row after stretch.
    const layout = packJustified(5, 800, 160, false, () => ({ width: 40, height: 20 }));
    expect(layout.rows).toHaveLength(3);
    expect(layout.rows[0]?.cells).toHaveLength(2);
    expect(layout.rows[1]?.cells).toHaveLength(2);
    expect(layout.rows[2]?.cells).toHaveLength(1);
    const available = 800 - 2 * PADDING;
    const first = layout.rows[0];
    expect(first).toBeDefined();
    if (!first) return;
    const widths = first.cells.map((cell) => cell.width);
    expect((widths[0] ?? 0) + (widths[1] ?? 0) + GAP).toBeCloseTo(available, 0);
    expect(first.cells[0]?.height).toBe(first.cells[1]?.height);
    expect(first.cells[0]?.height).not.toBe(160);
  });

  it("adds the name line to row height when Show Names is on", () => {
    const plain = packJustified(2, 800, 160, false, () => ({ width: 1, height: 1 }));
    const named = packJustified(2, 800, 160, true, () => ({ width: 1, height: 1 }));
    expect(named.rows[0]?.contentHeight).toBe((plain.rows[0]?.thumbHeight ?? 0) + NAME_HEIGHT);
  });

  it("finds cells a rubber band touches", () => {
    const layout = packJustified(6, 800, 160, false, () => ({ width: 40, height: 20 }));
    const second = justifiedCellRect(layout, 1);
    const fourth = justifiedCellRect(layout, 3);
    expect(second).not.toBeNull();
    expect(fourth).not.toBeNull();
    if (!second || !fourth) return;
    expect(
      justifiedIndicesInRect(layout, 6, {
        left: second.left + 2,
        top: second.top + 2,
        right: fourth.left + 2,
        bottom: fourth.top + 2,
      }),
    ).toEqual([1, 3]);
  });

  it("moves focus by row geometry, not a fixed column count", () => {
    const layout = packJustified(6, 800, 160, false, () => ({ width: 40, height: 20 }));
    // Two per row: down from 0 → 2, right from 0 → 1.
    expect(moveFocusJustified("ArrowRight", 0, layout, 2)).toBe(1);
    expect(moveFocusJustified("ArrowDown", 0, layout, 2)).toBe(2);
    expect(moveFocusJustified("ArrowDown", 1, layout, 2)).toBe(3);
    expect(moveFocusJustified("ArrowUp", 3, layout, 2)).toBe(1);
    expect(moveFocusJustified("Home", 4, layout, 2)).toBe(0);
    expect(moveFocusJustified("End", 0, layout, 2)).toBe(5);
    expect(moveFocusJustified("Tab", 0, layout, 2)).toBeNull();
  });

  it("anchors scroll to an item near the top", () => {
    const layout = packJustified(10, 800, 160, false, () => ({ width: 40, height: 20 }));
    const second = layout.rows[1];
    expect(second).toBeDefined();
    if (!second) return;
    expect(itemAtScrollTop(layout, PADDING + second.top + 4)).toBe(second.startIndex);
  });
});
