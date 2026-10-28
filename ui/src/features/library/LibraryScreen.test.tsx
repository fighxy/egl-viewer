import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES, type LibraryInfo } from "@/ipc";
import { LIBRARY } from "@/test/backend";
import { LibraryScreen } from "./LibraryScreen";

function renderScreen(library: LibraryInfo = LIBRARY, layout = DEFAULT_PREFERENCES.layout) {
  const handlers = { onLayoutChange: vi.fn(), onOpenForEditing: vi.fn() };
  render(<LibraryScreen library={library} layout={layout} {...handlers} />);
  return handlers;
}

describe("LibraryScreen", () => {
  it("shows the frame: left bar, contents, details and status bar", () => {
    renderScreen();
    const nav = screen.getByRole("navigation", { name: "Library" });
    expect(nav).toHaveTextContent("Refs");
    for (const row of ["All", "Uncategorized", "Untagged", "Trash"]) {
      expect(screen.getByRole("button", { name: row })).toBeInTheDocument();
    }
    expect(screen.getByRole("main", { name: "Library contents" })).toHaveTextContent(
      "This library is empty",
    );
    expect(screen.getByRole("complementary", { name: "Details" })).toHaveTextContent(
      "Select an item",
    );
    expect(screen.getByRole("contentinfo")).toHaveTextContent(LIBRARY.path ?? "");
    expect(screen.queryByRole("status", { name: "Read-only library" })).not.toBeInTheDocument();
  });

  it("hides closed bars", () => {
    renderScreen(LIBRARY, { ...DEFAULT_PREFERENCES.layout, leftOpen: false, rightOpen: false });
    expect(screen.queryByRole("navigation", { name: "Library" })).not.toBeInTheDocument();
    expect(screen.queryByRole("complementary", { name: "Details" })).not.toBeInTheDocument();
  });

  it("toggles the details panel from the toolbar", async () => {
    const handlers = renderScreen();
    await userEvent.setup().click(screen.getByRole("button", { name: "Details panel" }));
    expect(handlers.onLayoutChange).toHaveBeenCalledWith(
      { ...DEFAULT_PREFERENCES.layout, rightOpen: false },
      true,
    );
  });

  it("resizes the left bar with the keyboard within its limits", () => {
    const handlers = renderScreen();
    const handle = screen.getByRole("separator", { name: "Resize the sidebar" });
    fireEvent.keyDown(handle, { key: "ArrowRight" });
    expect(handlers.onLayoutChange).toHaveBeenLastCalledWith(
      { ...DEFAULT_PREFERENCES.layout, leftWidth: 256 },
      true,
    );
    renderScreen(LIBRARY, { ...DEFAULT_PREFERENCES.layout, leftWidth: 360 });
    const atMax = screen.getAllByRole("separator", { name: "Resize the sidebar" })[1];
    if (!atMax) throw new Error("second screen missing");
    expect(atMax).toHaveAttribute("aria-valuemax", "360");
  });

  it("explains a library open in another window and offers editing", async () => {
    const handlers = renderScreen({
      ...LIBRARY,
      readOnly: true,
      readOnlyReason: "otherInstance",
      holder: { computer: "DESKTOP-7", since: Date.UTC(2026, 9, 28, 9, 30) },
    });
    const banner = screen.getByRole("status", { name: "Read-only library" });
    expect(banner).toHaveTextContent("open in another egl-viewer window on DESKTOP-7");
    expect(screen.getByRole("navigation", { name: "Library" })).toHaveTextContent("Read-only");
    await userEvent.setup().click(screen.getByRole("button", { name: "Open for editing" }));
    expect(handlers.onOpenForEditing).toHaveBeenCalledOnce();
  });

  it("shows counts, and an empty library invites adding files", async () => {
    const onAddFiles = vi.fn();
    const { rerender } = render(
      <LibraryScreen
        library={LIBRARY}
        layout={DEFAULT_PREFERENCES.layout}
        onLayoutChange={vi.fn()}
        onOpenForEditing={vi.fn()}
        counts={{ all: 0, uncategorized: 0, untagged: 0, trash: 0 }}
        onAddFiles={onAddFiles}
      />,
    );
    expect(screen.getByRole("main")).toHaveTextContent("Drop images here, paste with Ctrl+V");
    await userEvent
      .setup()
      .click(screen.getAllByRole("button", { name: /Add files/ })[0] as HTMLElement);
    expect(onAddFiles).toHaveBeenCalledOnce();

    rerender(
      <LibraryScreen
        library={LIBRARY}
        layout={DEFAULT_PREFERENCES.layout}
        onLayoutChange={vi.fn()}
        onOpenForEditing={vi.fn()}
        counts={{ all: 1234, uncategorized: 1200, untagged: 1234, trash: 5 }}
        importStatus="Adding files: 3 of 10"
        onOpenImports={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: /^All/ })).toHaveTextContent("1,234");
    expect(screen.getByRole("button", { name: /^Trash/ })).toHaveTextContent("5");
    expect(screen.getByRole("main")).toHaveTextContent("1,234 items");
    expect(screen.getByRole("button", { name: "Adding files: 3 of 10" })).toBeInTheDocument();
  });

  it("disables adding in a read-only library and says why", () => {
    render(
      <LibraryScreen
        library={{ ...LIBRARY, readOnly: true, readOnlyReason: "readOnlyMedium" }}
        layout={DEFAULT_PREFERENCES.layout}
        onLayoutChange={vi.fn()}
        onOpenForEditing={vi.fn()}
        canAdd="This library is read-only."
        dropping
      />,
    );
    const add = screen.getAllByRole("button", { name: /Add files/ })[0];
    expect(add).toBeDisabled();
    expect(add).toHaveAttribute("title", "This library is read-only.");
    expect(screen.getByRole("status", { name: "Drop files" })).toHaveTextContent(
      "This library is read-only.",
    );
  });

  it("explains a library in a synced folder without offering editing", () => {
    renderScreen({ ...LIBRARY, readOnly: true, readOnlyReason: "syncFolder" });
    expect(screen.getByRole("status", { name: "Read-only library" })).toHaveTextContent(
      "cloud-synced folder",
    );
    expect(screen.queryByRole("button", { name: "Open for editing" })).not.toBeInTheDocument();
  });
});
