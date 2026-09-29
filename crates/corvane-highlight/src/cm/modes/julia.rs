//! `codemirror/mode/julia/julia.js` (`text/x-julia`).
//!
//! Ported line by line; the JS function names are kept on the Rust ones.
//! `state.scopes` only feeds `indent` (the pushes and pops never change a
//! token), so it is left out; the `nestedArrays` / `nestedGenerators`
//! counters that do change tokens are kept. Of `state.lastToken` only the
//! words the mode compares it with are kept.
//!
//! Regex notes: every `stream.match(/re/)` only accepts a match at the
//! stream position, so each regex is anchored with `^`. `wordRegexp`'s JS
//! `\b` after a word becomes `(?![A-Za-z0-9_])`; JS `\w`/`\d` are spelled
//! ASCII; `￿` range ends become `\x{10FFFF}` (JS matches both halves
//! of an astral char there). The `in`/`isa` operator's lookahead is
//! `(?!\.?\()` in the source, but inside a JS string literal `\.` is a plain
//! `.`, so it really reads `(?!.?\()` with `.` any BMP char but a line
//! terminator.

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

/// JS `.` (no `s` flag): anything but a line terminator.
macro_rules! dot {
    () => {
        r"[^\n\r\x{2028}\x{2029}]"
    };
}

/// `asciiOperatorsList` joined for `wordRegexp(list, "", pre)`.
macro_rules! ascii_operators {
    () => {
        r"(?:[<>]:|[<>=]=|<<=?|>>>?=?|=>|--?>|<--[\->]?|//|\.{2,3}|[.\\%*+\-<>!/^|&]=?|\?|\$|~|:)"
    };
}

/// A `tokenString`'s delimiter.
#[derive(Clone, Copy, PartialEq)]
enum Delimiter {
    Triple,
    Quote,
    Backtick,
}

impl Delimiter {
    fn as_str(self) -> &'static str {
        match self {
            Delimiter::Triple => "\"\"\"",
            Delimiter::Quote => "\"",
            Delimiter::Backtick => "`",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    Base,
    Comment,
    Annotation,
    Char,
    String(Delimiter),
}

/// The `state.lastToken` values the mode tests.
#[derive(Clone, Copy, PartialEq)]
enum LastToken {
    Other,
    End,
    Function,
    Macro,
    Type,
    Struct,
    Immutable,
}

#[derive(Clone)]
struct JuliaState {
    tokenize: Tokenize,
    last_token: LastToken,
    leaving_expr: bool,
    is_definition: bool,
    nested_arrays: i32,
    nested_comments: i32,
    nested_generators: i32,
    nested_parameters: i32,
}

pub struct Julia;

/// `tokenBase`
fn token_base(stream: &mut StringStream, state: &mut JuliaState) -> Option<&'static str> {
    // Handle multiline comments
    if stream.match_str("#=", false, false) {
        state.tokenize = Tokenize::Comment;
        return token_comment(stream, state);
    }

    // Handle scope changes
    let mut leaving_expr = state.leaving_expr;
    if stream.sol() {
        leaving_expr = false;
    }
    state.leaving_expr = false;

    if leaving_expr && stream.matches(re!(r"^'+")) {
        return Some("operator");
    }

    if stream.matches(re!(r"^\.{4,}")) {
        return Some("error");
    } else if stream.matches(re!(r"^\.{1,3}")) {
        return Some("operator");
    }

    if stream.eat_space() {
        return None;
    }

    let ch = stream.peek();

    // Handle single line comments
    if ch == Some('#') {
        stream.skip_to_end();
        return Some("comment");
    }

    if ch == Some('[') {
        state.nested_arrays += 1;
    }
    if ch == Some('(') {
        state.nested_generators += 1;
    }
    if state.nested_arrays > 0 && ch == Some(']') {
        state.nested_arrays -= 1;
        state.leaving_expr = true;
    }
    if state.nested_generators > 0 && ch == Some(')') {
        state.nested_generators -= 1;
        state.leaving_expr = true;
    }

    if state.nested_arrays > 0 {
        if state.last_token == LastToken::End && stream.match_str(":", true, false) {
            return Some("operator");
        }
        if stream.match_str("end", true, false) {
            return Some("number");
        }
    }

    // Handle type annotations
    if stream.matches(re!(r"^::(?![:$])")) {
        state.tokenize = Tokenize::Annotation;
        return token_annotation(stream, state);
    }

    // Handle symbols
    if !leaving_expr
        && (stream.matches(re!(
            r"^:[_A-Za-z\x{A1}-\x{10FFFF}][A-Za-z0-9_\x{A1}-\x{10FFFF}]*!*"
        )) || stream.matches(re!(concat!("^:", ascii_operators!()))))
    {
        return Some("builtin");
    }

