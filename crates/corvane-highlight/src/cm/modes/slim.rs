//! `codemirror/mode/slim/slim.js` (`text/x-slim`, `application/x-slim`),
//! ported line by line; the JS function names are kept on the Rust ones.
//!
//! slim.js swaps `state.tokenize` / `state.line` between named functions
//! and closures (`backup`, `lineContinuable`, `commaContinuable`,
//! `rubyInQuote`, `startRubySplat`, `startHtmlLine`, `readQuoted`), and
//! `finishContinue` compares them by identity. Here they are [`Tk`] values
//! behind `Rc`: a closure is identical only to itself (`Rc::ptr_eq`), a
//! named function to any value of its variant.
//!
//! Embedded blocks (`javascript:`, `css:` …) resolve like `createMode` with
//! only htmlmixed and ruby (and what they require: xml, javascript, css)
//! loaded: `ruby` is the shared ruby mode, `javascript` comes from the
//! mode factory directly (no `name`, so its tokens carry no `m-` class),
//! `css` / `scss` / `less` are css.js MIME configurations, anything else is
//! the `null` mode. Each is one instance per slim instance, so the
//! javascript / css closure scratch carries from one block to the next.
//!
//! `sub` runs the block's mode on `stream.string.slice(indented)`, where
//! `indented` is a column: with tabs in a block's indentation the JS slices
//! at the wrong place (and can go negative); only the space-indented case
//! is modelled.

use std::rc::Rc;
use std::sync::Arc;

use super::super::{Mode, ModeState, StringStream, js_len, state};
use super::htmlmixed::NullMode;
use super::javascript::JsState;
use super::ruby::{Ruby, has_context_prev, tokenize_depth};
use crate::re;

/// `nameStartChar`
macro_rules! name_start {
    () => {
        "_a-zA-Z\u{C0}-\u{D6}\u{D8}-\u{F6}\u{F8}-\u{2FF}\u{370}-\u{37D}\u{37F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}"
    };
}
/// `nameChar`
macro_rules! name_char {
    () => {
        concat!(
            name_start!(),
            r"\-0-9\u{B7}\u{300}-\u{36F}\u{203F}-\u{2040}"
        )
    };
}

/// `state.tokenize` / `state.line` / a stack entry's `tokenize`.
enum Tk {
    Slim,
    Html,
    Ruby,
    Comment,
    Sub,
    FirstSub,
    DoctypeLine,
    AttributeWrapper,
    AttributeWrapperAssign,
    AttributeWrapperValue,
    SlimTag,
    SlimTagExtras,
    SlimClass,
    SlimAttribute,
    SlimAttributeAssign,
    SlimAttributeValue,
    SlimAttributeSymbols,
    SlimContent,
    /// the function `backup(pos, tokenize, style)` returns
    Backup {
        pos: usize,
        tokenize: Rc<Tk>,
        style: Option<String>,
    },
    /// its `restore`
    Restore {
        pos: usize,
        tokenize: Rc<Tk>,
        style: Option<String>,
    },
    /// `lineContinuable(column, tokenize)`
    LineContinuable {
        column: usize,
        tokenize: Rc<Tk>,
    },
    /// `commaContinuable(column, tokenize)`
    CommaContinuable {
        column: usize,
        tokenize: Rc<Tk>,
    },
    /// `rubyInQuote(endQuote, tokenize)`
    RubyInQuote {
        end_quote: char,
        tokenize: Rc<Tk>,
    },
    /// the function `startRubySplat(tokenize)` returns
    StartRubySplat {
        tokenize: Rc<Tk>,
    },
    /// its `runSplat`, with the ruby state it saved
    RunSplat {
        ruby_state: Box<dyn ModeState>,
        tokenize: Rc<Tk>,
    },
    /// `startHtmlLine(lastTokenize)`
    StartHtmlLine {
        last: Rc<Tk>,
    },
    /// `readQuoted(quote, style, embed, unescaped, nextTokenize)`
    ReadQuoted {
        quote: char,
        style: &'static str,
        embed: bool,
        unescaped: bool,
        next: Rc<Tk>,
    },
}

