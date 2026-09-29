//! `codemirror/mode/css/css.js`, ported line by line: the `css` mode with
//! the four MIME configurations the file defines (`text/css`,
//! `text/x-scss`, `text/x-less`, `text/x-gss`), each with its keyword sets
//! and `tokenHooks`. [`css`] is public so nesting modes (htmlmixed) can run
//! it, `inline: true` included.
//!
//! JS's closure variables `type` and `override` are per mode instance; the
//! persistent `type` lives in the state here (a token whose tokenizer
//! returns `undefined` keeps the previous `type`, exactly like the JS).
//! `highlightNonStandardPropertyKeywords` is an editor option GHD never
//! sets, so it is always on.

use std::borrow::Cow;
use std::sync::Arc;

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

/// The `defineMIME` configurations of css.js.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssDialect {
    /// `text/css` (also what `getMode("css")` without keyword sets resolves to)
    Css,
    /// `text/x-scss`
    Scss,
    /// `text/x-less`
    Less,
    /// `text/x-gss`
    Gss,
}

impl CssDialect {
    pub fn for_mime(mime: &str) -> Option<Self> {
        Some(match mime {
            "text/css" => Self::Css,
            "text/x-scss" => Self::Scss,
            "text/x-less" => Self::Less,
            "text/x-gss" => Self::Gss,
            _ => return None,
        })
    }
}

/// `parserConfig` after `resolveMode`.
struct Config {
    dialect: CssDialect,
    inline: bool,
    document_types: &'static [&'static str],
    media_types: &'static [&'static str],
    media_features: &'static [&'static str],
    media_value_keywords: &'static [&'static str],
    property_keywords: &'static [&'static str],
    non_standard_property_keywords: &'static [&'static str],
    font_properties: &'static [&'static str],
    counter_descriptors: &'static [&'static str],
    color_keywords: &'static [&'static str],
    value_keywords: &'static [&'static str],
    allow_nested: bool,
    supports_at_component: bool,
}

/// `CodeMirror.getMode({}, {name: <dialect's MIME>, inline})`.
pub fn css(dialect: CssDialect, inline: bool) -> Arc<dyn Mode> {
    use CssDialect::*;
    let (scss_less, gss) = (matches!(dialect, Scss | Less), dialect == Gss);
    Arc::new(CssMode(Config {
        dialect,
        inline,
        document_types: if scss_less { &[] } else { DOCUMENT_TYPES },
        media_types: MEDIA_TYPES,
        media_features: MEDIA_FEATURES,
        media_value_keywords: if gss { &[] } else { MEDIA_VALUE_KEYWORDS },
        property_keywords: PROPERTY_KEYWORDS,
        non_standard_property_keywords: NON_STANDARD_PROPERTY_KEYWORDS,
        font_properties: FONT_PROPERTIES,
        counter_descriptors: if scss_less { &[] } else { COUNTER_DESCRIPTORS },
        color_keywords: COLOR_KEYWORDS,
        value_keywords: VALUE_KEYWORDS,
        allow_nested: scss_less,
        supports_at_component: gss,
    }))
}

/// `CodeMirror.getMode({}, mime)` for the css.js MIME types.
pub fn css_for_mime(mime: &str) -> Option<Arc<dyn Mode>> {
    CssDialect::for_mime(mime).map(|d| css(d, false))
}

/// `keys.hasOwnProperty(word.toLowerCase())` on a lower-cased sorted set.
fn has_ci(set: &[&str], word: &str) -> bool {
    // words are ASCII (`[\w\\\-]`), so ASCII lower-casing is `toLowerCase`
    set.binary_search_by(|k| k.bytes().cmp(word.bytes().map(|b| b.to_ascii_lowercase())))
        .is_ok()
}

/// `keys.hasOwnProperty(word)` (no lower-casing).
fn has(set: &[&str], word: &str) -> bool {
    set.binary_search(&word).is_ok()
}

/// `state.tokenize`
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tokenize {
    Base,
    /// `tokenString(quote)`
    Str(char),
    Parenthesized,
    CComment,
}

/// Names of `states` / context types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Top,
    Block,
    Maybeprop,
    Prop,
    PropBlock,
    Parens,
    Pseudo,
    DocumentTypes,
    AtBlock,
    AtComponentBlock,
    AtBlockParens,
    RestrictedAtBlockBefore,
    RestrictedAtBlock,
    Keyframes,
    At,
    Interpolation,
}

/// `state.stateArg`, only ever compared to these two.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StateArg {
    None,
    FontFace,
    CounterStyle,
    Other,
}

/// A token type (`ret(style, tp)`); `None` is JS `null` / `undefined`.
type Ty = Option<Cow<'static, str>>;

/// A tokenizer's style: a string, `null`, or `undefined` (only the `-`
/// branch of `tokenBase` that matches nothing).
#[derive(Clone)]
enum Style {
    Undefined,
    Null,
    S(Cow<'static, str>),
}

impl Style {
    /// `override += " error"`
    fn add_error(&mut self) {
        let base: &str = match self {
            Style::Undefined => "undefined",
            Style::Null => "null",
            Style::S(s) => s,
        };
        *self = Style::S(Cow::Owned(format!("{base} error")));
    }
}

fn s(style: &'static str) -> Style {
    Style::S(Cow::Borrowed(style))
}

#[derive(Clone)]
struct CssState {
    tokenize: Tokenize,
    state: St,
    state_arg: StateArg,
    /// the `Context` chain, innermost last (indentation is not needed)
    context: Vec<St>,
    /// the mode closure's `type`
    ty: Ty,
}

/// Carry the mode closure's `type` from a finished block's state into a
/// fresh one, for nesting modes that run one css instance over several
/// blocks (slim's cached `modes[…]`).
pub(crate) fn carry_type(prev: &dyn ModeState, next: &mut dyn ModeState) {
    if let (Some(prev), Some(next)) = (
        prev.as_any().downcast_ref::<CssState>(),
        next.as_any_mut().downcast_mut::<CssState>(),
    ) {
        next.ty = prev.ty.clone();
    }
}

impl CssState {
    fn cx(&self) -> St {
        self.context.last().copied().unwrap_or(St::Top)
    }
}

struct CssMode(Config);

/// `/[\w\\\-]/`
fn word_ch(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '\\' || c == '-'
}

/// `ret(style, tp)`
fn ret(st: &mut CssState, style: Option<&'static str>, tp: Option<&'static str>) -> Style {
    st.ty = tp.map(Cow::Borrowed);
    style.map_or(Style::Null, s)
}

fn ty_is(ty: &Ty, t: &str) -> bool {
    ty.as_deref() == Some(t)
}

impl CssMode {
    // Tokenizers

