/** Map between the UI place/filters/selection and the persisted library session (D-078). */

import type { Selection } from "@/features/browse/selection";
import { NOTHING } from "@/features/browse/selection";
import type { Filters } from "@/features/library/FilterBar";
import { NO_FILTERS } from "@/features/library/FilterBar";
import type { Place } from "@/features/library/LibraryScreen";
import type { LibrarySession, SavedFilters, SavedPlace, SavedSelection } from "@/ipc";

export function placeFromSaved(place: SavedPlace): Place {
  switch (place.kind) {
    case "folder":
      return { kind: "folder", id: place.id };
    case "smart":
      return { kind: "smart", id: place.id };
    default:
      return { kind: place.kind };
  }
}

export function savedPlace(place: Place): SavedPlace {
  switch (place.kind) {
    case "folder":
      return { kind: "folder", id: place.id };
    case "smart":
      return { kind: "smart", id: place.id };
    default:
      return { kind: place.kind };
  }
}

export function filtersFromSaved(filters: SavedFilters): Filters {
  return {
    tags: [...filters.tags],
    tagMode: filters.tagMode,
    folders: [...filters.folders],
    folderMode: filters.folderMode,
    ext: filters.ext,
    rating: filters.rating,
    shape: filters.shape,
  };
}

export function savedFilters(filters: Filters): SavedFilters {
  return {
    tags: [...filters.tags],
    tagMode: filters.tagMode,
    folders: [...filters.folders],
    folderMode: filters.folderMode,
    ext: filters.ext,
    rating: filters.rating,
    shape: filters.shape,
  };
}

export function selectionFromSaved(selection: SavedSelection): Selection {
  return {
    all: selection.all,
    ids: new Set(selection.ids),
    anchor: null,
    focus: null,
  };
}

export function savedSelection(selection: Selection): SavedSelection {
  return {
    all: selection.all,
    ids: [...selection.ids],
  };
}

export function emptySession(): LibrarySession {
  return {
    place: { kind: "all" },
    filters: savedFilters(NO_FILTERS),
    searchText: "",
    selection: savedSelection(NOTHING),
    screenSort: null,
    screenAscending: null,
  };
}
