//! `codemirror/mode/markdown/markdown.js` (`text/markdown`,
//! `text/x-markdown`) with the default options GHD runs it with: no
//! `highlightFormatting`, task lists, strikethrough or emoji, fenced code
//! highlighting on, inline HTML on.
//!
//! Ported function by function (JS names in comments); `state.f`,
//! `state.block` and `state.inline` become [`F`]. HTML blocks run
//! `CodeMirror.getMode(cmCfg, "text/html")`, which in GHD's worker (only
//! markdown.js, xml.js and meta.js loaded) is xml.js's `htmlMode`
//! definition. Fenced code blocks look their language up with meta.js's
//! `findModeByName` and `CodeMirror.getMode`; with only those modes
//! loaded, just XML, HTML and Markdown fences get an inner mode, every
//! other fence is plain `comment` ([`get_mode`]).
//!
//! JS compares `stream != state.thisLine.stream` to spot a new line; GHD
//! makes one stream per line, so the line index stands in for it.

use std::sync::Arc;

use fancy_regex::Regex;

use crate::re;

use super::super::{Mode, ModeState, StringStream, js_len, state, state_ref};
use super::xml::{XmlConfig, XmlMode, XmlState};

/// `state.f` / `state.block` / `state.inline`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum F {
    BlockNormal,
    HtmlBlock,
    Local,
    InlineNormal,
    LinkInline,
    LinkHref,
    /// `getLinkHrefInside(endChar)`
    LinkHrefInside(char),
    FootnoteLink,
    FootnoteLinkInside,
    FootnoteUrl,
}

/// `state.prevLine` / `state.thisLine`
#[derive(Clone, Copy, Default, Debug)]
struct LineInfo {
    /// `stream`: the line index, `None` for `{stream: null}`
    stream: Option<usize>,
    header: bool,
    hr: bool,
    fenced_code_end: bool,
}

/// `state.list`: `false`, `true` or `null`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum List {
    False,
    True,
    Null,
}

#[derive(Clone)]
pub struct MarkdownState {
    f: F,
    block: F,
    inline: F,
    prev_line: LineInfo,
    this_line: LineInfo,
    html_state: Option<XmlState>,
    indentation: i64,
    /// `null` until the line's first `blockNormal`
    indentation_diff: Option<i64>,
    local_mode: Option<Arc<dyn Mode>>,
    local_state: Option<Box<dyn ModeState>>,
    link_text: bool,
    link_href: bool,
    link_title: bool,
    /// backtick count of the open code span, `-1` in a fenced block
    code: i64,
    /// `false` or the delimiter char
    em: Option<char>,
    strong: Option<char>,
    header: i64,
    setext: i64,
    list: List,
    list_stack: Vec<i64>,
    quote: i64,
    indented_code: bool,
    trailing_space: i64,
    trailing_space_new_line: bool,
    image: bool,
    image_alt_text: bool,
    image_marker: bool,
    md_inside: bool,
    /// `fencedEndRE` (`new RegExp(fence + "+ *$")`): the fence char and
    /// the opening run's length
    fenced_end: Option<(char, usize)>,
}

pub struct Markdown {
    /// `htmlMode`
    html: XmlMode,
}

impl Default for Markdown {
    fn default() -> Self {
        Self::new()
    }
}

