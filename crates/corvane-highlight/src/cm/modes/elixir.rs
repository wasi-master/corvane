//! `codemirror-mode-elixir` (`dist/codemirror-mode-elixir.m.js`,
//! `text/x-elixir`): a fork of CodeMirror's ruby mode. The package ships
//! only minified code, so the names below are those of the ruby mode it
//! derives from (`tokenBase`, `readQuoted`, `tokenBaseUntilBrace`,
//! `tokenBaseOnce`, `readHereDoc`) plus `readDocComment` for the `"""`
//! block. Differences from ruby.js that the minified code has and this port
//! keeps: `"""` alone on a line (at column 0) starts a comment block, `#{`
//! stays inside the string token, and only a `.` swallows the operator
//! chars after it.
//!
//! `state.tokenize` is a stack of closures in JS; here it is a stack of
//! [`Tok`] values carrying the closures' captured variables. The context
//! stack only matters for its `read-quoted-paused` entries (whether a `}`
//! closes a `%w{…}` literal), so it is a stack of that flag. The
//! indentation bookkeeping never changes a token: `do` only indents when
//! `context.indented < state.indented`, and GHD's `indentUnit` is
//! `undefined`, which makes the root `indented` `NaN` and every other one
//! `0`, so that test is always false and is left out.

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

const KEYWORD_LIST: &[&str] = &[
    "alias",
    "case",
    "cond",
    "def",
    "defmodule",
    "defp",
    "defstruct",
    "defprotocol",
    "defimpl",
    "defmacro",
    "quote",
    "unquote",
    "receive",
    "fn",
    "do",
    "else",
    "else if",
    "end",
    "false",
    "if",
    "in",
    "next",
    "rescue",
    "for",
    "true",
    "unless",
    "when",
    "nil",
    "raise",
    "throw",
    "try",
    "catch",
    "after",
    "with",
    "require",
    "use",
    "__MODULE__",
    "__FILE__",
    "__DIR__",
    "__ENV__",
    "__CALLER__",
];

const INDENT_WORDS: &[&str] = &[
    "def",
    "defmodule",
    "defp",
    "case",
    "cond",
    "rescue",
    "try",
    "catch",
    "->",
];
const DEDENT_WORDS: &[&str] = &["end"];

/// `opening[delim]` (the `{"[": "]", "{": "}", "(": ")"}` map)
fn opening(c: char) -> Option<char> {
    match c {
        '[' => Some(']'),
        '{' => Some('}'),
        '(' => Some(')'),
        _ => None,
    }
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `[\xa1-￿]`: every non-ASCII UTF-16 unit from U+00A1, surrogate
/// halves of astral chars included.
fn is_high(c: char) -> bool {
    c >= '\u{a1}'
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
    /// `readHereDoc(phrase)`
    HereDoc(String),
    /// `readDocComment`: inside a `"""` block
    DocComment,
}

#[derive(Clone)]
struct ElixirState {
    tokenize: Vec<Tok>,
    /// the context stack: `true` for `read-quoted-paused`
    context: Vec<bool>,
    last_tok: Option<&'static str>,
    var_list: bool,
    /// the mode-wide `curPunc`
    cur_punc: Option<&'static str>,
}

pub struct Elixir;

/// `chain(newtok, stream, state)`
fn chain(tok: Tok, stream: &mut StringStream, st: &mut ElixirState) -> Option<&'static str> {
    st.tokenize.push(tok);
    call_top(stream, st)
}

/// `state.tokenize[state.tokenize.length - 1](stream, state)`
fn call_top(stream: &mut StringStream, st: &mut ElixirState) -> Option<&'static str> {
    match st.tokenize.last().cloned().unwrap_or(Tok::Base) {
        Tok::Base => token_base(stream, st),
        Tok::Quoted {
            quote,
            style,
            embed,
            unescaped,
        } => read_quoted(stream, st, quote, style, embed, unescaped),
        Tok::UntilBrace(depth) => token_base_until_brace(stream, st, depth),
        Tok::Once(called) => token_base_once(stream, st, called),
        Tok::HereDoc(phrase) => read_here_doc(stream, st, &phrase),
        Tok::DocComment => read_doc_comment(stream, st),
    }
}

fn set_top(st: &mut ElixirState, tok: Tok) {
    if let Some(top) = st.tokenize.last_mut() {
        *top = tok;
    }
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
        _ => ";",
    }
}

fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '=' | '+' | '-' | '/' | '*' | ':' | '.' | '^' | '%' | '<' | '>' | '~' | '|'
    )
}

