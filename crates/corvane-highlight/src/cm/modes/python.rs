//! `codemirror/mode/python/python.js` (`text/x-python`, `text/x-cython`).
//!
//! Ported line by line; the JS function names are kept on the Rust ones.
//! Only the Python 3 flavour exists here: no MIME GHD can pick sets
//! `version: 2`.
//!
//! GHD's highlighter calls `getMode({}, mime)`, so `conf.indentUnit` (and
//! with it `hangingIndent`) is `undefined` and every scope pushed after the
//! root one gets a `NaN` offset. Comparisons against `NaN` are false in JS
//! and in Rust alike, which is why the scope offsets are kept as `f64` and
//! [`INDENT_UNIT`] is `NaN`: the dedent checks then behave (never fire) the
//! way they do in GHD, while the scope *types* still decide bracket errors.

use std::borrow::Cow;
use std::collections::HashSet;

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

/// `conf.indentUnit` as GHD passes it: `undefined`, i.e. `NaN` in arithmetic.
const INDENT_UNIT: f64 = f64::NAN;
const ERRORCLASS: &str = "error";

const COMMON_KEYWORDS: &[&str] = &[
    "as", "assert", "break", "class", "continue", "def", "del", "elif", "else", "except",
    "finally", "for", "from", "global", "if", "import", "lambda", "pass", "raise", "return", "try",
    "while", "with", "yield", "in", "False", "True",
];
const COMMON_BUILTINS: &[&str] = &[
    "abs",
    "all",
    "any",
    "bin",
    "bool",
    "bytearray",
    "callable",
    "chr",
    "classmethod",
    "compile",
    "complex",
    "delattr",
    "dict",
    "dir",
    "divmod",
    "enumerate",
    "eval",
    "filter",
    "float",
    "format",
    "frozenset",
    "getattr",
    "globals",
    "hasattr",
    "hash",
    "help",
    "hex",
    "id",
    "input",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "list",
    "locals",
    "map",
    "max",
    "memoryview",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "ord",
    "pow",
    "property",
    "range",
    "repr",
    "reversed",
    "round",
    "set",
    "setattr",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "vars",
    "zip",
    "__import__",
    "NotImplemented",
    "Ellipsis",
    "__debug__",
];
const PY3_KEYWORDS: &[&str] = &[
    "nonlocal",
    "None",
    "aiter",
    "anext",
    "async",
    "await",
    "breakpoint",
    "match",
    "case",
];
const PY3_BUILTINS: &[&str] = &["ascii", "bytes", "exec", "print"];
const WORD_OPERATORS: &[&str] = &["and", "or", "not", "is"];
/// `text/x-cython`'s `extra_keywords`.
const CYTHON_KEYWORDS: &[&str] = &[
    "by", "cdef", "cimport", "cpdef", "ctypedef", "enum", "except", "extern", "gil", "include",
    "nogil", "property", "public", "readonly", "struct", "union", "DEF", "IF", "ELIF", "ELSE",
];

/// The python mode with a MIME's `parserConf`.
pub struct Python {
    keywords: HashSet<&'static str>,
    builtins: HashSet<&'static str>,
}

impl Python {
    /// `text/x-python`
    pub fn new() -> Self {
        Self::with_extra_keywords(&[])
    }

    /// `text/x-cython`
    pub fn cython() -> Self {
        Self::with_extra_keywords(CYTHON_KEYWORDS)
    }

    fn with_extra_keywords(extra: &[&'static str]) -> Self {
        let keywords = COMMON_KEYWORDS
            .iter()
            .chain(extra)
            .chain(PY3_KEYWORDS)
            .copied()
            .collect();
        let builtins = COMMON_BUILTINS
            .iter()
            .chain(PY3_BUILTINS)
            .copied()
            .collect();
        Self { keywords, builtins }
    }
}

impl Default for Python {
    fn default() -> Self {
        Self::new()
    }
}

type Style = Option<Cow<'static, str>>;

fn s(style: &'static str) -> Style {
    Some(Cow::Borrowed(style))
}

/// `tokenStringFactory` / `formatStringFactory`'s `tokenString` closure.
#[derive(Clone)]
struct StrTok {
    /// the quote(s), prefix stripped
    delimiter: &'static str,
    format: bool,
    outer: Tokenize,
}

/// `state.tokenize`
#[derive(Clone)]
enum Tokenize {
    Base,
    Str(Box<StrTok>),
    /// `tokenNestedExpr(depth)` inside an f-string, returning to `string`.
    Nested {
        depth: u32,
        string: Box<StrTok>,
    },
}

const PY: u8 = b'p';

#[derive(Clone, Copy)]
struct Scope {
    offset: f64,
    /// `"py"` ([`PY`]) or the closing bracket
    kind: u8,
}

/// The parts of `state.lastToken` the mode ever compares against.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LastToken {
    Null,
    Backslash,
    Dot,
    Def,
    Class,
    Meta,
    Other,
}

