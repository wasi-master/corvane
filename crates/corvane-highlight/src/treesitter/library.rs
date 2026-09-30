//! Where tree-sitter grammars come from: the table compiled in by the
//! `bundled-tree-sitter` feature (the "full" build) or an installed
//! `tree-sitter-all` / `tree-sitter-rest` pack. A pack is an `index.json`
//! plus one gzipped library per grammar package (a *unit*): detection reads
//! the index ([`Entry`]); a unit is unpacked into the cache and opened the
//! first time a file needs one of its grammars, so disk and memory hold only
//! the languages in use. Every library is read through the same C-ABI table
//! ([`super::ffi::Table`]).
//!
//! Loaded libraries are never closed: `Language`s and compiled queries point
//! into them and may still be in use on another thread. Removing a pack only
//! drops its grammars from the registry (the file can be deleted while
//! mapped).

use std::collections::{BTreeSet, HashMap};
use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};
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

/// What detection needs to know about a grammar, available before its
/// library is loaded (a pack's `index.json`).
pub struct Entry {
    pub name: String,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
    pub first_line: Option<Regex>,
    pub aliases: Vec<String>,
    pub injects: Vec<String>,
    unit: Arc<Unit>,
}

impl Entry {
    /// The loaded grammar, loading its unit (unpacking and opening the
    /// library) on first use. `None` when the unit cannot be loaded (logged
    /// once) or lacks the grammar.
    pub fn grammar(&self) -> Option<Arc<Grammar>> {
        self.unit.load().get(&self.name).cloned()
    }

    /// An entry with no grammar behind it (detection tests).
    #[cfg(test)]
    pub(crate) fn detached(
        name: &str,
        extensions: Vec<String>,
        filenames: Vec<String>,
        first_line: Option<Regex>,
        aliases: Vec<String>,
    ) -> Arc<Entry> {
        Arc::new(Entry {
            name: name.to_string(),
            extensions,
            filenames,
            first_line,
            aliases,
            injects: Vec::new(),
            unit: Unit::loaded(&[]),
        })
    }

    fn of(grammar: &Arc<Grammar>, unit: Arc<Unit>) -> Arc<Entry> {
        Arc::new(Entry {
            name: grammar.name.clone(),
            extensions: grammar.extensions.clone(),
            filenames: grammar.filenames.clone(),
            first_line: grammar.first_line.clone(),
            aliases: grammar.aliases.clone(),
            injects: grammar.injects.clone(),
            unit,
        })
    }
}

/// Grammars that load together: one library of a pack (a grammar package
/// such as `typescript` with `typescript` and `tsx`), or a whole table.
struct Unit {
    /// `None`: `loaded` was filled at registration
    file: Option<UnitFile>,
    loaded: OnceLock<HashMap<String, Arc<Grammar>>>,
}

/// A pack's compressed library and where it is unpacked to.
struct UnitFile {
    gz: PathBuf,
    cache: PathBuf,
}

impl Unit {
    fn loaded(grammars: &[Arc<Grammar>]) -> Arc<Unit> {
        let map = grammars
            .iter()
            .map(|g| (g.name.clone(), g.clone()))
            .collect();
        let unit = Unit {
            file: None,
            loaded: OnceLock::new(),
        };
        let _ = unit.loaded.set(map);
        Arc::new(unit)
    }

    fn load(&self) -> &HashMap<String, Arc<Grammar>> {
        self.loaded.get_or_init(|| {
            let Some(file) = &self.file else {
                return HashMap::new();
            };
            match unpack(file).and_then(|path| open_library(&path)) {
                Ok(grammars) => grammars.into_iter().map(|g| (g.name.clone(), g)).collect(),
                Err(err) => {
                    tracing::warn!("tree-sitter grammars in {}: {err}", file.gz.display());
                    HashMap::new()
                }
            }
        })
    }
}

/// Unpack a unit's `.dylib.gz` next to the other unpacked units (once; the
/// cache is keyed by pack version). Written to a temporary name and renamed:
/// a library must never be rewritten in place.
fn unpack(file: &UnitFile) -> Result<PathBuf, String> {
    if file.cache.metadata().is_ok_and(|m| m.len() > 0) {
        return Ok(file.cache.clone());
    }
    let dir = file
        .cache
        .parent()
        .ok_or_else(|| "no cache directory".to_string())?;
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let input =
        std::fs::File::open(&file.gz).map_err(|err| format!("{}: {err}", file.gz.display()))?;
    let tmp = file
        .cache
        .with_extension(format!("partial-{}", std::process::id()));
    let result = (|| {
        let mut out = std::fs::File::create(&tmp)?;
        std::io::copy(&mut flate2::read::GzDecoder::new(input), &mut out)?;
        out.sync_all()?;
        std::fs::rename(&tmp, &file.cache)
    })();
    if let Err(err) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("unpacking {}: {err}", file.gz.display()));
    }
    Ok(file.cache.clone())
}

