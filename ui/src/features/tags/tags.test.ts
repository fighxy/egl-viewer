import { describe, expect, it } from "vitest";
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