impl Tk {
    fn is_closure(&self) -> bool {
        matches!(
            self,
            Tk::Backup { .. }
                | Tk::Restore { .. }
                | Tk::LineContinuable { .. }
                | Tk::CommaContinuable { .. }
                | Tk::RubyInQuote { .. }
                | Tk::StartRubySplat { .. }
                | Tk::RunSplat { .. }
                | Tk::StartHtmlLine { .. }
                | Tk::ReadQuoted { .. }
        )
    }
}

/// JS `==` on two tokenize functions.
fn same(a: &Rc<Tk>, b: &Rc<Tk>) -> bool {
    Rc::ptr_eq(a, b)
        || (!a.is_closure() && std::mem::discriminant(&**a) == std::mem::discriminant(&**b))
}

fn named(t: Tk) -> Rc<Tk> {
    Rc::new(t)
}

/// A `state.stack` entry.
#[derive(Clone)]
struct Frame {
    style: &'static str,
    indented: usize,
    tokenize: Rc<Tk>,
    /// `line` / `endQuote` of an attribute wrapper
    line: Option<Rc<Tk>>,
    end_quote: Option<char>,
}

/// One of `embedded`'s modes, as `getMode(mode)` caches it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sub {
    Ruby,
    Javascript,
    Css,
    Scss,
    Less,
    Null,
}

#[derive(Clone)]
struct SlimState {
    html_state: Box<dyn ModeState>,
    ruby_state: Box<dyn ModeState>,
    sub_mode: Option<Sub>,
    sub_state: Option<Box<dyn ModeState>>,
    /// innermost last
    stack: Vec<Frame>,
    /// `state.last == "slimSubmode"`
    last_is_submode: bool,
    tokenize: Rc<Tk>,
    line: Rc<Tk>,
    indented: usize,
    start_of_line: bool,
    /// the last state of each cached block mode, for its instance scratch
    retired: Vec<(Sub, Box<dyn ModeState>)>,
}

pub struct Slim {
    html: Arc<dyn Mode>,
    ruby: Ruby,
    javascript: Arc<dyn Mode>,
    css: Arc<dyn Mode>,
    scss: Arc<dyn Mode>,
    less: Arc<dyn Mode>,
    null: Arc<dyn Mode>,
}

impl Default for Slim {
    fn default() -> Self {
        Self::new()
    }
}

type Style = Option<String>;

fn st(s: &str) -> Style {
    Some(s.to_string())
}

impl Slim {
    pub fn new() -> Self {
        let css = |mime| super::css::css_for_mime(mime).unwrap_or_else(|| Arc::new(NullMode));
        Self {
            html: super::htmlmixed(),
            ruby: Ruby,
            javascript: super::javascript(),
            css: css("text/css"),
            scss: css("text/x-scss"),
            less: css("text/x-less"),
            null: Arc::new(NullMode),
        }
    }

    fn sub_mode(&self, sub: Sub) -> &dyn Mode {
        match sub {
            Sub::Ruby => &self.ruby,
            Sub::Javascript => &*self.javascript,
            Sub::Css => &*self.css,
            Sub::Scss => &*self.scss,
            Sub::Less => &*self.less,
            Sub::Null => &*self.null,
        }
    }

