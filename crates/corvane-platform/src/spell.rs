//! Spellcheck for the commit form. GHD gets Chromium's built-in checker
//! (`commitSpellcheckEnabled`, `webContents.session.setSpellCheckerEnabled`);
//! Corvane asks `NSSpellChecker` on macOS. Word ranges come back as UTF-8
//! byte ranges into the checked string.
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

use std::ops::Range;

/// Every misspelled word in `text`, in document order.
#[cfg(target_os = "macos")]
pub fn misspelled_ranges(text: &str) -> Vec<Range<usize>> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    if text.trim().is_empty() {
        return Vec::new();
    }
    let Some(c_text) = std::ffi::CString::new(text).ok() else {
        return Vec::new();
    };
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let mut out = Vec::new();
    // SAFETY: message sends on the shared spell checker with an autoreleased
    // NSString; `NSRange` is two `usize`s on 64-bit macOS.
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let ns: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c_text.as_ptr()];
        let checker: *mut Object = msg_send![class!(NSSpellChecker), sharedSpellChecker];
        let mut start: usize = 0;
        let len = utf16.len();
        while start < len {
            let range: NSRange = msg_send![checker, checkSpellingOfString: ns startingAt: start];
            if range.location == NS_NOT_FOUND || range.length == 0 || range.location < start {
                break;
            }
            let end = range.location + range.length;
            if let (Some(a), Some(b)) = (
                utf16_to_byte(text, &utf16, range.location),
                utf16_to_byte(text, &utf16, end),
            ) {
                out.push(a..b);
            }
            start = end;
        }
        let _: () = msg_send![pool, drain];
    }
    out
}

/// Spelling suggestions for `word` (Chromium shows up to five).
#[cfg(target_os = "macos")]
pub fn guesses(word: &str) -> Vec<String> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use std::os::raw::c_char;

    let Some(c_word) = std::ffi::CString::new(word).ok() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // SAFETY: as above; the returned array and its strings are autoreleased.
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let ns: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c_word.as_ptr()];
        let checker: *mut Object = msg_send![class!(NSSpellChecker), sharedSpellChecker];
        let len: usize = msg_send![ns, length];
        let range = NSRange {
            location: 0,
            length: len,
        };
        let nil: *mut Object = std::ptr::null_mut();
        let array: *mut Object = msg_send![checker, guessesForWordRange: range inString: ns language: nil inSpellDocumentWithTag: 0usize];
        if !array.is_null() {
            let count: usize = msg_send![array, count];
            for ix in 0..count.min(5) {
                let item: *mut Object = msg_send![array, objectAtIndex: ix];
                let cstr: *const c_char = msg_send![item, UTF8String];
                if !cstr.is_null() {
                    out.push(
                        std::ffi::CStr::from_ptr(cstr)
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        let _: () = msg_send![pool, drain];
    }
    out
}

/// "Add to Dictionary": the user's learned-words list.
#[cfg(target_os = "macos")]
pub fn learn_word(word: &str) {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    let Some(c_word) = std::ffi::CString::new(word).ok() else {
        return;
    };
    // SAFETY: as above.
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let ns: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c_word.as_ptr()];
        let checker: *mut Object = msg_send![class!(NSSpellChecker), sharedSpellChecker];
        let _: () = msg_send![checker, learnWord: ns];
        let _: () = msg_send![pool, drain];
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
struct NSRange {
    location: usize,
    length: usize,
}

#[cfg(target_os = "macos")]
unsafe impl objc::Encode for NSRange {
    fn encode() -> objc::Encoding {
        // SAFETY: `NSRange` is `{NSUInteger, NSUInteger}` on 64-bit targets.
        unsafe { objc::Encoding::from_str("{_NSRange=QQ}") }
    }
}

#[cfg(target_os = "macos")]
const NS_NOT_FOUND: usize = i64::MAX as usize;

/// Byte offset of the `n`th UTF-16 code unit of `text`.
#[cfg(target_os = "macos")]
fn utf16_to_byte(text: &str, utf16: &[u16], n: usize) -> Option<usize> {
    if n == utf16.len() {
        return Some(text.len());
    }
    let mut units = 0usize;
    for (byte, ch) in text.char_indices() {
        if units == n {
            return Some(byte);
        }
        units += ch.len_utf16();
    }
    None
}

#[cfg(not(target_os = "macos"))]
pub fn misspelled_ranges(_text: &str) -> Vec<Range<usize>> {
    Vec::new()
}

#[cfg(not(target_os = "macos"))]
pub fn guesses(_word: &str) -> Vec<String> {
    Vec::new()
}

#[cfg(not(target_os = "macos"))]
pub fn learn_word(_word: &str) {}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn flags_a_misspelling() {
        let text = "fix wrold thing";
        let ranges = misspelled_ranges(text);
        assert!(
            ranges.iter().any(|r| &text[r.clone()] == "wrold"),
            "{ranges:?}"
        );
        assert!(misspelled_ranges("hello world").is_empty());
    }

    #[test]
    fn maps_utf16_offsets_to_bytes() {
        let text = "café wrold";
        let ranges = misspelled_ranges(text);
        assert!(
            ranges.iter().any(|r| &text[r.clone()] == "wrold"),
            "{ranges:?}"
        );
    }
}
