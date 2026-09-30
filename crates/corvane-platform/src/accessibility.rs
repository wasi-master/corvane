//! System accessibility display options (Corvane addition for the high
//! contrast theme; GHD has none): System Settings › Accessibility › Display
//! › "Increase contrast" on macOS; on Linux the XDG desktop portal's
//! `org.freedesktop.appearance` `contrast` setting (GNOME's Accessibility ›
//! High Contrast, KDE's high-contrast colour schemes).
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

/// Portal `Settings.ReadOne("org.freedesktop.appearance", "contrast")`:
/// `1` is higher contrast. `false` without a portal (or before it answers
/// within 250 ms, as this runs when the window is activated).
#[cfg(not(target_os = "macos"))]
pub fn increase_contrast() -> bool {
    portal_contrast() == Some(1)
}

#[cfg(not(target_os = "macos"))]
fn portal_contrast() -> Option<u32> {
    use std::sync::OnceLock;
    use std::time::Duration;

    use zbus::zvariant::OwnedValue;

    static BUS: OnceLock<Option<zbus::blocking::Connection>> = OnceLock::new();
    let bus = BUS
        .get_or_init(|| {
            zbus::blocking::connection::Builder::session()
                .ok()?
                .method_timeout(Duration::from_millis(250))
                .build()
                .ok()
        })
        .as_ref()?;
    let call = |method: &str| {
        bus.call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            method,
            &("org.freedesktop.appearance", "contrast"),
        )
    };
    // `ReadOne` (Settings v2) returns the value; the older `Read` wraps it
    // in one more variant
    let reply = call("ReadOne").or_else(|_| call("Read")).ok()?;
    let value: OwnedValue = reply.body().deserialize().ok()?;
    contrast_value(&value)
}

/// The `u` inside the reply, unwrapping `Read`'s extra variant.
#[cfg(not(target_os = "macos"))]
fn contrast_value(value: &zbus::zvariant::Value<'_>) -> Option<u32> {
    use zbus::zvariant::Value;
    match value {
        Value::U32(v) => Some(*v),
        Value::Value(inner) => contrast_value(inner),
        _ => None,
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use zbus::zvariant::Value;

    #[test]
    fn unwraps_both_portal_replies() {
        assert_eq!(super::contrast_value(&Value::U32(1)), Some(1));
        let read = Value::Value(Box::new(Value::U32(0)));
        assert_eq!(super::contrast_value(&read), Some(0));
        assert_eq!(super::contrast_value(&Value::Str("x".into())), None);
    }
}
