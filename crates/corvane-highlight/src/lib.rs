//! Line-stateful syntax highlighting for diffs, classified into
//! GitHub Desktop's CodeMirror token classes (`styles/ui/_diff.scss`
//! `.cm-s-default`) so the UI can colour them with the `--syntax-*-color`
//! tokens. Three tokenizers, chained per [`Engine`]:
//!
//! - [`cm`]: ports of the CodeMirror modes GHD's highlighter runs;
//! - syntect with the compiled-in grammar set or the `syntax-extended` pack
//!   ([`syntaxes`]), for languages no port covers;
//! - [`treesitter`] (Corvane addition, opt-in): grammars from the full build
//!   or a `tree-sitter-all` / `tree-sitter-rest` pack.
//!
//! Grammars load lazily on first use.

pub mod cm;
pub mod syntaxes;
pub mod treesitter;

use std::ops::Range;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};

use syntect::easy::ScopeRangeIterator;
use syntect::highlighting::ScopeSelectors;
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

/// GHD CodeMirror classes that get a colour (`number`, `def`, `builtin`,
/// `bracket`, `operator` inherit the line colour and are not emitted).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenClass {
    Variable,
    AltVariable,
    Keyword,
    Atom,
    String,
    Qualifier,
    Type,
    Comment,
    Tag,
    Attribute,
    Link,
    Header,
    Quote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub class: TokenClass,
}

/// Highlighting stops after this many bytes (GHD `MaxHighlightContentLength`).
pub const MAX_HIGHLIGHT_BYTES: usize = 256 * 1024;

/// Which tokenizers run, in order; the first that knows the file wins
/// (Settings › Appearance › Syntax highlighting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Engine {
    /// GHD's: the CodeMirror ports, then syntect
    #[default]
    GitHubDesktop,
    /// the CodeMirror ports, then tree-sitter, then syntect: tree-sitter only
    /// for languages GHD does not highlight itself
    TreeSitterFallback,
    /// tree-sitter, then the CodeMirror ports, then syntect
    TreeSitter,
}

fn syntax_set() -> Arc<SyntaxSet> {
    syntaxes::current()
}

struct Classifier {
    rules: Vec<(ScopeSelectors, TokenClass)>,
}

fn classifier() -> &'static Classifier {
    static C: OnceLock<Classifier> = OnceLock::new();
    C.get_or_init(|| {
        let rule = |sel: &str, class| (ScopeSelectors::from_str(sel).expect("selector"), class);
        Classifier {
            rules: vec![
                rule("comment", TokenClass::Comment),
                rule("string, constant.character.escape", TokenClass::String),
                rule(
                    "keyword - keyword.operator, storage.type, storage.modifier",
                    TokenClass::Keyword,
                ),
                rule(
                    "constant.language, constant.character, constant.other",
                    TokenClass::Atom,
                ),
                rule(
                    "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
                     entity.name.trait, entity.name.interface, support.type, support.class, \
                     entity.other.inherited-class",
                    TokenClass::Type,
                ),
                rule("variable.other, variable.language", TokenClass::Variable),
                rule(
                    "variable.parameter, variable.other.member",
                    TokenClass::AltVariable,
                ),
                rule(
                    "entity.name.namespace, entity.other.attribute-name.class",
                    TokenClass::Qualifier,
                ),
                rule(
                    "entity.name.tag, punctuation.definition.tag",
                    TokenClass::Tag,
                ),
                rule("entity.other.attribute-name", TokenClass::Attribute),
                rule("markup.underline.link", TokenClass::Link),
                rule("markup.heading", TokenClass::Header),
                rule("markup.quote", TokenClass::Quote),
                // CodeMirror's markdown mode: list items are `variable-2`,
                // their bullet `comment variable-2` (the comment colour wins)
                rule("punctuation.definition.list_item", TokenClass::Comment),
                rule("markup.list", TokenClass::AltVariable),
            ],
        }
    })
}

/// Grammar for a path by extension, then by first line (shebang etc.).
fn syntax_for<'a>(ss: &'a SyntaxSet, path: &str, first_line: &str) -> Option<&'a SyntaxReference> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    ss.find_syntax_by_extension(name)
        .or_else(|| ss.find_syntax_by_extension(ext))
        .or_else(|| ss.find_syntax_by_first_line(first_line))
}

/// Tokenize `lines` (without trailing newlines) in order, carrying parser
/// state from line to line, with GitHub Desktop's tokenizers. `None` when no
/// grammar matches the path. Lines after `MAX_HIGHLIGHT_BYTES` get no spans.
pub fn highlight_lines<'a>(
    path: &str,
    lines: impl IntoIterator<Item = &'a str>,
) -> Option<Vec<Vec<Span>>> {
    highlight_lines_with(Engine::GitHubDesktop, path, lines)
}

/// [`highlight_lines`] with the tokenizers `engine` chains.
pub fn highlight_lines_with<'a>(
    engine: Engine,
    path: &str,
    lines: impl IntoIterator<Item = &'a str>,
) -> Option<Vec<Vec<Span>>> {
    let lines: Vec<&str> = lines.into_iter().collect();
    let ts = || treesitter::highlight(path, &lines, MAX_HIGHLIGHT_BYTES);
    match engine {
        Engine::GitHubDesktop => {
            cm_highlight(path, &lines).or_else(|| syntect_highlight(path, &lines))
        }
        Engine::TreeSitterFallback => cm_highlight(path, &lines)
            .or_else(ts)
            .or_else(|| syntect_highlight(path, &lines)),
        Engine::TreeSitter => ts()
            .or_else(|| cm_highlight(path, &lines))
            .or_else(|| syntect_highlight(path, &lines)),
    }
}

