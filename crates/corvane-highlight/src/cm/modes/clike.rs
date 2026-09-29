//! `codemirror/mode/clike/clike.js`: the C-family mode and every MIME the
//! file defines (`def(...)` / `defineMIME`): C, C++, Java, C#, Scala,
//! Kotlin, GLSL shaders, nesC, Objective-C, Objective-C++, Squirrel and
//! Ceylon. `mode/dart/dart.js` builds on it (see [`super::dart`]).
//!
//! Each MIME's parser config is a [`Config`] value, mirroring the JS
//! `def(mimes, {...})` objects; the per-char `hooks` and `hooks.token`
//! functions become [`Hook`] / [`TokenHook`] enums, and the `state.tokenize`
//! closures become [`Tokenize`]. The closure variables `curPunc` and
//! `isDefKeyword` live in a per-call [`Cur`].
//!
//! Only the tokenizer is ported: `indent`, `electricInput` and `fold` are
//! editor-only, so a context keeps just what tokens depend on (its type,
//! whether its info is `"namespace"` for `isTopScope`, and `align` for
//! Scala's `=>` hook).
//!
//! Ceylon's `stringTokenizer` is a module-level variable in the JS; here it
//! lives in the document state, which is the same thing for one document.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use fancy_regex::Regex;

use super::dart;
use crate::cm::{self, Mode, ModeState, StringStream};
use crate::re;

/// A CodeMirror style (`null` = `None`).
pub(super) type Style = Option<&'static str>;

