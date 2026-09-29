//! `codemirror/mode/rst/rst.js` (`text/x-rst`), ported line by line. GHD
//! maps `.rst` to it.
//!
//! `rst` is `CodeMirror.overlayMode(rst-base, overlay, true)`
//! (`addon/mode/overlay.js`): the `rst-base` mode (sections, directives,
//! roles, links; Python for `>>>` examples and `.. python::`, stex for
//! `:math:` roles and `.. math::`) with an inline overlay (strong, em,
//! literals, numbers, URIs) whose style is appended to the base style.
//! Because `combine` is set and the overlay has no state, an overlay style
//! over an unstyled base token comes out as `"null <overlay>"`, exactly
//! like the JS string concatenation.
//!
//! `overlayMode` resets its positions when it sees a new `StringStream`
//! (`stream != state.streamSeen`); here a token at `stream.start == 0`
//! marks a new line. That also re-runs the overlay on a retried first
//! token, which is harmless: the overlay is stateless, so it yields the
//! same style and end position again.

use std::sync::Arc;

use crate::cm::{Mode, ModeState, StringStream, state};
use crate::re;

use super::{python, stex};

pub struct Rst {
    python: Arc<dyn Mode>,
    stex: Arc<dyn Mode>,
}

impl Rst {
    pub fn new() -> Self {
        Self {
            python: Arc::new(python::Python::new()),
            stex: Arc::new(stex::Stex::new()),
        }
    }
}

impl Default for Rst {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// regexes of `rst-base` (JS `\w`, `\W`, `\d` are ASCII; `\s` is Unicode)

/// `NAME`
macro_rules! name {
    () => {
        r##"(?:[A-Za-z](?:[A-Za-z0-9_!"#$%&'()*+,\-./:;<=>?]*[A-Za-z0-9])?)"##
    };
}
/// `REF_NAME` = `(?:NAME|`NAME_WWS`)`
macro_rules! ref_name {
    () => {
        concat!(
            "(?:",
            name!(),
            r##"|`(?:[A-Za-z](?:[A-Za-z0-9_\s!"#$%&'()*+,\-./:;<=>?]*[A-Za-z0-9])?)`)"##
        )
    };
}
/// `TAIL`
macro_rules! tail {
    () => {
        r"(?:\s*|[^A-Za-z0-9_]|$)"
    };
}
/// `TEXT1`
macro_rules! text1 {
    () => {
        r"(?:[^\s|](?:[^|]*[^\s|])?)"
    };
}
/// `TEXT2`
macro_rules! text2 {
    () => {
        r"(?:[^`]+)"
    };
}

fn rx_tail() -> &'static fancy_regex::Regex {
    re!(concat!("^", tail!()))
}
fn rx_name() -> &'static fancy_regex::Regex {
    re!(concat!("^", name!()))
}
fn rx_text2() -> &'static fancy_regex::Regex {
    re!(concat!("^", text2!()))
}
fn rx_section() -> &'static fancy_regex::Regex {
    re!(r"^([!'#$%&\x22()*+,\-./:;<=>?@\[\\\]^_`{|}~])\1{3,}\s*$")
}
fn rx_explicit() -> &'static fancy_regex::Regex {
    re!(r"^\.\.\s+")
}
fn rx_link() -> &'static fancy_regex::Regex {
    re!(concat!(
        "^(?:_",
        ref_name!(),
        ":",
        tail!(),
        "|__:",
        tail!(),
        ")"
    ))
}
fn rx_directive() -> &'static fancy_regex::Regex {
    re!(concat!("^", ref_name!(), "::", tail!()))
}
fn rx_substitution() -> &'static fancy_regex::Regex {
    re!(concat!(
        r"^\|",
        text1!(),
        r"\|\s+",
        ref_name!(),
        "::",
        tail!()
    ))
}
fn rx_footnote() -> &'static fancy_regex::Regex {
    re!(concat!(r"^\[(?:[0-9]+|#", ref_name!(), r"?|\*)\]", tail!()))
}
fn rx_citation() -> &'static fancy_regex::Regex {
    re!(concat!(r"^\[", ref_name!(), r"\]", tail!()))
}
fn rx_substitution_ref() -> &'static fancy_regex::Regex {
    re!(concat!(r"^\|", text1!(), r"\|"))
}
fn rx_footnote_ref() -> &'static fancy_regex::Regex {
    re!(concat!(r"^\[(?:[0-9]+|#", ref_name!(), r"?|\*)\]_"))
}
fn rx_citation_ref() -> &'static fancy_regex::Regex {
    re!(concat!(r"^\[", ref_name!(), r"\]_"))
}
fn rx_link_ref1() -> &'static fancy_regex::Regex {
    re!(concat!("^", ref_name!(), "__?"))
}
fn rx_link_ref2() -> &'static fancy_regex::Regex {
    re!(concat!("^`", text2!(), "`_"))
}
fn rx_role_pre() -> &'static fancy_regex::Regex {
    re!(concat!("^:", name!(), ":`", text2!(), "`", tail!()))
}
fn rx_role_suf() -> &'static fancy_regex::Regex {
    re!(concat!("^`", text2!(), "`:", name!(), ":", tail!()))
}
fn rx_role() -> &'static fancy_regex::Regex {
    re!(concat!("^:", name!(), ":", tail!()))
}
fn rx_ref_name() -> &'static fancy_regex::Regex {
    // rx_directive_name, rx_substitution_name
    re!(concat!("^", ref_name!()))
}
fn rx_double_colon_tail() -> &'static fancy_regex::Regex {
    // rx_directive_tail, rx_substitution_tail
    re!(concat!("^::", tail!()))
}
fn rx_link_name() -> &'static fancy_regex::Regex {
    // `^REF_NAME|_`: the bare `_` alternative is unanchored in JS, but
    // `stream.match` only accepts a match at the stream position
    re!(concat!("^(?:", ref_name!(), "|_)"))
}
fn rx_link_tail() -> &'static fancy_regex::Regex {
    re!(concat!("^:", tail!()))
}
fn rx_verbatim() -> &'static fancy_regex::Regex {
    re!(r"^::\s*$")
}
fn rx_examples() -> &'static fancy_regex::Regex {
    re!(r"^\s+(?:>>>|In \[[0-9]+\]:)\s")
}

