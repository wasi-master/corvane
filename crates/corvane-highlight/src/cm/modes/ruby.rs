//! `codemirror/mode/ruby/ruby.js` (`text/x-ruby`).
//!
//! Ported line by line; the JS function names are kept on the Rust ones.
//! `state.tokenize` is a stack of closures in JS; here it is a stack of
//! [`Tok`] values carrying the closures' captured variables. The context
//! stack is kept because one of its entries (`read-quoted-paused`) decides
//! whether a `}` closes a `%Q{…}` literal. GHD passes no `indentUnit`, so
//! the root context's `indented` is `-undefined` (`NaN`), like a paused
//! context's missing one.

use super::super::{Mode, ModeState, StringStream, state, state_ref};
use crate::re;

const KEYWORD_LIST: &[&str] = &[
    "alias",
    "and",
    "BEGIN",
    "begin",
    "break",
    "case",
    "class",
    "def",
    "defined?",
    "do",
    "else",
    "elsif",
    "END",
    "end",
    "ensure",
    "false",
    "for",
    "if",
    "in",
    "module",
    "next",
    "not",
    "or",
    "redo",
    "rescue",
    "retry",
    "return",
    "self",
    "super",
    "then",
    "true",
    "undef",
    "unless",
    "until",
    "when",
    "while",
    "yield",
    "nil",
    "raise",
    "throw",
    "catch",
    "fail",
    "loop",
    "callcc",
    "caller",
    "lambda",
    "proc",
    "public",
    "protected",
    "private",
    "require",
    "load",
    "require_relative",
    "extend",
    "autoload",
    "__END__",
    "__FILE__",
    "__LINE__",
    "__dir__",
];

const INDENT_WORDS: &[&str] = &[
    "def", "class", "case", "for", "while", "until", "module", "catch", "loop", "proc", "begin",
];
const DEDENT_WORDS: &[&str] = &["end", "until"];

/// `opening[delim]`
fn opening(c: char) -> Option<char> {
    match c {
        '[' => Some(']'),
        '{' => Some('}'),
        '(' => Some(')'),
        _ => None,
    }
}

/// A `state.tokenize` entry.
#[derive(Clone)]
enum Tok {
    Base,
    /// `readQuoted(quote, style, embed, unescaped)`
    Quoted {
        quote: char,
        style: &'static str,
        embed: bool,
        unescaped: bool,
    },
    /// `tokenBaseUntilBrace(depth)`
    UntilBrace(u32),
    /// `tokenBaseOnce()` (`alreadyCalled`)
    Once(bool),
    /// `readHereDoc(phrase, mayIndent)`
    HereDoc {
        phrase: String,
        may_indent: bool,
    },
    /// `readBlockComment`
    BlockComment,
}

#[derive(Clone, Copy)]
struct Context {
    paused: bool,
    indented: f64,
}

#[derive(Clone)]
struct RubyState {
    tokenize: Vec<Tok>,
    indented: usize,
    /// innermost last; `prev` exists while there is more than one
    context: Vec<Context>,
    last_tok: Option<&'static str>,
    var_list: bool,
    /// the mode-level `curPunc`
    cur_punc: Option<&'static str>,
}

fn punc(c: char) -> &'static str {
    match c {
        '(' => "(",
        ')' => ")",
        '[' => "[",
        ']' => "]",
        '{' => "{",
        '}' => "}",
        '\\' => "\\",
        ';' => ";",
        '|' => "|",
        _ => ".",
    }
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `[\w\xa1-￿]`
fn is_word_or_wide(c: char) -> bool {
    is_word(c) || c >= '\u{a1}'
}

