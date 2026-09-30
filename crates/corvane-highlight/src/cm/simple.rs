//! `codemirror/addon/mode/simple.js` (`CodeMirror.defineSimpleMode`): a
//! mode described as states of regex rules. Supported rule fields: `regex`,
//! `token` (one style, or one per capture group - dots become spaces),
//! `next`, `push`, `pop`, `sol`, and `mode` (a nested local mode with an
//! `end` regex). `indent` / `dedent` only affect indentation, which the
//! highlighter never asks for, so they are ignored.

use std::collections::HashMap;
use std::sync::Arc;

use fancy_regex::Regex;
use regex_automata::meta::Regex as Meta;
use regex_automata::{Anchored, Input};

use super::{Mode, ModeState, StringStream, state, state_ref};

/// Token spec: one style for the whole match, or one per group.
#[derive(Clone)]
pub enum Token {
    None,
    One(&'static str),
    Groups(&'static [Option<&'static str>]),
}

/// A nested mode entered by a rule (`mode: {spec, end, endToken}`).
#[derive(Clone)]
pub struct Local {
    pub mode: Arc<dyn Mode>,
    pub end: Option<&'static str>,
    pub end_token: Option<&'static str>,
}

#[derive(Clone)]
pub struct RuleSpec {
    pub regex: &'static str,
    pub case_insensitive: bool,
    pub token: Token,
    pub next: Option<&'static str>,
    pub push: Option<&'static str>,
    pub pop: bool,
    pub sol: bool,
    pub local: Option<Local>,
}

impl RuleSpec {
    pub const fn new(regex: &'static str, token: Token) -> Self {
        Self {
            regex,
            case_insensitive: false,
            token,
            next: None,
            push: None,
            pop: false,
            sol: false,
            local: None,
        }
    }
    pub const fn next(mut self, next: &'static str) -> Self {
        self.next = Some(next);
        self
    }
    pub const fn push(mut self, state: &'static str) -> Self {
        self.push = Some(state);
        self
    }
    pub const fn pop(mut self) -> Self {
        self.pop = true;
        self
    }
    pub const fn sol(mut self) -> Self {
        self.sol = true;
        self
    }
    pub const fn ci(mut self) -> Self {
        self.case_insensitive = true;
        self
    }
    pub fn local(mut self, local: Local) -> Self {
        self.local = Some(local);
        self
    }
}

