//! `codemirror/mode/sieve/sieve.js` (`application/sieve`), ported line by
//! line. GHD maps `.sieve` to it.
//!
//! The JS `state._indent` bracket stack and `_multiLineString` flag only
//! feed `indent`, so they are left out.

use crate::cm::{Mode, ModeState, StringStream, state};

pub struct Sieve;

#[derive(Clone, Copy)]
enum Tokenize {
    /// `tokenBase`
    Base,
    /// `tokenCComment`
    CComment,
    /// `tokenString(quote)`
    Str(char),
    /// `tokenMultiLineString`
    MultiLineString,
}

#[derive(Clone)]
struct SieveState {
    tokenize: Tokenize,
}

// tokenBase
fn token_base(stream: &mut StringStream, s: &mut SieveState) -> Option<&'static str> {
    let ch = stream.next()?;
    if ch == '/' && stream.eat('*').is_some() {
        s.tokenize = Tokenize::CComment;
        return Some(token_c_comment(stream, s));
    }
    if ch == '#' {
        stream.skip_to_end();
        return Some("comment");
    }
    if ch == '"' {
        s.tokenize = Tokenize::Str(ch);
        return Some(token_string(stream, s, ch));
    }
    if matches!(ch, '(' | '{' | ')' | '}' | ',' | ';') {
        return None;
    }
    // 1*DIGIT "K" / "M" / "G"
    if ch.is_ascii_digit() {
        stream.eat_while_if(|c| c.is_ascii_digit());
        stream.eat_if(|c| matches!(c, 'K' | 'k' | 'M' | 'm' | 'G' | 'g'));
        return Some("number");
    }
    // ":" (ALPHA / "_") *(ALPHA / DIGIT / "_")
    if ch == ':' {
        stream.eat_while_if(|c| c.is_ascii_alphabetic() || c == '_');
        stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_');
        return Some("operator");
    }
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_');
    // "text:" *(SP / HTAB) (hash-comment / CRLF)
    if stream.current() == "text" && stream.eat(':').is_some() {
        s.tokenize = Tokenize::MultiLineString;
        return Some("string");
    }
    match stream.current() {
        "if" | "elsif" | "else" | "stop" | "require" => Some("keyword"),
        "true" | "false" | "not" => Some("atom"),
        _ => None,
    }
}

// tokenMultiLineString
fn token_multi_line_string(stream: &mut StringStream, s: &mut SieveState) -> &'static str {
    // the first line is special it may contain a comment
    if !stream.sol() {
        stream.eat_space();
        if stream.peek() == Some('#') {
            stream.skip_to_end();
            return "comment";
        }
        stream.skip_to_end();
        return "string";
    }
    if stream.next() == Some('.') && stream.eol() {
        s.tokenize = Tokenize::Base;
    }
    "string"
}

// tokenCComment
fn token_c_comment(stream: &mut StringStream, s: &mut SieveState) -> &'static str {
    let mut maybe_end = false;
    while let Some(ch) = stream.next() {
        if maybe_end && ch == '/' {
            s.tokenize = Tokenize::Base;
            break;
        }
        maybe_end = ch == '*';
    }
    "comment"
}

// tokenString(quote)
fn token_string(stream: &mut StringStream, s: &mut SieveState, quote: char) -> &'static str {
    let mut escaped = false;
    while let Some(ch) = stream.next() {
        if ch == quote && !escaped {
            break;
        }
        escaped = !escaped && ch == '\\';
    }
    if !escaped {
        s.tokenize = Tokenize::Base;
    }
    "string"
}

impl Mode for Sieve {
    fn name(&self) -> &'static str {
        "sieve"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SieveState {
            tokenize: Tokenize::Base,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<SieveState>(st);
        if stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::CComment => Some(token_c_comment(stream, s)),
            Tokenize::Str(q) => Some(token_string(stream, s, q)),
            Tokenize::MultiLineString => Some(token_multi_line_string(stream, s)),
        };
        style.map(Into::into)
    }
}
