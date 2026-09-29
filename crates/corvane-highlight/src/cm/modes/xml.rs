//! `codemirror/mode/xml/xml.js`: XML and (with `htmlConfig`) HTML tags,
//! attributes, entities, comments, CDATA, processing instructions and
//! doctypes, with the tag-context stack that marks mismatched closing tags
//! as errors.
//!
//! Ported function by function (JS names in comments). The tokenizer
//! closures (`inAttribute(quote)`, `inBlock(style, terminator)`,
//! `doctype(depth)`) become [`Tokenize`] variants and the parser states
//! (`baseState`, `tagNameState`, …) become [`St`]. Indentation bookkeeping
//! (`indented`, `tagStart`, `startOfLine`, `noIndent`) is left out: it never
//! changes a token.
//!
//! GHD maps `text/html` to htmlmixed (`codemirror/mode/htmlmixed`), which
//! nests this mode with [`XmlConfig::html`]; only `text/xml` and
//! `application/xml` reach this mode directly.

use std::borrow::Cow;

use crate::re;

use super::super::{Mode, ModeState, StringStream, state};

/// `config`: `xmlConfig` / `htmlConfig` merged with the mode options that
/// change tokens.
#[derive(Clone, Copy, Debug)]
pub struct XmlConfig {
    pub html_mode: bool,
    pub allow_unquoted: bool,
    pub allow_missing: bool,
    pub allow_missing_tag_name: bool,
}

impl XmlConfig {
    /// `xmlConfig`
    pub const fn xml() -> Self {
        Self {
            html_mode: false,
            allow_unquoted: false,
            allow_missing: false,
            allow_missing_tag_name: false,
        }
    }
    /// `htmlConfig` (`{name: "xml", htmlMode: true}`)
    pub const fn html() -> Self {
        Self {
            html_mode: true,
            allow_unquoted: true,
            allow_missing: true,
            allow_missing_tag_name: false,
        }
    }
}

pub struct XmlMode {
    config: XmlConfig,
}

/// `state.tokenize`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tokenize {
    InText,
    InTag,
    /// `inAttribute(quote)`
    InAttribute(char),
    /// `inBlock(style, terminator)`
    InBlock(&'static str, &'static str),
    /// `doctype(depth)`
    Doctype(u32),
}

/// `state.state`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Base,
    TagName,
    CloseTagName,
    Close,
    CloseErr,
    Attr,
    AttrEq,
    AttrValue,
    AttrContinued,
}

#[derive(Clone, Debug)]
pub struct XmlState {
    tokenize: Tokenize,
    state: St,
    tag_name: Option<String>,
    /// `state.context` chain of tag names, innermost last
    context: Vec<String>,
}

impl XmlState {
    /// `state.tagName`
    pub fn tag_name(&self) -> Option<&str> {
        self.tag_name.as_deref()
    }
    /// `state.context` is set
    pub fn has_context(&self) -> bool {
        !self.context.is_empty()
    }
}

/// `htmlConfig.autoSelfClosers`
fn auto_self_closer(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "command"
            | "embed"
            | "frame"
            | "hr"
            | "img"
            | "input"
            | "keygen"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
            | "menuitem"
    )
}

/// `htmlConfig.implicitlyClosed`
fn implicitly_closed(tag: &str) -> bool {
    matches!(
        tag,
        "dd" | "li"
            | "optgroup"
            | "option"
            | "p"
            | "rp"
            | "rt"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "tr"
    )
}

