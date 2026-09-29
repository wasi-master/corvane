//! The OS locale's country (GHD reads it from Chromium's locale, `lc=XX` in
//! the window URL) for the formatting defaults.
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

/// ISO 3166 country code of the current locale (`NSLocale.countryCode`).
#[cfg(target_os = "macos")]
pub fn country_code() -> Option<String> {
    use std::os::raw::c_char;

    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {}

    // SAFETY: plain message sends on Foundation classes; the returned string
    // is autoreleased and copied before the pool drains.
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let locale: *mut Object = msg_send![class!(NSLocale), currentLocale];
        let code: *mut Object = msg_send![locale, countryCode];
        let result = if code.is_null() {
            None
        } else {
            let cstr: *const c_char = msg_send![code, UTF8String];
            if cstr.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr(cstr)
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        };
        let _: () = msg_send![pool, drain];
        result.filter(|c| c.len() == 2)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn country_code() -> Option<String> {
    std::env::var("LANG")
        .ok()
        .and_then(|lang| {
            lang.split('.')
                .next()
                .and_then(|l| l.split('_').nth(1))
                .map(String::from)
        })
        .filter(|c| c.len() == 2)
}
