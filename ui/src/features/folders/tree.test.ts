import { describe, expect, it } from "vitest";
import { folder } from "@/test/backend";
import {
  allRows,
  canMove,
  findFolder,
  folderCount,
  folderPath,
  parentOf,
  siblingsOf,
  visibleRows,
  withoutLockedFolders,
} from "./tree";

const TREE = [
  folder("A", "Alpha", {
    imageCount: 1,
    descendantImageCount: 4,
    children: [folder("A1", "One", { children: [folder("A1a", "Deep")] }), folder("A2", "Two")],
  }),
  folder("B", "Beta"),
];

describe("folder tree helpers", () => {
  it("finds folders, their paths and parents", () => {
    expect(findFolder(TREE, "A1a")?.name).toBe("Deep");
    expect(findFolder(TREE, "nope")).toBeUndefined();
    expect(folderPath(TREE, "A1a").map((step) => step.id)).toEqual(["A", "A1", "A1a"]);
    expect(folderPath(TREE, "nope")).toEqual([]);
    expect(parentOf(TREE, "A2")).toBe("A");
    expect(parentOf(TREE, "B")).toBeNull();
    expect(parentOf(TREE, "nope")).toBeUndefined();
    expect(siblingsOf(TREE, "A2").map((step) => step.id)).toEqual(["A1", "A2"]);
    expect(siblingsOf(TREE, "B").map((step) => step.id)).toEqual(["A", "B"]);
  });

  it("refuses moves into the folder itself or below it", () => {
    expect(canMove(TREE, "A", "A1a")).toBe(false);
    expect(canMove(TREE, "A", "A")).toBe(false);
    expect(canMove(TREE, "A1", "B")).toBe(true);
    expect(canMove(TREE, "A1a", null)).toBe(true);
    expect(canMove(TREE, "nope", "B")).toBe(false);
  });

  it("shows children only under expanded folders", () => {
    const rows = (expanded: string[]) =>
      visibleRows(TREE, new Set(expanded)).map(({ folder, depth }) => `${depth}:${folder.id}`);
    expect(rows([])).toEqual(["0:A", "0:B"]);
    expect(rows(["A"])).toEqual(["0:A", "1:A1", "1:A2", "0:B"]);
    // A collapsed parent hides an expanded child.
    expect(rows(["A1"])).toEqual(["0:A", "0:B"]);
    expect(allRows(TREE).map(({ folder }) => folder.id)).toEqual(["A", "A1", "A1a", "A2", "B"]);
  });

  it("counts direct items, or items with subfolders", () => {
    const alpha = TREE[0];
    if (!alpha) throw new Error("no folder");
    expect(folderCount(alpha, false)).toBe(1);
    expect(folderCount(alpha, true)).toBe(4);
  });

  it("drops locked folders and their children from the Folders tree", () => {
    const mixed = [
      folder("A", "Alpha", {
        children: [
          folder("L", "Secret", {
            isLocked: true,
            children: [folder("L1", "Inside")],
          }),
          folder("A2", "Open"),
        ],
      }),
      folder("B", "Locked root", { isLocked: true, isUnlocked: true }),
      folder("C", "Plain"),
    ];
    expect(withoutLockedFolders(mixed).map((folder) => folder.id)).toEqual(["A", "C"]);
    expect(withoutLockedFolders(mixed)[0]?.children.map((child) => child.id)).toEqual(["A2"]);
  });
});
