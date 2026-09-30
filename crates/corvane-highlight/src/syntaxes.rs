//! Which grammar set is in use: the `syntax-core` set
//! compiled in (syntect's default packages plus 330 TextMate grammars for
//! languages nothing else covers, converted from GitHub Linguist's collection
//! by tools/tm-grammars: `assets/syntaxes.packdump`), the `syntax-extended`
//! set (two-face's full collection) either compiled in by the
//! `bundled-syntax-extended` feature (the "full" build) or loaded from the
//! on-demand pack's `syntaxes.packdump` when the app points at it with
//! [`use_extended_dump`]. The core set stays behind the extended one
//! ([`sets`]), so its additions survive loading the pack.

use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

use syntect::parsing::SyntaxSet;

/// The pack version the "full" build carries and the on-demand pack must
/// match in shape (`packaging/packs.sh` writes the same dump).
pub const EXTENDED_PACK_VERSION: &str = "1.0.0";

/// The core set: syntect's defaults plus the converted TextMate grammars.
fn core() -> &'static Arc<SyntaxSet> {
    static SET: OnceLock<Arc<SyntaxSet>> = OnceLock::new();
    SET.get_or_init(|| {
        // the tools that write the dump (`pack-builder`) run without it
        #[cfg(not(feature = "pack-builder"))]
        {
            let dump: &[u8] = include_bytes!("../assets/syntaxes.packdump");
            match syntect::dumps::from_reader::<SyntaxSet, _>(dump) {
                Ok(set) => return Arc::new(set),
                Err(err) => tracing::warn!("the compiled-in grammar dump: {err}"),
            }
        }
        Arc::new(SyntaxSet::load_defaults_newlines())
    })
}

fn bundled() -> &'static Arc<SyntaxSet> {
    #[cfg(feature = "bundled-syntax-extended")]
    {
        static SET: OnceLock<Arc<SyntaxSet>> = OnceLock::new();
        SET.get_or_init(|| Arc::new(two_face::syntax::extra_newlines()))
    }
    #[cfg(not(feature = "bundled-syntax-extended"))]
    {
        core()
    }
}

/// The sets diffs highlight with, in lookup order: [`current`], then the
/// core set when that is another one (the TextMate additions two-face lacks).
pub fn sets() -> Vec<Arc<SyntaxSet>> {
    let first = current();
    let core = core().clone();
    if Arc::ptr_eq(&first, &core) {
        vec![first]
    } else {
        vec![first, core]
    }
}

fn loaded() -> &'static RwLock<Option<Arc<SyntaxSet>>> {
    static LOADED: OnceLock<RwLock<Option<Arc<SyntaxSet>>>> = OnceLock::new();
    LOADED.get_or_init(|| RwLock::new(None))
}

/// The grammar set diffs highlight with: the loaded extended pack when
/// there is one, else the compiled-in set.
pub fn current() -> Arc<SyntaxSet> {
    if let Ok(guard) = loaded().read()
        && let Some(set) = guard.as_ref()
    {
        return set.clone();
    }
    bundled().clone()
}

/// Whether the extended grammar set is in use (compiled in or loaded).
pub fn extended_active() -> bool {
    cfg!(feature = "bundled-syntax-extended")
        || loaded().read().map(|g| g.is_some()).unwrap_or(false)
}

/// Whether this build compiled the extended set in (the "full" build).
pub fn extended_bundled() -> bool {
    cfg!(feature = "bundled-syntax-extended")
}

/// Load the `syntax-extended` pack's dump and use it from now on. Returns
/// the number of syntaxes it holds.
pub fn use_extended_dump(path: &Path) -> Result<usize, String> {
    let set: SyntaxSet = syntect::dumps::from_dump_file(path)
        .map_err(|err| format!("could not load {}: {err}", path.display()))?;
    let count = set.syntaxes().len();
    if let Ok(mut guard) = loaded().write() {
        *guard = Some(Arc::new(set));
    }
    Ok(count)
}

/// Back to the compiled-in set (the pack was removed).
pub fn clear_extended_dump() {
    if let Ok(mut guard) = loaded().write() {
        *guard = None;
    }
}

/// Write the extended set as a syntect dump (`packaging/packs.sh` calls
/// this through the `dump-extended` example).
#[cfg(feature = "pack-builder")]
pub fn write_extended_dump(path: &Path) -> Result<usize, String> {
    let set = two_face::syntax::extra_newlines();
    syntect::dumps::dump_to_file(&set, path).map_err(|err| err.to_string())?;
    Ok(set.syntaxes().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_set_knows_rust_and_markdown() {
        let set = current();
        assert!(set.find_syntax_by_extension("rs").is_some());
        assert!(set.find_syntax_by_extension("md").is_some());
    }

    #[test]
    fn a_missing_dump_is_an_error_and_leaves_the_bundled_set() {
        assert!(use_extended_dump(Path::new("/nonexistent/syntaxes.packdump")).is_err());
        assert_eq!(extended_active(), cfg!(feature = "bundled-syntax-extended"));
    }
}
