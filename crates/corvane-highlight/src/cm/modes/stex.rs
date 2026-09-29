//! `codemirror/mode/stex/stex.js` (`text/x-stex`, `text/x-latex`), ported
//! line by line. GHD maps `.tex` to it.
//!
//! The JS plugin objects (`addPluginPattern`, `plugins["DEFAULT"]`) become
//! [`Plug`] values on the command stack; `state.f` becomes [`F`]. Quirks
//! kept: a closing bracket only pops its command on the next token
//! (`beginParams`), and the fallback branch of `normal` swallows the word
//! chars following any other char.

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

pub struct Stex {
    /// `parserConfig.inMathMode`
    in_math_mode: bool,
}

impl Stex {
    pub const fn new() -> Self {
        Self {
            in_math_mode: false,
        }
    }
}

impl Default for Stex {
    fn default() -> Self {
        Self::new()
    }
}

/// A command plugin: `plugins[name]` (`styles` per bracket argument) or
/// `plugins["DEFAULT"]` (`styles == None`).
#[derive(Clone)]
struct Plug {
    /// `this.styles` (the command style is always `"tag"`)
    styles: Option<&'static [&'static str]>,
    bracket_no: usize,
}

impl Plug {
    fn is_default(&self) -> bool {
        self.styles.is_none()
    }
    /// `styleIdentifier`
    fn style_identifier(&self) -> Option<&'static str> {
        let styles = self.styles?;
        let s = styles.get(self.bracket_no.checked_sub(1)?)?;
        (!s.is_empty()).then_some(*s)
    }
    /// `openBracket`
    fn open_bracket(&mut self) {
        if !self.is_default() {
            self.bracket_no += 1;
        }
    }
}

/// `plugins[cmdName]` or `plugins["DEFAULT"]`, instantiated
fn plugin(cmd_name: &str) -> Plug {
    let styles: Option<&'static [&'static str]> = match cmd_name {
        "importmodule" => Some(&["string", "builtin"]),
        "documentclass" => Some(&["", "atom"]),
        "usepackage" | "begin" | "end" | "label" | "ref" | "eqref" | "cite" | "bibitem"
        | "Bibitem" | "RBibitem" => Some(&["atom"]),
        _ => None,
    };
    Plug {
        styles,
        bracket_no: 0,
    }
}

/// `state.f`
#[derive(Clone, Copy, PartialEq)]
enum F {
    Normal,
    /// `inMathMode(source, state, endModeSeq)`
    Math(Option<&'static str>),
    BeginParams,
}

#[derive(Clone)]
struct StexState {
    cmd_state: Vec<Plug>,
    f: F,
}

/// `getMostPowerful`: the non-default plugin closest to the end of the list
fn most_powerful_style(s: &StexState) -> Option<&'static str> {
    s.cmd_state
        .iter()
        .rev()
        .find(|p| !p.is_default())
        .and_then(Plug::style_identifier)
}

/// `\` followed by one of `set` (`/^\\[…]/`)
fn match_escape(source: &mut StringStream, set: &str) -> bool {
    if source.peek() == Some('\\')
        && source
            .char_at(source.pos + 1)
            .is_some_and(|c| set.contains(c))
    {
        source.pos += 2;
        true
    } else {
        false
    }
}

