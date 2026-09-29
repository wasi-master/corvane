//! `codemirror/mode/stylus/stylus.js` (`text/x-styl`).
//!
//! Ported function by function (JS names in comments). The tokenizers
//! return `[style, type]`; the parser states (`states.block`,
//! `states.parens`, …) then rewrite the style through the `override`
//! variable and switch state, pushing and popping a context stack.
//! Context indentation (`indent`, `line.firstWord`, `line.indent`) only
//! feeds `indent()` (and is `NaN` anyway, GHD passing no `indentUnit`), so
//! the contexts keep just their type.
//!
//! JS keyword lookups use `word in keySet`, which also finds
//! `Object.prototype` members; of those only `constructor` and `__proto__`
//! can be a lower-cased word, and [`key_set`] treats them the same way.
//! Popping the last context throws in JS; here the root stays put.

use std::borrow::Cow;

use crate::re;

use super::super::{Mode, ModeState, StringStream, state};

/// `states` names (`state.state` and `context.type`)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Block,
    Parens,
    VendorPrefixes,
    Pseudo,
    AtBlock,
    AtBlockParens,
    Keyframes,
    Interpolation,
    Extend,
    VariableName,
}

/// `state.tokenize`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tokenize {
    /// `null`: `tokenBase`
    Base,
    CComment,
    /// `tokenString(quote)`
    String(char),
    Parenthesized,
}

#[derive(Clone, Debug)]
pub struct StylusState {
    tokenize: Tokenize,
    state: St,
    /// `state.context` chain of types, innermost last
    context: Vec<St>,
}

pub struct Stylus;

/// The per-token scratch variables of the JS closure (`type`, `override`).
struct Tok {
    kind: Option<Cow<'static, str>>,
    over: Option<&'static str>,
}

impl Tok {
    fn kind(&self) -> &str {
        self.kind.as_deref().unwrap_or("")
    }
    fn is(&self, t: &str) -> bool {
        self.kind.as_deref() == Some(t)
    }
}

/// JS `\s`
fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// JS `\w`
fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `word in keySet(list)` (`list` sorted; `Object.prototype` members too)
fn key_set(list: &[&str], word: &str) -> bool {
    list.binary_search(&word).is_ok() || word == "constructor" || word == "__proto__"
}

/// `keySet(list).hasOwnProperty(word)`
fn own_key(list: &[&str], word: &str) -> bool {
    list.binary_search(&word).is_ok()
}

/// `wordRegexp(words)` (`^((w1)|(w2)|…)\b`), consuming.
fn match_words(stream: &mut StringStream, words: &[&str]) -> bool {
    for w in words {
        if stream.match_str(w, false, false) {
            let n = w.len();
            let last_is_word = w.chars().last().is_some_and(is_word);
            let next_is_word = stream.char_at(stream.pos + n).is_some_and(is_word);
            if last_is_word != next_is_word {
                stream.pos += n;
                return true;
            }
        }
    }
    false
}

/// `wordIsTag`
fn word_is_tag(word: &str) -> bool {
    key_set(TAG_KEYWORDS, &word.to_lowercase())
}

/// `wordIsProperty`
fn word_is_property(word: &str) -> bool {
    let word = word.to_lowercase();
    key_set(PROPERTY_KEYWORDS, &word) || key_set(FONT_PROPERTIES, &word)
}

/// `wordIsBlock`
fn word_is_block(word: &str) -> bool {
    key_set(BLOCK_KEYWORDS, &word.to_lowercase())
}

/// `wordAsValue`
fn word_as_value(word: &str) -> &'static str {
    let lc = word.to_lowercase();
    if word_is_tag(word) {
        "tag"
    } else if word_is_block(word) {
        "block-keyword"
    } else if word_is_property(word) {
        "property"
    } else if key_set(VALUE_KEYWORDS, &lc) || key_set(COMMON_ATOMS, &lc) {
        "atom"
    } else if lc == "return" || key_set(COLOR_KEYWORDS, &lc) {
        "keyword"
    } else if word.starts_with(|c: char| c.is_ascii_uppercase()) {
        // Font family
        "string"
    } else {
        "variable-2"
    }
}

/// `tagVariablesRegexp` (`/^(a|b|i|s|col|em)$/i`)
fn is_tag_variable(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "a" | "b" | "i" | "s" | "col" | "em"
    )
}

/// `startOfLine`: `sol()`, or the line is blanks then the current token.
fn start_of_line(stream: &StringStream) -> bool {
    if stream.sol() {
        return true;
    }
    let current = stream.current();
    let line = stream.string();
    // `^\s*` + escaped current: any number of the leading blanks
    for (i, c) in line.char_indices() {
        if line[i..].starts_with(current) {
            return true;
        }
        if !js_space(c) {
            return false;
        }
    }
    current.is_empty()
}

/// `endOfLine`
fn end_of_line(stream: &mut StringStream) -> bool {
    stream.eol() || stream.match_re(re!(r"^\s*$"), false).is_some()
}

/// `firstWordOfLine(line)`
fn first_word_of_line(line: &str) -> &str {
    re!(r"^\s*[-_]*[a-zA-Z0-9]+[A-Za-z0-9_-]*")
        .find(line)
        .ok()
        .flatten()
        .map_or("", |m| m.as_str().trim_start_matches(js_space))
}

/// `stream.string.match(new RegExp("\\[\\s*" + word + "|" + word + "\\s*\\]"))`
/// for a tag name (letters and digits only).
fn tag_in_brackets(line: &str, word: &str) -> bool {
    if word.is_empty() {
        return true;
    }
    line.match_indices('[')
        .any(|(i, _)| line[i + 1..].trim_start_matches(js_space).starts_with(word))
        || line.match_indices(word).any(|(i, _)| {
            line[i + word.len()..]
                .trim_start_matches(js_space)
                .starts_with(']')
        })
}

/// `/@(font-face|media|supports|(-moz-)?document)/.test(type)`
fn is_at_block_type(t: &str) -> bool {
    [
        "@font-face",
        "@media",
        "@supports",
        "@document",
        "@-moz-document",
    ]
    .iter()
    .any(|w| t.contains(w))
}