// ---------------------------------------------------------------------------
// state of `rst-base`

#[derive(Clone, Copy, PartialEq)]
enum Tok {
    /// `to_normal`
    Normal,
    /// `to_explicit`
    Explicit,
    /// `to_comment`
    Comment,
    /// `to_verbatim`
    Verbatim,
    /// `to_mode`
    Mode,
}

/// `ctx.phase`: which regex the multi-token construct belongs to.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    RolePre,
    RoleSuf,
    Role,
    SubstitutionRef,
    LinkRef2,
    Substitution,
    Directive,
    Link,
}

#[derive(Clone, Copy, PartialEq)]
enum Inner {
    Python,
    Stex,
}

/// `{mode, local}`
#[derive(Clone)]
struct Local {
    mode: Inner,
    state: Box<dyn ModeState>,
}

/// `state.ctx` (`context(phase, stage, mode, local)`)
#[derive(Clone, Default)]
struct Ctx {
    phase: Option<Phase>,
    stage: usize,
    local: Option<Local>,
}

fn context(phase: Phase, stage: usize) -> Ctx {
    Ctx {
        phase: Some(phase),
        stage,
        local: None,
    }
}

#[derive(Clone)]
struct BaseState {
    tok: Tok,
    ctx: Ctx,
    tmp: Option<Local>,
    tmp_stex: bool,
    tmp_py: bool,
}

// change
fn change(s: &mut BaseState, tok: Tok, ctx: Ctx) {
    s.tok = tok;
    s.ctx = ctx;
}

/// `/^\W$/.test(stream.peek())`, or no next char
fn peek_is_end_or_non_word(stream: &StringStream) -> bool {
    stream
        .peek()
        .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
}

/// State of the overlay mode (`CodeMirror.overlayMode`).
#[derive(Clone)]
struct RstState {
    base: BaseState,
    base_pos: usize,
    base_cur: Option<String>,
    overlay_pos: usize,
    overlay_cur: Option<&'static str>,
}