/// JS `words(str)`: an object with one key per space-separated word.
#[derive(Default)]
pub(super) struct Words(HashSet<&'static str>);

impl Words {
    /// `words(a + " " + b + ...)`
    pub(super) fn of(parts: &[&'static str]) -> Self {
        Words(parts.iter().flat_map(|p| p.split(' ')).collect())
    }
    fn contains(&self, word: &str) -> bool {
        self.0.contains(word)
    }
}

/// `parserConfig.types`: a word set or a predicate.
#[derive(Default)]
pub(super) enum Types {
    #[default]
    None,
    Words(Words),
    /// `cTypes`
    C,
    /// `objCTypes`
    ObjC,
    /// Ceylon's "starts with an uppercase letter"
    Ceylon,
}

impl Types {
    fn contains(&self, word: &str) -> bool {
        match self {
            Types::None => false,
            Types::Words(w) => w.contains(word),
            Types::C => c_types(word),
            Types::ObjC => c_types(word) || BASIC_OBJC_TYPES.contains(&word),
            Types::Ceylon => word.chars().next().is_some_and(|c| {
                let upper = c.to_uppercase().eq(std::iter::once(c));
                let lower = c.to_lowercase().eq(std::iter::once(c));
                upper && !lower
            }),
        }
    }
}

/// `parserConfig.number`
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum NumberRe {
    #[default]
    Default,
    Java,
    Kotlin,
    Ceylon,
}

impl NumberRe {
    /// `stream.match(number)` (consuming).
    fn matches(self, stream: &mut StringStream) -> bool {
        // every pattern but Ceylon's (whose number start excludes ".")
        // needs a digit after a leading "."; skip the regex for member access
        if self != NumberRe::Ceylon
            && stream.peek() == Some('.')
            && !stream
                .char_at(stream.pos + 1)
                .is_some_and(|c| c.is_ascii_digit())
        {
            return false;
        }
        let re = match self {
            NumberRe::Default => {
                re!(
                    r"^(?i:0x[a-f0-9]+|0b[01]+|(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:e[-+]?[0-9]+)?)(?i:u|ll?|l|f)?"
                )
            }
            NumberRe::Java => {
                re!(
                    r"^(?i:0x[a-f0-9_]+|0b[01_]+|(?:[0-9_]+\.?[0-9]*|\.[0-9]+)(?:e[-+]?[0-9_]+)?)(?i:u|ll?|l|f)?"
                )
            }
            NumberRe::Kotlin => {
                re!(
                    r"^(?i:0x[a-f0-9_]+|0b[01_]+|(?:[0-9_]+(?:\.[0-9]+)?|\.[0-9]+)(?:e[-+]?[0-9_]+)?)(?i:u|ll?|l|f)?"
                )
            }
            NumberRe::Ceylon => {
                re!(
                    r"^(?i:#[0-9a-f_]+|\$[01_]+|[0-9_]+[kmgtpunf]?|[0-9_]+\.[0-9_]+(?:e[-+]?[0-9]+|[kmgtpunf]|)|)"
                )
            }
        };
        match_consume(stream, re)
    }
}

/// A per-char entry of `parserConfig.hooks`.
#[derive(Clone, Copy)]
pub(super) enum Hook {
    /// `cppHook`
    Cpp,
    /// `pointerHook`
    Pointer,
    /// `cpp11StringHook`
    Cpp11String,
    /// `cpp14Literal`
    Cpp14Literal,
    /// Java `"@"`: annotations, but not `@interface`
    JavaAt,
    /// Java `'"'`: text blocks
    JavaQuote,
    /// C# `"@"`: verbatim strings and attributes
    CSharpAt,
    /// Scala / Kotlin / Ceylon `"@"`: `eatWhile(/[\w\$_]/)` → meta
    MetaAt,
    /// Scala `'"'`: triple-quoted strings
    ScalaQuote,
    /// Scala `"'"`: chars and symbols
    ScalaApos,
    /// Scala `"="`: `=>` in a block
    ScalaEq,
    /// Scala / Kotlin / Dart `"/"`: nested block comments
    NestedComment,
    /// Kotlin `'*'`
    KotlinStar,
    /// Kotlin `'"'`
    KotlinQuote,
    /// Ceylon `'"'`
    CeylonQuote,
    /// Ceylon `` '`' ``
    CeylonBacktick,
    /// Ceylon `"'"`
    CeylonApos,
    /// Dart `"@"`
    DartAt,
    /// Dart `"'"` / `'"'`
    DartQuote,
    /// Dart `"r"`
    DartRaw,
    /// Dart `"}"`
    DartCloseBrace,
}

/// `parserConfig.hooks.token`
#[derive(Clone, Copy)]
pub(super) enum TokenHook {
    /// C++ / Objective-C++: `Foo::Foo(` is a definition
    Cpp,
    /// Ceylon: `.member` is variable-2
    Ceylon,
    /// Dart: capitalised identifiers are variable-2
    Dart,
}

/// A `def(...)` parser config.
pub(super) struct Config {
    pub(super) keywords: Words,
    pub(super) types: Types,
    pub(super) builtin: Words,
    pub(super) block_keywords: Words,
    pub(super) def_keywords: Words,
    pub(super) atoms: Words,
    pub(super) hooks: fn(char) -> Option<Hook>,
    pub(super) token_hook: Option<TokenHook>,
    pub(super) multi_line_strings: bool,
    pub(super) indent_statements: bool,
    pub(super) namespace_separator: Option<&'static str>,
    pub(super) is_punctuation_char: fn(char) -> bool,
    pub(super) number_start: fn(char) -> bool,
    pub(super) number: NumberRe,
    pub(super) is_operator_char: fn(char) -> bool,
    pub(super) is_identifier_char: fn(char) -> bool,
    pub(super) is_reserved_identifier: Option<fn(&str) -> bool>,
    pub(super) type_first_definitions: bool,
    pub(super) style_defs: bool,
}

impl Default for Config {
    /// The `parserConfig.x || default` fallbacks at the top of the mode.
    fn default() -> Self {
        Self {
            keywords: Words::default(),
            types: Types::None,
            builtin: Words::default(),
            block_keywords: Words::default(),
            def_keywords: Words::default(),
            atoms: Words::default(),
            hooks: |_| None,
            token_hook: None,
            multi_line_strings: false,
            indent_statements: true,
            namespace_separator: None,
            is_punctuation_char,
            number_start: |c| c.is_ascii_digit() || c == '.',
            number: NumberRe::Default,
            is_operator_char: |c| "+-*&%=<>!?|/".contains(c),
            is_identifier_char,
            is_reserved_identifier: None,
            type_first_definitions: false,
            style_defs: true,
        }
    }
}

/// `/[\[\]{}\(\),;\:\.]/`
fn is_punctuation_char(c: char) -> bool {
    matches!(c, '[' | ']' | '{' | '}' | '(' | ')' | ',' | ';' | ':' | '.')
}

/// `/[\w\$_\xa1-￿]/` (JS `\w` is ASCII; astral chars are two
/// surrogates in that range)
pub(super) fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c >= '\u{a1}'
}

/// `/[\w\$_]/`
fn is_word_dollar(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// JS `\s`
pub(super) fn is_js_space(c: char) -> bool {
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

/// `stream.match(re)` (consuming) without building the match groups.
pub(super) fn match_consume(stream: &mut StringStream, re: &Regex) -> bool {
    let rest = stream.slice(stream.pos, stream.len());
    let n = match re.find(rest) {
        Ok(Some(m)) if m.start() == 0 => rest[..m.end()].chars().count(),
        _ => return false,
    };
    stream.pos += n;
    true
}

/// `stream.match(/^\/[\/*]/, false)`
fn at_comment_start(stream: &StringStream) -> bool {
    stream.peek() == Some('/') && matches!(stream.char_at(stream.pos + 1), Some('/' | '*'))
}

/// A punctuation char as the `curPunc` / context type string.
fn punc_str(c: char) -> &'static str {
    match c {
        '[' => "[",
        ']' => "]",
        '{' => "{",
        '}' => "}",
        '(' => "(",
        ')' => ")",
        ',' => ",",
        ';' => ";",
        ':' => ":",
        '.' => ".",
        '`' => "`",
        _ => "",
    }
}

/// `state.tokenize` (`null` = [`Tokenize::Base`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tokenize {
    /// `tokenBase`
    Base,
    /// `tokenString(quote)`
    String(char),
    /// `tokenComment`
    Comment,
    /// `cppHook` (a preprocessor line continued with `\`)
    CppHook,
    /// `tokenRawString` (delimiter in `State::cpp11_raw_string_delim`)
    RawString,
    /// `tokenAtString`
    AtString,
    /// `tokenTripleString`
    TripleString,
    /// `tokenNestedComment(depth)`
    NestedComment(u32),
    /// `tokenKotlinString(tripleString)`
    KotlinString(bool),
    /// `tokenCeylonString(type)`, `true` for "triple"
    CeylonString(bool),
    /// dart.js `tokenStringHelper` of `tokenString(quote, …, raw)`
    DartString {
        quote: char,
        raw: bool,
        triple: bool,
    },
    /// dart.js `tokenInterpolation`
    DartInterpolation,
    /// dart.js `tokenInterpolationIdentifier`
    DartInterpolationIdentifier,
}

/// `Context`, reduced to what tokens depend on.
#[derive(Clone, Debug)]
pub(super) struct Context {
    /// `"top"`, `"}"`, `"]"`, `")"` or `"statement"`
    ty: &'static str,
    /// `info == "namespace"`
    namespace: bool,
    align: Option<bool>,
}

#[derive(Clone, Debug)]
pub(super) struct State {
    pub(super) tokenize: Tokenize,
    /// the context chain, innermost last (never empty: "top" stays)
    context: Vec<Context>,
    start_of_line: bool,
    prev_token: Option<&'static str>,
    type_at_end_of_line: bool,
    cpp11_raw_string_delim: String,
    /// dart.js `state.interpolationStack`
    pub(super) interpolation_stack: Vec<Tokenize>,
    /// clike.js module-level `stringTokenizer` (Ceylon)
    ceylon_string_tokenizer: Option<Tokenize>,
}

impl State {
    fn top_type(&self) -> &'static str {
        self.context.last().map_or("top", |c| c.ty)
    }
    /// `pushContext`
    fn push_context(&mut self, ty: &'static str, namespace: bool) {
        self.context.push(Context {
            ty,
            namespace,
            align: None,
        });
    }
    /// `popContext`
    fn pop_context(&mut self) {
        if self.context.len() > 1 {
            self.context.pop();
        }
    }
}

/// `curPunc` / `isDefKeyword` for one `token` call, plus whether a hook
/// replaced the current context object (Scala's `=>`).
#[derive(Default)]
pub(super) struct Cur {
    punc: Option<&'static str>,
    is_def_keyword: bool,
    ctx_replaced: bool,
}

/// `isTopScope(state.context)`
fn is_top_scope(context: &[Context]) -> bool {
    for i in (0..context.len()).rev() {
        let c = &context[i];
        if c.ty == "top" {
            return true;
        }
        if c.ty == "}" && !(i > 0 && context[i - 1].namespace) {
            return false;
        }
    }
    true
}

/// `/\S(?:[^- ]>|[*\]])\s*$|\*$/.test(text)`
fn type_before_text(text: &str) -> bool {
    if text.ends_with('*') {
        return true;
    }
    let mut it = text.trim_end_matches(is_js_space).chars().rev();
    match it.next() {
        Some('*' | ']') => it.next().is_some_and(|c| !is_js_space(c)),
        Some('>') => match it.next() {
            Some(c) if c != '-' && c != ' ' => it.next().is_some_and(|c| !is_js_space(c)),
            _ => false,
        },
        _ => false,
    }
}

