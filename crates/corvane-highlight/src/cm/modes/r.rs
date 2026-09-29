//! `codemirror/mode/r/r.js` (`text/x-rsrc`).
//!
//! Ported line by line; the JS function names are kept on the Rust ones.
//! The context stack is kept because `=` directly inside a call's
//! parentheses is styled `arg-is` from `state.ctx.argList`. Like the JS,
//! `setFlag` rebuilds the context without `argList`, while the start-of-line
//! `flags |= ALIGN_NO` mutates it in place and keeps it. Indentation and
//! column bookkeeping only feed the editor's `indent` hook and are left out.
//! JS keeps `curPunc` in the mode closure, reset only by `tokenBase`; it is
//! a state field here.

use super::super::{Mode, ModeState, StringStream, state};
use crate::re;

const ALIGN_YES: u8 = 1;
const ALIGN_NO: u8 = 2;
const BRACELESS: u8 = 4;

/// A `state.ctx` entry (`type`, `flags`, `argList`).
#[derive(Clone)]
struct Ctx {
    ty: &'static str,
    flags: u8,
    arg_list: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    /// `tokenBase`
    Base,
    /// `tokenString(quote)`
    Str(char),
}

#[derive(Clone)]
struct RState {
    tokenize: Tokenize,
    /// `state.ctx` and its `prev` chain; the last entry is `state.ctx`
    ctx: Vec<Ctx>,
    after_ident: bool,
    cur_punc: Option<&'static str>,
}

impl RState {
    fn ctx(&mut self) -> &mut Ctx {
        let last = self.ctx.len() - 1;
        &mut self.ctx[last]
    }
    // push(state, type, stream)
    fn push(&mut self, ty: &'static str) {
        self.ctx.push(Ctx {
            ty,
            flags: 0,
            arg_list: false,
        });
    }
    // setFlag(state, flag): a new ctx object, without `argList`
    fn set_flag(&mut self, flag: u8) {
        let ctx = self.ctx();
        ctx.flags |= flag;
        ctx.arg_list = false;
    }
    // pop(state)
    fn pop(&mut self) {
        if self.ctx.len() > 1 {
            self.ctx.pop();
        }
    }
}

/// JS `/\d/`
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// `opChars`: `/[+\-*\/^<>=!&|~$:]/`
fn is_op_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '*' | '/' | '^' | '<' | '>' | '=' | '!' | '&' | '|' | '~' | '$' | ':'
    )
}

/// `atoms`, `keywords` and `builtins`
fn word_style(word: &str) -> &'static str {
    match word {
        "NULL" | "NA" | "Inf" | "NaN" | "NA_integer_" | "NA_real_" | "NA_complex_"
        | "NA_character_" | "TRUE" | "FALSE" => "atom",
        "if" | "else" | "repeat" | "while" | "function" | "for" | "in" | "next" | "break" => {
            "keyword"
        }
        "list" | "quote" | "bquote" | "eval" | "return" | "call" | "parse" | "deparse" => "builtin",
        _ => "variable",
    }
}

/// `blockkeywords`
fn is_block_keyword(word: &str) -> bool {
    matches!(
        word,
        "if" | "else" | "repeat" | "while" | "function" | "for"
    )
}