impl Rst {
    fn inner(&self, kind: Inner) -> &dyn Mode {
        match kind {
            Inner::Python => &*self.python,
            Inner::Stex => &*self.stex,
        }
    }

    fn local(&self, kind: Inner) -> Local {
        Local {
            mode: kind,
            state: self.inner(kind).start_state(),
        }
    }

    // to_normal
    fn to_normal(&self, stream: &mut StringStream, s: &mut BaseState) -> Option<String> {
        let mut token: Option<&'static str> = None;
        let phase = s.ctx.phase;
        let stage = s.ctx.stage;

        if stream.sol() && stream.match_re(rx_examples(), false).is_some() {
            let local = self.local(Inner::Python);
            change(
                s,
                Tok::Mode,
                Ctx {
                    local: Some(local),
                    ..Ctx::default()
                },
            );
        } else if stream.sol() && stream.matches(rx_explicit()) {
            change(s, Tok::Explicit, Ctx::default());
            token = Some("meta");
        } else if stream.sol() && stream.matches(rx_section()) {
            change(s, Tok::Normal, Ctx::default());
            token = Some("header");
        } else if phase == Some(Phase::RolePre) || stream.match_re(rx_role_pre(), false).is_some() {
            match stage {
                0 => {
                    change(s, Tok::Normal, context(Phase::RolePre, 1));
                    stream.match_str(":", true, false);
                    token = Some("meta");
                }
                1 => {
                    change(s, Tok::Normal, context(Phase::RolePre, 2));
                    stream.matches(rx_name());
                    token = Some("keyword");
                    let cur = stream.current();
                    if cur.starts_with("math") || cur.starts_with("latex") {
                        s.tmp_stex = true;
                    }
                }
                2 => {
                    change(s, Tok::Normal, context(Phase::RolePre, 3));
                    stream.match_str(":`", true, false);
                    token = Some("meta");
                }
                3 => {
                    if s.tmp_stex {
                        s.tmp_stex = false;
                        s.tmp = Some(self.local(Inner::Stex));
                    }
                    if s.tmp.is_some() && stream.peek() == Some('`') {
                        change(s, Tok::Normal, context(Phase::RolePre, 4));
                        s.tmp = None;
                        return None;
                    }
                    if let Some(tmp) = &mut s.tmp {
                        return self.inner(tmp.mode).token(stream, &mut *tmp.state);
                    }
                    change(s, Tok::Normal, context(Phase::RolePre, 4));
                    stream.matches(rx_text2());
                    token = Some("string");
                }
                4 => {
                    change(s, Tok::Normal, context(Phase::RolePre, 5));
                    stream.match_str("`", true, false);
                    token = Some("meta");
                }
                5 => {
                    change(s, Tok::Normal, context(Phase::RolePre, 6));
                    stream.matches(rx_tail());
                }
                _ => change(s, Tok::Normal, Ctx::default()),
            }
        } else if phase == Some(Phase::RoleSuf) || stream.match_re(rx_role_suf(), false).is_some() {
            match stage {
                0 => {
                    change(s, Tok::Normal, context(Phase::RoleSuf, 1));
                    stream.match_str("`", true, false);
                    token = Some("meta");
                }
                1 => {
                    change(s, Tok::Normal, context(Phase::RoleSuf, 2));
                    stream.matches(rx_text2());
                    token = Some("string");
                }
                2 => {
                    change(s, Tok::Normal, context(Phase::RoleSuf, 3));
                    stream.match_str("`:", true, false);
                    token = Some("meta");
                }
                3 => {
                    change(s, Tok::Normal, context(Phase::RoleSuf, 4));
                    stream.matches(rx_name());
                    token = Some("keyword");
                }
                4 => {
                    change(s, Tok::Normal, context(Phase::RoleSuf, 5));
                    stream.match_str(":", true, false);
                    token = Some("meta");
                }
                5 => {
                    change(s, Tok::Normal, context(Phase::RoleSuf, 6));
                    stream.matches(rx_tail());
                }
                _ => change(s, Tok::Normal, Ctx::default()),
            }
        } else if phase == Some(Phase::Role) || stream.match_re(rx_role(), false).is_some() {
            match stage {
                0 => {
                    change(s, Tok::Normal, context(Phase::Role, 1));
                    stream.match_str(":", true, false);
                    token = Some("meta");
                }
                1 => {
                    change(s, Tok::Normal, context(Phase::Role, 2));
                    stream.matches(rx_name());
                    token = Some("keyword");
                }
                2 => {
                    change(s, Tok::Normal, context(Phase::Role, 3));
                    stream.match_str(":", true, false);
                    token = Some("meta");
                }
                3 => {
                    change(s, Tok::Normal, context(Phase::Role, 4));
                    stream.matches(rx_tail());
                }
                _ => change(s, Tok::Normal, Ctx::default()),
            }
        } else if phase == Some(Phase::SubstitutionRef)
            || stream.match_re(rx_substitution_ref(), false).is_some()
        {
            match stage {
                0 => {
                    change(s, Tok::Normal, context(Phase::SubstitutionRef, 1));
                    stream.matches(rx_substitution_ref());
                    token = Some("variable-2");
                }
                1 => {
                    change(s, Tok::Normal, context(Phase::SubstitutionRef, 2));
                    // `/^_?_?/` always matches, possibly empty
                    stream.eat('_');
                    stream.eat('_');
                    token = Some("link");
                }
                _ => change(s, Tok::Normal, Ctx::default()),
            }
        } else if stream.matches(rx_footnote_ref()) || stream.matches(rx_citation_ref()) {
            change(s, Tok::Normal, Ctx::default());
            token = Some("quote");
        } else if stream.matches(rx_link_ref1()) {
            change(s, Tok::Normal, Ctx::default());
            if peek_is_end_or_non_word(stream) {
                token = Some("link");
            }
        } else if phase == Some(Phase::LinkRef2) || stream.match_re(rx_link_ref2(), false).is_some()
        {
            match stage {
                0 => {
                    if peek_is_end_or_non_word(stream) {
                        change(s, Tok::Normal, context(Phase::LinkRef2, 1));
                    } else {
                        stream.matches(rx_link_ref2());
                    }
                }
                1 => {
                    change(s, Tok::Normal, context(Phase::LinkRef2, 2));
                    stream.match_str("`", true, false);
                    token = Some("link");
                }
                2 => {
                    change(s, Tok::Normal, context(Phase::LinkRef2, 3));
                    stream.matches(rx_text2());
                }
                3 => {
                    change(s, Tok::Normal, context(Phase::LinkRef2, 4));
                    stream.match_str("`_", true, false);
                    token = Some("link");
                }
                _ => change(s, Tok::Normal, Ctx::default()),
            }
        } else if stream.matches(rx_verbatim()) {
            change(s, Tok::Verbatim, Ctx::default());
        } else if stream.next().is_some() {
            change(s, Tok::Normal, Ctx::default());
        }
        token.map(Into::into)
    }