/// `tokenBase`
fn token_base(stream: &mut StringStream, st: &mut ElixirState) -> Option<&'static str> {
    if stream.sol() && stream.match_str("\"\"\"", true, false) && stream.eol() {
        st.tokenize.push(Tok::DocComment);
        return Some("comment");
    }
    if stream.eat_space() {
        return None;
    }
    let ch = stream.next()?;
    if ch == '\'' || ch == '"' {
        return chain(
            Tok::Quoted {
                quote: ch,
                style: "string",
                embed: ch == '"',
                unescaped: false,
            },
            stream,
            st,
        );
    }
    if ch == '/' {
        let current_index = stream.pos - stream.start;
        if stream.skip_to('/') {
            let search_till = stream.pos - stream.start;
            let mut balance: i32 = 0;
            stream.back_up(stream.pos - stream.start - current_index);
            while stream.pos - stream.start < search_till {
                let next_ch = stream.next();
                if next_ch == Some('(') {
                    balance += 1;
                } else if next_ch == Some(')') {
                    balance -= 1;
                }
                if balance < 0 {
                    break;
                }
            }
            stream.back_up(stream.pos - stream.start - current_index);
            if balance == 0 {
                return chain(
                    Tok::Quoted {
                        quote: ch,
                        style: "string-2",
                        embed: true,
                        unescaped: false,
                    },
                    stream,
                    st,
                );
            }
        }
        return Some("operator");
    }
    if ch == '%' {
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
        return match stream.eat_if(|c| !is_word(c) && !c.is_whitespace() && c != '=') {
            Some(delim) => chain(
                Tok::Quoted {
                    quote: opening(delim).unwrap_or(delim),
                    style,
                    embed,
                    unescaped: true,
                },
                stream,
                st,
            ),
            None => Some("operator"),
        };
    }
    if ch == '#' {
        stream.skip_to_end();
        return Some("comment");
    }
    if ch == '<'
        && let Some(m) = stream.match_re(
            re!(r#"^<-?[`"']?([a-zA-Z_?][A-Za-z0-9_]*)[`"']?(?:;|$)"#),
            true,
        )
    {
        let phrase = m.group(1).unwrap_or_default().to_string();
        return chain(Tok::HereDoc(phrase), stream, st);
    }
    if ch == '0' {
        if stream.eat('x').is_some() {
            stream.eat_while_if(|c| c.is_ascii_hexdigit());
        } else if stream.eat('b').is_some() {
            stream.eat_while_if(|c| c == '0' || c == '1');
        } else {
            stream.eat_while_if(|c| ('0'..='7').contains(&c));
        }
        return Some("number");
    }
    if ch.is_ascii_digit() {
        stream.matches(re!(r"^[0-9_]*(?:\.[0-9_]+)?(?:[eE][+\-]?[0-9_]+)?"));
        return Some("number");
    }
    if ch == '?' {
        while stream.matches(re!(r"^\\[CM]-")) {}
        if stream.eat('\\').is_some() {
            stream.eat_while_if(is_word);
        } else {
            stream.next();
        }
        return Some("string");
    }
    if ch == ':' {
        if stream.eat('\'').is_some() {
            return chain(
                Tok::Quoted {
                    quote: '\'',
                    style: "atom",
                    embed: false,
                    unescaped: false,
                },
                stream,
                st,
            );
        }
        if stream.eat('"').is_some() {
            return chain(
                Tok::Quoted {
                    quote: '"',
                    style: "atom",
                    embed: true,
                    unescaped: false,
                },
                stream,
                st,
            );
        }
        if stream.eat_if(|c| c == '<' || c == '>').is_some() {
            stream.eat_if(|c| c == '<' || c == '>');
            return Some("atom");
        }
        if stream
            .eat_if(|c| matches!(c, '+' | '-' | '*' | '/' | '&' | '|' | ':' | '!'))
            .is_some()
        {
            return Some("atom");
        }
        if stream
            .eat_if(|c| c.is_ascii_alphabetic() || matches!(c, '$' | '@' | '_') || is_high(c))
            .is_some()
        {
            stream.eat_while_if(|c| is_word(c) || c == '$' || is_high(c));
            stream.eat_if(|c| matches!(c, '?' | '!' | '='));
            return Some("atom");
        }
        return Some("operator");
    }
    if ch == '@' && stream.matches(re!(r"^@?[a-zA-Z_\x{a1}-\x{10ffff}]")) {
        stream.eat('@');
        stream.eat_while_if(|c| is_word(c) || is_high(c));
        return Some("variable-2");
    }
    if ch == '$' {
        if stream
            .eat_if(|c| c.is_ascii_alphabetic() || c == '_')
            .is_some()
        {
            stream.eat_while_if(is_word);
        } else if stream.eat_if(|c| c.is_ascii_digit()).is_some() {
            stream.eat_if(|c| c.is_ascii_digit());
        } else {
            stream.next();
        }
        return Some("variable-3");
    }
    if ch.is_ascii_alphabetic() || ch == '_' || is_high(ch) {
        stream.eat_while_if(|c| is_word(c) || is_high(c));
        stream.eat_if(|c| c == '?' || c == '!');
        if stream.eat(':').is_some() {
            return Some("atom");
        }
        return Some("ident");
    }
    if ch == '|' && (st.var_list || st.last_tok == Some("{") || st.last_tok == Some("do")) {
        st.cur_punc = Some("|");
        return None;
    }
    if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '\\' | ';') {
        st.cur_punc = Some(punc(ch));
        return None;
    }
    if ch == '-' && stream.eat('>').is_some() {
        return Some("arrow");
    }
    if ch == '|' && stream.eat('>').is_some() {
        return Some("pipe");
    }
    if is_operator_char(ch) {
        // only a `.` takes the operator chars after it
        if ch == '.' && !stream.eat_while_if(is_operator_char) {
            st.cur_punc = Some(".");
        }
        return Some("operator");
    }
    None
}

