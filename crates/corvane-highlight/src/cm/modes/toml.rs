//! `codemirror/mode/toml/toml.js` (`text/x-toml`), ported line by line.
//! GHD maps `.toml` and the basename `cargo.lock` to it.

use std::sync::{Arc, OnceLock};

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

#[derive(Clone)]
struct TomlState {
    in_string: bool,
    string_type: char,
    lhs: bool,
    in_array: usize,
}

struct Toml;

/// `CodeMirror.getMode({}, "text/x-toml")`
pub fn toml() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(Toml)).clone()
}

impl Mode for Toml {
    fn name(&self) -> &'static str {
        "toml"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(TomlState {
            in_string: false,
            string_type: '\0',
            lhs: true,
            in_array: 0,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<TomlState>(st);
        // check for state changes
        if !s.in_string
            && let Some(q @ ('"' | '\'')) = stream.peek()
        {
            s.string_type = q;
            stream.next(); // skip quote
            s.in_string = true;
        }
        if stream.sol() && s.in_array == 0 {
            s.lhs = true;
        }
        if s.in_string {
            while s.in_string && !stream.eol() {
                if stream.peek() == Some(s.string_type) {
                    stream.next(); // skip quote
                    s.in_string = false;
                } else if stream.peek() == Some('\\') {
                    stream.next();
                    stream.next();
                } else if !stream.matches(re!(r#"^.[^\\"']*"#)) {
                    // `.` does not match U+2028/2029; JS would spin here
                    stream.next();
                }
            }
            return Some(if s.lhs { "property string" } else { "string" }.into());
        } else if s.in_array > 0 && stream.peek() == Some(']') {
            stream.next();
            s.in_array -= 1;
            return Some("bracket".into());
        } else if s.lhs && stream.peek() == Some('[') && stream.skip_to(']') {
            stream.next(); // skip closing ]
            // array of objects has an extra open & close []
            if stream.peek() == Some(']') {
                stream.next();
            }
            return Some("atom".into());
        } else if stream.peek() == Some('#') {
            stream.skip_to_end();
            return Some("comment".into());
        } else if stream.eat_space() {
            return None;
        } else if s.lhs && stream.eat_while_if(|c| c != '=' && c != ' ') {
            return Some("property".into());
        } else if s.lhs && stream.peek() == Some('=') {
            stream.next();
            s.lhs = false;
            return None;
        } else if !s.lhs && stream.matches(re!(r"^[0-9]{4}[0-9\-:.T]*Z")) {
            return Some("atom".into()); // date
        } else if !s.lhs
            && (stream.match_str("true", true, false) || stream.match_str("false", true, false))
        {
            return Some("atom".into());
        } else if !s.lhs && stream.peek() == Some('[') {
            s.in_array += 1;
            stream.next();
            return Some("bracket".into());
        } else if !s.lhs && stream.matches(re!(r"^-?[0-9]+(?:\.[0-9]+)?")) {
            return Some("number".into());
        } else if !stream.eat_space() {
            stream.next();
        }
        None
    }
}
