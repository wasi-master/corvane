//! `codemirror/mode/soy/soy.js` (`text/x-soy`), ported line by line; the
//! JS names are kept in comments.
//!
//! Text between Soy tags goes to a stack of local modes (`localStates`):
//! htmlmixed with `allowMissingTagName` (and the modes it loads: xml,
//! javascript, css), switched by a tag's `kind="…"` attribute to `text/css`,
//! javascript or the `null` mode (`text`, `attributes`, `uri`,
//! `trusted_resource_uri`).
//!
//! GHD calls `getMode({}, …)`, so `config.indentUnit` is `undefined`: the
//! first tag makes `state.indent` `NaN` and it stays `NaN` until the `}` of
//! a `{/template}` / `{/deltemplate}` resets it to 0. `tokenUntil` then runs
//! the local mode under `hideFirstChars(NaN)`, which leaves the stream's
//! `lineStart` `NaN` for the rest of the line
//! ([`StringStream::hide_first_chars_nan`]).
//!
//! `innerMode` is soy itself while a Soy construct is open, else the top
//! local mode (followed down htmlmixed), which is the `m-` class every
//! token gets - Soy's own tags in text included.

use std::rc::Rc;
use std::sync::Arc;

use super::super::{Mode, ModeState, StringStream, state, state_ref};
use super::htmlmixed::{HtmlMixed, HtmlMixedConfig, NullMode};
use crate::re;

/// `tags[name]`: `(noEndTag, soyState, variableScope)`
fn tag_def(name: &str) -> Option<(bool, Option<&'static str>, bool)> {
    const PARAM: (bool, Option<&str>, bool) = (true, Some("param-def"), false);
    Some(match name {
        "alias" | "delpackage" => (true, None, false),
        "namespace" => (true, Some("namespace-def"), false),
        "@attribute" | "@attribute?" | "@param" | "@param?" | "@inject" | "@inject?" | "@state" => {
            PARAM
        }
        "template" | "deltemplate" => (false, Some("templ-def"), true),
        "extern" => (false, Some("param-def"), false),
        "export" => (false, Some("export"), false),
        "literal" | "msg" | "select" | "plural" | "if" | "javaimpl" | "jsimpl" | "switch"
        | "log" | "velog" => (false, None, false),
        "fallbackmsg" | "elseif" | "else" | "case" | "default" | "ifempty" | "print" => {
            (true, None, false)
        }
        "let" => (false, Some("var-def"), false),
        "foreach" | "for" => (false, Some("for-loop"), true),
        "call" | "delcall" => (false, Some("templ-ref"), false),
        "param" => (false, Some("param-ref"), false),
        "element" => (false, None, true),
        "const" => (false, Some("const-def"), false),
        _ => return None,
    })
}

/// A variables list node (`prepend` / `contains`).
struct Var {
    element: String,
    next: Option<Rc<Var>>,
}

type Vars = Option<Rc<Var>>;

fn prepend(list: &Vars, element: &str) -> Vars {
    Some(Rc::new(Var {
        element: element.to_string(),
        next: list.clone(),
    }))
}

fn contains(mut list: &Vars, element: &str) -> bool {
    while let Some(node) = list {
        if node.element == element {
            return true;
        }
        list = &node.next;
    }
    false
}

/// `ref(list, name, loose)`
fn reference(list: &Vars, name: &str, loose: bool) -> &'static str {
    if contains(list, name) {
        "variable-2"
    } else if loose {
        "variable"
    } else {
        "variable-2 error"
    }
}

/// `Context`
#[derive(Clone)]
struct Context {
    tag: String,
    kind: Option<String>,
    scope: Vars,
}

/// The local modes of `modes`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Local {
    Html,
    Text,
    Css,
    Js,
}

#[derive(Clone)]
struct SoyState {
    soy_state: Vec<&'static str>,
    variables: Vars,
    /// `state.indent` is `NaN` (else 0)
    indent_nan: bool,
    quote_kind: Option<String>,
    /// innermost last
    context: Vec<Context>,
    lookup_variables: bool,
    local_states: Vec<(Local, Box<dyn ModeState>)>,
    /// `state.tag` (`undefined` at first)
    tag: Option<String>,
}

