//! A port of the CodeMirror 5 pieces GitHub Desktop's highlighter worker
//! runs (`app/src/highlighter/index.ts` over
//! `codemirror/addon/runmode/runmode.node.js`), so diff colours come out of
//! the same tokenizers GHD uses:
//!
//! - [`StringStream`]: CodeMirror's stream API, indexed in UTF-16 code
//!   units like JS strings (a char outside the BMP takes two positions).
//! - [`Mode`]: a tokenizer with a boxed, clonable state, so modes nest
//!   (htmlmixed runs xml, css and javascript; php runs htmlmixed and clike).
//! - [`run`]: GHD's worker loop (blank lines → `blankLine`, up to ten tries
//!   per token, `m-<inner mode>` added to every token).
//! - [`resolve`]: the `.cm-s-default` cascade from `styles/ui/_diff.scss`
//!   that turns a token's class list into the one colour GHD shows.
//! - [`mode_for_path`]: GHD's extension / basename → MIME → mode tables.
//!
//! Every ported mode is checked against GHD's own JavaScript through the
//! oracle in `tools/cm-oracle` (see its README): same tokens, same classes.

pub mod modes;
pub mod simple;

use std::any::Any;
use std::sync::Arc;

use fancy_regex::Regex;

use crate::{Span, TokenClass};

/// A mode's per-document state. Any `Clone + 'static` type qualifies.
pub trait ModeState: Any {
    fn clone_box(&self) -> Box<dyn ModeState>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any + Clone> ModeState for T {
    fn clone_box(&self) -> Box<dyn ModeState> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Clone for Box<dyn ModeState> {
    fn clone(&self) -> Self {
        (**self).clone_box()
    }
}

/// Downcast a state the mode itself created. Panics on a foreign state,
/// which would be a bug in the calling mode.
pub fn state<T: 'static>(s: &mut dyn ModeState) -> &mut T {
    s.as_any_mut()
        .downcast_mut::<T>()
        .expect("mode state of another mode")
}

pub fn state_ref<T: 'static>(s: &dyn ModeState) -> &T {
    s.as_any()
        .downcast_ref::<T>()
        .expect("mode state of another mode")
}

/// A CodeMirror mode (`CodeMirror.defineMode` result).
pub trait Mode: Send + Sync {
    /// `mode.name` (the `m-<name>` class); `""` for an anonymous mode object
    /// (multiplex.js inner modes), whose tokens get no `m-` class.
    fn name(&self) -> &'static str;
    fn start_state(&self) -> Box<dyn ModeState>;
    /// `mode.token`: consume at least one char and return the style
    /// (space-separated CodeMirror token names), `None` for unstyled.
    fn token(&self, stream: &mut StringStream, state: &mut dyn ModeState) -> Option<String>;
    fn blank_line(&self, _state: &mut dyn ModeState) {}
    /// `CodeMirror.innerMode(mode, state).mode.name`: the name of the mode
    /// that will produce the next token (nesting modes override this).
    fn inner_mode_name(&self, _state: &dyn ModeState) -> &'static str {
        self.name()
    }
}

/// JS `String.prototype.match` on the rest of the line: the match text and
/// its capture groups (`None` for groups that did not take part).
#[derive(Clone, Debug, Default)]
pub struct Match {
    pub text: String,
    pub groups: Vec<Option<String>>,
}

impl Match {
    pub fn group(&self, i: usize) -> Option<&str> {
        if i == 0 {
            return Some(&self.text);
        }
        self.groups.get(i - 1).and_then(|g| g.as_deref())
    }
}

/// UTF-16 unit → byte offset (plus the end) and unit → char, the way JS
/// indexes a string: both halves of a surrogate pair map to the char's first
/// byte and to the char itself.
fn units(string: &str) -> (Vec<char>, Vec<usize>) {
    let mut chars = Vec::with_capacity(string.len());
    let mut bytes = Vec::with_capacity(string.len() + 1);
    for (b, c) in string.char_indices() {
        for _ in 0..c.len_utf16() {
            chars.push(c);
            bytes.push(b);
        }
    }
    bytes.push(string.len());
    (chars, bytes)
}

