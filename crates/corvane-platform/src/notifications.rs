//! OS notifications (GHD `vendor/desktop-notifications`:
//! `src/mac/GHDesktopNotificationsManager.m`, `lib/index.ts`):
//! `UNUserNotificationCenter` on macOS.
//!
//! - permission: `getNotificationsPermission` / `requestNotificationsPermission`
//!   / `getNotificationSettingsUrl`;
//! - posting: `showNotificationWithIdentifier:title:body:userInfo:` (asks for
//!   authorization first, then adds an immediate request with the default
//!   sound);
//! - clicks: a `UNUserNotificationCenterDelegate` (`CorvaneNotificationDelegate`)
//!   hands the identifier and the `userInfo` payload to the handler installed
//!   with [`install_click_handler`], and lets notifications show while
//!   Corvane is frontmost (`willPresentNotification:`).
//!
//! The notification centre only exists for a process started from an app
//! bundle (it raises an Objective-C exception otherwise), so every entry
//! point checks for a bundle identifier first and reports
//! [`NotificationPermission::Unsupported`] / does nothing without one.
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

/// A click on one of Corvane's notifications (GHD `notification-event`
/// `click`, `id`, `userInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationClick {
    pub identifier: String,
    /// The `payload` string posted with [`show`], if any.
    pub payload: Option<String>,
}

/// Why a notification was not shown.
#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("notifications need Corvane to run from its app bundle")]
    Unsupported,
    /// With the centre's error, when it gave one.
    #[error("permission to display notifications wasn't granted{}", .0.as_deref().map(|e| format!(": {e}")).unwrap_or_default())]
    NotGranted(Option<String>),
    #[error("could not post the notification: {0}")]
    Post(String),
}

/// Key of the payload string in the notification's `userInfo`.
#[cfg(target_os = "macos")]
const PAYLOAD_KEY: &str = "corvane-payload";

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::CStr;
    use std::sync::OnceLock;

    use block::Block;
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Protocol, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    use super::NotificationClick;

    #[link(name = "UserNotifications", kind = "framework")]
    unsafe extern "C" {}

    pub type ClickHandler = Box<dyn Fn(NotificationClick) + Send + Sync>;
    pub static CLICK_HANDLER: OnceLock<ClickHandler> = OnceLock::new();

    /// `[NSBundle mainBundle].bundleIdentifier != nil`: the centre exists.
    pub fn has_bundle() -> bool {
        // SAFETY: class messages on NSBundle; both may return nil, checked.
        unsafe {
            let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
            if bundle.is_null() {
                return false;
            }
            let identifier: *mut Object = msg_send![bundle, bundleIdentifier];
            !identifier.is_null()
        }
    }

    /// `[UNUserNotificationCenter currentNotificationCenter]`, when there is one.
    pub fn center() -> Option<*mut Object> {
        if !has_bundle() {
            return None;
        }
        // SAFETY: only reached from a bundled process (see `has_bundle`).
        let center: *mut Object =
            unsafe { msg_send![class!(UNUserNotificationCenter), currentNotificationCenter] };
        (!center.is_null()).then_some(center)
    }

    /// A +1 retained `NSString` (the caller releases it).
    pub unsafe fn ns_string(text: &str) -> *mut Object {
        // NSUTF8StringEncoding
        const UTF8: u64 = 4;
        // SAFETY: `initWithBytes:length:encoding:` copies the bytes.
        unsafe {
            let s: *mut Object = msg_send![class!(NSString), alloc];
            msg_send![s, initWithBytes: text.as_ptr() length: text.len() encoding: UTF8]
        }
    }

    /// The UTF-8 contents of an `NSString` (`None` for nil).
    pub unsafe fn rust_string(s: *mut Object) -> Option<String> {
        if s.is_null() {
            return None;
        }
        // SAFETY: `UTF8String` is valid while `s` lives; copied right away.
        unsafe {
            let ptr: *const std::os::raw::c_char = msg_send![s, UTF8String];
            (!ptr.is_null()).then(|| CStr::from_ptr(ptr).to_string_lossy().into_owned())
        }
    }

    /// `userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:`
    extern "C" fn did_receive(
        _this: &Object,
        _sel: Sel,
        _center: *mut Object,
        response: *mut Object,
        completion: *mut Object,
    ) {
        // blocks are objects; the method's type encoding uses `id`
        let completion = completion as *mut Block<(), ()>;
        // SAFETY: `response` is the framework's live UNNotificationResponse;
        // every step may be nil and is checked. The completion block is
        // called exactly once, as the delegate contract requires.
        unsafe {
            let click = (!response.is_null()).then(|| {
                let notification: *mut Object = msg_send![response, notification];
                let request: *mut Object = msg_send![notification, request];
                let identifier: *mut Object = msg_send![request, identifier];
                let content: *mut Object = msg_send![request, content];
                let user_info: *mut Object = msg_send![content, userInfo];
                let payload = if user_info.is_null() {
                    None
                } else {
                    let key = ns_string(super::PAYLOAD_KEY);
                    let value: *mut Object = msg_send![user_info, objectForKey: key];
                    let _: () = msg_send![key, release];
                    rust_string(value)
                };
                NotificationClick {
                    identifier: rust_string(identifier).unwrap_or_default(),
                    payload,
                }
            });
            if let (Some(click), Some(handler)) = (click, CLICK_HANDLER.get()) {
                handler(click);
            }
            if !completion.is_null() {
                (*completion).call(());
            }
        }
    }

    /// `userNotificationCenter:willPresentNotification:withCompletionHandler:`:
    /// show the banner even while Corvane is frontmost (GHD passes alert,
    /// badge and sound; `alert` is `banner | list` since macOS 11).
    extern "C" fn will_present(
        _this: &Object,
        _sel: Sel,
        _center: *mut Object,
        _notification: *mut Object,
        completion: *mut Object,
    ) {
        let completion = completion as *mut Block<(u64,), ()>;
        // UNNotificationPresentationOptionBadge | Sound | List | Banner
        const OPTIONS: u64 = 1 | 2 | 8 | 16;
        if !completion.is_null() {
            // SAFETY: the framework's completion block, called once.
            unsafe { (*completion).call((OPTIONS,)) };
        }
    }

    /// One `CorvaneNotificationDelegate` for the process (the centre holds
    /// its delegate weakly, so the instance is never released).
    pub fn delegate() -> *mut Object {
        static DELEGATE: OnceLock<usize> = OnceLock::new();
        *DELEGATE.get_or_init(|| {
            // SAFETY: the class is declared once (guarded by the OnceLock)
            // with methods whose signatures match the protocol's selectors.
            unsafe {
                let class: &'static Class = match ClassDecl::new(
                    "CorvaneNotificationDelegate",
                    class!(NSObject),
                ) {
                    Some(mut decl) => {
                        if let Some(protocol) = Protocol::get("UNUserNotificationCenterDelegate") {
                            decl.add_protocol(protocol);
                        }
                        decl.add_method(
                            sel!(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:),
                            did_receive
                                as extern "C" fn(
                                    &Object,
                                    Sel,
                                    *mut Object,
                                    *mut Object,
                                    *mut Object,
                                ),
                        );
                        decl.add_method(
                            sel!(userNotificationCenter:willPresentNotification:withCompletionHandler:),
                            will_present
                                as extern "C" fn(
                                    &Object,
                                    Sel,
                                    *mut Object,
                                    *mut Object,
                                    *mut Object,
                                ),
                        );
                        decl.register()
                    }
                    None => class!(CorvaneNotificationDelegate),
                };
                let instance: *mut Object = msg_send![class, new];
                instance as usize
            }
        }) as *mut Object
    }
}