    // to_explicit
    fn to_explicit(&self, stream: &mut StringStream, s: &mut BaseState) -> Option<String> {
        let mut token: Option<&'static str> = None;
        let phase = s.ctx.phase;
        let stage = s.ctx.stage;

        if phase == Some(Phase::Substitution) || stream.match_re(rx_substitution(), false).is_some()
        {
            match stage {
                0 => {
                    change(s, Tok::Explicit, context(Phase::Substitution, 1));
                    stream.matches(rx_substitution_ref());
                    token = Some("variable-2");
                }
                1 => {
                    change(s, Tok::Explicit, context(Phase::Substitution, 2));
                    stream.matches(re!(r"^\s+"));
                }
                2 => {
                    change(s, Tok::Explicit, context(Phase::Substitution, 3));
                    stream.matches(rx_ref_name());
                    token = Some("keyword");
                }
                3 => {
                    change(s, Tok::Explicit, context(Phase::Substitution, 4));
                    stream.matches(rx_double_colon_tail());
                    token = Some("meta");
                }
                _ => change(s, Tok::Normal, Ctx::default()),
            }
        } else if phase == Some(Phase::Directive)
            || stream.match_re(rx_directive(), false).is_some()
        {
            match stage {
                0 => {
                    change(s, Tok::Explicit, context(Phase::Directive, 1));
                    stream.matches(rx_ref_name());
                    token = Some("keyword");
                    let cur = stream.current();
                    if cur.starts_with("math") || cur.starts_with("latex") {
                        s.tmp_stex = true;
                    } else if cur.starts_with("python") {
                        s.tmp_py = true;
                    }
                }
                1 => {
                    change(s, Tok::Explicit, context(Phase::Directive, 2));
                    stream.matches(rx_double_colon_tail());
                    token = Some("meta");
                    if stream.matches(re!(r"^latex\s*$")) || s.tmp_stex {
                        s.tmp_stex = false;
                        let local = self.local(Inner::Stex);
                        change(
                            s,
                            Tok::Mode,
                            Ctx {
                                local: Some(local),
                                ..Ctx::default()
                            },
                        );
                    }
                }
                2 => {
                    change(s, Tok::Explicit, context(Phase::Directive, 3));
                    if stream.matches(re!(r"^python\s*$")) || s.tmp_py {
                        s.tmp_py = false;
                        let local = self.local(Inner::Python);
                        change(
                            s,
                            Tok::Mode,
                            Ctx {
                                local: Some(local),
                                ..Ctx::default()
                            },
                        );
                    }
                }
                _ => change(s, Tok::Normal, Ctx::default()),
            }
        } else if phase == Some(Phase::Link) || stream.match_re(rx_link(), false).is_some() {
            match stage {
                0 => {
                    change(s, Tok::Explicit, context(Phase::Link, 1));
                    stream.match_str("_", true, false);
                    stream.matches(rx_link_name());
                    token = Some("link");
                }
                1 => {
                    change(s, Tok::Explicit, context(Phase::Link, 2));
                    stream.matches(rx_link_tail());
                    token = Some("meta");
                }
                _ => change(s, Tok::Normal, Ctx::default()),
            }
        } else if stream.matches(rx_footnote()) || stream.matches(rx_citation()) {
            change(s, Tok::Normal, Ctx::default());
            token = Some("quote");
        } else {
            stream.eat_space();
            if stream.eol() {
                change(s, Tok::Normal, Ctx::default());
            } else {
                stream.skip_to_end();
                change(s, Tok::Comment, Ctx::default());
                token = Some("comment");
            }
        }
        token.map(Into::into)
    }

