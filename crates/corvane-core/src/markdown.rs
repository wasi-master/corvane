//! Markdown → block model for `corvane_ui::markdown`, standing in for GHD
//! `ui/lib/sandboxed-markdown.tsx`, which renders `marked(markdown, { gfm:
//! true, breaks: true })` (sanitised by DOMPurify) inside a sandboxed iframe
//! styled by `static/common/markdown.css`.
//!
//! The walk over `pulldown-cmark` events keeps headings, paragraphs,
//! bold / italic / strikethrough, inline code, fenced and indented code,
//! links, bullet and numbered lists, block quotes and rules. Everything else
//! degrades to plain text: images become their alt text, tables become one
//! line per row with cells separated by " | ", raw HTML loses its tags
//! (`<br>` becomes a line break), task-list markers become ☐ / ☑.
//!
//! As with `breaks: true`, a single newline inside a paragraph is a line
//! break. Bare `http(s)://` URLs are linkified as GFM does. GHD's markdown
//! filters (emoji, `@mention`, `#123` issue and commit-SHA links, videos) are
//! not ported.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::text_tokens::{Token, TokenRepository, tokenize};

/// Inline styles active over a span of text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub code: bool,
}

/// A styled run of a [`RichText`]. Spans are sorted, non-overlapping, and
/// only exist for text that is styled or linked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: InlineStyle,
    pub link: Option<String>,
}

/// One paragraph's text with its styled spans.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichText {
    pub text: String,
    pub spans: Vec<Span>,
}

impl RichText {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The linked spans in order, for click handling.
    pub fn links(&self) -> impl Iterator<Item = (&Range<usize>, &str)> {
        self.spans
            .iter()
            .filter_map(|s| s.link.as_deref().map(|l| (&s.range, l)))
    }

    fn push(&mut self, text: &str, style: InlineStyle, link: Option<&str>) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        if style == InlineStyle::default() && link.is_none() {
            return;
        }
        if let Some(last) = self.spans.last_mut()
            && last.range.end == start
            && last.style == style
            && last.link.as_deref() == link
        {
            last.range.end = end;
            return;
        }
        self.spans.push(Span {
            range: start..end,
            style,
            link: link.map(str::to_string),
        });
    }

    /// Trailing line breaks left by a soft break before a block end.
    fn trim_end(&mut self) {
        let trimmed = self.text.trim_end_matches(['\n', ' ']).len();
        self.text.truncate(trimmed);
        self.spans.retain(|s| s.range.start < trimmed);
        if let Some(last) = self.spans.last_mut() {
            last.range.end = last.range.end.min(trimmed);
        }
    }
}

/// A block of a Markdown document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// `level` is 1…6.
    Heading {
        level: u8,
        text: RichText,
    },
    Paragraph(RichText),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    /// `start` is the first number of an ordered list, `None` for bullets.
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    BlockQuote(Vec<Block>),
    Rule,
}

/// Parse Markdown into blocks.
pub fn parse(markdown: &str) -> Vec<Block> {
    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS;
    let mut walk = Walk::default();
    for event in Parser::new_ext(markdown, options) {
        walk.event(event);
    }
    walk.finish()
}

/// Plain text of a document (search, accessibility labels, previews).
pub fn plain_text(blocks: &[Block]) -> String {
    fn walk(blocks: &[Block], out: &mut Vec<String>) {
        for block in blocks {
            match block {
                Block::Heading { text, .. } | Block::Paragraph(text) => out.push(text.text.clone()),
                Block::CodeBlock { code, .. } => out.push(code.clone()),
                Block::List { items, .. } => items.iter().for_each(|i| walk(i, out)),
                Block::BlockQuote(inner) => walk(inner, out),
                Block::Rule => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(blocks, &mut out);
    out.join("\n")
}

/// The URL a clicked link opens, as GHD's link interceptor does: only
/// `http(s)` links, relative ones resolved against `base_href` (the page the
/// Markdown came from, e.g. a pull request's `html_url`).
pub fn resolve_link(href: &str, base_href: Option<&str>) -> Option<String> {
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(href.to_string());
    }
    if lower.contains(':') && !lower.starts_with('/') && !lower.starts_with('#') {
        // mailto:, javascript:, file: …
        return None;
    }
    let base = base_href?;
    let scheme_end = base.find("://")? + 3;
    let origin_end = base[scheme_end..]
        .find('/')
        .map_or(base.len(), |ix| scheme_end + ix);
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("{}{rest}", &base[..scheme_end]));
    }
    if href.starts_with('/') {
        return Some(format!("{}{href}", &base[..origin_end]));
    }
    let base_no_fragment = base.split('#').next().unwrap_or(base);
    if href.starts_with('#') {
        return Some(format!("{base_no_fragment}{href}"));
    }
    let dir_end = base_no_fragment
        .rfind('/')
        .filter(|ix| *ix >= origin_end)
        .map_or(base_no_fragment.len(), |ix| ix);
    Some(format!("{}/{href}", &base_no_fragment[..dir_end]))
}

