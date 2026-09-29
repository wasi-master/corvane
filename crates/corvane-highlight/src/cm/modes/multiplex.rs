//! `codemirror/addon/mode/multiplex.js` (`CodeMirror.multiplexingMode`): an
//! outer mode with inner modes between `open` / `close` delimiters. While
//! the outer mode (or an inner one) runs, the line is cut at the next
//! delimiter ([`StringStream::with_end`]), exactly like the JS swaps
//! `stream.string`.
//!
//! Each `other.mode` is one mode instance for the whole document, so JS
//! per-instance scratch (javascript's `type` / `content`) carries from one
//! inner block to the next; [`Other::carry`] moves it from the block that
//! ended into the state `startState` gives the next one.

use std::sync::Arc;

use fancy_regex::Regex;

use crate::cm::{Mode, ModeState, StringStream, js_len, state, state_ref};

/// A delimiter: a string or a regex (`indexOf(string, pattern, …)`).
pub enum Delim {
    Str(&'static str),
    Re(Regex),
}

/// Move per-instance scratch from the previous block's state into the
/// next block's fresh state.
pub type Carry = fn(&dyn ModeState, &mut dyn ModeState);

/// One of the `others`: `{open, close, mode, delimStyle, innerStyle,
/// parseDelimiters}`.
pub struct Other {
    pub open: Delim,
    pub close: Option<Delim>,
    pub mode: Arc<dyn Mode>,
    pub delim_style: Option<&'static str>,
    pub inner_style: Option<&'static str>,
    pub parse_delimiters: bool,
    pub carry: Option<Carry>,
}

impl Other {
    pub fn new(open: Delim, close: Option<Delim>, mode: Arc<dyn Mode>) -> Self {
        Self {
            open,
            close,
            mode,
            delim_style: None,
            inner_style: None,
            parse_delimiters: false,
            carry: None,
        }
    }
}

pub struct Multiplex {
    /// the `getMode` spec name (`modeObj.name`)
    name: &'static str,
    outer: Arc<dyn Mode>,
    others: Vec<Other>,
}

#[derive(Clone)]
pub struct MultiplexState {
    outer: Box<dyn ModeState>,
    /// `state.innerActive` (index into `others`)
    inner_active: Option<usize>,
    inner: Option<Box<dyn ModeState>>,
    starting_inner: bool,
    /// each other's last inner state, for [`Other::carry`]
    retired: Vec<Option<Box<dyn ModeState>>>,
}

/// `indexOf(string, pattern, from, returnEnd)` on the visible line.
fn index_of(
    stream: &StringStream,
    pattern: &Delim,
    from: usize,
    return_end: bool,
) -> Option<usize> {
    let rest = stream.slice(from, stream.len());
    let (start, end) = match pattern {
        Delim::Str(s) => {
            let b = rest.find(s)?;
            (b, b + s.len())
        }
        Delim::Re(re) => {
            let m = re.find(rest).ok().flatten()?;
            (m.start(), m.end())
        }
    };
    let at = if return_end { end } else { start };
    Some(from + js_len(&rest[..at]))
}

/// `stream.match(pattern)` (consuming).
fn match_delim(stream: &mut StringStream, pattern: &Delim) {
    match pattern {
        Delim::Str(s) => {
            stream.match_str(s, true, false);
        }
        Delim::Re(re) => {
            stream.match_re(re, true);
        }
    }
}

fn delim(style: Option<&'static str>, suffix: &str) -> Option<String> {
    style.map(|d| format!("{d} {d}-{suffix}"))
}

impl Multiplex {
    /// `CodeMirror.multiplexingMode(outer, ...others)` as the mode `name`.
    pub fn new(name: &'static str, outer: Arc<dyn Mode>, others: Vec<Other>) -> Self {
        Self {
            name,
            outer,
            others,
        }
    }

    /// `state.inner = CodeMirror.startState(other.mode, …)`
    fn start_inner(&self, i: usize, s: &mut MultiplexState) {
        let other = &self.others[i];
        let mut inner = other.mode.start_state();
        if let (Some(carry), Some(Some(prev))) = (other.carry, s.retired.get(i)) {
            carry(&**prev, &mut *inner);
        }
        s.inner_active = Some(i);
        s.inner = Some(inner);
    }