/// The current authorization status, asked synchronously (with a short
/// timeout: the centre answers on its own queue).
#[cfg(target_os = "macos")]
pub fn permission() -> NotificationPermission {
    use std::sync::mpsc;
    use std::time::Duration;

    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let Some(center) = mac::center() else {
        return NotificationPermission::Unsupported;
    };
    let (tx, rx) = mpsc::channel::<i64>();
    // SAFETY: message sends on the shared notification centre; the block
    // copies `tx` and is invoked at most once by the framework.
    unsafe {
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

/// UNAuthorizationOptionBadge | Sound | Alert
#[cfg(target_os = "macos")]
const AUTHORIZATION_OPTIONS: u64 = 1 | 2 | 4;

/// Ask the user (the system prompt is asynchronous; poll [`permission`] after).
#[cfg(target_os = "macos")]
pub fn request_permission() {
    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let Some(center) = mac::center() else {
        return;
    };
    // SAFETY: as above; the completion block ignores its arguments.
    unsafe {
        let block = ConcreteBlock::new(move |_granted: bool, _error: *mut Object| {});
        let block = block.copy();
        let _: () = msg_send![center, requestAuthorizationWithOptions: AUTHORIZATION_OPTIONS completionHandler: &*block];
    }
}

/// GHD `showNotification`: post `title` / `body` now, with `payload` in the
/// `userInfo` so a click (even in a later session) can hand it back.
/// Authorization is requested first, as GHD does; `done` runs on the
/// centre's queue with the outcome.
#[cfg(target_os = "macos")]
pub fn show(
    identifier: &str,
    title: &str,
    body: &str,
    payload: Option<&str>,
    done: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
) {
    use std::sync::{Arc, Mutex};

    use block::ConcreteBlock;
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    let Some(center) = mac::center() else {
        done(Err(NotificationError::Unsupported));
        return;
    };
    type Done = Box<dyn FnOnce(Result<(), NotificationError>) + Send>;
    let done: Arc<Mutex<Option<Done>>> = Arc::new(Mutex::new(Some(Box::new(done))));
    let finish = move |done: &Arc<Mutex<Option<Done>>>, result| {
        if let Some(done) = done.lock().ok().and_then(|mut d| d.take()) {
            done(result);
        }
    };
    // SAFETY: content and request are built on this thread and retained by
    // the request / centre; the NSStrings created here are released once
    // handed over. The blocks own clones of `done` and run at most once.
    unsafe {
        let content: *mut Object = msg_send![class!(UNMutableNotificationContent), new];
        let ns_title = mac::ns_string(title);
        let ns_body = mac::ns_string(body);
        let _: () = msg_send![content, setTitle: ns_title];
        let _: () = msg_send![content, setBody: ns_body];
        let _: () = msg_send![ns_title, release];
        let _: () = msg_send![ns_body, release];
        let sound: *mut Object = msg_send![class!(UNNotificationSound), defaultSound];
        let _: () = msg_send![content, setSound: sound];
        if let Some(payload) = payload {
            let key = mac::ns_string(PAYLOAD_KEY);
            let value = mac::ns_string(payload);
            let user_info: *mut Object =
                msg_send![class!(NSDictionary), dictionaryWithObject: value forKey: key];
            let _: () = msg_send![content, setUserInfo: user_info];
            let _: () = msg_send![key, release];
            let _: () = msg_send![value, release];
        }
        let ns_identifier = mac::ns_string(identifier);
        let nil: *mut Object = std::ptr::null_mut();
        let request: *mut Object = msg_send![class!(UNNotificationRequest), requestWithIdentifier: ns_identifier content: content trigger: nil];
        let _: () = msg_send![ns_identifier, release];
        let _: () = msg_send![content, release];
        // retained until the authorization answer adds it
        let request: *mut Object = msg_send![request, retain];
        let request = request as usize;
        let center_ptr = center as usize;

        let authorized = ConcreteBlock::new(move |granted: BOOL, error: *mut Object| {
            let request = request as *mut Object;
            if granted == NO || !error.is_null() {
                let _: () = msg_send![request, release];
                let reason = if error.is_null() {
                    None
                } else {
                    let description: *mut Object = msg_send![error, localizedDescription];
                    mac::rust_string(description)
                };
                finish(&done, Err(NotificationError::NotGranted(reason)));
                return;
            }
            let done = done.clone();
            let added = ConcreteBlock::new(move |error: *mut Object| {
                let result = if error.is_null() {
                    Ok(())
                } else {
                    let description: *mut Object = msg_send![error, localizedDescription];
                    Err(NotificationError::Post(
                        mac::rust_string(description).unwrap_or_default(),
                    ))
                };
                finish(&done, result);
            });
            let added = added.copy();
            let center = center_ptr as *mut Object;
            let _: () =
                msg_send![center, addNotificationRequest: request withCompletionHandler: &*added];
            let _: () = msg_send![request, release];
        });
        let authorized = authorized.copy();
        let _: () = msg_send![center, requestAuthorizationWithOptions: AUTHORIZATION_OPTIONS completionHandler: &*authorized];
    }
}

/// GHD `onNotificationEvent` + `configureNotificationsDelegate`: make
/// Corvane the centre's delegate and send every click to `handler`
/// (called on whatever queue the centre delivers on). Install once, early
/// at launch, so a click that starts Corvane is delivered too.
#[cfg(target_os = "macos")]
pub fn install_click_handler(handler: impl Fn(NotificationClick) + Send + Sync + 'static) {
    use objc::{msg_send, sel, sel_impl};

    if mac::CLICK_HANDLER.set(Box::new(handler)).is_err() {
        return;
    }
    let Some(center) = mac::center() else {
        return;
    };
    let delegate = mac::delegate();
    // SAFETY: the delegate instance lives for the whole process.
    unsafe {
        let _: () = msg_send![center, setDelegate: delegate];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn permission() -> NotificationPermission {
    NotificationPermission::Unsupported
}

#[cfg(not(target_os = "macos"))]
pub fn request_permission() {}

#[cfg(not(target_os = "macos"))]
pub fn show(
    _identifier: &str,
    _title: &str,
    _body: &str,
    _payload: Option<&str>,
    done: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
) {
    done(Err(NotificationError::Unsupported));
}

#[cfg(not(target_os = "macos"))]
pub fn install_click_handler(_handler: impl Fn(NotificationClick) + Send + Sync + 'static) {}

/// GHD `getNotificationSettingsUrl`: System Settings › Notifications for this app.
pub fn settings_url(bundle_id: &str) -> String {
    format!("x-apple.systempreferences:com.apple.preference.notifications?id={bundle_id}")
}