    fn call(&self, tk: Rc<Tk>, stream: &mut StringStream, s: &mut SlimState) -> Style {
        match &*tk {
            Tk::Slim => self.slim(stream, s),
            Tk::Html => self.html(stream, s),
            Tk::Ruby => self.ruby(stream, s),
            Tk::Comment => Self::comment(stream, s),
            Tk::Sub => self.sub(stream, s),
            Tk::FirstSub => self.first_sub(stream, s),
            Tk::DoctypeLine => {
                stream.skip_to_end();
                st("slimDoctype")
            }
            Tk::AttributeWrapper => Self::attribute_wrapper(stream, s),
            Tk::AttributeWrapperAssign => Self::attribute_wrapper_assign(stream, s),
            Tk::AttributeWrapperValue => self.attribute_wrapper_value(stream, s),
            Tk::SlimTag => self.slim_tag(stream, s),
            Tk::SlimTagExtras => self.slim_tag_extras(stream, s),
            Tk::SlimClass => self.slim_class(stream, s),
            Tk::SlimAttribute => self.slim_attribute(stream, s),
            Tk::SlimAttributeAssign => self.slim_attribute_assign(stream, s),
            Tk::SlimAttributeValue => self.slim_attribute_value(stream, s),
            Tk::SlimAttributeSymbols => self.slim_attribute_symbols(stream, s),
            Tk::SlimContent => self.slim_content(stream, s),
            Tk::Backup {
                pos,
                tokenize,
                style,
            } => {
                s.tokenize = Rc::new(Tk::Restore {
                    pos: *pos,
                    tokenize: tokenize.clone(),
                    style: style.clone(),
                });
                self.call(tokenize.clone(), stream, s)
            }
            Tk::Restore {
                pos,
                tokenize,
                style,
            } => {
                s.tokenize = tokenize.clone();
                if stream.pos < *pos {
                    stream.pos = *pos;
                    return style.clone();
                }
                self.call(s.tokenize.clone(), stream, s)
            }
            Tk::LineContinuable { column, tokenize } => {
                Self::finish_continue(s);
                if stream.matches(re!(r"^\\$")) {
                    Self::continue_line(s, *column);
                    return st("lineContinuation");
                }
                let style = self.call(tokenize.clone(), stream, s);
                if stream.eol()
                    && re!(r"(?:^|[^\\])(?:\\\\)*\\$")
                        .is_match(stream.current())
                        .unwrap_or(false)
                {
                    stream.back_up(1);
                }
                style
            }
            Tk::CommaContinuable { column, tokenize } => {
                Self::finish_continue(s);
                let style = self.call(tokenize.clone(), stream, s);
                if stream.eol() && stream.current().ends_with(',') {
                    Self::continue_line(s, *column);
                }
                style
            }
            Tk::RubyInQuote {
                end_quote,
                tokenize,
            } => {
                if stream.peek() == Some(*end_quote) && tokenize_depth(&*s.ruby_state) == 1 {
                    // step out of ruby context as it seems to complete
                    // processing all the braces
                    stream.next();
                    s.tokenize = tokenize.clone();
                    return st("closeAttributeTag");
                }
                self.ruby(stream, s)
            }
            Tk::StartRubySplat { tokenize } => self.start_ruby_splat(tokenize.clone(), stream, s),
            Tk::RunSplat {
                ruby_state,
                tokenize,
            } => {
                if tokenize_depth(&*s.ruby_state) == 1 && !has_context_prev(&*s.ruby_state) {
                    stream.back_up(1);
                    if stream.eat_space() {
                        s.ruby_state = ruby_state.clone();
                        s.tokenize = tokenize.clone();
                        return self.call(tokenize.clone(), stream, s);
                    }
                    stream.next();
                }
                self.ruby(stream, s)
            }
            Tk::StartHtmlLine { last } => {
                let style = self.html_line(stream, s);
                if stream.eol() {
                    s.tokenize = last.clone();
                }
                style
            }
            Tk::ReadQuoted {
                quote,
                style,
                embed,
                unescaped,
                next,
            } => {
                Self::finish_continue(s);
                let fresh = stream.pos == stream.start;
                if stream.match_re(re!(r"^\\$"), fresh).is_some() {
                    if !fresh {
                        return st(style);
                    }
                    let indented = s.indented;
                    Self::continue_line(s, indented);
                    return st("lineContinuation");
                }
                if stream.match_re(re!(r"^#\{"), fresh).is_some() {
                    if !fresh {
                        return st(style);
                    }
                    s.tokenize = Rc::new(Tk::RubyInQuote {
                        end_quote: '}',
                        tokenize: s.tokenize.clone(),
                    });
                    return None;
                }
                let mut escaped = false;
                while let Some(ch) = stream.next() {
                    if ch == *quote && (*unescaped || !escaped) {
                        s.tokenize = next.clone();
                        break;
                    }
                    if *embed && ch == '#' && !escaped && stream.eat('{').is_some() {
                        stream.back_up(2);
                        break;
                    }
                    escaped = !escaped && ch == '\\';
                }
                if stream.eol() && escaped {
                    stream.back_up(1);
                }
                st(style)
            }
        }
    }

    /// `continueLine(state, column)`
    fn continue_line(s: &mut SlimState, column: usize) {
        s.stack.push(Frame {
            style: "continuation",
            indented: column,
            tokenize: s.line.clone(),
            line: None,
            end_quote: None,
        });
        s.line = s.tokenize.clone();
    }

