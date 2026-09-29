//! `codemirror/mode/diff/diff.js` (`text/x-diff`): a line is styled by its
//! first char (`TOKEN_NAMES`), and trailing whitespace is an `error`.

use crate::cm::{Mode, ModeState, StringStream};

pub struct Diff;

/// `TOKEN_NAMES`
fn token_name(c: Option<char>) -> Option<&'static str> {
    match c? {
        '+' => Some("positive"),
        '-' => Some("negative"),
        '@' => Some("meta"),
        _ => None,
    }
}

/// `stream.string.search(/[\t ]+?$/)`: the char index where the line's
/// trailing run of tabs and spaces starts.
fn trailing_whitespace(stream: &StringStream) -> Option<usize> {
    let s = stream.string();
    let trimmed = s.trim_end_matches([' ', '\t']);
    (trimmed.len() < s.len()).then(|| trimmed.chars().count())
}

impl Mode for Diff {
    fn name(&self) -> &'static str {
        "diff"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(())
    }

    fn token(&self, stream: &mut StringStream, _state: &mut dyn ModeState) -> Option<String> {
        let tw_pos = trailing_whitespace(stream);

        if !stream.sol() || tw_pos == Some(0) {
            stream.skip_to_end();
            return Some(match token_name(stream.char_at(0)) {
                Some(name) => format!("error {name}"),
                None => "error".to_string(),
            });
        }

        let token_name = token_name(stream.peek());
        if token_name.is_none() {
            stream.skip_to_end();
        }

        match tw_pos {
            None => stream.skip_to_end(),
            Some(tw) => stream.pos = tw,
        }

        token_name.map(str::to_string)
    }
}
