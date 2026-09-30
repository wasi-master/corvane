//! Context menus on Linux - what GHD gets from Electron's `Menu.popup`
//! (`showContextualMenu`), a Chromium views menu in its own popup window.
//!
//! Phase 1 of the Linux port: menus are recorded for the parity harness but
//! not shown yet; the popup window lands with the menu bar work.

use gpui_kit::*;

use crate::context_menu::MenuItem;
pub use crate::native_menu_common::{
    clear_recorded, pick_recorded, recorded_menu, set_auto_dismiss,
};

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
    let _ = (position, window, cx);
}