    /// `finishContinue(state)`
    fn finish_continue(s: &mut SlimState) {
        if same(&s.line, &s.tokenize)
            && let Some(top) = s.stack.pop()
        {
            s.line = top.tokenize;
        }
    }

    /// the function `startRubySplat(tokenize)` returns
    fn start_ruby_splat(
        &self,
        next: Rc<Tk>,
        stream: &mut StringStream,
        s: &mut SlimState,
    ) -> Style {
        let saved = std::mem::replace(&mut s.ruby_state, self.ruby.start_state());
        s.tokenize = Rc::new(Tk::RunSplat {
            ruby_state: saved,
            tokenize: next,
        });
        self.ruby(stream, s)
    }

    /// `ruby`
    fn ruby(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        self.ruby.token(stream, &mut *s.ruby_state)
    }

    /// `htmlLine`
    fn html_line(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.matches(re!(r"^\\$")) {
            return st("lineContinuation");
        }
        self.html(stream, s)
    }

    /// `html`
    fn html(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.matches(re!(r"^#\{")) {
            s.tokenize = Rc::new(Tk::RubyInQuote {
                end_quote: '}',
                tokenize: s.tokenize.clone(),
            });
            return None;
        }
        let style = self.html.token(stream, &mut *s.html_state);
        // maybeBackup(stream, state, /[^\\]#\{/, 1, style)
        let cur = stream.current();
        if let Some(m) = re!(r"[^\\]#\{").find(cur).ok().flatten() {
            let idx = js_len(&cur[..m.start()]);
            let n = js_len(cur) - idx - 1;
            s.tokenize = Rc::new(Tk::Backup {
                pos: stream.pos,
                tokenize: s.tokenize.clone(),
                style: style.clone(),
            });
            stream.back_up(n);
        }
        style
    }

    /// `startHtmlMode(stream, state, offset)`
    fn start_html_mode(stream: &mut StringStream, s: &mut SlimState, offset: usize) -> Style {
        let indented = stream.column() + offset;
        s.stack.push(Frame {
            style: "html",
            indented,
            tokenize: s.line.clone(),
            line: None,
            end_quote: None,
        });
        let html = named(Tk::Html);
        s.line = html.clone();
        s.tokenize = html;
        None
    }

    /// `comment`
    fn comment(stream: &mut StringStream, s: &mut SlimState) -> Style {
        stream.skip_to_end();
        s.stack.last().map(|f| f.style.to_string())
    }

    /// `commentMode`
    fn comment_mode(stream: &mut StringStream, s: &mut SlimState) -> Style {
        s.stack.push(Frame {
            style: "comment",
            indented: s.indented + 1,
            tokenize: s.line.clone(),
            line: None,
            end_quote: None,
        });
        s.line = named(Tk::Comment);
        Self::comment(stream, s)
    }

    /// `attributeWrapper`
    fn attribute_wrapper(stream: &mut StringStream, s: &mut SlimState) -> Style {
        if let Some(q) = s.stack.last().and_then(|f| f.end_quote)
            && stream.eat(q).is_some()
            && let Some(top) = s.stack.pop()
        {
            if let Some(line) = top.line {
                s.line = line;
            }
            s.tokenize = top.tokenize;
            return None;
        }
        if stream.matches(re!(concat!(
            "^[:",
            name_start!(),
            r"][:\.",
            name_char!(),
            "]*"
        ))) {
            s.tokenize = named(Tk::AttributeWrapperAssign);
            return st("slimAttribute");
        }
        stream.next();
        None
    }

    /// `attributeWrapperAssign`
    fn attribute_wrapper_assign(stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.matches(re!(r"^==?")) {
            s.tokenize = named(Tk::AttributeWrapperValue);
            return None;
        }
        Self::attribute_wrapper(stream, s)
    }

    /// `attributeWrapperValue`
    fn attribute_wrapper_value(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        let ch = stream.peek();
        if let Some(q @ ('"' | '\'')) = ch {
            let t = Rc::new(Tk::ReadQuoted {
                quote: q,
                style: "string",
                embed: true,
                unescaped: false,
                next: named(Tk::AttributeWrapper),
            });
            s.tokenize = t.clone();
            stream.next();
            return self.call(t, stream, s);
        }
        if ch == Some('[') {
            return self.start_ruby_splat(named(Tk::AttributeWrapper), stream, s);
        }
        if stream.matches(re!(r"^(true|false|nil)(?![A-Za-z0-9_])")) {
            s.tokenize = named(Tk::AttributeWrapper);
            return st("keyword");
        }
        self.start_ruby_splat(named(Tk::AttributeWrapper), stream, s)
    }