enum Container {
    Root,
    Quote,
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Item,
}

struct Frame {
    container: Container,
    blocks: Vec<Block>,
}

/// Where the inline text being collected ends up.
enum InlineTarget {
    Paragraph,
    Heading(u8),
}

#[derive(Default)]
struct Walk {
    frames: Vec<Frame>,
    inline: Option<(InlineTarget, RichText)>,
    code: Option<(Option<String>, String)>,
    bold: u32,
    italic: u32,
    strikethrough: u32,
    links: Vec<String>,
    /// Inside a table row: cells seen so far.
    table_cells: Option<u32>,
}

/// A commit message as GHD's `RichText` shows it: the
/// [`crate::text_tokens`] emoji, `#123` issues, `@mentions` and links.
///
/// With `extras` (flag `804`), as GitHub.com shows it: `code` spans in
/// backticks (on one line; the backticks are dropped), bare `http(s)` URLs
/// also after punctuation and without trailing punctuation, and with
/// `commit_base` (the repository's `html_url`) 7–40 character hex words that
/// mix letters and digits linked to `<commit_base>/commit/<sha>`.
pub fn commit_message_rich_text(
    text: &str,
    repository: Option<&TokenRepository>,
    extras: bool,
    commit_base: Option<&str>,
) -> RichText {
    let mut out = RichText::default();
    if !extras {
        push_tokens(&mut out, text, repository, None);
        return out;
    }
    let code = InlineStyle {
        code: true,
        ..InlineStyle::default()
    };
    let mut rest = text;
    while !rest.is_empty() {
        let span = rest.find('`').and_then(|open| {
            let close = rest[open + 1..].find(['`', '\n'])?;
            (rest.as_bytes()[open + 1 + close] == b'`' && close > 0)
                .then_some((open, open + 1 + close))
        });
        let Some((open, close)) = span else {
            push_tokens(&mut out, rest, repository, Some(commit_base));
            break;
        };
        push_tokens(&mut out, &rest[..open], repository, Some(commit_base));
        out.push(&rest[open + 1..close], code, None);
        rest = &rest[close + 1..];
    }
    out
}

/// GHD's tokens for `text`; `extras` (with the SHA base) as in
/// [`commit_message_rich_text`].
fn push_tokens(
    out: &mut RichText,
    text: &str,
    repository: Option<&TokenRepository>,
    extras: Option<Option<&str>>,
) {
    let plain = InlineStyle::default();
    for token in tokenize(text, repository) {
        match (&token, extras) {
            (Token::Text(text), Some(commit_base)) => push_autolinked(out, text, commit_base),
            (Token::Link { text, url }, Some(_)) if text == url => {
                let linked = url.trim_end_matches(URL_TRAILING_PUNCTUATION);
                out.push(linked, plain, Some(linked));
                out.push(&url[linked.len()..], plain, None);
            }
            (Token::Link { text, url }, _) => out.push(text, plain, Some(url)),
            _ => out.push(token.display(), plain, None),
        }
    }
}

const URL_TRAILING_PUNCTUATION: [char; 8] = ['.', ',', ':', ';', '!', '?', ')', '\''];

