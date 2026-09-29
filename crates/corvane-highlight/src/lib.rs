//! Line-stateful syntax highlighting for diffs: syntect parsing
//! with the compiled-in grammar set or the `syntax-extended` pack
//! (`syntaxes`), classified into GitHub Desktop's CodeMirror token classes
//! (`styles/ui/_diff.scss` `.cm-s-default`) so the UI can colour them with
//! the `--syntax-*-color` tokens. Grammars load lazily on first use.

pub mod cm;
pub mod syntaxes;

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
/// state from line to line. `None` when no grammar matches the path.
/// Lines after `MAX_HIGHLIGHT_BYTES` get no spans.
pub fn highlight_lines<'a>(
    path: &str,
    lines: impl IntoIterator<Item = &'a str>,
) -> Option<Vec<Vec<Span>>> {
    let mut lines = lines.into_iter().peekable();
    let first = lines.peek().copied().unwrap_or("");
    // languages whose CodeMirror mode is ported run GHD's own tokenizer
    let cm_mode = cm::mode_for_path(path).or_else(|| {
        let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
        let has_known_ext = name
            .rfind('.')
            .is_some_and(|i| cm::modes::mime_for_extension(&name[i..]).is_some());
        (!has_known_ext && cm::modes::mime_for_basename(&name).is_none())
            .then(|| cm::modes::guess_mime(first).and_then(cm::modes::mode_for_mime))
            .flatten()
    });
    if let Some(mode) = cm_mode {
        let lines: Vec<&str> = lines.collect();
        return Some(cm::highlight(&*mode, &lines, MAX_HIGHLIGHT_BYTES));
    }
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
}
