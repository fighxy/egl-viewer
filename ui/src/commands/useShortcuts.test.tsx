import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
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
};

function Harness({ actions, enabled = true }: { actions: CommandActions; enabled?: boolean }) {
  useShortcuts(STATE, actions, enabled);
  return <input aria-label="Field" />;
}

function actionsWith(paste = vi.fn()) {
  return { paste } as unknown as CommandActions;
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

  it("does nothing while a dialog is open", () => {
    const actions = actionsWith();
    render(<Harness actions={actions} enabled={false} />);
    fireEvent.keyDown(document.body, { key: "v", ctrlKey: true });
    expect(actions.paste).not.toHaveBeenCalled();
  });
});