/// JS `/\s/` on one char.
fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// `punctuation.test(ch)` for one UTF-16 unit (JS hands it `charAt`, so
/// the astral alternatives never match; an astral char here stands for a
/// lone surrogate).
fn punctuation(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_punctuation();
    }
    matches!(
            c,
            '\u{A1}'
    | '\u{A7}'
    | '\u{AB}'
    | '\u{B6}'
    | '\u{B7}'
    | '\u{BB}'
    | '\u{BF}'
    | '\u{037E}'
    | '\u{0387}'
    | '\u{055A}'..='\u{055F}'
    | '\u{0589}'
    | '\u{058A}'
    | '\u{05BE}'
    | '\u{05C0}'
    | '\u{05C3}'
    | '\u{05C6}'
    | '\u{05F3}'
    | '\u{05F4}'
    | '\u{0609}'
    | '\u{060A}'
    | '\u{060C}'
    | '\u{060D}'
    | '\u{061B}'
    | '\u{061E}'
    | '\u{061F}'
    | '\u{066A}'..='\u{066D}'
    | '\u{06D4}'
    | '\u{0700}'..='\u{070D}'
    | '\u{07F7}'..='\u{07F9}'
    | '\u{0830}'..='\u{083E}'
    | '\u{085E}'
    | '\u{0964}'
    | '\u{0965}'
    | '\u{0970}'
    | '\u{0AF0}'
    | '\u{0DF4}'
    | '\u{0E4F}'
    | '\u{0E5A}'
    | '\u{0E5B}'
    | '\u{0F04}'..='\u{0F12}'
    | '\u{0F14}'
    | '\u{0F3A}'..='\u{0F3D}'
    | '\u{0F85}'
    | '\u{0FD0}'..='\u{0FD4}'
    | '\u{0FD9}'
    | '\u{0FDA}'
    | '\u{104A}'..='\u{104F}'
    | '\u{10FB}'
    | '\u{1360}'..='\u{1368}'
    | '\u{1400}'
    | '\u{166D}'
    | '\u{166E}'
    | '\u{169B}'
    | '\u{169C}'
    | '\u{16EB}'..='\u{16ED}'
    | '\u{1735}'
    | '\u{1736}'
    | '\u{17D4}'..='\u{17D6}'
    | '\u{17D8}'..='\u{17DA}'
    | '\u{1800}'..='\u{180A}'
    | '\u{1944}'
    | '\u{1945}'
    | '\u{1A1E}'
    | '\u{1A1F}'
    | '\u{1AA0}'..='\u{1AA6}'
    | '\u{1AA8}'..='\u{1AAD}'
    | '\u{1B5A}'..='\u{1B60}'
    | '\u{1BFC}'..='\u{1BFF}'
    | '\u{1C3B}'..='\u{1C3F}'
    | '\u{1C7E}'
    | '\u{1C7F}'
    | '\u{1CC0}'..='\u{1CC7}'
    | '\u{1CD3}'
    | '\u{2010}'..='\u{2027}'
    | '\u{2030}'..='\u{2043}'
    | '\u{2045}'..='\u{2051}'
    | '\u{2053}'..='\u{205E}'
    | '\u{207D}'
    | '\u{207E}'
    | '\u{208D}'
    | '\u{208E}'
    | '\u{2308}'..='\u{230B}'
    | '\u{2329}'
    | '\u{232A}'
    | '\u{2768}'..='\u{2775}'
    | '\u{27C5}'
    | '\u{27C6}'
    | '\u{27E6}'..='\u{27EF}'
    | '\u{2983}'..='\u{2998}'
    | '\u{29D8}'..='\u{29DB}'
    | '\u{29FC}'
    | '\u{29FD}'
    | '\u{2CF9}'..='\u{2CFC}'
    | '\u{2CFE}'
    | '\u{2CFF}'
    | '\u{2D70}'
    | '\u{2E00}'..='\u{2E2E}'
    | '\u{2E30}'..='\u{2E42}'
    | '\u{3001}'..='\u{3003}'
    | '\u{3008}'..='\u{3011}'
    | '\u{3014}'..='\u{301F}'
    | '\u{3030}'
    | '\u{303D}'
    | '\u{30A0}'
    | '\u{30FB}'
    | '\u{A4FE}'
    | '\u{A4FF}'
    | '\u{A60D}'..='\u{A60F}'
    | '\u{A673}'
    | '\u{A67E}'
    | '\u{A6F2}'..='\u{A6F7}'
    | '\u{A874}'..='\u{A877}'
    | '\u{A8CE}'
    | '\u{A8CF}'
    | '\u{A8F8}'..='\u{A8FA}'
    | '\u{A8FC}'
    | '\u{A92E}'
    | '\u{A92F}'
    | '\u{A95F}'
    | '\u{A9C1}'..='\u{A9CD}'
    | '\u{A9DE}'
    | '\u{A9DF}'
    | '\u{AA5C}'..='\u{AA5F}'
    | '\u{AADE}'
    | '\u{AADF}'
    | '\u{AAF0}'
    | '\u{AAF1}'
    | '\u{ABEB}'
    | '\u{FD3E}'
    | '\u{FD3F}'
    | '\u{FE10}'..='\u{FE19}'
    | '\u{FE30}'..='\u{FE52}'
    | '\u{FE54}'..='\u{FE61}'
    | '\u{FE63}'
    | '\u{FE68}'
    | '\u{FE6A}'
    | '\u{FE6B}'
    | '\u{FF01}'..='\u{FF03}'
    | '\u{FF05}'..='\u{FF0A}'
    | '\u{FF0C}'..='\u{FF0F}'
    | '\u{FF1A}'
    | '\u{FF1B}'
    | '\u{FF1F}'
    | '\u{FF20}'
    | '\u{FF3B}'..='\u{FF3D}'
    | '\u{FF3F}'
    | '\u{FF5B}'
    | '\u{FF5D}'
    | '\u{FF5F}'..='\u{FF65}'
        )
}