/// `normal`: called when in a normal (no environment) context
fn normal(source: &mut StringStream, s: &mut StexState) -> Option<&'static str> {
    // Do we look like '\command' ?  If so, attempt to apply the plugin 'command'
    if source.matches(re!(r"^\\[a-zA-Z@]+")) {
        let plug = plugin(&source.current()[1..]);
        s.cmd_state.push(plug);
        s.f = F::BeginParams;
        return Some("tag");
    }

    // escape characters
    if match_escape(source, "$&%#{}_") {
        return Some("tag");
    }

    // white space control characters
    if match_escape(source, ",;!/\\") {
        return Some("tag");
    }

    // find if we're starting various math modes
    if source.match_str("\\[", true, false) {
        s.f = F::Math(Some("\\]"));
        return Some("keyword");
    }
    if source.match_str("\\(", true, false) {
        s.f = F::Math(Some("\\)"));
        return Some("keyword");
    }
    if source.match_str("$$", true, false) {
        s.f = F::Math(Some("$$"));
        return Some("keyword");
    }
    if source.match_str("$", true, false) {
        s.f = F::Math(Some("$"));
        return Some("keyword");
    }

    let ch = source.next()?;
    if ch == '%' {
        source.skip_to_end();
        Some("comment")
    } else if ch == '}' || ch == ']' {
        // plug.closeBracket(ch) is a no-op for every plugin
        if s.cmd_state.is_empty() {
            return Some("error");
        }
        s.f = F::BeginParams;
        Some("bracket")
    } else if ch == '{' || ch == '[' {
        s.cmd_state.push(plugin("DEFAULT"));
        Some("bracket")
    } else if ch.is_ascii_digit() {
        source.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '%');
        Some("atom")
    } else {
        source.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        // (`plug.argument = source.current()` for \begin: unused for styling)
        most_powerful_style(s)
    }
}

/// `inMathMode`
fn in_math_mode(
    source: &mut StringStream,
    s: &mut StexState,
    end_mode_seq: Option<&'static str>,
) -> Option<&'static str> {
    if source.eat_space() {
        return None;
    }
    if let Some(end) = end_mode_seq
        && source.match_str(end, true, false)
    {
        s.f = F::Normal;
        return Some("keyword");
    }
    if source.matches(re!(r"^\\[a-zA-Z@]+")) {
        return Some("tag");
    }
    if source.eat_while_if(|c| c.is_ascii_alphabetic()) {
        return Some("variable-2");
    }
    // escape characters
    if match_escape(source, "$&%#{}_") {
        return Some("tag");
    }
    // white space control characters
    if match_escape(source, ",;!/") {
        return Some("tag");
    }
    // special math-mode characters
    if source.eat_if(|c| matches!(c, '^' | '_' | '&')).is_some() {
        return Some("tag");
    }
    // non-special characters
    if source
        .eat_if(|c| "+-<>|=,/@!*:;'\"`~#?".contains(c))
        .is_some()
    {
        return None;
    }
    if source.matches(re!(r"^([0-9]+\.[0-9]*|[0-9]*\.[0-9]+|[0-9]+)")) {
        return Some("number");
    }
    let ch = source.next()?;
    if matches!(ch, '{' | '}' | '[' | ']' | '(' | ')') {
        return Some("bracket");
    }
    if ch == '%' {
        source.skip_to_end();
        return Some("comment");
    }
    Some("error")
}

/// `beginParams`
fn begin_params(source: &mut StringStream, s: &mut StexState) -> Option<&'static str> {
    let ch = source.peek();
    if let Some(c @ ('{' | '[')) = ch {
        if let Some(last) = s.cmd_state.last_mut() {
            last.open_bracket();
        }
        source.eat(c);
        s.f = F::Normal;
        return Some("bracket");
    }
    if let Some(c @ (' ' | '\t' | '\r')) = ch {
        source.eat(c);
        return None;
    }
    s.f = F::Normal;
    // popCommand: closeBracket is a no-op
    s.cmd_state.pop();

    normal(source, s)
}

impl Mode for Stex {
    fn name(&self) -> &'static str {
        "stex"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(StexState {
            cmd_state: Vec::new(),
            f: if self.in_math_mode {
                F::Math(None)
            } else {
                F::Normal
            },
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<StexState>(st);
        let style = match s.f {
            F::Normal => normal(stream, s),
            F::Math(end) => in_math_mode(stream, s, end),
            F::BeginParams => begin_params(stream, s),
        };
        style.map(str::to_string)
    }

    // blankLine
    fn blank_line(&self, st: &mut dyn ModeState) {
        let s = state::<StexState>(st);
        s.f = F::Normal;
        s.cmd_state.clear();
    }
}
