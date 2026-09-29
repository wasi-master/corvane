//! `codemirror/mode/shell/shell.js` (`text/x-sh`, `application/x-sh`).
//!
//! Ported line by line; the JS function names are kept on the Rust ones.
//! `state.tokens` is a stack of closures in JS whose front (`tokens[0]`) is
//! the active one; here it is a `Vec` of [`Tok`] values whose *last* entry
//! is `tokens[0]`. The JS tokenizers end in tail calls to `tokenize`, which
//! [`Shell::tokenize`] runs as a loop.

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

/// `words`: `define('atom' | 'keyword' | 'builtin', …)`
fn word_style(word: &str) -> Option<&'static str> {
    Some(match word {
        "true" | "false" => "atom",
        "if" | "then" | "do" | "else" | "elif" | "while" | "until" | "for" | "in" | "esac"
        | "fi" | "fin" | "fil" | "done" | "exit" | "set" | "unset" | "export" | "function" => {
            "keyword"
        }
        "ab" | "awk" | "bash" | "beep" | "cat" | "cc" | "cd" | "chown" | "chmod" | "chroot"
        | "clear" | "cp" | "curl" | "cut" | "diff" | "echo" | "find" | "gawk" | "gcc" | "get"
        | "git" | "grep" | "hg" | "kill" | "killall" | "ln" | "ls" | "make" | "mkdir"
        | "openssl" | "mv" | "nc" | "nl" | "node" | "npm" | "ping" | "ps" | "restart" | "rm"
        | "rmdir" | "sed" | "service" | "sh" | "shopt" | "shred" | "source" | "sort" | "sleep"
        | "ssh" | "start" | "stop" | "su" | "sudo" | "svn" | "tee" | "telnet" | "top" | "touch"
        | "vi" | "vim" | "wall" | "wc" | "wget" | "who" | "write" | "yes" | "zsh" => "builtin",
        _ => return None,
    })
}