#[derive(Clone)]
struct PyState {
    tokenize: Tokenize,
    scopes: Vec<Scope>,
    indent: f64,
    last_token: LastToken,
    lambda: bool,
    dedent: bool,
    beginning_of_line: bool,
    error_token: bool,
}

fn top(state: &PyState) -> Scope {
    state.scopes.last().copied().unwrap_or(Scope {
        offset: 0.0,
        kind: PY,
    })
}

/// JS `\s`.
fn is_js_space(c: char) -> bool {
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

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic() || c >= '\u{a1}'
}

fn is_ident_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric() || c >= '\u{a1}'
}

/// `stream.match(identifiers, consume)`:
/// `/^[_A-Za-z¡-￿][_A-Za-z0-9¡-￿]*/`.
fn match_identifier(stream: &mut StringStream, consume: bool) -> bool {
    match stream.peek() {
        Some(c) if is_ident_start(c) => {
            if consume {
                stream.next();
                stream.eat_while_if(is_ident_char);
            }
            true
        }
        _ => false,
    }
}

/// `stream.match(wordRegexp(words))`: `/^((w1)|(w2)|…)\b/` with JS's ASCII
/// `\b`. Every word is made of ASCII word chars, so a match is exactly the
/// run of ASCII word chars at `pos` being one of the words.
fn match_word(stream: &mut StringStream, words: impl Fn(&str) -> bool) -> bool {
    let mut end = stream.pos;
    while stream
        .char_at(end)
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        end += 1;
    }
    if end > stream.pos && words(stream.slice(stream.pos, end)) {
        stream.pos = end;
        true
    } else {
        false
    }
}

/// `stream.match(re)` without building the match.
fn match_re(stream: &mut StringStream, re: &fancy_regex::Regex) -> bool {
    let rest = stream.slice(stream.pos, stream.len());
    match re.find(rest) {
        Ok(Some(m)) if m.start() == 0 => {
            let n = rest[..m.end()].chars().count();
            stream.pos += n;
            true
        }
        _ => false,
    }
}

impl Python {
    // tokenBase
    fn token_base(&self, stream: &mut StringStream, state: &mut PyState) -> Style {
        let sol = stream.sol() && state.last_token != LastToken::Backslash;
        if sol {
            state.indent = stream.indentation() as f64;
        }
        // Handle scope changes
        if sol && top(state).kind == PY {
            let scope_offset = top(state).offset;
            if stream.eat_space() {
                let line_offset = stream.indentation() as f64;
                if line_offset > scope_offset {
                    push_py_scope(state);
                } else if line_offset < scope_offset
                    && dedent(stream, state)
                    && stream.peek() != Some('#')
                {
                    state.error_token = true;
                }
                return None;
            } else {
                let style = self.token_base_inner(stream, state, false);
                if scope_offset > 0.0 && dedent(stream, state) {
                    return Some(Cow::Owned(format!(
                        "{} {ERRORCLASS}",
                        style.as_deref().unwrap_or("null")
                    )));
                }
                return style;
            }
        }
        self.token_base_inner(stream, state, false)
    }