    /// `startAttributeWrapperMode(state, endQuote, tokenize)`
    fn start_attribute_wrapper_mode(s: &mut SlimState, end_quote: char, tokenize: Rc<Tk>) -> Style {
        s.stack.push(Frame {
            style: "wrapper",
            indented: s.indented + 1,
            tokenize,
            line: Some(s.line.clone()),
            end_quote: Some(end_quote),
        });
        let t = named(Tk::AttributeWrapper);
        s.line = t.clone();
        s.tokenize = t;
        None
    }

    /// `sub`
    fn sub(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.matches(re!(r"^#\{")) {
            s.tokenize = Rc::new(Tk::RubyInQuote {
                end_quote: '}',
                tokenize: s.tokenize.clone(),
            });
            return None;
        }
        let indented = s.stack.last().map_or(0, |f| f.indented);
        let (Some(sub), Some(sub_state)) = (s.sub_mode, s.sub_state.as_mut()) else {
            return None;
        };
        let mut sub_stream = stream.tail(indented);
        sub_stream.pos = stream.pos.saturating_sub(indented);
        sub_stream.start = stream.start.saturating_sub(indented);
        let style = self.sub_mode(sub).token(&mut sub_stream, &mut **sub_state);
        stream.pos = sub_stream.pos + indented;
        style
    }

    /// `firstSub`
    fn first_sub(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        let column = stream.column();
        if let Some(top) = s.stack.last_mut() {
            top.indented = column;
        }
        let t = named(Tk::Sub);
        s.line = t.clone();
        s.tokenize = t;
        self.sub(stream, s)
    }

    /// `startSubMode(mode, state)`
    fn start_sub_mode(&self, mode: &str, s: &mut SlimState) -> Style {
        // getMode(mode) → createMode(mode) over `embedded`
        let sub = match mode {
            "ruby" => Sub::Ruby,
            "javascript" => Sub::Javascript,
            "css" => Sub::Css,
            "scss" => Sub::Scss,
            "less" => Sub::Less,
            _ => Sub::Null,
        };
        retire(s);
        let mut sub_state = self.sub_mode(sub).start_state();
        // the cached instance keeps its closure scratch between blocks
        if let Some((_, prev)) = s.retired.iter().find(|(k, _)| *k == sub) {
            carry(&**prev, &mut *sub_state);
        }
        s.sub_mode = Some(sub);
        s.sub_state = Some(sub_state);
        s.stack.push(Frame {
            style: "sub",
            indented: s.indented + 1,
            tokenize: s.line.clone(),
            line: None,
            end_quote: None,
        });
        let t = named(Tk::FirstSub);
        s.line = t.clone();
        s.tokenize = t;
        st("slimSubmode")
    }

    /// `startLine`
    fn start_line(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.peek() == Some('<') {
            let t = Rc::new(Tk::StartHtmlLine {
                last: s.tokenize.clone(),
            });
            s.tokenize = t.clone();
            return self.call(t, stream, s);
        }
        if stream.matches(re!(r"^[|']")) {
            return Self::start_html_mode(stream, s, 1);
        }
        if stream.matches(re!(r"^\/(!|\[[A-Za-z0-9_]+])?")) {
            return Self::comment_mode(stream, s);
        }
        if stream.matches(re!(r"^(-|==?[<>]?)")) {
            let column = stream.column();
            s.tokenize = Rc::new(Tk::LineContinuable {
                column,
                tokenize: Rc::new(Tk::CommaContinuable {
                    column,
                    tokenize: named(Tk::Ruby),
                }),
            });
            return st("slimSwitch");
        }
        if stream.matches(re!(r"^doctype(?![A-Za-z0-9_])")) {
            s.tokenize = named(Tk::DoctypeLine);
            return st("keyword");
        }
        if let Some(m) = stream.match_re(
            re!(
                r"^(ruby|javascript|css|sass|scss|less|styl|coffee|asciidoc|markdown|textile|creole|wiki|mediawiki|rdoc|builder|nokogiri|erb):"
            ),
            true,
        ) {
            return self.start_sub_mode(m.group(1).unwrap_or(""), s);
        }
        self.slim_tag(stream, s)
    }

