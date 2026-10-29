import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "@/App";
import type { LibraryQuery, SmartFolder, SmartFolderChange } from "@/ipc";
import { called, fakeLayout, folder, item, LIBRARY, mockBackend, viewOf } from "@/test/backend";

let restoreLayout: () => void;
beforeEach(() => {
  restoreLayout = fakeLayout(800, 600);
});
afterEach(() => {
  restoreLayout();
  localStorage.clear();
});

const POSTERS: SmartFolder = {
  id: "S1",
  name: "Posters",
  conditions: [
    {
      match: "AND",
      boolean: "TRUE",
      rules: [{ property: "tags", method: "union", value: ["Poster"] }],
    },
  ],
  orderBy: null,
  sortAscending: null,
  unsupported: [],
  children: [
    {
      id: "S2",
      name: "Red",
      conditions: [
        {
          match: "OR",
          rules: [
            { property: "color", method: "similar", value: "#ff0000" },
            { property: "rating", method: "equal", value: "5" },
          ],
        },
      ],
      orderBy: null,
      sortAscending: null,
      unsupported: ["color similar"],
      children: [],
    },
  ],
};

type Handler = (args: Record<string, unknown>) => unknown;

function library(overrides: Record<string, Handler> = {}) {
  let nextId = 0;
  return mockBackend({
    startup: () => ({ library: LIBRARY, notice: null }),
    ...viewOf(10),
    open_view: () => {
      nextId += 1;
      return { viewId: nextId, total: 10 };
    },
    item_detail: (args) => item(Number(String(args.id).slice(2))),
    folder_tree: () => [folder("F1", "Posters"), folder("F2", "Fonts")],
    tag_vocabulary: () => [{ name: "Poster", imageCount: 3 }],
    extensions: () => [
      { ext: "png", count: 7 },
      { ext: "jpg", count: 3 },
    ],
    smart_folders: () => [POSTERS],
    ...overrides,
  });
}

function lastQuery(calls: ReturnType<typeof mockBackend>): LibraryQuery {
  return called(calls, "open_view").at(-1)?.args.query as LibraryQuery;
}

async function openFilterBar(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: /^Filter/ }));
  return screen.getByRole("toolbar", { name: "Filters" });
}

describe("Search", () => {
  it("searches after a pause, inside the place, and clears with Escape", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("gridcell", { name: "image-0" });
    // Ctrl+F reaches the field.
    fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    const field = screen.getByRole("searchbox", { name: "Search" });
    expect(field).toHaveFocus();
    await user.type(field, "poster -draft");
    await waitFor(() => expect(lastQuery(calls).text).toBe("poster -draft"));
    // A folder keeps the search, and the search stays inside the folder.
    await user.click(screen.getByRole("treeitem", { name: "Posters" }));
    await waitFor(() =>
      expect(lastQuery(calls)).toMatchObject({
        scope: { kind: "folder", id: "F1" },
        text: "poster -draft",
      }),
    );
    field.focus();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(lastQuery(calls).text).toBeNull());
    expect(field).toHaveValue("");
  });

  it("an empty search result offers to clear", async () => {
    let searching = false;
    const calls = library({
      open_view: (args) => {
        searching = Boolean((args.query as LibraryQuery).text);
        return { viewId: searching ? 2 : 1, total: searching ? 0 : 10 };
      },
    });
    const user = userEvent.setup();
    render(<App />);
    await user.type(await screen.findByRole("searchbox", { name: "Search" }), "nothing{Enter}");
    expect(await screen.findByText("No items match the search.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => expect(lastQuery(calls).text).toBeNull());
  });
});

