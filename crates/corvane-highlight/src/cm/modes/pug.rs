//! `codemirror/mode/pug/pug.js` (`text/x-pug`, `text/x-jade`), ported line
//! by line; the JS function names are kept on the Rust ones.
//!
//! pug.js requires javascript, css and htmlmixed, so filters, `script.` /
//! `style.` blocks and inline `<html>` lines resolve through
//! [`super::html_modes`]; any other name falls back to the `null` mode
//! (the block becomes a plain `string`).
//!
//! Quirks of the JS kept on purpose:
//! - The mode has no `innerMode`, so every token (inner ones included)
//!   gets `m-pug`.
//! - `innerModeForLine` is never cleared once an inline `<html>` line set
//!   it: from then on the rest of every line after its first token is
//!   skipped by `innerMode` (style `indentToken`, usually `null`), and the
//!   remaining functions of `nextToken` run at the end of the line - so
//!   `javaScript` then calls the javascript mode at `eol`, whose
//!   `tokenBase` tests `undefined` as the word `"undefined"`.
//! - `attrsContinued` decides where an unquoted attribute value ends with
//!   `Function('', 'var x ' + attrValue)`; [`super::js_syntax`] stands in
//!   for that syntax check.
//! - The shared `jsMode` instance keeps its `type` / `content` scratch
//!   across `startState` resets of `state.jsState`, so the reset carries it.

use std::borrow::Cow;
use std::sync::Arc;

use super::super::{Mode, ModeState, StringStream, js_len, state};
use super::javascript::{JsConfig, JsMode, JsState};
use super::js_syntax::var_x_parses;
use crate::re;

/// A `nextToken` step result: `None` is falsy (`undefined` / `null` /
/// `''`), `Some(None)` is `true`, `Some(Some(style))` a style.
type Tok = Option<Option<Cow<'static, str>>>;

fn style(s: &'static str) -> Tok {
    Some(Some(Cow::Borrowed(s)))
}

/// `tok || true` for a nested mode's token.
fn or_true(t: Option<&'static str>) -> Tok {
    Some(t.map(Cow::Borrowed))
}

#[derive(Clone)]
struct PugState {
    java_script_line: bool,
    java_script_line_excludes_colon: bool,
    java_script_arguments: bool,
    java_script_arguments_depth: i32,
    is_interpolating: bool,
    interpolation_nesting: i32,
    js_state: JsState,
    /// `''` is `None`
    rest_of_line: Option<&'static str>,
    is_include_filtered: bool,
    is_each: bool,
    last_tag: String,
    script_type: String,
    is_attrs: bool,
    attrs_nest: Vec<char>,
    in_attribute_name: bool,
    attribute_is_type: bool,
    attr_value: String,
    /// `None` is `Infinity`
    indent_of: Option<usize>,
    /// `''` / `null` are `None`
    indent_token: Option<&'static str>,
    inner_mode: Option<Arc<dyn Mode>>,
    inner_state: Option<Box<dyn ModeState>>,
    inner_mode_for_line: bool,
    mixin_call_after: bool,
}

pub struct Pug {
    js: JsMode,
}

impl Default for Pug {
    fn default() -> Self {
        Self::new()
    }
}

/// A mode spec as `setInnerMode` receives it.
enum Spec<'a> {
    Name(&'a str),
    Mode(Arc<dyn Mode>),
    Null,
}

impl Pug {
    pub fn new() -> Self {
        Self {
            js: JsMode::new(JsConfig::default()),
        }
    }