/// `stream.eatSpace()` (`/[\s ]/`, JS `\s`)
fn eat_space(stream: &mut StringStream) -> bool {
    stream.eat_while_if(js_space)
}

/// `getMode(name)`: meta.js `findModeByName`, then `CodeMirror.getMode`,
/// `null` for the `null` mode. Only xml (`text/xml`, `application/xml`,
/// xml.js's `text/html`) and markdown are loaded in GHD's worker.
fn get_mode(name: &str) -> Option<Arc<dyn Mode>> {
    // findModeByName: name or alias, case-insensitively; a hit on any other
    // language names a MIME that is not loaded
    let lower = name.to_lowercase();
    let spec = match lower.as_str() {
        "xml" | "rss" | "wsdl" | "xsd" => "application/xml",
        "html" | "xhtml" => "text/html",
        "markdown" => "text/x-markdown",
        _ => name,
    };
    // getMode → resolveMode
    match spec {
        "text/xml" | "application/xml" | "xml" => Some(super::xml()),
        "text/html" => Some(super::xml_html()),
        "text/markdown" | "text/x-markdown" | "markdown" => Some(super::markdown()),
        s if re!(r"^[A-Za-z0-9_\-]+/[A-Za-z0-9_\-]+\+xml$")
            .is_match(s)
            .unwrap_or(false) =>
        {
            Some(super::xml())
        }
        _ => None,
    }
}

/// `hrRE` (`/^([*\-_])(?:\s*\1){2,}\s*$/`), consuming on a match.
fn match_hr(stream: &mut StringStream) -> bool {
    let Some(c) = stream.peek().filter(|c| matches!(c, '*' | '-' | '_')) else {
        return false;
    };
    let mut n = 0;
    for i in stream.pos + 1..stream.len() {
        match stream.char_at(i) {
            Some(x) if x == c => n += 1,
            Some(x) if js_space(x) => {}
            _ => return false,
        }
    }
    if n < 2 {
        return false;
    }
    stream.skip_to_end();
    true
}

/// `fencedCodeRE` (`/^(~~~+|```+)[ \t]*([\w\/+#-]*)[^\n`]*$/`), consuming
/// the line on a match: the fence char, its run length and the language.
fn match_fence(stream: &mut StringStream) -> Option<(char, usize, String)> {
    let c = stream.peek().filter(|c| matches!(c, '~' | '`'))?;
    let mut i = stream.pos;
    while stream.char_at(i) == Some(c) {
        i += 1;
    }
    let run = i - stream.pos;
    if run < 3 || stream.slice(i, stream.len()).contains('`') {
        return None;
    }
    while matches!(stream.char_at(i), Some(' ' | '\t')) {
        i += 1;
    }
    let lang_start = i;
    while stream
        .char_at(i)
        .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '+' | '#' | '-'))
    {
        i += 1;
    }
    let lang = stream.slice(lang_start, i).to_string();
    stream.skip_to_end();
    Some((c, run, lang))
}

/// `stream.match(state.fencedEndRE)`: at least `n` fence chars, then only
/// spaces to the end of the line.
fn match_fenced_end(stream: &mut StringStream, c: char, n: usize) -> bool {
    let mut i = stream.pos;
    while stream.char_at(i) == Some(c) {
        i += 1;
    }
    if i - stream.pos < n {
        return false;
    }
    while stream.char_at(i) == Some(' ') {
        i += 1;
    }
    if i < stream.len() {
        return false;
    }
    stream.skip_to_end();
    true
}

impl Markdown {
    pub const fn new() -> Self {
        Self {
            html: XmlMode::new(XmlConfig::html()),
        }
    }

    fn start(&self) -> MarkdownState {
        MarkdownState {
            f: F::BlockNormal,
            block: F::BlockNormal,
            inline: F::InlineNormal,
            prev_line: LineInfo::default(),
            this_line: LineInfo::default(),
            html_state: None,
            indentation: 0,
            indentation_diff: None,
            local_mode: None,
            local_state: None,
            link_text: false,
            link_href: false,
            link_title: false,
            code: 0,
            em: None,
            strong: None,
            header: 0,
            setext: 0,
            list: List::False,
            list_stack: Vec::new(),
            quote: 0,
            indented_code: false,
            trailing_space: 0,
            trailing_space_new_line: false,
            image: false,
            image_alt_text: false,
            image_marker: false,
            md_inside: false,
            fenced_end: None,
        }
    }

