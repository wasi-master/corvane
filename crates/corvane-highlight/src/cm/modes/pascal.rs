//! `codemirror/mode/pascal/pascal.js` (`text/x-pascal`), ported line by
//! line. GHD maps `.pas` to it.
//!
//! Keywords are case-sensitive (lower case only), as in the JS. The JS
//! checks `state.startOfLine` for `#` directives but never sets it, so `#`
//! is never `meta`: `#13` comes out as a `variable`, and so does this port.

use crate::cm::{Mode, ModeState, StringStream, state};

pub struct Pascal;

#[derive(Clone, Copy, PartialEq)]
enum Tokenize {
    /// `tokenBase` (the JS `null` tokenizer)
    Base,
    /// `tokenString(quote)`
    Str(char),
    /// `tokenComment`: `(* … *)`
    Comment,
    /// `tokenCommentBraces`: `{ … }`
    CommentBraces,
}

#[derive(Clone)]
struct PascalState {
    tokenize: Tokenize,
}

// `keywords`
fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "absolute"
            | "and"
            | "array"
            | "asm"
            | "begin"
            | "case"
            | "const"
            | "constructor"
            | "destructor"
            | "div"
            | "do"
            | "downto"
            | "else"
            | "end"
            | "file"
            | "for"
            | "function"
            | "goto"
            | "if"
            | "implementation"
            | "in"
            | "inherited"
            | "inline"
            | "interface"
            | "label"
            | "mod"
            | "nil"
            | "not"
            | "object"
            | "of"
            | "operator"
            | "or"
            | "packed"
            | "procedure"
            | "program"
            | "record"
            | "reintroduce"
            | "repeat"
            | "self"
            | "set"
            | "shl"
            | "shr"
            | "string"
            | "then"
            | "to"
            | "type"
            | "unit"
            | "until"
            | "uses"
            | "var"
            | "while"
            | "with"
            | "xor"
            | "as"
            | "class"
            | "dispinterface"
            | "except"
            | "exports"
            | "finalization"
            | "finally"
            | "initialization"
            | "is"
            | "library"
            | "on"
            | "out"
            | "property"
            | "raise"
            | "resourcestring"
            | "threadvar"
            | "try"
            | "abstract"
            | "alias"
            | "assembler"
            | "bitpacked"
            | "break"
            | "cdecl"
            | "continue"
            | "cppdecl"
            | "cvar"
            | "default"
            | "deprecated"
            | "dynamic"
            | "enumerator"
            | "experimental"
            | "export"
            | "external"
            | "far"
            | "far16"
            | "forward"
            | "generic"
            | "helper"
            | "implements"
            | "index"
            | "interrupt"
            | "iocheck"
            | "local"
            | "message"
            | "name"
            | "near"
            | "nodefault"
            | "noreturn"
            | "nostackframe"
            | "oldfpccall"
            | "otherwise"
            | "overload"
            | "override"
            | "pascal"
            | "platform"
            | "private"
            | "protected"
            | "public"
            | "published"
            | "read"
            | "register"
            | "result"
            | "safecall"
            | "saveregisters"
            | "softfloat"
            | "specialize"
            | "static"
            | "stdcall"
            | "stored"
            | "strict"
            | "unaligned"
            | "unimplemented"
            | "varargs"
            | "virtual"
            | "write"
    )
}

/// `isOperatorChar = /[+\-*&%=<>!?|\/]/`
fn is_operator_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '*' | '&' | '%' | '=' | '<' | '>' | '!' | '?' | '|' | '/'
    )
}

/// `tokenBase`
fn token_base(stream: &mut StringStream, s: &mut PascalState) -> Option<&'static str> {
    let ch = stream.next()?;
    // (`ch == "#" && state.startOfLine`: never true, see the module doc)
    if ch == '"' || ch == '\'' {
        s.tokenize = Tokenize::Str(ch);
        return token_string(ch, stream, s);
    }
    if ch == '(' && stream.eat('*').is_some() {
        s.tokenize = Tokenize::Comment;
        return token_comment(stream, s);
    }
    if ch == '{' {
        s.tokenize = Tokenize::CommentBraces;
        return token_comment_braces(stream, s);
    }
    if matches!(ch, '[' | ']' | '(' | ')' | ',' | ';' | ':' | '.') {
        return None;
    }
    if ch.is_ascii_digit() {
        stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
        return Some("number");
    }
    if ch == '/' && stream.eat('/').is_some() {
        stream.skip_to_end();
        return Some("comment");
    }
    if is_operator_char(ch) {
        stream.eat_while_if(is_operator_char);
        return Some("operator");
    }
    stream.eat_while_if(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    let cur = stream.current();
    if is_keyword(cur) {
        return Some("keyword");
    }
    if cur == "null" {
        return Some("atom");
    }
    Some("variable")
}

/// `tokenString(quote)`
fn token_string(
    quote: char,
    stream: &mut StringStream,
    s: &mut PascalState,
) -> Option<&'static str> {
    let mut escaped = false;
    let mut end = false;
    while let Some(next) = stream.next() {
        if next == quote && !escaped {
            end = true;
            break;
        }
        escaped = !escaped && next == '\\';
    }
    if end || !escaped {
        s.tokenize = Tokenize::Base;
    }
    Some("string")
}

/// `tokenComment`
fn token_comment(stream: &mut StringStream, s: &mut PascalState) -> Option<&'static str> {
    let mut maybe_end = false;
    while let Some(ch) = stream.next() {
        if ch == ')' && maybe_end {
            s.tokenize = Tokenize::Base;
            break;
        }
        maybe_end = ch == '*';
    }
    Some("comment")
}

/// `tokenCommentBraces`
fn token_comment_braces(stream: &mut StringStream, s: &mut PascalState) -> Option<&'static str> {
    while let Some(ch) = stream.next() {
        if ch == '}' {
            s.tokenize = Tokenize::Base;
            break;
        }
    }
    Some("comment")
}

impl Mode for Pascal {
    fn name(&self) -> &'static str {
        "pascal"
    }

    // startState
    fn start_state(&self) -> Box<dyn ModeState> {
        Box::new(PascalState {
            tokenize: Tokenize::Base,
        })
    }

    // token
    fn token(&self, stream: &mut StringStream, st: &mut dyn ModeState) -> Option<String> {
        let s = state::<PascalState>(st);
        if stream.eat_space() {
            return None;
        }
        let style = match s.tokenize {
            Tokenize::Base => token_base(stream, s),
            Tokenize::Str(q) => token_string(q, stream, s),
            Tokenize::Comment => token_comment(stream, s),
            Tokenize::CommentBraces => token_comment_braces(stream, s),
        };
        style.map(str::to_string)
    }
}