describe("Filter bar", () => {
  it("filters by type, rating, shape and folders, and clears them all", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    let bar = await openFilterBar(user);
    await user.click(within(bar).getByRole("button", { name: "Type" }));
    await user.click(await screen.findByRole("menuitemradio", { name: /PNG/ }));
    await waitFor(() => expect(lastQuery(calls).ext).toBe("png"));
    bar = screen.getByRole("toolbar", { name: "Filters" });
    await user.click(within(bar).getByRole("button", { name: "Rating" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Unrated" }));
    await waitFor(() => expect(lastQuery(calls).rating).toBe(0));
    await user.click(within(bar).getByRole("button", { name: "Shape" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Panoramic landscape" }));
    await waitFor(() => expect(lastQuery(calls).shape).toBe("panoramic-landscape"));
    await user.click(within(bar).getByRole("button", { name: "Folders" }));
    const panel = await screen.findByRole("dialog", { name: "Folders filter" });
    await user.click(within(panel).getByRole("option", { name: "Fonts" }));
    await user.click(within(panel).getByRole("button", { name: "None" }));
    await waitFor(() =>
      expect(lastQuery(calls)).toMatchObject({ extraFolderIds: ["F2"], folderMode: "none" }),
    );
    await user.keyboard("{Escape}");
    expect(screen.getByRole("button", { name: "Filter (4)" })).toBeInTheDocument();
    expect(bar).toHaveTextContent("Type: PNG");
    expect(bar).toHaveTextContent("Rating: Unrated");
    expect(bar).toHaveTextContent("Folders: Fonts · None");
    await user.click(within(bar).getByRole("button", { name: "Clear all" }));
    await waitFor(() =>
      expect(lastQuery(calls)).toMatchObject({
        ext: null,
        rating: null,
        shape: null,
        extraFolderIds: [],
      }),
    );
  });
});

describe("Smart folders", () => {
  it("lists the tree, warns about rules it cannot evaluate, and opens one", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    const section = await screen.findByRole("region", { name: "Smart folders" });
    // Smart subfolders show until their parent is collapsed.
    expect(
      await within(section).findByRole("button", { name: "Collapse Posters" }),
    ).toBeInTheDocument();
    expect(
      within(section).getByRole("img", { name: "Some rules are not supported" }),
    ).toHaveAttribute("title", "Not supported here, so they match nothing: color similar");
    await user.click(within(section).getByRole("button", { name: "Red" }));
    await waitFor(() => expect(lastQuery(calls).scope).toEqual({ kind: "smart", id: "S2" }));
    expect(screen.getByRole("main")).toHaveTextContent("Posters / Red");
  });

  it("saves the filters as a smart folder and opens it", async () => {
    let saved: SmartFolderChange | undefined;
    const draft = [
      {
        match: "AND",
        boolean: "TRUE",
        rules: [{ property: "tags", method: "union", value: ["Poster"] }],
      },
    ];
    const calls = library({
      smart_folders: () => [],
      smart_rules: () => ({ parent: null, conditions: draft }),
      save_smart_folder: (args) => {
        saved = args.change as SmartFolderChange;
        return [{ ...POSTERS, id: "S9", name: "Poster", children: [] }];
      },
    });
    const user = userEvent.setup();
    render(<App />);
    const bar = await openFilterBar(user);
    const save = within(bar).getByRole("button", { name: "Save as smart folder" });
    expect(save).toBeDisabled();
    expect(save).toHaveAttribute("title", "Add a filter first.");
    await user.click(within(bar).getByRole("button", { name: /^Tags/ }));
    const sheet = await screen.findByRole("dialog", { name: "Tags filter" });
    await user.click(
      within(within(sheet).getByRole("listbox", { name: "All tags" })).getByRole("option", {
        name: "Poster",
      }),
    );
    await user.keyboard("{Escape}");
    await user.click(save);
    expect(called(calls, "smart_rules")[0]?.args.query).toMatchObject({
      scope: { kind: "all" },
      tags: ["Poster"],
    });
    const dialog = await screen.findByRole("dialog", { name: "New smart folder" });
    expect(within(dialog).getByRole("textbox", { name: "Smart folder name" })).toHaveValue(
      "Poster",
    );
    const rule = within(dialog).getByRole("group", { name: "Rule 1" });
    expect(within(rule).getByRole("combobox", { name: "Property" })).toHaveValue("tags");
    expect(within(rule).getByRole("combobox", { name: "Method" })).toHaveValue("union");
    expect(rule).toHaveTextContent("Poster");
    await user.click(within(dialog).getByRole("button", { name: "Create" }));
    expect(saved).toEqual({ type: "create", name: "Poster", parent: null, conditions: draft });
    await waitFor(() =>
      expect(lastQuery(calls)).toMatchObject({ scope: { kind: "smart", id: "S9" }, tags: [] }),
    );
  });

  it("search text cannot go into a smart folder", async () => {
    library();
    const user = userEvent.setup();
    render(<App />);
    await user.type(await screen.findByRole("searchbox", { name: "Search" }), "poster{Enter}");
    const bar = await openFilterBar(user);
    const save = within(bar).getByRole("button", { name: "Save as smart folder" });
    expect(save).toBeDisabled();
    expect(save).toHaveAttribute(
      "title",
      "Search text cannot go into a smart folder: clear the search first.",
    );
  });

  it("builds rules in the editor", async () => {
    let saved: SmartFolderChange | undefined;
    library({
      smart_folders: () => [],
      save_smart_folder: (args) => {
        saved = args.change as SmartFolderChange;
        return [];
      },
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "New smart folder" }));
    const dialog = await screen.findByRole("dialog", { name: "New smart folder" });
    await user.type(within(dialog).getByRole("textbox", { name: "Smart folder name" }), "Best");
    let rule = within(dialog).getByRole("group", { name: "Rule 1" });
    await user.selectOptions(within(rule).getByRole("combobox", { name: "Property" }), "rating");
    rule = within(dialog).getByRole("group", { name: "Rule 1" });
    await user.selectOptions(within(rule).getByRole("combobox", { name: "Value" }), "4");
    await user.click(within(dialog).getByRole("button", { name: "Add condition" }));
    const second = within(dialog).getByRole("group", { name: "Condition 2" });
    let noteRule = within(second).getByRole("group", { name: "Rule 1" });
    await user.selectOptions(
      within(noteRule).getByRole("combobox", { name: "Property" }),
      "annotation",
    );
    noteRule = within(second).getByRole("group", { name: "Rule 1" });
    await user.type(within(noteRule).getByRole("textbox", { name: "Value" }), "draft");
    await user.click(within(second).getByRole("checkbox", { name: "Leave these items out" }));
    await user.click(within(second).getByRole("button", { name: "Add rule" }));
    let sizeRule = within(second).getByRole("group", { name: "Rule 2" });
    await user.selectOptions(
      within(sizeRule).getByRole("combobox", { name: "Property" }),
      "fileSize",
    );
    sizeRule = within(second).getByRole("group", { name: "Rule 2" });
    await user.selectOptions(within(sizeRule).getByRole("combobox", { name: "Method" }), "between");
    sizeRule = within(second).getByRole("group", { name: "Rule 2" });
    await user.clear(within(sizeRule).getByRole("spinbutton", { name: "To" }));
    await user.type(within(sizeRule).getByRole("spinbutton", { name: "To" }), "5");
    await user.selectOptions(within(second).getByRole("combobox", { name: "Match" }), "OR");
    await user.click(within(dialog).getByRole("button", { name: "Create" }));
    expect(saved).toEqual({
      type: "create",
      name: "Best",
      parent: null,
      conditions: [
        {
          match: "AND",
          boolean: "TRUE",
          rules: [{ property: "rating", method: "equal", value: "4" }],
        },
        {
          match: "OR",
          boolean: "FALSE",
          rules: [
            { property: "annotation", method: "contain", value: "draft" },
            { property: "fileSize", method: "between", value: [1, 5], unit: "mb" },
          ],
        },
      ],
    });
  });

  it("edits, keeps rules it cannot evaluate, and deletes", async () => {
    const calls = library({ save_smart_folder: () => [POSTERS], delete_smart_folder: () => [] });
    const user = userEvent.setup();
    render(<App />);
    const section = await screen.findByRole("region", { name: "Smart folders" });
    // Smart subfolders show until their parent is collapsed.
    expect(
      await within(section).findByRole("button", { name: "Collapse Posters" }),
    ).toBeInTheDocument();
    fireEvent.contextMenu(within(section).getByRole("button", { name: "Red" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit…" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit “Red”" });
    expect(dialog).toHaveTextContent("Inside “Posters”: its rules apply too.");
    expect(within(dialog).getByRole("group", { name: "Rule 1" })).toHaveTextContent(
      "color similar: not supported here, so it matches nothing",
    );
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(called(calls, "save_smart_folder")[0]?.args.change).toEqual({
      type: "update",
      id: "S2",
      name: "Red",
      conditions: POSTERS.children[0]?.conditions,
    });

    fireEvent.contextMenu(within(section).getByRole("button", { name: "Posters" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete…" }));
    const confirm = await screen.findByRole("dialog", {
      name: "Delete the smart folder “Posters”?",
    });
    expect(confirm).toHaveTextContent("The smart folders under it go too.");
    await user.click(within(confirm).getByRole("button", { name: "Delete smart folder" }));
    expect(called(calls, "delete_smart_folder")[0]?.args).toEqual({ id: "S1" });
  });
});