    fn call(&self, f: F, stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        match f {
            F::BlockNormal => self.block_normal(stream, s),
            F::HtmlBlock => self.html_block(stream, s),
            F::Local => self.local(stream, s),
            F::InlineNormal => self.inline_normal(stream, s),
            F::LinkInline => Self::link_inline(stream, s),
            F::LinkHref => Self::link_href(stream, s),
            F::LinkHrefInside(end) => Self::link_href_inside(end, stream, s),
            F::FootnoteLink => self.footnote_link(stream, s),
            F::FootnoteLinkInside => Self::footnote_link_inside(stream, s),
            F::FootnoteUrl => Self::footnote_url(stream, s),
        }
    }

    /// `switchInline`
    fn switch_inline(
        &self,
        stream: &mut StringStream,
        s: &mut MarkdownState,
        f: F,
    ) -> Option<String> {
        s.f = f;
        s.inline = f;
        self.call(f, stream, s)
    }

    /// `switchBlock`
    fn switch_block(
        &self,
        stream: &mut StringStream,
        s: &mut MarkdownState,
        f: F,
    ) -> Option<String> {
        s.f = f;
        s.block = f;
        self.call(f, stream, s)
    }

    /// The HTML block is over: the xml state is back in text, outside any
    /// tag or element.
    fn html_done(s: &MarkdownState) -> bool {
        s.html_state
            .as_ref()
            .is_some_and(|h| h.tag_start_is_null() && !h.has_context() && h.tokenize_is_in_text())
    }

    /// `blankLine`
    fn blank(s: &mut MarkdownState) {
        s.link_title = false;
        s.link_href = false;
        s.link_text = false;
        s.em = None;
        s.strong = None;
        s.quote = 0;
        s.indented_code = false;
        if s.f == F::HtmlBlock && Self::html_done(s) {
            s.f = F::InlineNormal;
            s.block = F::BlockNormal;
            s.html_state = None;
        }
        s.trailing_space = 0;
        s.trailing_space_new_line = false;
        s.prev_line = s.this_line;
        s.this_line = LineInfo::default();
    }