/// JS `\s`.
fn is_js_space(c: char) -> bool {
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

/// `stream.match(re)` without building the match.
fn match_re(stream: &mut StringStream, re: &fancy_regex::Regex) -> bool {
    let rest = stream.slice(stream.pos, stream.len());
    match re.find(rest) {
        Ok(Some(m)) if m.start() == 0 => {
            let n = rest[..m.end()].chars().count();
            stream.pos += n;
            true
        }
        _ => false,
    }
}

pub struct Ruby;

/// `state.tokenize.length` of a ruby state (haml's and slim's
/// `rubyInQuote` step out of ruby when it is back to 1).
pub fn tokenize_depth(st: &dyn ModeState) -> usize {
    state_ref::<RubyState>(st).tokenize.len()
}

/// `state.context.prev` is set (slim's `startRubySplat`).
pub fn has_context_prev(st: &dyn ModeState) -> bool {
    state_ref::<RubyState>(st).context.len() > 1
}

impl Ruby {
    /// `chain(newtok, stream, state)`
    fn chain(
        &self,
        tok: Tok,
        stream: &mut StringStream,
        state: &mut RubyState,
    ) -> Option<&'static str> {
        state.tokenize.push(tok);
        self.call_top(stream, state)
    }

    /// `state.tokenize[state.tokenize.length-1](stream, state)`
    fn call_top(&self, stream: &mut StringStream, state: &mut RubyState) -> Option<&'static str> {
        let ix = state.tokenize.len().saturating_sub(1);
        match small(state.tokenize.get(ix)) {
            Small::Base => self.token_base(stream, state),
            Small::Quoted {
                quote,
                style,
                embed,
                unescaped,
            } => self.read_quoted(quote, style, embed, unescaped, stream, state),
            Small::UntilBrace(depth) => self.token_base_until_brace(depth, stream, state),
            Small::Once(called) => self.token_base_once(called, stream, state),
            Small::HereDoc => read_here_doc(stream, state),
            Small::BlockComment => read_block_comment(stream, state),
        }
    }

    // tokenBase
    fn token_base(&self, stream: &mut StringStream, state: &mut RubyState) -> Option<&'static str> {
        if stream.sol() && stream.match_str("=begin", true, false) && stream.eol() {
            state.tokenize.push(Tok::BlockComment);
            return Some("comment");
        }
        if stream.eat_space() {
            return None;
        }
        let Some(ch) = stream.next() else {
            // JS tests `undefined` as a string: the identifier class matches it
            return Some("ident");
        };
        if ch == '`' || ch == '\'' || ch == '"' {
            return self.chain(
                Tok::Quoted {
                    quote: ch,
                    style: "string",
                    embed: ch == '"' || ch == '`',
                    unescaped: false,
                },
                stream,
                state,
            );
        } else if ch == '/' {
            if regexp_ahead(stream) {
                return self.chain(
                    Tok::Quoted {
                        quote: ch,
                        style: "string-2",
                        embed: true,
                        unescaped: false,
                    },
                    stream,
                    state,
                );
            } else {
                return Some("operator");
            }
        } else if ch == '%' {
            let mut style = "string";
            let mut embed = true;
            if stream.eat('s').is_some() {
                style = "atom";
            } else if stream.eat_if(|c| c == 'W' || c == 'Q').is_some() {
                style = "string";
            } else if stream.eat('r').is_some() {
                style = "string-2";
            } else if stream.eat_if(|c| matches!(c, 'w' | 'x' | 'q')).is_some() {
                style = "string";
                embed = false;
            }
            let Some(delim) = stream.eat_if(|c| !is_word(c) && !is_js_space(c) && c != '=') else {
                return Some("operator");
            };
            let delim = opening(delim).unwrap_or(delim);
            return self.chain(
                Tok::Quoted {
                    quote: delim,
                    style,
                    embed,
                    unescaped: true,
                },
                stream,
                state,
            );
        } else if ch == '#' {
            stream.skip_to_end();
            return Some("comment");
        } else if ch == '<'
            && let Some(m) = stream.match_re(
                re!(r#"^<([-~])[`"']?([a-zA-Z_?][A-Za-z0-9_]*)[`"']?(?:;|$)"#),
                true,
            )
        {
            let phrase = m.group(2).unwrap_or_default().to_string();
            let may_indent = m.group(1).is_some_and(|g| !g.is_empty());
            return self.chain(Tok::HereDoc { phrase, may_indent }, stream, state);
        } else if ch == '0' {
            if stream.eat('x').is_some() {
                stream.eat_while_if(|c| c.is_ascii_hexdigit());
            } else if stream.eat('b').is_some() {
                stream.eat_while_if(|c| c == '0' || c == '1');
            } else {
                stream.eat_while_if(|c| ('0'..='7').contains(&c));
            }
            return Some("number");
        } else if ch.is_ascii_digit() {
            match_re(stream, re!(r"^[0-9_]*(?:\.[0-9_]+)?(?:[eE][+\-]?[0-9_]+)?"));
            return Some("number");
        } else if ch == '?' {
            while stream.match_str("\\C-", true, false) || stream.match_str("\\M-", true, false) {}
            if stream.eat('\\').is_some() {
                stream.eat_while_if(is_word);
            } else {
                stream.next();
            }
            return Some("string");
        } else if ch == ':' {
            if stream.eat('\'').is_some() {
                return self.chain(
                    Tok::Quoted {
                        quote: '\'',
                        style: "atom",
                        embed: false,
                        unescaped: false,
                    },
                    stream,
                    state,
                );
            }
            if stream.eat('"').is_some() {
                return self.chain(
                    Tok::Quoted {
                        quote: '"',
                        style: "atom",
                        embed: true,
                        unescaped: false,
                    },
                    stream,
                    state,
                );
            }

            // :> :>> :< :<< are valid symbols
            if stream.eat_if(|c| c == '<' || c == '>').is_some() {
                stream.eat_if(|c| c == '<' || c == '>');
                return Some("atom");
            }

            // :+ :- :/ :* :| :& :! are valid symbols
            if stream
                .eat_if(|c| matches!(c, '+' | '-' | '*' | '/' | '&' | '|' | ':' | '!'))
                .is_some()
            {
                return Some("atom");
            }

            // Symbols can't start by a digit
            if stream
                .eat_if(|c| {
                    c.is_ascii_alphabetic() || c == '$' || c == '@' || c == '_' || c >= '\u{a1}'
                })
                .is_some()
            {
                stream.eat_while_if(|c| is_word_or_wide(c) || c == '$');
                // Only one ? ! = is allowed and only as the last character
                stream.eat_if(|c| matches!(c, '?' | '!' | '='));
                return Some("atom");
            }
            return Some("operator");
        } else if ch == '@' && match_at_ident(stream) {
            stream.eat('@');
            stream.eat_while_if(is_word_or_wide);
            return Some("variable-2");
        } else if ch == '$' {
            if stream
                .eat_if(|c| c.is_ascii_alphabetic() || c == '_')
                .is_some()
            {
                stream.eat_while_if(is_word);
            } else if stream.eat_if(|c| c.is_ascii_digit()).is_some() {
                stream.eat_if(|c| c.is_ascii_digit());
            } else {
                stream.next(); // Must be a special global like $: or $!
            }
            return Some("variable-3");
        } else if ch.is_ascii_alphabetic() || ch == '_' || ch >= '\u{a1}' {
            stream.eat_while_if(is_word_or_wide);
            stream.eat_if(|c| c == '?' || c == '!');
            if stream.eat(':').is_some() {
                return Some("atom");
            }
            return Some("ident");
        } else if ch == '|'
            && (state.var_list || state.last_tok == Some("{") || state.last_tok == Some("do"))
        {
            state.cur_punc = Some("|");
            return None;
        } else if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '\\' | ';') {
            state.cur_punc = Some(punc(ch));
            return None;
        } else if ch == '-' && stream.eat('>').is_some() {
            return Some("arrow");
        } else if is_operator_char(ch) {
            let more = stream.eat_while_if(is_operator_char);
            if ch == '.' && !more {
                state.cur_punc = Some(".");
            }
            return Some("operator");
        }
        None
    }

    // tokenBaseUntilBrace(depth)
    fn token_base_until_brace(
        &self,
        depth: u32,
        stream: &mut StringStream,
        state: &mut RubyState,
    ) -> Option<&'static str> {
        let depth = depth.max(1);
        if stream.peek() == Some('}') {
            if depth == 1 {
                state.tokenize.pop();
                return self.call_top(stream, state);
            } else if let Some(top) = state.tokenize.last_mut() {
                *top = Tok::UntilBrace(depth - 1);
            }
        } else if stream.peek() == Some('{')
            && let Some(top) = state.tokenize.last_mut()
        {
            *top = Tok::UntilBrace(depth + 1);
        }
        self.token_base(stream, state)
    }

    // tokenBaseOnce()
    fn token_base_once(
        &self,
        already_called: bool,
        stream: &mut StringStream,
        state: &mut RubyState,
    ) -> Option<&'static str> {
        if already_called {
            state.tokenize.pop();
            return self.call_top(stream, state);
        }
        if let Some(top) = state.tokenize.last_mut() {
            *top = Tok::Once(true);
        }
        self.token_base(stream, state)
    }

    // readQuoted(quote, style, embed, unescaped)
    fn read_quoted(
        &self,
        quote: char,
        style: &'static str,
        embed: bool,
        unescaped: bool,
        stream: &mut StringStream,
        state: &mut RubyState,
    ) -> Option<&'static str> {
        let mut escaped = false;

        if state.context.last().is_some_and(|c| c.paused) {
            state.context.pop();
            stream.eat('}');
        }

        while let Some(ch) = stream.next() {
            if ch == quote && (unescaped || !escaped) {
                state.tokenize.pop();
                break;
            }
            if embed && ch == '#' && !escaped {
                if stream.eat('{').is_some() {
                    if quote == '}' {
                        state.context.push(Context {
                            paused: true,
                            indented: f64::NAN,
                        });
                    }
                    state.tokenize.push(Tok::UntilBrace(1));
                    break;
                } else if matches!(stream.peek(), Some('@' | '$')) {
                    state.tokenize.push(Tok::Once(false));
                    break;
                }
            }
            escaped = !escaped && ch == '\\';
        }
        Some(style)
    }
}