pub struct Soy {
    html: Arc<dyn Mode>,
    text: Arc<dyn Mode>,
    css: Arc<dyn Mode>,
    js: Arc<dyn Mode>,
}

impl Default for Soy {
    fn default() -> Self {
        Self::new()
    }
}

type Style = Option<String>;

fn st(s: &str) -> Style {
    Some(s.to_string())
}

/// `popcontext(state)`
fn popcontext(s: &mut SoyState) {
    let Some(cx) = s.context.pop() else {
        return;
    };
    if cx.scope.is_some() {
        s.variables = cx.scope;
    }
}

impl Soy {
    pub fn new() -> Self {
        Self {
            html: Arc::new(HtmlMixed::new(
                HtmlMixedConfig {
                    allow_missing_tag_name: true,
                    ..Default::default()
                },
                super::html_modes,
            )),
            text: Arc::new(NullMode),
            css: super::css::css_for_mime("text/css").unwrap_or_else(|| Arc::new(NullMode)),
            js: super::javascript(),
        }
    }

    fn local(&self, l: Local) -> &dyn Mode {
        match l {
            Local::Html => &*self.html,
            Local::Text => &*self.text,
            Local::Css => &*self.css,
            Local::Js => &*self.js,
        }
    }

    /// `tokenUntil(stream, state, untilRegExp)`
    fn token_until(
        &self,
        stream: &mut StringStream,
        s: &mut SoyState,
        until: &fancy_regex::Regex,
    ) -> Style {
        // `for (indent = 0; indent < state.indent; …)` never runs: the
        // indent is 0 or NaN
        let rest = stream.slice(stream.pos, stream.len());
        let end = match until.find(rest).ok().flatten() {
            Some(m) => stream.pos + super::super::js_len(&rest[..m.start()]),
            None => stream.len(),
        };
        let nan = s.indent_nan;
        let Some((local, local_state)) = s.local_states.last_mut() else {
            stream.skip_to_end();
            return None;
        };
        let mode = self.local(*local);
        stream.with_end(end, |stream| {
            if nan {
                stream.hide_first_chars_nan(|stream| mode.token(stream, &mut **local_state))
            } else {
                stream.hide_first_chars(0, |stream| mode.token(stream, &mut **local_state))
            }
        })
    }

