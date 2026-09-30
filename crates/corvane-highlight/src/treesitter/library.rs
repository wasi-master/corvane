//! Where tree-sitter grammars come from: the table compiled in by the
//! `bundled-tree-sitter` feature (the "full" build) or the dylib of an
//! installed `tree-sitter-all` / `tree-sitter-rest` pack. Both are read
//! through the same C-ABI table ([`super::ffi::Table`]).
//!
//! Loaded libraries are never closed: `Language`s and compiled queries point
//! into them and may still be in use on another thread. Removing a pack only
//! drops its grammars from the registry (the file can be deleted while
//! mapped).

use std::collections::BTreeSet;
use std::ffi::{CStr, c_char};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use regex::Regex;
use tree_sitter::Language;
use tree_sitter_language::LanguageFn;

use super::ffi;

/// A grammar ready to highlight with.
pub struct Grammar {
    pub name: String,
    pub language: Language,
    pub highlights: String,
    pub injections: String,
    pub locals: String,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
    pub first_line: Option<Regex>,
    pub aliases: Vec<String>,
    pub injects: Vec<String>,
}

impl Grammar {
    /// Every capture name the grammar's queries use (`keyword.control`).
    pub fn capture_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for query in [&self.highlights, &self.injections, &self.locals] {
            names.extend(capture_names_in(query));
        }
        names
    }
}

/// Capture names in query text, skipping comments and strings.
pub fn capture_names_in(query: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in query.lines() {
        let code = strip_comment(line);
        let mut rest = code;
        while let Some(at) = rest.find('@') {
            let offset = code.len() - rest.len() + at;
            let tail = &rest[at + 1..];
            let len = tail
                .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '.' | '-')))
                .unwrap_or(tail.len());
            if len > 0 && !in_string(code, offset) {
                names.insert(tail[..len].to_string());
            }
            rest = &tail[len..];
        }
    }
    names
}

/// The line up to a `;` comment outside a string.
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ';' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Whether byte `at` of `code` sits inside a `"…"` string.
fn in_string(code: &str, at: usize) -> bool {
    let mut quoted = false;
    let mut escaped = false;
    for c in code[..at].chars() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            _ => {}
        }
    }
    quoted
}

struct Source {
    name: String,
    grammars: Vec<Arc<Grammar>>,
}

/// Source name of the compiled-in table.
pub const BUNDLED: &str = "bundled";

/// Sources in lookup order: the compiled-in table, then the packs (the
/// `tree-sitter-all` pack before `tree-sitter-rest`, which it contains).
fn rank(name: &str) -> u8 {
    match name {
        BUNDLED => 0,
        "tree-sitter-all" => 1,
        "tree-sitter-rest" => 2,
        _ => 3,
    }
}

fn sources() -> &'static RwLock<Vec<Source>> {
    static SOURCES: OnceLock<RwLock<Vec<Source>>> = OnceLock::new();
    SOURCES.get_or_init(|| {
        let lock = RwLock::new(Vec::new());
        #[cfg(feature = "bundled-tree-sitter")]
        {
            let table = corvane_grammars::corvane_grammars_v1().cast::<ffi::Table>();
            // SAFETY: the compiled-in table is a valid `corvane_grammars`
            // table (layouts checked by `ffi`'s tests) living forever.
            match unsafe { read_table(table) } {
                Ok(grammars) => {
                    if let Ok(mut guard) = lock.write() {
                        guard.push(Source {
                            name: BUNDLED.to_string(),
                            grammars,
                        });
                    }
                }
                Err(err) => tracing::warn!("compiled-in tree-sitter grammars: {err}"),
            }
        }
        lock
    })
}

static GENERATION: AtomicU64 = AtomicU64::new(1);

/// Changes whenever the set of grammars does (a pack loaded or removed), so
/// callers can re-highlight and caches can drop stale entries.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Whether this build compiles every grammar in (the "full" build).
pub fn bundled() -> bool {
    cfg!(feature = "bundled-tree-sitter")
}

/// Whether any grammar is available.
pub fn available() -> bool {
    sources()
        .read()
        .map(|s| s.iter().any(|source| !source.grammars.is_empty()))
        .unwrap_or(false)
}

