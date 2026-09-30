//! Which grammar highlights a file or an injected block.

use std::sync::Arc;

use super::library::Grammar;

/// The grammar for `path`: an exact file name, else the longest matching
/// extension (`d.ts` beats `ts`), else a first-line pattern (shebangs).
/// Grammars earlier in `grammars` win ties.
pub fn for_path(grammars: &[Arc<Grammar>], path: &str, first_line: &str) -> Option<Arc<Grammar>> {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_lowercase();
    if let Some(g) = grammars
        .iter()
        .find(|g| g.filenames.contains(&name))
    {
        return Some(g.clone());
    }
    let mut best: Option<(&Arc<Grammar>, usize)> = None;
    for grammar in grammars {
        for ext in &grammar.extensions {
            let matches = name.len() > ext.len()
                && name.ends_with(ext.as_str())
                && name.as_bytes()[name.len() - ext.len() - 1] == b'.';
            if matches && best.is_none_or(|(_, len)| ext.len() > len) {
                best = Some((grammar, ext.len()));
            }
        }
    }
    if let Some((grammar, _)) = best {
        return Some(grammar.clone());
    }
    grammars
        .iter()
        .find(|g| {
            g.first_line
                .as_ref()
                .is_some_and(|re| re.is_match(first_line))
        })
        .cloned()
}

/// The grammar an injection names (`javascript`, a code fence's `rs`): a
/// grammar name, an alias, else an extension.
pub fn for_injection(grammars: &[Arc<Grammar>], name: &str) -> Option<Arc<Grammar>> {
    let name = name.trim().to_lowercase();
    let name = name.trim_start_matches('.');
    if name.is_empty() {
        return None;
    }
    grammars
        .iter()
        .find(|g| g.name == name)
        .or_else(|| {
            grammars
                .iter()
                .find(|g| g.aliases.iter().any(|a| a == name))
        })
        .or_else(|| {
            grammars
                .iter()
                .find(|g| g.extensions.iter().any(|e| e == name))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grammar(name: &str, extensions: &[&str], filenames: &[&str], first: &str) -> Arc<Grammar> {
        let table = unsafe { &*corvane_grammars::corvane_grammars_v1() };
        let entries = unsafe { std::slice::from_raw_parts(table.grammars, table.len) };
        let language = unsafe { tree_sitter_language::LanguageFn::from_raw(entries[0].language) };
        Arc::new(Grammar {
            name: name.into(),
            language: language.into(),
            highlights: String::new(),
            injections: String::new(),
            locals: String::new(),
            extensions: extensions.iter().map(|s| s.to_string()).collect(),
            filenames: filenames.iter().map(|s| s.to_string()).collect(),
            first_line: (!first.is_empty()).then(|| regex::Regex::new(first).expect("re")),
            aliases: vec![format!("{name}-alias")],
            injects: vec![],
        })
    }

    fn set() -> Vec<Arc<Grammar>> {
        vec![
            grammar("typescript", &["ts", "mts"], &[], ""),
            grammar("dts", &["d.ts"], &[], ""),
            grammar("make", &["mk"], &["makefile"], ""),
            grammar("bash", &["sh"], &[".bashrc"], r"^#!.*\b(ba)?sh\b"),
        ]
    }

    fn name(g: Option<Arc<Grammar>>) -> Option<String> {
        g.map(|g| g.name.clone())
    }

    #[test]
    fn paths() {
        let set = set();
        assert_eq!(
            name(for_path(&set, "src/a.ts", "")),
            Some("typescript".into())
        );
        assert_eq!(name(for_path(&set, "types/x.D.TS", "")), Some("dts".into()));
        assert_eq!(
            name(for_path(&set, "sub/Makefile", "")),
            Some("make".into())
        );
        assert_eq!(
            name(for_path(&set, "home/.bashrc", "")),
            Some("bash".into())
        );
        assert_eq!(
            name(for_path(&set, "bin/tool", "#!/bin/bash")),
            Some("bash".into())
        );
        assert_eq!(name(for_path(&set, "ts", "")), None);
        assert_eq!(name(for_path(&set, "a.xts", "")), None);
        assert_eq!(name(for_path(&set, "README", "")), None);
    }

    #[test]
    fn injections() {
        let set = set();
        assert_eq!(
            name(for_injection(&set, "TypeScript")),
            Some("typescript".into())
        );
        assert_eq!(name(for_injection(&set, "make-alias")), Some("make".into()));
        assert_eq!(name(for_injection(&set, "mts")), Some("typescript".into()));
        assert_eq!(name(for_injection(&set, "")), None);
    }
}
