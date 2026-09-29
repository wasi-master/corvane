//! `codemirror-mode-luau/index.js` (`text/x-lua`, `text/x-luau`): GHD
//! highlights plain Lua with this Luau mode too.
//!
//! Every `stream.match(/re/)` of the JS searches the rest of the line and
//! only accepts a match at the stream position, so each regex here is
//! anchored with `^` (the leftmost match starts at 0 exactly when an
//! anchored one exists). JS `\b`, `\w` and `\d` are ASCII: `\w` is spelled
//! `[A-Za-z0-9_]`, and a `\b` right after a word char becomes
//! `(?![A-Za-z0-9_])`. A leading `\b` is always true at the start of the
//! sliced string when the next char is a word char, so it is dropped.

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
        if stream.matches(re!(concat!("^type", b_end!()))) {
            state.after_type_keyword = true;
            return tok("keyword");
        }
        if state.after_type_keyword && stream.matches(re!(IDENT)) {
            state.after_type_keyword = false;
            return tok("type");
        }
        // keywords
        if stream.matches(re!(concat!(
            "^(?:function|export|type|end|if|then|else|elseif|while|do|for|in|repeat|until|return|local|not|and|or)",
            b_end!()
        ))) {
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
        if stream.matches(re!(concat!(
            "^(?:print|math|table|string|coroutine|Vector2|Vector3|UDim|UDim2|os|io|debug|package|require|_G|shared|game|pairs|ipairs|setmetatable|getmetatable|newproxy)",
            b_end!()
        ))) {
            return tok("builtin");
        }
        if stream.matches(re!(concat!("^true", b_end!()))) {
            return tok("positive");
        }
        if stream.matches(re!(concat!("^false", b_end!()))) {
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