/// Plain text with its bare URLs and (with `commit_base`) SHAs linked.
fn push_autolinked(out: &mut RichText, text: &str, commit_base: Option<&str>) {
    let mut links: Vec<(Range<usize>, String)> = bare_urls(text)
        .into_iter()
        .map(|r| (r.clone(), text[r].to_string()))
        .collect();
    if let Some(base) = commit_base {
        let mut start = None;
        for (ix, c) in text
            .char_indices()
            .chain(std::iter::once((text.len(), ' ')))
        {
            if c.is_ascii_alphanumeric() {
                start.get_or_insert(ix);
                continue;
            }
            let Some(from) = start.take() else {
                continue;
            };
            let word = &text[from..ix];
            let is_sha = (7..=40).contains(&word.len())
                && word.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
                && word.bytes().any(|b| b.is_ascii_digit())
                && word.bytes().any(|b| b.is_ascii_alphabetic());
            if is_sha && !links.iter().any(|(r, _)| r.contains(&from)) {
                links.push((from..ix, format!("{base}/commit/{word}")));
            }
        }
        links.sort_by_key(|(r, _)| r.start);
    }
    let mut at = 0;
    for (range, url) in links {
        out.push(&text[at..range.start], InlineStyle::default(), None);
        out.push(&text[range.clone()], InlineStyle::default(), Some(&url));
        at = range.end;
    }
    out.push(&text[at..], InlineStyle::default(), None);
}

/// Bare `http://` / `https://` URLs in `text` as byte ranges, without the
/// trailing punctuation GFM's extended autolinks leave out.
fn bare_urls(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(ix) = text[from..].find("http") {
        let start = from + ix;
        let rest = &text[start..];
        let scheme = if rest.starts_with("https://") {
            8
        } else if rest.starts_with("http://") {
            7
        } else {
            from = start + 4;
            continue;
        };
        let len = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"'))
            .unwrap_or(rest.len());
        let url = rest[..len].trim_end_matches(URL_TRAILING_PUNCTUATION);
        let preceded_by_word = text[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric());
        if url.len() > scheme && !preceded_by_word {
            out.push(start..start + url.len());
        }
        from = start + len.max(4);
    }
    out
}

impl Walk {
    fn frame(&mut self) -> &mut Frame {
        if self.frames.is_empty() {
            self.frames.push(Frame {
                container: Container::Root,
                blocks: Vec::new(),
            });
        }
        let last = self.frames.len() - 1;
        &mut self.frames[last]
    }

    fn style(&self) -> InlineStyle {
        InlineStyle {
            bold: self.bold > 0,
            italic: self.italic > 0,
            strikethrough: self.strikethrough > 0,
            code: false,
        }
    }

    /// The paragraph that inline content goes into (an implicit one in a
    /// tight list item).
    fn inline(&mut self) -> &mut RichText {
        &mut self
            .inline
            .get_or_insert_with(|| (InlineTarget::Paragraph, RichText::default()))
            .1
    }

    fn flush_inline(&mut self) {
        if let Some((target, mut text)) = self.inline.take() {
            text.trim_end();
            let block = match target {
                InlineTarget::Heading(level) => Block::Heading { level, text },
                InlineTarget::Paragraph if text.is_empty() => return,
                InlineTarget::Paragraph => Block::Paragraph(text),
            };
            self.frame().blocks.push(block);
        }
    }

    fn push_block(&mut self, block: Block) {
        self.flush_inline();
        self.frame().blocks.push(block);
    }

    fn push_frame(&mut self, container: Container) {
        self.flush_inline();
        self.frame();
        self.frames.push(Frame {
            container,
            blocks: Vec::new(),
        });
    }

    fn pop_frame(&mut self) {
        self.flush_inline();
        if self.frames.len() < 2 {
            return;
        }
        let Some(frame) = self.frames.pop() else {
            return;
        };
        match frame.container {
            Container::Root => {}
            Container::Quote => self.frame().blocks.push(Block::BlockQuote(frame.blocks)),
            Container::List { start, items } => {
                self.frame().blocks.push(Block::List { start, items })
            }
            Container::Item => {
                if let Container::List { items, .. } = &mut self.frame().container {
                    items.push(frame.blocks);
                } else {
                    self.frame().blocks.extend(frame.blocks);
                }
            }
        }
    }