/// `tokenBaseUntilBrace(depth)`
fn token_base_until_brace(
    stream: &mut StringStream,
    st: &mut ElixirState,
    depth: u32,
) -> Option<&'static str> {
    match stream.peek() {
        Some('}') => {
            if depth == 1 {
                st.tokenize.pop();
                return call_top(stream, st);
            }
            set_top(st, Tok::UntilBrace(depth - 1));
        }
        Some('{') => set_top(st, Tok::UntilBrace(depth + 1)),
        _ => {}
    }
    token_base(stream, st)
}

/// `tokenBaseOnce()`
fn token_base_once(
    stream: &mut StringStream,
    st: &mut ElixirState,
    already_called: bool,
) -> Option<&'static str> {
    if already_called {
        st.tokenize.pop();
        return call_top(stream, st);
    }
    set_top(st, Tok::Once(true));
    token_base(stream, st)
}

/// `readQuoted(quote, style, embed, unescaped)`
fn read_quoted(
    stream: &mut StringStream,
    st: &mut ElixirState,
    quote: char,
    style: &'static str,
    embed: bool,
    unescaped: bool,
) -> Option<&'static str> {
    let mut escaped = false;
    if st.context.last() == Some(&true) {
        st.context.pop();
        stream.eat('}');
    }
    while let Some(ch) = stream.next() {
        if ch == quote && (unescaped || !escaped) {
            st.tokenize.pop();
            break;
        }
        if embed && ch == '#' && !escaped {
            if stream.eat('{').is_some() {
                if quote == '}' {
                    st.context.push(true);
                }
                st.tokenize.push(Tok::UntilBrace(1));
                break;
            }
            if matches!(stream.peek(), Some('@' | '$')) {
                st.tokenize.push(Tok::Once(false));
                break;
            }
        }
        escaped = !escaped && ch == '\\';
    }
    Some(style)
}

/// `readHereDoc(phrase)`
fn read_here_doc(
    stream: &mut StringStream,
    st: &mut ElixirState,
    phrase: &str,
) -> Option<&'static str> {
    if stream.match_str(phrase, true, false) {
        st.tokenize.pop();
    } else {
        stream.skip_to_end();
    }
    Some("string")
}

/// `readDocComment`: the `"""` block ends at a line that is only `"""`
fn read_doc_comment(stream: &mut StringStream, st: &mut ElixirState) -> Option<&'static str> {
    if stream.sol() && stream.match_str("\"\"\"", true, false) && stream.eol() {
        st.tokenize.pop();
    }
    stream.skip_to_end();
    Some("comment")
}

impl Mode for Elixir {
    fn name(&self) -> &'static str {
        "elixir"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(ElixirState {
            tokenize: vec![Tok::Base],
            context: vec![false],
            last_tok: None,
            var_list: false,
            cur_punc: None,
        })
    }

    fn token(&self, stream: &mut StringStream, s: &mut dyn ModeState) -> Option<String> {
        let st = state::<ElixirState>(s);
        st.cur_punc = None;
        let mut style = call_top(stream, st);
        let cur_punc = st.cur_punc;
        let mut kwtype = cur_punc;
        let mut indent = false;
        let mut dedent = false;
        if style == Some("ident") {
            let word = stream.current();
            let keyword = KEYWORD_LIST.iter().find(|k| **k == word).copied();
            style = Some(if st.last_tok == Some(".") {
                "property"
            } else if keyword.is_some() {
                "keyword"
            } else if word.starts_with(|c: char| c.is_ascii_uppercase()) {
                "tag"
            } else if st.last_tok == Some("def") || st.last_tok == Some("class") || st.var_list {
                "def"
            } else {
                "variable"
            });
            let first_on_line = stream.column() == stream.indentation();
            if style == Some("keyword")
                && let Some(word) = keyword
            {
                kwtype = Some(word);
                if INDENT_WORDS.contains(&word) {
                    indent = true;
                } else if DEDENT_WORDS.contains(&word) {
                    dedent = true;
                } else if (word == "if" || word == "unless") && first_on_line {
                    indent = true;
                }
            }
        }
        if cur_punc.is_some() || style.is_some_and(|s| s != "comment") {
            st.last_tok = kwtype;
        }
        if cur_punc == Some("|") {
            st.var_list = !st.var_list;
        }
        if indent || matches!(cur_punc, Some("(" | "[" | "{")) {
            st.context.push(false);
        } else if (dedent || matches!(cur_punc, Some(")" | "]" | "}"))) && st.context.len() > 1 {
            st.context.pop();
        }
        style.map(str::to_string)
    }
}