/// `htmlConfig.contextGrabbers[parent][next]`
fn context_grabs(parent: &str, next: &str) -> bool {
    match parent {
        "dd" | "dt" => matches!(next, "dd" | "dt"),
        "li" => next == "li",
        "option" => matches!(next, "option" | "optgroup"),
        "optgroup" => next == "optgroup",
        "p" => matches!(
            next,
            "address"
                | "article"
                | "aside"
                | "blockquote"
                | "dir"
                | "div"
                | "dl"
                | "fieldset"
                | "footer"
                | "form"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "header"
                | "hgroup"
                | "hr"
                | "menu"
                | "nav"
                | "ol"
                | "p"
                | "pre"
                | "section"
                | "table"
                | "ul"
        ),
        "rp" | "rt" => matches!(next, "rp" | "rt"),
        "tbody" => matches!(next, "tbody" | "tfoot"),
        "td" | "th" => matches!(next, "td" | "th"),
        "tfoot" => next == "tbody",
        "thead" => matches!(next, "tbody" | "tfoot"),
        "tr" => next == "tr",
        _ => false,
    }
}

/// `tokenize` results: the style plus the `type` scratch variable.
type Style = Option<Cow<'static, str>>;

impl XmlMode {
    pub const fn new(config: XmlConfig) -> Self {
        Self { config }
    }

    pub fn start_xml(&self) -> XmlState {
        XmlState {
            tokenize: Tokenize::InText,
            state: St::Base,
            tag_name: None,
            context: Vec::new(),
        }
    }

    fn tokenize(
        &self,
        stream: &mut StringStream,
        s: &mut XmlState,
        kind: &mut Option<&'static str>,
    ) -> Style {
        match s.tokenize {
            Tokenize::InText => self.in_text(stream, s, kind),
            Tokenize::InTag => self.in_tag(stream, s, kind),
            Tokenize::InAttribute(quote) => Self::in_attribute(quote, stream, s),
            Tokenize::InBlock(style, terminator) => Self::in_block(style, terminator, stream, s),
            Tokenize::Doctype(depth) => Self::doctype(depth, stream, s),
        }
    }