/// Length of a string in UTF-16 units (JS `.length`).
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// CodeMirror 5 `StringStream`. Positions are UTF-16 units, like JS.
pub struct StringStream<'a> {
    /// the char at every UTF-16 unit
    chars: Vec<char>,
    /// byte offset of every unit (plus the end), for regex matching
    bytes: Vec<usize>,
    string: &'a str,
    /// the visible length (`stream.string.length`), less than the line's
    /// inside [`StringStream::with_end`]
    end: usize,
    pub pos: usize,
    pub start: usize,
    tab_size: usize,
    last_column_pos: usize,
    last_column_value: usize,
    pub line_start: usize,
    lines: &'a [&'a str],
    line: usize,
}

fn count_column(
    chars: &[char],
    end: Option<usize>,
    tab_size: usize,
    start: usize,
    start_value: usize,
) -> usize {
    let end = end.unwrap_or_else(|| {
        chars
            .iter()
            .position(|c| !c.is_whitespace() && *c != '\u{a0}')
            .unwrap_or(chars.len())
    });
    let mut n = start_value;
    let mut i = start;
    loop {
        let next_tab = chars[i.min(chars.len())..]
            .iter()
            .position(|c| *c == '\t')
            .map(|p| p + i);
        match next_tab {
            Some(t) if t < end => {
                n += t - i;
                n += tab_size - (n % tab_size);
                i = t + 1;
            }
            _ => return n + end.saturating_sub(i),
        }
    }
}