    // Handle operators and Delimiters
    if stream.matches(re!(concat!(
        r"^(?:[<>]:|[<>=]=|[!=]==|<<=?|>>>?=?|=>?|--?>|<--[\->]?|//",
        r"|[\\%*+\-<>!/^|&\x{F7}\x{22BB}]=?|\?|\$|~|:",
        r"|\x{D7}|\x{2208}|\x{2209}|\x{220B}|\x{220C}|\x{2218}|\x{221A}|\x{221B}|\x{2229}",
        r"|\x{222A}|\x{2260}|\x{2264}|\x{2265}|\x{2286}|\x{2288}|\x{228A}|\x{22C5}",
        r"|(?:in|isa)(?![A-Za-z0-9_])(?![^\n\r\x{2028}\x{2029}\x{10000}-\x{10FFFF}]?\())"
    ))) {
        return Some("operator");
    }

    // Handle Number Literals
    if stream.match_re(re!(r"^\.?[0-9]"), false).is_some() {
        let mut number_literal = false;
        if stream.matches(re!(r"(?i)^0x\.[0-9a-f_]+p[+\-]?[_0-9]+")) {
            number_literal = true;
        }
        // Integers
        if stream.matches(re!(r"(?i)^0x[0-9a-f_]+")) {
            number_literal = true; // Hex
        }
        if stream.matches(re!(r"(?i)^0b[01_]+")) {
            number_literal = true; // Binary
        }
        if stream.matches(re!(r"(?i)^0o[0-7_]+")) {
            number_literal = true; // Octal
        }
        // Floats (the regex also matches the empty string)
        if stream.matches(re!(
            r"(?i)^(?:(?:[0-9][_0-9]*)?\.(?!\.)(?:[0-9][_0-9]*)?|[0-9][_0-9]*\.(?!\.)(?:[0-9][_0-9]*))?(?:[ef][+\-]?[_0-9]+)?"
        )) {
            number_literal = true;
        }
        if stream.matches(re!(r"(?i)^[0-9][_0-9]*(?:e[+\-]?[0-9]+)?")) {
            number_literal = true; // Decimal
        }
        if number_literal {
            // Integer literals may be "long"
            stream.matches(re!(r"^im(?![A-Za-z0-9_])"));
            state.leaving_expr = true;
            return Some("number");
        }
    }

    // Handle Chars
    if stream.match_str("'", true, false) {
        state.tokenize = Tokenize::Char;
        return token_char(stream, state);
    }

    // Handle Strings
    if stream.matches(re!(r#"^(?:`|[_A-Za-z\x{A1}-\x{10FFFF}]*"(?:"")?)"#)) {
        let current = stream.current();
        let delimiter = if current.ends_with("\"\"\"") {
            Delimiter::Triple
        } else if current.ends_with('"') {
            Delimiter::Quote
        } else {
            Delimiter::Backtick
        };
        state.tokenize = Tokenize::String(delimiter);
        return token_string(stream, state, delimiter);
    }

    if stream.matches(re!(
        r"^@[_A-Za-z\x{A1}-\x{10FFFF}][A-Za-z0-9_\x{A1}-\x{10FFFF}]*!*"
    )) || stream.matches(re!(concat!("^@", ascii_operators!())))
    {
        return Some("meta");
    }

    if stream
        .eat_if(|c| matches!(c, ';' | ',' | '(' | ')' | '[' | ']' | '{' | '}'))
        .is_some()
    {
        return None;
    }

    // keywords / builtins: `wordRegexp(list)` of plain words matches when
    // the whole run of ASCII word chars at the position is one of them
    let mut end = stream.pos;
    while stream
        .char_at(end)
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        end += 1;
    }
    let style = match stream.slice(stream.pos, end) {
        "if" | "else" | "elseif" | "while" | "for" | "begin" | "let" | "end" | "do" | "try"
        | "catch" | "finally" | "return" | "break" | "continue" | "global" | "local" | "const"
        | "export" | "import" | "importall" | "using" | "function" | "where" | "macro"
        | "module" | "baremodule" | "struct" | "type" | "mutable" | "immutable" | "quote"
        | "typealias" | "abstract" | "primitive" | "bitstype" => Some("keyword"),
        "true" | "false" | "nothing" | "NaN" | "Inf" => Some("builtin"),
        _ => None,
    };
    if style.is_some() {
        stream.pos = end;
        return style;
    }

    let is_definition = state.is_definition
        || matches!(
            state.last_token,
            LastToken::Function
                | LastToken::Macro
                | LastToken::Type
                | LastToken::Struct
                | LastToken::Immutable
        );

    if stream.matches(re!(
        r"^[_A-Za-z\x{A1}-\x{2217}\x{2219}-\x{10FFFF}][A-Za-z0-9_\x{A1}-\x{2217}\x{2219}-\x{10FFFF}]*!*"
    )) {
        if is_definition {
            if stream.peek() == Some('.') {
                state.is_definition = true;
                return Some("variable");
            }
            state.is_definition = false;
            return Some("def");
        }
        state.leaving_expr = true;
        return Some("variable");
    }