    /// `slim`
    fn slim(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if s.start_of_line {
            return self.start_line(stream, s);
        }
        self.slim_tag(stream, s)
    }

    /// `slimTag`
    fn slim_tag(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.eat('*').is_some() {
            s.tokenize = Rc::new(Tk::StartRubySplat {
                tokenize: named(Tk::SlimTagExtras),
            });
            return None;
        }
        if stream.matches(re!(concat!(
            "^[:",
            name_start!(),
            "](?::[",
            name_char!(),
            "]|[",
            name_char!(),
            "]*)"
        ))) {
            s.tokenize = named(Tk::SlimTagExtras);
            return st("slimTag");
        }
        self.slim_class(stream, s)
    }

    /// `slimTagExtras`
    fn slim_tag_extras(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.matches(re!(r"^(<>?|><?)")) {
            s.tokenize = named(Tk::SlimClass);
            return None;
        }
        self.slim_class(stream, s)
    }

    /// `slimClass`
    fn slim_class(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.matches(re!(r"^#[_a-zA-Z]+[A-Za-z0-9_\-]*")) {
            s.tokenize = named(Tk::SlimClass);
            return st("slimId");
        }
        if stream.matches(re!(r"^\.-?[_a-zA-Z]+[A-Za-z0-9_\-]*")) {
            s.tokenize = named(Tk::SlimClass);
            return st("slimClass");
        }
        self.slim_attribute(stream, s)
    }

    /// `slimAttribute`
    fn slim_attribute(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if let Some(m) = stream.match_re(re!(r"^([\[\{\(])"), true) {
            let close = match m.text.as_str() {
                "[" => ']',
                "{" => '}',
                _ => ')',
            };
            return Self::start_attribute_wrapper_mode(s, close, named(Tk::SlimAttribute));
        }
        if stream.matches(re!(concat!(
            "^[:",
            name_start!(),
            r"][:\.",
            name_char!(),
            r"]*(?=\s*=)"
        ))) {
            s.tokenize = named(Tk::SlimAttributeAssign);
            return st("slimAttribute");
        }
        if stream.peek() == Some('*') {
            stream.next();
            s.tokenize = Rc::new(Tk::StartRubySplat {
                tokenize: named(Tk::SlimContent),
            });
            return None;
        }
        self.slim_content(stream, s)
    }

    /// `slimAttributeAssign`
    fn slim_attribute_assign(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.matches(re!(r"^==?")) {
            s.tokenize = named(Tk::SlimAttributeValue);
            return None;
        }
        // should never happen, because of forward lookup
        self.slim_attribute(stream, s)
    }

    /// `slimAttributeValue`
    fn slim_attribute_value(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        let ch = stream.peek();
        if let Some(q @ ('"' | '\'')) = ch {
            let t = Rc::new(Tk::ReadQuoted {
                quote: q,
                style: "string",
                embed: true,
                unescaped: false,
                next: named(Tk::SlimAttribute),
            });
            s.tokenize = t.clone();
            stream.next();
            return self.call(t, stream, s);
        }
        if ch == Some('[') {
            return self.start_ruby_splat(named(Tk::SlimAttribute), stream, s);
        }
        if ch == Some(':') {
            return self.start_ruby_splat(named(Tk::SlimAttributeSymbols), stream, s);
        }
        if stream.matches(re!(r"^(true|false|nil)(?![A-Za-z0-9_])")) {
            s.tokenize = named(Tk::SlimAttribute);
            return st("keyword");
        }
        self.start_ruby_splat(named(Tk::SlimAttribute), stream, s)
    }

    /// `slimAttributeSymbols`
    fn slim_attribute_symbols(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        stream.back_up(1);
        if stream.matches(re!(r"^[^\s],(?=:)")) {
            s.tokenize = Rc::new(Tk::StartRubySplat {
                tokenize: named(Tk::SlimAttributeSymbols),
            });
            return None;
        }
        stream.next();
        self.slim_attribute(stream, s)
    }

