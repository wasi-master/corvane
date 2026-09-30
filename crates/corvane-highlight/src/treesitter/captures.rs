//! Tree-sitter capture names → GitHub Desktop's colour classes.
//!
//! Queries come from three vocabularies (the grammars' own and Zed's, nvim-
//! treesitter's, Helix's, plus a few TextMate-style names); all of them land on the thirteen `--syntax-*`
//! colours GHD's `.cm-s-default` theme has ([`TokenClass`]). Where CodeMirror
//! has an equivalent the mapping follows it: numbers, operators, brackets
//! and definitions stay in the line colour, `this`/`self` are keywords, C
//! preprocessor lines and doctypes are `meta` (comment colour), markdown
//! code is `comment`. Functions take the variable colour (GitHub's purple
//! "entity"), members and parameters the `variable-2` colour.
//!
//! A capture resolves by its longest dot-prefix in [`STYLES`] (Zed's rule):
//! `keyword.control.import` → `keyword`. A name with no prefix in the table
//! leaves the text in the colour of the enclosing capture.
//! `.docs/tree-sitter-captures.md` is generated from this table
//! (`UPDATE_TS_CAPTURES_DOC=1 cargo test -p corvane-highlight captures_doc`).

use crate::TokenClass;

/// What a capture does to its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// coloured with a class
    Class(TokenClass),
    /// the line's own colour, overriding an enclosing capture
    Plain,
    /// whatever the enclosing capture gives (no colour of its own)
    Inherit,
}

use Style::{Class, Inherit, Plain};
use TokenClass::*;

/// Capture name (or dot-prefix) → style, grouped by class. Sorted lookups
/// are not needed: the table is small and resolution runs once per name.
pub const STYLES: &[(&str, Style)] = &[
    // comment
    ("comment", Class(Comment)),
    ("keyword.directive", Class(Comment)),
    ("keyword.control.directive", Class(Comment)),
    ("preproc", Class(Comment)),
    ("tag.doctype", Class(Comment)),
    ("text.literal", Class(Comment)),
    ("markup.raw", Class(Comment)),
    ("punctuation.list_marker", Class(Comment)),
    ("markup.list.marker", Class(Comment)),
    // string
    ("string", Class(String)),
    ("character", Class(String)),
    ("escape", Class(String)),
    ("constant.character", Class(String)),
    ("text.uri", Class(String)),
    ("link_uri", Class(String)),
    ("markup.link.url", Class(String)),
    // keyword
    ("keyword", Class(Keyword)),
    ("conditional", Class(Keyword)),
    ("repeat", Class(Keyword)),
    ("include", Class(Keyword)),
    ("exception", Class(Keyword)),
    ("storageclass", Class(Keyword)),
    ("type.qualifier", Class(Keyword)),
    ("variable.builtin", Class(Keyword)),
    ("variable.special", Class(Keyword)),
    ("variable.language", Class(Keyword)),
    // atom
    ("boolean", Class(Atom)),
    ("constant", Class(Atom)),
    ("string.special.symbol", Class(Atom)),
    ("symbol", Class(Atom)),
    // type
    ("type", Class(Type)),
    ("constructor", Class(Type)),
    ("enum", Class(Type)),
    ("variant", Class(Type)),
    // variable
    ("variable", Class(Variable)),
    ("function", Class(Variable)),
    ("method", Class(Variable)),
    ("macro", Class(Variable)),
    // alt variable (`variable-2`)
    ("variable.parameter", Class(AltVariable)),
    ("parameter", Class(AltVariable)),
    ("variable.member", Class(AltVariable)),
    ("variable.other.member", Class(AltVariable)),
    ("property", Class(AltVariable)),
    ("field", Class(AltVariable)),
    ("label", Class(AltVariable)),
    ("markup.list", Class(AltVariable)),
    ("selector.pseudo", Class(AltVariable)),
    // qualifier
    ("module", Class(Qualifier)),
    ("namespace", Class(Qualifier)),
    // tag
    ("tag", Class(Tag)),
    ("selector", Class(Tag)),
    // attribute
    ("attribute", Class(Attribute)),
    ("tag.attribute", Class(Attribute)),
    // link, header, quote
    ("link_text", Class(Link)),
    ("markup.link", Class(Link)),
    ("text.reference", Class(Link)),
    ("title", Class(Header)),
    ("text.title", Class(Header)),
    ("markup.heading", Class(Header)),
    ("markup.quote", Class(Quote)),
    ("text.quote", Class(Quote)),
    // the line colour (CodeMirror leaves these unstyled)
    ("number", Plain),
    ("float", Plain),
    ("constant.numeric", Plain),
    ("operator", Plain),
    ("punctuation", Plain),
    ("embedded", Plain),
    ("emphasis", Plain),
    ("markup.italic", Plain),
    ("markup.strong", Plain),
    ("markup.strikethrough", Plain),
    ("markup.underline", Plain),
    ("text.emphasis", Plain),
    ("text.strong", Plain),
    ("text.strike", Plain),
    ("diff", Plain),
    ("error", Plain),
    // TextMate-style names a few grammars' own queries use
    ("entity.name.type", Class(Type)),
    ("entity.name.function", Class(Variable)),
    ("entity.name", Class(Variable)),
    ("meta.attribute", Class(Attribute)),
    ("storage", Class(Keyword)),
    // no colour of their own (nvim's `@none` is an empty group: an injected
    // HTML layer's `(text) @none` must not wipe the comment it sits in)
    ("none", Inherit),
    ("spell", Inherit),
    ("nospell", Inherit),
    ("conceal", Inherit),
    ("text", Inherit),
    ("markup", Inherit),
    ("local", Inherit),
    ("injection", Inherit),
    ("hint", Inherit),
    ("predictive", Inherit),
    ("primary", Inherit),
];