struct Source {
    name: String,
    entries: Vec<Arc<Entry>>,
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
                            entries: entries_of(&grammars),
                        });
                    }
                }
                Err(err) => tracing::warn!("compiled-in tree-sitter grammars: {err}"),
            }
        }
        lock
    })
}

fn entries_of(grammars: &[Arc<Grammar>]) -> Vec<Arc<Entry>> {
    let unit = Unit::loaded(grammars);
    grammars
        .iter()
        .map(|g| Entry::of(g, unit.clone()))
        .collect()
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
        .map(|s| s.iter().any(|source| !source.entries.is_empty()))
        .unwrap_or(false)
}

/// Whether the named source (a pack name) is loaded.
pub fn is_loaded(name: &str) -> bool {
    sources()
        .read()
        .map(|s| s.iter().any(|source| source.name == name))
        .unwrap_or(false)
}

/// Every known grammar, in lookup order; a name a better-ranked source
/// already has is skipped. Nothing is loaded.
pub fn entries() -> Vec<Arc<Entry>> {
    let Ok(sources) = sources().read() else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for source in sources.iter() {
        for entry in &source.entries {
            if seen.insert(entry.name.clone()) {
                out.push(entry.clone());
            }
        }
    }
    out
}

/// Every grammar, loaded (tests and tools; this unpacks and opens every
/// library of a pack).
pub fn grammars() -> Vec<Arc<Grammar>> {
    entries().iter().filter_map(|e| e.grammar()).collect()
}

/// Open one grammar library (a unit, or a single-library pack) and read its
/// table. The library stays loaded for the rest of the process: languages
/// and compiled queries point into it and may be in use on other threads.
fn open_library(path: &Path) -> Result<Vec<Arc<Grammar>>, String> {
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
            .map_err(|err| format!("{} is not a grammar library: {err}", path.display()))?;
        entry()
    };
    // SAFETY: the library is never closed (below).
    let grammars = unsafe { read_table(table) }?;
    std::mem::forget(library);
    Ok(grammars)
}

/// Use one grammar library (every grammar in it, loaded now) under `name`.
/// Returns how many grammars it holds.
pub fn load_library(name: &str, path: &Path) -> Result<usize, String> {
    let grammars = open_library(path)?;
    let count = grammars.len();
    insert(name, entries_of(&grammars));
    Ok(count)
}

/// A pack's `index.json`: the grammars of each unit, so detection works
/// before any library is unpacked.
#[derive(serde::Deserialize)]
struct Index {
    abi: u32,
    units: Vec<IndexUnit>,
}

#[derive(serde::Deserialize)]
struct IndexUnit {
    /// the unit's `.dylib.gz`, relative to the index
    file: String,
    grammars: Vec<IndexGrammar>,
}

#[derive(serde::Deserialize)]
struct IndexGrammar {
    name: String,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    first_line: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    injects: Vec<String>,
}

/// Use an installed grammar pack under `name`: read its `index.json` now,
/// unpack each unit into `cache_dir` and open it when a file first needs one
/// of its grammars. Returns how many grammars the pack lists.
pub fn load_pack(name: &str, index: &Path, cache_dir: &Path) -> Result<usize, String> {
    let text =
        std::fs::read_to_string(index).map_err(|err| format!("{}: {err}", index.display()))?;
    let parsed: Index =
        serde_json::from_str(&text).map_err(|err| format!("{}: {err}", index.display()))?;
    if parsed.abi != ffi::ABI {
        return Err(format!(
            "the grammar pack has table version {}, this build reads {}",
            parsed.abi,
            ffi::ABI
        ));
    }
    let base = index.parent().unwrap_or(Path::new("."));
    let mut entries = Vec::new();
    for unit in parsed.units {
        let stem = Path::new(&unit.file)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.trim_end_matches(".gz").to_string())
            .ok_or_else(|| format!("{}: a unit without a file name", index.display()))?;
        let shared = Arc::new(Unit {
            file: Some(UnitFile {
                gz: base.join(&unit.file),
                cache: cache_dir.join(stem),
            }),
            loaded: OnceLock::new(),
        });
        for g in unit.grammars {
            let first_line = if g.first_line.is_empty() {
                None
            } else {
                Some(
                    Regex::new(&g.first_line)
                        .map_err(|err| format!("{}: first_line: {err}", g.name))?,
                )
            };
            let lower = |v: Vec<String>| v.into_iter().map(|s| s.to_lowercase()).collect();
            entries.push(Arc::new(Entry {
                name: g.name,
                extensions: lower(g.extensions),
                filenames: lower(g.filenames),
                first_line,
                aliases: lower(g.aliases),
                injects: lower(g.injects),
                unit: shared.clone(),
            }));
        }
    }
    let count = entries.len();
    insert(name, entries);
    Ok(count)
}

