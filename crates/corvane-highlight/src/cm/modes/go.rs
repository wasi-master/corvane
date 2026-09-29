//! `codemirror/mode/go/go.js` (`text/x-go`).
//!
//! The JS mode also keeps a bracket context for indentation (`Context`,
//! `pushContext`, `popContext`, `curPunc`); it never changes a token's style,
//! so only the tokenizers are ported.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Go;

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    /// `tokenBase` (the JS `null` tokenizer)
    Base,
    /// `tokenString(quote)`
    Str(char),
    /// `tokenComment`
    Comment,
}

#[derive(Clone)]
struct GoState {
    tokenize: Tokenize,
}

// `keywords`
fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "break"
            | "case"
            | "chan"
            | "const"
            | "continue"
            | "default"
            | "defer"
            | "else"
            | "fallthrough"
            | "for"
            | "func"
            | "go"
            | "goto"
            | "if"
            | "import"
            | "interface"
            | "map"
            | "package"
            | "range"
            | "return"
            | "select"
            | "struct"
            | "switch"
            | "type"
            | "var"
            | "bool"
            | "byte"
            | "complex64"
            | "complex128"
            | "float32"
            | "float64"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "string"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "int"
            | "uint"
            | "uintptr"
            | "error"
            | "rune"
            | "any"
            | "comparable"
    )
}

// `atoms`
fn is_atom(w: &str) -> bool {
    matches!(
        w,
        "true"
            | "false"
            | "iota"
            | "nil"
            | "append"
            | "cap"
            | "close"
            | "complex"
            | "copy"
            | "delete"
            | "imag"
            | "len"
            | "make"
            | "new"
            | "panic"
            | "print"
            | "println"
            | "real"
            | "recover"
    )
}

/// `isOperatorChar = /[+\-*&^%:=<>!|\/]/`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '*' | '&' | '^' | '%' | ':' | '=' | '<' | '>' | '!' | '|' | '/'
    )
}

/// `/[\w\$_\xa1-￿]/` (chars past the BMP count too: JS sees their
/// surrogate halves, which fall in the range)
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c >= '\u{a1}'
}

fn token_base(stream: &mut StringStream, s: &mut GoState) -> Option<&'static str> {
    let ch = stream.next()?;
    if ch == '"' || ch == '\'' || ch == '`' {
        s.tokenize = Tokenize::Str(ch);
        return token_string(ch, stream, s);
    }
    if ch.is_ascii_digit() || ch == '.' {
        if ch == '.' {
            stream.match_re(re!(r"^[0-9_]+([eE][\-+]?[0-9_]+)?"), true);
        } else if ch == '0' {
            if stream.match_re(re!(r"^[xX][0-9a-fA-F_]+"), true).is_none() {
                stream.match_re(re!(r"^[0-7_]+"), true);
            }
        } else {
            stream.match_re(re!(r"^[0-9_]*\.?[0-9_]*([eE][\-+]?[0-9_]+)?"), true);
        }
        return Some("number");
    }
    if matches!(
        ch,
        '[' | ']' | '{' | '}' | '(' | ')' | ',' | ';' | ':' | '.'
    ) {
        return None;
    }
    if ch == '/' {
        if stream.eat('*').is_some() {
            s.tokenize = Tokenize::Comment;
            return token_comment(stream, s);
        }
        if stream.eat('/').is_some() {
            stream.skip_to_end();
            return Some("comment");
        }
    }
    if is_operator_char(ch) {
        stream.eat_while_if(is_operator_char);
        return Some("operator");
    }
    stream.eat_while_if(is_word_char);
    let cur = stream.current();
    if is_keyword(cur) {
        return Some("keyword");
    }
    if is_atom(cur) {
        return Some("atom");
    }
    Some("variable")
}

/// `tokenString(quote)`
fn token_string(quote: char, stream: &mut StringStream, s: &mut GoState) -> Option<&'static str> {
    let mut escaped = false;
    let mut end = false;
    while let Some(next) = stream.next() {
        if next == quote && !escaped {
            end = true;
            break;
        }
        escaped = !escaped && quote != '`' && next == '\\';
    }
    if end || !(escaped || quote == '`') {
        s.tokenize = Tokenize::Base;
    }
    Some("string")
}

/// `tokenComment`
fn token_comment(stream: &mut StringStream, s: &mut GoState) -> Option<&'static str> {
    let mut maybe_end = false;
    while let Some(ch) = stream.next() {
        if ch == '/' && maybe_end {
            s.tokenize = Tokenize::Base;
            break;
        }
        maybe_end = ch == '*';
    }
    Some("comment")
}

impl Mode for Go {
    fn name(&self) -> &'static str {
        "go"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(GoState {
            tokenize: Tokenize::Base,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<GoState>(st);
        if stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::Str(q) => token_string(q, stream, s),
            Tokenize::Comment => token_comment(stream, s),
        };
        style.map(str::to_string)
    }
}