    /// `blockNormal`
    fn block_normal(&self, stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        let first_token_on_line = stream.column() as i64 == s.indentation;
        let prev_line_is_empty = s.prev_line.stream.is_none();
        let prev_line_is_indented_code = s.indented_code;
        let prev_line_is_hr = s.prev_line.hr;
        let prev_line_is_list = s.list != List::False;
        let max_non_code_indentation = s.list_stack.last().copied().unwrap_or(0) + 3;

        s.indented_code = false;

        let line_indentation = s.indentation;
        // compute once per line (on first token)
        if s.indentation_diff.is_none() {
            s.indentation_diff = Some(s.indentation);
            if prev_line_is_list {
                s.list = List::Null;
                while s.list_stack.last().is_some_and(|&l| line_indentation < l) {
                    s.list_stack.pop();
                    match s.list_stack.last() {
                        Some(&l) => s.indentation = l,
                        None => s.list = List::False,
                    }
                }
                if s.list != List::False
                    && let Some(&l) = s.list_stack.last()
                {
                    s.indentation_diff = Some(line_indentation - l);
                }
            }
        }

        // not comprehensive (currently only for setext detection purposes)
        let allows_inline_continuation = !prev_line_is_empty
            && !prev_line_is_hr
            && !s.prev_line.header
            && (!prev_line_is_list || !prev_line_is_indented_code)
            && !s.prev_line.fenced_code_end;

        let is_hr = (s.list == List::False || prev_line_is_hr || prev_line_is_empty)
            && s.indentation <= max_non_code_indentation
            && match_hr(stream);

        if s.indentation_diff.is_some_and(|d| d >= 4)
            && (prev_line_is_indented_code
                || s.prev_line.fenced_code_end
                || s.prev_line.header
                || prev_line_is_empty)
        {
            stream.skip_to_end();
            s.indented_code = true;
            return Some("comment".into());
        }
        if eat_space(stream) {
            return None;
        }
        if first_token_on_line
            && s.indentation <= max_non_code_indentation
            && let Some(m) = stream.match_re(re!(r"^(#+)(?: |$)"), true)
            && m.group(1).map_or(0, js_len) <= 6
        {
            s.quote = 0;
            s.header = m.group(1).map_or(0, js_len) as i64;
            s.this_line.header = true;
            s.f = s.inline;
            return Self::get_type(s);
        }
        if s.indentation <= max_non_code_indentation && stream.eat('>').is_some() {
            s.quote = if first_token_on_line { 1 } else { s.quote + 1 };
            eat_space(stream);
            return Self::get_type(s);
        }
        if !is_hr
            && s.setext == 0
            && first_token_on_line
            && s.indentation <= max_non_code_indentation
            && stream.matches(re!(r"^(?:[*\-+]|^[0-9]+([.)]))\s+"))
        {
            s.indentation = line_indentation + js_len(stream.current()) as i64;
            s.list = List::True;
            s.quote = 0;
            // Add this list item's content's indentation to the stack
            s.list_stack.push(s.indentation);
            // Reset inline styles which shouldn't propagate across list items
            s.em = None;
            s.strong = None;
            s.code = 0;
            s.f = s.inline;
            return Self::get_type(s);
        }
        if first_token_on_line
            && s.indentation <= max_non_code_indentation
            && let Some((c, n, lang)) = match_fence(stream)
        {
            s.quote = 0;
            s.fenced_end = Some((c, n));
            // try switching mode
            s.local_mode = get_mode(if lang.is_empty() { "text/plain" } else { &lang });
            if let Some(mode) = &s.local_mode {
                s.local_state = Some(mode.start_state());
            }
            s.f = F::Local;
            s.block = F::Local;
            s.code = -1;
            return Self::get_type(s);
        }
        // SETEXT has lowest block-scope precedence after HR, so check it
        // after the others (code, blockquote, list...)
        let setext_line = if s.setext != 0 {
            None
        } else if (!allows_inline_continuation || !prev_line_is_list)
            && s.quote == 0
            && s.list == List::False
            && s.code == 0
            && !is_hr
            && !re!(r"^\s*\[[^\]]+?\]:.*$")
                .is_match(stream.string())
                .unwrap_or(false)
        {
            stream.look_ahead(1).filter(|next| {
                re!(r"^ {0,3}(?:=+|-{2,})\s*$")
                    .is_match(next)
                    .unwrap_or(false)
            })
        } else {
            None
        };
        if s.setext != 0 || setext_line.is_some() {
            if s.setext == 0 {
                s.header = if setext_line.is_some_and(|l| l.starts_with('=')) {
                    1
                } else {
                    2
                };
                s.setext = s.header;
            } else {
                s.header = s.setext;
                // has no effect on type so we can reset it now
                s.setext = 0;
                stream.skip_to_end();
            }
            s.this_line.header = true;
            s.f = s.inline;
            return Self::get_type(s);
        }
        if is_hr {
            stream.skip_to_end();
            s.this_line.hr = true;
            return Some("hr".into());
        }
        if stream.peek() == Some('[') {
            return self.switch_inline(stream, s, F::FootnoteLink);
        }
        self.switch_inline(stream, s, s.inline)
    }

    /// `htmlBlock`
    fn html_block(&self, stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        let style = match s.html_state.as_mut() {
            Some(h) => self.html.token_xml(stream, h).map(|c| c.into_owned()),
            None => None,
        };
        if Self::html_done(s) || (s.md_inside && stream.current().contains('>')) {
            s.f = F::InlineNormal;
            s.block = F::BlockNormal;
            s.html_state = None;
        }
        style
    }

    /// `local`
    fn local(&self, stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        let curr_list_ind = s.list_stack.last().copied().unwrap_or(0);
        let has_exited_list = s.indentation < curr_list_ind;
        let max_fenced_end_ind = curr_list_ind + 3;
        if let Some((c, n)) = s.fenced_end
            && s.indentation <= max_fenced_end_ind
            && (has_exited_list || match_fenced_end(stream, c, n))
        {
            let return_type = if has_exited_list {
                None
            } else {
                Self::get_type(s)
            };
            s.local_mode = None;
            s.local_state = None;
            s.block = F::BlockNormal;
            s.f = F::InlineNormal;
            s.fenced_end = None;
            s.code = 0;
            s.this_line.fenced_code_end = true;
            if has_exited_list {
                return self.switch_block(stream, s, s.block);
            }
            return return_type;
        }
        if let (Some(mode), Some(local)) = (s.local_mode.clone(), s.local_state.as_mut()) {
            return mode.token(stream, &mut **local);
        }
        stream.skip_to_end();
        Some("comment".into())
    }