    // tokenBaseInner
    fn token_base_inner(
        &self,
        stream: &mut StringStream,
        state: &mut PyState,
        in_format: bool,
    ) -> Style {
        if stream.eat_space() {
            return None;
        }

        // Handle Comments
        if !in_format && stream.peek() == Some('#') {
            // `match(/^#.*/)`
            stream.skip_js_dots();
            return s("comment");
        }

        // Handle Number Literals
        if stream
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || c == '.')
        {
            let mut float_literal = false;
            // Floats
            if match_re(stream, re!(r"(?i)^[0-9_]*\.[0-9]+(e[+\-]?[0-9]+)?")) {
                float_literal = true;
            }
            if match_re(stream, re!(r"^[0-9_]+\.[0-9]*")) {
                float_literal = true;
            }
            if match_re(stream, re!(r"^\.[0-9]+")) {
                float_literal = true;
            }
            if float_literal {
                // Float literals may be "imaginary"
                stream.eat_if(|c| c == 'j' || c == 'J');
                return s("number");
            }
            // Integers
            let mut int_literal = false;
            // Hex
            if match_re(stream, re!(r"(?i)^0x[0-9a-f_]+")) {
                int_literal = true;
            }
            // Binary
            if match_re(stream, re!(r"(?i)^0b[01_]+")) {
                int_literal = true;
            }
            // Octal
            if match_re(stream, re!(r"(?i)^0o[0-7_]+")) {
                int_literal = true;
            }
            // Decimal
            if match_re(stream, re!(r"^[1-9][0-9_]*(e[+\-]?[0-9_]+)?")) {
                // Decimal literals may be "imaginary"
                stream.eat_if(|c| c == 'j' || c == 'J');
                int_literal = true;
            }
            // Zero by itself with no other piece of number.
            if match_re(stream, re!(r"(?i)^0(?![0-9x])")) {
                int_literal = true;
            }
            if int_literal {
                // Integer literals may be "long"
                stream.eat_if(|c| c == 'l' || c == 'L');
                return s("number");
            }
        }

        // Handle Strings
        if match_re(
            stream,
            re!(r#"(?i)^(([rbuf]|(br)|(rb)|(fr)|(rf))?('{3}|"{3}|['"]))"#),
        ) {
            let current = stream.current();
            let format = current.chars().any(|c| c == 'f' || c == 'F');
            let quotes =
                current.trim_start_matches(|c: char| "rubf".contains(c.to_ascii_lowercase()));
            let delimiter = match quotes {
                "'''" => "'''",
                "\"\"\"" => "\"\"\"",
                "'" => "'",
                _ => "\"",
            };
            // tokenStringFactory / formatStringFactory
            let outer = std::mem::replace(&mut state.tokenize, Tokenize::Base);
            state.tokenize = Tokenize::Str(Box::new(StrTok {
                delimiter,
                format,
                outer,
            }));
            return self.token_string(stream, state);
        }

        // operators
        if match_re(
            stream,
            re!(r"^([-+*/%&|\^]=?|[<>=]+|//=?|\*\*=?|!=|[~!@]|\.\.\.)"),
        ) {
            return s("operator");
        }

        // delimiters: /^[\(\)\[\]\{\}@,:`=;\.\\]/
        if stream.eat_if(|c| "()[]{}@,:`=;.\\".contains(c)).is_some() {
            return s("punctuation");
        }

        if state.last_token == LastToken::Dot && match_identifier(stream, true) {
            return s("property");
        }

        if match_word(stream, |w| self.keywords.contains(w))
            || match_word(stream, |w| WORD_OPERATORS.contains(&w))
        {
            return s("keyword");
        }

        if match_word(stream, |w| self.builtins.contains(w)) {
            return s("builtin");
        }

        if match_word(stream, |w| w == "self" || w == "cls") {
            return s("variable-2");
        }

        if match_identifier(stream, true) {
            if matches!(state.last_token, LastToken::Def | LastToken::Class) {
                return s("def");
            }
            return s("variable");
        }

        // Handle non-detected items
        stream.next();
        if in_format { None } else { s(ERRORCLASS) }
    }

