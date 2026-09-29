//! `codemirror-mode-luau/index.js` (`text/x-lua`, `text/x-luau`): GHD
//! highlights plain Lua with this Luau mode too.
//!
//! Every `stream.match(/re/)` of the JS searches the rest of the line and
//! only accepts a match at the stream position, so each regex here is
//! anchored with `^` (the leftmost match starts at 0 exactly when an
//! anchored one exists). JS `\b`, `\w` and `\d` are ASCII: `\w` is spelled
//! `[A-Za-z0-9_]`, and a `\b` right after a word char becomes
//! `(?![A-Za-z0-9_])`. A leading `\b` is always true at the start of the
//! sliced string when the next char is a word char, so it is dropped; the
//! keyword-list regexes (`\b(a|b|…)\b`) become a lookup of the word at
//! the position ([`word_at`]).

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

/// `\w` in ASCII, as a regex fragment.
macro_rules! w {
    () => {
        "[A-Za-z0-9_]"
    };
}
/// A trailing JS `\b` after a word char.
macro_rules! b_end {
    () => {
        "(?![A-Za-z0-9_])"
    };
}

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    Base,
    LongComment,
    BacktickString,
}

#[derive(Clone)]
struct LuauState {
    tokenize: Tokenize,
    after_colon: bool,
    after_type_keyword: bool,
}

pub struct Luau;

/// `longComment`
fn long_comment(stream: &mut StringStream, state: &mut LuauState) -> Option<String> {
    while !stream.eol() {
        if stream.match_str("]]", true, false) {
            state.tokenize = Tokenize::Base;
            break;
        }
        stream.next();
    }
    Some("comment".into())
}

/// `backtickString`
fn backtick_string(stream: &mut StringStream, state: &mut LuauState) -> Option<String> {
    while !stream.eol() {
        if stream.eat('`').is_some() {
            state.tokenize = Tokenize::Base;
            break;
        }
        stream.next();
    }
    Some("string".into())
}

fn tokenize(stream: &mut StringStream, state: &mut LuauState) -> Option<String> {
    match state.tokenize {
        Tokenize::Base => None,
        Tokenize::LongComment => long_comment(stream, state),
        Tokenize::BacktickString => backtick_string(stream, state),
    }
}

/// Which of the mode's `\b(…)\b` word regexes match at the stream.
#[derive(Clone, Copy, PartialEq)]
enum Word {
    /// `/\btype\b/`
    Type,
    /// `keywords`
    Keyword,
    /// `globals`
    Global,
    True,
    False,
    Other,
}

/// The word regexes as a lookup: a regex `^(?:w1|w2|…)\b` of plain ASCII
/// words matches exactly when one of the words is the whole run of ASCII
/// word chars at the position (the leading `\b` holds there). Returns the
/// kind and the run's length.
fn word_at(stream: &StringStream) -> (Word, usize) {
    let mut end = stream.pos;
    while stream
        .char_at(end)
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        end += 1;
    }
    let kind = match stream.slice(stream.pos, end) {
        "type" => Word::Type,
        "function" | "export" | "end" | "if" | "then" | "else" | "elseif" | "while" | "do"
        | "for" | "in" | "repeat" | "until" | "return" | "local" | "not" | "and" | "or" => {
            Word::Keyword
        }
        "print" | "math" | "table" | "string" | "coroutine" | "Vector2" | "Vector3" | "UDim"
        | "UDim2" | "os" | "io" | "debug" | "package" | "require" | "_G" | "shared" | "game"
        | "pairs" | "ipairs" | "setmetatable" | "getmetatable" | "newproxy" => Word::Global,
        "true" => Word::True,
        "false" => Word::False,
        _ => Word::Other,
    };
    (kind, end - stream.pos)
}

const IDENT: &str = concat!("^[a-zA-Z_]", w!(), "*");

impl Mode for Luau {
    fn name(&self) -> &'static str {
        "luau"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(LuauState {
            tokenize: Tokenize::Base,
            after_colon: false,
            after_type_keyword: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<LuauState>(st);
        let tok = |s: &str| Some(s.to_string());
        if state.tokenize != Tokenize::Base {
            return tokenize(stream, state);
        }

        if stream.match_str("--[[", true, false) {
            state.tokenize = Tokenize::LongComment;
            return tokenize(stream, state);
        }
        if stream.eat('`').is_some() {
            state.tokenize = Tokenize::BacktickString;
            return tokenize(stream, state);
        }
        if stream.match_str("--", true, false) {
            stream.skip_to_end();
            return tok("comment");
        }
        if stream.matches(re!(r#"^"(?:[^"\\]|\\.)*"?"#)) {
            return tok("string");
        }
        if stream.matches(re!(r"^'(?:[^'\\]|\\.)*'?")) {
            return tok("string");
        }
        if stream.matches(re!(concat!(r"^[0-9]+(?:\.[0-9]+)?", b_end!()))) {
            return tok("number");
        }
        let (word, word_len) = word_at(stream);
        if word == Word::Type {
            stream.pos += word_len;
            state.after_type_keyword = true;
            return tok("keyword");
        }
        if state.after_type_keyword && stream.matches(re!(IDENT)) {
            state.after_type_keyword = false;
            return tok("type");
        }
        // keywords
        if word == Word::Keyword {
            stream.pos += word_len;
            return tok("keyword");
        }
        if stream.match_str("self", true, false) {
            return tok("variable-3");
        }
        if state.after_colon && stream.matches(re!(IDENT)) {
            state.after_colon = false;
            return tok("type");
        }
        // globals
        if word == Word::Global {
            stream.pos += word_len;
            return tok("builtin");
        }
        if word == Word::True {
            stream.pos += word_len;
            return tok("positive");
        }
        if word == Word::False {
            stream.pos += word_len;
            return tok("negative");
        }
        if stream.matches(re!(IDENT)) {
            if stream.match_re(re!(r"^\s*&\s*\{"), false).is_some() {
                return tok("type");
            }
            if stream
                .match_re(
                    re!(concat!(
                        r"^(?:\s*:\s*[a-zA-Z_]",
                        w!(),
                        r"*\s*,|\s*,\s*[a-zA-Z_]",
                        w!(),
                        r"*\s*|\s*\)\s*(?:do)?)"
                    )),
                    false,
                )
                .is_some()
            {
                return tok("variable-2");
            }
            return tok("variable");
        }
        if stream.eat(':').is_some() {
            if stream
                .match_re(re!(concat!(r"^\s*[a-zA-Z_]", w!(), r"*\s*\(")), false)
                .is_some()
            {
                return tok("operator");
            }
            state.after_colon = true;
            return tok("operator");
        }
        if stream.eat('|').is_some() {
            state.after_colon = true;
            return tok("operator");
        }
        if stream.matches(re!(r"^(?:==|~=|>=|<=|[=+\-*/|()?#\[\]&])")) {
            state.after_colon = false;
            return tok("operator");
        }
        if stream.eat_if(|c| c == '{' || c == '}').is_some() {
            return tok("bracket");
        }
        stream.next();
        None
    }
}
