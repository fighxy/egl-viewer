import { emit } from "@tauri-apps/api/event";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "@/App";
import { DEFAULT_PREFERENCES, type Folder, type LibraryQuery } from "@/ipc";
import {
  called,
  fakeLayout,
  folder,
  item,
  LIBRARY,
  listening,
  mockBackend,
  progress,
  viewOf,
} from "@/test/backend";

let restoreLayout: () => void;
beforeEach(() => {
  restoreLayout = fakeLayout(800, 600);
});
afterEach(() => {
  restoreLayout();
  Reflect.deleteProperty(document, "elementFromPoint");
  localStorage.clear();
});

const TREE: Folder[] = [
  folder("F1", "Posters", {
    imageCount: 2,
    descendantImageCount: 5,
    children: [folder("F2", "Swiss", { imageCount: 3, descendantImageCount: 3, iconColor: "red" })],
  }),
  folder("F3", "Fonts", { imageCount: 1, descendantImageCount: 1 }),
];

type Handler = (args: Record<string, unknown>) => unknown;

/** An open library with 20 items and the folders of `tree`. */
function library(tree: Folder[] = TREE, overrides: Record<string, Handler> = {}) {
  let nextId = 0;
  return mockBackend({
    startup: () => ({ library: LIBRARY, notice: null }),
    ...viewOf(20),
    open_view: () => {
      nextId += 1;
      return { viewId: nextId, total: 20 };
    },
    item_detail: (args) => item(Number(String(args.id).slice(2))),
    folder_tree: () => tree,
    save_folder: () => tree,
    ...overrides,
  });
}

function lastQuery(calls: ReturnType<typeof mockBackend>): LibraryQuery {
  return called(calls, "open_view").at(-1)?.args.query as LibraryQuery;
}

function treeItem(name: string) {
  return screen.findByRole("treeitem", { name });
}

/** Makes `element` the one under the pointer while a drag moves. */
function pointAt(element: Element) {
  Object.defineProperty(document, "elementFromPoint", {
    configurable: true,
    value: () => element,
  });
}

