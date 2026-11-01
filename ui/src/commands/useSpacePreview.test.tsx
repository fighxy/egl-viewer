import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SPACE_HOLD_MS } from "./spacePreview";
import { useSpacePreview } from "./useSpacePreview";

function Harness({
  enabled = true,
  isOpen = false,
  canOpen = true,
  open,
  close,
}: {
  enabled?: boolean;
  isOpen?: boolean;
  canOpen?: boolean;
  open: () => void;
  close: () => void;
}) {
  useSpacePreview({ enabled, isOpen, canOpen, open, close });
  return <div data-testid="grid" />;
}

describe("useSpacePreview", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("opens on a short Space and stays open on keyup", () => {
    const open = vi.fn();
    const close = vi.fn();
    render(<Harness open={open} close={close} />);
    fireEvent.keyDown(window, { key: " " });
    expect(open).toHaveBeenCalledOnce();
    fireEvent.keyUp(window, { key: " " });
    expect(close).not.toHaveBeenCalled();
  });

  it("closes on a short Space when already open", () => {
    const open = vi.fn();
    const close = vi.fn();
    render(<Harness isOpen open={open} close={close} />);
    fireEvent.keyDown(window, { key: " " });
    expect(open).not.toHaveBeenCalled();
    fireEvent.keyUp(window, { key: " " });
    expect(close).toHaveBeenCalledOnce();
  });

  it("opens on hold and closes on release", () => {
    const open = vi.fn();
    const close = vi.fn();
    render(<Harness open={open} close={close} />);
    fireEvent.keyDown(window, { key: " " });
    expect(open).toHaveBeenCalledOnce();
    act(() => {
      vi.advanceTimersByTime(SPACE_HOLD_MS);
    });
    fireEvent.keyUp(window, { key: " " });
    expect(close).toHaveBeenCalledOnce();
  });

  it("leaves Space to focused buttons", () => {
    const open = vi.fn();
    const close = vi.fn();
    render(
      <>
        <button type="button">Go</button>
        <Harness open={open} close={close} />
      </>,
    );
    fireEvent.keyDown(screen.getByRole("button", { name: "Go" }), { key: " " });
    expect(open).not.toHaveBeenCalled();
  });

  it("does nothing without a selection while closed", () => {
    const open = vi.fn();
    render(<Harness canOpen={false} open={open} close={vi.fn()} />);
    fireEvent.keyDown(window, { key: " " });
    expect(open).not.toHaveBeenCalled();
  });

  it("ignores key repeat while held", () => {
    const open = vi.fn();
    render(<Harness open={open} close={vi.fn()} />);
    fireEvent.keyDown(window, { key: " " });
    fireEvent.keyDown(window, { key: " ", repeat: true });
    expect(open).toHaveBeenCalledOnce();
  });
});
