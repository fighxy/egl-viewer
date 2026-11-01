import { describe, expect, it } from "vitest";
import { spaceHold, spaceKeyDown, spaceKeyUp } from "./spacePreview";

describe("spacePreview gesture", () => {
  it("opens on keydown when closed and keeps open on a short keyup", () => {
    const { down, open } = spaceKeyDown(false);
    expect(open).toBe(true);
    expect(down).toEqual({ wasOpen: false, held: false });
    expect(spaceKeyUp(down).close).toBe(false);
  });

  it("closes on a short keyup when already open (toggle)", () => {
    const { down, open } = spaceKeyDown(true);
    expect(open).toBe(false);
    expect(spaceKeyUp(down).close).toBe(true);
  });

  it("closes on keyup after a hold that opened the preview", () => {
    const { down, open } = spaceKeyDown(false);
    expect(open).toBe(true);
    expect(spaceKeyUp(spaceHold(down)).close).toBe(true);
  });

  it("closes on keyup after a hold when already open", () => {
    const { down } = spaceKeyDown(true);
    expect(spaceKeyUp(spaceHold(down)).close).toBe(true);
  });
});
