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

/// Linux: the territory of the locale Chromium's UI locale comes from, the
/// first set of `LC_ALL`, `LC_MESSAGES`, `LANG` (`en_GB.UTF-8` → `GB`);
/// `C` / `POSIX` have none.
#[cfg(not(target_os = "macos"))]
pub fn country_code() -> Option<String> {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let locale = var("LC_ALL")
        .or_else(|| var("LC_MESSAGES"))
        .or_else(|| var("LANG"))?;
    territory(&locale)
}

/// `ll_CC[.codeset][@modifier]` → `CC`.
#[cfg(not(target_os = "macos"))]
fn territory(locale: &str) -> Option<String> {
    let base = locale.split(['.', '@']).next()?;
    let (_, country) = base.split_once('_')?;
    (country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic()))
        .then(|| country.to_ascii_uppercase())
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    #[test]
    fn territories() {
        assert_eq!(super::territory("en_GB.UTF-8").as_deref(), Some("GB"));
        assert_eq!(super::territory("de_DE@euro").as_deref(), Some("DE"));
        assert_eq!(super::territory("sr_RS.UTF-8@latin").as_deref(), Some("RS"));
        assert_eq!(super::territory("C.UTF-8"), None);
        assert_eq!(super::territory("POSIX"), None);
    }
}