// tokenBase
fn token_base(stream: &mut StringStream, state: &mut RState) -> Option<&'static str> {
    state.cur_punc = None;
    let ch = stream.next()?;
    if ch == '#' {
        stream.skip_to_end();
        Some("comment")
    } else if ch == '0' && stream.eat('x').is_some() {
        stream.eat_while_if(|c| c.is_ascii_hexdigit());
        Some("number")
    } else if ch == '.' && stream.eat_if(is_digit).is_some() {
        stream.matches(re!(r"^[0-9]*(?:e[+\-]?[0-9]+)?"));
        Some("number")
    } else if is_digit(ch) {
        stream.matches(re!(r"^[0-9]*(?:\.[0-9]+)?(?:e[+\-][0-9]+)?L?"));
        Some("number")
    } else if ch == '\'' || ch == '"' {
        state.tokenize = Tokenize::Str(ch);
        Some("string")
    } else if ch == '`' {
        stream.matches(re!(r"[^`]+`"));
        Some("variable-3")
    } else if ch == '.'
        // JS `.` (one UTF-16 unit, no line terminators; a lone surrogate
        // can never be followed by `[.]` or a digit)
        && stream.matches(re!(r"^[^\n\r\u{2028}\u{2029}\u{10000}-\u{10FFFF}](?:[.]|[0-9]+)"))
    {
        Some("keyword")
    } else if ch.is_ascii_alphabetic() || ch == '.' {
        stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
        let style = word_style(stream.current());
        if style == "keyword" {
            // Block keywords start new blocks, except 'else if', which only starts
            // one new block for the 'if', no block for the 'else'.
            if is_block_keyword(stream.current())
                && stream.match_re(re!(r"\s*if(\s+|$)"), false).is_none()
            {
                state.cur_punc = Some("block");
            }
        }
        Some(style)
    } else if ch == '%' {
        if stream.skip_to('%') {
            stream.next();
        }
        Some("operator variable-2")
    } else if (ch == '<' && stream.eat('-').is_some())
        || (ch == '<' && stream.match_str("<-", true, false))
        || (ch == '-' && stream.matches(re!(r">>?")))
    {
        Some("operator arrow")
    } else if ch == '=' && state.ctx().arg_list {
        Some("arg-is")
    } else if is_op_char(ch) {
        if ch == '$' {
            return Some("operator dollar");
        }
        stream.eat_while_if(is_op_char);
        Some("operator")
    } else if let Some(punc) = match ch {
        '(' => Some("("),
        ')' => Some(")"),
        '{' => Some("{"),
        '}' => Some("}"),
        '[' => Some("["),
        ']' => Some("]"),
        ';' => Some(";"),
        _ => None,
    } {
        state.cur_punc = Some(punc);
        if ch == ';' {
            return Some("semi");
        }
        None
    } else {
        None
    }
}

// tokenString(quote)
fn token_string(
    quote: char,
    stream: &mut StringStream,
    state: &mut RState,
) -> Option<&'static str> {
    if stream.eat('\\').is_some() {
        let ch = stream.next();
        if ch == Some('x') {
            stream.matches(re!(r"^[a-fA-F0-9]{2}"));
        } else if (ch == Some('u') || ch == Some('U'))
            && stream.eat('{').is_some()
            && stream.skip_to('}')
        {
            stream.next();
        } else if ch == Some('u') {
            stream.matches(re!(r"^[a-fA-F0-9]{4}"));
        } else if ch == Some('U') {
            stream.matches(re!(r"^[a-fA-F0-9]{8}"));
        } else if ch.is_some_and(|c| ('0'..='7').contains(&c)) {
            stream.matches(re!(r"^[0-7]{1,2}"));
        }
        Some("string-2")
    } else {
        while let Some(next) = stream.next() {
            if next == quote {
                state.tokenize = Tokenize::Base;
                break;
            }
            if next == '\\' {
                stream.back_up(1);
                break;
            }
        }
        Some("string")
    }
}

pub struct R;

impl Mode for R {
    fn name(&self) -> &'static str {
        "r"
    }

    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(RState {
            tokenize: Tokenize::Base,
            ctx: vec![Ctx {
                ty: "top",
                flags: ALIGN_NO,
                arg_list: false,
            }],
            after_ident: false,
            cur_punc: None,
        })
    }

    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let state = state::<RState>(st);
        if stream.sol() {
            if state.ctx().flags & 3 == 0 {
                state.ctx().flags |= ALIGN_NO;
            }
            if state.ctx().flags & BRACELESS != 0 {
                state.pop();
            }
        }
        if stream.eat_space() {
            return None;
        }
        let style = match state.tokenize {
            Tokenize::Base => token_base(stream, state),
            Tokenize::Str(quote) => token_string(quote, stream, state),
        };
        if style != Some("comment") && state.ctx().flags & ALIGN_NO == 0 {
            state.set_flag(ALIGN_YES);
        }

        let cur_punc = state.cur_punc;
        if matches!(cur_punc, Some(";" | "{" | "}")) && state.ctx().ty == "block" {
            state.pop();
        }
        match cur_punc {
            Some("{") => state.push("}"),
            Some("(") => {
                state.push(")");
                if state.after_ident {
                    state.ctx().arg_list = true;
                }
            }
            Some("[") => state.push("]"),
            Some("block") => state.push("block"),
            Some(p) if p == state.ctx().ty => state.pop(),
            _ => {
                if state.ctx().ty == "block" && style != Some("comment") {
                    state.set_flag(BRACELESS);
                }
            }
        }
        state.after_ident = matches!(style, Some("variable" | "keyword"));
        style.map(str::to_string)
    }
}
