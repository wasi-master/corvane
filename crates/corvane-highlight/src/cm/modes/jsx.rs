//! `codemirror/mode/jsx/jsx.js`: JavaScript (or TypeScript) with embedded
//! XML elements, switching between a nested [`JsMode`] and [`XmlMode`]
//! through a stack of contexts (`Context(state, mode, depth, prev)`).
//! Tokens carry the inner mode's class (`m-javascript` / `m-xml`), like
//! `innerMode` makes them in GHD.
//!
//! The indentation arguments (`flatXMLIndent`, `jsMode.indent`) only seed
//! the nested states' indentation, which never changes a token, so they are
//! left out.

use std::borrow::Cow;

use crate::re;

use super::super::{Mode, ModeState, StringStream, state, state_ref};
use super::javascript::{JsConfig, JsMode, JsState, Scratch};
use super::xml::{XmlConfig, XmlMode, XmlState};

pub struct JsxMode {
    xml: XmlMode,
    js: JsMode,
}

#[derive(Clone, Debug)]
enum Inner {
    Js(JsState),
    Xml(XmlState),
}

/// `Context`: an inner state and its `depth` (open braces in JS; in XML 0
/// = not in a tag, 1 = in a tag, 2 = in a JS block comment in a tag).
#[derive(Clone, Debug)]
struct Context {
    inner: Inner,
    /// `undefined` for the outermost JS context
    depth: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct JsxState {
    /// `state.context` and its `prev` chain, innermost last
    stack: Vec<Context>,
    /// the JS mode instance's `type` / `content` scratch, shared by all its
    /// contexts as in JS
    js_scratch: Scratch,
}

impl JsxMode {
    /// `CodeMirror.defineMode("jsx", …)` with `modeConfig.base`
    pub fn new(base: JsConfig) -> Self {
        Self {
            xml: XmlMode::new(XmlConfig {
                allow_missing: true,
                allow_missing_tag_name: true,
                ..XmlConfig::xml()
            }),
            js: JsMode::new(base),
        }
    }

    fn push(s: &mut JsxState, inner: Inner) {
        s.stack.push(Context {
            inner,
            depth: Some(0),
        });
    }

    /// `token`
    fn token_jsx(&self, stream: &mut StringStream, s: &mut JsxState) -> Option<Cow<'static, str>> {
        match s.stack.last().map(|cx| &cx.inner) {
            Some(Inner::Xml(_)) => self.xml_token(stream, s),
            Some(Inner::Js(_)) => self.js_token(stream, s).map(Cow::Borrowed),
            None => {
                stream.next();
                None
            }
        }
    }

    /// `xmlToken`
    fn xml_token(&self, stream: &mut StringStream, s: &mut JsxState) -> Option<Cow<'static, str>> {
        let Some(Context {
            inner: Inner::Xml(xs),
            depth,
        }) = s.stack.last_mut()
        else {
            return None;
        };
        if *depth == Some(2) {
            // Inside a JS /* */ comment
            if stream.match_re(re!(r"^.*?\*\/"), true).is_some() {
                *depth = Some(1);
            } else {
                stream.skip_to_end();
            }
            return Some(Cow::Borrowed("comment"));
        }

        if stream.peek() == Some('{') {
            self.xml.skip_attribute(xs);
            let js = self.js.start_js();
            Self::push(s, Inner::Js(js));
            return None;
        }

        if *depth == Some(1) {
            // Inside of tag
            if stream.peek() == Some('<') {
                // Tag inside of tag
                self.xml.skip_attribute(xs);
                let xml = self.xml.start_xml();
                Self::push(s, Inner::Xml(xml));
                return None;
            } else if stream.match_str("//", true, false) {
                stream.skip_to_end();
                return Some(Cow::Borrowed("comment"));
            } else if stream.match_str("/*", true, false) {
                *depth = Some(2);
                return self.token_jsx(stream, s);
            }
        }

        let style = self.xml.token_xml(stream, xs);
        let cur = stream.current();
        if style
            .as_deref()
            .is_some_and(|st| st.split_whitespace().any(|c| c == "tag"))
        {
            if cur.ends_with('>') {
                if xs.has_context() {
                    *depth = Some(0);
                } else {
                    s.stack.pop();
                }
            } else if cur.starts_with('<') {
                *depth = Some(1);
            }
        } else if style.is_none()
            && let Some(stop) = cur.find('{')
        {
            let len = cur.chars().count();
            let stop = cur[..stop].chars().count();
            stream.back_up(len - stop);
        }
        style
    }

    /// `jsToken`
    fn js_token(&self, stream: &mut StringStream, s: &mut JsxState) -> Option<&'static str> {
        let Some(Context {
            inner: Inner::Js(js),
            depth,
        }) = s.stack.last_mut()
        else {
            return None;
        };
        if stream.peek() == Some('<')
            && stream
                .match_re(re!(r"^<([^<>]|<[^>]*>)+,\s*>"), false)
                .is_none()
            && self.js.expression_allowed(stream, js)
        {
            self.js.skip_expression(js);
            let xml = self.xml.start_xml();
            Self::push(s, Inner::Xml(xml));
            return None;
        }

        std::mem::swap(&mut js.scratch, &mut s.js_scratch);
        let style = self.js.token_js(stream, js);
        std::mem::swap(&mut js.scratch, &mut s.js_scratch);
        if style.is_none()
            && let Some(d) = depth.as_mut()
        {
            let cur = stream.current();
            if cur == "{" {
                *d += 1;
            } else if cur == "}" {
                *d -= 1;
                if *d == 0 {
                    s.stack.pop();
                }
            }
        }
        style
    }
}

impl Mode for JsxMode {
    fn name(&self) -> &'static str {
        "jsx"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(JsxState {
            stack: vec![Context {
                inner: Inner::Js(self.js.start_js()),
                depth: None,
            }],
            js_scratch: Scratch::default(),
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        self.token_jsx(stream, state::<JsxState>(st))
            .map(Cow::into_owned)
    }

    /// `innerMode`: the current context's mode
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        match state_ref::<JsxState>(st).stack.last().map(|cx| &cx.inner) {
            Some(Inner::Xml(_)) => "xml",
            _ => "javascript",
        }
    }
}
