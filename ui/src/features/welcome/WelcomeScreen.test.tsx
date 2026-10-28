import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { WelcomeScreen } from "./WelcomeScreen";

function renderWelcome(status?: string) {
  const handlers = {
    onCreateLibrary: vi.fn(),
    onOpenLibrary: vi.fn(),
    onImportEagleLibrary: vi.fn(),
    onConnectEagle: vi.fn(),
  };
  render(<WelcomeScreen {...handlers} status={status} />);
  return handlers;
}

describe("WelcomeScreen", () => {
  it("offers creating and opening a library first", () => {
    renderWelcome();
    const buttons = screen.getAllByRole("button").map((button) => button.textContent);
    expect(buttons).toEqual([
      "Create library",
      "Open library",
      "Import an Eagle library",
      "Connect to a running Eagle",
    ]);
  });

  it("shows an empty list of recent libraries", () => {
    renderWelcome();
    expect(screen.getByRole("region", { name: "Recent libraries" })).toHaveTextContent(
      "No recent libraries yet.",
    );
  });

  it("keeps the Eagle actions in their own section", () => {
    renderWelcome();
    const eagle = screen.getByRole("region", { name: "Eagle" });
    expect(eagle).toContainElement(screen.getByRole("button", { name: "Import an Eagle library" }));
    expect(eagle).toContainElement(
      screen.getByRole("button", { name: "Connect to a running Eagle" }),
    );
  });

  it("calls the handler of each action", async () => {
    const user = userEvent.setup();
    const handlers = renderWelcome();
    await user.click(screen.getByRole("button", { name: "Create library" }));
    await user.click(screen.getByRole("button", { name: "Open library" }));
    await user.click(screen.getByRole("button", { name: "Import an Eagle library" }));
    await user.click(screen.getByRole("button", { name: "Connect to a running Eagle" }));
    expect(handlers.onCreateLibrary).toHaveBeenCalledOnce();
    expect(handlers.onOpenLibrary).toHaveBeenCalledOnce();
    expect(handlers.onImportEagleLibrary).toHaveBeenCalledOnce();
    expect(handlers.onConnectEagle).toHaveBeenCalledOnce();
  });

  it("shows a status message only when there is one", () => {
    renderWelcome();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("shows the status message", () => {
    renderWelcome("Opening a library is not available yet.");
    expect(screen.getByRole("status")).toHaveTextContent("Opening a library is not available yet.");
  });
});
