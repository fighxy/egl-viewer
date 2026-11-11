# egl-viewer changes to gpui-pre-windows 0.3.12

This is `gpui-pre-windows` 0.3.12 from crates.io (Apache-2.0, see `LICENSE-APACHE`), used
through `[patch.crates-io]` in the workspace `Cargo.toml`. egl-viewer decision D-148 (moved to 0.3.12 with gpui-kit 0.9.1, D-155).

Changed file: `src/util.rs`.

- `load_cursor` gives `CursorStyle::OpenHand` and `CursorStyle::ClosedHand` their own cursors:
  cursor resources 102 and 101 of the running exe (`OPEN_HAND_CURSOR_RESOURCE`,
  `CLOSED_HAND_CURSOR_RESOURCE`); without them, the system hand (`IDC_HAND`). Upstream maps
  both to the arrow, because Windows has no hand cursor of either kind.
- `GetModuleHandleW` is imported for that.

Nothing else differs from the published crate. To move to a newer gpui-pre-windows: copy the
new version here, re-apply the `load_cursor` change, and update the version in this note.
