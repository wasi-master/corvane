//! System accessibility display options (Corvane addition for the high
//! contrast theme; GHD has none): System Settings › Accessibility › Display
//! › "Increase contrast".
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

/// `NSWorkspace.accessibilityDisplayShouldIncreaseContrast`
#[cfg(target_os = "macos")]
pub fn increase_contrast() -> bool {
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    // SAFETY: a class message and a BOOL property read on the shared
    // workspace, which always exists in an AppKit process.
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let increase: BOOL = msg_send![workspace, accessibilityDisplayShouldIncreaseContrast];
        increase != NO
    }
}

#[cfg(not(target_os = "macos"))]
pub fn increase_contrast() -> bool {
    false
}
