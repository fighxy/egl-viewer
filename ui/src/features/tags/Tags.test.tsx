import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "@/App";
import type { Item, LibraryQuery, Tag, TagGroup, TagGroupChange } from "@/ipc";
import { called, fakeLayout, item, LIBRARY, mockBackend, viewOf } from "@/test/backend";

let restoreLayout: () => void;
beforeEach(() => {
  restoreLayout = fakeLayout(800, 600);
});
afterEach(() => {
  restoreLayout();
  localStorage.clear();
});

const VOCABULARY: Tag[] = [
  { name: "Logo", imageCount: 4 },
  { name: "Poster", imageCount: 3 },
  { name: "poster", imageCount: 1 },
  { name: "Swiss", imageCount: 2 },
];

const GROUPS: TagGroup[] = [{ id: "G1", name: "Kinds", color: "red", tags: ["Logo", "Poster"] }];

type Handler = (args: Record<string, unknown>) => unknown;

/** An open library with 10 items, the vocabulary above and one tag group. */
function library(overrides: Record<string, Handler> = {}) {
  let nextId = 0;
  return mockBackend({
    startup: () => ({ library: LIBRARY, notice: null }),
    ...viewOf(10),
    open_view: () => {
      nextId += 1;
      return { viewId: nextId, total: 10 };
    },
    item_detail: (args) => item(Number(String(args.id).slice(2)), { tags: ["Poster"] }),
    tag_vocabulary: () => VOCABULARY,
    tag_groups: () => GROUPS,
    ...overrides,
  });
}

function lastQuery(calls: ReturnType<typeof mockBackend>): LibraryQuery {
  return called(calls, "open_view").at(-1)?.args.query as LibraryQuery;
}

function details() {
  return screen.getByRole("complementary", { name: "Details" });
}

async function openTagsFilter(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: /^Filter/ }));
  const bar = screen.getByRole("toolbar", { name: "Filters" });
  await user.click(within(bar).getByRole("button", { name: /^Tags/ }));
  return screen.findByRole("dialog", { name: "Tags filter" });
}