impl<'a> StringStream<'a> {
    pub fn new(string: &'a str, tab_size: usize, lines: &'a [&'a str], line: usize) -> Self {
        let (chars, bytes) = units(string);
        Self {
            end: chars.len(),
            chars,
            bytes,
            string,
            pos: 0,
            start: 0,
            tab_size,
            last_column_pos: 0,
            last_column_value: 0,
            line_start: 0,
            lines,
            line,
        }
    }

    /// Length of the line in UTF-16 units.
    pub fn len(&self) -> usize {
        self.end
    }
    pub fn is_empty(&self) -> bool {
        self.end == 0
    }
    /// The whole line (`stream.string`).
    pub fn string(&self) -> &str {
        &self.string[..self.bytes[self.end]]
    }
    /// Run `f` with the line cut at `end` (multiplex.js's
    /// `stream.string = oldContent.slice(0, cutOff)` … `stream.string =
    /// oldContent`): nothing past `end` is visible to `f`.
    pub fn with_end<R>(&mut self, end: usize, f: impl FnOnce(&mut Self) -> R) -> R {
        let whole = self.end;
        self.end = end.min(whole);
        let r = f(self);
        self.end = whole;
        r
    }
    /// Char at an index (`stream.string.charAt(i)`), `None` past the end.
    pub fn char_at(&self, i: usize) -> Option<char> {
        self.chars[..self.end].get(i).copied()
    }
    /// The UTF-16 code unit at an index (`stream.string.charCodeAt(i)`), so
    /// a mode can tell the two halves of a surrogate pair apart like JS.
    pub fn char_code_at(&self, i: usize) -> Option<u16> {
        let c = *self.chars.get(i)?;
        let mut buf = [0u16; 2];
        let enc = c.encode_utf16(&mut buf);
        let second = enc.len() == 2 && i > 0 && self.bytes[i - 1] == self.bytes[i];
        Some(enc[usize::from(second)])
    }
    /// `stream.string.slice(from, to)` in chars.
    pub fn slice(&self, from: usize, to: usize) -> &str {
        let to = to.min(self.end);
        let from = from.min(to);
        &self.string[self.bytes[from]..self.bytes[to]]
    }

    pub fn eol(&self) -> bool {
        self.pos >= self.end
    }
    pub fn sol(&self) -> bool {
        self.pos == self.line_start
    }
    pub fn peek(&self) -> Option<char> {
        self.chars[..self.end].get(self.pos).copied()
    }
    // named after CodeMirror's `stream.next()` so ported modes read the same
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<char> {
        let c = self.chars[..self.end].get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }
    /// `eat(ch)`
    pub fn eat(&mut self, ch: char) -> Option<char> {
        self.eat_if(|c| c == ch)
    }
    /// `eat(fn)` / `eat(/re/)` with a char predicate.
    pub fn eat_if(&mut self, f: impl Fn(char) -> bool) -> Option<char> {
        let c = self.chars[..self.end].get(self.pos).copied()?;
        if f(c) {
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }
    /// `eat(/re/)`: the regex tested against the single next char.
    pub fn eat_re(&mut self, re: &Regex) -> Option<char> {
        let c = self.chars[..self.end].get(self.pos).copied()?;
        let mut buf = [0u8; 4];
        if re.is_match(c.encode_utf8(&mut buf)).unwrap_or(false) {
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }
    pub fn eat_while(&mut self, ch: char) -> bool {
        self.eat_while_if(|c| c == ch)
    }
    pub fn eat_while_if(&mut self, f: impl Fn(char) -> bool) -> bool {
        let start = self.pos;
        while self.eat_if(&f).is_some() {}
        self.pos > start
    }
    pub fn eat_while_re(&mut self, re: &Regex) -> bool {
        let start = self.pos;
        while self.eat_re(re).is_some() {}
        self.pos > start
    }
    pub fn eat_space(&mut self) -> bool {
        self.eat_while_if(|c| c.is_whitespace() || c == '\u{a0}')
    }
    pub fn skip_to_end(&mut self) {
        self.pos = self.end;
    }
    /// `skipTo(ch)`: move to the next occurrence (not past it).
    pub fn skip_to(&mut self, ch: char) -> bool {
        match self.chars[self.pos.min(self.end)..self.end]
            .iter()
            .position(|c| *c == ch)
        {
            Some(i) => {
                self.pos += i;
                true
            }
            None => false,
        }
    }
    /// `skipTo(str)` for a multi-char needle.
    pub fn skip_to_str(&mut self, needle: &str) -> bool {
        let rest = &self.string[self.bytes[self.pos.min(self.end)]..self.bytes[self.end]];
        match rest.find(needle) {
            Some(b) => {
                self.pos += js_len(&rest[..b]);
                true
            }
            None => false,
        }
    }
    pub fn back_up(&mut self, n: usize) {
        self.pos = self.pos.saturating_sub(n);
    }
    pub fn column(&mut self) -> usize {
        if self.last_column_pos < self.start {
            self.last_column_value = count_column(
                &self.chars[..self.end],
                Some(self.start),
                self.tab_size,
                self.last_column_pos,
                self.last_column_value,
            );
            self.last_column_pos = self.start;
        }
        self.last_column_value
            - if self.line_start > 0 {
                count_column(
                    &self.chars[..self.end],
                    Some(self.line_start),
                    self.tab_size,
                    0,
                    0,
                )
            } else {
                0
            }
    }
    pub fn indentation(&self) -> usize {
        count_column(&self.chars[..self.end], None, self.tab_size, 0, 0)
            - if self.line_start > 0 {
                count_column(
                    &self.chars[..self.end],
                    Some(self.line_start),
                    self.tab_size,
                    0,
                    0,
                )
            } else {
                0
            }
    }
    /// `match(string, consume, caseInsensitive)`
    pub fn match_str(&mut self, pattern: &str, consume: bool, case_insensitive: bool) -> bool {
        let n = js_len(pattern);
        if self.pos + n > self.end {
            return false;
        }
        let sub = self.slice(self.pos, self.pos + n);
        let ok = if case_insensitive {
            sub.to_lowercase() == pattern.to_lowercase()
        } else {
            sub == pattern
        };
        if ok && consume {
            self.pos += n;
        }
        ok
    }
    /// `match(regex, consume)`: JS `slice(pos).match(re)`, null unless the
    /// match starts at `pos`.
    pub fn match_re(&mut self, re: &Regex, consume: bool) -> Option<Match> {
        let rest = &self.string[self.bytes[self.pos.min(self.end)]..self.bytes[self.end]];
        let caps = re.captures(rest).ok().flatten()?;
        let whole = caps.get(0)?;
        if whole.start() > 0 {
            return None;
        }
        let m = Match {
            text: whole.as_str().to_string(),
            groups: (1..caps.len())
                .map(|i| caps.get(i).map(|g| g.as_str().to_string()))
                .collect(),
        };
        if consume {
            self.pos += js_len(whole.as_str());
        }
        Some(m)
    }
    /// `match(re)` as a boolean, consuming.
    pub fn matches(&mut self, re: &Regex) -> bool {
        self.match_re(re, true).is_some()
    }
    pub fn current(&self) -> &str {
        self.slice(self.start, self.pos)
    }
    /// `hideFirstChars(n, inner)`
    pub fn hide_first_chars<R>(&mut self, n: usize, inner: impl FnOnce(&mut Self) -> R) -> R {
        self.line_start += n;
        let r = inner(self);
        self.line_start -= n;
        r
    }
    /// `lookAhead(n)`: line `n` below this one.
    pub fn look_ahead(&self, n: usize) -> Option<&'a str> {
        self.lines.get(self.line + n).copied()
    }
    /// The line's index in the document: GHD makes one stream per line, so
    /// this stands in for the stream's identity (markdown's
    /// `stream != state.thisLine.stream`).
    pub fn line(&self) -> usize {
        self.line
    }
}

/// Compile a regex once (`static` per call site).
#[macro_export]
macro_rules! re {
    ($pattern:expr) => {{
        static RE: std::sync::OnceLock<fancy_regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| fancy_regex::Regex::new($pattern).expect("mode regex"))
    }};
}

/// Tokens of one line: (char start, char length, style).
pub type LineTokens = Vec<(usize, usize, String)>;

/// GHD's highlighter worker loop over a whole document.
pub fn run(mode: &dyn Mode, lines: &[&str], tab_size: usize) -> Vec<LineTokens> {
    let mut state = mode.start_state();
    let mut out = Vec::with_capacity(lines.len());
    for (ix, line) in lines.iter().enumerate() {
        let mut tokens = LineTokens::new();
        if line.is_empty() {
            mode.blank_line(&mut *state);
            out.push(tokens);
            continue;
        }
        let mut stream = StringStream::new(line, tab_size, lines, ix);
        while !stream.eol() {
            let mut token = None;
            let mut advanced = false;
            for _ in 0..10 {
                let inner = mode.inner_mode_name(&*state);
                let t = mode.token(&mut stream, &mut *state);
                if stream.pos > stream.start {
                    // a mode object without a `name` (`""`) adds no class
                    token = t.map(|t| {
                        if inner.is_empty() {
                            t
                        } else {
                            format!("m-{inner} {t}")
                        }
                    });
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                // GHD throws "failed to advance stream"; skip the char instead
                stream.pos += 1;
            }
            if let Some(token) = token {
                tokens.push((stream.start, stream.pos - stream.start, token));
            }
            stream.start = stream.pos;
        }
        out.push(tokens);
    }
    out
}

/// The `.cm-s-default` cascade (`styles/ui/_diff.scss`): the colour GHD
/// shows for a token's classes. Rules in source order with their class
/// selectors; the most specific matching rule wins, the later one on ties.
/// `None` = `color: inherit` or no rule.
pub fn resolve(style: &str) -> Option<TokenClass> {
    use TokenClass::*;
    const RULES: &[(&[&str], Option<TokenClass>)] = &[
        (&["variable"], Some(Variable)),
        (&["keyword"], Some(Keyword)),
        (&["atom"], Some(Atom)),
        (&["number"], None),
        (&["def"], None),
        (&["quote"], Some(Quote)),
        (&["variable-2"], Some(AltVariable)),
        (&["variable-3"], Some(AltVariable)),
        (&["comment"], Some(Comment)),
        (&["meta"], Some(Comment)),
        (&["string"], Some(String)),
        (&["string-2"], Some(String)),
        (&["string", "property"], None),
        (&["string-2", "property"], None),
        (&["qualifier"], Some(Qualifier)),
        (&["type"], Some(Type)),
        (&["builtin"], None),
        (&["bracket"], None),
        (&["tag"], Some(Tag)),
        (&["attribute"], Some(Attribute)),
        (&["hr"], None),
        (&["link"], Some(Link)),
        (&["header"], Some(Header)),
        (&["m-cmake", "def"], Some(Atom)),
        (&["m-css", "property"], Some(Atom)),
        (&["m-shell", "builtin"], Some(Atom)),
        (&["m-javascript", "type"], Some(Variable)),
        (&["m-toml", "string", "property"], Some(String)),
    ];
    let classes: Vec<&str> = style.split_whitespace().collect();
    let mut best: Option<(usize, Option<TokenClass>)> = None;
    for (selector, class) in RULES {
        if selector.iter().all(|s| classes.contains(s))
            && best.is_none_or(|(spec, _)| selector.len() >= spec)
        {
            best = Some((selector.len(), *class));
        }
    }
    best.and_then(|(_, class)| class)
}

/// Run `mode` over `lines` and resolve every token to its colour class, as
/// byte-range spans per line (adjacent spans of one class merged).
pub fn highlight(mode: &dyn Mode, lines: &[&str], budget: usize) -> Vec<Vec<Span>> {
    // GHD's MaxHighlightContentLength: no highlighting past the budget
    let total: usize = lines.iter().map(|l| l.len()).sum();
    if total > budget {
        return vec![Vec::new(); lines.len()];
    }
    let tokens = run(mode, lines, 4);
    tokens
        .into_iter()
        .zip(lines)
        .map(|(toks, line)| {
            let (_, offsets) = units(line);
            let mut spans: Vec<Span> = Vec::new();
            for (start, len, style) in toks {
                let Some(class) = resolve(&style) else {
                    continue;
                };
                let range = offsets[start.min(offsets.len() - 1)]
                    ..offsets[(start + len).min(offsets.len() - 1)];
                match spans.last_mut() {
                    Some(last) if last.class == class && last.range.end == range.start => {
                        last.range.end = range.end;
                    }
                    _ => spans.push(Span { range, class }),
                }
            }
            spans
        })
        .collect()
}

/// GHD `extensionMIMEMap` / `basenameMIMEMap` → a ported mode, or `None`
/// (the caller falls back to syntect).
pub fn mode_for_path(path: &str) -> Option<Arc<dyn Mode>> {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let ext = name
        .rfind('.')
        .map(|i| name[i..].to_string())
        .unwrap_or_default();
    let mime = modes::mime_for_extension(&ext).or_else(|| modes::mime_for_basename(&name))?;
    modes::mode_for_mime(mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cascade_follows_css_order_and_specificity() {
        assert_eq!(resolve("m-rust variable"), Some(TokenClass::Variable));
        assert_eq!(
            resolve("m-markdown comment variable-2"),
            Some(TokenClass::Comment)
        );
        assert_eq!(resolve("m-json string property"), None);
        assert_eq!(resolve("m-toml string property"), Some(TokenClass::String));
        assert_eq!(resolve("m-css property"), Some(TokenClass::Atom));
        assert_eq!(resolve("m-rust def"), None);
        assert_eq!(resolve("m-javascript type"), Some(TokenClass::Variable));
    }

    #[test]
    fn stream_basics() {
        let lines = ["  héllo world"];
        let mut s = StringStream::new(lines[0], 4, &lines, 0);
        assert!(s.eat_space());
        assert_eq!(s.indentation(), 2);
        assert!(s.match_re(re!(r"[a-zé]+"), true).is_some());
        assert_eq!(s.current(), "  héllo");
        assert!(s.match_str(" WORLD", false, true));
        assert!(!s.match_str(" WORLD", false, false));
    }

    #[test]
    fn with_end_hides_the_rest_of_the_line() {
        let lines = ["ab<%cd"];
        let mut s = StringStream::new(lines[0], 4, &lines, 0);
        s.with_end(2, |s| {
            s.skip_to_end();
            assert!(s.eol());
            assert_eq!(s.string(), "ab");
            assert!(s.match_re(re!("<"), false).is_none());
        });
        assert_eq!(s.pos, 2);
        assert!(!s.eol());
        assert_eq!(s.string(), "ab<%cd");
    }

    #[test]
    fn positions_are_utf16_units_like_js() {
        let lines = ["a😀b"];
        let mut s = StringStream::new(lines[0], 4, &lines, 0);
        assert_eq!(s.len(), 4);
        assert!(s.match_re(re!("a😀"), true).is_some());
        assert_eq!(s.pos, 3);
        assert_eq!(s.current(), "a😀");
        assert_eq!(s.next(), Some('b'));
    }
}