/// `/[=+\-\/*:\.^%<>~|]/`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '=' | '+' | '-' | '/' | '*' | ':' | '.' | '^' | '%' | '<' | '>' | '~' | '|'
    )
}

/// `stream.match(/^@?[a-zA-Z_\xa1-\uffff]/)`
fn match_at_ident(stream: &mut StringStream) -> bool {
    let start =
        |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c >= '\u{a1}');
    let n = if stream.peek() == Some('@') { 1 } else { 0 };
    if start(stream.char_at(stream.pos + n)) {
        stream.pos += n + 1;
        true
    } else {
        false
    }
}

// regexpAhead
fn regexp_ahead(stream: &mut StringStream) -> bool {
    let start = stream.pos;
    let mut depth: i32 = 0;
    let mut found = false;
    let mut escaped = false;
    while let Some(next) = stream.next() {
        if !escaped {
            if "[{(".contains(next) {
                depth += 1;
            } else if "]})".contains(next) {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            } else if next == '/' && depth == 0 {
                found = true;
                break;
            }
            escaped = next == '\\';
        } else {
            escaped = false;
        }
    }
    stream.back_up(stream.pos - start);
    found
}

// readHereDoc(phrase, mayIndent)
fn read_here_doc(stream: &mut StringStream, state: &mut RubyState) -> Option<&'static str> {
    let Some(Tok::HereDoc { phrase, may_indent }) = state.tokenize.last() else {
        return None;
    };
    if *may_indent {
        stream.eat_space();
    }
    if stream.match_str(phrase, true, false) {
        state.tokenize.pop();
    } else {
        stream.skip_to_end();
    }
    Some("string")
}