    /// `getType`
    fn get_type(s: &MarkdownState) -> Option<String> {
        let mut out = String::new();
        let mut push = |t: &str| {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(t);
        };
        if s.link_href {
            push("string url");
        } else {
            // Only apply inline styles to non-url text
            if s.strong.is_some() {
                push("strong");
            }
            if s.em.is_some() {
                push("em");
            }
            if s.link_text {
                push("link");
            }
            if s.code != 0 {
                push("comment");
            }
            if s.image {
                push("image");
            }
            if s.image_alt_text {
                push("image-alt-text link");
            }
            if s.image_marker {
                push("image-marker");
            }
        }
        if s.header != 0 {
            push("header");
            push(&format!("header-{}", s.header));
        }
        if s.quote != 0 {
            push("quote");
            push(&format!("quote-{}", s.quote));
        }
        if s.list != List::False {
            // JS `%` keeps the sign, like Rust's
            match (s.list_stack.len() as i64 - 1) % 3 {
                0 => push("variable-2"),
                1 => push("variable-3"),
                _ => push("keyword"),
            }
        }
        if s.trailing_space_new_line {
            push("trailing-space-new-line");
        } else if s.trailing_space != 0 {
            push(if s.trailing_space % 2 != 0 {
                "trailing-space-a"
            } else {
                "trailing-space-b"
            });
        }
        (!out.is_empty()).then_some(out)
    }

