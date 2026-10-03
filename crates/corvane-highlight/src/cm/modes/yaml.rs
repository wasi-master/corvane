//! `codemirror/mode/yaml/yaml.js` (`text/yaml`, `text/x-yaml`), ported line
//! by line.

use std::sync::{Arc, OnceLock};

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

#[derive(Clone)]
struct YamlState {
    pair: bool,
    pair_start: bool,
    key_col: usize,
    inline_pairs: i32,
    inline_list: i32,
    literal: bool,
    escaped: bool,
}

struct Yaml;

/// JS `\b` (ASCII word chars; fancy_regex has no `(?-u:\b)`).
macro_rules! ascii_b {
    () => {
        r"(?:(?<=[A-Za-z0-9_])(?![A-Za-z0-9_])|(?<![A-Za-z0-9_])(?=[A-Za-z0-9_]))"
    };
}

/// `CodeMirror.getMode({}, "text/yaml")`
pub fn yaml() -> Arc<dyn Mode> {
    static MODE: OnceLock<Arc<dyn Mode>> = OnceLock::new();
    MODE.get_or_init(|| Arc::new(Yaml)).clone()
}

/// JS `/\s/` (Unicode whitespace plus U+FEFF, without U+0085).
fn js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

impl Mode for Yaml {
    fn name(&self) -> &'static str {
        "yaml"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(YamlState {
            pair: false,
            pair_start: false,
            key_col: 0,
            inline_pairs: 0,
            inline_list: 0,
            literal: false,
            escaped: false,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<YamlState>(st);
        let ch = stream.peek();
        let esc = s.escaped;
        s.escaped = false;
        // comments
        if ch == Some('#')
            && (stream.pos == 0 || stream.char_at(stream.pos - 1).is_some_and(js_space))
        {
            stream.skip_to_end();
            return Some("comment".into());
        }

        if stream.matches(re!(r#"^('([^']|\\.)*'?|"([^"]|\\.)*"?)"#)) {
            return Some("string".into());
        }

        if s.literal && stream.indentation() > s.key_col {
            stream.skip_to_end();
            return Some("string".into());
        } else if s.literal {
            s.literal = false;
        }
        if stream.sol() {
            s.key_col = 0;
            s.pair = false;
            s.pair_start = false;
            // document start
            if stream.match_str("---", true, false) {
                return Some("def".into());
            }
            // document end
            if stream.match_str("...", true, false) {
                return Some("def".into());
            }
            // array list item
            if stream.matches(re!(r"\s*-\s+")) {
                return Some("meta".into());
            }
        }
        // inline pairs/lists
        if stream.matches(re!(r"^(\{|\}|\[|\])")) {
            match ch {
                Some('{') => s.inline_pairs += 1,
                Some('}') => s.inline_pairs -= 1,
                Some('[') => s.inline_list += 1,
                _ => s.inline_list -= 1,
            }
            return Some("meta".into());
        }

        // list separator
        if s.inline_list > 0 && !esc && ch == Some(',') {
            stream.next();
            return Some("meta".into());
        }
        // pairs separator
        if s.inline_pairs > 0 && !esc && ch == Some(',') {
            s.key_col = 0;
            s.pair = false;
            s.pair_start = false;
            stream.next();
            return Some("meta".into());
        }

        // start of value of a pair
        if s.pair_start {
            // block literals
            if stream.matches(re!(r"^\s*(\||>)\s*")) {
                s.literal = true;
                return Some("meta".into());
            }
            // references
            if stream.matches(re!(concat!(r"^\s*(\&|\*)[a-zA-Z0-9\._-]+", ascii_b!()))) {
                return Some("variable-2".into());
            }
            // numbers
            if s.inline_pairs == 0 && stream.matches(re!(r"^\s*-?[0-9\.\,]+\s?$")) {
                return Some("number".into());
            }
            if s.inline_pairs > 0 && stream.matches(re!(r"^\s*-?[0-9\.\,]+\s?(?=(,|\}))")) {
                return Some("number".into());
            }
            // keywords: new RegExp("\\b((true)|(false)|...)$", 'i'); the
            // leading `\b` always holds where the match has to start
            if stream.matches(re!(
                r"^(?:[tT][rR][uU][eE]|[fF][aA][lL][sS][eE]|[oO][nN]|[oO][fF][fF]|[yY][eE][sS]|[nN][oO])$"
            )) {
                return Some("keyword".into());
            }
        }

        // pairs (associative arrays) -> key
        if !s.pair
            && stream.matches(re!(
                r#"^\s*(?:[,\[\]{}&*!|>'"%@`][^\s'":]|[^\s,\[\]{}#&*!|>'"%@`])[^#:]*(?=:($|\s))"#
            ))
        {
            s.pair = true;
            s.key_col = stream.indentation();
            return Some("atom".into());
        }
        if s.pair && stream.matches(re!(r"^:\s*")) {
            s.pair_start = true;
            return Some("meta".into());
        }

        // nothing found, continue
        s.pair_start = false;
        s.escaped = ch == Some('\\');
        stream.next();
        None
    }
}
