//! Native context menus on macOS - what GHD gets from Electron's
//! `Menu.popup` (`showContextualMenu`): a real `NSMenu` with vibrancy,
//! animations, keyboard navigation and Escape handling.
//!
//! The menu is built from [`MenuItem`]s and popped up from a foreground
//! task, outside GPUI's event dispatch, so GPUI keeps painting while AppKit
//! runs its tracking loop. The chosen item's action runs back inside GPUI
//! once `popUpMenuPositioningItem:` returns.

// objc's macros probe a `cargo-clippy` cfg that this crate does not declare
#![allow(unexpected_cfgs)]
// cocoa 0.26 deprecates its bindings in favour of objc2; GPUI still ships them
#![allow(deprecated)]

use std::cell::Cell;
use std::sync::OnceLock;

use cocoa::appkit::NSMenuItem;
use cocoa::base::{NO, YES, id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSString};
use gpui_kit::*;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::context_menu::{MenuAction, MenuItem, MenuItemKind};
pub use crate::native_menu_common::{
    clear_recorded, pick_recorded, recorded_menu, set_auto_dismiss,
};

thread_local! {
    /// Tag of the item chosen in the current popup, set by `menuAction:`.
    static CHOSEN: Cell<Option<i64>> = const { Cell::new(None) };
}

extern "C" fn menu_action(_this: &Object, _sel: Sel, item: id) {
    let tag: i64 = unsafe { msg_send![item, tag] };
    CHOSEN.with(|c| c.set(Some(tag)));
}

/// One shared `CorvaneMenuTarget` instance receiving every item's action.
fn target() -> id {
    static TARGET: OnceLock<usize> = OnceLock::new();
    *TARGET.get_or_init(|| unsafe {
        let superclass = class!(NSObject);
        let mut decl =
            ClassDecl::new("CorvaneMenuTarget", superclass).expect("class registered once");
        decl.add_method(
            sel!(menuAction:),
            menu_action as extern "C" fn(&Object, Sel, id),
        );
        let class: &'static Class = decl.register();
        let instance: id = msg_send![class, new];
        instance as usize
    }) as id
}

unsafe fn ns_string(text: &str) -> id {
    unsafe { NSString::alloc(nil).init_str(text) }
}

/// Build an `NSMenu` (+1 retained) and collect the actions by tag.
unsafe fn build_menu(items: &[MenuItem], actions: &mut Vec<Option<MenuAction>>) -> id {
    unsafe {
        let menu: id = msg_send![class!(NSMenu), new];
        let _: () = msg_send![menu, setAutoenablesItems: NO];
        for item in items {
            match &item.kind {
                MenuItemKind::Separator => {
                    let sep = NSMenuItem::separatorItem(nil);
                    let _: () = msg_send![menu, addItem: sep];
                }
                MenuItemKind::Action(action) => {
                    let tag = actions.len() as i64;
                    actions.push(Some(action.clone()));
                    let title = ns_string(&item.label);
                    let ns_item: id = msg_send![class!(NSMenuItem), alloc];
                    let ns_item: id = msg_send![ns_item, initWithTitle: title action: sel!(menuAction:) keyEquivalent: ns_string("")];
                    let _: () = msg_send![ns_item, setTarget: target()];
                    let _: () = msg_send![ns_item, setTag: tag];
                    let _: () = msg_send![ns_item, setEnabled: if item.enabled { YES } else { NO }];
                    if item.checked == Some(true) {
                        let _: () = msg_send![ns_item, setState: 1i64];
                    }
                    let _: () = msg_send![menu, addItem: ns_item];
                    let _: () = msg_send![ns_item, release];
                }
                MenuItemKind::Submenu(children) => {
                    actions.push(None);
                    let title = ns_string(&item.label);
                    let ns_item: id = msg_send![class!(NSMenuItem), alloc];
                    let ns_item: id = msg_send![ns_item, initWithTitle: title action: nil keyEquivalent: ns_string("")];
                    let _: () = msg_send![ns_item, setEnabled: if item.enabled { YES } else { NO }];
                    let submenu = build_menu(children, actions);
                    let _: () = msg_send![ns_item, setSubmenu: submenu];
                    let _: () = msg_send![submenu, release];
                    let _: () = msg_send![menu, addItem: ns_item];
                    let _: () = msg_send![ns_item, release];
                }
            }
        }
        menu
    }
}

/// Pop up a native menu with its top-left corner at `position` (window
/// coordinates, top-left origin as GPUI reports them).
pub fn show_context_menu(
    items: Vec<MenuItem>,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let auto_dismiss = crate::native_menu_common::auto_dismiss();
    if auto_dismiss.is_some() {
        crate::native_menu_common::record(&items);
    }
    let ns_view = match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::AppKit(h)) => h.ns_view.as_ptr() as usize,
        _ => return,
    };
    let mut actions: Vec<Option<MenuAction>> = Vec::new();
    let menu = unsafe { build_menu(&items, &mut actions) } as usize;
    let (x, y) = (f64::from(position.x), f64::from(position.y));
    window
        .spawn(cx, async move |cx| {
            let chosen = unsafe {
                let view = ns_view as id;
                let menu = menu as id;
                // GPUI's view is not flipped: AppKit's origin is bottom-left.
                let bounds: NSRect = msg_send![view, bounds];
                let location = NSPoint::new(x, bounds.size.height - y);
                CHOSEN.with(|c| c.set(None));
                if let Some(hold) = auto_dismiss {
                    // performed in NSRunLoopCommonModes, which includes the
                    // event-tracking mode the menu's loop runs in
                    let modes: id = msg_send![class!(NSArray), arrayWithObject: ns_string("kCFRunLoopCommonModes")];
                    let _: () = msg_send![menu, performSelector: sel!(cancelTracking) withObject: nil afterDelay: hold.as_secs_f64() inModes: modes];
                }
                let _: bool = msg_send![menu, popUpMenuPositioningItem: nil atLocation: location inView: view];
                let _: () = msg_send![menu, release];
                CHOSEN.with(|c| c.take())
            };
            if let Some(tag) = chosen
                && let Some(Some(action)) = actions.get(tag as usize)
            {
                cx.update(|window, cx| action(window, cx)).ok();
            }
        })
        .detach();
}
