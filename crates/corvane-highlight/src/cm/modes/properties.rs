//! `codemirror/mode/properties/properties.js` (`text/x-properties`,
//! `text/x-ini`), ported line by line. GHD maps `.properties`,
//! `.gitattributes`, `.gitignore`, `.editorconfig` and `.ini` to it.
//!
//! Every token is one char (plus leading whitespace at the start of a
//! line), styled by the current `position`, exactly like the JS.

use crate::cm::{Mode, ModeState, StringStream, state};

pub struct Properties;

#[derive(Clone, Copy, PartialEq)]
enum Position {
    Def,
    Quote,
    Comment,
}

impl Position {
    fn style(self) -> &'static str {
        match self {
            Position::Def => "def",
            Position::Quote => "quote",
            Position::Comment => "comment",
        }
    }
}

#[derive(Clone)]
struct PropertiesState {
    /// Current position, "def", "quote" or "comment"
    position: Position,
    /// Is the next line multiline value
    next_multiline: bool,
    /// Is the current line a multiline value
    in_multiline: bool,
    /// Did we just open a section
    after_section: bool,
}

impl Mode for Properties {
    fn name(&self) -> &'static str {
        "properties"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PropertiesState {
            position: Position::Def,
            next_multiline: false,
            in_multiline: false,
            after_section: false,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<PropertiesState>(st);
        let sol = stream.sol() || s.after_section;
        let eol = stream.eol();

        s.after_section = false;

        if sol {
            if s.next_multiline {
                s.in_multiline = true;
                s.next_multiline = false;
            } else {
                s.position = Position::Def;
            }
        }

        if eol && !s.next_multiline {
            s.in_multiline = false;
            s.position = Position::Def;
        }

        if sol {
            while stream.eat_space() {}
        }

        let ch = stream.next();

        if sol && matches!(ch, Some('#' | '!' | ';')) {
            s.position = Position::Comment;
            stream.skip_to_end();
            return Some("comment".into());
        } else if sol && ch == Some('[') {
            s.after_section = true;
            stream.skip_to(']');
            stream.eat(']');
            return Some("header".into());
        } else if matches!(ch, Some('=' | ':')) {
            s.position = Position::Quote;
            return None;
        } else if ch == Some('\\') && s.position == Position::Quote && stream.eol() {
            // Multiline value
            s.next_multiline = true;
        }

        Some(s.position.style().into())
    }
}