/// Whether the named source (a pack name) is loaded.
pub fn is_loaded(name: &str) -> bool {
    sources()
        .read()
        .map(|s| s.iter().any(|source| source.name == name))
        .unwrap_or(false)
}

/// Every available grammar, in lookup order; a name a better-ranked source
/// already has is skipped.
pub fn grammars() -> Vec<Arc<Grammar>> {
    let Ok(sources) = sources().read() else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for source in sources.iter() {
        for grammar in &source.grammars {
            if seen.insert(grammar.name.clone()) {
                out.push(grammar.clone());
            }
        }
    }
    out
}

/// Open a pack's grammar library and use its grammars from now on, under the
/// pack's `name`. Returns how many grammars it holds.
pub fn load_library(name: &str, path: &Path) -> Result<usize, String> {
    // SAFETY: loading a library runs its initialisers. The pack comes from a
    // manifest verified with the release key and a matching sha256
    // (corvane-packs), the same trust as an app update.
    let library = unsafe { libloading::Library::new(path) }
        .map_err(|err| format!("could not open {}: {err}", path.display()))?;
    // SAFETY: `corvane_grammars_v1` has this signature in every ABI version;
    // the table's `abi` field is checked before anything else is read.
    let table = unsafe {
        let entry = library
            .get::<unsafe extern "C" fn() -> *const ffi::Table>(ffi::ENTRY_SYMBOL)
            .map_err(|err| format!("{} is not a grammar pack: {err}", path.display()))?;
        entry()
    };
    // SAFETY: the library stays loaded for the rest of the process (below).
    let grammars = unsafe { read_table(table) }?;
    let count = grammars.len();
    std::mem::forget(library);
    insert(name, grammars);
    Ok(count)
}

