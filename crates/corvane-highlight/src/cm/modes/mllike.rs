//! `codemirror/mode/mllike/mllike.js` (`text/x-ocaml`, `text/x-fsharp`,
//! `text/x-sml`).
//!
//! Ported line by line. The JS merges each MIME's `extraWords` into the
//! shared `words` table; here that is [`Dialect::word`]. Words containing
//! `!` (`do!`, `let!`, …) can never match since the tokenizer only reads
//! `[\w\xa1-￿]`, exactly as in the JS.

use crate::cm::{Mode, ModeState, StringStream, state};

#[derive(Clone, Copy, PartialEq)]
pub enum Dialect {
    OCaml,
    FSharp,
    Sml,
}

impl Dialect {
    /// `words[cur]` after merging `parserConfig.extraWords`.
    fn word(self, w: &str) -> Option<&'static str> {
        let extra = match self {
            Dialect::OCaml => match w {
                "and" | "assert" | "begin" | "class" | "constraint" | "done" | "downto"
                | "external" | "function" | "initializer" | "lazy" | "match" | "method"
                | "module" | "mutable" | "new" | "nonrec" | "object" | "private" | "sig" | "to"
                | "try" | "value" | "virtual" | "when" => Some("keyword"),
                "raise" | "failwith" | "true" | "false" | "asr" | "land" | "lor" | "lsl"
                | "lsr" | "lxor" | "mod" | "or" | "raise_notrace" | "trace" | "exit"
                | "print_string" | "print_endline" | "List" => Some("builtin"),
                "int" | "float" | "bool" | "char" | "string" | "unit" => Some("type"),
                _ => None,
            },
            Dialect::FSharp => match w {
                "abstract" | "assert" | "base" | "begin" | "class" | "default" | "delegate"
                | "done" | "downcast" | "downto" | "elif" | "extern" | "finally" | "for"
                | "function" | "global" | "inherit" | "inline" | "interface" | "internal"
                | "lazy" | "match" | "member" | "module" | "mutable" | "namespace" | "new"
                | "null" | "override" | "private" | "public" | "return" | "select" | "static"
                | "to" | "try" | "upcast" | "use" | "void" | "when" | "yield" | "atomic"
                | "break" | "checked" | "component" | "const" | "constraint" | "constructor"
                | "continue" | "eager" | "event" | "external" | "fixed" | "method" | "mixin"
                | "object" | "parallel" | "process" | "protected" | "pure" | "sealed"
                | "tailcall" | "trait" | "virtual" | "volatile" => Some("keyword"),
                "List" | "Seq" | "Map" | "Set" | "Option" | "int" | "string" | "not" | "true"
                | "false" | "raise" | "failwith" => Some("builtin"),
                _ => None,
            },
            Dialect::Sml => match w {
                "abstype" | "and" | "andalso" | "case" | "datatype" | "fn" | "handle" | "infix"
                | "infixr" | "local" | "nonfix" | "op" | "orelse" | "raise" | "withtype"
                | "eqtype" | "sharing" | "sig" | "signature" | "structure" | "where" | "true"
                | "false" => Some("keyword"),
                "int" | "real" | "string" | "char" | "bool" => Some("builtin"),
                _ => None,
            },
        };
        extra.or(match w {
            "as" | "do" | "else" | "end" | "exception" | "fun" | "functor" | "if" | "in"
            | "include" | "let" | "of" | "open" | "rec" | "struct" | "then" | "type" | "val"
            | "while" | "with" => Some("keyword"),
            _ => None,
        })
    }

    /// `parserConfig.slashComments`
    fn slash_comments(self) -> bool {
        self != Dialect::OCaml
    }
}

pub struct MlLike {
    dialect: Dialect,
}

