//! `codemirror/mode/haml/haml.js` (`text/x-haml`), ported line by line;
//! the JS function names are kept on the Rust ones.
//!
//! HTML goes to htmlmixed (with the modes haml.js loads: xml, javascript,
//! css), Ruby after `=` / `-` and inside `{…}` / `(…)` attribute hashes to
//! the ruby mode. The mode has no `innerMode`, so every token gets `m-haml`.

use std::sync::Arc;

use super::super::{Mode, ModeState, StringStream, state};
use super::ruby::{Ruby, tokenize_depth};
use crate::re;

/// `state.tokenize`
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tokenize {
    Html,
    Ruby,
    /// `rubyInQuote(endQuote)`
    RubyInQuote(char),
}

/// The haml-specific styles `previousToken.style` is compared with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prev {
    None,
    Comment,
    HamlTag,
    CloseAttributeTag,
    HamlAttribute,
    Other,
}

#[derive(Clone)]
struct HamlState {
    html_state: Box<dyn ModeState>,
    ruby_state: Box<dyn ModeState>,
    indented: usize,
    /// `previousToken.style` / `previousToken.indented`
    previous_style: Prev,
    previous_indented: usize,
    tokenize: Tokenize,
    start_of_line: bool,
}

pub struct Haml {
    html: Arc<dyn Mode>,
    ruby: Ruby,
}

impl Default for Haml {
    fn default() -> Self {
        Self::new()
    }
}

impl Haml {
    pub fn new() -> Self {
        Self {
            html: super::htmlmixed(),
            ruby: Ruby,
        }
    }

    fn call(&self, stream: &mut StringStream, s: &mut HamlState) -> Option<String> {
        match s.tokenize {
            Tokenize::Html => self.html(stream, s),
            Tokenize::Ruby => self.ruby(stream, s),
            Tokenize::RubyInQuote(q) => self.ruby_in_quote(q, stream, s),
        }
    }

    /// `rubyInQuote(endQuote)`
    fn ruby_in_quote(
        &self,
        end_quote: char,
        stream: &mut StringStream,
        s: &mut HamlState,
    ) -> Option<String> {
        if stream.peek() == Some(end_quote) && tokenize_depth(&*s.ruby_state) == 1 {
            // step out of ruby context as it seems to complete processing
            // all the braces
            stream.next();
            s.tokenize = Tokenize::Html;
            return Some("closeAttributeTag".into());
        }
        self.ruby(stream, s)
    }

    /// `ruby`
    fn ruby(&self, stream: &mut StringStream, s: &mut HamlState) -> Option<String> {
        if stream.match_str("-#", true, false) {
            stream.skip_to_end();
            return Some("comment".into());
        }
        self.ruby.token(stream, &mut *s.ruby_state)
    }

    /// `html`
    fn html(&self, stream: &mut StringStream, s: &mut HamlState) -> Option<String> {
        let ch = stream.peek();

        // handle haml declarations. All declarations that cant be handled
        // here will be passed to html mode
        if s.previous_style == Prev::Comment && s.indented > s.previous_indented {
            stream.skip_to_end();
            return Some("commentLine".into());
        }

        if s.start_of_line {
            if ch == Some('!') && stream.match_str("!!", true, false) {
                stream.skip_to_end();
                return Some("tag".into());
            } else if stream.matches(re!(r"^%[A-Za-z0-9_:#\.]+=")) {
                s.tokenize = Tokenize::Ruby;
                return Some("hamlTag".into());
            } else if stream.matches(re!(r"^%[A-Za-z0-9_:]+")) {
                return Some("hamlTag".into());
            } else if ch == Some('/') {
                stream.skip_to_end();
                return Some("comment".into());
            }
        }

        if (s.start_of_line || s.previous_style == Prev::HamlTag) && matches!(ch, Some('#' | '.')) {
            // /[\w-#\.]*/
            stream.matches(re!(r"[A-Za-z0-9_\-#.]*"));
            return Some("hamlAttribute".into());
        }

        // do not handle --> as valid ruby, make it HTML close comment instead
        if s.start_of_line
            && !stream.match_str("-->", false, false)
            && matches!(ch, Some('=' | '-'))
        {
            s.tokenize = Tokenize::Ruby;
            return self.ruby(stream, s);
        }

        if matches!(
            s.previous_style,
            Prev::HamlTag | Prev::CloseAttributeTag | Prev::HamlAttribute
        ) {
            if ch == Some('(') {
                s.tokenize = Tokenize::RubyInQuote(')');
                return self.ruby_in_quote(')', stream, s);
            } else if ch == Some('{') && !stream.matches(re!(r"^\{%.*")) {
                s.tokenize = Tokenize::RubyInQuote('}');
                return self.ruby_in_quote('}', stream, s);
            }
        }

        self.html.token(stream, &mut *s.html_state)
    }
}

impl Mode for Haml {
    fn name(&self) -> &'static str {
        "haml"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(HamlState {
            html_state: self.html.start_state(),
            ruby_state: self.ruby.start_state(),
            indented: 0,
            previous_style: Prev::None,
            previous_indented: 0,
            tokenize: Tokenize::Html,
            start_of_line: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<HamlState>(st);
        if stream.sol() {
            s.indented = stream.indentation();
            s.start_of_line = true;
        }
        if stream.eat_space() {
            return None;
        }
        let style = self.call(stream, s);
        s.start_of_line = false;
        // dont record comment line as we only want to measure comment line
        // with the opening comment block
        if let Some(st) = style.as_deref().filter(|st| !st.is_empty())
            && st != "commentLine"
        {
            s.previous_style = match st {
                "comment" => Prev::Comment,
                "hamlTag" => Prev::HamlTag,
                "closeAttributeTag" => Prev::CloseAttributeTag,
                "hamlAttribute" => Prev::HamlAttribute,
                _ => Prev::Other,
            };
            s.previous_indented = s.indented;
        }
        // if current state is ruby and the previous token is not `,` reset
        // the tokenize to html
        if stream.eol() && s.tokenize == Tokenize::Ruby {
            stream.back_up(1);
            let ch = stream.peek();
            stream.next();
            if ch.is_some_and(|c| c != ',') {
                s.tokenize = Tokenize::Html;
            }
        }
        // reprocess some of the specific style tag when finish setting
        // previousToken
        match style.as_deref() {
            Some("hamlTag") => Some("tag".into()),
            Some("commentLine") => Some("comment".into()),
            Some("hamlAttribute") => Some("attribute".into()),
            Some("closeAttributeTag") => None,
            _ => style,
        }
    }
}

/// `CodeMirror.getMode({}, "text/x-haml")`
pub fn haml() -> Arc<dyn Mode> {
    static MODE: std::sync::OnceLock<Arc<dyn Mode>> = std::sync::OnceLock::new();
    MODE.get_or_init(|| Arc::new(Haml::new())).clone()
}