describe("Tag editor", () => {
  it("adds a tag with vocabulary suggestions and removes one, keeping the others", async () => {
    let stored: Item = item(0, { tags: ["Poster", "Swiss"] });
    const calls = library({
      item_detail: () => stored,
      apply_item_edit: (args) => {
        const edit = args.edit as { type: string; value: string[] };
        const tags =
          edit.type === "addTags"
            ? [...stored.tags, ...edit.value]
            : stored.tags.filter((tag) => !edit.value.includes(tag));
        stored = { ...stored, tags };
        return stored;
      },
    });
    const user = userEvent.setup();
    render(<App />);
    fireEvent.mouseDown(await screen.findByRole("gridcell", { name: "image-0" }));
    const panel = details();
    const field = await within(panel).findByRole("combobox", { name: "Add a tag" });

    // Suggestions ignore case, but each keeps its own case; the item's own tags are left out.
    await user.type(field, "post");
    const suggestions = screen.getByRole("listbox", { name: "Suggestions" });
    expect(
      within(suggestions)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual(["poster1", "Create “post”"]);
    await user.click(within(suggestions).getByRole("option", { name: "poster" }));
    expect(called(calls, "apply_item_edit")[0]?.args).toEqual({
      id: "ID0000",
      edit: { type: "addTags", value: ["poster"] },
    });
    const chips = await within(panel).findByRole("list", { name: "Tags" });
    await waitFor(() =>
      expect(
        within(chips)
          .getAllByRole("listitem")
          .map((chip) => chip.textContent),
      ).toEqual(["Poster", "Swiss", "poster"]),
    );

    // A new name is taken exactly as typed.
    await user.type(field, "Brutalism{Enter}");
    expect(called(calls, "apply_item_edit")[1]?.args.edit).toEqual({
      type: "addTags",
      value: ["Brutalism"],
    });

    await user.click(within(panel).getByRole("button", { name: "Remove Swiss" }));
    expect(called(calls, "apply_item_edit")[2]?.args.edit).toEqual({
      type: "removeTags",
      value: ["Swiss"],
    });
    await waitFor(() =>
      expect(
        within(chips)
          .getAllByRole("listitem")
          .map((chip) => chip.textContent),
      ).toEqual(["Poster", "poster", "Brutalism"]),
    );
    // Added tags become recent tags in the left bar.
    const recent = screen.getByRole("region", { name: "Recent tags" });
    expect(within(recent).getByRole("button", { name: "poster" })).toBeInTheDocument();
  });

  it("rolls the chip back when the write fails", async () => {
    let attempts = 0;
    library({
      item_detail: () => item(0, { tags: ["Poster"] }),
      apply_item_edit: () => {
        attempts += 1;
        throw { kind: "readOnly" };
      },
    });
    const user = userEvent.setup();
    render(<App />);
    fireEvent.mouseDown(await screen.findByRole("gridcell", { name: "image-0" }));
    const panel = details();
    await user.type(
      await within(panel).findByRole("combobox", { name: "Add a tag" }),
      "Swiss{Enter}",
    );
    expect(attempts).toBe(1);
    await waitFor(() =>
      expect(
        within(within(panel).getByRole("list", { name: "Tags" }))
          .getAllByRole("listitem")
          .map((chip) => chip.textContent),
      ).toEqual(["Poster"]),
    );
    expect(await screen.findByRole("alert")).toBeInTheDocument();
  });

  it("tags a selection, with tags on only some items dimmed", async () => {
    const calls = library({
      selection_tags: () => ({
        items: 3,
        tags: [
          { name: "Poster", imageCount: 3 },
          { name: "Swiss", imageCount: 1 },
        ],
      }),
      apply_batch_edit: () => 3,
    });
    const user = userEvent.setup();
    render(<App />);
    fireEvent.mouseDown(await screen.findByRole("gridcell", { name: "image-0" }));
    const panel = details();
    await waitFor(() =>
      expect(within(panel).getByRole("textbox", { name: "Name" })).toHaveValue("image-0"),
    );
    fireEvent.mouseDown(screen.getByRole("gridcell", { name: "image-1" }), { ctrlKey: true });
    await waitFor(() => expect(panel).toHaveTextContent("2 items selected"));
    fireEvent.mouseDown(screen.getByRole("gridcell", { name: "image-2" }), { ctrlKey: true });
    expect(await within(panel).findByText("3 items selected")).toBeInTheDocument();
    const chips = await within(panel).findByRole("list", { name: "Tags" });
    const swiss = within(chips).getByTitle("On 1 of 3 items");
    expect(swiss).toHaveTextContent("Swiss");
    expect(within(chips).getByText("Poster").closest("li")).not.toHaveAttribute("title");
    expect(called(calls, "selection_tags").at(-1)?.args.target).toEqual({
      viewId: 1,
      all: false,
      ids: ["ID0000", "ID0001", "ID0002"],
    });

    await user.type(within(panel).getByRole("combobox", { name: "Add a tag" }), "Logo{Enter}");
    expect(called(calls, "apply_batch_edit")[0]?.args).toEqual({
      target: { viewId: 1, all: false, ids: ["ID0000", "ID0001", "ID0002"] },
      edit: { type: "addTags", value: ["Logo"] },
    });
    await user.click(
      within(chips).getByRole("button", { name: "Add Swiss to every selected item" }),
    );
    expect(called(calls, "apply_batch_edit")[1]?.args.edit).toEqual({
      type: "addTags",
      value: ["Swiss"],
    });
    await user.click(within(chips).getByRole("button", { name: "Remove Poster" }));
    expect(called(calls, "apply_batch_edit")[2]?.args.edit).toEqual({
      type: "removeTags",
      value: ["Poster"],
    });
  });

  it("a read-only library shows tags without editing them", async () => {
    library({ startup: () => ({ library: { ...LIBRARY, readOnly: true }, notice: null }) });
    render(<App />);
    fireEvent.mouseDown(await screen.findByRole("gridcell", { name: "image-0" }));
    const panel = details();
    expect(await within(panel).findByText("Poster")).toBeInTheDocument();
    expect(within(panel).queryByRole("combobox")).not.toBeInTheDocument();
    expect(within(panel).queryByRole("button", { name: "Remove Poster" })).not.toBeInTheDocument();
  });
});

describe("Tags filter", () => {
  it("filters by exact tags in each mode and clears", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    const sheet = await openTagsFilter(user);
    const all = within(sheet).getByRole("listbox", { name: "All tags" });
    // Counts per tag; "Poster" and "poster" are two tags.
    expect(
      within(all)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual(["Logo4", "Poster3", "poster1", "Swiss2"]);
    await user.click(within(all).getByRole("option", { name: "Poster" }));
    await user.click(within(all).getByRole("option", { name: "Swiss" }));
    await waitFor(() =>
      expect(lastQuery(calls)).toMatchObject({ tags: ["Poster", "Swiss"], tagMode: "any" }),
    );
    for (const [label, mode] of [
      ["All", "all"],
      ["Exact", "exact"],
      ["None", "none"],
    ] as const) {
      await user.click(within(sheet).getByRole("button", { name: label }));
      await waitFor(() => expect(lastQuery(calls).tagMode).toBe(mode));
    }
    expect(lastQuery(calls).tags).toEqual(["Poster", "Swiss"]);
    expect(screen.getByRole("button", { name: "Filter (1)" })).toBeInTheDocument();

    // The search ignores case; the filter does not.
    await user.type(within(sheet).getByRole("textbox", { name: "Find a tag" }), "POST");
    expect(
      within(all)
        .getAllByRole("option")
        .map((option) => option.getAttribute("aria-label")),
    ).toEqual(["Poster", "poster"]);

    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Clear all" }));
    await waitFor(() => expect(lastQuery(calls).tags).toEqual([]));
    expect(screen.getByRole("button", { name: "Filter" })).toBeInTheDocument();
  });

  it("offers recent tags and groups in the sheet", async () => {
    localStorage.setItem(
      `egl-viewer.recentTags:${LIBRARY.path}`,
      JSON.stringify(["Swiss", "Gone"]),
    );
    library();
    const user = userEvent.setup();
    render(<App />);
    const sheet = await openTagsFilter(user);
    // A recent tag no longer in the vocabulary is left out.
    const recent = within(sheet).getByRole("listbox", { name: "Recent tags" });
    expect(
      within(recent)
        .getAllByRole("option")
        .map((option) => option.getAttribute("aria-label")),
    ).toEqual(["Swiss"]);
    const group = within(sheet).getByRole("listbox", { name: "Kinds" });
    expect(
      within(group)
        .getAllByRole("option")
        .map((option) => option.getAttribute("aria-label")),
    ).toEqual(["Logo", "Poster"]);
  });

  it("shows an empty result with Clear filters", async () => {
    let filtered = false;
    library({
      open_view: (args) => {
        filtered = (args.query as LibraryQuery).tags.length > 0;
        return { viewId: filtered ? 2 : 1, total: filtered ? 0 : 10 };
      },
    });
    const user = userEvent.setup();
    render(<App />);
    const sheet = await openTagsFilter(user);
    await user.click(
      within(within(sheet).getByRole("listbox", { name: "All tags" })).getByRole("option", {
        name: "Logo",
      }),
    );
    await user.keyboard("{Escape}");
    expect(await screen.findByText("No items match the filters.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => expect(filtered).toBe(false));
  });
});

describe("Rename, merge and delete", () => {
  it("renames a tag everywhere and the filter follows", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    const sheet = await openTagsFilter(user);
    const all = within(sheet).getByRole("listbox", { name: "All tags" });
    await user.click(within(all).getByRole("option", { name: "Poster" }));
    fireEvent.contextMenu(within(all).getByRole("option", { name: "Poster" }));
    await user.click(await screen.findByRole("menuitem", { name: "Rename…" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename “Poster”" });
    const name = within(dialog).getByRole("textbox", { name: "New name" });

    // Onto an existing tag, exact strings: "poster" exists, so the two would merge.
    await user.clear(name);
    await user.type(name, "poster");
    expect(within(dialog).getByRole("status")).toHaveTextContent(
      "“poster” already exists: the two tags will be merged.",
    );
    expect(within(dialog).getByRole("button", { name: "Merge" })).toBeEnabled();
    await user.clear(name);
    await user.type(name, "Posters{Enter}");
    expect(called(calls, "rename_tag")[0]?.args).toEqual({ from: "Poster", to: "Posters" });
    await waitFor(() => expect(lastQuery(calls).tags).toEqual(["Posters"]));
    expect(screen.queryByRole("dialog", { name: /Rename/ })).not.toBeInTheDocument();
  });

  it("merges into an existing tag", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    const sheet = await openTagsFilter(user);
    const all = within(sheet).getByRole("listbox", { name: "All tags" });
    fireEvent.contextMenu(within(all).getByRole("option", { name: "poster" }));
    await user.click(await screen.findByRole("menuitem", { name: "Merge Into…" }));
    const dialog = await screen.findByRole("dialog", { name: "Merge “poster” into…" });
    expect(dialog).toHaveTextContent("1 item with “poster” get the other tag");
    const merge = within(dialog).getByRole("button", { name: "Merge" });
    expect(merge).toBeDisabled();
    // Only existing tags: a new name is not offered.
    await user.type(within(dialog).getByRole("combobox", { name: "Merge into" }), "Pos");
    const suggestions = screen.getByRole("listbox", { name: "Suggestions" });
    expect(
      within(suggestions)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual(["Poster3"]);
    await user.click(within(suggestions).getByRole("option", { name: "Poster" }));
    await user.click(merge);
    expect(called(calls, "merge_tags")[0]?.args).toEqual({ source: "poster", target: "Poster" });
  });

  it("deletes a tag after a confirmation that names it and its item count", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    const recent = await screen.findByRole("region", { name: "Tag groups" });
    fireEvent.contextMenu(await within(recent).findByRole("button", { name: "Logo in Kinds" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete…" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete the tag “Logo”?" });
    expect(dialog).toHaveTextContent("It is removed from 4 items");
    await user.click(within(dialog).getByRole("button", { name: "Delete tag" }));
    expect(called(calls, "delete_tag")[0]?.args).toEqual({ name: "Logo" });
  });

  it("a read-only library has no tag menu", async () => {
    library({ startup: () => ({ library: { ...LIBRARY, readOnly: true }, notice: null }) });
    const user = userEvent.setup();
    render(<App />);
    const sheet = await openTagsFilter(user);
    fireEvent.contextMenu(
      within(within(sheet).getByRole("listbox", { name: "All tags" })).getByRole("option", {
        name: "Poster",
      }),
    );
    expect(screen.queryByRole("menuitem", { name: "Rename…" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New tag group" })).toBeDisabled();
  });
});

describe("Left bar", () => {
  it("a tag of a group opens All filtered by it", async () => {
    const calls = library({ folder_tree: () => [] });
    const user = userEvent.setup();
    render(<App />);
    const groups = await screen.findByRole("region", { name: "Tag groups" });
    await user.click(await within(groups).findByRole("button", { name: "Logo in Kinds" }));
    await waitFor(() =>
      expect(lastQuery(calls)).toMatchObject({
        scope: { kind: "all" },
        tags: ["Logo"],
        tagMode: "any",
      }),
    );
    // The filter bar shows on its own while a filter is on.
    expect(screen.getByRole("toolbar", { name: "Filters" })).toHaveTextContent("Tags: Logo · Any");
    // Groups collapse and stay collapsed.
    await user.click(within(groups).getByRole("button", { name: "Kinds" }));
    expect(within(groups).queryByRole("button", { name: "Logo in Kinds" })).not.toBeInTheDocument();
  });

  it("creates, edits and deletes a tag group", async () => {
    const calls = library({
      save_tag_group: (args) => {
        const change = args.change as TagGroupChange;
        if (change.type === "update") return GROUPS;
        return [...GROUPS, { id: "G2", name: change.name, color: change.color, tags: change.tags }];
      },
      delete_tag_group: () => [],
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "New tag group" }));
    let dialog = await screen.findByRole("dialog", { name: "New tag group" });
    await user.type(within(dialog).getByRole("textbox", { name: "Group name" }), "Type");
    await user.click(within(dialog).getByRole("button", { name: "blue" }));
    const add = within(dialog).getByRole("combobox", { name: "Add a tag to the group" });
    await user.type(add, "Swiss{Enter}");
    await user.type(add, "Grotesk{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Remove Swiss" }));
    await user.click(within(dialog).getByRole("button", { name: "Create" }));
    expect(called(calls, "save_tag_group")[0]?.args.change).toEqual({
      type: "create",
      name: "Type",
      color: "blue",
      tags: ["Grotesk"],
    });
    const groups = screen.getByRole("region", { name: "Tag groups" });
    expect(await within(groups).findByRole("button", { name: "Type" })).toBeInTheDocument();

    fireEvent.contextMenu(within(groups).getByRole("button", { name: "Kinds" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit Group…" }));
    dialog = await screen.findByRole("dialog", { name: "Edit “Kinds”" });
    await user.click(within(dialog).getByRole("button", { name: "Remove Logo" }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(called(calls, "save_tag_group")[1]?.args.change).toEqual({
      type: "update",
      id: "G1",
      name: "Kinds",
      color: "red",
      tags: ["Poster"],
    });

    fireEvent.contextMenu(within(groups).getByRole("button", { name: "Kinds" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete Group…" }));
    dialog = await screen.findByRole("dialog", { name: "Delete the group “Kinds”?" });
    await user.click(within(dialog).getByRole("button", { name: "Delete group" }));
    expect(called(calls, "delete_tag_group")[0]?.args).toEqual({ id: "G1" });
  });
});
