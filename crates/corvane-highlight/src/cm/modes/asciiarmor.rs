//! `codemirror/mode/asciiarmor/asciiarmor.js` (`application/pgp`,
//! `application/pgp-encrypted`, `application/pgp-keys`,
//! `application/pgp-signature`), ported line by line. GHD maps `.pgp` to it.
//!
//! The armor type is captured by `(.*)?`, which in JS leaves the group
//! `undefined` rather than `""` when nothing is between the dashes; an empty
//! capture is stored as `None` here so BEGIN/END comparisons agree.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct AsciiArmor;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Top,
    Headers,
    Header,
    Body,
    End,
}

#[derive(Clone)]
struct ArmorState {
    state: Phase,
    ty: Option<String>,
}

// errorIfNotEmpty
fn error_if_not_empty(stream: &mut StringStream) -> Option<&'static str> {
    let non_ws = stream.match_re(re!(r"^\s*\S"), true).is_some();
    stream.skip_to_end();
    non_ws.then_some("error")
}

/// The `(.*)?` group of a BEGIN/END line (`undefined` when empty).
fn armor_type(m: &crate::cm::Match) -> Option<String> {
    m.group(1).filter(|t| !t.is_empty()).map(str::to_string)
}

impl Mode for AsciiArmor {
    fn name(&self) -> &'static str {
        "asciiarmor"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(ArmorState {
            state: Phase::Top,
            ty: None,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<ArmorState>(st);
        let style = match s.state {
            Phase::Top => {
                if stream.sol()
                    && let Some(m) = stream.match_re(
                        re!(r"^-----BEGIN ([^\n\r\x{2028}\x{2029}]*)?-----\s*$"),
                        true,
                    )
                {
                    s.state = Phase::Headers;
                    s.ty = armor_type(&m);
                    return Some("tag".into());
                }
                error_if_not_empty(stream)
            }
            Phase::Headers => {
                if stream.sol() && stream.matches(re!(r"^[A-Za-z0-9_]+:")) {
                    s.state = Phase::Header;
                    Some("atom")
                } else {
                    let result = error_if_not_empty(stream);
                    if result.is_some() {
                        s.state = Phase::Body;
                    }
                    result
                }
            }
            Phase::Header => {
                stream.skip_to_end();
                s.state = Phase::Headers;
                Some("string")
            }
            Phase::Body => {
                if stream.sol()
                    && let Some(m) = stream
                        .match_re(re!(r"^-----END ([^\n\r\x{2028}\x{2029}]*)?-----\s*$"), true)
                {
                    if armor_type(&m) != s.ty {
                        return Some("error".into());
                    }
                    s.state = Phase::End;
                    Some("tag")
                } else if stream
                    .eat_while_if(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
                {
                    None
                } else {
                    stream.next();
                    Some("error")
                }
            }
            Phase::End => error_if_not_empty(stream),
        };
        style.map(Into::into)
    }

    // blankLine
    fn blank_line(&self, st: &mut dyn ModeState) {
        let s = state::<ArmorState>(st);
        if s.state == Phase::Headers {
            s.state = Phase::Body;
        }
    }
}