/// Register grammars under `name`, replacing what it had.
fn insert(name: &str, grammars: Vec<Arc<Grammar>>) {
    if let Ok(mut sources) = sources().write() {
        sources.retain(|s| s.name != name);
        sources.push(Source {
            name: name.to_string(),
            grammars,
        });
        sources.sort_by_key(|s| rank(&s.name));
    }
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

/// Stop using a pack's grammars (it was removed). The library stays mapped.
pub fn unload_library(name: &str) {
    if let Ok(mut sources) = sources().write() {
        let before = sources.len();
        sources.retain(|s| s.name != name);
        if sources.len() == before {
            return;
        }
    }
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

/// Register a grammar table directly (tests, examples).
///
/// # Safety
///
/// `table` must point at a valid `corvane_grammars` table whose data lives
/// for the rest of the process.
pub unsafe fn register_table(name: &str, table: *const ffi::Table) -> Result<usize, String> {
    // SAFETY: forwarded from the caller.
    let grammars = unsafe { read_table(table) }?;
    let count = grammars.len();
    insert(name, grammars);
    Ok(count)
}

/// Copy a table into owned grammars, skipping ones this runtime cannot use.
///
/// # Safety
///
/// `table` must be null or point at a `corvane_grammars` table: `abi` first,
/// the rest laid out as [`ffi::Table`] when `abi == ffi::ABI`.
unsafe fn read_table(table: *const ffi::Table) -> Result<Vec<Arc<Grammar>>, String> {
    if table.is_null() {
        return Err("the grammar table is missing".to_string());
    }
    // SAFETY: `abi` is the first field in every version of the table.
    let abi = unsafe { (*table).abi };
    if abi != ffi::ABI {
        return Err(format!(
            "the grammar pack has table version {abi}, this build reads {}",
            ffi::ABI
        ));
    }
    // SAFETY: same ABI, so the layout is `ffi::Table`.
    let table = unsafe { &*table };
    if table.grammars.is_null() && table.len > 0 {
        return Err("the grammar table has no entries pointer".to_string());
    }
    let entries: &[ffi::Grammar] = if table.len == 0 {
        &[]
    } else {
        // SAFETY: `grammars` points at `len` entries owned by the table.
        unsafe { std::slice::from_raw_parts(table.grammars, table.len) }
    };
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        // SAFETY: every string field is a NUL-terminated string or null.
        match unsafe { read_grammar(entry) } {
            Ok(grammar) => out.push(Arc::new(grammar)),
            Err(err) => tracing::warn!("skipping a tree-sitter grammar: {err}"),
        }
    }
    Ok(out)
}

/// # Safety
///
/// String fields must be null or NUL-terminated; `language` a grammar's
/// `tree_sitter_<name>` function.
unsafe fn read_grammar(entry: &ffi::Grammar) -> Result<Grammar, String> {
    // SAFETY: forwarded from the caller.
    let text = |ptr: *const c_char| unsafe { text(ptr) };
    let name = text(entry.name)?;
    if name.is_empty() {
        return Err("a grammar without a name".to_string());
    }
    // SAFETY: `language` is the grammar's language function.
    let language = Language::new(unsafe { LanguageFn::from_raw(entry.language) });
    let abi = language.abi_version();
    if !(tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION..=tree_sitter::LANGUAGE_VERSION)
        .contains(&abi)
    {
        return Err(format!(
            "{name} has ABI {abi}, this build reads {}..={}",
            tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION,
            tree_sitter::LANGUAGE_VERSION
        ));
    }
    let list = |ptr| -> Result<Vec<String>, String> {
        Ok(text(ptr)?
            .lines()
            .map(|l| l.trim().to_lowercase())
            .filter(|l| !l.is_empty())
            .collect())
    };
    let first_line = text(entry.first_line)?;
    let first_line = if first_line.is_empty() {
        None
    } else {
        Some(Regex::new(&first_line).map_err(|err| format!("{name}: first_line: {err}"))?)
    };
    Ok(Grammar {
        highlights: text(entry.highlights)?,
        injections: text(entry.injections)?,
        locals: text(entry.locals)?,
        extensions: list(entry.extensions)?,
        filenames: list(entry.filenames)?,
        first_line,
        aliases: list(entry.aliases)?,
        injects: list(entry.injects)?,
        language,
        name,
    })
}

/// # Safety
///
/// `ptr` must be null or NUL-terminated.
unsafe fn text(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Ok(String::new());
    }
    // SAFETY: forwarded from the caller.
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map(str::to_string)
        .map_err(|err| format!("a grammar string is not UTF-8: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_names_skip_comments_and_strings() {
        let query = "; @nope\n(identifier) @variable ; @comment-too\n\"@interface\" @keyword.control\n\
                     ((x) @injection.content (#set! injection.language \"a@b\"))";
        let names: Vec<String> = capture_names_in(query).into_iter().collect();
        assert_eq!(
            names,
            ["injection.content", "keyword.control", "variable"].map(String::from)
        );
    }

    #[test]
    fn the_compiled_table_reads_back() {
        let table = corvane_grammars::corvane_grammars_v1().cast::<ffi::Table>();
        let grammars = unsafe { read_table(table) }.expect("table");
        let names: Vec<&str> = grammars.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"rust"), "{names:?}");
        assert!(names.contains(&"markdown_inline"), "{names:?}");
        let rust = grammars.iter().find(|g| g.name == "rust").expect("rust");
        assert_eq!(rust.extensions, ["rs"]);
        assert!(rust.highlights.contains("@keyword"));
    }

    #[test]
    fn a_table_of_another_version_is_refused() {
        let table = ffi::Table {
            abi: ffi::ABI + 1,
            version: std::ptr::null(),
            len: 0,
            grammars: std::ptr::null(),
        };
        let err = unsafe { read_table(&table) }.err().expect("refused");
        assert!(err.contains("table version"), "{err}");
    }

    #[test]
    fn a_missing_library_is_an_error() {
        let err = load_library("tree-sitter-all", Path::new("/nonexistent/libx.dylib"))
            .expect_err("missing");
        assert!(err.contains("could not open"), "{err}");
        assert!(!is_loaded("tree-sitter-all"));
    }

    /// `CORVANE_TS_LIBRARY=<pack dylib> cargo test -p corvane-highlight -- --ignored`
    #[test]
    #[ignore]
    fn a_built_pack_library_loads() {
        let path = std::env::var("CORVANE_TS_LIBRARY").expect("CORVANE_TS_LIBRARY");
        let count = load_library("tree-sitter-all", Path::new(&path)).expect("loads");
        assert!(count > 0);
        assert!(is_loaded("tree-sitter-all"));
        let spans = crate::treesitter::highlight("main.rs", &["fn main() {}"], 1024)
            .expect("rust highlights");
        assert!(!spans[0].is_empty());
        unload_library("tree-sitter-all");
    }
}
