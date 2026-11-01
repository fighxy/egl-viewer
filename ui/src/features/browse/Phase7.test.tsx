import { emit } from "@tauri-apps/api/event";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "@/App";
import { DEFAULT_PREFERENCES, type ItemEdit, type LibraryCheck, type LibraryQuery } from "@/ipc";
import {
  CLEAN_CHECK,
  called,
  cell,
  fakeLayout,
  folder,
  item,
  LIBRARY,
  listening,
  mockBackend,
  viewOf,
} from "@/test/backend";

let restoreLayout: () => void;
beforeEach(() => {
  restoreLayout = fakeLayout(800, 600);
});
afterEach(() => {
  restoreLayout();
  Reflect.deleteProperty(document, "elementFromPoint");
});

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

type Handlers = Record<string, (args: Record<string, unknown>) => unknown>;

/** A library with `total` items whose details come from `item(n)`. */
function library(total = 3, overrides: Handlers = {}) {
  return mockBackend({
    startup: () => ({ library: LIBRARY, notice: null }),
    library_counts: () => ({ all: total, uncategorized: total, untagged: total, trash: 2 }),
    ...viewOf(total),
    item_detail: (args) => item(Number(String(args.id).slice(2))),
    apply_item_edit: (args) => {
      const edit = args.edit as ItemEdit;
      const base = item(Number(String(args.id).slice(2)));
      if (edit.type === "rename") return { ...base, name: edit.value };
      if (edit.type === "setStar") return { ...base, star: edit.value };
      if (edit.type === "setUrl") return { ...base, url: edit.value };
      return base;
    },
    ...overrides,
  });
}

async function pick(name: string, options: { ctrlKey?: boolean } = {}) {
  fireEvent.mouseDown(await screen.findByRole("gridcell", { name }), options);
}

function details() {
  return screen.getByRole("complementary", { name: "Details" });
}

function lastEdit(calls: ReturnType<typeof mockBackend>, cmd: string) {
  return called(calls, cmd).at(-1)?.args;
}

