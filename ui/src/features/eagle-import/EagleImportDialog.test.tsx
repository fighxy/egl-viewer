import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { LIBRARY } from "@/test/backend";
import { EagleImportDialog } from "./EagleImportDialog";

const SUMMARY = {
  path: "D:\\Libraries\\Synthetic.library",
  name: "Synthetic",
  itemCount: 7,
  bytes: 5 * 1024 * 1024,
};

function renderDialog(overrides: Record<string, unknown> = {}) {
  const props = {
    open: true,
    onOpenChange: vi.fn(),
    openLibrary: null,
    loadDefaultLocation: vi.fn().mockResolvedValue("C:\\Users\\me\\egl-library"),
    browse: vi.fn().mockResolvedValue("D:\\Libraries"),
    info: vi.fn().mockResolvedValue(SUMMARY),
    start: vi.fn().mockResolvedValue({ id: 3 }),
    onNewLibraryOpened: vi.fn(),
    progress: null,
    onCancel: vi.fn(),
    ...overrides,
  };
  const view = render(<EagleImportDialog {...props} />);
  return {
    props,
    rerender: (next: Record<string, unknown>) =>
      view.rerender(<EagleImportDialog {...props} {...next} />),
  };
}

describe("EagleImportDialog", () => {
  it("validates the picked folder, then imports into a new library", async () => {
    const user = userEvent.setup();
    const { props } = renderDialog();
    await user.type(
      screen.getByLabelText("Eagle library folder"),
      "D:\\Libraries\\Synthetic.library",
    );
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText(/Synthetic: 7 items/)).toBeInTheDocument();
    // The suggested name comes from the Eagle library.
    expect(screen.getByDisplayValue("Synthetic")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Import" }));
    expect(props.start).toHaveBeenCalledWith({
      eaglePath: "D:\\Libraries\\Synthetic.library",
      create: { parent: "C:\\Users\\me\\egl-library", name: "Synthetic" },
      importTrash: false,
    });
    expect(props.onNewLibraryOpened).toHaveBeenCalledOnce();
  });

  it("shows why the folder cannot be imported and stays on the pick step", async () => {
    const user = userEvent.setup();
    renderDialog({
      info: vi.fn().mockRejectedValue({ kind: "notALibrary", detail: { eagle: true } }),
    });
    await user.type(screen.getByLabelText("Eagle library folder"), "D:\\NotEagle");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
  });

  it("offers the open library as the target", async () => {
    const user = userEvent.setup();
    const { props } = renderDialog({ openLibrary: LIBRARY });
    await user.type(
      screen.getByLabelText("Eagle library folder"),
      "D:\\Libraries\\Synthetic.library",
    );
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Synthetic: 7 items/);
    await user.click(screen.getByRole("radio", { name: /Into the open library/ }));
    await user.click(screen.getByRole("button", { name: "Import" }));
    expect(props.start).toHaveBeenCalledWith({
      eaglePath: "D:\\Libraries\\Synthetic.library",
      create: null,
      importTrash: false,
    });
    expect(props.onNewLibraryOpened).not.toHaveBeenCalled();
  });

  it("shows the report of a finished import", async () => {
    const user = userEvent.setup();
    const { rerender } = renderDialog();
    await user.type(
      screen.getByLabelText("Eagle library folder"),
      "D:\\Libraries\\Synthetic.library",
    );
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Synthetic: 7 items/);
    await user.click(screen.getByRole("button", { name: "Import" }));
    expect(screen.getByText(/Importing|Starting/)).toBeInTheDocument();
    rerender({
      progress: {
        id: 3,
        kind: "eagle",
        state: "done",
        total: 7,
        processed: 7,
        added: 5,
        duplicates: 1,
        failed: 1,
        previewsTotal: 1,
        previewsDone: 1,
        current: null,
        duplicate: null,
        failures: [],
        skipped: [],
        report: {
          imported: 5,
          duplicates: 1,
          trashSkipped: 1,
          skipped: [{ path: "D:\\…\\EITEMBROKEN1.info", reason: "the original file is missing" }],
          lockedFolders: ["Secret"],
        },
        startedAt: 0,
        finishedAt: 1,
      },
    });
    expect(await screen.findByText("Imported: 5")).toBeInTheDocument();
    expect(screen.getByText(/Locked folders.*Secret/)).toBeInTheDocument();
    expect(screen.getByText(/the original file is missing/)).toBeInTheDocument();
  });
});