    /// `slimContent`
    fn slim_content(&self, stream: &mut StringStream, s: &mut SlimState) -> Style {
        if stream.matches(re!(r"^==?")) {
            s.tokenize = named(Tk::Ruby);
            return st("slimSwitch");
        }
        if stream.matches(re!(r"^\/$")) {
            // tag close hint
            s.tokenize = named(Tk::Slim);
            return None;
        }
        if stream.matches(re!(r"^:")) {
            // inline tag
            s.tokenize = named(Tk::SlimTag);
            return st("slimSwitch");
        }
        Self::start_html_mode(stream, s, 0);
        self.call(s.tokenize.clone(), stream, s)
    }
}

/// Carry a cached block mode's closure scratch into the next block's
/// fresh state.
fn carry(prev: &dyn ModeState, next: &mut dyn ModeState) {
    if let (Some(prev), Some(next)) = (
        prev.as_any().downcast_ref::<JsState>(),
        next.as_any_mut().downcast_mut::<JsState>(),
    ) {
        next.scratch = prev.scratch.clone();
    }
    super::css::carry_type(prev, next);
}

/// `styleMap`
fn map_style(style: Style) -> Style {
    match style.as_deref() {
        Some("commentLine") => st("comment"),
        Some("slimSwitch") => st("operator special"),
        Some("slimTag") => st("tag"),
        Some("slimId") => st("attribute def"),
        Some("slimClass") => st("attribute qualifier"),
        Some("slimAttribute") => st("attribute"),
        Some("slimSubmode") => st("keyword special"),
        Some("closeAttributeTag" | "slimDoctype" | "lineContinuation") => None,
        _ => style,
    }
}

impl Mode for Slim {
    fn name(&self) -> &'static str {
        "slim"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SlimState {
            html_state: self.html.start_state(),
            ruby_state: self.ruby.start_state(),
            sub_mode: None,
            sub_state: None,
            stack: Vec::new(),
            last_is_submode: false,
            tokenize: named(Tk::Slim),
            line: named(Tk::Slim),
            indented: 0,
            start_of_line: false,
            retired: Vec::new(),
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<SlimState>(st);
        if stream.sol() {
            s.indented = stream.indentation();
            s.start_of_line = true;
            s.tokenize = s.line.clone();
            while !s.last_is_submode
                && let Some(top) = s.stack.last()
                && top.indented > s.indented
            {
                let t = top.tokenize.clone();
                s.line = t.clone();
                s.tokenize = t;
                s.stack.pop();
                retire(s);
            }
        }
        if stream.eat_space() {
            return None;
        }
        let tk = s.tokenize.clone();
        let style = self.call(tk, stream, s);
        s.start_of_line = false;
        if let Some(style) = style.as_deref().filter(|st| !st.is_empty()) {
            s.last_is_submode = style == "slimSubmode";
        }
        map_style(style)
    }

    fn blank_line(&self, st: &mut dyn ModeState) {
        let s = state::<SlimState>(st);
        if let (Some(sub), Some(sub_state)) = (s.sub_mode, s.sub_state.as_mut()) {
            self.sub_mode(sub).blank_line(&mut **sub_state);
        }
    }

    /// `innerMode`: the block's mode while one runs
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        let s = super::super::state_ref::<SlimState>(st);
        match (s.sub_mode, s.sub_state.as_ref()) {
            // made by the mode factory, without getMode's `name`
            (Some(Sub::Javascript), _) => "",
            (Some(sub), Some(sub_state)) => self.sub_mode(sub).inner_mode_name(&**sub_state),
            _ => "slim",
        }
    }
}

/// `state.subMode = null; state.subState = null`, keeping the state for
/// the cached instance's scratch.
fn retire(s: &mut SlimState) {
    if let (Some(sub), Some(state)) = (s.sub_mode.take(), s.sub_state.take()) {
        match s.retired.iter_mut().find(|(k, _)| *k == sub) {
            Some(slot) => slot.1 = state,
            None => s.retired.push((sub, state)),
        }
    }
}

/// `CodeMirror.getMode({}, "text/x-slim")`
pub fn slim() -> Arc<dyn Mode> {
    static MODE: std::sync::OnceLock<Arc<dyn Mode>> = std::sync::OnceLock::new();
    MODE.get_or_init(|| Arc::new(Slim::new())).clone()
}