/// A `state.tokens` entry.
#[derive(Clone)]
enum Tok {
    /// `tokenString(quote, style)`
    Str { quote: char, style: &'static str },
    /// `tokenStringStart(quote, style)`
    StrStart { quote: char, style: &'static str },
    /// `tokenDollar`
    Dollar,
    /// `tokenHeredoc(delim)`
    Heredoc(String),
}

#[derive(Clone, Default)]
struct ShellState {
    tokens: Vec<Tok>,
}

/// A tokenizer's result: a style, or `return tokenize(stream, state)`.
enum Step {
    Done(Option<&'static str>),
    Tokenize,
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub struct Shell;

impl Shell {
    // tokenize
    fn tokenize(&self, stream: &mut StringStream, state: &mut ShellState) -> Option<&'static str> {
        loop {
            let step = match state.tokens.last() {
                None => token_base(stream, state),
                Some(Tok::Str { quote, style }) => {
                    let (quote, style) = (*quote, *style);
                    token_string(quote, style, stream, state)
                }
                Some(Tok::StrStart { quote, style }) => {
                    let (quote, style) = (*quote, *style);
                    token_string_start(quote, style, stream, state)
                }
                Some(Tok::Dollar) => token_dollar(stream, state),
                Some(Tok::Heredoc(_)) => token_heredoc(stream, state),
            };
            match step {
                Step::Done(style) => return style,
                Step::Tokenize => continue,
            }
        }
    }
}

// tokenBase
fn token_base(stream: &mut StringStream, state: &mut ShellState) -> Step {
    if stream.eat_space() {
        return Step::Done(None);
    }

    let sol = stream.sol();
    let Some(ch) = stream.next() else {
        return Step::Done(None);
    };

    if ch == '\\' {
        stream.next();
        return Step::Done(None);
    }
    if ch == '\'' || ch == '"' || ch == '`' {
        state.tokens.push(Tok::Str {
            quote: ch,
            style: if ch == '`' { "quote" } else { "string" },
        });
        return Step::Tokenize;
    }
    if ch == '#' {
        if sol && stream.eat('!').is_some() {
            stream.skip_to_end();
            return Step::Done(Some("meta")); // 'comment'?
        }
        stream.skip_to_end();
        return Step::Done(Some("comment"));
    }
    if ch == '$' {
        state.tokens.push(Tok::Dollar);
        return Step::Tokenize;
    }
    if ch == '+' || ch == '=' {
        return Step::Done(Some("operator"));
    }
    if ch == '-' {
        stream.eat('-');
        stream.eat_while_if(is_word);
        return Step::Done(Some("attribute"));
    }
    if ch == '<' {
        if stream.match_str("<<", true, false) {
            return Step::Done(Some("operator"));
        }
        if let Some(heredoc) = stream.match_re(re!(r#"^<-?\s*['"]?([^'"]*)['"]?"#), true) {
            let delim = heredoc.group(1).unwrap_or_default().to_string();
            state.tokens.push(Tok::Heredoc(delim));
            return Step::Done(Some("string-2"));
        }
    }
    if ch.is_ascii_digit() {
        stream.eat_while_if(|c| c.is_ascii_digit());
        if stream.eol() || !stream.peek().is_some_and(is_word) {
            return Step::Done(Some("number"));
        }
    }
    stream.eat_while_if(|c| is_word(c) || c == '-');
    let cur = stream.current();
    if stream.peek() == Some('=') && cur.chars().any(is_word) {
        return Step::Done(Some("def"));
    }
    Step::Done(word_style(cur))
}

// tokenString(quote, style)
fn token_string(
    quote: char,
    style: &'static str,
    stream: &mut StringStream,
    state: &mut ShellState,
) -> Step {
    let close = match quote {
        '(' => ')',
        '{' => '}',
        q => q,
    };
    let mut escaped = false;
    while let Some(next) = stream.next() {
        if next == close && !escaped {
            state.tokens.pop();
            break;
        } else if next == '$' && !escaped && quote != '\'' && stream.peek() != Some(close) {
            stream.back_up(1);
            state.tokens.push(Tok::Dollar);
            break;
        } else if !escaped && quote != close && next == quote {
            state.tokens.push(Tok::Str { quote, style });
            return Step::Tokenize;
        } else if !escaped && (next == '\'' || next == '"') && !(quote == '\'' || quote == '"') {
            state.tokens.push(Tok::StrStart {
                quote: next,
                style: "string",
            });
            stream.back_up(1);
            break;
        }
        escaped = !escaped && next == '\\';
    }
    Step::Done(Some(style))
}

// tokenStringStart(quote, style)
fn token_string_start(
    quote: char,
    style: &'static str,
    stream: &mut StringStream,
    state: &mut ShellState,
) -> Step {
    if let Some(top) = state.tokens.last_mut() {
        *top = Tok::Str { quote, style };
    }
    stream.next();
    Step::Tokenize
}

// tokenDollar
fn token_dollar(stream: &mut StringStream, state: &mut ShellState) -> Step {
    if state.tokens.len() > 1 {
        stream.eat('$');
    }
    let ch = stream.next();
    if let Some(ch @ ('\'' | '"' | '(' | '{')) = ch {
        if let Some(top) = state.tokens.last_mut() {
            *top = Tok::Str {
                quote: ch,
                style: match ch {
                    '(' => "quote",
                    '{' => "def",
                    _ => "string",
                },
            };
        }
        return Step::Tokenize;
    }
    if !ch.is_some_and(|c| c.is_ascii_digit()) {
        stream.eat_while_if(is_word);
    }
    state.tokens.pop();
    Step::Done(Some("def"))
}

// tokenHeredoc(delim)
fn token_heredoc(stream: &mut StringStream, state: &mut ShellState) -> Step {
    if stream.sol()
        && matches!(state.tokens.last(), Some(Tok::Heredoc(delim)) if stream.string() == delim)
    {
        state.tokens.pop();
    }
    stream.skip_to_end();
    Step::Done(Some("string-2"))
}

impl Mode for Shell {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(ShellState::default())
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<ShellState>(st);
        self.tokenize(stream, state).map(str::to_string)
    }
}