// readBlockComment
fn read_block_comment(stream: &mut StringStream, state: &mut RubyState) -> Option<&'static str> {
    if stream.sol() && stream.match_str("=end", true, false) && stream.eol() {
        state.tokenize.pop();
    }
    stream.skip_to_end();
    Some("comment")
}

/// A `Tok` without its heap parts, so the dispatcher can release the borrow.
enum Small {
    Base,
    Quoted {
        quote: char,
        style: &'static str,
        embed: bool,
        unescaped: bool,
    },
    UntilBrace(u32),
    Once(bool),
    HereDoc,
    BlockComment,
}

fn small(tok: Option<&Tok>) -> Small {
    match tok {
        None | Some(Tok::Base) => Small::Base,
        Some(Tok::Quoted {
            quote,
            style,
            embed,
            unescaped,
        }) => Small::Quoted {
            quote: *quote,
            style,
            embed: *embed,
            unescaped: *unescaped,
        },
        Some(Tok::UntilBrace(d)) => Small::UntilBrace(*d),
        Some(Tok::Once(c)) => Small::Once(*c),
        Some(Tok::HereDoc { .. }) => Small::HereDoc,
        Some(Tok::BlockComment) => Small::BlockComment,
    }
}

impl Mode for Ruby {
    fn name(&self) -> &'static str {
        "ruby"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(RubyState {
            tokenize: vec![Tok::Base],
            indented: 0,
            // {type: "top", indented: -config.indentUnit}
            context: vec![Context {
                paused: false,
                indented: f64::NAN,
            }],
            last_tok: None,
            var_list: false,
            cur_punc: None,
        })
    }

    // the `kwtype` chain mirrors the JS branches one to one
    #[allow(clippy::if_same_then_else)]
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<RubyState>(st);
        state.cur_punc = None;
        if stream.sol() {
            state.indented = stream.indentation();
        }
        let mut style = self.call_top(stream, state);
        let mut kwtype = None;
        let mut this_tok = state.cur_punc;
        if style == Some("ident") {
            let word = stream.current();
            let keyword = KEYWORD_LIST.iter().find(|k| **k == word).copied();
            style = Some(if state.last_tok == Some(".") {
                "property"
            } else if keyword.is_some() {
                "keyword"
            } else if word.starts_with(|c: char| c.is_ascii_uppercase()) {
                "tag"
            } else if matches!(state.last_tok, Some("def" | "class")) || state.var_list {
                "def"
            } else {
                "variable"
            });
            if style == Some("keyword")
                && let Some(word) = keyword
            {
                this_tok = Some(word);
                if INDENT_WORDS.contains(&word) {
                    kwtype = Some("indent");
                } else if DEDENT_WORDS.contains(&word) {
                    kwtype = Some("dedent");
                } else if (word == "if" || word == "unless")
                    && stream.column() == stream.indentation()
                {
                    kwtype = Some("indent");
                } else if word == "do"
                    && state
                        .context
                        .last()
                        .is_some_and(|c| c.indented < state.indented as f64)
                {
                    kwtype = Some("indent");
                }
            }
        }
        let cur_punc = state.cur_punc;
        if cur_punc.is_some() || style.is_some_and(|s| s != "comment") {
            state.last_tok = this_tok;
        }
        if cur_punc == Some("|") {
            state.var_list = !state.var_list;
        }

        if kwtype == Some("indent") || matches!(cur_punc, Some("(" | "[" | "{")) {
            state.context.push(Context {
                paused: false,
                indented: state.indented as f64,
            });
        } else if (kwtype == Some("dedent") || matches!(cur_punc, Some(")" | "]" | "}")))
            && state.context.len() > 1
        {
            state.context.pop();
        }

        style.map(str::to_string)
    }
}
