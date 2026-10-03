//! `codemirror/mode/smalltalk/smalltalk.js` (`text/x-stsrc`), ported line
//! by line. GHD maps `.st` to it.
//!
//! The JS `Context` linked list (`next` tokenizer + `parent`) becomes a
//! stack of tokenizer kinds; the indentation bookkeeping (`userIndent`,
//! `state.indentation`) only feeds `indent` and is left out.

use crate::cm::{Mode, ModeState, StringStream, state};

pub struct Smalltalk;

#[derive(Clone, Copy, PartialEq)]
enum Tokenizer {
    /// `next`
    Base,
    /// `nextComment`
    Comment,
    /// `nextString`
    Str,
    /// `nextSymbol`
    Symbol,
    /// `nextTemporaries`
    Temporaries,
}

#[derive(Clone)]
struct StState {
    /// `state.context` chain, innermost last
    context: Vec<Tokenizer>,
    expect_variable: bool,
}

/// `Token`: the style and whether the statement ended (`eos`); the new
/// context is applied to the stack directly.
struct Token {
    name: Option<&'static str>,
    eos: bool,
}

/// `specialChars = /[+\-\/\\*~<>=@%|&?!.,:;^]/`
fn is_special(c: char) -> bool {
    matches!(
        c,
        '+' | '-'
            | '/'
            | '\\'
            | '*'
            | '~'
            | '<'
            | '>'
            | '='
            | '@'
            | '%'
            | '|'
            | '&'
            | '?'
            | '!'
            | '.'
            | ','
            | ':'
            | ';'
            | '^'
    )
}

/// `keywords = /true|false|nil|self|super|thisContext/` (unanchored `test`)
fn is_keyword(w: &str) -> bool {
    ["true", "false", "nil", "self", "super", "thisContext"]
        .iter()
        .any(|k| w.contains(k))
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

// next
fn next(stream: &mut StringStream, s: &mut StState) -> Token {
    let mut token = Token {
        name: None,
        eos: false,
    };
    let Some(a_char) = stream.next() else {
        return token;
    };
    if a_char == '"' {
        s.context.push(Tokenizer::Comment);
        token = next_comment(stream, s);
    } else if a_char == '\'' {
        s.context.push(Tokenizer::Str);
        token = next_string(stream, s);
    } else if a_char == '#' {
        if stream.peek() == Some('\'') {
            stream.next();
            s.context.push(Tokenizer::Symbol);
            token = next_symbol(stream, s);
        } else if stream.eat_while_if(|c| {
            !(c.is_whitespace() || matches!(c, '.' | '{' | '}' | '[' | ']' | '(' | ')'))
        }) {
            token.name = Some("string-2");
        } else {
            token.name = Some("meta");
        }
    } else if a_char == '$' {
        if stream.next() == Some('<') {
            stream.eat_while_if(|c| !(c.is_whitespace() || c == '>'));
            stream.next();
        }
        token.name = Some("string-2");
    } else if a_char == '|' && s.expect_variable {
        s.context.push(Tokenizer::Temporaries);
    } else if matches!(a_char, '[' | ']' | '{' | '}' | '(' | ')') {
        token.name = Some("bracket");
        token.eos = matches!(a_char, '[' | '{' | '(');
    } else if is_special(a_char) {
        stream.eat_while_if(is_special);
        token.name = Some("operator");
        token.eos = a_char != ';'; // ; cascaded message expression
    } else if a_char.is_ascii_digit() {
        stream.eat_while_if(is_word);
        token.name = Some("number");
    } else if is_word(a_char) {
        stream.eat_while_if(is_word);
        token.name = if s.expect_variable {
            Some(if is_keyword(stream.current()) {
                "keyword"
            } else {
                "variable"
            })
        } else {
            None
        };
    } else {
        token.eos = s.expect_variable;
    }
    token
}

/// Pop the innermost context when the construct closed (`context.parent`).
fn close(s: &mut StState, closed: bool) {
    if closed {
        s.context.pop();
    }
}

// nextComment
fn next_comment(stream: &mut StringStream, s: &mut StState) -> Token {
    stream.eat_while_if(|c| c != '"');
    let closed = stream.eat('"').is_some();
    close(s, closed);
    Token {
        name: Some("comment"),
        eos: true,
    }
}

// nextString
fn next_string(stream: &mut StringStream, s: &mut StState) -> Token {
    stream.eat_while_if(|c| c != '\'');
    let closed = stream.eat('\'').is_some();
    close(s, closed);
    Token {
        name: Some("string"),
        eos: false,
    }
}

// nextSymbol
fn next_symbol(stream: &mut StringStream, s: &mut StState) -> Token {
    stream.eat_while_if(|c| c != '\'');
    let closed = stream.eat('\'').is_some();
    close(s, closed);
    Token {
        name: Some("string-2"),
        eos: false,
    }
}

// nextTemporaries
fn next_temporaries(stream: &mut StringStream, s: &mut StState) -> Token {
    let mut token = Token {
        name: None,
        eos: false,
    };
    if stream.next() == Some('|') {
        s.context.pop();
        token.eos = true;
    } else {
        stream.eat_while_if(|c| c != '|');
        token.name = Some("variable");
    }
    token
}

impl Mode for Smalltalk {
    fn name(&self) -> &'static str {
        "smalltalk"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(StState {
            context: vec![Tokenizer::Base],
            expect_variable: true,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<StState>(st);
        if stream.eat_space() {
            return None;
        }
        let token = match s.context.last().copied().unwrap_or(Tokenizer::Base) {
            Tokenizer::Base => next(stream, s),
            Tokenizer::Comment => next_comment(stream, s),
            Tokenizer::Str => next_string(stream, s),
            Tokenizer::Symbol => next_symbol(stream, s),
            Tokenizer::Temporaries => next_temporaries(stream, s),
        };
        s.expect_variable = token.eos;
        token.name.map(Into::into)
    }
}
