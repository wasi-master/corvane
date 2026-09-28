//! Move files to the Trash (GHD `shell.moveItemToTrash` via Electron).
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

use std::path::Path;

/// `NSFileManager trashItemAtURL:` - keeps the "Put Back" affordance.
#[cfg(target_os = "macos")]
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    let c_path = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| "path contains a NUL byte".to_string())?;
    // SAFETY: Foundation message sends; `error` is an out-parameter that
    // Foundation fills with an autoreleased NSError on failure.
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let ns_path: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: c_path.as_ptr()];
        let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_path];
        let manager: *mut Object = msg_send![class!(NSFileManager), defaultManager];
        let mut error: *mut Object = std::ptr::null_mut();
        let ok: BOOL = msg_send![manager, trashItemAtURL: url resultingItemURL: std::ptr::null_mut::<*mut Object>() error: &mut error];
        let result = if ok == NO {
            let message = if error.is_null() {
                "could not move item to Trash".to_string()
            } else {
                let desc: *mut Object = msg_send![error, localizedDescription];
                let cstr: *const c_char = msg_send![desc, UTF8String];
                if cstr.is_null() {
                    "could not move item to Trash".to_string()
                } else {
                    CStr::from_ptr(cstr).to_string_lossy().into_owned()
                }
            };
            Err(message)
        } else {
            Ok(())
        };
        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(not(target_os = "macos"))]
pub fn move_to_trash(_path: &Path) -> Result<(), String> {
    Err("moving to Trash is not supported on this platform yet".into())
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "macos")]
    fn trashing_a_missing_path_fails_cleanly() {
        let err = super::move_to_trash(std::path::Path::new(
            "/tmp/corvane-definitely-missing-4f2a9c",
        ))
        .unwrap_err();
        assert!(!err.is_empty());
    }
}