    /// Both factories' `tokenString`, and `tokenNestedExpr`.
    fn token_string(&self, stream: &mut StringStream, state: &mut PyState) -> Style {
        let (delimiter, format) = match &state.tokenize {
            Tokenize::Str(t) => (t.delimiter, t.format),
            _ => return None,
        };
        let singleline = delimiter.len() == 1;
        let out = "string";
        if format {
            // formatStringFactory's tokenString
            while !stream.eol() {
                stream.eat_while_if(|c| !matches!(c, '\'' | '"' | '{' | '}' | '\\'));
                if stream.eat('\\').is_some() {
                    stream.next();
                    if singleline && stream.eol() {
                        return s(out);
                    }
                } else if stream.match_str(delimiter, true, false) {
                    to_outer(state);
                    return s(out);
                } else if stream.match_str("{{", true, false) {
                    // ignore {{ in f-str
                    return s(out);
                } else if stream.match_str("{", false, false) {
                    // switch to nested mode
                    if let Tokenize::Str(string) =
                        std::mem::replace(&mut state.tokenize, Tokenize::Base)
                    {
                        state.tokenize = Tokenize::Nested { depth: 0, string };
                    }
                    if !stream.current().is_empty() {
                        return s(out);
                    }
                    return self.token_nested_expr(stream, state);
                } else if stream.match_str("}}", true, false) {
                    return s(out);
                } else if stream.match_str("}", true, false) {
                    // single } in f-string is an error
                    return s(ERRORCLASS);
                } else {
                    stream.eat_if(|c| c == '\'' || c == '"');
                }
            }
        } else {
            // tokenStringFactory's tokenString
            while !stream.eol() {
                stream.eat_while_if(|c| !matches!(c, '\'' | '"' | '\\'));
                if stream.eat('\\').is_some() {
                    stream.next();
                    if singleline && stream.eol() {
                        return s(out);
                    }
                } else if stream.match_str(delimiter, true, false) {
                    to_outer(state);
                    return s(out);
                } else {
                    stream.eat_if(|c| c == '\'' || c == '"');
                }
            }
        }
        if singleline {
            // parserConf.singleLineStringErrors is never set
            to_outer(state);
        }
        s(out)
    }

    // tokenNestedExpr(depth)
    fn token_nested_expr(&self, stream: &mut StringStream, state: &mut PyState) -> Style {
        let inner = self.token_base_inner(stream, state, true);
        if inner.as_deref() == Some("punctuation") {
            let current = stream.current();
            if current == "{" {
                if let Tokenize::Nested { depth, .. } = &mut state.tokenize {
                    *depth += 1;
                }
            } else if current == "}" {
                match std::mem::replace(&mut state.tokenize, Tokenize::Base) {
                    Tokenize::Nested { depth, string } if depth > 1 => {
                        state.tokenize = Tokenize::Nested {
                            depth: depth - 1,
                            string,
                        };
                    }
                    Tokenize::Nested { string, .. } => state.tokenize = Tokenize::Str(string),
                    other => state.tokenize = other,
                }
            }
        }
        inner
    }

    /// `state.tokenize(stream, state)`
    fn call_tokenize(&self, stream: &mut StringStream, state: &mut PyState) -> Style {
        match state.tokenize {
            Tokenize::Base => self.token_base(stream, state),
            Tokenize::Str(_) => self.token_string(stream, state),
            Tokenize::Nested { .. } => self.token_nested_expr(stream, state),
        }
    }

    // tokenLexer
    fn token_lexer(&self, stream: &mut StringStream, state: &mut PyState) -> Style {
        if stream.sol() {
            state.beginning_of_line = true;
            state.dedent = false;
        }

        let mut style = self.call_tokenize(stream, state);
        let current = stream.current();
        let is_at = current == "@";
        let non_space = current.chars().any(|c| !is_js_space(c));
        let pass_or_return = current == "pass" || current == "return";
        let is_lambda = current == "lambda";
        let is_colon = current == ":";
        let mut chars = current.chars();
        let single = match (chars.next(), chars.next()) {
            (Some(c), None) => Some(c),
            _ => None,
        };

        // Handle decorators
        if state.beginning_of_line && is_at {
            return if match_identifier(stream, false) {
                s("meta")
            } else {
                // py3
                s("operator")
            };
        }

        if non_space {
            state.beginning_of_line = false;
        }

        if matches!(style.as_deref(), Some("variable" | "builtin"))
            && state.last_token == LastToken::Meta
        {
            style = s("meta");
        }

        // Handle scope changes.
        if pass_or_return {
            state.dedent = true;
        }

        if is_lambda {
            state.lambda = true;
        }
        if is_colon && !state.lambda && top(state).kind == PY && rest_is_blank_or_comment(stream) {
            push_py_scope(state);
        }

        if let Some(c) = single
            && !style
                .as_deref()
                .is_some_and(|st| st.contains("string") || st.contains("comment"))
        {
            if let Some(i) = "[({".find(c) {
                push_bracket_scope(state, b"])}"[i]);
            }
            if matches!(c, ']' | ')' | '}') {
                if top(state).kind == c as u8 {
                    let popped = state.scopes.pop().map_or(f64::NAN, |sc| sc.offset);
                    state.indent = popped - INDENT_UNIT;
                } else {
                    return s(ERRORCLASS);
                }
            }
        }
        if state.dedent && stream.eol() && top(state).kind == PY && state.scopes.len() > 1 {
            state.scopes.pop();
        }

        style
    }
}