/// The style of a capture name: its longest dot-prefix in [`STYLES`];
/// private names (`_x`) and unknown ones inherit.
pub fn resolve(name: &str) -> Style {
    if name.starts_with('_') {
        return Inherit;
    }
    let mut prefix = name;
    loop {
        if let Some((_, style)) = STYLES.iter().find(|(n, _)| *n == prefix) {
            return *style;
        }
        match prefix.rfind('.') {
            Some(dot) => prefix = &prefix[..dot],
            None => return Inherit,
        }
    }
}

/// Whether `name` has an entry (itself or a dot-prefix) in [`STYLES`].
pub fn is_known(name: &str) -> bool {
    if name.starts_with('_') {
        return true;
    }
    let mut prefix = name;
    loop {
        if STYLES.iter().any(|(n, _)| *n == prefix) {
            return true;
        }
        match prefix.rfind('.') {
            Some(dot) => prefix = &prefix[..dot],
            None => return false,
        }
    }
}

/// The Markdown reference `.docs/tree-sitter-captures.md`.
pub fn reference_doc() -> std::string::String {
    let mut out = std::string::String::from(
        "# Tree-sitter captures\n\n\
         Generated from `crates/corvane-highlight/src/treesitter/captures.rs` \
         (`UPDATE_TS_CAPTURES_DOC=1 cargo test -p corvane-highlight captures_doc`).\n\n\
         How the tree-sitter highlighter (Settings › Appearance › Syntax highlighting, \
         flag `105-tree-sitter-highlighting`) colours a capture: its longest dot-prefix below \
         decides (`keyword.control.import` → `keyword`). \"line colour\" leaves the text \
         unstyled like CodeMirror does; \"enclosing\" keeps the colour of the capture around it.\n\n\
         | Capture | Colour |\n|---|---|\n",
    );
    for (name, style) in STYLES {
        let colour = match style {
            Class(class) => format!("`--syntax-{}-color`", css_name(*class)),
            Plain => "line colour".to_string(),
            Inherit => "enclosing".to_string(),
        };
        out.push_str(&format!("| `{name}` | {colour} |\n"));
    }
    out
}

fn css_name(class: TokenClass) -> &'static str {
    match class {
        Variable => "variable",
        AltVariable => "alt-variable",
        Keyword => "keyword",
        Atom => "atom",
        String => "string",
        Qualifier => "qualifier",
        Type => "type",
        Comment => "comment",
        Tag => "tag",
        Attribute => "attribute",
        Link => "link",
        Header => "header",
        Quote => "quote",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_prefix_wins() {
        assert_eq!(resolve("keyword.control.import"), Class(Keyword));
        assert_eq!(resolve("keyword.directive.define"), Class(Comment));
        assert_eq!(resolve("variable"), Class(Variable));
        assert_eq!(resolve("variable.parameter.builtin"), Class(AltVariable));
        assert_eq!(resolve("variable.builtin"), Class(Keyword));
        assert_eq!(resolve("string.special.symbol"), Class(Atom));
        assert_eq!(resolve("string.special.url"), Class(String));
        assert_eq!(resolve("constant.numeric.integer"), Plain);
        assert_eq!(resolve("constant.character.escape"), Class(String));
        assert_eq!(resolve("markup.heading.1"), Class(Header));
        assert_eq!(resolve("markup.list.unchecked"), Class(AltVariable));
        assert_eq!(resolve("punctuation.bracket"), Plain);
        assert_eq!(resolve("spell"), Inherit);
        assert_eq!(resolve("_private"), Inherit);
        assert_eq!(resolve("somethingelse"), Inherit);
    }

    #[test]
    fn entries_are_unique() {
        let mut names: Vec<&str> = STYLES.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate capture entries");
    }

    #[test]
    fn captures_doc() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.docs/tree-sitter-captures.md"
        );
        let doc = reference_doc();
        if std::env::var_os("UPDATE_TS_CAPTURES_DOC").is_some() {
            std::fs::write(path, &doc).expect("write doc");
        }
        let current = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            current == doc,
            "{path} is stale: UPDATE_TS_CAPTURES_DOC=1 cargo test -p corvane-highlight captures_doc"
        );
    }
}
