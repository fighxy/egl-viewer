import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { progress } from "@/test/backend";
import { DuplicateDialog } from "./DuplicateDialog";
import { ImportsPanel } from "./ImportsPanel";

describe("ImportsPanel", () => {
  it("lists imports with their problems, skipped duplicates and a cancel button", async () => {
    const onCancel = vi.fn();
    render(
      <ImportsPanel
        open
        onOpenChange={vi.fn()}
        onCancel={onCancel}
        imports={[
          progress({ id: 2 }),
          progress({
            id: 1,
            state: "done",
            added: 2,
            failed: 1,
            duplicates: 1,
            failures: [{ path: "C:\\in\\broken.jpg", reason: "No preview: cannot decode" }],
            skipped: [{ path: "C:\\in\\same.png", existing: "01J", inTrash: true }],
          }),
        ]}
      />,
    );
    const running = screen.getByRole("listitem", { name: "Import 2" });
    expect(running).toHaveTextContent("Adding files: 1 of 3");
    await userEvent.setup().click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledWith(2);

    const finished = screen.getByRole("listitem", { name: "Import 1" });
    expect(finished).toHaveTextContent("Added 2 files · 1 duplicate skipped · 1 problem");
    expect(finished).toHaveTextContent("C:\\in\\broken.jpg — No preview: cannot decode");
    expect(finished).toHaveTextContent("C:\\in\\same.png (in the trash)");
    expect(screen.getAllByRole("button", { name: "Cancel" })).toHaveLength(1);
  });

  it("says when nothing was added", () => {
    render(<ImportsPanel open onOpenChange={vi.fn()} onCancel={vi.fn()} imports={[]} />);
    expect(screen.getByText("Nothing added yet.")).toBeInTheDocument();
  });
});

describe("DuplicateDialog", () => {
  it("asks once and passes the answer on", async () => {
    const onAnswer = vi.fn();
    render(
      <DuplicateDialog
        onAnswer={onAnswer}
        waiting={progress({
          id: 7,
          state: "waitingForDecision",
          duplicate: { path: "D:\\refs\\poster.png", existing: "01J", inTrash: false },
        })}
      />,
    );
    expect(screen.getByRole("dialog")).toHaveTextContent("“poster.png” is already in the library");
    await userEvent.setup().click(screen.getByRole("button", { name: "Add anyway" }));
    expect(onAnswer).toHaveBeenCalledWith(7, "addAnyway");
  });

  it("stays closed without a waiting import", () => {
    render(<DuplicateDialog onAnswer={vi.fn()} waiting={undefined} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
