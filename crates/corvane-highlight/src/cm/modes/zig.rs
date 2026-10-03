//! `codemirror-mode-zig/index.js` (`text/x-zig`), ported line by line.
//! GHD maps `.zig` to it.
//!
//! The mode is small: no block comments, no char literals and no
//! `\\` multi-line strings (a lone `\` is an unstyled token), exactly like
//! the JS.

use crate::cm::{Mode, ModeState, StringStream, state};

pub struct Zig;

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    /// `tokenBase`
    Base,
    /// `tokenString(quote)`
    Str(char),
}

#[derive(Clone)]
struct ZigState {
    tokenize: Tokenize,
}

// `keywords`
fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "const"
            | "var"
            | "extern"
            | "packed"
            | "export"
            | "pub"
            | "noalias"
            | "inline"
            | "comptime"
            | "test"
            | "fn"
            | "usingnamespace"
            | "struct"
            | "enum"
            | "union"
            | "if"
            | "else"
            | "switch"
            | "while"
            | "for"
            | "break"
            | "continue"
            | "return"
            | "defer"
            | "errdefer"
            | "as"
            | "null"
    )
}

/// `isOperatorChar = /[+\-*&%=<>!?|]/`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '*' | '&' | '%' | '=' | '<' | '>' | '!' | '?' | '|'
    )
}

/// JS `\w` (ASCII)
fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `tokenBase`
fn token_base(stream: &mut StringStream, s: &mut ZigState) -> Option<&'static str> {
    let ch = stream.next()?;
    if ch == '"' {
        s.tokenize = Tokenize::Str(ch);
        return token_string(ch, stream, s);
    }
    if ch.is_ascii_digit() {
        stream.eat_while_if(|c| is_word(c) || c == '.');
        return Some("number");
    }
    if is_word(ch) {
        stream.eat_while_if(is_word);
        if is_keyword(stream.current()) {
            return Some("keyword");
        }
        return Some("variable");
    }
    if is_operator_char(ch) {
        stream.eat_while_if(is_operator_char);
        return Some("operator");
    }
    if ch == '/' && stream.eat('/').is_some() {
        stream.skip_to_end();
        return Some("comment");
    }
    None
}

/// `tokenString(quote)`
fn token_string(quote: char, stream: &mut StringStream, s: &mut ZigState) -> Option<&'static str> {
    let mut escaped = false;
    let mut end = false;
    while let Some(next) = stream.next() {
        if next == quote && !escaped {
            end = true;
            break;
        }
        escaped = !escaped && next == '\\';
    }
    if end || !escaped {
        s.tokenize = Tokenize::Base;
    }
    Some("string")
}

impl Mode for Zig {
    fn name(&self) -> &'static str {
        "zig"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(ZigState {
            tokenize: Tokenize::Base,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<ZigState>(st);
        if stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::Str(q) => token_string(q, stream, s),
        };
        style.map(str::to_string)
    }
}
