import { emit } from "@tauri-apps/api/event";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "@/App";
import { SPACE_HOLD_MS } from "@/commands/spacePreview";
import { DEFAULT_PREFERENCES, type LibraryQuery } from "@/ipc";
import { called, fakeLayout, item, LIBRARY, listening, mockBackend, viewOf } from "@/test/backend";

let restoreLayout: () => void;
beforeEach(() => {
  restoreLayout = fakeLayout(800, 600);
});
afterEach(() => restoreLayout());

const opened = { startup: () => ({ library: LIBRARY, notice: null }) };

/** A library with `total` items; every `open_view` gets a new id. */
function browse(
  total = 1000,
  overrides: Record<string, (args: Record<string, unknown>) => unknown> = {},
) {
  let nextId = 0;
  return mockBackend({
    ...opened,
    ...viewOf(total),
    open_view: () => {
      nextId += 1;
      return { viewId: nextId, total };
    },
    item_detail: (args) => item(Number(String(args.id).slice(2))),
    ...overrides,
  });
}

async function cellNamed(name: string) {
  return screen.findByRole("gridcell", { name });
}

function grid() {
  return screen.getByRole("grid", { name: "Items" });
}

describe("Browsing a library", () => {
  it("shows the items in a virtualized grid fed by windows of cells", async () => {
    const calls = browse();
    render(<App />);
    const first = await cellNamed("image-0");
    expect(first.querySelector("img")).toHaveAttribute("src", "thumb://localhost/ID0000?v=1");
    // Only the rows in sight (and a few more) exist.
    expect(screen.getAllByRole("gridcell").length).toBeLessThan(60);
    expect(called(calls, "view_range")[0]?.args).toEqual({ viewId: 1, start: 0, len: 120 });
    const query = called(calls, "open_view")[0]?.args.query as LibraryQuery;
    expect(query.scope).toEqual({ kind: "all" });
    expect(query.sort).toBe("added");
    expect(screen.getByRole("main")).toHaveTextContent("1,000 items");
  });

  it("selects with click, Ctrl, Shift and Ctrl+A, and shows one item in the details", async () => {
    const calls = browse();
    render(<App />);
    fireEvent.mouseDown(await cellNamed("image-2"));
    const details = screen.getByRole("complementary", { name: "Details" });
    await waitFor(() =>
      expect(within(details).getByRole("textbox", { name: "Name" })).toHaveValue("image-2"),
    );
    expect(called(calls, "item_detail")[0]?.args).toEqual({ id: "ID0002" });
    expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected");

    fireEvent.mouseDown(await cellNamed("image-5"), { ctrlKey: true });
    await waitFor(() => expect(details).toHaveTextContent("2 items selected"));

    fireEvent.mouseDown(await cellNamed("image-8"), { shiftKey: true });
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("4 selected"));
    expect(called(calls, "view_ids").at(-1)?.args).toEqual({ viewId: 1, start: 5, len: 4 });
    expect(await cellNamed("image-6")).toHaveAttribute("aria-selected", "true");
    expect(await cellNamed("image-2")).toHaveAttribute("aria-selected", "false");

    fireEvent.keyDown(grid(), { key: "a", ctrlKey: true });
    await waitFor(() =>
      expect(screen.getByRole("contentinfo")).toHaveTextContent("1,000 selected"),
    );
    expect(await cellNamed("image-0")).toHaveAttribute("aria-selected", "true");
  });

  it("moves the selection with the arrow keys", async () => {
    const calls = browse();
    render(<App />);
    await cellNamed("image-0");
    fireEvent.keyDown(grid(), { key: "ArrowRight" });
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));
    expect(await cellNamed("image-0")).toHaveAttribute("aria-selected", "true");
    // Fake cells are 40×20 (2:1); at 800×160 that packs two per justified row.
    fireEvent.keyDown(grid(), { key: "ArrowDown" });
    await waitFor(async () =>
      expect(await cellNamed("image-2")).toHaveAttribute("aria-selected", "true"),
    );
    expect(grid()).toHaveAttribute("aria-activedescendant", "item-cell-2");
    fireEvent.keyDown(grid(), { key: "ArrowRight", shiftKey: true });
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("2 selected"));
    expect(called(calls, "view_ids").at(-1)?.args).toEqual({ viewId: 1, start: 2, len: 2 });
  });

  it("selects the cells a rubber band touches", async () => {
    const calls = browse();
    render(<App />);
    await cellNamed("image-0");
    // Justified 2:1 cells are ~half the width; drag past the mid gutter into image-1.
    fireEvent.mouseDown(grid(), { button: 0, clientX: 2, clientY: 2 });
    fireEvent.mouseMove(window, { clientX: 500, clientY: 100 });
    fireEvent.mouseUp(window);
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("selected"));
    expect(called(calls, "view_ids").at(-1)?.args).toMatchObject({ viewId: 1, start: 0 });
    expect(await cellNamed("image-1")).toHaveAttribute("aria-selected", "true");
  });

  it("opens another view for another scope and closes the old one", async () => {
    const calls = browse(3);
    const user = userEvent.setup();
    render(<App />);
    await cellNamed("image-0");
    await user.click(screen.getByRole("button", { name: /^Trash/ }));
    await waitFor(() => expect(called(calls, "open_view")).toHaveLength(2));
    const query = called(calls, "open_view")[1]?.args.query as LibraryQuery;
    expect(query.scope).toEqual({ kind: "trash" });
    await waitFor(() => expect(called(calls, "close_view")[0]?.args).toEqual({ viewId: 1 }));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Trash");
  });

  it("follows views-invalidated: a new total and refetched windows", async () => {
    const calls = browse(3);
    render(<App />);
    await cellNamed("image-0");
    await listening("views-invalidated");
    const fetched = called(calls, "view_range").length;
    await act(() => emit("views-invalidated", { views: [{ viewId: 1, total: 5 }] }));
    await waitFor(() => expect(screen.getByRole("main")).toHaveTextContent("5 items"));
    await waitFor(() => expect(called(calls, "view_range").length).toBeGreaterThan(fetched));
  });

  it("sorts from the View menu and remembers the sort", async () => {
    const calls = browse(3);
    const user = userEvent.setup();
    render(<App />);
    await cellNamed("image-0");
    await user.click(screen.getByRole("menuitem", { name: "View" }));
    // jsdom has no geometry for Radix's pointer grace, so the submenu goes by keyboard.
    (await screen.findByRole("menuitem", { name: "Sort By" })).focus();
    await user.keyboard("{ArrowRight}");
    (await screen.findByRole("menuitemradio", { name: "Title" })).focus();
    await user.keyboard("{Enter}");
    await waitFor(() => {
      const query = called(calls, "open_view").at(-1)?.args.query as LibraryQuery;
      expect(query.sort).toBe("name");
    });
    const saved = called(calls, "set_preferences").at(-1)?.args.preferences as {
      browse: { sort: string };
    };
    expect(saved.browse.sort).toBe("name");
  });

  it("shows a list with columns when the list layout is chosen", async () => {
    browse(3, {
      get_preferences: () => ({
        ...DEFAULT_PREFERENCES,
        browse: { ...DEFAULT_PREFERENCES.browse, viewMode: "list" },
      }),
    });
    render(<App />);
    const row = await cellNamed("image-1");
    expect(row).toHaveTextContent("40 × 20");
    expect(row).toHaveTextContent("1001 B");
    expect(screen.getByText("Dimensions")).toBeInTheDocument();
    expect(screen.queryByRole("slider", { name: "Thumbnail size" })).not.toBeInTheDocument();
  });

  it("opens the preview on double-click and Space, moves with arrows, closes with Esc", async () => {
    browse(3);
    const user = userEvent.setup();
    render(<App />);
    fireEvent.doubleClick(await cellNamed("image-0"));
    const preview = await screen.findByRole("dialog", { name: "image-0" });
    expect(preview).toHaveTextContent("1 of 3");
    expect(within(preview).getByRole("img", { name: "image-0" })).toHaveAttribute(
      "src",
      "thumb://localhost/ID0000/original",
    );
    fireEvent.keyDown(preview, { key: "ArrowRight" });
    expect(await screen.findByRole("dialog", { name: "image-1" })).toHaveTextContent("2 of 3");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    fireEvent.mouseDown(await cellNamed("image-2"));
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));
    fireEvent.keyDown(window, { key: " " });
    fireEvent.keyUp(window, { key: " " });
    expect(await screen.findByRole("dialog", { name: "image-2" })).toHaveTextContent("3 of 3");
  });

  it("toggles the preview with a short Space and peeks while Space is held", async () => {
    browse(3);
    render(<App />);
    fireEvent.mouseDown(await cellNamed("image-0"));
    await waitFor(() => expect(screen.getByRole("contentinfo")).toHaveTextContent("1 selected"));

    fireEvent.keyDown(window, { key: " " });
    fireEvent.keyUp(window, { key: " " });
    expect(await screen.findByRole("dialog", { name: "image-0" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: " " });
    fireEvent.keyUp(window, { key: " " });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    fireEvent.keyDown(window, { key: " " });
    expect(await screen.findByRole("dialog", { name: "image-0" })).toBeInTheDocument();
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, SPACE_HOLD_MS + 20));
    });
    fireEvent.keyUp(window, { key: " " });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("closes the preview on a left or right click in the letterbox beside the image", async () => {
    browse(3);
    render(<App />);
    fireEvent.doubleClick(await cellNamed("image-0"));
    const preview = await screen.findByRole("dialog", { name: "image-0" });
    const image = within(preview).getByRole("img", { name: "image-0" });
    const stage = image.parentElement;
    if (!stage) throw new Error("preview stage missing");

    fireEvent.mouseDown(stage, { button: 0 });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    fireEvent.doubleClick(await cellNamed("image-0"));
    const again = await screen.findByRole("dialog", { name: "image-0" });
    const imageAgain = within(again).getByRole("img", { name: "image-0" });
    fireEvent.mouseDown(imageAgain, { button: 0 });
    expect(screen.getByRole("dialog", { name: "image-0" })).toBeInTheDocument();
    const stageAgain = imageAgain.parentElement;
    if (!stageAgain) throw new Error("preview stage missing");
    fireEvent.mouseDown(stageAgain, { button: 2 });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("rotates the preview with the toolbar button and R, without breaking arrows", async () => {
    browse(3);
    render(<App />);
    fireEvent.doubleClick(await cellNamed("image-0"));
    const preview = await screen.findByRole("dialog", { name: "image-0" });
    const image = within(preview).getByRole("img", { name: "image-0" });
    expect(image.style.transform).toContain("rotate(0deg)");

    fireEvent.click(within(preview).getByRole("button", { name: "Rotate clockwise" }));
    expect(image.style.transform).toContain("rotate(90deg)");

    fireEvent.keyDown(preview, { key: "r", code: "KeyR" });
    expect(image.style.transform).toContain("rotate(180deg)");

    // The same physical key on the Russian layout, and Ctrl+R, which is not a rotation.
    fireEvent.keyDown(preview, { key: "к", code: "KeyR" });
    expect(image.style.transform).toContain("rotate(270deg)");
    fireEvent.keyDown(preview, { key: "r", code: "KeyR", ctrlKey: true });
    expect(image.style.transform).toContain("rotate(270deg)");
    fireEvent.keyDown(preview, { key: "к", code: "KeyR", ctrlKey: true });
    expect(image.style.transform).toContain("rotate(270deg)");

    fireEvent.keyDown(preview, { key: "ArrowRight" });
    const next = await screen.findByRole("dialog", { name: "image-1" });
    expect(next).toHaveTextContent("2 of 3");
    expect(within(next).getByRole("img", { name: "image-1" }).style.transform).toContain(
      "rotate(0deg)",
    );
  });

  it("changes the thumbnail size with Ctrl+= and Ctrl+-", async () => {
    const calls = browse(3);
    render(<App />);
    await cellNamed("image-0");
    fireEvent.keyDown(grid(), { key: "=", ctrlKey: true });
    await waitFor(() =>
      expect(screen.getByRole("slider", { name: "Thumbnail size" })).toHaveAttribute(
        "aria-valuenow",
        "180",
      ),
    );
    fireEvent.keyDown(grid(), { key: "-", ctrlKey: true });
    fireEvent.keyDown(grid(), { key: "-", ctrlKey: true });
    await waitFor(() =>
      expect(screen.getByRole("slider", { name: "Thumbnail size" })).toHaveAttribute(
        "aria-valuenow",
        "140",
      ),
    );
    const saved = called(calls, "set_preferences").at(-1)?.args.preferences as {
      browse: { thumbnailSize: number };
    };
    expect(saved.browse.thumbnailSize).toBe(140);
  });

  it("moves focus to the details panel with Enter", async () => {
    browse(3);
    render(<App />);
    fireEvent.mouseDown(await cellNamed("image-1"));
    const details = screen.getByRole("complementary", { name: "Details" });
    await waitFor(() =>
      expect(within(details).getByRole("textbox", { name: "Name" })).toHaveValue("image-1"),
    );
    fireEvent.keyDown(grid(), { key: "Enter" });
    await waitFor(() => expect(details).toContainElement(document.activeElement as HTMLElement));
  });
});