describe("Phase 7: editing, the trash and file actions", () => {
  it("edits the name, rating, note and link in place", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    await pick("image-1");
    const name = await within(details()).findByRole("textbox", { name: "Name" });
    await waitFor(() => expect(name).toHaveValue("image-1"));

    // Esc brings back the stored name; Enter saves.
    await user.clear(name);
    await user.type(name, "Draft{Escape}");
    expect(name).toHaveValue("image-1");
    expect(called(calls, "apply_item_edit")).toHaveLength(0);
    await user.clear(name);
    await user.type(name, "Poster final{Enter}");
    await waitFor(() =>
      expect(lastEdit(calls, "apply_item_edit")).toEqual({
        id: "ID0001",
        edit: { type: "rename", value: "Poster final" },
      }),
    );
    // An empty name is not saved.
    await user.clear(name);
    await user.tab();
    expect(await within(details()).findByRole("alert")).toHaveTextContent("Enter a name.");
    expect(called(calls, "apply_item_edit")).toHaveLength(1);

    await user.click(within(details()).getByRole("button", { name: "Rate 4 of 5" }));
    await waitFor(() =>
      expect(lastEdit(calls, "apply_item_edit")).toEqual({
        id: "ID0001",
        edit: { type: "setStar", value: 4 },
      }),
    );

    const link = within(details()).getByRole("textbox", { name: "Link" });
    await user.type(link, "example.com{Enter}");
    expect(
      await within(details()).findByText("A link starts with http:// or https://."),
    ).toBeTruthy();
    await user.clear(link);
    await user.type(link, "https://example.com/a{Enter}");
    await waitFor(() =>
      expect(lastEdit(calls, "apply_item_edit")?.edit).toEqual({
        type: "setUrl",
        value: "https://example.com/a",
      }),
    );
    await user.click(
      await within(details()).findByRole("button", { name: "Open the link in the browser" }),
    );
    expect(lastEdit(calls, "open_link")).toEqual({ url: "https://example.com/a" });

    const note = within(details()).getByRole("textbox", { name: "Note" });
    await user.type(note, "Moodboard{Control>}{Enter}{/Control}");
    await waitFor(() =>
      expect(lastEdit(calls, "apply_item_edit")?.edit).toEqual({
        type: "setAnnotation",
        value: "Moodboard",
      }),
    );
  });

  it("rolls a field back when Rust refuses the edit", async () => {
    const calls = library(3, {
      apply_item_edit: () => {
        throw { kind: "invalid", detail: "name must not be empty" };
      },
    });
    const user = userEvent.setup();
    render(<App />);
    await pick("image-0");
    const name = await within(details()).findByRole("textbox", { name: "Name" });
    await waitFor(() => expect(name).toHaveValue("image-0"));
    await user.clear(name);
    await user.type(name, "Other{Enter}");
    await waitFor(() => expect(calls.some((call) => call.cmd === "apply_item_edit")).toBe(true));
    await waitFor(() => expect(name).toHaveValue("image-0"));
  });

  it("moves the selection to the trash after a confirmation and undoes it", async () => {
    let undo: unknown = null;
    const calls = library(3, {
      apply_batch_edit: () => {
        undo = { kind: "trash", items: 2 };
        return 2;
      },
      undo_status: () => undo,
      undo: () => {
        const was = undo;
        undo = null;
        return was;
      },
    });
    const user = userEvent.setup();
    render(<App />);
    await pick("image-0");
    await pick("image-2", { ctrlKey: true });
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("2 selected"));
    fireEvent.keyDown(screen.getByRole("grid", { name: "Items" }), { key: "Delete" });
    const dialog = await screen.findByRole("dialog", { name: "Move to the trash?" });
    expect(dialog).toHaveTextContent("2 items go to the trash");
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));
    await waitFor(() =>
      expect(lastEdit(calls, "apply_batch_edit")).toEqual({
        target: { viewId: 1, all: false, ids: ["ID0000", "ID0002"] },
        edit: { type: "trash" },
      }),
    );

    await listening("views-invalidated");
    await act(() => emit("views-invalidated", { views: [{ viewId: 1, total: 1 }] }));
    await user.click(screen.getByRole("menuitem", { name: "Edit" }));
    await user.click(await screen.findByRole("menuitem", { name: /Undo Move to Trash/ }));
    await waitFor(() => expect(called(calls, "undo")).toHaveLength(1));
    // Ctrl+Z is the same command; with nothing left it does nothing.
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(called(calls, "undo")).toHaveLength(1);
  });

  it("stops asking after Don't ask again, and skips the dialog when the setting is off", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    await pick("image-0");
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));
    fireEvent.keyDown(screen.getByRole("grid", { name: "Items" }), { key: "Delete" });
    const dialog = await screen.findByRole("dialog", { name: "Move to the trash?" });
    expect(dialog).toHaveTextContent("You can turn this back on in Settings.");
    await user.click(within(dialog).getByRole("checkbox", { name: "Don't ask again" }));
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));
    await waitFor(() =>
      expect(called(calls, "set_preferences").at(-1)?.args).toEqual({
        preferences: { ...DEFAULT_PREFERENCES, confirmBeforeTrash: false },
      }),
    );

    // The next Delete goes straight to the trash.
    await pick("image-1");
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));
    fireEvent.keyDown(screen.getByRole("grid", { name: "Items" }), { key: "Delete" });
    await waitFor(() => expect(called(calls, "apply_batch_edit")).toHaveLength(2));
    expect(screen.queryByRole("dialog", { name: "Move to the trash?" })).not.toBeInTheDocument();
  });

  it("moves to the trash without asking when the confirmation is off in Settings", async () => {
    const calls = library(3, {
      get_preferences: () => ({ ...DEFAULT_PREFERENCES, confirmBeforeTrash: false }),
    });
    render(<App />);
    await pick("image-0");
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));
    fireEvent.keyDown(screen.getByRole("grid", { name: "Items" }), { key: "Delete" });
    await waitFor(() => expect(called(calls, "apply_batch_edit")).toHaveLength(1));
    expect(screen.queryByRole("dialog", { name: "Move to the trash?" })).not.toBeInTheDocument();
    expect(called(calls, "set_preferences")).toHaveLength(0);
  });

  it("drops items on Trash with the same confirmation as Delete", async () => {
    const calls = library();
    const user = userEvent.setup();
    render(<App />);
    const trash = await screen.findByRole("button", { name: /^Trash/ });
    expect(trash).toHaveAttribute("data-drop", "trash");
    const cell = await screen.findByRole("gridcell", { name: "image-1" });
    pointAt(trash);
    fireEvent.mouseDown(cell, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(window, { clientX: 30, clientY: 30 });
    expect(trash).toHaveClass("bg-primary/15", "ring-primary");
    fireEvent.mouseUp(window);
    const dialog = await screen.findByRole("dialog", { name: "Move to the trash?" });
    expect(dialog).toHaveTextContent("1 item go to the trash");
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));
    await waitFor(() =>
      expect(lastEdit(calls, "apply_batch_edit")).toEqual({
        target: { viewId: 1, all: false, ids: ["ID0001"] },
        edit: { type: "trash" },
      }),
    );
  });

  it("drops a multi-selection on Trash without asking when confirmation is off", async () => {
    const calls = library(3, {
      get_preferences: () => ({ ...DEFAULT_PREFERENCES, confirmBeforeTrash: false }),
    });
    render(<App />);
    const cell = await screen.findByRole("gridcell", { name: "image-0" });
    fireEvent.mouseDown(cell);
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));
    fireEvent.mouseDown(screen.getByRole("gridcell", { name: "image-1" }), { ctrlKey: true });
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("2 selected"));
    drag(cell, await screen.findByRole("button", { name: /^Trash/ }));
    await waitFor(() =>
      expect(lastEdit(calls, "apply_batch_edit")).toEqual({
        target: { viewId: 1, all: false, ids: ["ID0000", "ID0001"] },
        edit: { type: "trash" },
      }),
    );
    expect(screen.queryByRole("dialog", { name: "Move to the trash?" })).not.toBeInTheDocument();
  });

  it("a folder dragged onto Trash is not a trash edit", async () => {
    const calls = library(3, {
      folder_tree: () => [folder("F1", "Posters")],
    });
    render(<App />);
    const trash = await screen.findByRole("button", { name: /^Trash/ });
    drag(await screen.findByRole("treeitem", { name: "Posters" }), trash);
    expect(trash).not.toHaveClass("ring-primary");
    expect(called(calls, "apply_batch_edit")).toHaveLength(0);
    expect(called(calls, "save_folder")).toHaveLength(0);
  });

  it("a read-only library refuses a drop on Trash", async () => {
    const calls = library(3, {
      startup: () => ({ library: { ...LIBRARY, readOnly: true }, notice: null }),
    });
    render(<App />);
    const trash = await screen.findByRole("button", { name: /^Trash/ });
    drag(await screen.findByRole("gridcell", { name: "image-0" }), trash);
    expect(trash).not.toHaveClass("ring-primary");
    expect(called(calls, "apply_batch_edit")).toHaveLength(0);
  });

  it("restores, deletes for good and empties the trash", async () => {
    const calls = library(3, {
      view_range: (args) =>
        [0, 1, 2]
          .slice(Number(args.start), Number(args.start) + Number(args.len))
          .map((n) => cell(n, { deletedAt: Date.UTC(2026, 9, 29, 9) })),
      item_detail: (args) =>
        item(Number(String(args.id).slice(2)), {
          isDeleted: true,
          deletedAt: Date.UTC(2026, 9, 29, 9),
        }),
      delete_permanently: () => 1,
      empty_trash: () => 2,
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: /^Trash/ }));
    await waitFor(() => {
      const query = called(calls, "open_view").at(-1)?.args.query as LibraryQuery | undefined;
      expect(query?.scope).toEqual({ kind: "trash" });
    });
    expect(screen.queryByRole("button", { name: /Add files/ })).not.toBeInTheDocument();
    await pick("image-1");
    expect(await within(details()).findByText(/In the trash since/)).toBeTruthy();
    await user.click(within(details()).getByRole("button", { name: "Restore" }));
    await waitFor(() =>
      expect(lastEdit(calls, "apply_batch_edit")?.edit).toEqual({ type: "restore" }),
    );

    await pick("image-2");
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));
    fireEvent.keyDown(screen.getByRole("grid", { name: "Items" }), { key: "Delete" });
    const confirm = await screen.findByRole("dialog", { name: "Delete permanently?" });
    expect(confirm).toHaveTextContent("There is no undo");
    await user.click(within(confirm).getByRole("button", { name: "Delete permanently" }));
    await waitFor(() =>
      expect(lastEdit(calls, "delete_permanently")).toEqual({
        target: { viewId: 1, all: false, ids: ["ID0002"] },
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    // Counts load on a 300 ms debounce; the toolbar button stays disabled until then.
    const emptyTrash = await screen.findByRole("button", { name: "Empty Trash" });
    await waitFor(() => expect(emptyTrash).toBeEnabled());
    await user.click(emptyTrash);
    const empty = await screen.findByRole("dialog", { name: "Empty the trash?" });
    expect(empty).toHaveTextContent("2 items and their files");
    await user.click(within(empty).getByRole("button", { name: "Empty Trash" }));
    await waitFor(() => expect(called(calls, "empty_trash")).toHaveLength(1));
  });

  it("shows items and size and restores or empties from the Trash row menu", async () => {
    const calls = library(3, {
      trash_info: () => ({ items: 2, bytes: 3 * 1024 * 1024 }),
      restore_trash: () => 2,
    });
    const user = userEvent.setup();
    render(<App />);
    // Counts and trash info arrive on a short debounce; the row shows the count when ready.
    const trash = await screen.findByRole("button", { name: /^Trash/ });
    await waitFor(() => expect(trash).toHaveTextContent("2"));

    fireEvent.contextMenu(trash);
    const menu = await screen.findByRole("menu");
    await waitFor(() => expect(menu).toHaveTextContent("2 items · 3.0 MB"));
    await user.click(within(menu).getByRole("menuitem", { name: "Restore All" }));
    await waitFor(() => expect(called(calls, "restore_trash")).toHaveLength(1));

    // Empty Trash… asks first.
    fireEvent.contextMenu(trash);
    await user.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Empty Trash…" }),
    );
    const confirm = await screen.findByRole("dialog", { name: "Empty the trash?" });
    await user.click(within(confirm).getByRole("button", { name: "Empty Trash" }));
    await waitFor(() => expect(called(calls, "empty_trash")).toHaveLength(1));
  });

  it("copies, reveals and opens originals, also from the right-click menu", async () => {
    const calls = library(3);
    const user = userEvent.setup();
    render(<App />);
    await pick("image-0");
    await within(details()).findByRole("textbox", { name: "Name" });
    fireEvent.keyDown(screen.getByRole("grid", { name: "Items" }), { key: "c", ctrlKey: true });
    await waitFor(() => expect(lastEdit(calls, "copy_originals")).toEqual({ ids: ["ID0000"] }));
    await user.click(within(details()).getByRole("button", { name: "Open with default app" }));
    expect(lastEdit(calls, "open_original")).toEqual({ id: "ID0000" });

    // A right click on another item selects it first.
    fireEvent.contextMenu(await screen.findByRole("gridcell", { name: "image-2" }));
    await user.click(await screen.findByRole("menuitem", { name: "Show in Explorer" }));
    await waitFor(() => expect(lastEdit(calls, "reveal_originals")).toEqual({ ids: ["ID0002"] }));
  });

  it("drags the original out of the window from the preview", async () => {
    const calls = library(3);
    render(<App />);
    await pick("image-1");
    const preview = await within(details()).findByTitle(/Show full size/);
    fireEvent.pointerDown(preview, { button: 0, buttons: 1, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(preview, { buttons: 1, clientX: 30, clientY: 30 });
    await waitFor(() => expect(lastEdit(calls, "start_drag")).toEqual({ ids: ["ID0001"] }));
    // The click that ends the drag opens no preview.
    fireEvent.click(preview);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows a selection as a stack with its size, and rates it at once", async () => {
    const calls = library(3, {
      selection_tags: () => ({
        items: 2,
        tags: [],
        size: 3 * 1024 * 1024,
        stack: [cell(0), cell(2)],
      }),
    });
    const user = userEvent.setup();
    render(<App />);
    await pick("image-0");
    await pick("image-2", { ctrlKey: true });
    await waitFor(() => expect(details()).toHaveTextContent("2 items selected · 3.0 MB"));
    expect(within(details()).getByRole("img", { name: "2 items" })).toBeTruthy();
    await user.click(within(details()).getByRole("button", { name: "Rate 5 of 5" }));
    await waitFor(() =>
      expect(lastEdit(calls, "apply_batch_edit")?.edit).toEqual({ type: "setStar", value: 5 }),
    );
  });
});