    // to_mode
    fn to_mode(&self, stream: &mut StringStream, s: &mut BaseState) -> Option<String> {
        if s.ctx.local.is_some() && stream.sol() {
            if !stream.eat_space() {
                change(s, Tok::Normal, Ctx::default());
            }
            return None;
        }
        if let Some(local) = &mut s.ctx.local {
            return self.inner(local.mode).token(stream, &mut *local.state);
        }
        change(s, Tok::Normal, Ctx::default());
        None
    }

    /// `rst-base` `token`
    fn base_token(&self, stream: &mut StringStream, s: &mut BaseState) -> Option<String> {
        match s.tok {
            Tok::Normal => self.to_normal(stream, s),
            Tok::Explicit => self.to_explicit(stream, s),
            Tok::Comment => as_block(stream, s, "comment"),
            Tok::Verbatim => as_block(stream, s, "meta"),
            Tok::Mode => self.to_mode(stream, s),
        }
    }
}

// as_block (to_comment, to_verbatim)
fn as_block(stream: &mut StringStream, s: &mut BaseState, token: &'static str) -> Option<String> {
    if stream.eol() || stream.eat_space() {
        stream.skip_to_end();
        Some(token.into())
    } else {
        change(s, Tok::Normal, Ctx::default());
        None
    }
}

// ---------------------------------------------------------------------------
// the `rst` overlay

fn rx_strong() -> &'static fancy_regex::Regex {
    re!(r"^\*\*[^*\s](?:[^*]*[^*\s])?\*\*")
}
fn rx_emphasis() -> &'static fancy_regex::Regex {
    re!(r"^\*[^*\s](?:[^*]*[^*\s])?\*")
}
fn rx_literal() -> &'static fancy_regex::Regex {
    re!(r"^``[^`\s](?:[^`]*[^`\s])``")
}
fn rx_number() -> &'static fancy_regex::Regex {
    re!(r"^(?:[0-9]+(?:[.,][0-9]+)*)")
}
fn rx_positive() -> &'static fancy_regex::Regex {
    re!(r"^(?:\s\+[0-9]+(?:[.,][0-9]+)*)")
}
fn rx_negative() -> &'static fancy_regex::Regex {
    re!(r"^(?:\s\-[0-9]+(?:[.,][0-9]+)*)")
}
fn rx_uri() -> &'static fancy_regex::Regex {
    re!(
        r"^[Hh][Tt][Tt][Pp][Ss]?://(?:[0-9A-Za-z_.-]+)\.(?:[A-Za-z0-9_]{2,6})(?:/[0-9A-Za-z_#%&\-.,/:=?~]+)*"
    )
}