/// The ported CodeMirror mode GHD would pick for `path`: by extension or
/// file name, else (for a file with no known extension) by the first line.
fn cm_mode(path: &str, first_line: &str) -> Option<std::sync::Arc<dyn cm::Mode>> {
    cm::mode_for_path(path).or_else(|| {
        let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
        let has_known_ext = name
            .rfind('.')
            .is_some_and(|i| cm::modes::mime_for_extension(&name[i..]).is_some());
        (!has_known_ext && cm::modes::mime_for_basename(&name).is_none())
            .then(|| cm::modes::guess_mime(first_line).and_then(cm::modes::mode_for_mime))
            .flatten()
    })
}

/// Whether GitHub Desktop's own tokenizer (a CodeMirror mode) covers `path`.
pub fn cm_covers(path: &str, first_line: &str) -> bool {
    cm_mode(path, first_line).is_some()
}

fn cm_highlight(path: &str, lines: &[&str]) -> Option<Vec<Vec<Span>>> {
    let mode = cm_mode(path, lines.first().copied().unwrap_or(""))?;
    Some(cm::highlight(&*mode, lines, MAX_HIGHLIGHT_BYTES))
}

fn syntect_highlight(path: &str, lines: &[&str]) -> Option<Vec<Vec<Span>>> {
    let first = lines.first().copied().unwrap_or("");
    let ss = syntax_set();
    let syntax = syntax_for(&ss, path, first)?;
    let ss = &*ss;
    let classes = classifier();
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut budget = MAX_HIGHLIGHT_BYTES;
    let mut out = Vec::new();
    for line in lines {
        if budget < line.len() {
            out.push(Vec::new());
            continue;
        }
        budget -= line.len();
        let with_newline = format!("{line}\n");
        let Ok(ops) = state.parse_line(&with_newline, ss) else {
            out.push(Vec::new());
            continue;
        };
        let mut spans: Vec<Span> = Vec::new();
        for (range, op) in ScopeRangeIterator::new(&ops, &with_newline) {
            let _ = stack.apply(op);
            if range.is_empty() {
                continue;
            }
            let end = range.end.min(line.len());
            if range.start >= end {
                continue;
            }
            if let Some(class) = classes.best(&stack) {
                match spans.last_mut() {
                    Some(last) if last.class == class && last.range.end == range.start => {
                        last.range.end = end;
                    }
                    _ => spans.push(Span {
                        range: range.start..end,
                        class,
                    }),
                }
            }
        }
        out.push(spans);
    }
    Some(out)
}

impl Classifier {
    /// The most specific matching rule for the scope stack, like a theme lookup.
    fn best(&self, stack: &ScopeStack) -> Option<TokenClass> {
        let scopes = stack.as_slice();
        let mut best: Option<(syntect::parsing::MatchPower, TokenClass)> = None;
        for (selectors, class) in &self.rules {
            if let Some(power) = selectors.does_match(scopes)
                && best.as_ref().is_none_or(|(p, _)| power > *p)
            {
                best = Some((power, *class));
            }
        }
        best.map(|(_, c)| c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_keywords_and_strings() {
        let spans = highlight_lines("main.rs", ["fn main() {", "    let s = \"hi\"; // c", "}"])
            .expect("rust grammar");
        assert_eq!(spans.len(), 3);
        let first: Vec<_> = spans[0]
            .iter()
            .map(|s| (s.range.clone(), s.class))
            .collect();
        assert!(
            first
                .iter()
                .any(|(r, c)| *c == TokenClass::Keyword && r.start == 0)
        );
        assert!(spans[1].iter().any(|s| s.class == TokenClass::String));
        assert!(spans[1].iter().any(|s| s.class == TokenClass::Comment));
    }

    #[test]
    fn unknown_extension_is_none() {
        assert!(highlight_lines("file.unknownext", ["x"]).is_none());
    }

    fn first_class(engine: Engine, path: &str, line: &str) -> Option<TokenClass> {
        treesitter::tests::with_grammars();
        highlight_lines_with(engine, path, [line])
            .and_then(|spans| spans[0].first().map(|s| s.class))
    }

    #[test]
    fn engines_chain_their_tokenizers() {
        // GHD has no Haskell mode: syntect, unless tree-sitter may run
        let haskell = "module Main where";
        assert_eq!(
            first_class(Engine::GitHubDesktop, "Main.hs", haskell),
            syntect_first("Main.hs", haskell)
        );
        assert_eq!(
            first_class(Engine::TreeSitterFallback, "Main.hs", haskell),
            Some(TokenClass::Keyword)
        );
        assert_eq!(
            first_class(Engine::TreeSitter, "Main.hs", haskell),
            Some(TokenClass::Keyword)
        );
        // GHD covers .js: the fallback engine keeps CodeMirror's tokens
        let js = "foo(1)";
        assert_eq!(
            first_class(Engine::TreeSitterFallback, "a.js", js),
            cm_first("a.js", js)
        );
        assert!(cm_covers("a.js", ""));
        assert!(!cm_covers("Main.hs", ""));
        // nobody knows it
        assert!(highlight_lines_with(Engine::TreeSitter, "x.unknownext", ["x"]).is_none());
    }

    fn syntect_first(path: &str, line: &str) -> Option<TokenClass> {
        syntect_highlight(path, &[line]).and_then(|s| s[0].first().map(|s| s.class))
    }

    fn cm_first(path: &str, line: &str) -> Option<TokenClass> {
        cm_highlight(path, &[line]).and_then(|s| s[0].first().map(|s| s.class))
    }
}
