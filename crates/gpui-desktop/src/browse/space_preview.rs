//! Hold-vs-toggle Space for the full-size preview (D-073).
//! Port of `ui/src/commands/spacePreview.ts`.

/// Milliseconds before a held Space counts as a peek instead of a toggle.
pub const SPACE_HOLD_MS: u64 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceDown {
    /// The preview was already open when Space went down.
    pub was_open: bool,
    /// True once [`SPACE_HOLD_MS`] has elapsed while Space is still down.
    pub held: bool,
}

/// Space keydown (ignoring repeats): open if closed; remember whether it was already open.
pub fn space_key_down(is_open: bool) -> (SpaceDown, bool) {
    (
        SpaceDown {
            was_open: is_open,
            held: false,
        },
        !is_open,
    )
}

/// The hold threshold elapsed while Space is still down.
pub fn space_hold(down: SpaceDown) -> SpaceDown {
    SpaceDown {
        held: true,
        ..down
    }
}

/// Space keyup: a hold always closes; a short press closes only when the preview was already
/// open (toggle off). A short press that opened the preview leaves it open.
pub fn space_key_up(down: SpaceDown) -> bool {
    down.held || down.was_open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_space_opens_and_stays_open() {
        let (down, open) = space_key_down(false);
        assert!(open);
        assert!(!space_key_up(down));
    }

    #[test]
    fn short_space_closes_when_already_open() {
        let (down, open) = space_key_down(true);
        assert!(!open);
        assert!(space_key_up(down));
    }

    #[test]
    fn hold_closes_on_keyup() {
        let (down, open) = space_key_down(false);
        assert!(open);
        let down = space_hold(down);
        assert!(space_key_up(down));
    }
}
