//! Hiding and showing the main window the way GHD's main process does on
//! macOS (`app/src/main-process/app-window.ts`): Window › Close Window and
//! the red close button call `BrowserWindow.hide()` instead of closing, and
//! the Dock icon (`app.on('activate')`) shows it again. GPUI has no
//! per-window hide, so this sends `orderOut:` / `makeKeyAndOrderFront:` to
//! the `NSWindow` from a foreground task, outside GPUI's event dispatch
//! (AppKit calls back into GPUI while the window changes key status).

// objc's macros probe a `cargo-clippy` cfg that this crate does not declare
#![allow(unexpected_cfgs)]

use gpui_kit::*;
use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// The `NSWindow` behind `window`, as an address that can cross into a task.
fn ns_window(window: &Window) -> Option<usize> {
    let view = match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
        Ok(RawWindowHandle::AppKit(h)) => h.ns_view.as_ptr() as *mut Object,
        _ => return None,
    };
    let ns_window: *mut Object = unsafe { msg_send![view, window] };
    (!ns_window.is_null()).then_some(ns_window as usize)
}

/// `NSWindowStyleMaskFullScreen`
const FULL_SCREEN: usize = 1 << 14;

fn is_full_screen(ns_window: usize) -> bool {
    let mask: usize = unsafe { msg_send![ns_window as *mut Object, styleMask] };
    mask & FULL_SCREEN != 0
}

/// `BrowserWindow.hide()`: order the window out; the app stays active. A
/// full-screen window leaves full screen first (GHD desktop/desktop#12838).
pub fn hide_window(window: &Window, cx: &mut App) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    cx.spawn(async move |cx: &mut AsyncApp| {
        let nil: *mut Object = std::ptr::null_mut();
        if is_full_screen(ns_window) {
            let _: () = unsafe { msg_send![ns_window as *mut Object, toggleFullScreen: nil] };
            // `once('leave-full-screen')`: the animation takes about a second
            for _ in 0..40 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                if !is_full_screen(ns_window) {
                    break;
                }
            }
        }
        let _: () = unsafe { msg_send![ns_window as *mut Object, orderOut: nil] };
    })
    .detach();
}

/// `BrowserWindow.show()`: bring the window back and make it key.
pub fn show_window(window: &Window, cx: &mut App) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    cx.spawn(async move |_| unsafe {
        let nil: *mut Object = std::ptr::null_mut();
        let _: () = msg_send![ns_window as *mut Object, makeKeyAndOrderFront: nil];
    })
    .detach();
}
