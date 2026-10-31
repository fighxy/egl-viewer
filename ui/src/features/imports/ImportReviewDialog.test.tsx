import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ImportPreview } from "@/ipc";
import { ImportReviewDialog } from "./ImportReviewDialog";

const preview: ImportPreview = {
  files: 3,
  bytes: 3 * 1024,
  unlisted: 0,
  tree: [
    {
      name: "incoming",
      folder: true,
      bytes: 3 * 1024,
      image: false,
      children: [
        { name: "a.png", folder: false, bytes: 1024, image: true, children: [] },
        { name: "b.txt", folder: false, bytes: 1024, image: false, children: [] },
        {
          name: "sub",
          folder: true,
          bytes: 1024,
          image: false,
          children: [{ name: "c.png", folder: false, bytes: 1024, image: true, children: [] }],
        },
      ],
    },
  ],
};

describe("ImportReviewDialog", () => {
  it("preselects images and imports the confirmed exclude/include sets", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    render(
      <ImportReviewDialog
        review={{ path: "C:\\incoming", preview }}
        target="Photos"
        onConfirm={onConfirm}
      />,
    );
    const dialog = screen.getByRole("dialog", { name: "Import “incoming”" });
    expect(dialog).toHaveTextContent("into “Photos”");
    expect(dialog).toHaveTextContent("2 files · 2.0 KB selected");

    await user.click(within(dialog).getByRole("checkbox", { name: "b.txt" }));
    expect(dialog).toHaveTextContent("3 files · 3.0 KB selected");
    await user.click(within(dialog).getByRole("checkbox", { name: "sub" }));
    expect(dialog).toHaveTextContent("2 files · 2.0 KB selected");
    expect(within(dialog).getByRole("checkbox", { name: "sub" })).not.toBeChecked();

    await user.click(within(dialog).getByRole("button", { name: "Import" }));
    expect(onConfirm).toHaveBeenCalledWith({ exclude: ["sub"], include: ["b.txt"] });
  });

  it("Images only clears overrides, and cancel copies nothing", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    render(
      <ImportReviewDialog
        review={{ path: "C:\\incoming", preview }}
        target={null}
        onConfirm={onConfirm}
      />,
    );
    const dialog = screen.getByRole("dialog", { name: "Import “incoming”" });
    await user.click(within(dialog).getByRole("checkbox", { name: "b.txt" }));
    await user.click(within(dialog).getByRole("checkbox", { name: "sub" }));
    // Only b.txt (a.png still on; sub excluded).
    expect(dialog).toHaveTextContent("2 files · 2.0 KB selected");
    await user.click(within(dialog).getByRole("checkbox", { name: "a.png" }));
    expect(dialog).toHaveTextContent("1 file · 1.0 KB selected");

    await user.click(within(dialog).getByRole("button", { name: "Images only" }));
    expect(dialog).toHaveTextContent("2 files · 2.0 KB selected");

    await user.keyboard("{Escape}");
    expect(onConfirm).toHaveBeenCalledWith(null);
  });

  it("stays closed without a review", () => {
    render(<ImportReviewDialog review={null} target={null} onConfirm={vi.fn()} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