/// `typeBefore(stream, state, pos)`
fn type_before(stream: &mut StringStream, state: &State, pos: usize) -> bool {
    if matches!(state.prev_token, Some("variable" | "type")) {
        return true;
    }
    if type_before_text(stream.slice(0, pos)) {
        return true;
    }
    state.type_at_end_of_line && stream.column() == stream.indentation()
}

/// `/.+_t$/` or a basic C type (`cTypes`)
fn c_types(word: &str) -> bool {
    const BASIC: &[&str] = &[
        "int", "long", "char", "short", "double", "float", "unsigned", "signed", "void", "bool",
    ];
    BASIC.contains(&word) || (word.ends_with("_t") && word.chars().nth(2).is_some())
}

const BASIC_OBJC_TYPES: &[&str] = &["SEL", "instancetype", "id", "Class", "Protocol", "BOOL"];

/// `cIsReservedIdentifier`: `__x` or `_X`
fn c_is_reserved_identifier(token: &str) -> bool {
    let mut chars = token.chars();
    if chars.next() != Some('_') {
        return false;
    }
    match chars.next() {
        Some('_') => true,
        Some(c) => !c.to_lowercase().eq(std::iter::once(c)),
        None => false,
    }
}

/// `cppHook`: a preprocessor line (only at the start of a line), carried
/// to the next line by a trailing `\`. `None` = `false`.
fn cpp_hook(stream: &mut StringStream, state: &mut State) -> Option<Style> {
    if !state.start_of_line {
        return None;
    }
    let mut next = Tokenize::Base;
    while let Some(ch) = stream.peek() {
        if ch == '\\' && stream.pos + 1 == stream.len() {
            stream.pos += 1;
            next = Tokenize::CppHook;
            break;
        } else if ch == '/' && at_comment_start(stream) {
            break;
        }
        stream.next();
    }
    state.tokenize = next;
    Some(Some("meta"))
}

