//! The parity harness's view of context menus, shared by the macOS
//! `NSMenu` popups (`native_menu.rs`) and the Linux views-style popups
//! (`native_menu_linux.rs`): every menu shown while the harness holds menus
//! open is recorded so `context_menu` / `context_menu_pick` steps can read
//! and choose its items.

use std::cell::Cell;

use gpui_kit::*;

use crate::context_menu::{MenuAction, MenuItem, MenuItemKind};

thread_local! {
    /// Parity-harness mode: every menu is recorded and, after being shown for
    /// this long, closes itself (`cancelTracking`), so a remote-controlled
    /// window is never stuck in AppKit's modal tracking loop.
    static AUTO_DISMISS: Cell<Option<std::time::Duration>> = const { Cell::new(None) };
    /// The last menu recorded in headless mode.
    static RECORDED: std::cell::RefCell<Vec<MenuItem>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Harness mode: record every menu and close it after `hold` (the real
/// `NSMenu` is still shown, so screen captures see GHD-comparable chrome).
pub fn set_auto_dismiss(hold: Option<std::time::Duration>) {
    AUTO_DISMISS.with(|h| h.set(hold));
}

/// The last shown menu as `label` lines: `-` for separators, `  ` indent
/// for submenu items, a `[disabled]` / `[x]` suffix like GHD's item flags.
pub fn recorded_menu() -> Vec<String> {
    fn walk(items: &[MenuItem], depth: usize, out: &mut Vec<String>) {
        for item in items {
            let pad = "  ".repeat(depth);
            match &item.kind {
                MenuItemKind::Separator => out.push(format!("{pad}-")),
                MenuItemKind::Action(_) | MenuItemKind::Submenu(_) => {
                    let mut line = format!("{pad}{}", item.label);
                    if !item.enabled {
                        line.push_str(" [disabled]");
                    }
                    if item.checked == Some(true) {
                        line.push_str(" [x]");
                    }
                    out.push(line);
                    if let MenuItemKind::Submenu(children) = &item.kind {
                        walk(children, depth + 1, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    RECORDED.with(|r| walk(&r.borrow(), 0, &mut out));
    out
}

/// Forget the last recorded menu (before an action that may open one).
pub fn clear_recorded() {
    RECORDED.with(|r| r.borrow_mut().clear());
}

/// Run the recorded menu's item with this label (a click on it).
pub fn pick_recorded(label: &str, window: &mut Window, cx: &mut App) -> bool {
    fn find(items: &[MenuItem], label: &str) -> Option<MenuAction> {
        items.iter().find_map(|item| match &item.kind {
            MenuItemKind::Action(action) if item.label.as_ref() == label && item.enabled => {
                Some(action.clone())
            }
            MenuItemKind::Submenu(children) => find(children, label),
            _ => None,
        })
    }
    let action = RECORDED.with(|r| find(&r.borrow(), label));
    match action {
        Some(action) => {
            RECORDED.with(|r| r.borrow_mut().clear());
            action(window, cx);
            true
        }
        None => false,
    }
}

/// The harness hold time, when menus are recorded and closed automatically.
pub fn auto_dismiss() -> Option<std::time::Duration> {
    AUTO_DISMISS.with(|h| h.get())
}

/// Remember `items` as the last shown menu.
pub fn record(items: &[MenuItem]) {
    RECORDED.with(|r| *r.borrow_mut() = items.to_vec());
}