    /// `tokenHooks[ch]`; `None` = the hook returned `false` (or none).
    fn token_hook(&self, ch: char, stream: &mut StringStream, st: &mut CssState) -> Option<Style> {
        use CssDialect::*;
        let d = self.0.dialect;
        match (d, ch) {
            (Css | Gss, '/') => {
                stream.eat('*')?;
                st.tokenize = Tokenize::CComment;
                Some(token_c_comment(stream, st))
            }
            (Scss | Less, '/') => {
                if stream.eat('/').is_some() {
                    stream.skip_to_end();
                    st.ty = Some(Cow::Borrowed("comment"));
                    Some(s("comment"))
                } else if stream.eat('*').is_some() {
                    st.tokenize = Tokenize::CComment;
                    Some(token_c_comment(stream, st))
                } else {
                    st.ty = Some(Cow::Borrowed("operator"));
                    Some(s("operator"))
                }
            }
            (Scss, ':') => {
                if stream.match_re(re!(r"^\s*\{"), false).is_some() {
                    st.ty = None;
                    return Some(Style::Null);
                }
                None
            }
            (Scss, '$') => {
                stream.matches(re!(r"^[A-Za-z0-9_-]+"));
                if stream.match_re(re!(r"^\s*:"), false).is_some() {
                    st.ty = Some(Cow::Borrowed("variable-definition"));
                } else {
                    st.ty = Some(Cow::Borrowed("variable"));
                }
                Some(s("variable-2"))
            }
            (Scss, '#') => {
                stream.eat('{')?;
                st.ty = Some(Cow::Borrowed("interpolation"));
                Some(Style::Null)
            }
            (Less, '@') => {
                if stream.eat('{').is_some() {
                    st.ty = Some(Cow::Borrowed("interpolation"));
                    return Some(Style::Null);
                }
                if stream
                    .match_re(
                        re!(
                            r"^(?i:charset|document|font-face|import|(-(moz|ms|o|webkit)-)?keyframes|media|namespace|page|supports)(?![A-Za-z0-9_])"
                        ),
                        false,
                    )
                    .is_some()
                {
                    return None;
                }
                stream.eat_while_if(word_ch);
                if stream.match_re(re!(r"^\s*:"), false).is_some() {
                    st.ty = Some(Cow::Borrowed("variable-definition"));
                } else {
                    st.ty = Some(Cow::Borrowed("variable"));
                }
                Some(s("variable-2"))
            }
            (Less, '&') => {
                st.ty = Some(Cow::Borrowed("atom"));
                Some(s("atom"))
            }
            _ => None,
        }
    }

    /// `tokenBase`
    fn token_base(&self, stream: &mut StringStream, st: &mut CssState) -> Style {
        let Some(ch) = stream.next() else {
            return ret(st, None, None);
        };
        if let Some(result) = self.token_hook(ch, stream, st) {
            return result;
        }
        if ch == '@' {
            stream.eat_while_if(word_ch);
            let style = s("def");
            st.ty = Some(Cow::Owned(stream.current().to_string()));
            style
        } else if ch == '=' || (ch == '~' || ch == '|') && stream.eat('=').is_some() {
            ret(st, None, Some("compare"))
        } else if ch == '"' || ch == '\'' {
            st.tokenize = Tokenize::Str(ch);
            token_string(ch, stream, st)
        } else if ch == '#' {
            stream.eat_while_if(word_ch);
            ret(st, Some("atom"), Some("hash"))
        } else if ch == '!' {
            stream.matches(re!(r"^\s*[A-Za-z0-9_]*"));
            ret(st, Some("keyword"), Some("important"))
        } else if ch.is_ascii_digit()
            || ch == '.' && stream.eat_if(|c| c.is_ascii_digit()).is_some()
        {
            stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '%');
            ret(st, Some("number"), Some("unit"))
        } else if ch == '-' {
            if stream
                .peek()
                .is_some_and(|c| c.is_ascii_digit() || c == '.')
            {
                stream.eat_while_if(|c| {
                    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '%'
                });
                ret(st, Some("number"), Some("unit"))
            } else if stream.matches(re!(r"^-[A-Za-z0-9_\\\-]*")) {
                stream.eat_while_if(word_ch);
                if stream.match_re(re!(r"^\s*:"), false).is_some() {
                    return ret(st, Some("variable-2"), Some("variable-definition"));
                }
                ret(st, Some("variable-2"), Some("variable"))
            } else if stream.matches(re!(r"^[A-Za-z0-9_]+-")) {
                ret(st, Some("meta"), Some("meta"))
            } else {
                // falls off the end of tokenBase: `undefined`, `type` kept
                Style::Undefined
            }
        } else if matches!(ch, ',' | '+' | '>' | '*' | '/') {
            ret(st, None, Some("select-op"))
        } else if ch == '.' && stream.matches(re!(r"^-?[_a-zA-Z][_a-zA-Z0-9-]*")) {
            ret(st, Some("qualifier"), Some("qualifier"))
        } else if let Some(tp) = match ch {
            ':' => Some(":"),
            ';' => Some(";"),
            '{' => Some("{"),
            '}' => Some("}"),
            '[' => Some("["),
            ']' => Some("]"),
            '(' => Some("("),
            ')' => Some(")"),
            _ => None,
        } {
            ret(st, None, Some(tp))
        } else if stream.matches(re!(r"^[A-Za-z0-9_.-]+(?=\()")) {
            let cur = stream.current();
            if ["url", "url-prefix", "domain", "regexp"]
                .iter()
                .any(|w| cur.eq_ignore_ascii_case(w))
            {
                st.tokenize = Tokenize::Parenthesized;
            }
            ret(st, Some("variable callee"), Some("variable"))
        } else if word_ch(ch) {
            stream.eat_while_if(word_ch);
            ret(st, Some("property"), Some("word"))
        } else {
            ret(st, None, None)
        }
    }