    /// `expression(stream, state)`
    fn expression(stream: &mut StringStream, s: &mut SoyState) -> Style {
        if stream.matches(re!(r"\[")) {
            s.soy_state.push("list-literal");
            s.context.push(Context {
                tag: "list-literal".to_string(),
                kind: None,
                scope: s.variables.clone(),
            });
            s.lookup_variables = false;
            return None;
        } else if stream.matches(re!(r"(?<![A-Za-z0-9_])map(?=\()")) {
            s.soy_state.push("map-literal");
            return st("keyword");
        } else if stream.matches(re!(r"(?<![A-Za-z0-9_])record(?=\()")) {
            s.soy_state.push("record-literal");
            return st("keyword");
        } else if stream.matches(re!(r"([A-Za-z0-9_]+)(?=\()")) {
            return st("variable callee");
        } else if let Some(m) = stream.match_re(re!(r#"^["']"#), true) {
            s.soy_state.push("string");
            s.quote_kind = Some(m.text);
            return st("string");
        } else if stream.matches(re!(r"^[(]")) {
            s.soy_state.push("open-parentheses");
            return None;
        } else if stream.matches(re!(r"(null|true|false)(?![A-Za-z0-9_])"))
            || stream.matches(re!(r"0x([0-9a-fA-F]{2,})"))
            || stream.matches(re!(r"-?([0-9]*[.])?[0-9]+(e[0-9]*)?"))
        {
            return st("atom");
        } else if stream.matches(re!(r"(\||[+\-*\/%]|[=!]=|\?:|[<>]=?)")) {
            // Tokenize filter, binary, null propagator, and equality operators.
            return st("operator");
        } else if let Some(m) = stream.match_re(re!(r"^\$([A-Za-z0-9_]+)"), true) {
            return st(reference(
                &s.variables,
                m.group(1).unwrap_or(""),
                !s.lookup_variables,
            ));
        } else if let Some(m) = stream.match_re(re!(r"^[A-Za-z0-9_]+"), true) {
            return matches!(m.text.as_str(), "as" | "and" | "or" | "not" | "in" | "if")
                .then(|| "keyword".to_string());
        }
        stream.next();
        None
    }

    fn token_soy(&self, stream: &mut StringStream, s: &mut SoyState) -> Style {
        match s.soy_state.last().copied() {
            Some("comment") => {
                if !stream.matches(re!(r"^.*?\*\/")) {
                    stream.skip_to_end();
                } else {
                    s.soy_state.pop();
                }
                if s.context.last().is_none_or(|c| c.scope.is_none()) {
                    let current = stream.current().to_string();
                    for m in re!(r"@param\??\s+(\S+)").captures_iter(&current).flatten() {
                        if let Some(g) = m.get(1) {
                            s.variables = prepend(&s.variables, g.as_str());
                        }
                    }
                }
                return st("comment");
            }
            Some("string") => {
                match stream.match_re(re!(r#"^.*?(["']|\\[\s\S])"#), true) {
                    None => stream.skip_to_end(),
                    Some(m) => {
                        if m.group(1) == s.quote_kind.as_deref() {
                            s.quote_kind = None;
                            s.soy_state.pop();
                        }
                    }
                }
                return st("string");
            }
            _ => {}
        }

        if s.soy_state.last() != Some(&"literal") {
            if stream.matches(re!(r"^\/\*")) {
                s.soy_state.push("comment");
                return st("comment");
            } else if stream.matches(if stream.sol() {
                re!(r"^\s*\/\/.*")
            } else {
                re!(r"^\s+\/\/.*")
            }) {
                return st("comment");
            }
        }

        match s.soy_state.last().copied() {
            Some("templ-def") => {
                // /^\.?([\w]+(?!\.[\w]+)*)/: the quantified lookahead is a no-op
                if stream.matches(re!(r"^\.?([A-Za-z0-9_]+)")) {
                    s.soy_state.pop();
                    return st("def");
                }
                stream.next();
                return None;
            }
            Some("templ-ref") => {
                if let Some(m) = stream.match_re(re!(r"(\.?[a-zA-Z_][a-zA-Z_0-9]+)+"), true) {
                    s.soy_state.pop();
                    // If the first character is '.', it can only be a local template.
                    if m.text.starts_with('.') {
                        return st("variable-2");
                    }
                    return st("variable");
                }
                if let Some(m) = stream.match_re(re!(r"^\$([A-Za-z0-9_]+)"), true) {
                    s.soy_state.pop();
                    return st(reference(
                        &s.variables,
                        m.group(1).unwrap_or(""),
                        !s.lookup_variables,
                    ));
                }
                stream.next();
                return None;
            }
            Some("namespace-def") => {
                if stream.matches(re!(r"^\.?([A-Za-z0-9_\.]+)")) {
                    s.soy_state.pop();
                    return st("variable");
                }
                stream.next();
                return None;
            }
            Some("param-def") => {
                if stream.matches(re!(r"^\*")) {
                    s.soy_state.pop();
                    s.soy_state.push("param-type");
                    return st("type");
                }
                if let Some(m) = stream.match_re(re!(r"^[A-Za-z0-9_]+"), true) {
                    s.variables = prepend(&s.variables, &m.text);
                    s.soy_state.pop();
                    s.soy_state.push("param-type");
                    return st("def");
                }
                stream.next();
                return None;
            }
            Some("param-ref") => {
                if stream.matches(re!(r"^[A-Za-z0-9_]+")) {
                    s.soy_state.pop();
                    return st("property");
                }
                stream.next();
                return None;
            }
            Some("open-parentheses") => {
                if stream.matches(re!(r"[)]")) {
                    s.soy_state.pop();
                    return None;
                }
                return Self::expression(stream, s);
            }
            Some("param-type") => {
                let peek = stream.peek();
                if peek.is_some_and(|c| "}]=>,".contains(c)) {
                    s.soy_state.pop();
                    return None;
                } else if peek == Some('[') {
                    s.soy_state.push("param-type-record");
                    return None;
                } else if peek == Some('(') {
                    s.soy_state.push("param-type-template");
                    return None;
                } else if peek == Some('<') {
                    s.soy_state.push("param-type-parameter");
                    return None;
                } else if stream.matches(re!(r"^([A-Za-z0-9_]+|[?])")) {
                    return st("type");
                }
                stream.next();
                return None;
            }
            Some("param-type-record") => {
                if stream.peek() == Some(']') {
                    s.soy_state.pop();
                    return None;
                }
                if stream.matches(re!(r"^[A-Za-z0-9_]+")) {
                    s.soy_state.push("param-type");
                    return st("property");
                }
                stream.next();
                return None;
            }
            Some("param-type-parameter") => {
                if stream.matches(re!(r"^[>]")) {
                    s.soy_state.pop();
                    return None;
                }
                if stream.matches(re!(r"^[<,]")) {
                    s.soy_state.push("param-type");
                    return None;
                }
                stream.next();
                return None;
            }
            Some("param-type-template") => {
                if stream.matches(re!(r"[>]")) {
                    s.soy_state.pop();
                    s.soy_state.push("param-type");
                    return None;
                }
                if stream.matches(re!(r"^[A-Za-z0-9_]+")) {
                    s.soy_state.push("param-type");
                    return st("def");
                }
                stream.next();
                return None;
            }
            Some("var-def") => {
                if let Some(m) = stream.match_re(re!(r"^\$([A-Za-z0-9_]+)"), true) {
                    s.variables = prepend(&s.variables, m.group(1).unwrap_or(""));
                    s.soy_state.pop();
                    return st("def");
                }
                stream.next();
                return None;
            }
            Some("for-loop") => {
                if stream.matches(re!(r"(?<![A-Za-z0-9_])in(?![A-Za-z0-9_])")) {
                    s.soy_state.pop();
                    return st("keyword");
                }
                if stream.peek() == Some('$') {
                    s.soy_state.push("var-def");
                    return None;
                }
                stream.next();
                return None;
            }
            Some("record-literal") => {
                if stream.matches(re!(r"^[)]")) {
                    s.soy_state.pop();
                    return None;
                }
                if stream.matches(re!(r"[(,]")) {
                    s.soy_state.push("map-value");
                    s.soy_state.push("record-key");
                    return None;
                }
                stream.next();
                return None;
            }
            Some("map-literal") => {
                if stream.matches(re!(r"^[)]")) {
                    s.soy_state.pop();
                    return None;
                }
                if stream.matches(re!(r"[(,]")) {
                    s.soy_state.push("map-value");
                    s.soy_state.push("map-value");
                    return None;
                }
                stream.next();
                return None;
            }
            Some("list-literal") => {
                if stream.match_str("]", true, false) {
                    s.soy_state.pop();
                    s.lookup_variables = true;
                    popcontext(s);
                    return None;
                }
                if stream.matches(re!(r"(?<![A-Za-z0-9_])for(?![A-Za-z0-9_])")) {
                    s.lookup_variables = true;
                    s.soy_state.push("for-loop");
                    return st("keyword");
                }
                return Self::expression(stream, s);
            }
            Some("record-key") => {
                if stream.matches(re!(r"[A-Za-z0-9_]+")) {
                    return st("property");
                }
                if stream.matches(re!(r"^[:]")) {
                    s.soy_state.pop();
                    return None;
                }
                stream.next();
                return None;
            }
            Some("map-value") => {
                if matches!(stream.peek(), Some(')' | ',')) || stream.matches(re!(r"^[:)]")) {
                    s.soy_state.pop();
                    return None;
                }
                return Self::expression(stream, s);
            }
            Some("import") => {
                if stream.eat(';').is_some() {
                    s.soy_state.pop();
                    s.indent_nan = true;
                    return None;
                }
                if stream.matches(re!(r"[A-Za-z0-9_]+(?=\s+as(?![A-Za-z0-9_]))")) {
                    return st("variable");
                }
                if let Some(m) = stream.match_re(re!(r"[A-Za-z0-9_]+"), true) {
                    let kw = re!(r"(?<![A-Za-z0-9_])(from|as)(?![A-Za-z0-9_])")
                        .is_match(&m.text)
                        .unwrap_or(false);
                    return st(if kw { "keyword" } else { "def" });
                }
                if let Some(m) = stream.match_re(re!(r#"^["']"#), true) {
                    s.soy_state.push("string");
                    s.quote_kind = Some(m.text);
                    return st("string");
                }
                stream.next();
                return None;
            }
            Some("tag") => {
                let (end_tag, tag_name) = match &s.tag {
                    None => (true, String::new()),
                    Some(t) => match t.strip_prefix('/') {
                        Some(rest) => (true, rest.to_string()),
                        None => (false, t.clone()),
                    },
                };
                if stream.matches(re!(r"^\/?}")) {
                    let self_closed = stream.current() == "/}";
                    if self_closed && !end_tag {
                        popcontext(s);
                    }
                    if matches!(s.tag.as_deref(), Some("/template" | "/deltemplate")) {
                        s.variables = prepend(&None, "ij");
                        s.indent_nan = false;
                    } else {
                        s.indent_nan = true;
                    }
                    s.soy_state.pop();
                    return st("keyword");
                } else if stream.matches(re!(r"^([A-Za-z0-9_?]+)(?==)")) {
                    if s.context.last().is_some_and(|c| c.tag == tag_name)
                        && stream.current() == "kind"
                        && let Some(m) = stream.match_re(re!(r#"^="([^"]+)"#), false)
                    {
                        let kind = m.group(1).unwrap_or("").to_string();
                        let local = match kind.as_str() {
                            "attributes" | "text" | "uri" | "trusted_resource_uri" => Local::Text,
                            "css" => Local::Css,
                            "js" => Local::Js,
                            _ => Local::Html,
                        };
                        if let Some(cx) = s.context.last_mut() {
                            cx.kind = Some(kind);
                        }
                        s.local_states
                            .push((local, self.local(local).start_state()));
                    }
                    return st("attribute");
                }
                return Self::expression(stream, s);
            }
            Some("template-call-expression") => {
                if stream.matches(re!(r"^([A-Za-z0-9_\-?]+)(?==)")) {
                    return st("attribute");
                } else if stream.eat('>').is_some() {
                    s.soy_state.pop();
                    return st("keyword");
                }
                // `stream.eat('/>')` compares one char with "/>": never
                return Self::expression(stream, s);
            }
            Some("literal") => {
                if stream.match_str("{/literal}", false, false) {
                    s.soy_state.pop();
                    return self.token_soy(stream, s);
                }
                return self.token_until(stream, s, re!(r"\{\/literal}"));
            }
            Some("export") => {
                if let Some(m) = stream.match_re(re!(r"[A-Za-z0-9_]+"), true) {
                    s.soy_state.pop();
                    if m.text == "const" {
                        s.soy_state.push("const-def");
                        return st("keyword");
                    } else if m.text == "extern" {
                        s.soy_state.push("param-def");
                        return st("keyword");
                    }
                } else {
                    stream.next();
                }
                return None;
            }
            Some("const-def") => {
                if stream.matches(re!(r"^[A-Za-z0-9_]+")) {
                    s.soy_state.pop();
                    return st("def");
                }
                stream.next();
                return None;
            }
            _ => {}
        }

        if stream.match_str("{literal}", true, false) {
            s.indent_nan = true;
            s.soy_state.push("literal");
            s.context.push(Context {
                tag: "literal".to_string(),
                kind: None,
                scope: s.variables.clone(),
            });
            return st("keyword");
        // A tag-keyword must be followed by whitespace, comment or a closing tag.
        } else if let Some(m) =
            stream.match_re(re!(r"^\{([/@\\]?[A-Za-z0-9_]+\??)(?=$|[\s}]|\/[/*])"), true)
        {
            let tag = m.group(1).unwrap_or("").to_string();
            let end_tag = tag.starts_with('/');
            let indenting_tag = tag_def(&tag).is_some();
            let tag_name = if end_tag { &tag[1..] } else { &tag[..] };
            let def = tag_def(tag_name);
            if tag != "/switch" {
                s.indent_nan = true;
            }
            s.soy_state.push("tag");
            let mut tag_error = false;
            if let Some((no_end_tag, soy_state, variable_scope)) = def {
                if !end_tag && let Some(st) = soy_state {
                    s.soy_state.push(st);
                }
                // If a new tag, open a new context.
                if !no_end_tag && (indenting_tag || !end_tag) {
                    s.context.push(Context {
                        tag: tag.clone(),
                        kind: None,
                        scope: if variable_scope {
                            s.variables.clone()
                        } else {
                            None
                        },
                    });
                // Otherwise close the current context.
                } else if end_tag {
                    let balanced_for_extern =
                        tag_name == "extern" && s.context.last().is_some_and(|c| c.tag == "export");
                    match s.context.last() {
                        None => tag_error = true,
                        Some(c) if c.tag != tag_name && !balanced_for_extern => tag_error = true,
                        Some(c) => {
                            if c.kind.is_some() {
                                s.local_states.pop();
                            }
                            popcontext(s);
                        }
                    }
                }
            } else if end_tag {
                // Assume all tags with a closing tag are defined in the config.
                tag_error = true;
            }
            s.tag = Some(tag);
            return st(if tag_error {
                "error keyword"
            } else {
                "keyword"
            });
        // Not a tag-keyword; it's an implicit print tag.
        } else if stream.eat('{').is_some() {
            s.tag = Some("print".to_string());
            s.indent_nan = true;
            s.soy_state.push("tag");
            return st("keyword");
        } else if s.context.is_empty()
            && stream.sol()
            && stream.matches(re!(r"import(?![A-Za-z0-9_])"))
        {
            s.soy_state.push("import");
            s.indent_nan = true;
            return st("keyword");
        } else if stream.match_str("<{", true, false) {
            s.soy_state.push("template-call-expression");
            s.indent_nan = true;
            s.soy_state.push("tag");
            return st("keyword");
        } else if stream.match_str("</>", true, false) {
            s.indent_nan = true;
            return st("keyword");
        }

        self.token_until(stream, s, re!(r"\{|\s+\/\/|\/\*"))
    }
}

impl Mode for Soy {
    fn name(&self) -> &'static str {
        "soy"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SoyState {
            soy_state: Vec::new(),
            variables: prepend(&None, "ij"),
            indent_nan: false,
            quote_kind: None,
            context: Vec::new(),
            lookup_variables: true,
            local_states: vec![(Local::Html, self.html.start_state())],
            tag: None,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        self.token_soy(stream, state::<SoyState>(st))
    }

    /// `innerMode`: soy while a Soy construct is open, else the top local
    /// mode
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        let s = state_ref::<SoyState>(st);
        if s.soy_state.last().is_some_and(|t| *t != "literal") {
            return "soy";
        }
        match s.local_states.last() {
            Some((local, local_state)) => self.local(*local).inner_mode_name(&**local_state),
            None => "soy",
        }
    }
}

/// `CodeMirror.getMode({}, "text/x-soy")`
pub fn soy() -> Arc<dyn Mode> {
    static MODE: std::sync::OnceLock<Arc<dyn Mode>> = std::sync::OnceLock::new();
    MODE.get_or_init(|| Arc::new(Soy::new())).clone()
}