    /// Text, with bare URLs outside links turned into links.
    fn text(&mut self, text: &str) {
        if let Some((_, code)) = &mut self.code {
            code.push_str(text);
            return;
        }
        let style = self.style();
        if let Some(link) = self.links.last().cloned() {
            self.inline().push(text, style, Some(&link));
            return;
        }
        let mut last = 0;
        for range in bare_urls(text) {
            let url = &text[range.clone()];
            self.inline().push(&text[last..range.start], style, None);
            self.inline().push(url, style, Some(url));
            last = range.end;
        }
        self.inline().push(&text[last..], style, None);
    }

    fn html(&mut self, html: &str, block: bool) {
        let mut out = String::new();
        let mut rest = html;
        while let Some(open) = rest.find('<') {
            out.push_str(&rest[..open]);
            let Some(close) = rest[open..].find('>') else {
                out.push_str(&rest[open..]);
                rest = "";
                break;
            };
            let tag = rest[open + 1..open + close].trim().to_ascii_lowercase();
            if tag.starts_with("br") {
                out.push('\n');
            }
            rest = &rest[open + close + 1..];
        }
        out.push_str(rest);
        let out = if block { out.trim_matches('\n') } else { &out };
        if !out.is_empty() {
            self.text(out);
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => {
                let mut style = self.style();
                style.code = true;
                let link = self.links.last().cloned();
                self.inline().push(&code, style, link.as_deref());
            }
            Event::Html(html) => self.html(&html, true),
            Event::InlineHtml(html) => self.html(&html, false),
            Event::InlineMath(m) | Event::DisplayMath(m) => self.text(&m),
            Event::FootnoteReference(r) => self.text(&format!("[{r}]")),
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, code)) = &mut self.code {
                    code.push('\n');
                } else {
                    self.inline().push("\n", InlineStyle::default(), None);
                }
            }
            Event::Rule => self.push_block(Block::Rule),
            Event::TaskListMarker(checked) => {
                self.inline().push(
                    if checked { "☑ " } else { "☐ " },
                    InlineStyle::default(),
                    None,
                );
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.flush_inline();
                self.inline = Some((InlineTarget::Paragraph, RichText::default()));
            }
            Tag::Heading { level, .. } => {
                self.flush_inline();
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                self.inline = Some((InlineTarget::Heading(level), RichText::default()));
            }
            Tag::CodeBlock(kind) => {
                self.flush_inline();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .filter(|l| !l.is_empty())
                        .map(str::to_string),
                    CodeBlockKind::Indented => None,
                };
                self.code = Some((language, String::new()));
            }
            Tag::BlockQuote(_) => self.push_frame(Container::Quote),
            Tag::List(start) => self.push_frame(Container::List {
                start,
                items: Vec::new(),
            }),
            Tag::Item => self.push_frame(Container::Item),
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strikethrough += 1,
            Tag::Link { dest_url, .. } => self.links.push(dest_url.to_string()),
            Tag::TableRow | Tag::TableHead => {
                self.flush_inline();
                self.table_cells = Some(0);
                self.inline = Some((InlineTarget::Paragraph, RichText::default()));
            }
            Tag::TableCell => {
                if let Some(cells) = self.table_cells {
                    if cells > 0 {
                        self.inline().push(" | ", InlineStyle::default(), None);
                    }
                    self.table_cells = Some(cells + 1);
                }
            }
            // images: the alt text arrives as text events; everything else
            // (HTML blocks, tables, footnotes, definition lists, metadata)
            // falls through to its text
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::HtmlBlock => self.flush_inline(),
            TagEnd::CodeBlock => {
                if let Some((language, mut code)) = self.code.take() {
                    if code.ends_with('\n') {
                        code.pop();
                    }
                    self.push_block(Block::CodeBlock { language, code });
                }
            }
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item => self.pop_frame(),
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strikethrough = self.strikethrough.saturating_sub(1),
            TagEnd::Link => {
                self.links.pop();
            }
            TagEnd::TableRow | TagEnd::TableHead => {
                self.table_cells = None;
                self.flush_inline();
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush_inline();
        while self.frames.len() > 1 {
            self.pop_frame();
        }
        self.frames.pop().map(|f| f.blocks).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_message_code_urls_and_shas() {
        let base = "https://github.com/o/r";
        let t = commit_message_rich_text(
            "Fix `foo()` per a5c3785 and https://x.io/a. Not `open\nor` 1234567 or defaced",
            None,
            true,
            Some(base),
        );
        assert_eq!(
            t.text,
            "Fix foo() per a5c3785 and https://x.io/a. Not `open\nor` 1234567 or defaced"
        );
        let spans: Vec<(&str, bool, Option<&str>)> = t
            .spans
            .iter()
            .map(|s| (&t.text[s.range.clone()], s.style.code, s.link.as_deref()))
            .collect();
        assert_eq!(
            spans,
            [
                ("foo()", true, None),
                (
                    "a5c3785",
                    false,
                    Some("https://github.com/o/r/commit/a5c3785")
                ),
                ("https://x.io/a", false, Some("https://x.io/a")),
            ]
        );
        // no base: SHAs stay plain
        let t = commit_message_rich_text("see a5c3785", None, true, None);
        assert!(t.spans.is_empty());
    }

    #[test]
    fn commit_message_ghd_tokens() {
        let repo = TokenRepository {
            html_url: "https://github.com/o/r".into(),
            web_base: "https://github.com".into(),
        };
        let spans = |t: &RichText| -> Vec<(String, Option<String>)> {
            t.spans
                .iter()
                .map(|s| (t.text[s.range.clone()].to_string(), s.link.clone()))
                .collect()
        };
        let msg = ":tada: `x` a5c3785 @me (#452) https://x.io/a.";
        // GHD: emoji, issue, mention, whole-word URL; backticks and SHAs plain
        let t = commit_message_rich_text(msg, Some(&repo), false, None);
        assert_eq!(t.text, "🎉 `x` a5c3785 @me (#452) https://x.io/a.");
        assert_eq!(
            spans(&t),
            [
                ("@me".into(), Some("https://github.com/me".into())),
                (
                    "#452".into(),
                    Some("https://github.com/o/r/issues/452".into())
                ),
                ("https://x.io/a.".into(), Some("https://x.io/a.".into())),
            ]
        );
        // `804` on top: code span, SHA, URL without the full stop
        let t = commit_message_rich_text(msg, Some(&repo), true, Some("https://github.com/o/r"));
        assert_eq!(t.text, "🎉 x a5c3785 @me (#452) https://x.io/a.");
        assert_eq!(spans(&t)[0], ("x".into(), None));
        assert_eq!(
            spans(&t)[1..],
            [
                (
                    "a5c3785".into(),
                    Some("https://github.com/o/r/commit/a5c3785".into())
                ),
                ("@me".into(), Some("https://github.com/me".into())),
                (
                    "#452".into(),
                    Some("https://github.com/o/r/issues/452".into())
                ),
                ("https://x.io/a".into(), Some("https://x.io/a".into())),
            ]
        );
    }

    fn para(blocks: &[Block], ix: usize) -> &RichText {
        match &blocks[ix] {
            Block::Paragraph(t) => t,
            other => panic!("expected a paragraph, got {other:?}"),
        }
    }

    #[test]
    fn headings_and_paragraphs() {
        let blocks = parse("# Title\n\nSome text\nnext line\n\n### Small");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Heading { level: 1, text } if text.text == "Title"));
        // `breaks: true`: a single newline is a line break
        assert_eq!(para(&blocks, 1).text, "Some text\nnext line");
        assert!(matches!(&blocks[2], Block::Heading { level: 3, .. }));
    }

    #[test]
    fn inline_styles_are_sorted_non_overlapping_spans() {
        let blocks = parse("a **bold _both_** `code` ~~gone~~ z");
        let t = para(&blocks, 0);
        assert_eq!(t.text, "a bold both code gone z");
        let styles: Vec<(&str, InlineStyle)> = t
            .spans
            .iter()
            .map(|s| (&t.text[s.range.clone()], s.style))
            .collect();
        assert_eq!(styles[0].0, "bold ");
        assert!(styles[0].1.bold && !styles[0].1.italic);
        assert_eq!(styles[1].0, "both");
        assert!(styles[1].1.bold && styles[1].1.italic);
        assert_eq!(styles[2].0, "code");
        assert!(styles[2].1.code);
        assert_eq!(styles[3].0, "gone");
        assert!(styles[3].1.strikethrough);
        for w in t.spans.windows(2) {
            assert!(w[0].range.end <= w[1].range.start);
        }
    }

    #[test]
    fn links_and_bare_urls() {
        let blocks = parse("See [the docs](https://example.com/docs) or https://github.com/x.");
        let t = para(&blocks, 0);
        let links: Vec<(&str, &str)> = t.links().map(|(r, l)| (&t.text[r.clone()], l)).collect();
        assert_eq!(
            links,
            vec![
                ("the docs", "https://example.com/docs"),
                ("https://github.com/x", "https://github.com/x"),
            ]
        );
        assert!(t.text.ends_with("https://github.com/x."));
    }

    #[test]
    fn fenced_code_keeps_language_and_text() {
        let blocks = parse("```rust\nfn main() {}\n  x\n```\n");
        assert_eq!(
            blocks,
            vec![Block::CodeBlock {
                language: Some("rust".into()),
                code: "fn main() {}\n  x".into()
            }]
        );
    }

    #[test]
    fn tight_and_loose_lists_nest() {
        let blocks = parse("- one\n- two\n  1. inner\n  2. more\n\n3. three\n");
        let Block::List { start, items } = &blocks[0] else {
            panic!("expected a list: {blocks:?}")
        };
        assert_eq!(*start, None);
        assert_eq!(items.len(), 2);
        assert_eq!(para(&items[0], 0).text, "one");
        let Block::List {
            start,
            items: inner,
        } = &items[1][1]
        else {
            panic!("expected a nested list: {:?}", items[1])
        };
        assert_eq!(*start, Some(1));
        assert_eq!(inner.len(), 2);
        assert!(matches!(&blocks[1], Block::List { start: Some(3), .. }));
    }

    #[test]
    fn block_quotes_hold_blocks() {
        let blocks = parse("> quoted **text**\n>\n> - item\n\nafter");
        let Block::BlockQuote(inner) = &blocks[0] else {
            panic!("expected a quote: {blocks:?}")
        };
        assert_eq!(para(inner, 0).text, "quoted text");
        assert!(matches!(inner[1], Block::List { .. }));
        assert_eq!(para(&blocks, 1).text, "after");
    }

    #[test]
    fn unsupported_constructs_degrade_to_text() {
        let blocks = parse("![a screenshot](https://x/y.png)\n\nline<br>next <b>bold</b>");
        assert_eq!(para(&blocks, 0).text, "a screenshot");
        assert_eq!(para(&blocks, 1).text, "line\nnext bold");

        let blocks = parse("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert_eq!(para(&blocks, 0).text, "a | b");
        assert_eq!(para(&blocks, 1).text, "1 | 2");

        let blocks = parse("- [x] done\n- [ ] open");
        let Block::List { items, .. } = &blocks[0] else {
            panic!()
        };
        assert_eq!(para(&items[0], 0).text, "☑ done");
        assert_eq!(para(&items[1], 0).text, "☐ open");
    }

    #[test]
    fn links_resolve_like_ghds_interceptor() {
        let base = Some("https://github.com/desktop/desktop/pull/1");
        assert_eq!(
            resolve_link("https://x.dev/a", base).as_deref(),
            Some("https://x.dev/a")
        );
        assert_eq!(
            resolve_link("/desktop/desktop/issues/2", base).as_deref(),
            Some("https://github.com/desktop/desktop/issues/2")
        );
        assert_eq!(
            resolve_link("files", base).as_deref(),
            Some("https://github.com/desktop/desktop/pull/files")
        );
        assert_eq!(
            resolve_link("#top", base).as_deref(),
            Some("https://github.com/desktop/desktop/pull/1#top")
        );
        assert_eq!(resolve_link("mailto:a@b.c", base), None);
        assert_eq!(resolve_link("/relative", None), None);
    }

    #[test]
    fn plain_text_joins_blocks() {
        let blocks = parse("# T\n\n- a\n- b\n\n```\ncode\n```");
        assert_eq!(plain_text(&blocks), "T\na\nb\ncode");
    }
}