impl MlLike {
    pub fn new(dialect: Dialect) -> Self {
        Self { dialect }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    Base,
    String,
    Comment,
    LongString,
}

#[derive(Clone)]
struct MlState {
    tokenize: Tokenize,
    comment_level: i32,
    long_string: bool,
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `[\w\xa1-￿]` (a char past the BMP is two surrogate units in JS,
/// both in the range)
fn is_word_or_wide(c: char) -> bool {
    is_word(c) || c >= '\u{a1}'
}

impl MlLike {
    /// `tokenBase`
    fn token_base(&self, stream: &mut StringStream, s: &mut MlState) -> Option<&'static str> {
        let ch = stream.next()?;
        if ch == '"' {
            s.tokenize = Tokenize::String;
            return token_string(stream, s);
        }
        if ch == '{' && stream.eat('|').is_some() {
            s.long_string = true;
            s.tokenize = Tokenize::LongString;
            return token_long_string(stream, s);
        }
        // stream.match(/^\*(?!\))/)
        if ch == '(' && stream.peek() == Some('*') && stream.char_at(stream.pos + 1) != Some(')') {
            stream.pos += 1;
            s.comment_level += 1;
            s.tokenize = Tokenize::Comment;
            return token_comment(stream, s);
        }
        if ch == '~' || ch == '?' {
            stream.eat_while_if(is_word);
            return Some("variable-2");
        }
        if ch == '`' {
            stream.eat_while_if(is_word);
            return Some("quote");
        }
        if ch == '/' && self.dialect.slash_comments() && stream.eat('/').is_some() {
            stream.skip_to_end();
            return Some("comment");
        }
        if ch.is_ascii_digit() {
            // the JS chains three `if`s; only the last has the `else`
            if ch == '0' && stream.eat_if(|c| c == 'b' || c == 'B').is_some() {
                stream.eat_while_if(|c| c == '0' || c == '1');
            }
            if ch == '0' && stream.eat_if(|c| c == 'x' || c == 'X').is_some() {
                stream.eat_while_if(|c| c.is_ascii_hexdigit());
            }
            if ch == '0' && stream.eat_if(|c| c == 'o' || c == 'O').is_some() {
                stream.eat_while_if(|c| ('0'..='7').contains(&c));
            } else {
                stream.eat_while_if(|c| c.is_ascii_digit() || c == '_');
                if stream.eat('.').is_some() {
                    stream.eat_while_if(|c| c.is_ascii_digit());
                }
                if stream.eat_if(|c| c == 'e' || c == 'E').is_some() {
                    stream.eat_while_if(|c| c.is_ascii_digit() || c == '-' || c == '+');
                }
            }
            return Some("number");
        }
        if matches!(
            ch,
            '+' | '-' | '*' | '&' | '%' | '=' | '<' | '>' | '!' | '?' | '|' | '@' | '.' | '~' | ':'
        ) {
            return Some("operator");
        }
        if is_word_or_wide(ch) {
            stream.eat_while_if(is_word_or_wide);
            return Some(self.dialect.word(stream.current()).unwrap_or("variable"));
        }
        None
    }
}

/// `tokenString`
fn token_string(stream: &mut StringStream, s: &mut MlState) -> Option<&'static str> {
    let mut end = false;
    let mut escaped = false;
    while let Some(next) = stream.next() {
        if next == '"' && !escaped {
            end = true;
            break;
        }
        escaped = !escaped && next == '\\';
    }
    if end && !escaped {
        s.tokenize = Tokenize::Base;
    }
    Some("string")
}

/// `tokenComment`
fn token_comment(stream: &mut StringStream, s: &mut MlState) -> Option<&'static str> {
    let mut prev: Option<char> = None;
    while s.comment_level > 0 {
        let Some(next) = stream.next() else { break };
        if prev == Some('(') && next == '*' {
            s.comment_level += 1;
        }
        if prev == Some('*') && next == ')' {
            s.comment_level -= 1;
        }
        prev = Some(next);
    }
    if s.comment_level <= 0 {
        s.tokenize = Tokenize::Base;
    }
    Some("comment")
}

/// `tokenLongString`
fn token_long_string(stream: &mut StringStream, s: &mut MlState) -> Option<&'static str> {
    let mut prev: Option<char> = None;
    while s.long_string {
        let Some(next) = stream.next() else { break };
        if prev == Some('|') && next == '}' {
            s.long_string = false;
        }
        prev = Some(next);
    }
    if !s.long_string {
        s.tokenize = Tokenize::Base;
    }
    Some("string")
}

impl Mode for MlLike {
    fn name(&self) -> &'static str {
        "mllike"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(MlState {
            tokenize: Tokenize::Base,
            comment_level: 0,
            long_string: false,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<MlState>(st);
        if stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            Tokenize::Base => self.token_base(stream, s),
            Tokenize::String => token_string(stream, s),
            Tokenize::Comment => token_comment(stream, s),
            Tokenize::LongString => token_long_string(stream, s),
        };
        style.map(str::to_string)
    }
}
