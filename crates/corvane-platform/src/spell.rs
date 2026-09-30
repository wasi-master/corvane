//! Spellcheck for the commit form. GHD gets Chromium's built-in checker
//! (`commitSpellcheckEnabled`, `webContents.session.setSpellCheckerEnabled`);
//! Corvane asks `NSSpellChecker` on macOS. Word ranges come back as UTF-8
//! byte ranges into the checked string.
//!
//! Linux: Chromium checks with Hunspell there, against `.bdic` dictionaries
//! it downloads for the UI language. Corvane checks with Hunspell too
//! (`spellbook`), against the system's dictionaries
//! (`/usr/share/hunspell/<lang>.{aff,dic}`, e.g. Ubuntu's `hunspell-en-us`)
//! for `LC_ALL` / `LC_MESSAGES` / `LANG`, falling back to `en_US`; "Add to
//! Dictionary" appends to `custom-dictionary.txt` in the data folder
//! (Chromium: `Custom Dictionary.txt` in the profile). Without an installed
//! dictionary nothing is flagged.
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
mod hunspell {
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    use spellbook::Dictionary;

    /// Folders distributions install Hunspell dictionaries into.
    const DICTIONARY_DIRS: &[&str] = &[
        "/usr/share/hunspell",
        "/usr/share/myspell",
        "/usr/share/myspell/dicts",
    ];

    /// `en_GB.UTF-8` → `en_GB`, from the first set locale variable.
    fn wanted_language() -> String {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        var("LC_ALL")
            .or_else(|| var("LC_MESSAGES"))
            .or_else(|| var("LANG"))
            .and_then(|l| l.split(['.', '@']).next().map(str::to_string))
            .filter(|l| l != "C" && l != "POSIX")
            .unwrap_or_else(|| "en_US".to_string())
    }

    fn load(language: &str) -> Option<Dictionary> {
        DICTIONARY_DIRS.iter().find_map(|dir| {
            let aff = std::fs::read_to_string(format!("{dir}/{language}.aff")).ok()?;
            let dic = std::fs::read_to_string(format!("{dir}/{language}.dic")).ok()?;
            Dictionary::new(&aff, &dic)
                .map_err(|err| tracing::warn!(%err, language, "unreadable Hunspell dictionary"))
                .ok()
        })
    }

    pub fn custom_words_path() -> PathBuf {
        crate::paths::app_support_dir().join("custom-dictionary.txt")
    }

    /// The dictionary (loaded on first use, with the learned words), or
    /// `None` when no dictionary is installed.
    pub fn dictionary() -> &'static Mutex<Option<Dictionary>> {
        static DICTIONARY: OnceLock<Mutex<Option<Dictionary>>> = OnceLock::new();
        DICTIONARY.get_or_init(|| {
            let language = wanted_language();
            let mut dictionary = load(&language).or_else(|| load("en_US"));
            if let Some(dictionary) = dictionary.as_mut()
                && let Ok(words) = std::fs::read_to_string(custom_words_path())
            {
                for word in words.lines().filter(|w| !w.trim().is_empty()) {
                    let _ = dictionary.add(word.trim());
                }
            }
            if dictionary.is_none() {
                tracing::info!(%language, "no Hunspell dictionary installed: spellcheck is off");
            }
            Mutex::new(dictionary)
        })
    }
}

/// Words as Chromium's spellchecker splits them: letters with inner
/// apostrophes; tokens with digits are skipped. UTF-8 byte ranges.
#[cfg(not(target_os = "macos"))]
fn words(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut has_digit = false;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let inner_apostrophe = (c == '\'' || c == '’')
            && start.is_some()
            && chars.peek().is_some_and(|(_, n)| n.is_alphabetic());
        if c.is_alphanumeric() || inner_apostrophe {
            has_digit |= c.is_numeric();
            start.get_or_insert(i);
        } else if let Some(s) = start.take() {
            if !has_digit {
                out.push(s..i);
            }
            has_digit = false;
        }
    }
    if let Some(s) = start
        && !has_digit
    {
        out.push(s..text.len());
    }
    out
}

#[cfg(not(target_os = "macos"))]
pub fn misspelled_ranges(text: &str) -> Vec<Range<usize>> {
    let Ok(guard) = hunspell::dictionary().lock() else {
        return Vec::new();
    };
    let Some(dictionary) = guard.as_ref() else {
        return Vec::new();
    };
    words(text)
        .into_iter()
        .filter(|r| !dictionary.check(&text[r.clone()]))
        .collect()
}

/// Spelling suggestions for `word` (Chromium shows up to five).
#[cfg(not(target_os = "macos"))]
pub fn guesses(word: &str) -> Vec<String> {
    let Ok(guard) = hunspell::dictionary().lock() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(dictionary) = guard.as_ref() {
        dictionary.suggest(word, &mut out);
    }
    out.truncate(5);
    out
}

/// "Add to Dictionary": remember `word` now and in `custom-dictionary.txt`.
#[cfg(not(target_os = "macos"))]
pub fn learn_word(word: &str) {
    use std::io::Write;

    if let Ok(mut guard) = hunspell::dictionary().lock()
        && let Some(dictionary) = guard.as_mut()
    {
        let _ = dictionary.add(word);
    }
    let path = hunspell::custom_words_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| writeln!(file, "{word}"));
    if let Err(err) = written {
        tracing::warn!(%err, "could not save the learned word");
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod linux_tests {
    #[test]
    fn splits_words_like_chromium() {
        let text = "don't fix wrold v2 café";
        let words: Vec<&str> = super::words(text).into_iter().map(|r| &text[r]).collect();
        assert_eq!(words, ["don't", "fix", "wrold", "café"]);
    }

    #[test]
    fn flags_a_misspelling_with_a_dictionary() {
        if !std::path::Path::new("/usr/share/hunspell/en_US.dic").exists() {
            return;
        }
        let text = "fix wrold thing";
        let ranges = super::misspelled_ranges(text);
        assert_eq!(
            ranges.iter().map(|r| &text[r.clone()]).collect::<Vec<_>>(),
            ["wrold"]
        );
        assert!(super::guesses("wrold").iter().any(|g| g == "world"));
    }
}

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
