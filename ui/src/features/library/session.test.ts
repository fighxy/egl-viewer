import { describe, expect, it } from "vitest";
import { NOTHING } from "@/features/browse/selection";
import { NO_FILTERS } from "@/features/library/FilterBar";
import {
  filtersFromSaved,
  placeFromSaved,
  savedFilters,
  savedPlace,
  savedSelection,
  selectionFromSaved,
} from "./session";

describe("library session mapping", () => {
  it("round-trips places", () => {
    expect(placeFromSaved({ kind: "all" })).toEqual({ kind: "all" });
    expect(savedPlace({ kind: "folder", id: "F1" })).toEqual({ kind: "folder", id: "F1" });
    expect(placeFromSaved({ kind: "smart", id: "S1" })).toEqual({ kind: "smart", id: "S1" });
  });

  it("round-trips filters and selection", () => {
    const filters = {
      ...NO_FILTERS,
      tags: ["Poster"],
      tagMode: "all" as const,
      rating: 3,
    };
    expect(filtersFromSaved(savedFilters(filters))).toEqual(filters);
    const selection = { all: false, ids: new Set(["a", "b"]), anchor: 1, focus: 2 };
    expect(selectionFromSaved(savedSelection(selection))).toEqual({
      all: false,
      ids: new Set(["a", "b"]),
      anchor: null,
      focus: null,
    });
    expect(savedSelection(NOTHING)).toEqual({ all: false, ids: [] });
  });
});
