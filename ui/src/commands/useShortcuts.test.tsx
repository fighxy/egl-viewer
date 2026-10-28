import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES } from "@/ipc";
import { LIBRARY } from "@/test/backend";
import type { CommandActions, CommandState } from "./registry";
import { useShortcuts } from "./useShortcuts";

const STATE: CommandState = {
  library: LIBRARY,
  recent: [],
  theme: "system",
  fullscreen: false,
  leftOpen: true,
  rightOpen: true,
  browse: DEFAULT_PREFERENCES.browse,
  items: 3,
  selected: 1,
};

function Harness({ actions, enabled = true }: { actions: CommandActions; enabled?: boolean }) {
  useShortcuts(STATE, actions, enabled);
  return (
    <>
      <input aria-label="Field" />
      <button type="button">Button</button>
    </>
  );
}

function actionsWith(paste = vi.fn()) {
  return { paste, preview: vi.fn() } as unknown as CommandActions;
}

describe("useShortcuts", () => {
  it("leaves Ctrl+V to text fields and pastes into the library elsewhere", () => {
    const actions = actionsWith();
    render(<Harness actions={actions} />);
    fireEvent.keyDown(screen.getByLabelText("Field"), { key: "v", ctrlKey: true });
    expect(actions.paste).not.toHaveBeenCalled();
    fireEvent.keyDown(document.body, { key: "v", ctrlKey: true });
    expect(actions.paste).toHaveBeenCalledOnce();
  });

  it("leaves Space to buttons and fields and opens the preview elsewhere", () => {
    const actions = actionsWith();
    render(<Harness actions={actions} />);
    fireEvent.keyDown(screen.getByRole("button", { name: "Button" }), { key: " " });
    fireEvent.keyDown(screen.getByLabelText("Field"), { key: " " });
    expect(actions.preview).not.toHaveBeenCalled();
    fireEvent.keyDown(document.body, { key: " " });
    expect(actions.preview).toHaveBeenCalledOnce();
  });

  it("does nothing while a dialog is open", () => {
    const actions = actionsWith();
    render(<Harness actions={actions} enabled={false} />);
    fireEvent.keyDown(document.body, { key: "v", ctrlKey: true });
    expect(actions.paste).not.toHaveBeenCalled();
  });
});
