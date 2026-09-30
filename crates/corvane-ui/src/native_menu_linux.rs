//! Context menus on Linux - what GHD gets from Electron's `Menu.popup`
//! (`showContextualMenu`): a Chromium views menu in its own popup window
//! (`views_menu`), with keyboard navigation and Escape.
//!
//! In parity-harness mode ([`set_auto_dismiss`]) every menu is recorded and
//! closes itself after the hold, as the macOS `NSMenu` does.

use gpui_kit::*;

use crate::context_menu::MenuItem;
pub use crate::native_menu_common::{clear_recorded, recorded_menu, set_auto_dismiss};

/// Pop up a menu with its top-left corner at `position` (window coordinates).
pub fn show_context_menu(
    items: Vec<MenuItem>,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    if crate::native_menu_common::auto_dismiss().is_some() {
        crate::native_menu_common::record(&items);
    }
    crate::views_menu::show_context_menu(&items, position, window, cx);
}

/// Choose the recorded menu's item with this label: in the open menu when
/// it is still up, else from the record.
pub fn pick_recorded(label: &str, window: &mut Window, cx: &mut App) -> bool {
    if crate::views_menu::pick(label, cx) {
        crate::native_menu_common::clear_recorded();
        return true;
    }
    crate::native_menu_common::pick_recorded(label, window, cx)
}
