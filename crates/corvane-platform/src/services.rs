//! Finder Services "Open in Corvane" (Corvane addition; GHD has no
//! service). `Info.plist` declares an `NSServices` entry (message
//! `openInCorvane`, `NSSendFileTypes` `public.folder`); this module
//! registers the `CorvaneServicesProvider` object that AppKit calls with the
//! pasteboard holding the folders selected in Finder.
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{CStr, c_void};
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    pub type Handler = Box<dyn Fn(PathBuf) + Send + Sync>;
    pub static HANDLER: OnceLock<Handler> = OnceLock::new();

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        fn NSUpdateDynamicServices();
    }

    /// The file URLs on `pboard`, as paths.
    unsafe fn file_paths(pboard: *mut Object) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if pboard.is_null() {
            return out;
        }
        // SAFETY: `pboard` is the live NSPasteboard AppKit passed in; every
        // returned object is checked for nil before use.
        unsafe {
            let classes: *mut Object = msg_send![class!(NSArray), arrayWithObject: class!(NSURL)];
            let nil: *mut Object = std::ptr::null_mut();
            let urls: *mut Object = msg_send![pboard, readObjectsForClasses: classes options: nil];
            if urls.is_null() {
                return out;
            }
            let count: usize = msg_send![urls, count];
            for ix in 0..count {
                let url: *mut Object = msg_send![urls, objectAtIndex: ix];
                let is_file: bool = msg_send![url, isFileURL];
                if !is_file {
                    continue;
                }
                let path: *mut Object = msg_send![url, path];
                if path.is_null() {
                    continue;
                }
                let utf8: *const std::os::raw::c_char = msg_send![path, UTF8String];
                if !utf8.is_null() {
                    out.push(PathBuf::from(
                        CStr::from_ptr(utf8).to_string_lossy().into_owned(),
                    ));
                }
            }
        }
        out
    }

    /// `-openInCorvane:userData:error:` (the `NSMessage` in Info.plist).
    extern "C" fn open_in_corvane(
        _this: &Object,
        _sel: Sel,
        pboard: *mut Object,
        _user_data: *mut Object,
        _error: *mut c_void,
    ) {
        // SAFETY: see `file_paths`.
        let paths = unsafe { file_paths(pboard) };
        if let Some(handler) = HANDLER.get() {
            for path in paths {
                handler(path);
            }
        }
    }

    /// One `CorvaneServicesProvider` for the process, never released
    /// (`NSApp.servicesProvider` does not retain it on every macOS).
    pub fn provider() -> *mut Object {
        static PROVIDER: OnceLock<usize> = OnceLock::new();
        *PROVIDER.get_or_init(|| {
            // SAFETY: declared once under the OnceLock; the method matches
            // the service selector's `(NSPasteboard *, NSString *, NSString **)`.
            unsafe {
                let class: &'static Class =
                    match ClassDecl::new("CorvaneServicesProvider", class!(NSObject)) {
                        Some(mut decl) => {
                            decl.add_method(
                                sel!(openInCorvane:userData:error:),
                                open_in_corvane
                                    as extern "C" fn(
                                        &Object,
                                        Sel,
                                        *mut Object,
                                        *mut Object,
                                        *mut c_void,
                                    ),
                            );
                            decl.register()
                        }
                        None => class!(CorvaneServicesProvider),
                    };
                let instance: *mut Object = msg_send![class, new];
                instance as usize
            }
        }) as *mut Object
    }

    /// `NSApp.servicesProvider = provider`, then refresh the Services menu.
    pub fn register() {
        // SAFETY: called on the main thread once NSApp exists (inside the
        // GPUI run callback); the provider lives for the whole process.
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            if app.is_null() {
                return;
            }
            let _: () = msg_send![app, setServicesProvider: provider()];
            NSUpdateDynamicServices();
        }
    }
}

/// Make "Open in Corvane" work: `handler` gets each folder picked in
/// Finder (on the main thread). Call once, from inside the app's run loop.
#[cfg(target_os = "macos")]
pub fn register_open_in_corvane(handler: impl Fn(PathBuf) + Send + Sync + 'static) {
    if mac::HANDLER.set(Box::new(handler)).is_err() {
        return;
    }
    mac::register();
}

#[cfg(not(target_os = "macos"))]
pub fn register_open_in_corvane(_handler: impl Fn(PathBuf) + Send + Sync + 'static) {}
