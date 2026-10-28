import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { LIBRARY } from "@/test/backend";
import { CreateLibraryDialog } from "./CreateLibraryDialog";

function renderDialog(create = vi.fn().mockResolvedValue(LIBRARY)) {
  const props = {
    onOpenChange: vi.fn(),
    loadDefaultLocation: vi.fn().mockResolvedValue("C:\\Users\\me\\egl-library"),
    browse: vi.fn().mockResolvedValue("D:\\Art"),
    create,
    onCreated: vi.fn(),
  };
  render(<CreateLibraryDialog open {...props} />);
  return props;
}

describe("CreateLibraryDialog", () => {
  it("starts in the default folder and shows where the library goes", async () => {
    const user = userEvent.setup();
    const props = renderDialog();
    expect(await screen.findByDisplayValue("C:\\Users\\me\\egl-library")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create" })).toBeDisabled();
    await user.type(screen.getByLabelText("Name"), "Refs");
    expect(screen.getByText("Creates C:\\Users\\me\\egl-library\\Refs.eglib")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Create" }));
    expect(props.create).toHaveBeenCalledWith("C:\\Users\\me\\egl-library", "Refs");
    expect(props.onCreated).toHaveBeenCalledWith(LIBRARY);
  });

  it("takes the location from the folder picker", async () => {
    const user = userEvent.setup();
    renderDialog();
    await screen.findByDisplayValue("C:\\Users\\me\\egl-library");
    await user.click(screen.getByRole("button", { name: "Browse…" }));
    expect(await screen.findByDisplayValue("D:\\Art")).toBeInTheDocument();
  });

  it("shows why a library cannot be created and stays open", async () => {
    const user = userEvent.setup();
    const props = renderDialog(
      vi.fn().mockRejectedValue({ kind: "unsupportedLocation", detail: "syncFolder" }),
    );
    await screen.findByDisplayValue("C:\\Users\\me\\egl-library");
    await user.type(screen.getByLabelText("Name"), "Cloud");
    await user.click(screen.getByRole("button", { name: "Create" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("cloud-synced folder");
    expect(props.onCreated).not.toHaveBeenCalled();
  });
});