    /// `inlineNormal`
    fn inline_normal(&self, stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        // `state.text` = `handleText` (textRE)
        if stream.eat_while_if(|c| {
            !matches!(
                c,
                '#' | '!'
                    | '['
                    | ']'
                    | '*'
                    | '_'
                    | '\\'
                    | '<'
                    | '>'
                    | '`'
                    | ' '
                    | '"'
                    | '\''
                    | '('
                    | '~'
                    | ':'
            )
        }) {
            return Self::get_type(s);
        }

        if s.list == List::True {
            // List marker (*, +, -, 1., etc)
            s.list = List::Null;
            return Self::get_type(s);
        }

        if s.header != 0 && stream.matches(re!(r"^#+$")) {
            return Self::get_type(s);
        }

        let ch = stream.next()?;

        // Matches link titles present on next line
        if s.link_title {
            s.link_title = false;
            let match_ch = if ch == '(' { ')' } else { ch };
            let mut escaped = String::new();
            if ".?*+^[]\\(){}|-".contains(match_ch) {
                escaped.push('\\');
            }
            escaped.push(match_ch);
            let pattern = format!(r"^\s*(?:[^{escaped}\\]+|\\\\|\\.){escaped}");
            // built per link title like the JS; rare (a footnote URL ending
            // its line, then an inline special char)
            if let Ok(re) = Regex::new(&crate::cm::js_pattern(&pattern))
                && stream.matches(&re)
            {
                return Some("string".into());
            }
        }

        // If this block is changed, it may need to be updated in GFM mode
        if ch == '`' {
            stream.eat_while('`');
            let count = js_len(stream.current()) as i64;
            if s.code == 0 && (s.quote == 0 || count == 1) {
                s.code = count;
                return Self::get_type(s);
            } else if count == s.code {
                // Must be exact
                let t = Self::get_type(s);
                s.code = 0;
                return t;
            }
            return Self::get_type(s);
        } else if s.code != 0 {
            return Self::get_type(s);
        }

        if ch == '\\' {
            stream.next();
        }

        if ch == '!'
            && stream
                .match_re(re!(r"^\[[^\]]*\] ?(?:\(|\[)"), false)
                .is_some()
        {
            s.image_marker = true;
            s.image = true;
            return Self::get_type(s);
        }

        if ch == '['
            && s.image_marker
            && stream
                .match_re(re!(r"^[^\]]*\](\(.*?\)| ?\[.*?\])"), false)
                .is_some()
        {
            s.image_marker = false;
            s.image_alt_text = true;
            return Self::get_type(s);
        }

        if ch == ']' && s.image_alt_text {
            let t = Self::get_type(s);
            s.image_alt_text = false;
            s.image = false;
            s.inline = F::LinkHref;
            s.f = F::LinkHref;
            return t;
        }

        if ch == '[' && !s.image {
            if s.link_text && stream.matches(re!(r"^.*?\]")) {
                return Self::get_type(s);
            }
            s.link_text = true;
            return Self::get_type(s);
        }

        if ch == ']' && s.link_text {
            let t = Self::get_type(s);
            s.link_text = false;
            let f = if stream
                .match_re(re!(r"^(?:\(.*?\)| ?\[.*?\])"), false)
                .is_some()
            {
                F::LinkHref
            } else {
                F::InlineNormal
            };
            s.inline = f;
            s.f = f;
            return t;
        }

        if ch == '<'
            && stream
                .match_re(re!(r"^(https?|ftps?)://(?:[^\\>]|\\.)+>"), false)
                .is_some()
        {
            s.f = F::LinkInline;
            s.inline = F::LinkInline;
            return Some(match Self::get_type(s) {
                Some(t) => t + " link",
                None => "link".into(),
            });
        }

        if ch == '<'
            && stream
                .match_re(re!(r"^[^> \\]+@(?:[^\\>]|\\.)+>"), false)
                .is_some()
        {
            s.f = F::LinkInline;
            s.inline = F::LinkInline;
            return Some(match Self::get_type(s) {
                Some(t) => t + " link",
                None => "link".into(),
            });
        }

        if ch == '<'
            && stream
                .match_re(
                    re!(
                        r"^(!--|\?|!\[[Cc][Dd][Aa][Tt][Aa]\[|[A-Za-z][A-Za-z0-9-]*(?:\s+[A-Za-z_:.\-]+(?:\s*=\s*[^>]+)?)*\s*(?:>|$))"
                    ),
                    false,
                )
                .is_some()
        {
            if let Some(end) = (stream.pos..stream.len()).find(|&i| stream.char_at(i) == Some('>'))
            {
                let atts = stream.slice(stream.start, end);
                if re!(r#"markdown\s*=\s*('|"){0,1}1('|"){0,1}"#)
                    .is_match(atts)
                    .unwrap_or(false)
                {
                    s.md_inside = true;
                }
            }
            stream.back_up(1);
            s.html_state = Some(self.html.start_xml());
            return self.switch_block(stream, s, F::HtmlBlock);
        }

        if ch == '<' && stream.matches(re!(r"^/[A-Za-z0-9_]*?>")) {
            s.md_inside = false;
            return Some("tag".into());
        } else if ch == '*' || ch == '_' {
            let mut len = 1;
            let before = if stream.pos == 1 {
                ' '
            } else {
                stream.char_at(stream.pos - 2).unwrap_or(' ')
            };
            while len < 3 && stream.eat(ch).is_some() {
                len += 1;
            }
            let after = stream.peek().unwrap_or(' ');
            // See http://spec.commonmark.org/0.27/#emphasis-and-strong-emphasis
            let left_flanking = !js_space(after)
                && (!punctuation(after) || js_space(before) || punctuation(before));
            let right_flanking = !js_space(before)
                && (!punctuation(before) || js_space(after) || punctuation(after));
            let mut set_em = None;
            let mut set_strong = None;
            if len % 2 == 1 {
                // Em
                if s.em.is_none()
                    && left_flanking
                    && (ch == '*' || !right_flanking || punctuation(before))
                {
                    set_em = Some(true);
                } else if s.em == Some(ch)
                    && right_flanking
                    && (ch == '*' || !left_flanking || punctuation(after))
                {
                    set_em = Some(false);
                }
            }
            if len > 1 {
                // Strong
                if s.strong.is_none()
                    && left_flanking
                    && (ch == '*' || !right_flanking || punctuation(before))
                {
                    set_strong = Some(true);
                } else if s.strong == Some(ch)
                    && right_flanking
                    && (ch == '*' || !left_flanking || punctuation(after))
                {
                    set_strong = Some(false);
                }
            }
            if set_strong.is_some() || set_em.is_some() {
                if set_em == Some(true) {
                    s.em = Some(ch);
                }
                if set_strong == Some(true) {
                    s.strong = Some(ch);
                }
                let t = Self::get_type(s);
                if set_em == Some(false) {
                    s.em = None;
                }
                if set_strong == Some(false) {
                    s.strong = None;
                }
                return t;
            }
        } else if ch == ' ' && (stream.eat('*').is_some() || stream.eat('_').is_some()) {
            // Probably surrounded by spaces
            if stream.peek() == Some(' ') {
                // Surrounded by spaces, ignore
                return Self::get_type(s);
            }
            // Not surrounded by spaces, back up pointer
            stream.back_up(1);
        }

        if ch == ' ' {
            if stream.match_re(re!(r"^ +$"), false).is_some() {
                s.trailing_space += 1;
            } else if s.trailing_space != 0 {
                s.trailing_space_new_line = true;
            }
        }

        Self::get_type(s)
    }

    /// `linkInline`
    fn link_inline(stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        let ch = stream.next();
        if ch == Some('>') {
            s.f = F::InlineNormal;
            s.inline = F::InlineNormal;
            return Some(match Self::get_type(s) {
                Some(t) => t + " link",
                None => "link".into(),
            });
        }
        stream.eat_while_if(|c| c != '>');
        Some("link".into())
    }

    /// `linkHref`
    fn link_href(stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        // Check if space, and return NULL if so (to avoid marking the space)
        if eat_space(stream) {
            return None;
        }
        let ch = stream.next();
        if ch == Some('(') || ch == Some('[') {
            let f = F::LinkHrefInside(if ch == Some('(') { ')' } else { ']' });
            s.f = f;
            s.inline = f;
            s.link_href = true;
            return Self::get_type(s);
        }
        Some("error".into())
    }

    /// `getLinkHrefInside(endChar)`
    fn link_href_inside(
        end: char,
        stream: &mut StringStream,
        s: &mut MarkdownState,
    ) -> Option<String> {
        let ch = stream.next();
        if ch == Some(end) {
            s.f = F::InlineNormal;
            s.inline = F::InlineNormal;
            let t = Self::get_type(s);
            s.link_href = false;
            return t;
        }
        // linkRE[endChar]
        if end == ')' {
            stream.matches(re!(r"^(?:[^\\\(\)]|\\.|\((?:[^\\\(\)]|\\.)*\))*?(?=\))"));
        } else {
            stream.matches(re!(r"^(?:[^\\\[\]]|\\.|\[(?:[^\\\[\]]|\\.)*\])*?(?=\])"));
        }
        s.link_href = true;
        Self::get_type(s)
    }

    /// `footnoteLink`
    fn footnote_link(&self, stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        if stream.match_re(re!(r"^([^\]\\]|\\.)*\]:"), false).is_some() {
            s.f = F::FootnoteLinkInside;
            stream.next(); // Consume [
            s.link_text = true;
            return Self::get_type(s);
        }
        self.switch_inline(stream, s, F::InlineNormal)
    }

    /// `footnoteLinkInside`
    fn footnote_link_inside(stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        if stream.match_str("]:", true, false) {
            s.f = F::FootnoteUrl;
            s.inline = F::FootnoteUrl;
            let t = Self::get_type(s);
            s.link_text = false;
            return t;
        }
        stream.matches(re!(r"^([^\]\\]|\\.)+"));
        Some("link".into())
    }

    /// `footnoteUrl`
    fn footnote_url(stream: &mut StringStream, s: &mut MarkdownState) -> Option<String> {
        // Check if space, and return NULL if so (to avoid marking the space)
        if eat_space(stream) {
            return None;
        }
        // Match URL
        stream.eat_while_if(|c| !js_space(c));
        // Check for link title
        if stream.eol() {
            // End of line, set flag to check next line
            s.link_title = true;
        } else {
            // More content on line, check if link title
            stream.matches(re!(
                r#"^(?:\s+(?:"(?:[^"\\]|\\.)+"|'(?:[^'\\]|\\.)+'|\((?:[^)\\]|\\.)+\)))?"#
            ));
        }
        s.f = F::InlineNormal;
        s.inline = F::InlineNormal;
        Some("string url".into())
    }
}

impl Mode for Markdown {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(self.start())
    }