    /// `state.innerActive = state.inner = null`
    fn end_inner(s: &mut MultiplexState) {
        if let (Some(i), Some(inner)) = (s.inner_active.take(), s.inner.take())
            && let Some(slot) = s.retired.get_mut(i)
        {
            *slot = Some(inner);
        }
    }

    fn token_mx(&self, stream: &mut StringStream, s: &mut MultiplexState) -> Option<String> {
        let Some(i) = s.inner_active else {
            let mut cut_off: Option<usize> = None;
            for (i, other) in self.others.iter().enumerate() {
                let found = index_of(stream, &other.open, stream.pos, false);
                if found == Some(stream.pos) {
                    if !other.parse_delimiters {
                        match_delim(stream, &other.open);
                    }
                    s.starting_inner = other.parse_delimiters;
                    self.start_inner(i, s);
                    return delim(other.delim_style, "open");
                } else if let Some(found) = found
                    && cut_off.is_none_or(|c| found < c)
                {
                    cut_off = Some(found);
                }
            }
            let outer = &mut *s.outer;
            return match cut_off {
                Some(cut) => stream.with_end(cut, |stream| self.outer.token(stream, outer)),
                None => self.outer.token(stream, outer),
            };
        };
        let cur = &self.others[i];
        if cur.close.is_none() && stream.sol() {
            Self::end_inner(s);
            return self.token_mx(stream, s);
        }
        let found = match &cur.close {
            Some(close) if !s.starting_inner => {
                index_of(stream, close, stream.pos, cur.parse_delimiters)
            }
            _ => None,
        };
        if found == Some(stream.pos) && !cur.parse_delimiters {
            if let Some(close) = &cur.close {
                match_delim(stream, close);
            }
            Self::end_inner(s);
            return delim(cur.delim_style, "close");
        }
        let inner = &mut **s.inner.as_mut()?;
        let mut token = match found {
            Some(end) => stream.with_end(end, |stream| cur.mode.token(stream, inner)),
            None => {
                let token = cur.mode.token(stream, inner);
                if stream.pos > stream.start {
                    s.starting_inner = false;
                }
                token
            }
        };
        if found == Some(stream.pos) && cur.parse_delimiters {
            Self::end_inner(s);
        }
        if let Some(style) = cur.inner_style {
            token = Some(match token {
                Some(t) => format!("{t} {style}"),
                None => style.to_string(),
            });
        }
        token
    }
}

impl Mode for Multiplex {
    fn name(&self) -> &'static str {
        self.name
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(MultiplexState {
            outer: self.outer.start_state(),
            inner_active: None,
            inner: None,
            starting_inner: false,
            retired: vec![None; self.others.len()],
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        self.token_mx(stream, state::<MultiplexState>(st))
    }

    /// `blankLine`
    fn blank_line(&self, st: &mut dyn ModeState) {
        let s = state::<MultiplexState>(st);
        match (s.inner_active, s.inner.as_mut()) {
            (Some(i), Some(inner)) => self.others[i].mode.blank_line(&mut **inner),
            _ => self.outer.blank_line(&mut *s.outer),
        }
        if s.inner_active.is_none() {
            for i in 0..self.others.len() {
                if matches!(self.others[i].open, Delim::Str("\n")) {
                    self.start_inner(i, s);
                }
            }
        } else if let Some(i) = s.inner_active
            && matches!(self.others[i].close, Some(Delim::Str("\n")))
        {
            Self::end_inner(s);
        }
    }

    /// `innerMode`: the inner mode while one is active, else the outer
    /// mode (followed down its own nesting)
    fn inner_mode_name(&self, st: &dyn ModeState) -> &'static str {
        let s = state_ref::<MultiplexState>(st);
        match (s.inner_active, &s.inner) {
            (Some(i), Some(inner)) => self.others[i].mode.inner_mode_name(&**inner),
            _ => self.outer.inner_mode_name(&*s.outer),
        }
    }
}