    /// `inText`
    fn in_text(
        &self,
        stream: &mut StringStream,
        s: &mut XmlState,
        kind: &mut Option<&'static str>,
    ) -> Style {
        // `chain(parser)`
        fn chain(
            mode: &XmlMode,
            t: Tokenize,
            stream: &mut StringStream,
            s: &mut XmlState,
            kind: &mut Option<&'static str>,
        ) -> Style {
            s.tokenize = t;
            mode.tokenize(stream, s, kind)
        }

        let ch = stream.next()?;
        if ch == '<' {
            if stream.eat('!').is_some() {
                if stream.eat('[').is_some() {
                    if stream.match_str("CDATA[", true, false) {
                        chain(self, Tokenize::InBlock("atom", "]]>"), stream, s, kind)
                    } else {
                        None
                    }
                } else if stream.match_str("--", true, false) {
                    chain(self, Tokenize::InBlock("comment", "-->"), stream, s, kind)
                } else if stream.match_str("DOCTYPE", true, true) {
                    stream.eat_while_if(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
                    });
                    chain(self, Tokenize::Doctype(1), stream, s, kind)
                } else {
                    None
                }
            } else if stream.eat('?').is_some() {
                stream.eat_while_if(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
                s.tokenize = Tokenize::InBlock("meta", "?>");
                Some(Cow::Borrowed("meta"))
            } else {
                *kind = Some(if stream.eat('/').is_some() {
                    "closeTag"
                } else {
                    "openTag"
                });
                s.tokenize = Tokenize::InTag;
                Some(Cow::Borrowed("tag bracket"))
            }
        } else if ch == '&' {
            let ok = if stream.eat('#').is_some() {
                if stream.eat('x').is_some() {
                    stream.eat_while_if(|c| c.is_ascii_hexdigit()) && stream.eat(';').is_some()
                } else {
                    stream.eat_while_if(|c| c.is_ascii_digit()) && stream.eat(';').is_some()
                }
            } else {
                stream.eat_while_if(|c| {
                    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':')
                }) && stream.eat(';').is_some()
            };
            Some(Cow::Borrowed(if ok { "atom" } else { "error" }))
        } else {
            stream.eat_while_if(|c| c != '&' && c != '<');
            None
        }
    }

    /// `inTag`
    fn in_tag(
        &self,
        stream: &mut StringStream,
        s: &mut XmlState,
        kind: &mut Option<&'static str>,
    ) -> Style {
        let ch = stream.next()?;
        if ch == '>' || (ch == '/' && stream.eat('>').is_some()) {
            s.tokenize = Tokenize::InText;
            *kind = Some(if ch == '>' { "endTag" } else { "selfcloseTag" });
            Some(Cow::Borrowed("tag bracket"))
        } else if ch == '=' {
            *kind = Some("equals");
            None
        } else if ch == '<' {
            s.tokenize = Tokenize::InText;
            s.state = St::Base;
            s.tag_name = None;
            let next = self.tokenize(stream, s, kind);
            Some(match next {
                Some(next) => Cow::Owned(format!("{next} tag error")),
                None => Cow::Borrowed("tag error"),
            })
        } else if ch == '\'' || ch == '"' {
            s.tokenize = Tokenize::InAttribute(ch);
            Self::in_attribute(ch, stream, s)
        } else {
            stream.match_re(re!(r#"^[^\s\x{a0}=<>"']*[^\s\x{a0}=<>"'/]"#), true);
            Some(Cow::Borrowed("word"))
        }
    }

    /// `inAttribute(quote)`
    fn in_attribute(quote: char, stream: &mut StringStream, s: &mut XmlState) -> Style {
        while !stream.eol() {
            if stream.next() == Some(quote) {
                s.tokenize = Tokenize::InTag;
                break;
            }
        }
        Some(Cow::Borrowed("string"))
    }

    /// `inBlock(style, terminator)`
    fn in_block(
        style: &'static str,
        terminator: &'static str,
        stream: &mut StringStream,
        s: &mut XmlState,
    ) -> Style {
        while !stream.eol() {
            if stream.match_str(terminator, true, false) {
                s.tokenize = Tokenize::InText;
                break;
            }
            stream.next();
        }
        Some(Cow::Borrowed(style))
    }

    /// `doctype(depth)`
    fn doctype(depth: u32, stream: &mut StringStream, s: &mut XmlState) -> Style {
        while let Some(ch) = stream.next() {
            if ch == '<' {
                s.tokenize = Tokenize::Doctype(depth + 1);
                return Self::doctype(depth + 1, stream, s);
            } else if ch == '>' {
                if depth == 1 {
                    s.tokenize = Tokenize::InText;
                    break;
                } else {
                    s.tokenize = Tokenize::Doctype(depth - 1);
                    return Self::doctype(depth - 1, stream, s);
                }
            }
        }
        Some(Cow::Borrowed("meta"))
    }

    /// `maybePopContext`
    fn maybe_pop_context(&self, s: &mut XmlState, next_tag_name: Option<&str>) {
        if !self.config.html_mode {
            return;
        }
        let Some(next) = next_tag_name.map(str::to_lowercase) else {
            return;
        };
        while let Some(parent) = s.context.last() {
            if !context_grabs(&parent.to_lowercase(), &next) {
                return;
            }
            s.context.pop();
        }
    }

    /// The parser states (`baseState`, `tagNameState`, …): `type` is the
    /// token type (or its style), `set_style` the JS `setStyle`.
    fn run_state(
        &self,
        st: St,
        kind: &str,
        stream: &StringStream,
        s: &mut XmlState,
        set_style: &mut Option<&'static str>,
    ) -> St {
        let html = self.config.html_mode;
        match st {
            // baseState
            St::Base => {
                if kind == "openTag" {
                    St::TagName
                } else if kind == "closeTag" {
                    St::CloseTagName
                } else {
                    St::Base
                }
            }
            // tagNameState
            St::TagName => {
                if kind == "word" {
                    s.tag_name = Some(stream.current().to_string());
                    *set_style = Some("tag");
                    St::Attr
                } else if self.config.allow_missing_tag_name && kind == "endTag" {
                    *set_style = Some("tag bracket");
                    self.run_state(St::Attr, kind, stream, s, set_style)
                } else {
                    *set_style = Some("error");
                    St::TagName
                }
            }
            // closeTagNameState
            St::CloseTagName => {
                if kind == "word" {
                    let tag_name = stream.current();
                    if let Some(cx) = s.context.last()
                        && cx != tag_name
                        && html
                        && implicitly_closed(&cx.to_lowercase())
                    {
                        s.context.pop();
                    }
                    if s.context.last().is_some_and(|cx| cx == tag_name) {
                        *set_style = Some("tag");
                        St::Close
                    } else {
                        *set_style = Some("tag error");
                        St::CloseErr
                    }
                } else if self.config.allow_missing_tag_name && kind == "endTag" {
                    *set_style = Some("tag bracket");
                    self.run_state(St::Close, kind, stream, s, set_style)
                } else {
                    *set_style = Some("error");
                    St::CloseErr
                }
            }
            // closeState
            St::Close => {
                if kind != "endTag" {
                    *set_style = Some("error");
                    return St::Close;
                }
                s.context.pop();
                St::Base
            }
            // closeStateErr
            St::CloseErr => {
                *set_style = Some("error");
                self.run_state(St::Close, kind, stream, s, set_style)
            }
            // attrState
            St::Attr => {
                if kind == "word" {
                    *set_style = Some("attribute");
                    St::AttrEq
                } else if kind == "endTag" || kind == "selfcloseTag" {
                    let tag_name = s.tag_name.take();
                    let self_closing = kind == "selfcloseTag"
                        || (html
                            && tag_name
                                .as_deref()
                                .is_some_and(|t| auto_self_closer(&t.to_lowercase())));
                    self.maybe_pop_context(s, tag_name.as_deref());
                    if !self_closing {
                        // `new Context(state, tagName)`: `tagName || ""`
                        s.context.push(tag_name.unwrap_or_default());
                    }
                    St::Base
                } else {
                    *set_style = Some("error");
                    St::Attr
                }
            }
            // attrEqState
            St::AttrEq => {
                if kind == "equals" {
                    return St::AttrValue;
                }
                if !self.config.allow_missing {
                    *set_style = Some("error");
                }
                self.run_state(St::Attr, kind, stream, s, set_style)
            }
            // attrValueState
            St::AttrValue => {
                if kind == "string" {
                    return St::AttrContinued;
                }
                if kind == "word" && self.config.allow_unquoted {
                    *set_style = Some("string");
                    return St::Attr;
                }
                *set_style = Some("error");
                self.run_state(St::Attr, kind, stream, s, set_style)
            }
            // attrContinuedState
            St::AttrContinued => {
                if kind == "string" {
                    return St::AttrContinued;
                }
                self.run_state(St::Attr, kind, stream, s, set_style)
            }
        }
    }

    /// `mode.token`
    pub fn token_xml(&self, stream: &mut StringStream, s: &mut XmlState) -> Style {
        if stream.eat_space() {
            return None;
        }
        let mut kind = None;
        let mut style = self.tokenize(stream, s, &mut kind);
        if (style.is_some() || kind.is_some()) && style.as_deref() != Some("comment") {
            let mut set_style = None;
            // `type || style`
            let arg: Cow<'static, str> = match kind {
                Some(k) => Cow::Borrowed(k),
                None => style.clone().unwrap_or_default(),
            };
            s.state = self.run_state(s.state, &arg, stream, s, &mut set_style);
            if let Some(set) = set_style {
                style = Some(if set == "error" {
                    Cow::Owned(format!("{} error", style.as_deref().unwrap_or("null")))
                } else {
                    Cow::Borrowed(set)
                });
            }
        }
        style
    }

    /// `mode.skipAttribute`
    pub fn skip_attribute(&self, s: &mut XmlState) {
        if s.state == St::AttrValue {
            s.state = St::Attr;
        }
    }
}

impl Mode for XmlMode {
    fn name(&self) -> &'static str {
        "xml"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(self.start_xml())
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        self.token_xml(stream, state::<XmlState>(st))
            .map(Cow::into_owned)
    }
}