    /// `token`
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<MarkdownState>(st);
        if s.this_line.stream != Some(stream.line()) {
            s.header = 0;

            if stream.pos == stream.len()
                || (stream.pos..stream.len()).all(|i| stream.char_at(i).is_some_and(js_space))
            {
                stream.skip_to_end();
                Self::blank(s);
                return None;
            }

            s.prev_line = s.this_line;
            s.this_line = LineInfo {
                stream: Some(stream.line()),
                ..LineInfo::default()
            };

            // Reset state.trailingSpace
            s.trailing_space = 0;
            s.trailing_space_new_line = false;

            if s.local_state.is_none() {
                s.f = s.block;
                if s.f != F::HtmlBlock {
                    let from = stream.pos;
                    eat_space(stream);
                    let indentation: i64 = (from..stream.pos)
                        .map(|i| {
                            if stream.char_at(i) == Some('\t') {
                                4
                            } else {
                                1
                            }
                        })
                        .sum();
                    s.indentation = indentation;
                    s.indentation_diff = None;
                    if indentation > 0 {
                        return None;
                    }
                }
            }
        }
        let f = s.f;
        self.call(f, stream, s)
    }

    fn blank_line(&self, st: &mut dyn ModeState) {
        Self::blank(state::<MarkdownState>(st));
    }

    /// `innerMode`
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        let s = state_ref::<MarkdownState>(st);
        if s.block == F::HtmlBlock {
            return "xml";
        }
        if let (Some(mode), Some(local)) = (&s.local_mode, &s.local_state) {
            return mode.inner_mode_name(&**local);
        }
        "markdown"
    }
}