/// `stream.match(/\W+|$/, false)`: a non-word char or the end next.
fn followed_by_non_word(stream: &StringStream) -> bool {
    peek_is_end_or_non_word(stream)
}

/// Whether any overlay construct starts at the stream position (the
/// `stream.match(rx, false)` checks of the skipping loop), with a cheap
/// first-char filter before each regex.
fn overlay_starts_here(stream: &mut StringStream) -> bool {
    let Some(c) = stream.peek() else {
        return false;
    };
    (c == '*'
        && (stream.match_re(rx_strong(), false).is_some()
            || stream.match_re(rx_emphasis(), false).is_some()))
        || (c == '`' && stream.match_re(rx_literal(), false).is_some())
        || (c.is_ascii_digit() && stream.match_re(rx_number(), false).is_some())
        || (c.is_whitespace()
            && (stream.match_re(rx_positive(), false).is_some()
                || stream.match_re(rx_negative(), false).is_some()))
        || ((c == 'h' || c == 'H') && stream.match_re(rx_uri(), false).is_some())
}

/// `overlay.token`
fn overlay_token(stream: &mut StringStream) -> Option<&'static str> {
    if stream.matches(rx_strong()) && followed_by_non_word(stream) {
        return Some("strong");
    }
    if stream.matches(rx_emphasis()) && followed_by_non_word(stream) {
        return Some("em");
    }
    if stream.matches(rx_literal()) && followed_by_non_word(stream) {
        return Some("string-2");
    }
    if stream.matches(rx_number()) {
        return Some("number");
    }
    if stream.matches(rx_positive()) {
        return Some("positive");
    }
    if stream.matches(rx_negative()) {
        return Some("negative");
    }
    if stream.matches(rx_uri()) {
        return Some("link");
    }
    while stream.next().is_some() {
        if overlay_starts_here(stream) {
            break;
        }
    }
    None
}

impl Mode for Rst {
    fn name(&self) -> &'static str {
        "rst"
    }

    // overlayMode startState (+ rst-base startState)
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(RstState {
            base: BaseState {
                tok: Tok::Normal,
                ctx: Ctx::default(),
                tmp: None,
                tmp_stex: false,
                tmp_py: false,
            },
            base_pos: 0,
            base_cur: None,
            overlay_pos: 0,
            overlay_cur: None,
        })
    }

    // overlayMode token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<RstState>(st);
        if stream.start == 0 || s.base_pos.min(s.overlay_pos) < stream.start {
            s.base_pos = stream.start;
            s.overlay_pos = stream.start;
        }
        if stream.start == s.base_pos {
            s.base_cur = self.base_token(stream, &mut s.base);
            s.base_pos = stream.pos;
        }
        if stream.start == s.overlay_pos {
            stream.pos = stream.start;
            s.overlay_cur = overlay_token(stream);
            s.overlay_pos = stream.pos;
        }
        stream.pos = s.base_pos.min(s.overlay_pos);

        match s.overlay_cur {
            None => s.base_cur.clone(),
            // combine, and the overlay state (`true`) has no combineTokens
            Some(overlay) => Some(format!(
                "{} {overlay}",
                s.base_cur.as_deref().unwrap_or("null")
            )),
        }
    }

    // overlayMode innerMode → rst-base innerMode
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        let s = crate::cm::state_ref::<RstState>(st);
        if let Some(tmp) = &s.base.tmp {
            self.inner(tmp.mode).name()
        } else if let Some(local) = &s.base.ctx.local {
            self.inner(local.mode).name()
        } else {
            "rst-base"
        }
    }
}
