/**
 * Space on the grid: a short press toggles the full-size preview; a hold opens it for as long
 * as the key is down and closes it on release (D-073). Pure state so the timing can be tested
 * without the DOM.
 */

/** Milliseconds before a held Space counts as a peek instead of a toggle. */
export const SPACE_HOLD_MS = 250;

export type SpaceDown = {
  /** The preview was already open when Space went down. */
  wasOpen: boolean;
  /** True once {@link SPACE_HOLD_MS} has elapsed while Space is still down. */
  held: boolean;
};

/** Space keydown (ignoring repeats): open if closed; remember whether it was already open. */
export function spaceKeyDown(isOpen: boolean): { down: SpaceDown; open: boolean } {
  return { down: { wasOpen: isOpen, held: false }, open: !isOpen };
}

/** The hold threshold elapsed while Space is still down. */
export function spaceHold(down: SpaceDown): SpaceDown {
  return { ...down, held: true };
}

/**
 * Space keyup: a hold always closes; a short press closes only when the preview was already
 * open (toggle off). A short press that opened the preview leaves it open.
 */
export function spaceKeyUp(down: SpaceDown): { close: boolean } {
  return { close: down.held || down.wasOpen };
}
