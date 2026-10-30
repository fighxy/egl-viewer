import { describe, expect, it } from "vitest";
import type { ItemsTarget, SelectionTags } from "@/ipc";
import { dropAddsNothing } from "./drop";
import { matchTags } from "./match";
import { RECENT_LIMIT, remember, replaced } from "./recent";

const tag = (name: string) => ({ name, imageCount: 1 });

describe("matchTags", () => {
  it("finds names that start with the text first, ignoring case but keeping each tag", () => {
    const tags = ["Grotesk", "Poster", "poster", "Typeposter", "Swiss"].map(tag);
    expect(matchTags(tags, "post").map((found) => found.name)).toEqual([
      "Poster",
      "poster",
      "Typeposter",
    ]);
    expect(matchTags(tags, "  ")).toHaveLength(5);
  });
});

function target(ids: string[], all = false): ItemsTarget {
  return { viewId: 1, all, ids };
}

function counts(items: number, tags: { name: string; imageCount: number | null }[]): SelectionTags {
  return { items, tags, size: 0, stack: [] };
}

describe("dropping items on a tag", () => {
  it("adds nothing only when every item is known to already have that exact tag", () => {
    const poster = new Map<string, readonly string[]>([["a", ["Poster", "Swiss"]]]);
    expect(dropAddsNothing("Poster", target(["a"]), poster, null)).toBe(true);
    // A different case is a different tag, and an unknown item is not skipped.
    expect(dropAddsNothing("poster", target(["a"]), poster, null)).toBe(false);
    expect(dropAddsNothing("Logo", target(["a"]), poster, null)).toBe(false);
    expect(dropAddsNothing("Poster", target(["b"]), poster, null)).toBe(false);

    const both = {
      target: target(["a", "b"]),
      tags: counts(2, [
        { name: "Poster", imageCount: 2 },
        { name: "Swiss", imageCount: 1 },
      ]),
    };
    expect(dropAddsNothing("Poster", target(["a", "b"]), new Map(), both)).toBe(true);
    expect(dropAddsNothing("Swiss", target(["a", "b"]), new Map(), both)).toBe(false);
    expect(dropAddsNothing("Logo", target(["a", "b"]), new Map(), both)).toBe(false);
    // Counts for a different selection do not decide this drop.
    expect(dropAddsNothing("Poster", target(["a"]), new Map(), both)).toBe(false);
    expect(dropAddsNothing("Poster", target([], true), new Map(), null)).toBe(false);
    const everyone = {
      target: target([], true),
      tags: counts(4, [{ name: "Poster", imageCount: 4 }]),
    };
    expect(dropAddsNothing("Poster", target([], true), new Map(), everyone)).toBe(true);
    expect(
      dropAddsNothing("Poster", target([], true), new Map(), {
        target: target([], true),
        tags: counts(4, [{ name: "Poster", imageCount: null }]),
      }),
    ).toBe(false);
  });
});

describe("recent tags", () => {
  it("puts used tags first, each exact name once, up to the limit", () => {
    expect(remember(["Swiss", "Logo"], ["Logo", "poster"])).toEqual(["Logo", "poster", "Swiss"]);
    expect(remember(["Poster"], ["poster"])).toEqual(["poster", "Poster"]);
    const many = Array.from({ length: 30 }, (_, n) => `t${n}`);
    expect(remember([], many)).toHaveLength(RECENT_LIMIT);
  });

  it("follows renames, merges and deletes", () => {
    expect(replaced(["Poster", "Swiss"], "Poster", "Posters")).toEqual(["Posters", "Swiss"]);
    expect(replaced(["poster", "Poster"], "poster", "Poster")).toEqual(["Poster"]);
    expect(replaced(["Poster", "Swiss"], "Swiss", null)).toEqual(["Poster"]);
  });
});