    // Context management

    /// `pushContext`
    fn push(st: &mut CssState, tp: St) -> St {
        st.context.push(tp);
        tp
    }

    /// `popContext`
    fn pop(st: &mut CssState) -> St {
        if st.context.len() > 1 {
            st.context.pop();
        }
        st.cx()
    }

    /// `pass`
    fn pass(&self, stream: &mut StringStream, st: &mut CssState, ov: &mut Style) -> St {
        self.step(st.cx(), stream, st, ov)
    }

    /// `popAndPass` (JS would crash popping the root context; it stays)
    fn pop_and_pass(
        &self,
        stream: &mut StringStream,
        st: &mut CssState,
        ov: &mut Style,
        n: usize,
    ) -> St {
        for _ in 0..n {
            if st.context.len() > 1 {
                st.context.pop();
            }
        }
        self.pass(stream, st, ov)
    }

    // Parser

    /// `wordAsValue`
    fn word_as_value(&self, stream: &StringStream, ov: &mut Style) {
        let word = stream.current();
        *ov = if has_ci(self.0.value_keywords, word) {
            s("atom")
        } else if has_ci(self.0.color_keywords, word) {
            s("keyword")
        } else {
            s("variable")
        };
    }

    /// `states[name](type, stream, state)`
    fn step(&self, name: St, stream: &mut StringStream, st: &mut CssState, ov: &mut Style) -> St {
        let c = &self.0;
        let ty = st.ty.clone();
        let t = ty.as_deref();
        match name {
            St::Top => {
                let at = t.is_some_and(|t| t.starts_with('@'));
                if t == Some("{") {
                    Self::push(st, St::Block)
                } else if t == Some("}") && st.context.len() > 1 {
                    Self::pop(st)
                } else if c.supports_at_component
                    && at
                    && re!(r"(?i)@component")
                        .is_match(t.unwrap_or(""))
                        .unwrap_or(false)
                {
                    Self::push(st, St::AtComponentBlock)
                } else if at
                    && re!(r"(?i)^@(-moz-)?document$")
                        .is_match(t.unwrap_or(""))
                        .unwrap_or(false)
                {
                    Self::push(st, St::DocumentTypes)
                } else if at
                    && re!(r"(?i)^@(media|supports|(-moz-)?document|import)$")
                        .is_match(t.unwrap_or(""))
                        .unwrap_or(false)
                {
                    Self::push(st, St::AtBlock)
                } else if at
                    && re!(r"(?i)^@(font-face|counter-style)")
                        .is_match(t.unwrap_or(""))
                        .unwrap_or(false)
                {
                    st.state_arg = match t {
                        Some("@font-face") => StateArg::FontFace,
                        Some("@counter-style") => StateArg::CounterStyle,
                        _ => StateArg::Other,
                    };
                    St::RestrictedAtBlockBefore
                } else if at
                    && re!(r"(?i)^@(-(moz|ms|o|webkit)-)?keyframes$")
                        .is_match(t.unwrap_or(""))
                        .unwrap_or(false)
                {
                    St::Keyframes
                } else if at {
                    Self::push(st, St::At)
                } else if t == Some("hash") {
                    *ov = s("builtin");
                    st.cx()
                } else if t == Some("word") {
                    *ov = s("tag");
                    st.cx()
                } else if t == Some("variable-definition") {
                    St::Maybeprop
                } else if t == Some("interpolation") {
                    Self::push(st, St::Interpolation)
                } else if t == Some(":") {
                    St::Pseudo
                } else if c.allow_nested && t == Some("(") {
                    Self::push(st, St::Parens)
                } else {
                    st.cx()
                }
            }

            St::Block => {
                if t == Some("word") {
                    let word = stream.current();
                    let (prop, non_std) = (
                        has_ci(c.property_keywords, word),
                        has_ci(c.non_standard_property_keywords, word),
                    );
                    if prop {
                        *ov = s("property");
                        St::Maybeprop
                    } else if non_std {
                        *ov = s("string-2");
                        St::Maybeprop
                    } else if c.allow_nested {
                        *ov = if stream.match_re(re!(r"^\s*:(?:\s|$)"), false).is_some() {
                            s("property")
                        } else {
                            s("tag")
                        };
                        St::Block
                    } else {
                        ov.add_error();
                        St::Maybeprop
                    }
                } else if t == Some("meta") {
                    St::Block
                } else if !c.allow_nested && (t == Some("hash") || t == Some("qualifier")) {
                    *ov = s("error");
                    St::Block
                } else {
                    self.step(St::Top, stream, st, ov)
                }
            }

            St::Maybeprop => {
                if t == Some(":") {
                    return Self::push(st, St::Prop);
                }
                self.pass(stream, st, ov)
            }

            St::Prop => {
                if t == Some(";") {
                    return Self::pop(st);
                }
                if t == Some("{") && c.allow_nested {
                    return Self::push(st, St::PropBlock);
                }
                if t == Some("}") || t == Some("{") {
                    return self.pop_and_pass(stream, st, ov, 1);
                }
                if t == Some("(") {
                    return Self::push(st, St::Parens);
                }
                if t == Some("hash")
                    && !re!(r"^#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$")
                        .is_match(stream.current())
                        .unwrap_or(false)
                {
                    ov.add_error();
                } else if t == Some("word") {
                    self.word_as_value(stream, ov);
                } else if t == Some("interpolation") {
                    return Self::push(st, St::Interpolation);
                }
                St::Prop
            }

            St::PropBlock => {
                if t == Some("}") {
                    return Self::pop(st);
                }
                if t == Some("word") {
                    *ov = s("property");
                    return St::Maybeprop;
                }
                st.cx()
            }

            St::Parens => {
                if t == Some("{") || t == Some("}") {
                    return self.pop_and_pass(stream, st, ov, 1);
                }
                if t == Some(")") {
                    return Self::pop(st);
                }
                if t == Some("(") {
                    return Self::push(st, St::Parens);
                }
                if t == Some("interpolation") {
                    return Self::push(st, St::Interpolation);
                }
                if t == Some("word") {
                    self.word_as_value(stream, ov);
                }
                St::Parens
            }

            St::Pseudo => {
                if t == Some("meta") {
                    return St::Pseudo;
                }
                if t == Some("word") {
                    *ov = s("variable-3");
                    return st.cx();
                }
                self.pass(stream, st, ov)
            }

            St::DocumentTypes => {
                if t == Some("word") && has(c.document_types, stream.current()) {
                    *ov = s("tag");
                    st.cx()
                } else {
                    self.step(St::AtBlock, stream, st, ov)
                }
            }

            St::AtBlock => {
                if t == Some("(") {
                    return Self::push(st, St::AtBlockParens);
                }
                if t == Some("}") || t == Some(";") {
                    return self.pop_and_pass(stream, st, ov, 1);
                }
                if t == Some("{") {
                    Self::pop(st);
                    return Self::push(st, if c.allow_nested { St::Block } else { St::Top });
                }
                if t == Some("interpolation") {
                    return Self::push(st, St::Interpolation);
                }
                if t == Some("word") {
                    let word = stream.current();
                    *ov = if ["only", "not", "and", "or"]
                        .iter()
                        .any(|w| word.eq_ignore_ascii_case(w))
                    {
                        s("keyword")
                    } else if has_ci(c.media_types, word) {
                        s("attribute")
                    } else if has_ci(c.media_features, word) {
                        s("property")
                    } else if has_ci(c.media_value_keywords, word) {
                        s("keyword")
                    } else if has_ci(c.property_keywords, word) {
                        s("property")
                    } else if has_ci(c.non_standard_property_keywords, word) {
                        s("string-2")
                    } else if has_ci(c.value_keywords, word) {
                        s("atom")
                    } else if has_ci(c.color_keywords, word) {
                        s("keyword")
                    } else {
                        s("error")
                    };
                }
                st.cx()
            }

            St::AtComponentBlock => {
                if t == Some("}") {
                    return self.pop_and_pass(stream, st, ov, 1);
                }
                if t == Some("{") {
                    Self::pop(st);
                    return Self::push(st, if c.allow_nested { St::Block } else { St::Top });
                }
                if t == Some("word") {
                    *ov = s("error");
                }
                st.cx()
            }

            St::AtBlockParens => {
                if t == Some(")") {
                    return Self::pop(st);
                }
                if t == Some("{") || t == Some("}") {
                    return self.pop_and_pass(stream, st, ov, 2);
                }
                self.step(St::AtBlock, stream, st, ov)
            }

            St::RestrictedAtBlockBefore => {
                if t == Some("{") {
                    return Self::push(st, St::RestrictedAtBlock);
                }
                if t == Some("word") && st.state_arg == StateArg::CounterStyle {
                    *ov = s("variable");
                    return St::RestrictedAtBlockBefore;
                }
                self.pass(stream, st, ov)
            }

            St::RestrictedAtBlock => {
                if t == Some("}") {
                    st.state_arg = StateArg::None;
                    return Self::pop(st);
                }
                if t == Some("word") {
                    let word = stream.current();
                    *ov = if (st.state_arg == StateArg::FontFace
                        && !has_ci(c.font_properties, word))
                        || (st.state_arg == StateArg::CounterStyle
                            && !has_ci(c.counter_descriptors, word))
                    {
                        s("error")
                    } else {
                        s("property")
                    };
                    return St::Maybeprop;
                }
                St::RestrictedAtBlock
            }

            St::Keyframes => {
                if t == Some("word") {
                    *ov = s("variable");
                    return St::Keyframes;
                }
                if t == Some("{") {
                    return Self::push(st, St::Top);
                }
                self.pass(stream, st, ov)
            }

            St::At => {
                if t == Some(";") {
                    return Self::pop(st);
                }
                if t == Some("{") || t == Some("}") {
                    return self.pop_and_pass(stream, st, ov, 1);
                }
                if t == Some("word") {
                    *ov = s("tag");
                } else if t == Some("hash") {
                    *ov = s("builtin");
                }
                St::At
            }

            St::Interpolation => {
                if t == Some("}") {
                    return Self::pop(st);
                }
                if t == Some("{") || t == Some(";") {
                    return self.pop_and_pass(stream, st, ov, 1);
                }
                if t == Some("word") {
                    *ov = s("variable");
                } else if t != Some("variable") && t != Some("(") && t != Some(")") {
                    *ov = s("error");
                }
                St::Interpolation
            }
        }
    }
}

