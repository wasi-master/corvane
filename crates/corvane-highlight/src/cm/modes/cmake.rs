//! `codemirror/mode/cmake/cmake.js` (`text/x-cmake`), ported line by line.
//! GHD maps `.cmake` to it (no `CMakeLists.txt` basename mapping).
//!
//! Quirks kept from the JS: the "function" test (`\w+(`) runs after the
//! first char is consumed and before the comment / string checks, so
//! `# foo(` and `"bar(` come out as `def`; a `$` inside a double-quoted
//! string ends the string token so the variable gets its own token.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Cmake;

#[derive(Clone)]
struct CmakeState {
    continue_string: bool,
    /// `state.pending` (`false` until the first string)
    pending: Option<char>,
}

/// `tokenString`
fn token_string(stream: &mut StringStream, s: &mut CmakeState) -> Option<&'static str> {
    let mut current: Option<char> = None;
    let mut prev: Option<char> = None;
    let mut found_var = false;
    while !stream.eol() {
        current = stream.next();
        if current == s.pending {
            break;
        }
        if current == Some('$') && prev != Some('\\') && s.pending == Some('"') {
            found_var = true;
            break;
        }
        prev = current;
    }
    if found_var {
        stream.back_up(1);
    }
    s.continue_string = current.is_none() || current != s.pending;
    Some("string")
}

/// `tokenize`
fn tokenize(stream: &mut StringStream, s: &mut CmakeState) -> Option<&'static str> {
    let ch = stream.next()?;

    // Have we found a variable?
    if ch == '$' {
        // variable_regex = /({)?[a-zA-Z0-9_]+(})?/
        if stream.matches(re!(r"^(\{)?[a-zA-Z0-9_]+(\})?")) {
            return Some("variable-2");
        }
        return Some("variable");
    }
    // Should we still be looking for the end of a string?
    if s.continue_string {
        stream.back_up(1);
        return token_string(stream, s);
    }
    // Do we just have a function on our hands?
    if stream.matches(re!(r"^([\s\x{feff}]+)?[A-Za-z0-9_]+\("))
        || stream.matches(re!(r"^([\s\x{feff}]+)?[A-Za-z0-9_]+ \("))
    {
        stream.back_up(1);
        return Some("def");
    }
    if ch == '#' {
        stream.skip_to_end();
        return Some("comment");
    }
    // Have we found a string?
    if ch == '\'' || ch == '"' {
        s.pending = Some(ch);
        return token_string(stream, s);
    }
    if ch == '(' || ch == ')' {
        return Some("bracket");
    }
    if ch.is_ascii_digit() {
        return Some("number");
    }
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    None
}

impl Mode for Cmake {
    fn name(&self) -> &'static str {
        "cmake"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(CmakeState {
            continue_string: false,
            pending: None,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<CmakeState>(st);
        if stream.eat_space() {
            return None;
        }
        tokenize(stream, s).map(str::to_string)
    }
}
