import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { UpdateDialog } from "./UpdateDialog";

describe("UpdateDialog", () => {
  it("offers Install and Restart when an update is available", async () => {
    const user = userEvent.setup();
    const onInstall = vi.fn();
    const onClose = vi.fn();
    render(
      <UpdateDialog
        state={{
          kind: "available",
          update: {
            currentVersion: "0.1.4",
            version: "0.1.5",
            body: "Signed desktop updates.",
          },
        }}
        onClose={onClose}
        onInstall={onInstall}
      />,
    );
    expect(screen.getByRole("dialog", { name: "Update available" })).toHaveTextContent("0.1.5");
    expect(screen.getByRole("dialog")).toHaveTextContent("Signed desktop updates.");
    await user.click(screen.getByRole("button", { name: "Install and Restart" }));
    expect(onInstall).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Later" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("blocks closing while a download is running", () => {
    render(
      <UpdateDialog
        state={{
          kind: "downloading",
          update: { currentVersion: "0.1.4", version: "0.1.5" },
          progress: { downloaded: 10, contentLength: 100 },
        }}
        onClose={vi.fn()}
        onInstall={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Please wait…" })).toBeDisabled();
    expect(screen.getByText("Downloading update… 10%")).toBeInTheDocument();
  });
});