/// Shorthands for table-style mode definitions.
pub const fn r(regex: &'static str, token: &'static str) -> RuleSpec {
    RuleSpec::new(regex, Token::One(token))
}
pub const fn r0(regex: &'static str) -> RuleSpec {
    RuleSpec::new(regex, Token::None)
}
pub const fn rg(regex: &'static str, tokens: &'static [Option<&'static str>]) -> RuleSpec {
    RuleSpec::new(regex, Token::Groups(tokens))
}

struct Rule {
    re: Regex,
    spec: RuleSpec,
    end: Option<Regex>,
}

/// A run of rules to try in order (see [`SimpleMode::new`]).
enum Segment {
    /// Consecutive rules the `regex` crate runs, as one multi-pattern regex
    /// per case: an anchored leftmost-first search returns the first rule
    /// (lowest pattern id) that matches here, which is what trying them in
    /// order does, in one pass instead of one search per rule.
    Fast {
        /// at the start of the line: every rule, with its index
        at_sol: Option<(Meta, Vec<usize>)>,
        /// elsewhere: the rules without `sol`
        elsewhere: Option<(Meta, Vec<usize>)>,
    },
    /// a rule only fancy-regex runs (lookaround, backreferences)
    Slow(usize),
}

pub struct SimpleMode {
    name: &'static str,
    states: HashMap<&'static str, Vec<Rule>>,
    segments: HashMap<&'static str, Vec<Segment>>,
}

fn segments(rules: &[Rule]) -> Vec<Segment> {
    let many = |ixs: Vec<usize>| {
        if ixs.is_empty() {
            return None;
        }
        let patterns: Vec<&str> = ixs.iter().map(|&i| rules[i].re.as_str()).collect();
        Meta::new_many(&patterns).ok().map(|re| (re, ixs))
    };
    let mut out = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    let flush = |run: &mut Vec<usize>, out: &mut Vec<Segment>| {
        if run.is_empty() {
            return;
        }
        let ixs = std::mem::take(run);
        let elsewhere: Vec<usize> = ixs
            .iter()
            .copied()
            .filter(|&i| !rules[i].spec.sol)
            .collect();
        match (many(ixs.clone()), many(elsewhere.clone())) {
            (Some(at_sol), elsewhere_re) if elsewhere.is_empty() || elsewhere_re.is_some() => out
                .push(Segment::Fast {
                    at_sol: Some(at_sol),
                    elsewhere: elsewhere_re,
                }),
            // too big for one regex: one rule at a time
            _ => out.extend(ixs.into_iter().map(Segment::Slow)),
        }
    };
    for (i, rule) in rules.iter().enumerate() {
        if Meta::new(rule.re.as_str()).is_ok() {
            run.push(i);
        } else {
            flush(&mut run, &mut out);
            out.push(Segment::Slow(i));
        }
    }
    flush(&mut run, &mut out);
    out
}

#[derive(Clone)]
struct SimpleState {
    state: &'static str,
    stack: Vec<&'static str>,
    pending: Vec<(usize, Option<&'static str>)>,
    local: Option<(Local, Option<Regex>, Box<dyn ModeState>)>,
}

fn style(s: &'static str) -> String {
    s.replace('.', " ")
}

impl SimpleMode {
    pub fn new(name: &'static str, states: Vec<(&'static str, Vec<RuleSpec>)>) -> Self {
        let states = states
            .into_iter()
            .map(|(state, rules)| {
                let rules = rules
                    .into_iter()
                    .map(|spec| {
                        let flags = if spec.case_insensitive { "(?i)" } else { "" };
                        Rule {
                            re: Regex::new(&format!(
                                "{flags}^(?:{})",
                                super::js_pattern(spec.regex)
                            ))
                            .expect("mode regex"),
                            end: spec.local.as_ref().and_then(|l| l.end).map(|e| {
                                Regex::new(&format!("^(?:{})", super::js_pattern(e)))
                                    .expect("mode regex")
                            }),
                            spec,
                        }
                    })
                    .collect();
                (state, rules)
            })
            .collect::<HashMap<_, Vec<Rule>>>();
        let segments = states
            .iter()
            .map(|(state, rules)| (*state, segments(rules)))
            .collect();
        Self {
            name,
            states,
            segments,
        }
    }
}

impl Mode for SimpleMode {
    fn name(&self) -> &'static str {
        self.name
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(SimpleState {
            state: "start",
            stack: Vec::new(),
            pending: Vec::new(),
            local: None,
        })
    }

    fn inner_mode_name(&self, s: &dyn ModeState) -> &'static str {
        let s = state_ref::<SimpleState>(s);
        match &s.local {
            Some((local, _, inner)) => local.mode.inner_mode_name(&**inner),
            None => self.name,
        }
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<SimpleState>(st);
        if !s.pending.is_empty() {
            let (len, token) = s.pending.remove(0);
            stream.pos += len;
            return token.map(style);
        }
        if let Some((local, end, inner)) = s.local.as_mut() {
            if let Some(end) = end.as_ref()
                && stream.match_re(end, true).is_some()
            {
                let tok = local.end_token.map(style);
                s.local = None;
                return tok;
            }
            return local.mode.token(stream, &mut **inner);
        }
        let rules = self.states.get(s.state).map(Vec::as_slice).unwrap_or(&[]);
        let segments = self.segments.get(s.state).map(Vec::as_slice).unwrap_or(&[]);
        let sol = stream.sol();
        let mut found = None;
        for segment in segments {
            let rule = match segment {
                Segment::Fast { at_sol, elsewhere } => {
                    let Some((re, ixs)) = (if sol { at_sol } else { elsewhere }) else {
                        continue;
                    };
                    let rest = stream.rest();
                    let Some(hit) = re.search(&Input::new(&*rest).anchored(Anchored::Yes)) else {
                        continue;
                    };
                    &rules[ixs[hit.pattern().as_usize()]]
                }
                Segment::Slow(ix) => {
                    let rule = &rules[*ix];
                    if rule.spec.sol && !sol {
                        continue;
                    }
                    rule
                }
            };
            if let Some(m) = stream.match_re(&rule.re, true) {
                found = Some((rule, m));
                break;
            }
        }
        if let Some((rule, m)) = found {
            if let Some(next) = rule.spec.next {
                s.state = next;
            } else if let Some(push) = rule.spec.push {
                s.stack.push(s.state);
                s.state = push;
            } else if rule.spec.pop
                && let Some(prev) = s.stack.pop()
            {
                s.state = prev;
            }
            if let Some(local) = &rule.spec.local {
                let inner = local.mode.start_state();
                s.local = Some((local.clone(), rule.end.clone(), inner));
            }
            return match &rule.spec.token {
                Token::None => None,
                Token::One(t) => Some(style(t)),
                Token::Groups(tokens) => {
                    // `matches.length > 2`: the first group is returned now,
                    // the others queued as pending tokens
                    if m.groups.len() > 1 {
                        for (j, g) in m.groups.iter().enumerate().skip(1) {
                            if let Some(g) = g.as_ref().filter(|g| !g.is_empty()) {
                                s.pending
                                    .push((super::js_len(g), tokens.get(j).copied().flatten()));
                            }
                        }
                        let first = m.group(1).map_or(0, super::js_len);
                        stream.back_up(super::js_len(&m.text) - first);
                    }
                    tokens.first().copied().flatten().map(style)
                }
            };
        }
        stream.next();
        None
    }
}