/// `cpp11StringHook`: raw strings and `u8"…"` / `L'…'` prefixes.
fn cpp11_string_hook(stream: &mut StringStream, state: &mut State) -> Option<Style> {
    stream.back_up(1);
    // raw strings
    if match_consume(stream, re!(r"^(?:R|u8R|uR|UR|LR)")) {
        let m = stream.match_re(re!(r#"^"([^\s\\()]{0,16})\("#), true)?;
        state.cpp11_raw_string_delim = m.group(1).unwrap_or("").to_string();
        state.tokenize = Tokenize::RawString;
        return Some(token_raw_string(stream, state));
    }
    // unicode strings / chars
    if match_consume(stream, re!(r"^(?:u8|u|U|L)")) {
        if matches!(stream.peek(), Some('"' | '\'')) {
            return Some(Some("string"));
        }
        return None;
    }
    // ignore this hook
    stream.next();
    None
}

/// `cppLooksLikeConstructor`: `/(\w+)::~?(\w+)$/` with equal names.
fn cpp_looks_like_constructor(word: &str) -> bool {
    re!(r"([A-Za-z0-9_]+)::~?([A-Za-z0-9_]+)$")
        .captures(word)
        .ok()
        .flatten()
        .is_some_and(|c| c.get(1).map(|m| m.as_str()) == c.get(2).map(|m| m.as_str()))
}

/// `tokenString(quote)`
fn token_string(
    quote: char,
    multi_line: bool,
    stream: &mut StringStream,
    state: &mut State,
) -> Style {
    let mut escaped = false;
    let mut end = false;
    while let Some(next) = stream.next() {
        if next == quote && !escaped {
            end = true;
            break;
        }
        escaped = !escaped && next == '\\';
    }
    if end || !(escaped || multi_line) {
        state.tokenize = Tokenize::Base;
    }
    Some("string")
}

/// `tokenComment`
fn token_comment(stream: &mut StringStream, state: &mut State) -> Style {
    let mut maybe_end = false;
    while let Some(ch) = stream.next() {
        if ch == '/' && maybe_end {
            state.tokenize = Tokenize::Base;
            break;
        }
        maybe_end = ch == '*';
    }
    Some("comment")
}

/// `tokenAtString`: C# `@"…"` where `""` escapes a quote.
fn token_at_string(stream: &mut StringStream, state: &mut State) -> Style {
    while let Some(next) = stream.next() {
        if next == '"' && stream.eat('"').is_none() {
            state.tokenize = Tokenize::Base;
            break;
        }
    }
    Some("string")
}

/// `tokenRawString`: up to `)<delim>"`.
fn token_raw_string(stream: &mut StringStream, state: &mut State) -> Style {
    // `stream.match(new RegExp(".*?\\)" + delim + '"'))`: the first
    // occurrence of the terminator
    let needle = format!("){}\"", state.cpp11_raw_string_delim);
    if stream.skip_to_str(&needle) {
        stream.pos += needle.chars().count();
        state.tokenize = Tokenize::Base;
    } else {
        stream.skip_to_end();
    }
    Some("string")
}

/// `tokenTripleString`
fn token_triple_string(stream: &mut StringStream, state: &mut State) -> Style {
    let mut escaped = false;
    while !stream.eol() {
        if !escaped && stream.match_str("\"\"\"", true, false) {
            state.tokenize = Tokenize::Base;
            break;
        }
        escaped = stream.next() == Some('\\') && !escaped;
    }
    Some("string")
}

/// `tokenNestedComment(depth)` (the JS recursion on a depth change is the
/// same loop continuing with the new depth).
pub(super) fn token_nested_comment(
    mut depth: u32,
    stream: &mut StringStream,
    state: &mut State,
) -> Style {
    while let Some(ch) = stream.next() {
        if ch == '*' && stream.eat('/').is_some() {
            if depth == 1 {
                state.tokenize = Tokenize::Base;
                break;
            }
            depth -= 1;
            state.tokenize = Tokenize::NestedComment(depth);
        } else if ch == '/' && stream.eat('*').is_some() {
            depth += 1;
            state.tokenize = Tokenize::NestedComment(depth);
        }
    }
    Some("comment")
}

/// `tokenKotlinString(tripleString)`
fn token_kotlin_string(triple: bool, stream: &mut StringStream, state: &mut State) -> Style {
    let mut escaped = false;
    let mut end = false;
    while !stream.eol() {
        if !triple && !escaped && stream.match_str("\"", true, false) {
            end = true;
            break;
        }
        if triple && stream.match_str("\"\"\"", true, false) {
            end = true;
            break;
        }
        let next = stream.next();
        if !escaped && next == Some('$') && stream.match_str("{", true, false) {
            stream.skip_to('}');
        }
        escaped = !escaped && next == Some('\\') && !triple;
    }
    if end || !triple {
        state.tokenize = Tokenize::Base;
    }
    Some("string")
}

/// `tokenCeylonString(type)`
fn token_ceylon_string(triple: bool, stream: &mut StringStream, state: &mut State) -> Style {
    let mut escaped = false;
    let mut end = false;
    while !stream.eol() {
        if !escaped
            && stream.match_str("\"", true, false)
            && (!triple || stream.match_str("\"\"", true, false))
        {
            end = true;
            break;
        }
        if !escaped && stream.match_str("``", true, false) {
            state.ceylon_string_tokenizer = Some(Tokenize::CeylonString(triple));
            end = true;
            break;
        }
        let next = stream.next();
        escaped = !triple && !escaped && next == Some('\\');
    }
    if end {
        state.tokenize = Tokenize::Base;
    }
    Some("string")
}

/// The clike mode for one parser config.
pub struct Clike {
    cfg: Config,
}

impl Clike {
    /// `(state.tokenize || tokenBase)(stream, state)`
    fn tokenize(&self, stream: &mut StringStream, state: &mut State, cur: &mut Cur) -> Style {
        match state.tokenize {
            Tokenize::Base => self.token_base(stream, state, cur),
            Tokenize::String(q) => token_string(q, self.cfg.multi_line_strings, stream, state),
            Tokenize::Comment => token_comment(stream, state),
            Tokenize::CppHook => cpp_hook(stream, state).flatten(),
            Tokenize::RawString => token_raw_string(stream, state),
            Tokenize::AtString => token_at_string(stream, state),
            Tokenize::TripleString => token_triple_string(stream, state),
            Tokenize::NestedComment(depth) => token_nested_comment(depth, stream, state),
            Tokenize::KotlinString(triple) => token_kotlin_string(triple, stream, state),
            Tokenize::CeylonString(triple) => token_ceylon_string(triple, stream, state),
            Tokenize::DartString { quote, raw, triple } => {
                dart::token_string_helper(quote, raw, triple, stream, state)
            }
            Tokenize::DartInterpolation => dart::token_interpolation(stream, state),
            Tokenize::DartInterpolationIdentifier => {
                dart::token_interpolation_identifier(stream, state)
            }
        }
    }

    /// `hooks[ch](stream, state)`; `None` = the hook returned `false`.
    fn run_hook(
        &self,
        hook: Hook,
        stream: &mut StringStream,
        state: &mut State,
        cur: &mut Cur,
    ) -> Option<Style> {
        match hook {
            Hook::Cpp => cpp_hook(stream, state),
            Hook::Pointer => (state.prev_token == Some("type")).then_some(Some("type")),
            Hook::Cpp11String => cpp11_string_hook(stream, state),
            Hook::Cpp14Literal => {
                stream.eat_while_if(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '\''));
                Some(Some("number"))
            }
            Hook::JavaAt => {
                // don't match the @interface keyword
                if stream.match_str("interface", false, false) {
                    return None;
                }
                stream.eat_while_if(is_word_dollar);
                Some(Some("meta"))
            }
            Hook::JavaQuote => {
                // `stream.match(/""$/)`: the line ends right after `"""`
                if !(stream.pos + 2 == stream.len() && stream.match_str("\"\"", true, false)) {
                    return None;
                }
                state.tokenize = Tokenize::TripleString;
                Some(self.tokenize(stream, state, cur))
            }
            Hook::CSharpAt => {
                if stream.eat('"').is_some() {
                    state.tokenize = Tokenize::AtString;
                    return Some(token_at_string(stream, state));
                }
                stream.eat_while_if(is_word_dollar);
                Some(Some("meta"))
            }
            Hook::MetaAt => {
                stream.eat_while_if(is_word_dollar);
                Some(Some("meta"))
            }
            Hook::ScalaQuote => {
                if !stream.match_str("\"\"", true, false) {
                    return None;
                }
                state.tokenize = Tokenize::TripleString;
                Some(self.tokenize(stream, state, cur))
            }
            Hook::ScalaApos => {
                if match_consume(stream, re!(r"^(?:\\[^'\s]+|[^\\'])'")) {
                    return Some(Some("string-2"));
                }
                stream.eat_while_if(is_identifier_char);
                Some(Some("atom"))
            }
            Hook::ScalaEq => {
                let cx = state.context.last_mut()?;
                if cx.ty == "}" && cx.align == Some(true) && stream.eat('>').is_some() {
                    // `state.context = new Context(…, null, cx.prev)`
                    cx.align = None;
                    cur.ctx_replaced = true;
                    Some(Some("operator"))
                } else {
                    None
                }
            }
            Hook::NestedComment => {
                stream.eat('*')?;
                state.tokenize = Tokenize::NestedComment(1);
                Some(self.tokenize(stream, state, cur))
            }
            Hook::KotlinStar => Some(Some(if state.prev_token == Some(".") {
                "variable"
            } else {
                "operator"
            })),
            Hook::KotlinQuote => {
                let triple = stream.match_str("\"\"", true, false);
                state.tokenize = Tokenize::KotlinString(triple);
                Some(self.tokenize(stream, state, cur))
            }
            Hook::CeylonQuote => {
                let triple = stream.match_str("\"\"", true, false);
                state.tokenize = Tokenize::CeylonString(triple);
                Some(self.tokenize(stream, state, cur))
            }
            Hook::CeylonBacktick => {
                if state.ceylon_string_tokenizer.is_none() || !stream.match_str("`", true, false) {
                    return None;
                }
                state.tokenize = state.ceylon_string_tokenizer.take()?;
                Some(self.tokenize(stream, state, cur))
            }
            Hook::CeylonApos => {
                stream.eat_while_if(is_identifier_char);
                Some(Some("atom"))
            }
            Hook::DartAt => {
                stream.eat_while_if(|c| is_word_dollar(c) || c == '.');
                Some(Some("meta"))
            }
            Hook::DartQuote => {
                let quote = stream.char_at(stream.pos.wrapping_sub(1))?;
                Some(dart::token_string(quote, stream, state, false))
            }
            Hook::DartRaw => match stream.peek() {
                Some(q @ ('\'' | '"')) => {
                    stream.next();
                    Some(dart::token_string(q, stream, state, true))
                }
                _ => None,
            },
            Hook::DartCloseBrace => {
                // "}" ends an interpolation if the stack is non-empty
                let prev = state.interpolation_stack.pop()?;
                state.tokenize = prev;
                Some(None)
            }
        }
    }

    /// `tokenBase`
    fn token_base(&self, stream: &mut StringStream, state: &mut State, cur: &mut Cur) -> Style {
        let cfg = &self.cfg;
        let ch = stream.next()?;
        if let Some(hook) = (cfg.hooks)(ch)
            && let Some(result) = self.run_hook(hook, stream, state, cur)
        {
            return result;
        }
        if ch == '"' || ch == '\'' {
            state.tokenize = Tokenize::String(ch);
            return self.tokenize(stream, state, cur);
        }
        if (cfg.number_start)(ch) {
            stream.back_up(1);
            if cfg.number.matches(stream) {
                return Some("number");
            }
            stream.next();
        }
        if (cfg.is_punctuation_char)(ch) {
            cur.punc = Some(punc_str(ch));
            return None;
        }
        if ch == '/' {
            if stream.eat('*').is_some() {
                state.tokenize = Tokenize::Comment;
                return token_comment(stream, state);
            }
            if stream.eat('/').is_some() {
                stream.skip_to_end();
                return Some("comment");
            }
        }
        if (cfg.is_operator_char)(ch) {
            while !at_comment_start(stream) && stream.eat_if(cfg.is_operator_char).is_some() {}
            return Some("operator");
        }
        stream.eat_while_if(cfg.is_identifier_char);
        if let Some(sep) = cfg.namespace_separator {
            while stream.match_str(sep, true, false) {
                stream.eat_while_if(cfg.is_identifier_char);
            }
        }

        let word = stream.current();
        if cfg.keywords.contains(word) {
            if cfg.block_keywords.contains(word) {
                cur.punc = Some("newstatement");
            }
            if cfg.def_keywords.contains(word) {
                cur.is_def_keyword = true;
            }
            return Some("keyword");
        }
        if cfg.types.contains(word) {
            return Some("type");
        }
        if cfg.builtin.contains(word) || cfg.is_reserved_identifier.is_some_and(|f| f(word)) {
            if cfg.block_keywords.contains(word) {
                cur.punc = Some("newstatement");
            }
            return Some("builtin");
        }
        if cfg.atoms.contains(word) {
            return Some("atom");
        }
        Some("variable")
    }

    /// `maybeEOL`
    fn maybe_eol(&self, stream: &mut StringStream, state: &mut State) {
        if self.cfg.type_first_definitions && stream.eol() && is_top_scope(&state.context) {
            state.type_at_end_of_line = type_before(stream, state, stream.pos);
        }
    }

    /// `hooks.token(stream, state, style)`; `None` = `undefined`.
    fn token_hook(
        &self,
        stream: &StringStream,
        state: &State,
        style: Style,
    ) -> Option<&'static str> {
        match self.cfg.token_hook? {
            TokenHook::Cpp => (style == Some("variable")
                && stream.peek() == Some('(')
                && matches!(state.prev_token, Some(";") | None | Some("}"))
                && cpp_looks_like_constructor(stream.current()))
            .then_some("def"),
            TokenHook::Ceylon => (matches!(style, Some("variable" | "type"))
                && state.prev_token == Some("."))
            .then_some("variable-2"),
            TokenHook::Dart => (style == Some("variable") && dart::is_upper(stream.current()))
                .then_some("variable-2"),
        }
    }
}