    fn js_token(&self, stream: &mut StringStream, s: &mut PugState) -> Option<&'static str> {
        self.js.token_js(stream, &mut s.js_state)
    }

    /// `javaScript`
    fn java_script(&self, stream: &mut StringStream, s: &mut PugState) -> Tok {
        if stream.sol() {
            s.java_script_line = false;
            s.java_script_line_excludes_colon = false;
        }
        if s.java_script_line {
            if s.java_script_line_excludes_colon && stream.peek() == Some(':') {
                s.java_script_line = false;
                s.java_script_line_excludes_colon = false;
                return None;
            }
            let tok = self.js_token(stream, s);
            if stream.eol() {
                s.java_script_line = false;
            }
            return or_true(tok);
        }
        None
    }

    /// `javaScriptArguments`
    fn java_script_arguments(&self, stream: &mut StringStream, s: &mut PugState) -> Tok {
        if s.java_script_arguments {
            if s.java_script_arguments_depth == 0 && stream.peek() != Some('(') {
                s.java_script_arguments = false;
                return None;
            }
            if stream.peek() == Some('(') {
                s.java_script_arguments_depth += 1;
            } else if stream.peek() == Some(')') {
                s.java_script_arguments_depth -= 1;
            }
            if s.java_script_arguments_depth == 0 {
                s.java_script_arguments = false;
                return None;
            }
            return or_true(self.js_token(stream, s));
        }
        None
    }

    /// `interpolation`
    fn interpolation(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if stream.match_str("#{", true, false) {
            s.is_interpolating = true;
            s.interpolation_nesting = 0;
            return style("punctuation");
        }
        None
    }

    /// `interpolationContinued`
    fn interpolation_continued(&self, stream: &mut StringStream, s: &mut PugState) -> Tok {
        if s.is_interpolating {
            if stream.peek() == Some('}') {
                s.interpolation_nesting -= 1;
                if s.interpolation_nesting < 0 {
                    stream.next();
                    s.is_interpolating = false;
                    return style("punctuation");
                }
            } else if stream.peek() == Some('{') {
                s.interpolation_nesting += 1;
            }
            return or_true(self.js_token(stream, s));
        }
        None
    }

    /// `includeFilteredContinued`
    fn include_filtered_continued(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if s.is_include_filtered {
            let tok = Self::filter(stream, s);
            s.is_include_filtered = false;
            s.rest_of_line = Some("string");
            return tok;
        }
        None
    }

    /// `call`
    fn call(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if stream.matches(re!(r"^\+([-A-Za-z0-9_]+)")) {
            if stream
                .match_re(re!(r"^\( *[-A-Za-z0-9_]+ *="), false)
                .is_none()
            {
                s.java_script_arguments = true;
                s.java_script_arguments_depth = 0;
            }
            return style("variable");
        }
        if stream.match_str("+#{", false, false) {
            stream.next();
            s.mixin_call_after = true;
            return Self::interpolation(stream, s);
        }
        None
    }

    /// `callArguments`
    fn call_arguments(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if s.mixin_call_after {
            s.mixin_call_after = false;
            if stream
                .match_re(re!(r"^\( *[-A-Za-z0-9_]+ *="), false)
                .is_none()
            {
                s.java_script_arguments = true;
                s.java_script_arguments_depth = 0;
            }
            return Some(None);
        }
        None
    }

    /// `eachContinued`
    fn each_continued(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if s.is_each {
            if stream.matches(re!(r"^ in(?![A-Za-z0-9_])")) {
                s.java_script_line = true;
                s.is_each = false;
                return style("keyword");
            } else if stream.sol() || stream.eol() {
                s.is_each = false;
            } else if stream.next().is_some() {
                while stream
                    .match_re(re!(r"^ in(?![A-Za-z0-9_])"), false)
                    .is_none()
                    && stream.next().is_some()
                {}
                return style("variable");
            }
        }
        None
    }

    /// `tag`
    fn tag(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if let Some(m) = stream.match_re(
            re!(r"^([A-Za-z0-9_](?:[-:A-Za-z0-9_]*[A-Za-z0-9_])?)\/?"),
            true,
        ) {
            s.last_tag = m.group(1).unwrap_or("").to_lowercase();
            if s.last_tag == "script" {
                s.script_type = "application/javascript".to_string();
            }
            return style("tag");
        }
        None
    }

    /// `filter`
    fn filter(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if let Some(m) = stream.match_re(re!(r"^:([A-Za-z0-9_\-]+)"), true) {
            // getMode(config, name), then setInnerMode with the mode object
            let name = m.group(1).unwrap_or("");
            let spec = match super::html_modes(name) {
                Some(mode) => Spec::Mode(mode),
                None => Spec::Null,
            };
            Self::set_inner_mode(stream, s, spec);
            return style("atom");
        }
        None
    }

    /// `attrs`
    fn attrs(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if stream.peek() == Some('(') {
            stream.next();
            s.is_attrs = true;
            s.attrs_nest.clear();
            s.in_attribute_name = true;
            s.attr_value.clear();
            s.attribute_is_type = false;
            return style("punctuation");
        }
        None
    }

    /// `attrsContinued`
    fn attrs_continued(&self, stream: &mut StringStream, s: &mut PugState) -> Tok {
        if !s.is_attrs {
            return None;
        }
        let peek = stream.peek();
        if let Some(close) = peek.and_then(|c| match c {
            '{' => Some('}'),
            '(' => Some(')'),
            '[' => Some(']'),
            _ => None,
        }) {
            s.attrs_nest.push(close);
        }
        if s.attrs_nest.last().copied() == peek {
            s.attrs_nest.pop();
        } else if stream.eat(')').is_some() {
            s.is_attrs = false;
            return style("punctuation");
        }
        if s.in_attribute_name && stream.matches(re!(r"^[^=,\)!]+")) {
            if matches!(stream.peek(), Some('=' | '!')) {
                s.in_attribute_name = false;
                // CodeMirror.startState(jsMode); the instance scratch stays
                let scratch = s.js_state.scratch.clone();
                s.js_state = self.js.start_js();
                s.js_state.scratch = scratch;
                s.attribute_is_type =
                    s.last_tag == "script" && stream.current().trim().to_lowercase() == "type";
            }
            return style("attribute");
        }
        let tok = self.js_token(stream, s);
        if s.attribute_is_type && tok == Some("string") {
            s.script_type = stream.current().to_string();
        }
        if s.attrs_nest.is_empty() && matches!(tok, Some("string" | "variable" | "keyword")) {
            let value = s.attr_value.as_str();
            // attrValue.replace(/,\s*$/, '').replace(/^!/, '')
            let value = match value.rfind(',') {
                Some(i) if value[i + 1..].chars().all(char::is_whitespace) => &value[..i],
                _ => value,
            };
            let value = value.strip_prefix('!').unwrap_or(value);
            if var_x_parses(value) {
                s.in_attribute_name = true;
                s.attr_value.clear();
                let n = js_len(stream.current());
                stream.back_up(n);
                return self.attrs_continued(stream, s);
            }
        }
        s.attr_value.push_str(stream.current());
        or_true(tok)
    }

    /// `attributesBlock`
    fn attributes_block(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if stream.matches(re!(r"^&attributes(?![A-Za-z0-9_])")) {
            s.java_script_arguments = true;
            s.java_script_arguments_depth = 0;
            return style("keyword");
        }
        None
    }

    /// `text`
    fn text(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if stream.matches(re!(r"^(?:\| ?| )([^\n]+)")) {
            return style("string");
        }
        if stream.match_re(re!(r"^(<[^\n]*)"), false).is_some() {
            // html string
            Self::set_inner_mode(stream, s, Spec::Name("htmlmixed"));
            s.inner_mode_for_line = true;
            return Self::inner_mode(stream, s, true);
        }
        None
    }

    /// `dot`
    fn dot(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if stream.eat('.').is_some() {
            let script = s.script_type.to_lowercase();
            let inner = if s.last_tag == "script" && script.contains("javascript") {
                Some(script.replace(['"', '\''], ""))
            } else if s.last_tag == "style" {
                Some("css".to_string())
            } else {
                None
            };
            match &inner {
                Some(name) => Self::set_inner_mode(stream, s, Spec::Name(name)),
                None => Self::set_inner_mode(stream, s, Spec::Null),
            }
            return style("dot");
        }
        None
    }

    /// `setInnerMode`
    fn set_inner_mode(stream: &mut StringStream, s: &mut PugState, spec: Spec) {
        // mimeModes lookups + getMode, limited to the modes pug.js loads
        let mode = match spec {
            Spec::Name(name) => super::html_modes(name),
            Spec::Mode(mode) => Some(mode),
            Spec::Null => None,
        };
        s.indent_of = Some(stream.indentation());
        match mode {
            Some(mode) => s.inner_mode = Some(mode),
            None => s.indent_token = Some("string"),
        }
    }

    /// `innerMode`
    fn inner_mode(stream: &mut StringStream, s: &mut PugState, force: bool) -> Tok {
        let deeper = s.indent_of.is_some_and(|of| stream.indentation() > of);
        if deeper || (s.inner_mode_for_line && !stream.sol()) || force {
            if let Some(mode) = s.inner_mode.clone() {
                let inner = s.inner_state.get_or_insert_with(|| mode.start_state());
                let hide = s.indent_of.unwrap_or(0) + 2;
                let t = stream.hide_first_chars(hide, |stream| mode.token(stream, &mut **inner));
                return Some(t.map(Cow::Owned));
            }
            stream.skip_to_end();
            return s.indent_token.map(|t| Some(Cow::Borrowed(t)));
        } else if stream.sol() {
            s.indent_of = None;
            s.indent_token = None;
            s.inner_mode = None;
            s.inner_state = None;
        }
        None
    }

    /// `restOfLine`
    fn rest_of_line(stream: &mut StringStream, s: &mut PugState) -> Tok {
        if stream.sol() {
            s.rest_of_line = None;
        }
        if let Some(tok) = s.rest_of_line.take() {
            stream.skip_to_end();
            return style(tok);
        }
        None
    }

    /// `nextToken`
    fn next_token(&self, stream: &mut StringStream, s: &mut PugState) -> Tok {
        macro_rules! first {
            ($($e:expr),* $(,)?) => {{
                $( let t = $e; if t.is_some() { return t; } )*
                None
            }};
        }
        first!(
            Self::inner_mode(stream, s, false),
            Self::rest_of_line(stream, s),
            self.interpolation_continued(stream, s),
            Self::include_filtered_continued(stream, s),
            Self::each_continued(stream, s),
            self.attrs_continued(stream, s),
            self.java_script(stream, s),
            self.java_script_arguments(stream, s),
            Self::call_arguments(stream, s),
            // yieldStatement
            stream
                .matches(re!(r"^yield(?![A-Za-z0-9_])"))
                .then_some(Some(Cow::Borrowed("keyword"))),
            // doctype
            stream
                .matches(re!(r"^(?:doctype) *([^\n]+)?"))
                .then_some(Some(Cow::Borrowed("meta"))),
            Self::interpolation(stream, s),
            // caseStatement
            Self::js_line_keyword(stream, s, re!(r"^case(?![A-Za-z0-9_])"), false),
            // when
            Self::js_line_keyword(stream, s, re!(r"^when(?![A-Za-z0-9_])"), true),
            // defaultStatement
            stream
                .matches(re!(r"^default(?![A-Za-z0-9_])"))
                .then_some(Some(Cow::Borrowed("keyword"))),
            // extendsStatement
            Self::rest_keyword(stream, s, re!(r"^extends?(?![A-Za-z0-9_])"), "string"),
            // append
            Self::rest_keyword(stream, s, re!(r"^append(?![A-Za-z0-9_])"), "variable"),
            // prepend
            Self::rest_keyword(stream, s, re!(r"^prepend(?![A-Za-z0-9_])"), "variable"),
            // block
            Self::rest_keyword(
                stream,
                s,
                re!(r"^block(?![A-Za-z0-9_]) *(?:(prepend|append)(?![A-Za-z0-9_]))?"),
                "variable"
            ),
            // include
            Self::rest_keyword(stream, s, re!(r"^include(?![A-Za-z0-9_])"), "string"),
            // includeFiltered
            if stream
                .match_re(re!(r"^include:([a-zA-Z0-9\-]+)"), false)
                .is_some()
                && stream.match_str("include", true, false)
            {
                s.is_include_filtered = true;
                style("keyword")
            } else {
                None
            },
            // mixin
            Self::js_line_keyword(stream, s, re!(r"^mixin(?![A-Za-z0-9_])"), false),
            Self::call(stream, s),
            // conditional
            Self::js_line_keyword(
                stream,
                s,
                re!(r"^(if|unless|else if|else)(?![A-Za-z0-9_])"),
                false
            ),
            // each
            if stream.matches(re!(r"^(- *)?(each|for)(?![A-Za-z0-9_])")) {
                s.is_each = true;
                style("keyword")
            } else {
                None
            },
            // whileStatement
            Self::js_line_keyword(stream, s, re!(r"^while(?![A-Za-z0-9_])"), false),
            Self::tag(stream, s),
            Self::filter(stream, s),
            // code
            if stream.matches(re!(r"^(!?=|-)")) {
                s.java_script_line = true;
                style("punctuation")
            } else {
                None
            },
            // id
            stream
                .matches(re!(r"^#([A-Za-z0-9_-]+)"))
                .then_some(Some(Cow::Borrowed("builtin"))),
            // className
            stream
                .matches(re!(r"^\.([A-Za-z0-9_-]+)"))
                .then_some(Some(Cow::Borrowed("qualifier"))),
            Self::attrs(stream, s),
            Self::attributes_block(stream, s),
            // indent
            (stream.sol() && stream.eat_space()).then_some(Some(Cow::Borrowed("indent"))),
            Self::text(stream, s),
            // comment
            if stream.matches(re!(r"^ *\/\/(-)?([^\n]*)")) {
                s.indent_of = Some(stream.indentation());
                s.indent_token = Some("comment");
                style("comment")
            } else {
                None
            },
            // colon
            stream
                .matches(re!(r"^: *"))
                .then_some(Some(Cow::Borrowed("colon"))),
            Self::dot(stream, s),
            // fail
            {
                stream.next();
                Some(None)
            },
        )
    }

    /// `caseStatement` / `when` / `mixin` / `conditional` /
    /// `whileStatement`: a keyword that starts a javascript line.
    fn js_line_keyword(
        stream: &mut StringStream,
        s: &mut PugState,
        re: &fancy_regex::Regex,
        excludes_colon: bool,
    ) -> Tok {
        if stream.matches(re) {
            s.java_script_line = true;
            if excludes_colon {
                s.java_script_line_excludes_colon = true;
            }
            return style("keyword");
        }
        None
    }

    /// `extendsStatement` / `append` / `prepend` / `block` / `include`: a
    /// keyword whose line rest gets one style.
    fn rest_keyword(
        stream: &mut StringStream,
        s: &mut PugState,
        re: &fancy_regex::Regex,
        rest: &'static str,
    ) -> Tok {
        if stream.matches(re) {
            s.rest_of_line = Some(rest);
            return style("keyword");
        }
        None
    }
}

impl Mode for Pug {
    fn name(&self) -> &'static str {
        "pug"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PugState {
            java_script_line: false,
            java_script_line_excludes_colon: false,
            java_script_arguments: false,
            java_script_arguments_depth: 0,
            is_interpolating: false,
            interpolation_nesting: 0,
            js_state: self.js.start_js(),
            rest_of_line: None,
            is_include_filtered: false,
            is_each: false,
            last_tag: String::new(),
            script_type: String::new(),
            is_attrs: false,
            attrs_nest: Vec::new(),
            in_attribute_name: true,
            attribute_is_type: false,
            attr_value: String::new(),
            indent_of: None,
            indent_token: None,
            inner_mode: None,
            inner_state: None,
            inner_mode_for_line: false,
            mixin_call_after: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<PugState>(st);
        // `tok === true ? null : tok`
        self.next_token(stream, s).flatten().map(Cow::into_owned)
    }
}

/// `CodeMirror.getMode({}, "text/x-pug")`
pub fn pug() -> Arc<dyn Mode> {
    static MODE: std::sync::OnceLock<Arc<dyn Mode>> = std::sync::OnceLock::new();
    MODE.get_or_init(|| Arc::new(Pug::new())).clone()
}