/// Register grammars under `name`, replacing what it had.
fn insert(name: &str, entries: Vec<Arc<Entry>>) {
    if let Ok(mut sources) = sources().write() {
        sources.retain(|s| s.name != name);
        sources.push(Source {
            name: name.to_string(),
            entries,
        });
        sources.sort_by_key(|s| rank(&s.name));
    }
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

/// Stop using a pack's grammars (it was removed). Opened libraries stay
/// mapped.
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
    insert(name, entries_of(&grammars));
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

    #[test]
    fn a_pack_index_is_read_without_opening_libraries() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("corvane-ts-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("grammars")).unwrap();
        // not a library: detection works, loading fails quietly
        let mut gz = flate2::write::GzEncoder::new(
            std::fs::File::create(dir.join("grammars/fake.dylib.gz")).unwrap(),
            flate2::Compression::default(),
        );
        gz.write_all(b"not a dylib").unwrap();
        gz.finish().unwrap();
        std::fs::write(
            dir.join("index.json"),
            r#"{"abi":1,"version":"t","units":[{"file":"grammars/fake.dylib.gz","grammars":[
                {"name":"fakelang","extensions":["FAKE"],"aliases":["fk"],"first_line":"^#!.*fake"}]}]}"#,
        )
        .unwrap();
        let cache = dir.join("cache");
        let count = load_pack("test-pack", &dir.join("index.json"), &cache).expect("index");
        assert_eq!(count, 1);
        let entries = entries();
        let entry =
            crate::treesitter::detect::for_path(&entries, "a.fake", "").expect("by extension");
        assert_eq!(entry.name, "fakelang");
        assert!(crate::treesitter::detect::for_injection(&entries, "fk").is_some());
        assert!(entry.grammar().is_none());
        // unpacked (then refused by dlopen)
        assert_eq!(
            std::fs::read(cache.join("fake.dylib")).unwrap(),
            b"not a dylib"
        );
        unload_library("test-pack");
        assert!(!is_loaded("test-pack"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `CORVANE_TS_LIBRARY=<pack dylib> cargo test -p corvane-highlight -- --ignored`
    #[test]
    #[ignore]
    fn a_built_pack_library_loads() {
        let path = std::env::var("CORVANE_TS_LIBRARY").expect("CORVANE_TS_LIBRARY");
        let grammars = open_library(Path::new(&path)).expect("loads");
        assert!(!grammars.is_empty());
        for grammar in &grammars {
            crate::treesitter::check_queries(grammar)
                .unwrap_or_else(|err| panic!("{}: {err}", grammar.name));
        }
        eprintln!("{:?}", grammars.iter().map(|g| &g.name).collect::<Vec<_>>());
        let count = load_library("tree-sitter-all", Path::new(&path)).expect("registers");
        assert_eq!(count, grammars.len());
        if grammars.iter().any(|g| g.name == "rust") {
            // a whole budget's worth of Rust, as a large diff would highlight
            let one = include_str!("mod.rs");
            let source = one.repeat(crate::MAX_HIGHLIGHT_BYTES / one.len());
            let lines: Vec<&str> = source.lines().collect();
            let started = std::time::Instant::now();
            let spans = crate::treesitter::highlight("big.rs", &lines, crate::MAX_HIGHLIGHT_BYTES)
                .expect("rust highlights");
            eprintln!("{} bytes: {:?}", source.len(), started.elapsed());
            assert!(spans.iter().filter(|s| !s.is_empty()).count() > lines.len() / 2);
        }
        unload_library("tree-sitter-all");
    }
}