impl Mode for Clike {
    fn name(&self) -> &'static str {
        "clike"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(State {
            tokenize: Tokenize::Base,
            context: vec![Context {
                ty: "top",
                namespace: false,
                align: Some(false),
            }],
            start_of_line: true,
            prev_token: None,
            type_at_end_of_line: false,
            cpp11_raw_string_delim: String::new(),
            interpolation_stack: Vec::new(),
            ceylon_string_tokenizer: None,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = cm::state::<State>(st);
        let cfg = &self.cfg;
        if stream.sol() {
            if let Some(ctx) = state.context.last_mut()
                && ctx.align.is_none()
            {
                ctx.align = Some(false);
            }
            state.start_of_line = true;
        }
        if stream.eat_space() {
            self.maybe_eol(stream, state);
            return None;
        }
        let mut cur = Cur::default();
        let mut style = self.tokenize(stream, state, &mut cur);
        if matches!(style, Some("comment" | "meta")) {
            return style.map(str::to_string);
        }
        if !cur.ctx_replaced
            && let Some(ctx) = state.context.last_mut()
            && ctx.align.is_none()
        {
            ctx.align = Some(true);
        }

        let punc = cur.punc;
        let ctx_type = state.top_type();
        let rest_blank = |stream: &StringStream| {
            // `stream.match(/^\s*(?:\/\/.*)?$/, false)`
            let rest = stream.slice(stream.pos, stream.len());
            let rest = rest.trim_start_matches(is_js_space);
            rest.is_empty() || rest.starts_with("//")
        };
        if punc == Some(";") || punc == Some(":") || (punc == Some(",") && rest_blank(stream)) {
            while state.top_type() == "statement" {
                state.pop_context();
            }
        } else if punc == Some("{") {
            state.push_context("}", false);
        } else if punc == Some("[") {
            state.push_context("]", false);
        } else if punc == Some("(") {
            state.push_context(")", false);
        } else if punc == Some("}") {
            while state.top_type() == "statement" {
                state.pop_context();
            }
            if state.top_type() == "}" {
                state.pop_context();
            }
            while state.top_type() == "statement" {
                state.pop_context();
            }
        } else if punc == Some(ctx_type) {
            state.pop_context();
        } else if cfg.indent_statements
            && (((ctx_type == "}" || ctx_type == "top") && punc != Some(";"))
                || (ctx_type == "statement" && punc == Some("newstatement")))
        {
            let namespace = stream.current() == "namespace";
            state.push_context("statement", namespace);
        }

        if style == Some("variable")
            && (state.prev_token == Some("def")
                || (cfg.type_first_definitions
                    && type_before(stream, state, stream.start)
                    && is_top_scope(&state.context)
                    && {
                        // `stream.match(/^\s*\(/, false)`
                        let rest = stream.slice(stream.pos, stream.len());
                        rest.trim_start_matches(is_js_space).starts_with('(')
                    }))
        {
            style = Some("def");
        }

        if let Some(result) = self.token_hook(stream, state, style) {
            style = Some(result);
        }

        if style == Some("def") && !cfg.style_defs {
            style = Some("variable");
        }

        state.start_of_line = false;
        state.prev_token = if cur.is_def_keyword {
            Some("def")
        } else {
            style.or(punc)
        };
        self.maybe_eol(stream, state);
        style.map(str::to_string)
    }
}

// ---- the `def(...)` configs -------------------------------------------

const C_KEYWORDS: &str = "auto if break case register continue return default do sizeof \
static else struct switch extern typedef union for goto while enum const \
volatile inline restrict asm fortran";

// Keywords from https://en.cppreference.com/w/cpp/keyword includes C++20.
const CPP_KEYWORDS: &str = "alignas alignof and and_eq audit axiom bitand bitor catch \
class compl concept constexpr const_cast decltype delete dynamic_cast \
explicit export final friend import module mutable namespace new noexcept \
not not_eq operator or or_eq override private protected public \
reinterpret_cast requires static_assert static_cast template this \
thread_local throw try typeid typename using virtual xor xor_eq";

const OBJC_KEYWORDS: &str = "bycopy byref in inout oneway out self super atomic nonatomic retain copy \
readwrite readonly strong weak assign typeof nullable nonnull null_resettable _cmd \
@interface @implementation @end @protocol @encode @property @synthesize @dynamic @class \
@public @package @private @protected @required @optional @try @catch @finally @import \
@selector @encode @defs @synchronized @autoreleasepool @compatibility_alias @available";

const OBJC_BUILTINS: &str = "FOUNDATION_EXPORT FOUNDATION_EXTERN NS_INLINE NS_FORMAT_FUNCTION \
 NS_RETURNS_RETAINEDNS_ERROR_ENUM NS_RETURNS_NOT_RETAINED NS_RETURNS_INNER_POINTER \
NS_DESIGNATED_INITIALIZER NS_ENUM NS_OPTIONS NS_REQUIRES_NIL_TERMINATION \
NS_ASSUME_NONNULL_BEGIN NS_ASSUME_NONNULL_END NS_SWIFT_NAME NS_REFINED_FOR_SWIFT";

const C_BLOCK_KEYWORDS: &str = "case do else for if switch while struct enum union";
const C_DEF_KEYWORDS: &str = "struct enum union";

fn c_hooks(ch: char) -> Option<Hook> {
    match ch {
        '#' => Some(Hook::Cpp),
        '*' => Some(Hook::Pointer),
        _ => None,
    }
}

fn cpp_hooks(ch: char) -> Option<Hook> {
    match ch {
        '#' => Some(Hook::Cpp),
        '*' => Some(Hook::Pointer),
        'u' | 'U' | 'L' | 'R' => Some(Hook::Cpp11String),
        '0'..='9' => Some(Hook::Cpp14Literal),
        _ => None,
    }
}

fn preprocessor_hooks(ch: char) -> Option<Hook> {
    (ch == '#').then_some(Hook::Cpp)
}

/// `text/x-csrc`, `text/x-c`, `text/x-chdr`
fn c_config() -> Config {
    Config {
        keywords: Words::of(&[C_KEYWORDS]),
        types: Types::C,
        block_keywords: Words::of(&[C_BLOCK_KEYWORDS]),
        def_keywords: Words::of(&[C_DEF_KEYWORDS]),
        type_first_definitions: true,
        atoms: Words::of(&["NULL true false"]),
        is_reserved_identifier: Some(c_is_reserved_identifier),
        hooks: c_hooks,
        ..Config::default()
    }
}

/// `text/x-c++src`, `text/x-c++hdr`
fn cpp_config() -> Config {
    Config {
        keywords: Words::of(&[C_KEYWORDS, CPP_KEYWORDS]),
        types: Types::C,
        block_keywords: Words::of(&[C_BLOCK_KEYWORDS, "class try catch"]),
        def_keywords: Words::of(&[C_DEF_KEYWORDS, "class namespace"]),
        type_first_definitions: true,
        atoms: Words::of(&["true false NULL nullptr"]),
        is_identifier_char: |c| is_identifier_char(c) || c == '~',
        is_reserved_identifier: Some(c_is_reserved_identifier),
        hooks: cpp_hooks,
        token_hook: Some(TokenHook::Cpp),
        namespace_separator: Some("::"),
        ..Config::default()
    }
}

/// `text/x-java`
fn java_config() -> Config {
    Config {
        keywords: Words::of(&[
            "abstract assert break case catch class const continue default \
do else enum extends final finally for goto if implements import \
instanceof interface native new package private protected public \
return static strictfp super switch synchronized this throw throws transient \
try volatile while @interface",
        ]),
        types: Types::Words(Words::of(&[
            "var byte short int long float double boolean char void Boolean Byte Character Double Float \
Integer Long Number Object Short String StringBuffer StringBuilder Void",
        ])),
        block_keywords: Words::of(&["catch class do else finally for if switch try while"]),
        def_keywords: Words::of(&["class interface enum @interface"]),
        type_first_definitions: true,
        atoms: Words::of(&["true false null"]),
        number: NumberRe::Java,
        hooks: |ch| match ch {
            '@' => Some(Hook::JavaAt),
            '"' => Some(Hook::JavaQuote),
            _ => None,
        },
        ..Config::default()
    }
}

/// `text/x-csharp`
fn csharp_config() -> Config {
    Config {
        keywords: Words::of(&[
            "abstract as async await base break case catch checked class const continue \
default delegate do else enum event explicit extern finally fixed for \
foreach goto if implicit in init interface internal is lock namespace new \
operator out override params private protected public readonly record ref required return sealed \
sizeof stackalloc static struct switch this throw try typeof unchecked \
unsafe using virtual void volatile while add alias ascending descending dynamic from get \
global group into join let orderby partial remove select set value var yield",
        ]),
        types: Types::Words(Words::of(&[
            "Action Boolean Byte Char DateTime DateTimeOffset Decimal Double Func \
Guid Int16 Int32 Int64 Object SByte Single String Task TimeSpan UInt16 UInt32 \
UInt64 bool byte char decimal double short int long object \
sbyte float string ushort uint ulong",
        ])),
        block_keywords: Words::of(&[
            "catch class do else finally for foreach if struct switch try while",
        ]),
        def_keywords: Words::of(&["class interface namespace record struct var"]),
        type_first_definitions: true,
        atoms: Words::of(&["true false null"]),
        hooks: |ch| (ch == '@').then_some(Hook::CSharpAt),
        ..Config::default()
    }
}

/// `text/x-scala`
fn scala_config() -> Config {
    Config {
        keywords: Words::of(&[
            // scala
            "abstract case catch class def do else extends final finally for forSome if \
implicit import lazy match new null object override package private protected return \
sealed super this throw trait try type val var while with yield _ \
assert assume require print println printf readLine readBoolean readByte readShort \
readChar readInt readLong readFloat readDouble",
        ]),
        types: Types::Words(Words::of(&[
            "AnyVal App Application Array BufferedIterator BigDecimal BigInt Char Console Either \
Enumeration Equiv Error Exception Fractional Function IndexedSeq Int Integral Iterable \
Iterator List Map Numeric Nil NotNull Option Ordered Ordering PartialFunction PartialOrdering \
Product Proxy Range Responder Seq Serializable Set Specializable Stream StringBuilder \
StringContext Symbol Throwable Traversable TraversableOnce Tuple Unit Vector \
Boolean Byte Character CharSequence Class ClassLoader Cloneable Comparable \
Compiler Double Exception Float Integer Long Math Number Object Package Pair Process \
Runtime Runnable SecurityManager Short StackTraceElement StrictMath String \
StringBuffer System Thread ThreadGroup ThreadLocal Throwable Triple Void",
        ])),
        multi_line_strings: true,
        block_keywords: Words::of(&[
            "catch class enum do else finally for forSome if match switch try while",
        ]),
        def_keywords: Words::of(&["class enum def object package trait type val var"]),
        atoms: Words::of(&["true false null"]),
        indent_statements: false,
        is_operator_char: |c| "+-*&%=<>!?|/#:@".contains(c),
        hooks: |ch| match ch {
            '@' => Some(Hook::MetaAt),
            '"' => Some(Hook::ScalaQuote),
            '\'' => Some(Hook::ScalaApos),
            '=' => Some(Hook::ScalaEq),
            '/' => Some(Hook::NestedComment),
            _ => None,
        },
        ..Config::default()
    }
}

/// `text/x-kotlin`
fn kotlin_config() -> Config {
    Config {
        keywords: Words::of(&[
            // keywords
            "package as typealias class interface this super val operator \
var fun for is in This throw return annotation \
break continue object if else while do try when !in !is as? \
file import where by get set abstract enum open inner override private public internal \
protected catch finally out final vararg reified dynamic companion constructor init \
sealed field property receiver param sparam lateinit data inline noinline tailrec \
external annotation crossinline const operator infix suspend actual expect setparam value",
        ]),
        types: Types::Words(Words::of(&[
            "Boolean Byte Character CharSequence Class ClassLoader Cloneable Comparable \
Compiler Double Exception Float Integer Long Math Number Object Package Pair Process \
Runtime Runnable SecurityManager Short StackTraceElement StrictMath String \
StringBuffer System Thread ThreadGroup ThreadLocal Throwable Triple Void Annotation Any BooleanArray \
ByteArray Char CharArray DeprecationLevel DoubleArray Enum FloatArray Function Int IntArray Lazy \
LazyThreadSafetyMode LongArray Nothing ShortArray Unit",
        ])),
        indent_statements: false,
        multi_line_strings: true,
        number: NumberRe::Kotlin,
        block_keywords: Words::of(&["catch class do else finally for if where try while enum"]),
        def_keywords: Words::of(&["class val var object interface fun"]),
        atoms: Words::of(&["true false null this"]),
        hooks: |ch| match ch {
            '@' => Some(Hook::MetaAt),
            '*' => Some(Hook::KotlinStar),
            '"' => Some(Hook::KotlinQuote),
            '/' => Some(Hook::NestedComment),
            _ => None,
        },
        ..Config::default()
    }
}

/// `x-shader/x-vertex`, `x-shader/x-fragment`
fn shader_config() -> Config {
    Config {
        keywords: Words::of(&["sampler1D sampler2D sampler3D samplerCube \
sampler1DShadow sampler2DShadow \
const attribute uniform varying \
break continue discard return \
for while do if else struct \
in out inout"]),
        types: Types::Words(Words::of(&["float int bool void \
vec2 vec3 vec4 ivec2 ivec3 ivec4 bvec2 bvec3 bvec4 \
mat2 mat3 mat4"])),
        block_keywords: Words::of(&["for while do if else struct"]),
        builtin: Words::of(&["radians degrees sin cos tan asin acos atan \
pow exp log exp2 sqrt inversesqrt \
abs sign floor ceil fract mod min max clamp mix step smoothstep \
length distance dot cross normalize ftransform faceforward \
reflect refract matrixCompMult \
lessThan lessThanEqual greaterThan greaterThanEqual \
equal notEqual any all not \
texture1D texture1DProj texture1DLod texture1DProjLod \
texture2D texture2DProj texture2DLod texture2DProjLod \
texture3D texture3DProj texture3DLod texture3DProjLod \
textureCube textureCubeLod \
shadow1D shadow2D shadow1DProj shadow2DProj \
shadow1DLod shadow2DLod shadow1DProjLod shadow2DProjLod \
dFdx dFdy fwidth \
noise1 noise2 noise3 noise4"]),
        atoms: Words::of(&["true false \
gl_FragColor gl_SecondaryColor gl_Normal gl_Vertex \
gl_MultiTexCoord0 gl_MultiTexCoord1 gl_MultiTexCoord2 gl_MultiTexCoord3 \
gl_MultiTexCoord4 gl_MultiTexCoord5 gl_MultiTexCoord6 gl_MultiTexCoord7 \
gl_FogCoord gl_PointCoord \
gl_Position gl_PointSize gl_ClipVertex \
gl_FrontColor gl_BackColor gl_FrontSecondaryColor gl_BackSecondaryColor \
gl_TexCoord gl_FogFragCoord \
gl_FragCoord gl_FrontFacing \
gl_FragData gl_FragDepth \
gl_ModelViewMatrix gl_ProjectionMatrix gl_ModelViewProjectionMatrix \
gl_TextureMatrix gl_NormalMatrix gl_ModelViewMatrixInverse \
gl_ProjectionMatrixInverse gl_ModelViewProjectionMatrixInverse \
gl_TextureMatrixTranspose gl_ModelViewMatrixInverseTranspose \
gl_ProjectionMatrixInverseTranspose \
gl_ModelViewProjectionMatrixInverseTranspose \
gl_TextureMatrixInverseTranspose \
gl_NormalScale gl_DepthRange gl_ClipPlane \
gl_Point gl_FrontMaterial gl_BackMaterial gl_LightSource gl_LightModel \
gl_FrontLightModelProduct gl_BackLightModelProduct \
gl_TextureColor gl_EyePlaneS gl_EyePlaneT gl_EyePlaneR gl_EyePlaneQ \
gl_FogParameters \
gl_MaxLights gl_MaxClipPlanes gl_MaxTextureUnits gl_MaxTextureCoords \
gl_MaxVertexAttribs gl_MaxVertexUniformComponents gl_MaxVaryingFloats \
gl_MaxVertexTextureImageUnits gl_MaxTextureImageUnits \
gl_MaxFragmentUniformComponents gl_MaxCombineTextureImageUnits \
gl_MaxDrawBuffers"]),
        hooks: preprocessor_hooks,
        ..Config::default()
    }
}

/// `text/x-nesc`
fn nesc_config() -> Config {
    Config {
        keywords: Words::of(&[
            C_KEYWORDS,
            "as atomic async call command component components configuration event generic \
implementation includes interface module new norace nx_struct nx_union post provides \
signal task uses abstract extends",
        ]),
        types: Types::C,
        block_keywords: Words::of(&[C_BLOCK_KEYWORDS]),
        atoms: Words::of(&["null true false"]),
        hooks: preprocessor_hooks,
        ..Config::default()
    }
}

/// `text/x-objectivec`
fn objc_config() -> Config {
    Config {
        keywords: Words::of(&[C_KEYWORDS, OBJC_KEYWORDS]),
        types: Types::ObjC,
        builtin: Words::of(&[OBJC_BUILTINS]),
        block_keywords: Words::of(&[
            C_BLOCK_KEYWORDS,
            "@synthesize @try @catch @finally @autoreleasepool @synchronized",
        ]),
        def_keywords: Words::of(&[
            C_DEF_KEYWORDS,
            "@interface @implementation @protocol @class",
        ]),
        type_first_definitions: true,
        atoms: Words::of(&["YES NO NULL Nil nil true false nullptr"]),
        is_reserved_identifier: Some(c_is_reserved_identifier),
        hooks: c_hooks,
        ..Config::default()
    }
}

/// `text/x-objectivec++`
fn objcpp_config() -> Config {
    Config {
        keywords: Words::of(&[C_KEYWORDS, OBJC_KEYWORDS, CPP_KEYWORDS]),
        types: Types::ObjC,
        builtin: Words::of(&[OBJC_BUILTINS]),
        block_keywords: Words::of(&[
            C_BLOCK_KEYWORDS,
            "@synthesize @try @catch @finally @autoreleasepool @synchronized class try catch",
        ]),
        def_keywords: Words::of(&[
            C_DEF_KEYWORDS,
            "@interface @implementation @protocol @class class namespace",
        ]),
        type_first_definitions: true,
        atoms: Words::of(&["YES NO NULL Nil nil true false nullptr"]),
        is_reserved_identifier: Some(c_is_reserved_identifier),
        hooks: cpp_hooks,
        token_hook: Some(TokenHook::Cpp),
        namespace_separator: Some("::"),
        ..Config::default()
    }
}

/// `text/x-squirrel`
fn squirrel_config() -> Config {
    Config {
        keywords: Words::of(&[
            "base break clone continue const default delete enum extends function in class \
foreach local resume return this throw typeof yield constructor instanceof static",
        ]),
        types: Types::C,
        block_keywords: Words::of(&["case catch class else for foreach if switch try while"]),
        def_keywords: Words::of(&["function local class"]),
        type_first_definitions: true,
        atoms: Words::of(&["true false null"]),
        hooks: preprocessor_hooks,
        ..Config::default()
    }
}

/// `text/x-ceylon`
fn ceylon_config() -> Config {
    Config {
        keywords: Words::of(&[
            "abstracts alias assembly assert assign break case catch class continue dynamic else \
exists extends finally for function given if import in interface is let module new \
nonempty object of out outer package return satisfies super switch then this throw \
try value void while",
        ]),
        types: Types::Ceylon,
        block_keywords: Words::of(&[
            "case catch class dynamic else finally for function if interface module new object switch try while",
        ]),
        def_keywords: Words::of(&["class dynamic function interface module object package value"]),
        builtin: Words::of(&[
            "abstract actual aliased annotation by default deprecated doc final formal late license \
native optional sealed see serializable shared suppressWarnings tagged throws variable",
        ]),
        is_punctuation_char: |c| is_punctuation_char(c) || c == '`',
        is_operator_char: |c| "+-*&%=<>!?|^~:/".contains(c),
        number_start: |c| c.is_ascii_digit() || c == '#' || c == '$',
        number: NumberRe::Ceylon,
        multi_line_strings: true,
        type_first_definitions: true,
        atoms: Words::of(&["true false null larger smaller equal empty finished"]),
        style_defs: false,
        hooks: |ch| match ch {
            '@' => Some(Hook::MetaAt),
            '"' => Some(Hook::CeylonQuote),
            '`' => Some(Hook::CeylonBacktick),
            '\'' => Some(Hook::CeylonApos),
            _ => None,
        },
        token_hook: Some(TokenHook::Ceylon),
        ..Config::default()
    }
}

/// A mode per config, built once.
macro_rules! mode_fn {
    ($(#[$doc:meta])* $name:ident, $config:expr) => {
        $(#[$doc])*
        pub fn $name() -> Arc<dyn Mode> {
            static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
            MODE.get_or_init(|| Arc::new(Clike { cfg: $config() })).clone()
        }
    };
}

mode_fn!(
    /// `text/x-csrc`, `text/x-c`, `text/x-chdr`
    c,
    c_config
);
mode_fn!(
    /// `text/x-c++src`, `text/x-c++hdr`
    cpp,
    cpp_config
);
mode_fn!(
    /// `text/x-java`
    java,
    java_config
);
mode_fn!(
    /// `text/x-csharp`
    csharp,
    csharp_config
);
mode_fn!(
    /// `text/x-scala`
    scala,
    scala_config
);
mode_fn!(
    /// `text/x-kotlin`
    kotlin,
    kotlin_config
);
mode_fn!(
    /// `x-shader/x-vertex`, `x-shader/x-fragment`
    shader,
    shader_config
);
mode_fn!(
    /// `text/x-nesc`
    nesc,
    nesc_config
);
mode_fn!(
    /// `text/x-objectivec`
    objectivec,
    objc_config
);
mode_fn!(
    /// `text/x-objectivec++`
    objectivecpp,
    objcpp_config
);
mode_fn!(
    /// `text/x-squirrel`
    squirrel,
    squirrel_config
);
mode_fn!(
    /// `text/x-ceylon`
    ceylon,
    ceylon_config
);

/// The clike mode for a config built elsewhere (dart.js).
pub(super) fn with_config(cfg: Config) -> Clike {
    Clike { cfg }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_before_matches_the_js_regex() {
        assert!(type_before_text("int *"));
        assert!(type_before_text("int*"));
        assert!(type_before_text("a[] "));
        assert!(type_before_text("vector<int> "));
        assert!(!type_before_text("a -> "));
        assert!(!type_before_text("a > "));
        assert!(!type_before_text("> "));
        assert!(!type_before_text("]"));
        assert!(!type_before_text("int "));
    }

    #[test]
    fn c_types_and_reserved_identifiers() {
        assert!(c_types("size_t"));
        assert!(!c_types("_t"));
        assert!(c_types("x_t"));
        assert!(c_is_reserved_identifier("__attribute__"));
        assert!(c_is_reserved_identifier("_Bool"));
        assert!(!c_is_reserved_identifier("_bool"));
        assert!(!c_is_reserved_identifier("_"));
        assert!(cpp_looks_like_constructor("a::b::b"));
        assert!(cpp_looks_like_constructor("Foo::~Foo"));
        assert!(!cpp_looks_like_constructor("xab::ab"));
    }
}