/// `tokenString(quote)`
fn token_string(quote: char, stream: &mut StringStream, st: &mut CssState) -> Style {
    let mut escaped = false;
    let mut ch;
    loop {
        ch = stream.next();
        let Some(c) = ch else { break };
        if c == quote && !escaped {
            if quote == ')' {
                stream.back_up(1);
            }
            break;
        }
        escaped = !escaped && c == '\\';
    }
    if ch == Some(quote) || !escaped && quote != ')' {
        st.tokenize = Tokenize::Base;
    }
    ret(st, Some("string"), Some("string"))
}

/// `tokenParenthesized`
fn token_parenthesized(stream: &mut StringStream, st: &mut CssState) -> Style {
    stream.next(); // Must be '('
    if stream.match_re(re!(r#"^\s*["')]"#), false).is_none() {
        st.tokenize = Tokenize::Str(')');
    } else {
        st.tokenize = Tokenize::Base;
    }
    ret(st, None, Some("("))
}

/// `tokenCComment`: `["comment", "comment"]`
fn token_c_comment(stream: &mut StringStream, st: &mut CssState) -> Style {
    let mut maybe_end = false;
    while let Some(ch) = stream.next() {
        if maybe_end && ch == '/' {
            st.tokenize = Tokenize::Base;
            break;
        }
        maybe_end = ch == '*';
    }
    st.ty = Some(Cow::Borrowed("comment"));
    s("comment")
}

impl Mode for CssMode {
    fn name(&self) -> &'static str {
        "css"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        let top = if self.0.inline { St::Block } else { St::Top };
        Box::new(CssState {
            tokenize: Tokenize::Base,
            state: top,
            state_arg: StateArg::None,
            context: vec![top],
            ty: None,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let st = state::<CssState>(st);
        if st.tokenize == Tokenize::Base && stream.eat_space() {
            return None;
        }
        let mut ov = match st.tokenize {
            Tokenize::Base => self.token_base(stream, st),
            Tokenize::Str(q) => token_string(q, stream, st),
            Tokenize::Parenthesized => token_parenthesized(stream, st),
            Tokenize::CComment => token_c_comment(stream, st),
        };
        if !ty_is(&st.ty, "comment") {
            st.state = self.step(st.state, stream, st, &mut ov);
        }
        match ov {
            Style::S(s) => Some(s.into_owned()),
            Style::Null | Style::Undefined => None,
        }
    }
}

/// `documentTypes_` (lower-cased, sorted, deduplicated: `keySet`)
const DOCUMENT_TYPES: &[&str] = &["domain", "regexp", "url", "url-prefix"];

/// `mediaTypes_` (lower-cased, sorted, deduplicated: `keySet`)
const MEDIA_TYPES: &[&str] = &[
    "all",
    "aural",
    "braille",
    "embossed",
    "handheld",
    "print",
    "projection",
    "screen",
    "tty",
    "tv",
];

/// `mediaFeatures_` (lower-cased, sorted, deduplicated: `keySet`)
const MEDIA_FEATURES: &[&str] = &[
    "any-hover",
    "any-pointer",
    "aspect-ratio",
    "color",
    "color-index",
    "device-aspect-ratio",
    "device-height",
    "device-pixel-ratio",
    "device-width",
    "dynamic-range",
    "grid",
    "height",
    "hover",
    "max-aspect-ratio",
    "max-color",
    "max-color-index",
    "max-device-aspect-ratio",
    "max-device-height",
    "max-device-pixel-ratio",
    "max-device-width",
    "max-height",
    "max-monochrome",
    "max-resolution",
    "max-width",
    "min-aspect-ratio",
    "min-color",
    "min-color-index",
    "min-device-aspect-ratio",
    "min-device-height",
    "min-device-pixel-ratio",
    "min-device-width",
    "min-height",
    "min-monochrome",
    "min-resolution",
    "min-width",
    "monochrome",
    "orientation",
    "pointer",
    "prefers-color-scheme",
    "resolution",
    "scan",
    "video-dynamic-range",
    "width",
];

/// `mediaValueKeywords_` (lower-cased, sorted, deduplicated: `keySet`)
const MEDIA_VALUE_KEYWORDS: &[&str] = &[
    "coarse",
    "dark",
    "fine",
    "high",
    "hover",
    "interlace",
    "landscape",
    "light",
    "none",
    "on-demand",
    "portrait",
    "progressive",
    "standard",
];

/// `propertyKeywords_` (lower-cased, sorted, deduplicated: `keySet`)
const PROPERTY_KEYWORDS: &[&str] = &[
    "align-content",
    "align-items",
    "align-self",
    "alignment-adjust",
    "alignment-baseline",
    "all",
    "anchor-point",
    "animation",
    "animation-delay",
    "animation-direction",
    "animation-duration",
    "animation-fill-mode",
    "animation-iteration-count",
    "animation-name",
    "animation-play-state",
    "animation-timing-function",
    "appearance",
    "azimuth",
    "backdrop-filter",
    "backface-visibility",
    "background",
    "background-attachment",
    "background-blend-mode",
    "background-clip",
    "background-color",
    "background-image",
    "background-origin",
    "background-position",
    "background-position-x",
    "background-position-y",
    "background-repeat",
    "background-size",
    "baseline-shift",
    "binding",
    "bleed",
    "block-size",
    "bookmark-label",
    "bookmark-level",
    "bookmark-state",
    "bookmark-target",
    "border",
    "border-bottom",
    "border-bottom-color",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
    "border-bottom-style",
    "border-bottom-width",
    "border-collapse",
    "border-color",
    "border-image",
    "border-image-outset",
    "border-image-repeat",
    "border-image-slice",
    "border-image-source",
    "border-image-width",
    "border-left",
    "border-left-color",
    "border-left-style",
    "border-left-width",
    "border-radius",
    "border-right",
    "border-right-color",
    "border-right-style",
    "border-right-width",
    "border-spacing",
    "border-style",
    "border-top",
    "border-top-color",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-top-style",
    "border-top-width",
    "border-width",
    "bottom",
    "box-decoration-break",
    "box-shadow",
    "box-sizing",
    "break-after",
    "break-before",
    "break-inside",
    "caption-side",
    "caret-color",
    "clear",
    "clip",
    "clip-path",
    "clip-rule",
    "color",
    "color-interpolation",
    "color-interpolation-filters",
    "color-profile",
    "color-rendering",
    "column-count",
    "column-fill",
    "column-gap",
    "column-rule",
    "column-rule-color",
    "column-rule-style",
    "column-rule-width",
    "column-span",
    "column-width",
    "columns",
    "contain",
    "content",
    "counter-increment",
    "counter-reset",
    "crop",
    "cue",
    "cue-after",
    "cue-before",
    "cursor",
    "direction",
    "display",
    "dominant-baseline",
    "drop-initial-after-adjust",
    "drop-initial-after-align",
    "drop-initial-before-adjust",
    "drop-initial-before-align",
    "drop-initial-size",
    "drop-initial-value",
    "elevation",
    "empty-cells",
    "enable-background",
    "fill",
    "fill-opacity",
    "fill-rule",
    "filter",
    "fit",
    "fit-content",
    "fit-position",
    "flex",
    "flex-basis",
    "flex-direction",
    "flex-flow",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "float",
    "float-offset",
    "flood-color",
    "flood-opacity",
    "flow-from",
    "flow-into",
    "font",
    "font-family",
    "font-feature-settings",
    "font-kerning",
    "font-language-override",
    "font-optical-sizing",
    "font-size",
    "font-size-adjust",
    "font-stretch",
    "font-style",
    "font-synthesis",
    "font-variant",
    "font-variant-alternates",
    "font-variant-caps",
    "font-variant-east-asian",
    "font-variant-ligatures",
    "font-variant-numeric",
    "font-variant-position",
    "font-variation-settings",
    "font-weight",
    "gap",
    "glyph-orientation-horizontal",
    "glyph-orientation-vertical",
    "grid",
    "grid-area",
    "grid-auto-columns",
    "grid-auto-flow",
    "grid-auto-rows",
    "grid-column",
    "grid-column-end",
    "grid-column-gap",
    "grid-column-start",
    "grid-gap",
    "grid-row",
    "grid-row-end",
    "grid-row-gap",
    "grid-row-start",
    "grid-template",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
    "hanging-punctuation",
    "height",
    "hyphens",
    "icon",
    "image-orientation",
    "image-rendering",
    "image-resolution",
    "inline-box-align",
    "inset",
    "inset-block",
    "inset-block-end",
    "inset-block-start",
    "inset-inline",
    "inset-inline-end",
    "inset-inline-start",
    "isolation",
    "justify-content",
    "justify-items",
    "justify-self",
    "left",
    "letter-spacing",
    "lighting-color",
    "line-break",
    "line-height",
    "line-height-step",
    "line-stacking",
    "line-stacking-ruby",
    "line-stacking-shift",
    "line-stacking-strategy",
    "list-style",
    "list-style-image",
    "list-style-position",
    "list-style-type",
    "margin",
    "margin-bottom",
    "margin-left",
    "margin-right",
    "margin-top",
    "marker",
    "marker-end",
    "marker-mid",
    "marker-start",
    "marks",
    "marquee-direction",
    "marquee-loop",
    "marquee-play-count",
    "marquee-speed",
    "marquee-style",
    "mask",
    "mask-clip",
    "mask-composite",
    "mask-image",
    "mask-mode",
    "mask-origin",
    "mask-position",
    "mask-repeat",
    "mask-size",
    "mask-type",
    "max-block-size",
    "max-height",
    "max-inline-size",
    "max-width",
    "min-block-size",
    "min-height",
    "min-inline-size",
    "min-width",
    "mix-blend-mode",
    "move-to",
    "nav-down",
    "nav-index",
    "nav-left",
    "nav-right",
    "nav-up",
    "object-fit",
    "object-position",
    "offset",
    "offset-anchor",
    "offset-distance",
    "offset-path",
    "offset-position",
    "offset-rotate",
    "opacity",
    "order",
    "orphans",
    "outline",
    "outline-color",
    "outline-offset",
    "outline-style",
    "outline-width",
    "overflow",
    "overflow-style",
    "overflow-wrap",
    "overflow-x",
    "overflow-y",
    "padding",
    "padding-bottom",
    "padding-left",
    "padding-right",
    "padding-top",
    "page",
    "page-break-after",
    "page-break-before",
    "page-break-inside",
    "page-policy",
    "paint-order",
    "pause",
    "pause-after",
    "pause-before",
    "perspective",
    "perspective-origin",
    "pitch",
    "pitch-range",
    "place-content",
    "place-items",
    "place-self",
    "play-during",
    "pointer-events",
    "position",
    "presentation-level",
    "punctuation-trim",
    "quotes",
    "region-break-after",
    "region-break-before",
    "region-break-inside",
    "region-fragment",
    "rendering-intent",
    "resize",
    "rest",
    "rest-after",
    "rest-before",
    "richness",
    "right",
    "rotate",
    "rotation",
    "rotation-point",
    "row-gap",
    "ruby-align",
    "ruby-overhang",
    "ruby-position",
    "ruby-span",
    "scale",
    "scroll-behavior",
    "scroll-margin",
    "scroll-margin-block",
    "scroll-margin-block-end",
    "scroll-margin-block-start",
    "scroll-margin-bottom",
    "scroll-margin-inline",
    "scroll-margin-inline-end",
    "scroll-margin-inline-start",
    "scroll-margin-left",
    "scroll-margin-right",
    "scroll-margin-top",
    "scroll-padding",
    "scroll-padding-block",
    "scroll-padding-block-end",
    "scroll-padding-block-start",
    "scroll-padding-bottom",
    "scroll-padding-inline",
    "scroll-padding-inline-end",
    "scroll-padding-inline-start",
    "scroll-padding-left",
    "scroll-padding-right",
    "scroll-padding-top",
    "scroll-snap-align",
    "scroll-snap-type",
    "shape-image-threshold",
    "shape-inside",
    "shape-margin",
    "shape-outside",
    "shape-rendering",
    "size",
    "speak",
    "speak-as",
    "speak-header",
    "speak-numeral",
    "speak-punctuation",
    "speech-rate",
    "stop-color",
    "stop-opacity",
    "stress",
    "string-set",
    "stroke",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "stroke-width",
    "tab-size",
    "table-layout",
    "target",
    "target-name",
    "target-new",
    "target-position",
    "text-align",
    "text-align-last",
    "text-anchor",
    "text-combine-upright",
    "text-decoration",
    "text-decoration-color",
    "text-decoration-line",
    "text-decoration-skip",
    "text-decoration-skip-ink",
    "text-decoration-style",
    "text-emphasis",
    "text-emphasis-color",
    "text-emphasis-position",
    "text-emphasis-style",
    "text-height",
    "text-indent",
    "text-justify",
    "text-orientation",
    "text-outline",
    "text-overflow",
    "text-rendering",
    "text-shadow",
    "text-size-adjust",
    "text-space-collapse",
    "text-transform",
    "text-underline-position",
    "text-wrap",
    "top",
    "touch-action",
    "transform",
    "transform-origin",
    "transform-style",
    "transition",
    "transition-delay",
    "transition-duration",
    "transition-property",
    "transition-timing-function",
    "translate",
    "unicode-bidi",
    "user-select",
    "vertical-align",
    "visibility",
    "voice-balance",
    "voice-duration",
    "voice-family",
    "voice-pitch",
    "voice-range",
    "voice-rate",
    "voice-stress",
    "voice-volume",
    "volume",
    "white-space",
    "widows",
    "width",
    "will-change",
    "word-break",
    "word-spacing",
    "word-wrap",
    "writing-mode",
    "z-index",
];

/// `nonStandardPropertyKeywords_` (lower-cased, sorted, deduplicated: `keySet`)
const NON_STANDARD_PROPERTY_KEYWORDS: &[&str] = &[
    "accent-color",
    "aspect-ratio",
    "border-block",
    "border-block-color",
    "border-block-end",
    "border-block-end-color",
    "border-block-end-style",
    "border-block-end-width",
    "border-block-start",
    "border-block-start-color",
    "border-block-start-style",
    "border-block-start-width",
    "border-block-style",
    "border-block-width",
    "border-inline",
    "border-inline-color",
    "border-inline-end",
    "border-inline-end-color",
    "border-inline-end-style",
    "border-inline-end-width",
    "border-inline-start",
    "border-inline-start-color",
    "border-inline-start-style",
    "border-inline-start-width",
    "border-inline-style",
    "border-inline-width",
    "content-visibility",
    "margin-block",
    "margin-block-end",
    "margin-block-start",
    "margin-inline",
    "margin-inline-end",
    "margin-inline-start",
    "overflow-anchor",
    "overscroll-behavior",
    "padding-block",
    "padding-block-end",
    "padding-block-start",
    "padding-inline",
    "padding-inline-end",
    "padding-inline-start",
    "scroll-snap-stop",
    "scrollbar-3d-light-color",
    "scrollbar-arrow-color",
    "scrollbar-base-color",
    "scrollbar-dark-shadow-color",
    "scrollbar-face-color",
    "scrollbar-highlight-color",
    "scrollbar-shadow-color",
    "scrollbar-track-color",
    "searchfield-cancel-button",
    "searchfield-decoration",
    "searchfield-results-button",
    "searchfield-results-decoration",
    "shape-inside",
    "zoom",
];

/// `fontProperties_` (lower-cased, sorted, deduplicated: `keySet`)
const FONT_PROPERTIES: &[&str] = &[
    "font-display",
    "font-family",
    "font-feature-settings",
    "font-stretch",
    "font-style",
    "font-variant",
    "font-weight",
    "src",
    "unicode-range",
];

/// `counterDescriptors_` (lower-cased, sorted, deduplicated: `keySet`)
const COUNTER_DESCRIPTORS: &[&str] = &[
    "additive-symbols",
    "fallback",
    "negative",
    "pad",
    "prefix",
    "range",
    "speak-as",
    "suffix",
    "symbols",
    "system",
];

/// `colorKeywords_` (lower-cased, sorted, deduplicated: `keySet`)
const COLOR_KEYWORDS: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

/// `valueKeywords_` (lower-cased, sorted, deduplicated: `keySet`)
const VALUE_KEYWORDS: &[&str] = &[
    "above",
    "absolute",
    "activeborder",
    "activecaption",
    "additive",
    "afar",
    "after-white-space",
    "ahead",
    "alias",
    "all",
    "all-scroll",
    "alphabetic",
    "alternate",
    "always",
    "amharic",
    "amharic-abegede",
    "antialiased",
    "appworkspace",
    "arabic-indic",
    "armenian",
    "asterisks",
    "attr",
    "auto",
    "auto-flow",
    "avoid",
    "avoid-column",
    "avoid-page",
    "avoid-region",
    "axis-pan",
    "background",
    "backwards",
    "baseline",
    "below",
    "bengali",
    "bidi-override",
    "binary",
    "blink",
    "block",
    "block-axis",
    "blur",
    "bold",
    "bolder",
    "border",
    "border-box",
    "both",
    "bottom",
    "break",
    "break-all",
    "break-word",
    "brightness",
    "bullets",
    "button",
    "buttonface",
    "buttonhighlight",
    "buttonshadow",
    "buttontext",
    "calc",
    "cambodian",
    "capitalize",
    "caps-lock-indicator",
    "caption",
    "captiontext",
    "caret",
    "cell",
    "center",
    "checkbox",
    "circle",
    "cjk-decimal",
    "cjk-earthly-branch",
    "cjk-heavenly-stem",
    "cjk-ideographic",
    "clear",
    "clip",
    "close-quote",
    "col-resize",
    "collapse",
    "color",
    "color-burn",
    "color-dodge",
    "column",
    "column-reverse",
    "compact",
    "condensed",
    "conic-gradient",
    "contain",
    "content",
    "content-box",
    "contents",
    "context-menu",
    "continuous",
    "contrast",
    "copy",
    "counter",
    "counters",
    "cover",
    "crop",
    "cross",
    "crosshair",
    "cubic-bezier",
    "currentcolor",
    "cursive",
    "cyclic",
    "darken",
    "dashed",
    "decimal",
    "decimal-leading-zero",
    "default",
    "default-button",
    "dense",
    "destination-atop",
    "destination-in",
    "destination-out",
    "destination-over",
    "devanagari",
    "difference",
    "disc",
    "discard",
    "disclosure-closed",
    "disclosure-open",
    "document",
    "dot-dash",
    "dot-dot-dash",
    "dotted",
    "double",
    "down",
    "drop-shadow",
    "e-resize",
    "ease",
    "ease-in",
    "ease-in-out",
    "ease-out",
    "element",
    "ellipse",
    "ellipsis",
    "embed",
    "end",
    "ethiopic",
    "ethiopic-abegede",
    "ethiopic-abegede-am-et",
    "ethiopic-abegede-gez",
    "ethiopic-abegede-ti-er",
    "ethiopic-abegede-ti-et",
    "ethiopic-halehame-aa-er",
    "ethiopic-halehame-aa-et",
    "ethiopic-halehame-am-et",
    "ethiopic-halehame-gez",
    "ethiopic-halehame-om-et",
    "ethiopic-halehame-sid-et",
    "ethiopic-halehame-so-et",
    "ethiopic-halehame-ti-er",
    "ethiopic-halehame-ti-et",
    "ethiopic-halehame-tig",
    "ethiopic-numeric",
    "ew-resize",
    "exclusion",
    "expanded",
    "extends",
    "extra-condensed",
    "extra-expanded",
    "fantasy",
    "fast",
    "fill",
    "fill-box",
    "fixed",
    "flat",
    "flex",
    "flex-end",
    "flex-start",
    "footnotes",
    "forwards",
    "from",
    "geometricprecision",
    "georgian",
    "grayscale",
    "graytext",
    "grid",
    "groove",
    "gujarati",
    "gurmukhi",
    "hand",
    "hangul",
    "hangul-consonant",
    "hard-light",
    "hebrew",
    "help",
    "hidden",
    "hide",
    "higher",
    "highlight",
    "highlighttext",
    "hiragana",
    "hiragana-iroha",
    "horizontal",
    "hsl",
    "hsla",
    "hue",
    "hue-rotate",
    "icon",
    "ignore",
    "inactiveborder",
    "inactivecaption",
    "inactivecaptiontext",
    "infinite",
    "infobackground",
    "infotext",
    "inherit",
    "initial",
    "inline",
    "inline-axis",
    "inline-block",
    "inline-flex",
    "inline-grid",
    "inline-table",
    "inset",
    "inside",
    "intrinsic",
    "invert",
    "italic",
    "japanese-formal",
    "japanese-informal",
    "justify",
    "kannada",
    "katakana",
    "katakana-iroha",
    "keep-all",
    "khmer",
    "korean-hangul-formal",
    "korean-hanja-formal",
    "korean-hanja-informal",
    "landscape",
    "lao",
    "large",
    "larger",
    "left",
    "level",
    "lighten",
    "lighter",
    "line-through",
    "linear",
    "linear-gradient",
    "lines",
    "list-item",
    "listbox",
    "listitem",
    "local",
    "logical",
    "loud",
    "lower",
    "lower-alpha",
    "lower-armenian",
    "lower-greek",
    "lower-hexadecimal",
    "lower-latin",
    "lower-norwegian",
    "lower-roman",
    "lowercase",
    "ltr",
    "luminosity",
    "malayalam",
    "manipulation",
    "match",
    "matrix",
    "matrix3d",
    "media-play-button",
    "media-slider",
    "media-sliderthumb",
    "media-volume-slider",
    "media-volume-sliderthumb",
    "medium",
    "menu",
    "menulist",
    "menulist-button",
    "menutext",
    "message-box",
    "middle",
    "min-intrinsic",
    "mix",
    "mongolian",
    "monospace",
    "move",
    "multiple",
    "multiple_mask_images",
    "multiply",
    "myanmar",
    "n-resize",
    "narrower",
    "ne-resize",
    "nesw-resize",
    "no-close-quote",
    "no-drop",
    "no-open-quote",
    "no-repeat",
    "none",
    "normal",
    "not-allowed",
    "nowrap",
    "ns-resize",
    "numbers",
    "numeric",
    "nw-resize",
    "nwse-resize",
    "oblique",
    "octal",
    "opacity",
    "open-quote",
    "optimizelegibility",
    "optimizespeed",
    "oriya",
    "oromo",
    "outset",
    "outside",
    "outside-shape",
    "overlay",
    "overline",
    "padding",
    "padding-box",
    "page",
    "painted",
    "paused",
    "persian",
    "perspective",
    "pinch-zoom",
    "plus-darker",
    "plus-lighter",
    "pointer",
    "polygon",
    "portrait",
    "pre",
    "pre-line",
    "pre-wrap",
    "preserve-3d",
    "progress",
    "push-button",
    "radial-gradient",
    "radio",
    "read-only",
    "read-write",
    "read-write-plaintext-only",
    "rectangle",
    "region",
    "relative",
    "repeat",
    "repeat-x",
    "repeat-y",
    "repeating-conic-gradient",
    "repeating-linear-gradient",
    "repeating-radial-gradient",
    "reset",
    "reverse",
    "rgb",
    "rgba",
    "ridge",
    "right",
    "rotate",
    "rotate3d",
    "rotatex",
    "rotatey",
    "rotatez",
    "round",
    "row",
    "row-resize",
    "row-reverse",
    "rtl",
    "run-in",
    "running",
    "s-resize",
    "sans-serif",
    "saturate",
    "saturation",
    "scale",
    "scale3d",
    "scalex",
    "scaley",
    "scalez",
    "screen",
    "scroll",
    "scroll-position",
    "scrollbar",
    "se-resize",
    "searchfield",
    "searchfield-cancel-button",
    "searchfield-decoration",
    "searchfield-results-button",
    "searchfield-results-decoration",
    "self-end",
    "self-start",
    "semi-condensed",
    "semi-expanded",
    "separate",
    "sepia",
    "serif",
    "show",
    "sidama",
    "simp-chinese-formal",
    "simp-chinese-informal",
    "single",
    "skew",
    "skewx",
    "skewy",
    "skip-white-space",
    "slide",
    "slider-horizontal",
    "slider-vertical",
    "sliderthumb-horizontal",
    "sliderthumb-vertical",
    "slow",
    "small",
    "small-caps",
    "small-caption",
    "smaller",
    "soft-light",
    "solid",
    "somali",
    "source-atop",
    "source-in",
    "source-out",
    "source-over",
    "space",
    "space-around",
    "space-between",
    "space-evenly",
    "spell-out",
    "square",
    "square-button",
    "start",
    "static",
    "status-bar",
    "stretch",
    "stroke",
    "stroke-box",
    "sub",
    "subpixel-antialiased",
    "super",
    "svg_masks",
    "sw-resize",
    "symbolic",
    "symbols",
    "system-ui",
    "table",
    "table-caption",
    "table-cell",
    "table-column",
    "table-column-group",
    "table-footer-group",
    "table-header-group",
    "table-row",
    "table-row-group",
    "tamil",
    "telugu",
    "text",
    "text-bottom",
    "text-top",
    "textarea",
    "textfield",
    "thai",
    "thick",
    "thin",
    "threeddarkshadow",
    "threedface",
    "threedhighlight",
    "threedlightshadow",
    "threedshadow",
    "tibetan",
    "tigre",
    "tigrinya-er",
    "tigrinya-er-abegede",
    "tigrinya-et",
    "tigrinya-et-abegede",
    "to",
    "top",
    "trad-chinese-formal",
    "trad-chinese-informal",
    "transform",
    "translate",
    "translate3d",
    "translatex",
    "translatey",
    "translatez",
    "transparent",
    "ultra-condensed",
    "ultra-expanded",
    "underline",
    "unidirectional-pan",
    "unset",
    "up",
    "upper-alpha",
    "upper-armenian",
    "upper-greek",
    "upper-hexadecimal",
    "upper-latin",
    "upper-norwegian",
    "upper-roman",
    "uppercase",
    "urdu",
    "url",
    "var",
    "vertical",
    "vertical-text",
    "view-box",
    "visible",
    "visiblefill",
    "visiblepainted",
    "visiblestroke",
    "visual",
    "w-resize",
    "wait",
    "wave",
    "wider",
    "window",
    "windowframe",
    "windowtext",
    "words",
    "wrap",
    "wrap-reverse",
    "x-large",
    "x-small",
    "xor",
    "xx-large",
    "xx-small",
];
