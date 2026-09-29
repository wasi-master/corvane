//! Notification permission (GHD `desktop-notifications`
//! `getNotificationsPermission` / `requestNotificationsPermission` /
//! `getNotificationSettingsUrl`): `UNUserNotificationCenter` on macOS.
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

/// GHD `NotificationPermission`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationPermission {
    /// Not asked yet (`UNAuthorizationStatusNotDetermined`).
    Default,
    Granted,
    Denied,
    /// No notification centre (not running from an app bundle).
    Unsupported,
}

/// The current authorization status, asked synchronously (with a short
/// timeout: the centre answers on its own queue).
#[cfg(target_os = "macos")]
pub fn permission() -> NotificationPermission {
    use std::sync::mpsc;
    use std::time::Duration;

    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "UserNotifications", kind = "framework")]
    unsafe extern "C" {}

    let (tx, rx) = mpsc::channel::<i64>();
    // SAFETY: message sends on the shared notification centre; the block
    // copies `tx` and is invoked at most once by the framework.
    unsafe {
        let center: *mut Object =
            msg_send![class!(UNUserNotificationCenter), currentNotificationCenter];
        if center.is_null() {
            return NotificationPermission::Unsupported;
        }
        let block = ConcreteBlock::new(move |settings: *mut Object| {
            let status: i64 = if settings.is_null() {
                -1
            } else {
                msg_send![settings, authorizationStatus]
            };
            let _ = tx.send(status);
        });
        let block = block.copy();
        let _: () = msg_send![center, getNotificationSettingsWithCompletionHandler: &*block];
    }
    match rx.recv_timeout(Duration::from_secs(2)) {
        // UNAuthorizationStatus: notDetermined 0, denied 1, authorized 2,
        // provisional 3, ephemeral 4
        Ok(0) => NotificationPermission::Default,
        Ok(1) => NotificationPermission::Denied,
        Ok(2..=4) => NotificationPermission::Granted,
        _ => NotificationPermission::Unsupported,
    }
}

/// Ask the user (the system prompt is asynchronous; poll [`permission`] after).
#[cfg(target_os = "macos")]
pub fn request_permission() {
    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    // UNAuthorizationOptionBadge | Sound | Alert
    const OPTIONS: u64 = 1 | 2 | 4;
    // SAFETY: as above; the completion block ignores its arguments.
    unsafe {
        let center: *mut Object =
            msg_send![class!(UNUserNotificationCenter), currentNotificationCenter];
        if center.is_null() {
            return;
        }
        let block = ConcreteBlock::new(move |_granted: bool, _error: *mut Object| {});
        let block = block.copy();
        let _: () =
            msg_send![center, requestAuthorizationWithOptions: OPTIONS completionHandler: &*block];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn permission() -> NotificationPermission {
    NotificationPermission::Unsupported
}

#[cfg(not(target_os = "macos"))]
pub fn request_permission() {}

/// GHD `getNotificationSettingsUrl`: System Settings › Notifications for this app.
pub fn settings_url(bundle_id: &str) -> String {
    format!("x-apple.systempreferences:com.apple.preference.notifications?id={bundle_id}")
}