impl Stylus {
    /// `tokenBase`
    fn token_base(
        stream: &mut StringStream,
        s: &mut StylusState,
    ) -> (Option<&'static str>, Option<Cow<'static, str>>) {
        let ch = stream.peek();

        // Line comment
        if stream.match_str("//", true, false) {
            stream.skip_to_end();
            return (Some("comment"), Some("comment".into()));
        }
        // Block comment
        if stream.match_str("/*", true, false) {
            s.tokenize = Tokenize::CComment;
            return Self::token_c_comment(stream, s);
        }
        // String
        if let Some(q @ ('"' | '\'')) = ch {
            stream.next();
            s.tokenize = Tokenize::String(q);
            return Self::token_string(q, stream, s);
        }
        // Def
        if ch == Some('@') {
            stream.next();
            stream.eat_while_if(|c| is_word(c) || c == '\\' || c == '-');
            return (Some("def"), Some(Cow::Owned(stream.current().to_string())));
        }
        // ID selector or Hex color
        if ch == Some('#') {
            stream.next();
            // Hex color
            if stream.matches(re!(
                r"^[0-9a-fA-F]{3}([0-9a-fA-F]([0-9a-fA-F]{2}){0,2})?(?![A-Za-z0-9_])(?!-)"
            )) {
                return (Some("atom"), Some("atom".into()));
            }
            // ID selector
            if stream.matches(re!(r"^[a-zA-Z][A-Za-z0-9_-]*")) {
                return (Some("builtin"), Some("hash".into()));
            }
        }
        // Vendor prefixes
        if stream.matches(re!(
            r"^-([Mm][Oo][Zz]|[Mm][Ss]|[Oo]|[Ww][Ee][Bb][Kk][Ii][Tt])-"
        )) {
            return (Some("meta"), Some("vendor-prefixes".into()));
        }
        // Numbers
        if stream.matches(re!(r"^-?[0-9]?\.?[0-9]")) {
            stream.eat_while_if(|c| c.is_ascii_alphabetic() || c == '%');
            return (Some("number"), Some("unit".into()));
        }
        // !important|optional
        if ch == Some('!') {
            stream.next();
            let important = stream.match_str("important", true, true)
                || stream.match_str("optional", true, true);
            return (
                Some(if important { "keyword" } else { "operator" }),
                Some("important".into()),
            );
        }
        // Class
        if ch == Some('.') && stream.matches(re!(r"^\.[a-zA-Z][A-Za-z0-9_-]*")) {
            return (Some("qualifier"), Some("qualifier".into()));
        }
        // url url-prefix domain regexp
        if match_words(stream, DOCUMENT_TYPES_REGEXP) {
            if stream.peek() == Some('(') {
                s.tokenize = Tokenize::Parenthesized;
            }
            return (Some("property"), Some("word".into()));
        }
        // Mixins / Functions
        if stream.matches(re!(r"^[a-zA-Z][A-Za-z0-9_-]*\(")) {
            stream.back_up(1);
            return (Some("keyword"), Some("mixin".into()));
        }
        // Block mixins
        if stream.matches(re!(r"^(\+|-)[a-zA-Z][A-Za-z0-9_-]*\(")) {
            stream.back_up(1);
            return (Some("keyword"), Some("block-mixin".into()));
        }
        // Parent Reference BEM naming
        if re!(r"^\s*&").is_match(stream.string()).unwrap_or(false)
            && stream.matches(re!(r"^[-_]+[a-z][A-Za-z0-9_-]*"))
        {
            return (Some("qualifier"), Some("qualifier".into()));
        }
        // / Root Reference & Parent Reference
        if stream.matches(re!(r"^(/|&)(-|_|:|\.|#|[a-z])")) {
            stream.back_up(1);
            return (Some("variable-3"), Some("reference".into()));
        }
        if stream.matches(re!(r"^&{1}\s*$")) {
            return (Some("variable-3"), Some("reference".into()));
        }
        // Word operator
        if match_words(stream, WORD_OPERATOR_KEYWORDS_REGEXP) {
            return (Some("operator"), Some("operator".into()));
        }
        // Word
        if stream.matches(re!(r"^\$?[-_]*[a-zA-Z0-9]+[A-Za-z0-9_-]*")) {
            // Variable
            if stream
                .match_re(re!(r#"^(\.|\[)[A-Za-z0-9_\-'"\]]+"#), false)
                .is_some()
                && !word_is_tag(stream.current())
            {
                stream.match_str(".", true, false);
                return (Some("variable-2"), Some("variable-name".into()));
            }
            return (Some("variable-2"), Some("word".into()));
        }
        // Operators
        if stream.matches(re!(
            r"^\s*([.]{2,3}|&&|\|\||\*\*|[?!=:]?=|[-+*/%<>]=?|\?:|~)"
        )) {
            return (
                Some("operator"),
                Some(Cow::Owned(stream.current().to_string())),
            );
        }
        // Delimiters
        if let Some(c) = ch
            && matches!(c, ':' | ';' | ',' | '{' | '}' | '[' | ']' | '(' | ')')
        {
            stream.next();
            return (None, Some(Cow::Owned(c.to_string())));
        }
        // Non-detected items
        stream.next();
        (None, None)
    }

    /// `tokenCComment`
    fn token_c_comment(
        stream: &mut StringStream,
        s: &mut StylusState,
    ) -> (Option<&'static str>, Option<Cow<'static, str>>) {
        let mut maybe_end = false;
        while let Some(ch) = stream.next() {
            if maybe_end && ch == '/' {
                s.tokenize = Tokenize::Base;
                break;
            }
            maybe_end = ch == '*';
        }
        (Some("comment"), Some("comment".into()))
    }

    /// `tokenString(quote)`
    fn token_string(
        quote: char,
        stream: &mut StringStream,
        s: &mut StylusState,
    ) -> (Option<&'static str>, Option<Cow<'static, str>>) {
        let mut escaped = false;
        // `ch == quote` after the loop: only when it broke on the quote
        let mut closed = false;
        while let Some(ch) = stream.next() {
            if ch == quote && !escaped {
                if quote == ')' {
                    stream.back_up(1);
                }
                closed = true;
                break;
            }
            escaped = !escaped && ch == '\\';
        }
        if closed || (!escaped && quote != ')') {
            s.tokenize = Tokenize::Base;
        }
        (Some("string"), Some("string".into()))
    }

    /// `tokenParenthesized`
    fn token_parenthesized(
        stream: &mut StringStream,
        s: &mut StylusState,
    ) -> (Option<&'static str>, Option<Cow<'static, str>>) {
        stream.next(); // Must be "("
        s.tokenize = if stream.match_re(re!(r#"^\s*["')]"#), false).is_none() {
            Tokenize::String(')')
        } else {
            Tokenize::Base
        };
        (None, Some("(".into()))
    }

    /// `pushContext` (the indent argument only moves `context.indent`)
    fn push(s: &mut StylusState, kind: St) -> St {
        s.context.push(kind);
        kind
    }

    /// `popContext`
    fn pop(s: &mut StylusState) -> St {
        if s.context.len() > 1 {
            s.context.pop();
        }
        Self::top(s)
    }

    fn top(s: &StylusState) -> St {
        s.context.last().copied().unwrap_or(St::Block)
    }

    /// `pass`
    fn pass(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        let st = Self::top(s);
        Self::run(st, t, stream, s)
    }

    /// `popAndPass(type, stream, state)` (n = 1)
    fn pop_and_pass(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        if s.context.len() > 1 {
            s.context.pop();
        }
        Self::pass(t, stream, s)
    }

    fn run(st: St, t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        match st {
            St::Block => Self::block(t, stream, s),
            St::Parens => Self::parens(t, stream, s),
            St::VendorPrefixes => {
                // states.vendorPrefixes
                if t.is("word") {
                    t.over = Some("property");
                    return Self::push(s, St::Block);
                }
                Self::pop(s)
            }
            St::Pseudo => Self::pseudo(t, stream, s),
            St::AtBlock => Self::at_block(t, stream, s),
            St::AtBlockParens => Self::at_block_parens(t, stream, s),
            St::Keyframes => Self::keyframes(t, stream, s),
            St::Interpolation => Self::interpolation(t, stream, s),
            St::Extend => {
                // states.extend
                if t.is("[") || t.is("=") {
                    return St::Extend;
                }
                if t.is("]") {
                    return Self::pop(s);
                }
                if t.is("word") {
                    t.over = Some(word_as_value(stream.current()));
                    return St::Extend;
                }
                Self::pop(s)
            }
            St::VariableName => {
                // states.variableName
                let current = stream.current();
                if t.is("string")
                    || t.is("[")
                    || t.is("]")
                    || current.starts_with('.')
                    || current.starts_with('$')
                {
                    if re!(r"^\.[A-Za-z0-9_-]+").is_match(current).unwrap_or(false) {
                        t.over = Some("variable-2");
                    }
                    return St::VariableName;
                }
                Self::pop_and_pass(t, stream, s)
            }
        }
    }

    /// `typeIsBlock`
    fn type_is_block(t: &Tok, stream: &mut StringStream) -> bool {
        (matches!(t.kind(), "{" | "]" | "hash" | "qualifier") && end_of_line(stream))
            || t.is("block-mixin")
    }

    /// `typeIsInterpolation`
    fn type_is_interpolation(t: &Tok, stream: &mut StringStream) -> bool {
        t.is("{")
            && stream
                .match_re(re!(r"^\s*\$?[A-Za-z0-9_-]+"), false)
                .is_some()
    }

    /// `typeIsPseudo`
    fn type_is_pseudo(t: &Tok, stream: &mut StringStream) -> bool {
        t.is(":") && stream.match_re(re!(r"^[a-z-]+"), false).is_some()
    }

    /// `states.block`
    fn block(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        if (t.is("comment") && start_of_line(stream))
            || (t.is(",") && end_of_line(stream))
            || t.is("mixin")
        {
            return Self::push(s, St::Block);
        }
        if Self::type_is_interpolation(t, stream) {
            return Self::push(s, St::Interpolation);
        }
        if t.is("]")
            && end_of_line(stream)
            && !re!(r"^\s*(\.|#|:|\[|\*|&)")
                .is_match(stream.string())
                .unwrap_or(false)
            && !word_is_tag(first_word_of_line(stream.string()))
        {
            return Self::push(s, St::Block);
        }
        if Self::type_is_block(t, stream) {
            return Self::push(s, St::Block);
        }
        if t.is("}") && end_of_line(stream) {
            return Self::push(s, St::Block);
        }
        if t.is("variable-name") {
            return Self::push(s, St::VariableName);
        }
        if t.is("=") {
            return Self::push(s, St::Block);
        }
        if t.is("*")
            && (end_of_line(stream)
                || stream
                    .match_re(re!(r"^\s*(,|\.|#|\[|:|\{)"), false)
                    .is_some())
        {
            t.over = Some("tag");
            return Self::push(s, St::Block);
        }
        if Self::type_is_pseudo(t, stream) {
            return Self::push(s, St::Pseudo);
        }
        let kind = t.kind();
        if is_at_block_type(kind) {
            let eol = end_of_line(stream);
            return Self::push(s, if eol { St::Block } else { St::AtBlock });
        }
        if re!(r"@(-(moz|ms|o|webkit)-)?keyframes$")
            .is_match(kind)
            .unwrap_or(false)
        {
            return Self::push(s, St::Keyframes);
        }
        if kind.contains("@extend") {
            return Self::push(s, St::Extend);
        }
        if kind.starts_with('@') {
            // Property Lookup
            if stream.indentation() > 0 && word_is_property(stream.current().get(1..).unwrap_or(""))
            {
                t.over = Some("variable-2");
                return St::Block;
            }
            return Self::push(s, St::Block);
        }
        if t.is("reference") && end_of_line(stream) {
            return Self::push(s, St::Block);
        }
        if t.is("(") {
            return Self::push(s, St::Parens);
        }
        if t.is("vendor-prefixes") {
            return Self::push(s, St::VendorPrefixes);
        }
        if t.is("word") {
            let word = stream.current().to_string();
            let over = word_as_value(&word);
            t.over = Some(over);

            if over == "property" {
                if start_of_line(stream) {
                    return Self::push(s, St::Block);
                }
                t.over = Some("atom");
                return St::Block;
            }

            if over == "tag" {
                // tag is a css value
                if re!(r"embed|menu|pre|progress|sub|table")
                    .is_match(&word)
                    .unwrap_or(false)
                    && word_is_property(first_word_of_line(stream.string()))
                {
                    t.over = Some("atom");
                    return St::Block;
                }

                // tag is an attribute
                if tag_in_brackets(stream.string(), &word) {
                    t.over = Some("atom");
                    return St::Block;
                }

                // tag is a variable
                if is_tag_variable(&word) {
                    let sol = start_of_line(stream);
                    let first = first_word_of_line(stream.string());
                    if (sol && stream.string().contains('='))
                        || (!sol
                            && !re!(r"^(\s*\.|#|&|\[|/|>|\*)")
                                .is_match(stream.string())
                                .unwrap_or(false)
                            && !word_is_tag(first))
                    {
                        t.over = Some("variable-2");
                        if word_is_block(first) {
                            return St::Block;
                        }
                        return Self::push(s, St::Block);
                    }
                }

                if end_of_line(stream) {
                    return Self::push(s, St::Block);
                }
            }
            if over == "block-keyword" {
                t.over = Some("keyword");

                // Postfix conditionals (`stream.current(/(if|unless)/)` is
                // just the non-empty current token)
                if !start_of_line(stream) {
                    return St::Block;
                }
                return Self::push(s, St::Block);
            }
            if word == "return" {
                return Self::push(s, St::Block);
            }

            // Placeholder selector
            if t.over == Some("variable-2")
                && re!(r#"^\s?\$[A-Za-z0-9_\-.\[\]'"]+$"#)
                    .is_match(stream.string())
                    .unwrap_or(false)
            {
                return Self::push(s, St::Block);
            }
        }
        Self::top(s)
    }

    /// `states.parens`
    fn parens(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        if t.is("(") {
            return Self::push(s, St::Parens);
        }
        if t.is(")") {
            if s.context.len() > 1 && s.context[s.context.len() - 2] == St::Parens {
                return Self::pop(s);
            }
            let eol = end_of_line(stream);
            let line = stream.string();
            let first = first_word_of_line(line);
            if (re!(r"^[a-zA-Z][A-Za-z0-9_-]*\(")
                .is_match(line)
                .unwrap_or(false)
                && eol)
                || word_is_block(first)
                || first.contains(['.', '#', ':', '[', '*', '&', '>', '~', '+', '/'])
                || (!re!(r#"^-?[a-z][A-Za-z0-9_\-.\[\]'"]*\s*="#)
                    .is_match(line)
                    .unwrap_or(false)
                    && word_is_tag(first))
            {
                return Self::push(s, St::Block);
            }
            if re!(r#"^[$-]?[a-z][A-Za-z0-9_\-.\[\]'"]*\s*="#)
                .is_match(line)
                .unwrap_or(false)
                || re!(r"^\s*(\(|\)|[0-9])").is_match(line).unwrap_or(false)
                || re!(r"^\s+[a-zA-Z][A-Za-z0-9_-]*\(")
                    .is_match(line)
                    .unwrap_or(false)
                || re!(r"^\s+[$-]?[a-zA-Z]").is_match(line).unwrap_or(false)
            {
                return Self::push(s, St::Block);
            }
            return Self::push(s, St::Block);
        }
        if t.kind().starts_with('@') && word_is_property(stream.current().get(1..).unwrap_or("")) {
            t.over = Some("variable-2");
        }
        if t.is("word") {
            let word = stream.current();
            let mut over = word_as_value(word);
            if over == "tag" && is_tag_variable(word) {
                over = "variable-2";
            }
            if over == "property" || word == "to" {
                over = "atom";
            }
            t.over = Some(over);
        }
        if t.is("variable-name") {
            return Self::push(s, St::VariableName);
        }
        if Self::type_is_pseudo(t, stream) {
            return Self::push(s, St::Pseudo);
        }
        Self::top(s)
    }

    /// `states.pseudo`
    fn pseudo(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        if !word_is_property(first_word_of_line(stream.string())) {
            stream.matches(re!(r"^[a-z-]+"));
            t.over = Some("variable-3");
            if end_of_line(stream) {
                return Self::push(s, St::Block);
            }
            return Self::pop(s);
        }
        Self::pop_and_pass(t, stream, s)
    }

    /// `states.atBlock`
    fn at_block(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        if t.is("(") {
            return Self::push(s, St::AtBlockParens);
        }
        if Self::type_is_block(t, stream) {
            return Self::push(s, St::Block);
        }
        if Self::type_is_interpolation(t, stream) {
            return Self::push(s, St::Interpolation);
        }
        if t.is("word") {
            let word = stream.current().to_lowercase();
            let over = if matches!(word.as_str(), "only" | "not" | "and" | "or") {
                "keyword"
            } else if own_key(DOCUMENT_TYPES, &word) {
                "tag"
            } else if own_key(MEDIA_TYPES, &word) {
                "attribute"
            } else if own_key(MEDIA_FEATURES, &word) {
                "property"
            } else if own_key(NON_STANDARD_PROPERTY_KEYWORDS, &word) {
                "string-2"
            } else {
                word_as_value(stream.current())
            };
            t.over = Some(over);
            if over == "tag" && end_of_line(stream) {
                return Self::push(s, St::Block);
            }
        }
        if t.is("operator") && matches!(stream.current(), "not" | "and" | "or") {
            t.over = Some("keyword");
        }
        Self::top(s)
    }

    /// `states.atBlock_parens`
    fn at_block_parens(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        if t.is("{") || t.is("}") {
            return Self::top(s);
        }
        if t.is(")") {
            let eol = end_of_line(stream);
            return Self::push(s, if eol { St::Block } else { St::AtBlock });
        }
        if t.is("word") {
            let word = stream.current().to_lowercase();
            let mut over = word_as_value(&word);
            if word.starts_with("max") || word.starts_with("min") {
                over = "property";
            }
            if over == "tag" {
                over = if is_tag_variable(&word) {
                    "variable-2"
                } else {
                    "atom"
                };
            }
            t.over = Some(over);
            return Self::top(s);
        }
        Self::at_block(t, stream, s)
    }

    /// `states.keyframes`
    fn keyframes(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        if stream.indentation() == 0
            && ((t.is("}") && start_of_line(stream))
                || t.is("]")
                || t.is("hash")
                || t.is("qualifier")
                || word_is_tag(stream.current()))
        {
            return Self::pop_and_pass(t, stream, s);
        }
        if t.is("{") {
            return Self::push(s, St::Keyframes);
        }
        if t.is("}") {
            if start_of_line(stream) {
                return Self::pop(s);
            }
            return Self::push(s, St::Keyframes);
        }
        if t.is("unit")
            && re!(r"^[0-9]+%$")
                .is_match(stream.current())
                .unwrap_or(false)
        {
            return Self::push(s, St::Keyframes);
        }
        if t.is("word") {
            let over = word_as_value(stream.current());
            t.over = Some(over);
            if over == "block-keyword" {
                t.over = Some("keyword");
                return Self::push(s, St::Keyframes);
            }
        }
        if is_at_block_type(t.kind()) {
            let eol = end_of_line(stream);
            return Self::push(s, if eol { St::Block } else { St::AtBlock });
        }
        if t.is("mixin") {
            return Self::push(s, St::Block);
        }
        Self::top(s)
    }

    /// `states.interpolation`
    fn interpolation(t: &mut Tok, stream: &mut StringStream, s: &mut StylusState) -> St {
        if t.is("{") {
            Self::pop(s);
            Self::push(s, St::Block);
        }
        if t.is("}") {
            let line = stream.string();
            if re!(r"^\s*(\.|#|:|\[|\*|&|>|~|\+|/)")
                .is_match(line)
                .unwrap_or(false)
                || (re!(r"^\s*[a-zA-Z]").is_match(line).unwrap_or(false)
                    && word_is_tag(first_word_of_line(line)))
            {
                return Self::push(s, St::Block);
            }
            return Self::push(s, St::Block);
        }
        if t.is("variable-name") {
            return Self::push(s, St::VariableName);
        }
        if t.is("word") {
            let mut over = word_as_value(stream.current());
            if over == "tag" {
                over = "atom";
            }
            t.over = Some(over);
        }
        Self::top(s)
    }
}

impl Mode for Stylus {
    fn name(&self) -> &'static str {
        "stylus"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(StylusState {
            tokenize: Tokenize::Base,
            state: St::Block,
            context: vec![St::Block],
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<StylusState>(st);
        if s.tokenize == Tokenize::Base && stream.eat_while_if(js_space) {
            return None;
        }
        let (style, kind) = match s.tokenize {
            Tokenize::Base => Self::token_base(stream, s),
            Tokenize::CComment => Self::token_c_comment(stream, s),
            Tokenize::String(q) => Self::token_string(q, stream, s),
            Tokenize::Parenthesized => Self::token_parenthesized(stream, s),
        };
        let mut t = Tok { kind, over: style };
        s.state = Self::run(s.state, &mut t, stream, s);
        t.over.map(str::to_string)
    }
}

/// `wordRegexp(documentTypes_)` after its `sort(b > a)` (order unchanged)
static DOCUMENT_TYPES_REGEXP: &[&str] = &["domain", "regexp", "url-prefix", "url"];

/// `wordRegexp(wordOperatorKeywords_)` after its `sort(b > a)` (order
/// unchanged)
static WORD_OPERATOR_KEYWORDS_REGEXP: &[&str] = &[
    "in",
    "and",
    "or",
    "not",
    "is not",
    "is a",
    "is",
    "isnt",
    "defined",
    "if unless",
];

/// `tagKeywords_`
static TAG_KEYWORDS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "base",
    "bdi",
    "bdo",
    "bgsound",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "caption",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "datalist",
    "dd",
    "del",
    "details",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "i",
    "iframe",
    "img",
    "input",
    "ins",
    "kbd",
    "keygen",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "map",
    "mark",
    "marquee",
    "menu",
    "menuitem",
    "meta",
    "meter",
    "nav",
    "nobr",
    "noframes",
    "noscript",
    "object",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "param",
    "pre",
    "progress",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "script",
    "section",
    "select",
    "small",
    "source",
    "span",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "tr",
    "track",
    "u",
    "ul",
    "var",
    "video",
];

/// `documentTypes_`
static DOCUMENT_TYPES: &[&str] = &["domain", "regexp", "url", "url-prefix"];

/// `mediaTypes_`
static MEDIA_TYPES: &[&str] = &[
    "all",
    "aural",
    "braille",
    "embossed",
    "handheld",
    "print",
    "projection",
    "screen",
    "tty",
    "tv",
];

/// `mediaFeatures_`
static MEDIA_FEATURES: &[&str] = &[
    "aspect-ratio",
    "color",
    "color-index",
    "device-aspect-ratio",
    "device-height",
    "device-width",
    "dynamic-range",
    "grid",
    "height",
    "max-aspect-ratio",
    "max-color",
    "max-color-index",
    "max-device-aspect-ratio",
    "max-device-height",
    "max-device-width",
    "max-height",
    "max-monochrome",
    "max-resolution",
    "max-width",
    "min-aspect-ratio",
    "min-color",
    "min-color-index",
    "min-device-aspect-ratio",
    "min-device-height",
    "min-device-width",
    "min-height",
    "min-monochrome",
    "min-resolution",
    "min-width",
    "monochrome",
    "resolution",
    "scan",
    "video-dynamic-range",
    "width",
];

/// `propertyKeywords_`
static PROPERTY_KEYWORDS: &[&str] = &[
    "align-content",
    "align-items",
    "align-self",
    "alignment-adjust",
    "alignment-baseline",
    "anchor-point",
    "animation",
    "animation-delay",
    "animation-direction",
    "animation-duration",
    "animation-fill-mode",
    "animation-iteration-count",
    "animation-name",
    "animation-play-state",
    "animation-timing-function",
    "appearance",
    "azimuth",
    "backface-visibility",
    "background",
    "background-attachment",
    "background-clip",
    "background-color",
    "background-image",
    "background-origin",
    "background-position",
    "background-repeat",
    "background-size",
    "baseline-shift",
    "binding",
    "bleed",
    "bookmark-label",
    "bookmark-level",
    "bookmark-state",
    "bookmark-target",
    "border",
    "border-bottom",
    "border-bottom-color",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
    "border-bottom-style",
    "border-bottom-width",
    "border-collapse",
    "border-color",
    "border-image",
    "border-image-outset",
    "border-image-repeat",
    "border-image-slice",
    "border-image-source",
    "border-image-width",
    "border-left",
    "border-left-color",
    "border-left-style",
    "border-left-width",
    "border-radius",
    "border-right",
    "border-right-color",
    "border-right-style",
    "border-right-width",
    "border-spacing",
    "border-style",
    "border-top",
    "border-top-color",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-top-style",
    "border-top-width",
    "border-width",
    "bottom",
    "box-decoration-break",
    "box-shadow",
    "box-sizing",
    "break-after",
    "break-before",
    "break-inside",
    "caption-side",
    "clear",
    "clip",
    "clip-path",
    "clip-rule",
    "color",
    "color-interpolation",
    "color-interpolation-filters",
    "color-profile",
    "color-rendering",
    "column-count",
    "column-fill",
    "column-gap",
    "column-rule",
    "column-rule-color",
    "column-rule-style",
    "column-rule-width",
    "column-span",
    "column-width",
    "columns",
    "content",
    "counter-increment",
    "counter-reset",
    "crop",
    "cue",
    "cue-after",
    "cue-before",
    "cursor",
    "direction",
    "display",
    "dominant-baseline",
    "drop-initial-after-adjust",
    "drop-initial-after-align",
    "drop-initial-before-adjust",
    "drop-initial-before-align",
    "drop-initial-size",
    "drop-initial-value",
    "elevation",
    "empty-cells",
    "enable-background",
    "fill",
    "fill-opacity",
    "fill-rule",
    "filter",
    "fit",
    "fit-position",
    "flex",
    "flex-basis",
    "flex-direction",
    "flex-flow",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "float",
    "float-offset",
    "flood-color",
    "flood-opacity",
    "flow-from",
    "flow-into",
    "font",
    "font-family",
    "font-feature-settings",
    "font-kerning",
    "font-language-override",
    "font-size",
    "font-size-adjust",
    "font-smoothing",
    "font-stretch",
    "font-style",
    "font-synthesis",
    "font-variant",
    "font-variant-alternates",
    "font-variant-caps",
    "font-variant-east-asian",
    "font-variant-ligatures",
    "font-variant-numeric",
    "font-variant-position",
    "font-weight",
    "glyph-orientation-horizontal",
    "glyph-orientation-vertical",
    "grid",
    "grid-area",
    "grid-auto-columns",
    "grid-auto-flow",
    "grid-auto-position",
    "grid-auto-rows",
    "grid-column",
    "grid-column-end",
    "grid-column-start",
    "grid-row",
    "grid-row-end",
    "grid-row-start",
    "grid-template",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
    "hanging-punctuation",
    "height",
    "hyphens",
    "icon",
    "image-orientation",
    "image-rendering",
    "image-resolution",
    "inline-box-align",
    "justify-content",
    "left",
    "letter-spacing",
    "lighting-color",
    "line-break",
    "line-height",
    "line-stacking",
    "line-stacking-ruby",
    "line-stacking-shift",
    "line-stacking-strategy",
    "list-style",
    "list-style-image",
    "list-style-position",
    "list-style-type",
    "margin",
    "margin-bottom",
    "margin-left",
    "margin-right",
    "margin-top",
    "marker",
    "marker-end",
    "marker-mid",
    "marker-offset",
    "marker-start",
    "marks",
    "marquee-direction",
    "marquee-loop",
    "marquee-play-count",
    "marquee-speed",
    "marquee-style",
    "mask",
    "max-height",
    "max-width",
    "min-height",
    "min-width",
    "move-to",
    "nav-down",
    "nav-index",
    "nav-left",
    "nav-right",
    "nav-up",
    "object-fit",
    "object-position",
    "opacity",
    "order",
    "orphans",
    "osx-font-smoothing",
    "outline",
    "outline-color",
    "outline-offset",
    "outline-style",
    "outline-width",
    "overflow",
    "overflow-style",
    "overflow-wrap",
    "overflow-x",
    "overflow-y",
    "padding",
    "padding-bottom",
    "padding-left",
    "padding-right",
    "padding-top",
    "page",
    "page-break-after",
    "page-break-before",
    "page-break-inside",
    "page-policy",
    "pause",
    "pause-after",
    "pause-before",
    "perspective",
    "perspective-origin",
    "pitch",
    "pitch-range",
    "play-during",
    "pointer-events",
    "position",
    "presentation-level",
    "punctuation-trim",
    "quotes",
    "region-break-after",
    "region-break-before",
    "region-break-inside",
    "region-fragment",
    "rendering-intent",
    "resize",
    "rest",
    "rest-after",
    "rest-before",
    "richness",
    "right",
    "rotation",
    "rotation-point",
    "ruby-align",
    "ruby-overhang",
    "ruby-position",
    "ruby-span",
    "shape-image-threshold",
    "shape-inside",
    "shape-margin",
    "shape-outside",
    "shape-rendering",
    "size",
    "speak",
    "speak-as",
    "speak-header",
    "speak-numeral",
    "speak-punctuation",
    "speech-rate",
    "stop-color",
    "stop-opacity",
    "stress",
    "string-set",
    "stroke",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "stroke-width",
    "tab-size",
    "table-layout",
    "target",
    "target-name",
    "target-new",
    "target-position",
    "text-align",
    "text-align-last",
    "text-anchor",
    "text-decoration",
    "text-decoration-color",
    "text-decoration-line",
    "text-decoration-skip",
    "text-decoration-style",
    "text-emphasis",
    "text-emphasis-color",
    "text-emphasis-position",
    "text-emphasis-style",
    "text-height",
    "text-indent",
    "text-justify",
    "text-outline",
    "text-overflow",
    "text-rendering",
    "text-shadow",
    "text-size-adjust",
    "text-space-collapse",
    "text-transform",
    "text-underline-position",
    "text-wrap",
    "top",
    "transform",
    "transform-origin",
    "transform-style",
    "transition",
    "transition-delay",
    "transition-duration",
    "transition-property",
    "transition-timing-function",
    "unicode-bidi",
    "vertical-align",
    "visibility",
    "voice-balance",
    "voice-duration",
    "voice-family",
    "voice-pitch",
    "voice-range",
    "voice-rate",
    "voice-stress",
    "voice-volume",
    "volume",
    "white-space",
    "widows",
    "width",
    "will-change",
    "word-break",
    "word-spacing",
    "word-wrap",
    "writing-mode",
    "z-index",
];

/// `nonStandardPropertyKeywords_`
static NON_STANDARD_PROPERTY_KEYWORDS: &[&str] = &[
    "scrollbar-3d-light-color",
    "scrollbar-arrow-color",
    "scrollbar-base-color",
    "scrollbar-dark-shadow-color",
    "scrollbar-face-color",
    "scrollbar-highlight-color",
    "scrollbar-shadow-color",
    "scrollbar-track-color",
    "searchfield-cancel-button",
    "searchfield-decoration",
    "searchfield-results-button",
    "searchfield-results-decoration",
    "shape-inside",
    "zoom",
];

/// `fontProperties_`
static FONT_PROPERTIES: &[&str] = &[
    "font-family",
    "font-feature-settings",
    "font-stretch",
    "font-style",
    "font-variant",
    "font-weight",
    "src",
    "unicode-range",
];

/// `colorKeywords_`
static COLOR_KEYWORDS: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

/// `valueKeywords_`
static VALUE_KEYWORDS: &[&str] = &[
    "above",
    "absolute",
    "activeborder",
    "activecaption",
    "additive",
    "afar",
    "after-white-space",
    "ahead",
    "alias",
    "all",
    "all-scroll",
    "alphabetic",
    "alternate",
    "always",
    "amharic",
    "amharic-abegede",
    "antialiased",
    "appworkspace",
    "arabic-indic",
    "armenian",
    "asterisks",
    "attr",
    "auto",
    "avoid",
    "avoid-column",
    "avoid-page",
    "avoid-region",
    "background",
    "backwards",
    "baseline",
    "below",
    "bengali",
    "bicubic",
    "bidi-override",
    "binary",
    "blink",
    "block",
    "block-axis",
    "bold",
    "bolder",
    "border",
    "border-box",
    "both",
    "bottom",
    "break",
    "break-all",
    "break-word",
    "bullets",
    "button",
    "buttonface",
    "buttonhighlight",
    "buttonshadow",
    "buttontext",
    "calc",
    "cambodian",
    "capitalize",
    "caps-lock-indicator",
    "caption",
    "captiontext",
    "caret",
    "cell",
    "center",
    "checkbox",
    "circle",
    "cjk-decimal",
    "cjk-earthly-branch",
    "cjk-heavenly-stem",
    "cjk-ideographic",
    "clear",
    "clip",
    "close-quote",
    "col-resize",
    "collapse",
    "column",
    "column-reverse",
    "compact",
    "condensed",
    "conic-gradient",
    "contain",
    "content",
    "content-box",
    "contents",
    "context-menu",
    "continuous",
    "copy",
    "counter",
    "counters",
    "cover",
    "crop",
    "cross",
    "crosshair",
    "currentcolor",
    "cursive",
    "cyclic",
    "dashed",
    "decimal",
    "decimal-leading-zero",
    "default",
    "default-button",
    "destination-atop",
    "destination-in",
    "destination-out",
    "destination-over",
    "devanagari",
    "disc",
    "discard",
    "disclosure-closed",
    "disclosure-open",
    "document",
    "dot-dash",
    "dot-dot-dash",
    "dotted",
    "double",
    "down",
    "e-resize",
    "ease",
    "ease-in",
    "ease-in-out",
    "ease-out",
    "element",
    "ellipse",
    "ellipsis",
    "embed",
    "end",
    "ethiopic",
    "ethiopic-abegede",
    "ethiopic-abegede-am-et",
    "ethiopic-abegede-gez",
    "ethiopic-abegede-ti-er",
    "ethiopic-abegede-ti-et",
    "ethiopic-halehame-aa-er",
    "ethiopic-halehame-aa-et",
    "ethiopic-halehame-am-et",
    "ethiopic-halehame-gez",
    "ethiopic-halehame-om-et",
    "ethiopic-halehame-sid-et",
    "ethiopic-halehame-so-et",
    "ethiopic-halehame-ti-er",
    "ethiopic-halehame-ti-et",
    "ethiopic-halehame-tig",
    "ethiopic-numeric",
    "ew-resize",
    "expanded",
    "extends",
    "extra-condensed",
    "extra-expanded",
    "fantasy",
    "fast",
    "fill",
    "fixed",
    "flat",
    "flex",
    "flex-end",
    "flex-start",
    "footnotes",
    "forwards",
    "from",
    "geometricPrecision",
    "georgian",
    "grayscale",
    "graytext",
    "groove",
    "gujarati",
    "gurmukhi",
    "hand",
    "hangul",
    "hangul-consonant",
    "hebrew",
    "help",
    "hidden",
    "hide",
    "high",
    "higher",
    "highlight",
    "highlighttext",
    "hiragana",
    "hiragana-iroha",
    "horizontal",
    "hsl",
    "hsla",
    "icon",
    "ignore",
    "inactiveborder",
    "inactivecaption",
    "inactivecaptiontext",
    "infinite",
    "infobackground",
    "infotext",
    "inherit",
    "initial",
    "inline",
    "inline-axis",
    "inline-block",
    "inline-flex",
    "inline-table",
    "inset",
    "inside",
    "intrinsic",
    "invert",
    "italic",
    "japanese-formal",
    "japanese-informal",
    "justify",
    "kannada",
    "katakana",
    "katakana-iroha",
    "keep-all",
    "khmer",
    "korean-hangul-formal",
    "korean-hanja-formal",
    "korean-hanja-informal",
    "landscape",
    "lao",
    "large",
    "larger",
    "left",
    "level",
    "lighter",
    "line-through",
    "linear",
    "linear-gradient",
    "lines",
    "list-item",
    "listbox",
    "listitem",
    "local",
    "logical",
    "loud",
    "lower",
    "lower-alpha",
    "lower-armenian",
    "lower-greek",
    "lower-hexadecimal",
    "lower-latin",
    "lower-norwegian",
    "lower-roman",
    "lowercase",
    "ltr",
    "malayalam",
    "match",
    "matrix",
    "matrix3d",
    "media-play-button",
    "media-slider",
    "media-sliderthumb",
    "media-volume-slider",
    "media-volume-sliderthumb",
    "medium",
    "menu",
    "menulist",
    "menulist-button",
    "menutext",
    "message-box",
    "middle",
    "min-intrinsic",
    "mix",
    "mongolian",
    "monospace",
    "move",
    "multiple",
    "myanmar",
    "n-resize",
    "narrower",
    "ne-resize",
    "nesw-resize",
    "no-close-quote",
    "no-drop",
    "no-open-quote",
    "no-repeat",
    "none",
    "normal",
    "not-allowed",
    "nowrap",
    "ns-resize",
    "numbers",
    "numeric",
    "nw-resize",
    "nwse-resize",
    "oblique",
    "octal",
    "open-quote",
    "optimizeLegibility",
    "optimizeSpeed",
    "optimizespeed",
    "oriya",
    "oromo",
    "outset",
    "outside",
    "outside-shape",
    "overlay",
    "overline",
    "padding",
    "padding-box",
    "page",
    "painted",
    "paused",
    "persian",
    "perspective",
    "plus-darker",
    "plus-lighter",
    "pointer",
    "polygon",
    "portrait",
    "pre",
    "pre-line",
    "pre-wrap",
    "preserve-3d",
    "progress",
    "push-button",
    "radial-gradient",
    "radio",
    "read-only",
    "read-write",
    "read-write-plaintext-only",
    "rectangle",
    "region",
    "relative",
    "repeat",
    "repeat-x",
    "repeat-y",
    "repeating-conic-gradient",
    "repeating-linear-gradient",
    "repeating-radial-gradient",
    "reset",
    "reverse",
    "rgb",
    "rgba",
    "ridge",
    "right",
    "rotate",
    "rotate3d",
    "rotateX",
    "rotateY",
    "rotateZ",
    "round",
    "row",
    "row-resize",
    "row-reverse",
    "rtl",
    "run-in",
    "running",
    "s-resize",
    "sans-serif",
    "scale",
    "scale3d",
    "scaleX",
    "scaleY",
    "scaleZ",
    "scroll",
    "scroll-position",
    "scrollbar",
    "se-resize",
    "searchfield",
    "searchfield-cancel-button",
    "searchfield-decoration",
    "searchfield-results-button",
    "searchfield-results-decoration",
    "semi-condensed",
    "semi-expanded",
    "separate",
    "serif",
    "show",
    "sidama",
    "simp-chinese-formal",
    "simp-chinese-informal",
    "single",
    "skew",
    "skewX",
    "skewY",
    "skip-white-space",
    "slide",
    "slider-horizontal",
    "slider-vertical",
    "sliderthumb-horizontal",
    "sliderthumb-vertical",
    "slow",
    "small",
    "small-caps",
    "small-caption",
    "smaller",
    "solid",
    "somali",
    "source-atop",
    "source-in",
    "source-out",
    "source-over",
    "space",
    "space-around",
    "space-between",
    "spell-out",
    "square",
    "square-button",
    "standard",
    "start",
    "static",
    "status-bar",
    "stretch",
    "stroke",
    "sub",
    "subpixel-antialiased",
    "super",
    "sw-resize",
    "symbolic",
    "symbols",
    "table",
    "table-caption",
    "table-cell",
    "table-column",
    "table-column-group",
    "table-footer-group",
    "table-header-group",
    "table-row",
    "table-row-group",
    "tamil",
    "telugu",
    "text",
    "text-bottom",
    "text-top",
    "textarea",
    "textfield",
    "thai",
    "thick",
    "thin",
    "threeddarkshadow",
    "threedface",
    "threedhighlight",
    "threedlightshadow",
    "threedshadow",
    "tibetan",
    "tigre",
    "tigrinya-er",
    "tigrinya-er-abegede",
    "tigrinya-et",
    "tigrinya-et-abegede",
    "to",
    "top",
    "trad-chinese-formal",
    "trad-chinese-informal",
    "translate",
    "translate3d",
    "translateX",
    "translateY",
    "translateZ",
    "transparent",
    "ultra-condensed",
    "ultra-expanded",
    "underline",
    "unset",
    "up",
    "upper-alpha",
    "upper-armenian",
    "upper-greek",
    "upper-hexadecimal",
    "upper-latin",
    "upper-norwegian",
    "upper-roman",
    "uppercase",
    "urdu",
    "url",
    "var",
    "vertical",
    "vertical-text",
    "visible",
    "visibleFill",
    "visiblePainted",
    "visibleStroke",
    "visual",
    "w-resize",
    "wait",
    "wave",
    "wider",
    "window",
    "windowframe",
    "windowtext",
    "words",
    "wrap",
    "wrap-reverse",
    "x-large",
    "x-small",
    "xor",
    "xx-large",
    "xx-small",
];

/// `blockKeywords_`
static BLOCK_KEYWORDS: &[&str] = &["else", "for", "from", "if", "to", "unless"];

/// `commonAtoms_`
static COMMON_ATOMS: &[&str] = &[
    "disabled",
    "false",
    "href",
    "not-allowed",
    "null",
    "readonly",
    "title",
    "true",
    "type",
];