describe("Phase 7: Library Settings and Check Library", () => {
  async function openLibraryMenu(user: ReturnType<typeof userEvent.setup>, item: string) {
    await user.click(screen.getByRole("menuitem", { name: "Library" }));
    await user.click(await screen.findByRole("menuitem", { name: item }));
  }

  it("renames the library, backs it up and restores a backup", async () => {
    const backups = [
      {
        name: "library-2026-10-28.db",
        kind: "daily",
        createdAt: Date.UTC(2026, 9, 28, 8),
        size: 8192,
      },
    ];
    const calls = library(3, { backups: () => backups });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("gridcell", { name: "image-0" });
    await openLibraryMenu(user, "Library Settings…");
    const dialog = await screen.findByRole("dialog", { name: "Library Settings" });
    expect(await within(dialog).findByText(/on disk: originals/)).toBeTruthy();

    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "Mood{Enter}");
    await waitFor(() => expect(lastEdit(calls, "rename_library")).toEqual({ name: "Mood" }));

    await user.click(within(dialog).getByRole("button", { name: "Back up now" }));
    await waitFor(() => expect(called(calls, "back_up_now")).toHaveLength(1));

    const list = within(dialog).getByRole("list", { name: "Backups" });
    expect(list).toHaveTextContent("On opening");
    await user.click(within(list).getByRole("button", { name: "Restore…" }));
    await user.click(within(list).getByRole("button", { name: "Restore" }));
    await waitFor(() =>
      expect(lastEdit(calls, "restore_backup")).toEqual({ name: "library-2026-10-28.db" }),
    );
    expect(await screen.findByText(/The backup was restored/)).toBeTruthy();
  });

  it("checks the library and repairs only what the user picks", async () => {
    const found: LibraryCheck = {
      ...CLEAN_CHECK,
      missingCount: 1,
      missing: [{ id: "ID0001", name: "image-1", inTrash: false }],
      orphanCount: 2,
      orphanBytes: 2048,
      orphans: ["files/zz/A/a.jpg", "files/zz/B/b.jpg"],
    };
    const calls = library(3, { check_library: () => found });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("gridcell", { name: "image-0" });
    await openLibraryMenu(user, "Check Library");
    const dialog = await screen.findByRole("dialog", { name: "Check Library" });
    expect(await within(dialog).findByText("1 item without a file:")).toBeTruthy();
    expect(dialog).toHaveTextContent("2 files (2.0 KB)");
    expect(within(dialog).queryByRole("button", { name: "Repair" })).toBeNull();
    await user.click(within(dialog).getByRole("switch", { name: "Delete the stray files" }));
    await user.click(within(dialog).getByRole("button", { name: "Repair" }));
    await waitFor(() =>
      expect(lastEdit(calls, "repair_library")).toEqual({
        options: { orphans: true, missing: false },
      }),
    );
    expect(await within(dialog).findByText("Every item has its file.")).toBeTruthy();
  });

  it("backs up and rebuilds thumbnails from the Library menu", async () => {
    const calls = library(3, { rebuild_thumbnails: () => 3 });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("gridcell", { name: "image-0" });
    await openLibraryMenu(user, "Back Up Now");
    expect(await screen.findByText("The library was backed up.")).toBeTruthy();
    await openLibraryMenu(user, "Rebuild Thumbnails");
    expect(await screen.findByText("Making 3 thumbnails again…")).toBeTruthy();
    await openLibraryMenu(user, "Show in Explorer");
    await waitFor(() => expect(called(calls, "reveal_library")).toHaveLength(1));
  });
});