/// `state.tokenize = tokenOuter`
fn to_outer(state: &mut PyState) {
    if let Tokenize::Str(t) = std::mem::replace(&mut state.tokenize, Tokenize::Base) {
        state.tokenize = t.outer;
    }
}

/// `stream.match(/^\s*(?:#|$)/, false)`
fn rest_is_blank_or_comment(stream: &StringStream) -> bool {
    let mut i = stream.pos;
    while stream.char_at(i).is_some_and(is_js_space) {
        i += 1;
    }
    matches!(stream.char_at(i), None | Some('#'))
}

// pushPyScope
fn push_py_scope(state: &mut PyState) {
    while top(state).kind != PY {
        state.scopes.pop();
    }
    state.scopes.push(Scope {
        offset: top(state).offset + INDENT_UNIT,
        kind: PY,
    });
}

// pushBracketScope (`align` only feeds `indent`, which the highlighter never
// asks for, so the `stream.column()` it needs is skipped)
fn push_bracket_scope(state: &mut PyState, kind: u8) {
    state.scopes.push(Scope {
        offset: state.indent + INDENT_UNIT,
        kind,
    });
}

// dedent
fn dedent(stream: &StringStream, state: &mut PyState) -> bool {
    let indented = stream.indentation() as f64;
    while state.scopes.len() > 1 && top(state).offset > indented {
        if top(state).kind != PY {
            return true;
        }
        state.scopes.pop();
    }
    top(state).offset != indented
}

impl Mode for Python {
    fn name(&self) -> &'static str {
        "python"
    }

    // startState(basecolumn) - the highlighter passes no basecolumn
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PyState {
            tokenize: Tokenize::Base,
            scopes: vec![Scope {
                offset: 0.0,
                kind: PY,
            }],
            indent: 0.0,
            last_token: LastToken::Null,
            lambda: false,
            dedent: false,
            beginning_of_line: false,
            error_token: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<PyState>(st);
        let add_err = state.error_token;
        if add_err {
            state.error_token = false;
        }
        let mut style = self.token_lexer(stream, state);

        if let Some(st) = style.as_deref()
            && st != "comment"
        {
            state.last_token = if st == "keyword" || st == "punctuation" {
                match stream.current() {
                    "def" => LastToken::Def,
                    "class" => LastToken::Class,
                    "." => LastToken::Dot,
                    "\\" => LastToken::Backslash,
                    _ => LastToken::Other,
                }
            } else {
                match st {
                    "def" => LastToken::Def,
                    "meta" => LastToken::Meta,
                    _ => LastToken::Other,
                }
            };
        }
        if style.as_deref() == Some("punctuation") {
            style = None;
        }

        if stream.eol() && state.lambda {
            state.lambda = false;
        }
        if add_err {
            return Some(format!(
                "{} {ERRORCLASS}",
                style.as_deref().unwrap_or("null")
            ));
        }
        style.map(Cow::into_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cm::run;

    #[test]
    fn cython_adds_its_keywords() {
        let lines = ["cdef int x"];
        let py = run(&Python::new(), &lines, 4);
        let cy = run(&Python::cython(), &lines, 4);
        assert_eq!(py[0][0].2, "m-python variable");
        assert_eq!(cy[0][0].2, "m-python keyword");
    }
}