/** Presses on `from`, moves past the drag threshold over `to`, and releases. */
function drag(from: Element, to: Element) {
  pointAt(to);
  fireEvent.mouseDown(from, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.mouseMove(window, { clientX: 30, clientY: 30 });
  fireEvent.mouseMove(window, { clientX: 40, clientY: 40 });
  fireEvent.mouseUp(window);
}

describe("The folder tree", () => {
  it("shows folders with counts, opens one, and includes subfolders on request", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    const posters = await treeItem("Posters");
    expect(posters).toHaveTextContent("2");
    expect(screen.queryByRole("treeitem", { name: "Swiss" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Expand Posters" }));
    await user.click(await treeItem("Swiss"));
    await waitFor(() =>
      expect(lastQuery(calls).scope).toEqual({
        kind: "folder",
        id: "F2",
        includeDescendants: false,
      }),
    );
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Posters / Swiss");
    expect(await treeItem("Swiss")).toHaveAttribute("aria-selected", "true");
    // The tree remembers what was expanded.
    expect(localStorage.getItem(`egl-viewer.expanded:${LIBRARY.path}`)).toBe('["F1"]');

    await user.click(screen.getByRole("button", { name: "Include subfolders" }));
    await waitFor(() => expect(lastQuery(calls).scope).toMatchObject({ includeDescendants: true }));
    expect(posters).toHaveTextContent("5");
    expect(called(calls, "set_preferences").at(-1)?.args.preferences).toMatchObject({
      browse: { includeSubfolders: true },
    });
  });

  it("moves through the tree with the keyboard", async () => {
    library();
    render(<App />);
    const posters = await treeItem("Posters");
    posters.focus();
    fireEvent.keyDown(posters, { key: "ArrowRight" });
    const swiss = await treeItem("Swiss");
    fireEvent.keyDown(posters, { key: "ArrowDown" });
    await waitFor(() => expect(swiss).toHaveFocus());
    fireEvent.keyDown(swiss, { key: "ArrowLeft" });
    await waitFor(() => expect(posters).toHaveFocus());
    fireEvent.keyDown(posters, { key: "Enter" });
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Posters"),
    );
  });

  it("creates a folder and opens it", async () => {
    const created = [...TREE, folder("F9", "Moodboards")];
    const calls = library(TREE, { save_folder: () => created });
    const user = userEvent.setup();
    render(<App />);
    await treeItem("Posters");
    await user.click(screen.getByRole("button", { name: "New folder" }));
    const dialog = await screen.findByRole("dialog", { name: "New folder" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Moodboards");
    await user.click(within(dialog).getByRole("button", { name: "Create" }));
    expect(called(calls, "save_folder")[0]?.args.change).toEqual({
      type: "create",
      name: "Moodboards",
      parent: null,
      description: "",
      iconColor: null,
    });
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Moodboards"),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("renames in place and always sends the full folder", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    const fonts = await treeItem("Fonts");
    fonts.focus();
    fireEvent.keyDown(fonts, { key: "F2" });
    const field = await screen.findByRole("textbox", { name: "Folder name" });
    await user.clear(field);
    await user.type(field, "Type{Enter}");
    expect(called(calls, "save_folder")[0]?.args.change).toEqual({
      type: "update",
      id: "F3",
      name: "Type",
      description: "",
      tags: [],
      iconColor: null,
      orderBy: null,
      sortAscending: null,
    });
  });

  it("deletes a folder, optionally with the items only it holds", async () => {
    const calls = library(TREE, { delete_folder: () => [TREE[1]] });
    const user = userEvent.setup();
    render(<App />);
    const posters = await treeItem("Posters");
    await user.click(posters);
    fireEvent.keyDown(posters, { key: "Delete" });
    const dialog = await screen.findByRole("dialog", { name: "Delete “Posters”?" });
    expect(dialog).toHaveTextContent("Its subfolder goes too.");
    expect(dialog).toHaveTextContent("5 items are in it; they stay in the library.");
    await user.click(within(dialog).getByRole("checkbox"));
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect(called(calls, "delete_folder")[0]?.args).toEqual({ id: "F1", trashItems: true });
    // The open folder is gone: back to All.
    await waitFor(() => expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("All"));
    expect(screen.queryByRole("treeitem", { name: "Posters" })).not.toBeInTheDocument();
  });

  it("moves a folder with Move To, never into its own subfolders", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    fireEvent.contextMenu(await treeItem("Posters"));
    await user.click(await screen.findByRole("menuitem", { name: "Move To…" }));
    const dialog = await screen.findByRole("dialog", { name: "Move “Posters”" });
    expect(within(dialog).getByRole("option", { name: "Posters" })).toBeDisabled();
    expect(within(dialog).getByRole("option", { name: "Swiss" })).toBeDisabled();
    await user.click(within(dialog).getByRole("option", { name: "Fonts" }));
    expect(called(calls, "save_folder")[0]?.args.change).toEqual({
      type: "move",
      id: "F1",
      parent: "F3",
      index: 0,
    });
  });
});

describe("Folder properties", () => {
  it("edits the open folder in the right bar when nothing is selected", async () => {
    const manual = [
      TREE[0] as Folder,
      folder("F3", "Fonts", { orderBy: "manual", sortAscending: true }),
    ];
    let tree = TREE;
    const calls = library(TREE, {
      folder_tree: () => tree,
      save_folder: (args) => {
        const change = args.change as { orderBy?: string };
        if (change.orderBy === "manual") tree = manual;
        return tree;
      },
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await treeItem("Fonts"));
    const details = screen.getByRole("complementary", { name: "Details" });
    expect(within(details).getByRole("textbox", { name: "Name" })).toHaveValue("Fonts");

    const description = within(details).getByRole("textbox", { name: "Description" });
    await user.type(description, "Type specimens");
    await user.tab();
    expect(called(calls, "save_folder")[0]?.args.change).toMatchObject({
      type: "update",
      id: "F3",
      name: "Fonts",
      description: "Type specimens",
    });

    await user.selectOptions(within(details).getByRole("combobox", { name: "Sort" }), "manual:asc");
    expect(called(calls, "save_folder")[1]?.args.change).toMatchObject({
      orderBy: "manual",
      sortAscending: true,
    });
    await waitFor(() =>
      expect(lastQuery(calls)).toMatchObject({ sort: "manual", ascending: true }),
    );
  });

  it("makes Manual the folder's own sort, and holds other picks for the screen only", async () => {
    let tree = TREE;
    const calls = library(TREE, {
      folder_tree: () => tree,
      save_folder: () => {
        tree = [
          TREE[0] as Folder,
          folder("F3", "Fonts", { orderBy: "manual", sortAscending: true }),
        ];
        return tree;
      },
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await treeItem("Fonts"));
    await user.click(screen.getByRole("button", { name: "Sort by Date Added" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Manual" }));
    expect(called(calls, "save_folder")[0]?.args.change).toMatchObject({
      id: "F3",
      orderBy: "manual",
      sortAscending: true,
    });
    await waitFor(() => expect(lastQuery(calls).sort).toBe("manual"));

    const saved = called(calls, "set_preferences").length;
    await user.click(screen.getByRole("button", { name: "Sort by Manual" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Title" }));
    await waitFor(() => expect(lastQuery(calls).sort).toBe("name"));
    expect(called(calls, "set_preferences")).toHaveLength(saved);
    expect(called(calls, "save_folder")).toHaveLength(1);
  });
});

describe("Folder membership", () => {
  it("adds and removes the folders of the selected items", async () => {
    const calls = library(TREE, {
      item_detail: (args) => item(Number(String(args.id).slice(2)), { folders: ["F3"] }),
      apply_item_edit: (args) => item(0, { folders: [], id: String(args.id) }),
    });
    const user = userEvent.setup();
    render(<App />);
    fireEvent.mouseDown(await screen.findByRole("gridcell", { name: "image-0" }));
    const details = screen.getByRole("complementary", { name: "Details" });
    await within(details).findByText("Fonts");

    await user.click(within(details).getByRole("button", { name: "Remove from Fonts" }));
    expect(called(calls, "apply_item_edit")[0]?.args).toEqual({
      id: "ID0000",
      edit: { type: "removeFromFolders", value: ["F3"] },
    });

    await user.click(within(details).getByRole("button", { name: "Add" }));
    await user.click(await screen.findByRole("option", { name: "Posters" }));
    expect(called(calls, "apply_item_edit")[1]?.args).toEqual({
      id: "ID0000",
      edit: { type: "addToFolders", value: ["F1"] },
    });

    fireEvent.keyDown(screen.getByRole("grid", { name: "Items" }), { key: "a", ctrlKey: true });
    await user.click(await within(details).findByRole("button", { name: "Add to folder…" }));
    await user.click(await screen.findByRole("option", { name: "Fonts" }));
    expect(called(calls, "apply_batch_edit")[0]?.args).toEqual({
      target: { viewId: 1, all: true, ids: [] },
      edit: { type: "addToFolders", value: ["F3"] },
    });
  });

  it("adds files to the open folder, and a duplicate can join it", async () => {
    const calls = library(TREE, { "plugin:dialog|open": () => ["C:\\in\\a.jpg"] });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await treeItem("Fonts"));
    await user.click(screen.getByRole("button", { name: "Add files" }));
    await waitFor(() =>
      expect(called(calls, "add_files")[0]?.args).toEqual({
        paths: ["C:\\in\\a.jpg"],
        folder: "F3",
      }),
    );

    await listening("import-progress");
    await act(() =>
      emit(
        "import-progress",
        progress({
          state: "waitingForDecision",
          duplicate: { path: "C:\\in\\a.jpg", existing: "01J", inTrash: false, targetFolder: "F3" },
        }),
      ),
    );
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("“Add to Fonts” puts the existing items into that folder.");
    await user.click(within(dialog).getByRole("button", { name: "Add to Fonts" }));
    expect(called(calls, "answer_import")[0]?.args).toEqual({ id: 1, answer: "addToFolder" });
  });
});

describe("Dragging", () => {
  it("drops the selected items on a folder", async () => {
    const calls = library();
    render(<App />);
    const cell = await screen.findByRole("gridcell", { name: "image-1" });
    const fonts = await treeItem("Fonts");
    drag(cell, fonts);
    await waitFor(() =>
      expect(called(calls, "apply_batch_edit")[0]?.args).toEqual({
        target: { viewId: 1, all: false, ids: ["ID0001"] },
        edit: { type: "addToFolders", value: ["F3"] },
      }),
    );
  });

  it("keeps a selection pressed for a drag, and a click selects one item", async () => {
    library();
    render(<App />);
    const cell = await screen.findByRole("gridcell", { name: "image-1" });
    const grid = screen.getByRole("grid", { name: "Items" });
    fireEvent.keyDown(grid, { key: "a", ctrlKey: true });
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("20 selected"));
    fireEvent.mouseDown(cell, { button: 0, clientX: 10, clientY: 10 });
    expect(screen.getByRole("contentinfo")).toHaveTextContent("20 selected");
    fireEvent.mouseUp(window);
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));
  });

  it("moves a folder into another one, and Escape cancels a drag", async () => {
    const calls = library();
    render(<App />);
    const fonts = await treeItem("Fonts");
    const posters = await treeItem("Posters");

    pointAt(posters);
    fireEvent.mouseDown(fonts, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(window, { clientX: 30, clientY: 30 });
    expect(await screen.findByText("Fonts", { selector: "div[aria-hidden]" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.mouseUp(window);
    expect(called(calls, "save_folder")).toHaveLength(0);

    drag(fonts, posters);
    await waitFor(() =>
      expect(called(calls, "save_folder")[0]?.args.change).toEqual({
        type: "move",
        id: "F3",
        parent: "F1",
        index: 1,
      }),
    );
  });

  it("reorders items in a folder's manual order", async () => {
    const manual = [
      TREE[0] as Folder,
      folder("F3", "Fonts", { orderBy: "manual", sortAscending: true }),
    ];
    const calls = library(manual);
    const user = userEvent.setup();
    render(<App />);
    await user.click(await treeItem("Fonts"));
    await waitFor(() => expect(lastQuery(calls).sort).toBe("manual"));
    await waitFor(() =>
      expect(screen.getByRole("gridcell", { name: "image-0" })).toHaveAttribute(
        "data-drop",
        "cell",
      ),
    );
    drag(
      screen.getByRole("gridcell", { name: "image-2" }),
      screen.getByRole("gridcell", { name: "image-0" }),
    );
    await waitFor(() =>
      expect(called(calls, "reorder_items")[0]?.args).toEqual({
        folder: "F3",
        ids: ["ID0002"],
        before: "ID0000",
      }),
    );
  });

  it("reorders with Include subfolders on only in a folder without subfolders", async () => {
    const tree = [
      folder("F1", "Posters", {
        orderBy: "manual",
        sortAscending: true,
        children: [folder("F2", "Swiss")],
      }),
      folder("F3", "Fonts", { orderBy: "manual", sortAscending: true }),
    ];
    const calls = library(tree, {
      get_preferences: () => ({
        ...DEFAULT_PREFERENCES,
        browse: { ...DEFAULT_PREFERENCES.browse, includeSubfolders: true },
      }),
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await treeItem("Posters"));
    await waitFor(() =>
      expect(lastQuery(calls).scope).toMatchObject({ id: "F1", includeDescendants: true }),
    );
    expect(await screen.findByRole("gridcell", { name: "image-0" })).not.toHaveAttribute(
      "data-drop",
    );
    expect(screen.getByRole("complementary", { name: "Details" })).toHaveTextContent(
      "Turn off Include subfolders to drag items into a new order.",
    );
    await user.click(await treeItem("Fonts"));
    await waitFor(() =>
      expect(screen.getByRole("gridcell", { name: "image-0" })).toHaveAttribute(
        "data-drop",
        "cell",
      ),
    );
  });

  it("offers no reordering outside a manual order", async () => {
    library();
    render(<App />);
    const cell = await screen.findByRole("gridcell", { name: "image-0" });
    expect(cell).not.toHaveAttribute("data-drop");
  });
});