    // Handle non-detected items
    stream.next();
    Some("error")
}

/// `tokenAnnotation`
fn token_annotation(stream: &mut StringStream, state: &mut JuliaState) -> Option<&'static str> {
    stream.matches(re!(concat!("^", dot!(), r"*?(?=[,;{}()=\s]|$)")));
    if stream.match_str("{", true, false) {
        state.nested_parameters += 1;
    } else if stream.match_str("}", true, false) && state.nested_parameters > 0 {
        state.nested_parameters -= 1;
    }
    if state.nested_parameters > 0 {
        if !stream.matches(re!(concat!("^", dot!(), r"*?(?=\{|\})"))) {
            stream.next();
        }
    } else if state.nested_parameters == 0 {
        state.tokenize = Tokenize::Base;
    }
    Some("builtin")
}

/// `tokenComment`
fn token_comment(stream: &mut StringStream, state: &mut JuliaState) -> Option<&'static str> {
    if stream.match_str("#=", true, false) {
        state.nested_comments += 1;
    }
    if !stream.matches(re!(concat!("^", dot!(), r"*?(?=#=|=#)"))) {
        stream.skip_to_end();
    }
    if stream.match_str("=#", true, false) {
        state.nested_comments -= 1;
        if state.nested_comments == 0 {
            state.tokenize = Tokenize::Base;
        }
    }
    Some("comment")
}

/// `tokenChar`
fn token_char(stream: &mut StringStream, state: &mut JuliaState) -> Option<&'static str> {
    let mut is_char = false;
    if stream.matches(re!(
        r#"^(?:\\[0-7]{1,3}|\\x[A-Fa-f0-9]{1,2}|\\[abefnrtv0%?'"\\]|[^'\\])'"#
    )) {
        is_char = true;
    } else if let Some(m) = stream.match_re(re!(r"(?i)^\\u([a-f0-9]{1,4})(?=')"), true) {
        let value = u32::from_str_radix(m.group(1).unwrap_or_default(), 16).unwrap_or(0);
        if value <= 55295 || value >= 57344 {
            // (U+0,U+D7FF), (U+E000,U+FFFF)
            is_char = true;
            stream.next();
        }
    } else if let Some(m) = stream.match_re(re!(r"^\\U([A-Fa-f0-9]{5,8})(?=')"), true) {
        let value = u32::from_str_radix(m.group(1).unwrap_or_default(), 16).unwrap_or(u32::MAX);
        if value <= 1114111 {
            // U+10FFFF
            is_char = true;
            stream.next();
        }
    }
    if is_char {
        state.leaving_expr = true;
        state.tokenize = Tokenize::Base;
        return Some("string");
    }
    if !stream.matches(re!(r"^[^']+(?=')")) {
        stream.skip_to_end();
    }
    if stream.match_str("'", true, false) {
        state.tokenize = Tokenize::Base;
    }
    Some("error")
}

/// `tokenStringFactory(delimiter)`'s `tokenString`
fn token_string(
    stream: &mut StringStream,
    state: &mut JuliaState,
    delimiter: Delimiter,
) -> Option<&'static str> {
    if stream.eat('\\').is_some() {
        stream.next();
    } else if stream.match_str(delimiter.as_str(), true, false) {
        state.tokenize = Tokenize::Base;
        state.leaving_expr = true;
        return Some("string");
    } else {
        stream.eat_if(|c| c == '`' || c == '"');
    }
    stream.eat_while_if(|c| !matches!(c, '\\' | '`' | '"'));
    Some("string")
}

impl Mode for Julia {
    fn name(&self) -> &'static str {
        "julia"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(JuliaState {
            tokenize: Tokenize::Base,
            last_token: LastToken::Other,
            leaving_expr: false,
            is_definition: false,
            nested_arrays: 0,
            nested_comments: 0,
            nested_generators: 0,
            nested_parameters: 0,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<JuliaState>(st);
        let style = match state.tokenize {
            Tokenize::Base => token_base(stream, state),
            Tokenize::Comment => token_comment(stream, state),
            Tokenize::Annotation => token_annotation(stream, state),
            Tokenize::Char => token_char(stream, state),
            Tokenize::String(delimiter) => token_string(stream, state, delimiter),
        };
        let current = stream.current();
        if !current.is_empty() && style.is_some() {
            state.last_token = match current {
                "end" => LastToken::End,
                "function" => LastToken::Function,
                "macro" => LastToken::Macro,
                "type" => LastToken::Type,
                "struct" => LastToken::Struct,
                "immutable" => LastToken::Immutable,
                _ => LastToken::Other,
            };
        }
        style.map(str::to_string)
    }
}
